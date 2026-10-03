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
