//! Maintained corpus batch input for the native admission operation.
//!
//! A checked batch is only a proposal. Full candidate validation and the
//! accepted-pointer transaction belong to the native admission caller.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, RelativePath, canonical_bytes_v1, parse_json,
};

pub(crate) fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}
pub(crate) fn active(deadline: Instant, cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "source admission cancelled",
        ));
    }
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "source admission deadline exceeded",
        ));
    }
    Ok(())
}

/// One operation-wide profile, validated before opening batch or source data.
#[derive(Clone, Copy)]
pub struct AdmissionLimits {
    pub max_batch_bytes: usize,
    pub max_members: usize,
    pub max_member_bytes: u64,
    pub max_source_bytes: u64,
    pub json: JsonLimits,
}
impl AdmissionLimits {
    pub fn validate(self) -> io::Result<Self> {
        if self.max_batch_bytes == 0
            || self.max_batch_bytes == usize::MAX
            || self.max_members == 0
            || self.max_members == usize::MAX
            || self.max_member_bytes == 0
            || self.max_member_bytes == u64::MAX
            || self.max_source_bytes == 0
            || self.max_source_bytes == u64::MAX
            || self.max_member_bytes > self.max_source_bytes
            || self.json.max_bytes < self.max_batch_bytes
        {
            return Err(invalid("invalid source admission limits"));
        }
        JsonLimits::new(
            self.json.max_bytes,
            self.json.max_depth,
            self.json.max_visits,
            self.json.max_integer_digits,
        )
        .map_err(invalid)?;
        Ok(self)
    }
}

pub struct SourceUpdate {
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub mode: u32,
}
pub struct SourceRetirement {
    pub event_ref: RelativePath,
    pub event_sha256: Digest256,
}
pub struct AdmissionBatch {
    pub batch_sha256: Digest256,
    pub base_revision: Option<Digest256>,
    pub validator_sha256: Digest256,
    pub updates: BTreeMap<String, SourceUpdate>,
    pub retirements: BTreeMap<String, SourceRetirement>,
    input: File,
    bytes_read: u64,
    budgeted_io: Option<tos_source_store::PinnedSqliteIoBudget>,
}

