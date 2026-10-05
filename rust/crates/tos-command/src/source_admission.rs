//! Maintained corpus batch input for the native admission operation.
//!
//! A checked batch is only a proposal. Full candidate validation and the
//! accepted-pointer transaction belong to the native admission caller.
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fs::File,
    io::{self, Read, Write},
    mem::size_of,
    os::unix::fs::MetadataExt,
    path::Path,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, FoundationError, FoundationErrorCode, JsonLimits,
    JsonMode, JsonValue, RelativePath, canonical_bytes_v1, parse_json,
};
use tos_source_store::PinnedSqliteConnection;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    initial_updates: Option<Rc<VerifiedCensusUpdates>>,
    selected_work_budget: Option<AdmissionWorkBudget>,
    indexed_input: Option<crate::source_admission_indexed_input::IndexedInputReaderV1>,
    indexed_input_heap_state_upper_bound_bytes: usize,
    source_stream_failed: bool,
    source_stream_finished: bool,
    input: File,
    bytes_read: u64,
    budgeted_io: Option<tos_source_store::PinnedSqliteIoBudget>,
}

/// Counters returned by one source update stream. They report the source-byte
/// count and deltas from the same original IO/work ledgers across its census
/// lookup and payload operation; they are not independent budgets. The caller
/// owns sink-write accounting. Root and terminal descriptor fences run outside
/// this per-update window but remain on the shared ledgers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AdmissionUpdateStreamWorkV1 {
    pub(crate) source_bytes: u64,
    pub(crate) read_bytes: u64,
    pub(crate) read_upper_bound_bytes: u64,
    pub(crate) work_units: u64,
}

#[derive(Clone)]
pub(crate) struct AdmissionWorkBudget {
    inner: Rc<AdmissionWorkState>,
}

struct AdmissionWorkState {
    used: Cell<u64>,
    maximum: Cell<u64>,
}

impl AdmissionWorkBudget {
    pub(crate) fn new(maximum: u64) -> io::Result<Self> {
        if maximum == 0 || maximum == u64::MAX {
            return Err(invalid("selected source work bound is not finite"));
        }
        Ok(Self {
            inner: Rc::new(AdmissionWorkState {
                used: Cell::new(0),
                maximum: Cell::new(maximum),
            }),
        })
    }

    pub(crate) fn charge(&self, _kind: impl Copy) -> io::Result<()> {
        self.charge_many(1)
    }

    pub(crate) fn charge_many(&self, count: u64) -> io::Result<()> {
        let used = self
            .inner
            .used
            .get()
            .checked_add(count)
            .filter(|used| *used <= self.inner.maximum.get())
            .ok_or_else(|| invalid("selected source work ceiling exhausted"))?;
        self.inner.used.set(used);
        Ok(())
    }

    pub(crate) fn used(&self) -> u64 {
        self.inner.used.get()
    }

    pub(crate) fn remaining(&self) -> io::Result<u64> {
        self.inner
            .maximum
            .get()
            .checked_sub(self.inner.used.get())
            .ok_or_else(|| invalid("selected source work meter regressed"))
    }

    pub(crate) fn maximum(&self) -> u64 {
        self.inner.maximum.get()
    }

    /// Narrow the original shared operation ceiling after identity preparation.
    /// Already charged preparation remains charged on every retained clone.
    pub(crate) fn lower_maximum(&self, maximum: u64) -> io::Result<()> {
        if maximum == 0
            || maximum == u64::MAX
            || maximum > self.inner.maximum.get()
            || maximum < self.inner.used.get()
        {
            return Err(invalid("selected source work ceiling cannot be narrowed"));
        }
        self.inner.maximum.set(maximum);
        Ok(())
    }

    pub(crate) fn retained_allocation_upper_bound_bytes() -> usize {
        size_of::<AdmissionWorkState>() + 2 * size_of::<usize>()
    }
}

struct VerifiedCensusUpdates {
    db: Rc<RefCell<PinnedSqliteConnection>>,
    scan_label: &'static str,
    expected_rows: u64,
    expected_source_bytes: u64,
    max_member_bytes: u64,
    max_path_bytes: usize,
    row_state_bytes: usize,
    work: AdmissionWorkBudget,
    identity: Rc<()>,
}

pub(crate) struct AdmissionUpdateCursor {
    source: Rc<VerifiedCensusUpdates>,
    after: Option<String>,
    seen_rows: u64,
    seen_bytes: u64,
    eof: bool,
}