fn keys(value: &Value, expected: &[&str]) -> io::Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("source batch object required"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("unexpected source batch fields"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| invalid(format!("source batch {key} must be text")))
}
fn digest(text: &str) -> io::Result<Digest256> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("source batch requires a lowercase SHA-256 digest"));
    }
    Digest256::from_hex(text).map_err(invalid)
}
fn source_path(text: &str) -> io::Result<RelativePath> {
    let path = RelativePath::parse(text).map_err(invalid)?;
    if !tos_source_store::is_authored_source_path_v1(text) {
        return Err(invalid(
            "source batch member is outside the authored source boundary",
        ));
    }
    Ok(path)
}
fn open_member(
    root: &File,
    path: &RelativePath,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<File> {
    active(deadline, cancel)?;
    let mut directory = root.try_clone()?;
    let mut pieces = path.as_str().split('/').peekable();
    while let Some(piece) = pieces.next() {
        active(deadline, cancel)?;
        if pieces.peek().is_none() {
            let file =
                tos_fd_open::open_regular_at(&directory, Path::new(piece)).map_err(invalid)?;
            active(deadline, cancel)?;
            return Ok(file);
        }
        directory =
            tos_fd_open::open_directory_at(&directory, Path::new(piece)).map_err(invalid)?;
    }
    Err(invalid("empty source member path"))
}
fn stamp(metadata: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

impl AdmissionBatch {
    /// Construct the same canonical proposal shape as `read_budgeted` after a
    /// protected owner has derived its rows from a complete held-root census.
    /// This does not create an admission witness: the normal candidate and
    /// Native validator still consume the batch, and the input root remains a
    /// held directory descriptor.
    pub(crate) fn from_verified_rows(
        base_revision: Digest256,
        validator_sha256: Digest256,
        updates: BTreeMap<String, SourceUpdate>,
        input: File,
        limits: AdmissionLimits,
        additional_state_bytes: usize,
        io: &tos_source_store::PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        active(deadline, cancel)?;
        if updates.is_empty() || updates.len() > limits.max_members {
            return Err(invalid("verified source update count exceeds profile"));
        }
        // The caller subtracts its live census, Work inspection, retained base
        // and update map from the original operation state ledger before
        // supplying this slice. Bound additional UTF-16 strings, object/array
        // capacity, transient path parsing and canonical visitor workspace
        // before any path clone or JSON node allocation. The fixed per-row
        // term covers four field nodes/keys, digest, numeric lexemes, Vec
        // growth slack and the visitor's sorted references. No output Vec is
        // retained: the maintained canonical visitor feeds the digest directly.
        if additional_state_bytes == 0 || additional_state_bytes == usize::MAX {
            return Err(invalid(
                "verified batch requires a finite original state slice",
            ));
        }
        let workspace = updates.keys().try_fold(8192usize, |total, path| {
            active(deadline, cancel)?;
            path.len()
                .checked_mul(16)
                .and_then(|bytes| bytes.checked_add(2048))
                .and_then(|bytes| total.checked_add(bytes))
                .filter(|bytes| *bytes <= additional_state_bytes)
                .ok_or_else(|| invalid("verified batch original state slice exhausted"))
        })?;
        if workspace > additional_state_bytes {
            return Err(invalid("verified batch original state slice exhausted"));
        }
        io.charge_read_upper_bound(4096).map_err(invalid)?;
        if !input.metadata()?.is_dir() {
            return Err(invalid("verified batch input must be a held directory"));
        }
        let mut declared = 0u64;
        let rows = updates
            .iter()
            .map(|(path, row)| -> io::Result<_> {
                active(deadline, cancel)?;
                let parsed = source_path(path)?;
                if parsed.as_str() != path
                    || row.size_bytes > limits.max_member_bytes
                    || !matches!(row.mode, 0o600 | 0o644 | 0o755)
                {
                    return Err(invalid("verified source update differs from profile"));
                }
                declared = declared
                    .checked_add(row.size_bytes)
                    .filter(|bytes| *bytes <= limits.max_source_bytes)
                    .ok_or_else(|| invalid("verified source aggregate exceeds profile"))?;
                Ok(crate::source_command::object(vec![
                    ("path", crate::source_command::string(path)),
                    (
                        "sha256",
                        crate::source_command::string(&row.sha256.to_hex()),
                    ),
                    ("size_bytes", crate::source_command::number(row.size_bytes)),
                    ("mode", crate::source_command::number(u64::from(row.mode))),
                ]))
            })
            .collect::<io::Result<Vec<_>>>()?;
        let value = crate::source_command::object(vec![
            (
                "schema_version",
                crate::source_command::string("tos_corpus_batch_v1"),
            ),
            (
                "base_revision",
                crate::source_command::string(&base_revision.to_hex()),
            ),
            (
                "validator_sha256",
                crate::source_command::string(&validator_sha256.to_hex()),
            ),
            ("updates", crate::source_command::JsonValue::Array(rows)),
            (
                "retirements",
                crate::source_command::JsonValue::Array(Vec::new()),
            ),
        ]);
        let mut hasher = tos_foundation::Digest256Hasher::new();
        let mut written = 0usize;
        let mut visits = 0usize;
        let mut json_limits = limits.json;
        json_limits.max_bytes = limits.max_batch_bytes;
        tos_foundation::canonical_feed_digest_v1(
            &value,
            CanonicalProfile::CorpusSnapshotV1,
            json_limits,
            &mut hasher,
            &mut written,
            &mut visits,
            0,
        )
        .map_err(invalid)?;
        active(deadline, cancel)?;
        Ok(Self {
            batch_sha256: hasher.finalize(),
            base_revision: Some(base_revision),
            validator_sha256,
            updates,
            retirements: BTreeMap::new(),
            input,
            // The rows were derived from held source bytes, not read from an
            // external JSON batch. Payload reads are charged when the usual
            // candidate opens each update.
            bytes_read: 0,
            budgeted_io: Some(io.clone()),
        })
    }

    pub(crate) fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
    pub(crate) fn shares_spooled_io_budget(
        &self,
        budget: &tos_source_store::PinnedSqliteIoBudget,
    ) -> bool {
        self.budgeted_io
            .as_ref()
            .is_some_and(|selected| selected.shares_with(budget))
    }
    /// Bind the strict canonical maintained batch and an exact no-follow input
    /// directory. No store, objects, revision or pointer is created here.
    pub fn read(
        batch_path: &Path,
        input_root: &Path,
        limits: AdmissionLimits,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        Self::read_with_budget(batch_path, input_root, limits, deadline, cancel, None)
    }
    pub(crate) fn read_budgeted(
        batch_path: &Path,
        input_root: &Path,
        limits: AdmissionLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        io: &tos_source_store::PinnedSqliteIoBudget,
    ) -> io::Result<Self> {
        Self::read_with_budget(batch_path, input_root, limits, deadline, cancel, Some(io))
    }
    fn read_with_budget(
        batch_path: &Path,
        input_root: &Path,
        limits: AdmissionLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        io: Option<&tos_source_store::PinnedSqliteIoBudget>,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        active(deadline, cancel)?;
        let mut file =
            tos_fd_open::open_absolute_regular(batch_path, limits.max_batch_bytes as u64)
                .map_err(invalid)?;
        let before = file.metadata()?;
        let mut raw = Vec::with_capacity(before.len() as usize);
        let mut chunk = [0; 65536];
        loop {
            active(deadline, cancel)?;
            if let Some(io) = io {
                io.charge_read(chunk.len() as u64).map_err(invalid)?;
            }
            let count = file.read(&mut chunk)?;
            if let Some(io) = io {
                io.record_read_returned(count as u64).map_err(invalid)?;
            }
            active(deadline, cancel)?;
            if count == 0 {
                break;
            }
            if count > limits.max_batch_bytes.saturating_sub(raw.len()) {
                return Err(invalid("source batch byte limit exceeded"));
            }
            raw.extend_from_slice(&chunk[..count]);
        }
        if stamp(&before) != stamp(&file.metadata()?) {
            return Err(invalid("source batch changed while reading"));
        }
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits.json).map_err(invalid)?;
        let canonical = canonical_bytes_v1(
            parsed.root(),
            CanonicalProfile::CorpusSnapshotV1,
            limits.json,
        )
        .map_err(invalid)?;
        if canonical != raw {
            return Err(invalid("source batch must be canonical JSON"));
        }
        let value: Value = serde_json::from_slice(&raw).map_err(invalid)?;
        keys(
            &value,
            &[
                "schema_version",
                "base_revision",
                "validator_sha256",
                "updates",
                "retirements",
            ],
        )?;
        if text(&value, "schema_version")? != "tos_corpus_batch_v1" {
            return Err(invalid("unsupported source batch"));
        }
        let base_revision = if value["base_revision"].is_null() {
            None
        } else {
            Some(digest(text(&value, "base_revision")?)?)
        };
        let validator_sha256 = digest(text(&value, "validator_sha256")?)?;
        let update_rows = value["updates"]
            .as_array()
            .ok_or_else(|| invalid("source updates must be an array"))?;
        let retirement_rows = value["retirements"]
            .as_array()
            .ok_or_else(|| invalid("source retirements must be an array"))?;
        if update_rows
            .len()
            .checked_add(retirement_rows.len())
            .is_none_or(|n| n > limits.max_members)
        {
            return Err(invalid("source batch member limit exceeded"));
        }
        let input = tos_fd_open::open_absolute_directory(input_root).map_err(invalid)?;
        let mut updates = BTreeMap::new();
        let mut declared = 0u64;
        for row in update_rows {
            active(deadline, cancel)?;
            keys(row, &["path", "sha256", "size_bytes", "mode"])?;
            let path = source_path(text(row, "path")?)?;
            let size_bytes = row["size_bytes"]
                .as_u64()
                .filter(|n| *n <= limits.max_member_bytes)
                .ok_or_else(|| invalid("invalid source update byte size"))?;
            declared = declared
                .checked_add(size_bytes)
                .filter(|n| *n <= limits.max_source_bytes)
                .ok_or_else(|| invalid("source batch aggregate byte limit exceeded"))?;
            let mode = row["mode"]
                .as_u64()
                .filter(|n| matches!(*n, 0o600 | 0o644 | 0o755))
                .ok_or_else(|| invalid("invalid source update mode"))?
                as u32;
            let update = SourceUpdate {
                sha256: digest(text(row, "sha256")?)?,
                size_bytes,
                mode,
            };
            if updates.insert(path.as_str().to_owned(), update).is_some() {
                return Err(invalid("duplicate source update"));
            }
            let source = open_member(&input, &path, deadline, cancel)?;
            if source.metadata()?.len() != size_bytes {
                return Err(invalid("source update size differs from declared bytes"));
            }
        }
        let mut retirements = BTreeMap::new();
        for row in retirement_rows {
            active(deadline, cancel)?;
            keys(row, &["path", "event_ref", "event_sha256"])?;
            let path = source_path(text(row, "path")?)?;
            let retirement = SourceRetirement {
                event_ref: source_path(text(row, "event_ref")?)?,
                event_sha256: digest(text(row, "event_sha256")?)?,
            };
            if updates.contains_key(path.as_str())
                || retirements
                    .insert(path.as_str().to_owned(), retirement)
                    .is_some()
            {
                return Err(invalid("duplicate or conflicting source retirement"));
            }
        }
        active(deadline, cancel)?;
        Ok(Self {
            batch_sha256: Digest256::of_bytes(&canonical),
            base_revision,
            validator_sha256,
            updates,
            retirements,
            input,
            bytes_read: raw.len() as u64,
            budgeted_io: io.cloned(),
        })
    }

    /// The writer must stream, hash and recheck this held regular inode before
    /// installing an immutable object; successful open alone is not fixity.
    pub(crate) fn open_update(
        &self,
        relative: &str,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<File> {
        if !self.updates.contains_key(relative) {
            return Err(invalid("update not in checked batch"));
        }
        open_member(&self.input, &source_path(relative)?, deadline, cancel)
    }
}