pub(crate) struct AdmissionUpdateRow {
    pub(crate) path: String,
    pub(crate) update: SourceUpdate,
    pub(crate) workspace_state_bytes: usize,
    source_identity: Rc<()>,
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
    io: Option<&tos_source_store::PinnedSqliteIoBudget>,
    work: Option<&AdmissionWorkBudget>,
) -> io::Result<File> {
    active(deadline, cancel)?;
    let mut directory = root.try_clone()?;
    let mut pieces = path.as_str().split('/').peekable();
    while let Some(piece) = pieces.next() {
        active(deadline, cancel)?;
        if let Some(work) = work {
            work.charge_many(1)?;
        }
        if let Some(io) = io {
            crate::source_admission_initial_cut::charge_name_guard(io, piece)?;
        }
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
        base_revision: Option<Digest256>,
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
                base_revision.map_or(JsonValue::Null, |revision| {
                    crate::source_command::string(&revision.to_hex())
                }),
            ),
            (
                "validator_sha256",
                crate::source_command::string(&validator_sha256.to_hex()),
            ),
            ("updates", JsonValue::Array(rows)),
            ("retirements", JsonValue::Array(Vec::new())),
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
            base_revision,
            validator_sha256,
            updates,
            retirements: BTreeMap::new(),
            initial_updates: None,
            selected_work_budget: None,
            indexed_input: None,
            indexed_input_heap_state_upper_bound_bytes: 0,
            source_stream_failed: false,
            source_stream_finished: false,
            input,
            // The rows were derived from held source bytes, not read from an
            // external JSON batch. Payload reads are charged when the usual
            // candidate opens each update.
            bytes_read: 0,
            budgeted_io: Some(io.clone()),
        })
    }

    /// SourceEntry can derive a bounded verified-row batch from its selected
    /// readset and carry that invocation's original work meter into the packed
    /// successor writer. The legacy constructor remains unchanged.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_verified_rows_with_work(
        base_revision: Option<Digest256>,
        validator_sha256: Digest256,
        updates: BTreeMap<String, SourceUpdate>,
        input: File,
        limits: AdmissionLimits,
        additional_state_bytes: usize,
        io: &tos_source_store::PinnedSqliteIoBudget,
        work: AdmissionWorkBudget,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        let mut batch = Self::from_verified_rows(
            base_revision,
            validator_sha256,
            updates,
            input,
            limits,
            additional_state_bytes,
            io,
            deadline,
            cancel,
        )?;
        batch.selected_work_budget = Some(work);
        Ok(batch)
    }

    /// Construct an initial-only batch from the proposal rows already held in
    /// the caller's AUX database. The rows remain SQL-backed for the spooled
    /// candidate; only one bounded row and its keyset cursor are resident at a
    /// time. The maintained canonical writer still owns every output byte.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_verified_census_rows(
        db: Rc<RefCell<PinnedSqliteConnection>>,
        scan_label: &'static str,
        expected_rows: u64,
        expected_source_bytes: u64,
        validator_sha256: Digest256,
        input: File,
        limits: AdmissionLimits,
        max_path_bytes: usize,
        row_state_bytes: usize,
        canonical_row_state_bytes: usize,
        work: AdmissionWorkBudget,
        io: &tos_source_store::PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        active(deadline, cancel)?;
        if expected_rows == 0
            || expected_rows > limits.max_members as u64
            || expected_source_bytes > limits.max_source_bytes
            || max_path_bytes == 0
            || row_state_bytes == 0
            || canonical_row_state_bytes == 0
            || max_path_bytes
                .checked_mul(24)
                .and_then(|bytes| bytes.checked_add(4096))
                .is_none_or(|bytes| bytes > row_state_bytes)
            || max_path_bytes
                .checked_mul(24)
                .and_then(|bytes| bytes.checked_add(8192))
                .is_none_or(|bytes| bytes > canonical_row_state_bytes)
        {
            return Err(invalid(
                "verified source census exceeds its selected row profile",
            ));
        }
        io.charge_read_upper_bound(4096).map_err(invalid)?;
        if !input.metadata()?.is_dir() {
            return Err(invalid("verified source input must be a held directory"));
        }

        let value = crate::source_command::object(vec![
            (
                "schema_version",
                crate::source_command::string("tos_corpus_batch_v1"),
            ),
            ("base_revision", JsonValue::Null),
            (
                "validator_sha256",
                crate::source_command::string(&validator_sha256.to_hex()),
            ),
            ("updates", JsonValue::Array(Vec::new())),
            ("retirements", JsonValue::Array(Vec::new())),
        ]);
        let mut json_limits = limits.json;
        json_limits.max_bytes = limits.max_batch_bytes;
        let mut hasher = tos_foundation::Digest256Hasher::new();
        let mut written = 0usize;
        let mut visits = 0usize;
        let mut count = 0u64;
        let mut source_bytes = 0u64;
        let mut previous: Option<String> = None;
        let mut saw_eof = false;
        let mut feed_error = None;
        work.charge_many(2)?;
        let db_guard = db.borrow_mut();
        let mut statement = db_guard
            .prepare(
                "SELECT path,sha256,size,mode FROM source_member_census \
                 WHERE scan_label=?1 ORDER BY path COLLATE BINARY",
            )
            .map_err(|error| invalid(format!("initial census cursor refused: {error}")))?;
        let mut rows = statement
            .query(params![scan_label])
            .map_err(|error| invalid(format!("initial census rows refused: {error}")))?;
        let result = value.canonical_feed_digest_v1_with_streamed_array(
            CanonicalProfile::CorpusSnapshotV1,
            json_limits,
            "updates",
            |write_item| {
                loop {
                    if let Err(error) = active(deadline, cancel) {
                        feed_error = Some(error);
                        return Err(FoundationError::new(
                            FoundationErrorCode::InvalidJson,
                            "initial census stream stopped",
                        ));
                    }
                    if let Err(error) = work.charge_many(2) {
                        feed_error = Some(error);
                        return Err(FoundationError::new(
                            FoundationErrorCode::BudgetExceeded,
                            "initial census work ceiling exhausted",
                        ));
                    }
                    let row = match rows.next() {
                        Ok(row) => row,
                        Err(error) => {
                            feed_error =
                                Some(invalid(format!("initial census row read refused: {error}")));
                            return Err(FoundationError::new(
                                FoundationErrorCode::InvalidJson,
                                "initial census row read refused",
                            ));
                        }
                    };
                    let Some(row) = row else {
                        saw_eof = true;
                        break;
                    };
                    let path_len = match row.get_ref(0) {
                        Ok(rusqlite::types::ValueRef::Text(path)) => path.len(),
                        _ => {
                            feed_error = Some(invalid("initial census path is not text"));
                            return Err(FoundationError::new(
                                FoundationErrorCode::InvalidJson,
                                "initial census path is not text",
                            ));
                        }
                    };
                    let previous_len = previous.as_ref().map_or(0, String::len);
                    let row_workspace = path_len
                        .checked_mul(16)
                        .and_then(|bytes| bytes.checked_add(previous_len.checked_mul(8)?))
                        .and_then(|bytes| bytes.checked_add(4096))
                        .ok_or_else(|| {
                            feed_error = Some(invalid("initial census row state overflow"));
                            FoundationError::new(
                                FoundationErrorCode::BudgetExceeded,
                                "initial census row state overflow",
                            )
                        })?;
                    if path_len > max_path_bytes || row_workspace > canonical_row_state_bytes {
                        feed_error = Some(invalid("initial census row exceeds its state profile"));
                        return Err(FoundationError::new(
                            FoundationErrorCode::BudgetExceeded,
                            "initial census row exceeds its state profile",
                        ));
                    }
                    let path: String = match row.get(0) {
                        Ok(path) => path,
                        Err(error) => {
                            feed_error = Some(invalid(format!(
                                "initial census path decode refused: {error}"
                            )));
                            return Err(FoundationError::new(
                                FoundationErrorCode::InvalidJson,
                                "initial census path decode refused",
                            ));
                        }
                    };
                    if !tos_source_store::is_authored_source_path_v1(&path) {
                        feed_error = Some(invalid(
                            "initial census path is outside the authored source boundary",
                        ));
                        return Err(FoundationError::new(
                            FoundationErrorCode::UnsafePath,
                            "initial census path is outside the authored source boundary",
                        ));
                    }
                    let relative = match RelativePath::parse(&path) {
                        Ok(relative) if relative.as_str() == path.as_str() => relative,
                        _ => {
                            feed_error = Some(invalid("initial census path is not canonical"));
                            return Err(FoundationError::new(
                                FoundationErrorCode::UnsafePath,
                                "initial census path is not canonical",
                            ));
                        }
                    };
                    if previous
                        .as_deref()
                        .is_some_and(|prior| prior.as_bytes() >= path.as_bytes())
                    {
                        feed_error = Some(invalid(
                            "initial census rows are not unique and BINARY ordered",
                        ));
                        return Err(FoundationError::new(
                            FoundationErrorCode::DuplicateMember,
                            "initial census rows are not unique and BINARY ordered",
                        ));
                    }
                    let raw_digest = match row.get_ref(1) {
                        Ok(rusqlite::types::ValueRef::Blob(bytes)) if bytes.len() == 32 => bytes,
                        _ => {
                            feed_error = Some(invalid("initial census digest width differs"));
                            return Err(FoundationError::new(
                                FoundationErrorCode::InvalidDigest,
                                "initial census digest width differs",
                            ));
                        }
                    };
                    let mut digest_bytes = [0u8; 32];
                    digest_bytes.copy_from_slice(raw_digest);
                    let size = match row.get::<_, i64>(2) {
                        Ok(size) if size >= 0 => size as u64,
                        _ => {
                            feed_error = Some(invalid("initial census size range differs"));
                            return Err(FoundationError::new(
                                FoundationErrorCode::BudgetExceeded,
                                "initial census size range differs",
                            ));
                        }
                    };
                    let mode = match row.get::<_, i64>(3) {
                        Ok(mode) if matches!(mode, 0o600 | 0o644 | 0o755) => mode as u32,
                        _ => {
                            feed_error = Some(invalid("initial census mode differs"));
                            return Err(FoundationError::new(
                                FoundationErrorCode::InvalidJson,
                                "initial census mode differs",
                            ));
                        }
                    };
                    if size > limits.max_member_bytes {
                        feed_error = Some(invalid("initial census member exceeds its byte limit"));
                        return Err(FoundationError::new(
                            FoundationErrorCode::BudgetExceeded,
                            "initial census member exceeds its byte limit",
                        ));
                    }
                    count = match count.checked_add(1).filter(|count| *count <= expected_rows) {
                        Some(count) => count,
                        None => {
                            feed_error = Some(invalid("initial census row count exceeds its seal"));
                            return Err(FoundationError::new(
                                FoundationErrorCode::BudgetExceeded,
                                "initial census row count exceeds its seal",
                            ));
                        }
                    };
                    source_bytes = match source_bytes
                        .checked_add(size)
                        .filter(|bytes| *bytes <= limits.max_source_bytes)
                    {
                        Some(bytes) => bytes,
                        None => {
                            feed_error =
                                Some(invalid("initial census source bytes exceed profile"));
                            return Err(FoundationError::new(
                                FoundationErrorCode::BudgetExceeded,
                                "initial census source bytes exceed profile",
                            ));
                        }
                    };
                    let update = SourceUpdate {
                        sha256: Digest256::from_bytes(digest_bytes),
                        size_bytes: size,
                        mode,
                    };
                    let item = crate::source_command::object(vec![
                        ("path", crate::source_command::string(relative.as_str())),
                        (
                            "sha256",
                            crate::source_command::string(&update.sha256.to_hex()),
                        ),
                        ("size_bytes", crate::source_command::number(size)),
                        (
                            "mode",
                            crate::source_command::number(u64::from(update.mode)),
                        ),
                    ]);
                    if let Err(error) = write_item(&item) {
                        feed_error = Some(invalid(&error));
                        return Err(error);
                    }
                    previous = Some(path);
                }
                Ok(())
            },
            &mut hasher,
            &mut written,
            &mut visits,
            0,
        );
        drop(rows);
        drop(statement);
        drop(db_guard);
        if let Some(error) = feed_error {
            return Err(error);
        }
        result.map_err(invalid)?;
        active(deadline, cancel)?;
        if !saw_eof
            || count != expected_rows
            || source_bytes != expected_source_bytes
            || count > limits.max_members as u64
        {
            return Err(invalid("initial census ordered EOF totals differ"));
        }
        Ok(Self {
            batch_sha256: hasher.finalize(),
            base_revision: None,
            validator_sha256,
            updates: BTreeMap::new(),
            retirements: BTreeMap::new(),
            initial_updates: Some(Rc::new(VerifiedCensusUpdates {
                db,
                scan_label,
                expected_rows,
                expected_source_bytes,
                max_member_bytes: limits.max_member_bytes,
                max_path_bytes,
                row_state_bytes,
                work,
                identity: Rc::new(()),
            })),
            selected_work_budget: None,
            indexed_input: None,
            indexed_input_heap_state_upper_bound_bytes: 0,
            source_stream_failed: false,
            source_stream_finished: false,
            input,
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
            let source = open_member(&input, &path, deadline, cancel, io, None)?;
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
            initial_updates: None,
            selected_work_budget: None,
            indexed_input: None,
            indexed_input_heap_state_upper_bound_bytes: 0,
            source_stream_failed: false,
            source_stream_finished: false,
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
        open_member(
            &self.input,
            &source_path(relative)?,
            deadline,
            cancel,
            self.budgeted_io.as_ref(),
            None,
        )
    }

    pub(crate) fn has_updates(&self) -> bool {
        !self.updates.is_empty()
            || self
                .initial_updates
                .as_ref()
                .is_some_and(|rows| rows.expected_rows != 0)
    }

    pub(crate) fn is_census_backed(&self) -> bool {
        self.initial_updates.is_some()
    }

    pub(crate) fn update_source_bytes(&self) -> io::Result<u64> {
        if let Some(rows) = &self.initial_updates {
            Ok(rows.expected_source_bytes)
        } else {
            self.updates.values().try_fold(0u64, |total, update| {
                total
                    .checked_add(update.size_bytes)
                    .ok_or_else(|| invalid("admission batch source-byte total overflow"))
            })
        }
    }

    pub(crate) fn admission_work_budget(&self) -> io::Result<AdmissionWorkBudget> {
        self.selected_work_budget
            .clone()
            .or_else(|| {
                self.initial_updates
                    .as_ref()
                    .map(|updates| updates.work.clone())
            })
            .ok_or_else(|| invalid("admission batch has no selected shared work meter"))
    }

    /// Additional heap retained by the initial census-backed source. The
    /// owned cursor and batch share one Rc-carried update source so the caller
    /// can stream each row without borrowing the batch. The source carrier and
    /// row-identity Rc header are charged here; the database and work-meter
    /// allocations remain owned by the retained initial-cut fence.
    pub(crate) fn retained_update_source_state_upper_bound_bytes(&self) -> usize {
        let census_identity = if self.initial_updates.is_some() {
            size_of::<VerifiedCensusUpdates>() + 2 * size_of::<usize>()
        } else {
            0
        };
        census_identity.saturating_add(self.indexed_input_heap_state_upper_bound_bytes)
    }

    /// Attach a held packed source only to the exact census-backed initial
    /// proposal. The provider stays inside this batch so the ordinary
    /// candidate consumes its rows and payloads under the same operation.
    pub(crate) fn attach_indexed_input(
        &mut self,
        reader: crate::source_admission_indexed_input::IndexedInputReaderV1,
        expected_member_count: u64,
        expected_source_bytes: u64,
    ) -> io::Result<()> {
        let source = self
            .initial_updates
            .as_ref()
            .ok_or_else(|| invalid("indexed input requires the initial census source"))?;
        if self.indexed_input.is_some()
            || self.source_stream_failed
            || source.expected_rows != expected_member_count
            || source.expected_source_bytes != expected_source_bytes
            || reader.expected_totals() != (expected_member_count, expected_source_bytes)
        {
            return Err(invalid(
                "indexed input does not match the held initial census",
            ));
        }
        self.indexed_input_heap_state_upper_bound_bytes =
            reader.retained_heap_state_upper_bound_bytes()?;
        self.indexed_input = Some(reader);
        Ok(())
    }

    /// Stream exactly one selected update into a caller-owned sink. Indexed
    /// mode advances the authenticated path cursor and streams its packed
    /// payload; file mode retains the held-root fallback. Both paths match the
    /// census or checked batch metadata and verify digest, length, mode, and
    /// EOF before success. Source reads and work are charged here once; the
    /// sink owner charges its own writes.
    pub(crate) fn stream_verified_update(
        &mut self,
        path: &str,
        update: SourceUpdate,
        caller_live_state_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
        sink: &mut dyn Write,
    ) -> io::Result<AdmissionUpdateStreamWorkV1> {
        if self.source_stream_failed || self.source_stream_finished {
            return Err(invalid("source update stream was already refused"));
        }
        let shared_work_before = self
            .initial_updates
            .as_ref()
            .map(|source| source.work.remaining())
            .transpose()?;
        let io_before = self.budgeted_io.as_ref().map(|io| io.snapshot());
        let result = (|| {
            active(deadline, cancel)?;
            let expected = self
                .update(path, caller_live_state_bytes)?
                .ok_or_else(|| invalid("streamed update is absent from the selected batch"))?;
            if expected != update {
                return Err(invalid(
                    "streamed update differs from selected census metadata",
                ));
            }

            if let Some(reader) = self.indexed_input.as_mut() {
                let before = reader.cost();
                let member = reader
                    .next_member(caller_live_state_bytes)?
                    .ok_or_else(|| invalid("indexed input ended before the update cursor"))?;
                if member.path.as_str() != path
                    || member.sha256 != update.sha256
                    || member.size_bytes != update.size_bytes
                    || member.source_mode != update.mode
                {
                    return Err(invalid(
                        "indexed input member differs from the held census row",
                    ));
                }
                let member_size_bytes = member.size_bytes;
                reader.read_member_payload(
                    &member,
                    caller_live_state_bytes
                        .checked_add(size_of::<&mut dyn Write>() + size_of::<Digest256Hasher>())
                        .ok_or_else(|| invalid("indexed input sink state overflow"))?,
                    sink,
                )?;
                if reader.cost().payload_members == reader.expected_totals().0 {
                    drop(member);
                    if reader.next_member(caller_live_state_bytes)?.is_some() {
                        return Err(invalid("indexed input has rows beyond the selected census"));
                    }
                }
                let after = reader.cost();
                let tree_read_bytes = after
                    .member_tree
                    .read_bytes
                    .checked_sub(before.member_tree.read_bytes)
                    .ok_or_else(|| invalid("indexed input tree read counter moved backwards"))?;
                let object_read_bytes = after
                    .object_reads
                    .read_bytes
                    .checked_sub(before.object_reads.read_bytes)
                    .ok_or_else(|| invalid("indexed input object read counter moved backwards"))?;
                let object_tree_read_bytes = after
                    .object_tree
                    .read_bytes
                    .checked_sub(before.object_tree.read_bytes)
                    .ok_or_else(|| invalid("indexed input object-tree counter moved backwards"))?;
                let read_upper_bound_bytes = after
                    .object_reads
                    .read_upper_bound_bytes
                    .checked_sub(before.object_reads.read_upper_bound_bytes)
                    .ok_or_else(|| invalid("indexed input read guard counter moved backwards"))?;
                let work_units = after
                    .shared_work_units
                    .checked_sub(before.shared_work_units)
                    .ok_or_else(|| invalid("indexed input work counter moved backwards"))?;
                Ok(AdmissionUpdateStreamWorkV1 {
                    source_bytes: member_size_bytes,
                    read_bytes: tree_read_bytes
                        .checked_add(object_read_bytes)
                        .and_then(|bytes| bytes.checked_add(object_tree_read_bytes))
                        .ok_or_else(|| invalid("indexed input read counter overflow"))?,
                    read_upper_bound_bytes,
                    work_units,
                })
            } else {
                self.stream_file_update(path, update, deadline, cancel, sink)
            }
        })()
        .and_then(|mut streamed| {
            if let Some(before) = shared_work_before {
                let after = self
                    .initial_updates
                    .as_ref()
                    .ok_or_else(|| invalid("initial source work meter disappeared"))?
                    .work
                    .remaining()?;
                streamed.work_units = before
                    .checked_sub(after)
                    .ok_or_else(|| invalid("initial source work counter regressed"))?;
            }
            if let Some(before) = io_before {
                let after = self
                    .budgeted_io
                    .as_ref()
                    .ok_or_else(|| invalid("initial source IO ledger disappeared"))?
                    .snapshot();
                streamed.read_bytes = after
                    .read_returned_bytes
                    .checked_sub(before.read_returned_bytes)
                    .ok_or_else(|| invalid("initial source read counter regressed"))?;
                streamed.read_upper_bound_bytes = after
                    .read_upper_bound_attempted_bytes
                    .checked_sub(before.read_upper_bound_attempted_bytes)
                    .ok_or_else(|| invalid("initial source guard counter regressed"))?;
            }
            Ok(streamed)
        });
        if result.is_err() {
            self.source_stream_failed = true;
        }
        result
    }

    fn stream_file_update(
        &self,
        path: &str,
        update: SourceUpdate,
        deadline: Instant,
        cancel: &AtomicBool,
        sink: &mut dyn Write,
    ) -> io::Result<AdmissionUpdateStreamWorkV1> {
        let source_path = source_path(path)?;
        let source = self.initial_updates.as_ref();
        let io = self
            .budgeted_io
            .as_ref()
            .ok_or_else(|| invalid("streamed update lacks the original IO ledger"))?;
        let mut file = open_member(
            &self.input,
            &source_path,
            deadline,
            cancel,
            Some(io),
            source.map(|source| &source.work),
        )?;
        let before = file.metadata()?;
        if !before.is_file()
            || before.len() != update.size_bytes
            || before.mode() & 0o7777 != update.mode
        {
            return Err(invalid("streamed source inode differs from its census row"));
        }

        let read_limit = update
            .size_bytes
            .checked_add(1)
            .ok_or_else(|| invalid("streamed source size overflow"))?;
        let mut bounded = (&mut file).take(read_limit);
        let mut buffer = [0u8; 64 * 1024];
        let mut hasher = Digest256Hasher::new();
        let mut bytes_read = 0u64;
        let mut work_units = 0u64;
        loop {
            active(deadline, cancel)?;
            let remaining = read_limit
                .checked_sub(bytes_read)
                .ok_or_else(|| invalid("streamed source length overflow"))?;
            if remaining == 0 {
                break;
            }
            if let Some(source) = source {
                source.work.charge_many(1)?;
            }
            work_units = work_units
                .checked_add(1)
                .ok_or_else(|| invalid("streamed source work counter overflow"))?;
            let requested = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| invalid("streamed source read request exceeds range"))?;
            io.charge_read(requested as u64).map_err(invalid)?;
            let read = bounded.read(&mut buffer[..requested]);
            let count = match read {
                Ok(count) => count,
                Err(error) => return Err(error),
            };
            io.record_read_returned(count as u64).map_err(invalid)?;
            active(deadline, cancel)?;
            if count == 0 {
                break;
            }
            let next = bytes_read
                .checked_add(count as u64)
                .ok_or_else(|| invalid("streamed source byte count overflow"))?;
            if next > update.size_bytes {
                return Err(invalid("streamed source exceeds its declared size"));
            }
            hasher.update(&buffer[..count]);
            sink.write_all(&buffer[..count])?;
            bytes_read = next;
        }
        if bytes_read != update.size_bytes || hasher.finalize() != update.sha256 {
            return Err(invalid("streamed source fixity, size, or EOF differs"));
        }
        active(deadline, cancel)?;
        let after = file.metadata()?;
        if stamp(&before) != stamp(&after) || before.mode() & 0o7777 != after.mode() & 0o7777 {
            return Err(invalid("streamed source inode changed while reading"));
        }
        let named = open_member(
            &self.input,
            &source_path,
            deadline,
            cancel,
            Some(io),
            source.map(|source| &source.work),
        )?;
        let named_metadata = named.metadata()?;
        if stamp(&after) != stamp(&named_metadata)
            || after.mode() & 0o7777 != named_metadata.mode() & 0o7777
        {
            return Err(invalid(
                "streamed source name no longer selects the held inode",
            ));
        }
        Ok(AdmissionUpdateStreamWorkV1 {
            source_bytes: bytes_read,
            read_bytes: bytes_read,
            read_upper_bound_bytes: 0,
            work_units,
        })
    }

    /// Final fence for indexed source custody. It succeeds only after the
    /// complete authenticated member cursor and every payload have been read.
    /// File mode already uses the held filesystem root and leaves this extra
    /// packed-container fence absent.
    pub(crate) fn finish_streamed_updates(
        &mut self,
    ) -> io::Result<Option<crate::source_admission_indexed_input::IndexedInputCostV1>> {
        if self.source_stream_failed {
            return Err(invalid("source update stream was already refused"));
        }
        if self.source_stream_finished {
            return Err(invalid("source update stream was already finalized"));
        }
        let Some(reader) = self.indexed_input.as_mut() else {
            self.source_stream_finished = true;
            return Ok(None);
        };
        match reader.finish() {
            Ok(cost) => {
                self.source_stream_finished = true;
                Ok(Some(cost))
            }
            Err(error) => {
                self.source_stream_failed = true;
                Err(error)
            }
        }
    }

    /// Return update metadata for a path without materializing the initial
    /// census. Initial batches have no retirements today, but keeping this
    /// point lookup complete preserves the ordinary candidate contract if a
    /// future initial producer adds a related same-batch operation.
    pub(crate) fn update(
        &self,
        relative: &str,
        max_state_bytes: usize,
    ) -> io::Result<Option<SourceUpdate>> {
        let Some(source) = &self.initial_updates else {
            return Ok(self.updates.get(relative).copied());
        };
        let workspace = relative
            .len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| invalid("initial update lookup state overflow"))?;
        if relative.len() > source.max_path_bytes
            || workspace > source.row_state_bytes
            || workspace > max_state_bytes
        {
            return Err(invalid("initial update lookup path exceeds its bound"));
        }
        source.work.charge_many(2)?;
        let db = source.db.borrow();
        let raw: Option<(Vec<u8>, i64, i64)> = db
            .query_row(
                "SELECT sha256,size,mode FROM source_member_census \
                 WHERE scan_label=?1 AND path=?2 COLLATE BINARY",
                params![source.scan_label, relative],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| invalid(format!("initial update lookup refused: {error}")))?;
        let Some((digest, size, mode)) = raw else {
            return Ok(None);
        };
        if digest.len() != 32 {
            return Err(invalid("initial update lookup digest width differs"));
        }
        let size = u64::try_from(size).map_err(|_| invalid("initial update size is negative"))?;
        let mode = u32::try_from(mode).map_err(|_| invalid("initial update mode is invalid"))?;
        if size > source.max_member_bytes || !matches!(mode, 0o600 | 0o644 | 0o755) {
            return Err(invalid(
                "initial update lookup row differs from its profile",
            ));
        }
        let mut digest_bytes = [0u8; 32];
        digest_bytes.copy_from_slice(&digest);
        Ok(Some(SourceUpdate {
            sha256: Digest256::from_bytes(digest_bytes),
            size_bytes: size,
            mode,
        }))
    }

    pub(crate) fn update_cursor(&self) -> Option<AdmissionUpdateCursor> {
        self.initial_updates
            .as_ref()
            .map(|source| AdmissionUpdateCursor {
                source: source.clone(),
                after: None,
                seen_rows: 0,
                seen_bytes: 0,
                eof: false,
            })
    }

    pub(crate) fn open_verified_update(
        &self,
        row: &AdmissionUpdateRow,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<File> {
        let source = self
            .initial_updates
            .as_ref()
            .ok_or_else(|| invalid("verified update row has no held initial source"))?;
        if !Rc::ptr_eq(&source.identity, &row.source_identity) {
            return Err(invalid("verified update row belongs to another census"));
        }
        let io = self
            .budgeted_io
            .as_ref()
            .ok_or_else(|| invalid("initial update source lacks its original IO ledger"))?;
        open_member(
            &self.input,
            &source_path(&row.path)?,
            deadline,
            cancel,
            Some(io),
            Some(&source.work),
        )
    }
}

impl AdmissionUpdateCursor {
    pub(crate) fn next(
        &mut self,
        max_state_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Option<AdmissionUpdateRow>> {
        if self.eof {
            return Err(invalid("initial update cursor was read after EOF"));
        }
        active(deadline, cancel)?;
        let source = self.source.as_ref();
        let previous_len = self.after.as_ref().map_or(0, String::len);
        let max_workspace = source
            .max_path_bytes
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(previous_len.checked_mul(8)?))
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| invalid("initial update cursor state overflow"))?;
        if max_workspace > source.row_state_bytes || max_workspace > max_state_bytes {
            return Err(invalid(
                "initial update cursor exceeds its selected state slice",
            ));
        }
        source.work.charge_many(2)?;
        let db = source.db.borrow_mut();
        let mut statement = db
            .prepare(if self.after.is_some() {
                "SELECT path,sha256,size,mode FROM source_member_census \
                 WHERE scan_label=?1 AND path COLLATE BINARY > ?2 COLLATE BINARY \
                 ORDER BY path COLLATE BINARY LIMIT 1"
            } else {
                "SELECT path,sha256,size,mode FROM source_member_census \
                 WHERE scan_label=?1 ORDER BY path COLLATE BINARY LIMIT 1"
            })
            .map_err(|error| invalid(format!("initial update cursor refused: {error}")))?;
        let mut rows = match self.after.as_deref() {
            Some(after) => statement.query(params![source.scan_label, after]),
            None => statement.query(params![source.scan_label]),
        }
        .map_err(|error| invalid(format!("initial update cursor rows refused: {error}")))?;
        source.work.charge_many(1)?;
        let Some(row) = rows
            .next()
            .map_err(|error| invalid(format!("initial update cursor row refused: {error}")))?
        else {
            drop(rows);
            drop(statement);
            if self.seen_rows != source.expected_rows
                || self.seen_bytes != source.expected_source_bytes
            {
                return Err(invalid("initial update cursor count/bytes differ at EOF"));
            }
            self.eof = true;
            return Ok(None);
        };
        let path_len = match row.get_ref(0) {
            Ok(rusqlite::types::ValueRef::Text(path)) => path.len(),
            _ => return Err(invalid("initial update path is not text")),
        };
        let workspace = path_len
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(previous_len.checked_mul(8)?))
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| invalid("initial update row state overflow"))?;
        if path_len > source.max_path_bytes || workspace > max_state_bytes {
            return Err(invalid(
                "initial update row exceeds its selected state slice",
            ));
        }
        let path: String = row
            .get(0)
            .map_err(|error| invalid(format!("initial update path decode refused: {error}")))?;
        let relative = source_path(&path)?;
        if relative.as_str() != path
            || self
                .after
                .as_deref()
                .is_some_and(|after| after.as_bytes() >= path.as_bytes())
        {
            return Err(invalid("initial update cursor path order differs"));
        }
        let digest = match row.get_ref(1) {
            Ok(rusqlite::types::ValueRef::Blob(bytes)) if bytes.len() == 32 => {
                let mut digest = [0u8; 32];
                digest.copy_from_slice(bytes);
                digest
            }
            _ => return Err(invalid("initial update digest width differs")),
        };
        let size = row
            .get::<_, i64>(2)
            .map_err(|error| invalid(format!("initial update size decode refused: {error}")))?;
        let mode = row
            .get::<_, i64>(3)
            .map_err(|error| invalid(format!("initial update mode decode refused: {error}")))?;
        let size = u64::try_from(size).map_err(|_| invalid("initial update size is negative"))?;
        let mode =
            u32::try_from(mode).map_err(|_| invalid("initial update mode is out of range"))?;
        if size > source.max_member_bytes || !matches!(mode, 0o600 | 0o644 | 0o755) {
            return Err(invalid("initial update mode is unsupported"));
        }
        drop(rows);
        drop(statement);
        let next_seen = self
            .seen_rows
            .checked_add(1)
            .filter(|count| *count <= source.expected_rows)
            .ok_or_else(|| invalid("initial update cursor exceeds sealed count"))?;
        let next_bytes = self
            .seen_bytes
            .checked_add(size)
            .filter(|bytes| *bytes <= source.expected_source_bytes)
            .ok_or_else(|| invalid("initial update cursor exceeds sealed byte total"))?;
        let row = AdmissionUpdateRow {
            path: path.clone(),
            update: SourceUpdate {
                sha256: Digest256::from_bytes(digest),
                size_bytes: size,
                mode,
            },
            workspace_state_bytes: workspace,
            source_identity: source.identity.clone(),
        };
        self.after = Some(path);
        self.seen_rows = next_seen;
        self.seen_bytes = next_bytes;
        active(deadline, cancel)?;
        Ok(Some(row))
    }
}
