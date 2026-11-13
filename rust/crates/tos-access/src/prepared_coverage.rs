//! Bounded coverage observation over an explicitly selected local prepared
//! publication. Rows are streamed one at a time and never become source,
//! rights, review, canon, or semantic authority.

use rusqlite::{Connection, OpenFlags, limits::Limit, types::ValueRef};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tos_compiler::local_prepared::{PreparedReadLimits, PreparedReadTransaction};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonValue, emit_python_compact_json, parse_json,
};
use tos_query::{AbortProbe, AbortReason};

const DEFAULT_BINDING_BYTES: usize = 65_536;
const DIGEST_BYTES: usize = 1_024;
const DIGEST_CHUNKS: usize = 256;
const PROGRESS_GRANULARITY: u64 = 100;

const NODE_COLUMNS: &[&str] = &[
    "id",
    "entity_id",
    "native_id",
    "source_graph",
    "kind_id",
    "type_id",
];
const RELATION_COLUMNS: &[&str] = &[
    "id",
    "native_id",
    "source_graph",
    "from_id",
    "to_id",
    "predicate_id",
    "relation_type_id",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedCoverageKind {
    Node,
    Relation,
}

/// The carrier borrow is valid only for the duration of the visitor call.
#[derive(Clone, Copy)]
pub struct PreparedCoverageRow<'a> {
    pub kind: PreparedCoverageKind,
    pub source_revision: &'a str,
    pub record: &'a JsonValue,
}

/// Admission limits cover the prepared schema and selected metadata. The
/// remaining limits bound the complete row walk and its SQLite work.
#[derive(Clone, Copy, Debug)]
pub struct PreparedCoverageLimits {
    pub admission: PreparedReadLimits,
    pub max_rows: usize,
    pub max_row_bytes: usize,
    pub max_total_bytes: usize,
    pub max_vm_steps: u64,
    pub max_binding_bytes: usize,
}

impl Default for PreparedCoverageLimits {
    fn default() -> Self {
        Self {
            admission: PreparedReadLimits::default(),
            max_rows: 1_000_000,
            max_row_bytes: 1_048_576,
            max_total_bytes: 256 * 1024 * 1024,
            max_vm_steps: 200_000_000,
            max_binding_bytes: DEFAULT_BINDING_BYTES,
        }
    }
}

impl PreparedCoverageLimits {
    fn validate(self) -> Result<(), String> {
        if self.max_rows == 0
            || self.max_row_bytes == 0
            || self.max_row_bytes > (i32::MAX as usize).saturating_sub(8192)
            || self.max_total_bytes == 0
            || self.max_vm_steps == 0
            || self.max_binding_bytes == 0
            || self.max_binding_bytes > 1_048_576
        {
            return Err("prepared coverage limits must be finite positive values".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCoverageSummary {
    pub source_revision: String,
    pub nodes: usize,
    pub relations: usize,
    /// Includes the selected binding, SQLite result rows and returned payloads.
    pub charged_rows: usize,
    pub charged_bytes: usize,
    /// Progress-handler estimate rounded up in 100-op SQLite intervals.
    pub sqlite_vm_steps: u64,
}

#[derive(Clone, Copy)]
struct CoverageMeter {
    rows: usize,
    bytes: usize,
    max_rows: usize,
    max_bytes: usize,
}

impl CoverageMeter {
    fn new(limits: PreparedCoverageLimits) -> Self {
        Self {
            rows: 0,
            bytes: 0,
            max_rows: limits.max_rows,
            max_bytes: limits.max_total_bytes,
        }
    }

    fn charge(&mut self, rows: usize, bytes: usize) -> Result<(), String> {
        self.rows = self
            .rows
            .checked_add(rows)
            .filter(|total| *total <= self.max_rows)
            .ok_or("prepared coverage cumulative row budget exceeded")?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.max_bytes)
            .ok_or("prepared coverage cumulative byte budget exceeded")?;
        Ok(())
    }

    fn admission_limits(
        self,
        mut limits: PreparedReadLimits,
    ) -> Result<PreparedReadLimits, String> {
        let remaining_rows = self
            .max_rows
            .checked_sub(self.rows)
            .filter(|remaining| *remaining > 0)
            .ok_or("prepared coverage cumulative row budget exceeded")?;
        let remaining_bytes = self
            .max_bytes
            .checked_sub(self.bytes)
            .filter(|remaining| *remaining > 0)
            .ok_or("prepared coverage cumulative byte budget exceeded")?;
        limits.max_rows = limits.max_rows.min(remaining_rows);
        limits.max_bytes = limits.max_bytes.min(remaining_bytes);
        Ok(limits)
    }
}

pub struct PreparedCoverageResult {
    pub summary: PreparedCoverageSummary,
    pub guard: PreparedCoverageGuard,
}

/// Retains the selected files and SQLite connection until the terminal result
/// is ready. Call `recheck` immediately before emitting that result.
pub struct PreparedCoverageGuard {
    path: PathBuf,
    binding_path: PathBuf,
    wal_path: PathBuf,
    main_state: FileState,
    binding_state: FileState,
    wal_state: Option<FileState>,
    _publication_file: File,
    binding_file: File,
    db: Connection,
    binding: JsonValue,
    limits: PreparedCoverageLimits,
    meter: CoverageMeter,
    steps: Arc<AtomicU64>,
    reason: Arc<AtomicU8>,
}

impl PreparedCoverageGuard {
    pub fn recheck(
        &mut self,
        summary: &mut PreparedCoverageSummary,
        deadline: Instant,
        abort: Arc<dyn AbortProbe>,
    ) -> Result<(), String> {
        check_abort(deadline, abort.as_ref())?;
        self.check_selected_files()?;
        install_progress_handler(
            &self.db,
            Arc::clone(&self.steps),
            Arc::clone(&self.reason),
            Arc::clone(&abort),
            deadline,
            self.limits.max_vm_steps,
        );
        let admission = self.meter.admission_limits(self.limits.admission)?;
        self.db
            .execute_batch("BEGIN")
            .map_err(|error| sqlite_error(error, &self.reason, deadline, abort.as_ref()))?;
        let fresh = PreparedReadTransaction::recheck_binding(&self.db, &self.binding, admission)
            .map_err(|error| format!("prepared coverage terminal binding check failed: {error}"))?;
        if fresh
            .top()
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            != Some(summary.source_revision.as_str())
        {
            return Err("prepared coverage terminal source revision changed".into());
        }
        let mut meter = self.meter;
        meter.charge(fresh.read_rows(), fresh.read_bytes())?;
        drop(fresh);
        self.db
            .execute_batch("COMMIT")
            .map_err(|error| sqlite_error(error, &self.reason, deadline, abort.as_ref()))?;
        check_abort(deadline, abort.as_ref())?;
        self.check_selected_files()?;
        self.meter = meter;
        summary.charged_rows = meter.rows;
        summary.charged_bytes = meter.bytes;
        summary.sqlite_vm_steps = self.steps.load(Ordering::Relaxed);
        Ok(())
    }

    fn check_selected_files(&self) -> Result<(), String> {
        if path_state(&self.path)? != self.main_state
            || file_state(&self._publication_file)? != self.main_state
            || optional_state(&self.wal_path)? != self.wal_state
            || path_state(&self.binding_path)? != self.binding_state
            || file_state(&self.binding_file)? != self.binding_state
        {
            return Err(
                "prepared coverage file, WAL, or binding changed before terminal output".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileState {
    dev: u64,
    ino: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl FileState {
    fn from_metadata(metadata: &fs::Metadata) -> Result<Self, String> {
        if !metadata.is_file() {
            return Err("prepared coverage selection is not a regular file".into());
        }
        Ok(Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            size: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

fn path_state(path: &Path) -> Result<FileState, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    FileState::from_metadata(&metadata)
}

fn file_state(file: &File) -> Result<FileState, String> {
    FileState::from_metadata(&file.metadata().map_err(|error| error.to_string())?)
}

fn optional_state(path: &Path) -> Result<Option<FileState>, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
        Ok(metadata) => FileState::from_metadata(&metadata).map(Some),
    }
}

fn wal_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push("-wal");
    PathBuf::from(value)
}

fn json_limits(max_bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes,
        max_depth: 96,
        max_visits: 1_000_000,
        max_integer_digits: 4_096,
    }
}

fn load_binding(
    path: &Path,
    max_bytes: usize,
) -> Result<(File, FileState, JsonValue, usize), String> {
    let selected_before = path_state(path)?;
    let mut pinned = tos_fd_open::open_absolute_regular(path, max_bytes as u64)
        .map_err(|error| format!("selected prepared binding unavailable: {error}"))?;
    let pinned_before = file_state(&pinned)?;
    if pinned_before != selected_before {
        return Err("selected prepared binding changed while opening".into());
    }
    let mut bytes = Vec::new();
    pinned
        .by_ref()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > max_bytes {
        return Err("selected prepared binding exceeds its byte budget".into());
    }
    let binding = parse_json(&bytes, JsonMode::PublishedStrict, json_limits(max_bytes))
        .map_err(|error| format!("selected prepared binding JSON invalid: {error}"))?
        .into_root();
    if file_state(&pinned)? != pinned_before || path_state(path)? != selected_before {
        return Err("selected prepared binding changed during read".into());
    }
    let length = bytes.len();
    Ok((pinned, pinned_before, binding, length))
}

fn check_abort(deadline: Instant, abort: &dyn AbortProbe) -> Result<(), String> {
    if let Some(reason) = abort.reason() {
        return Err(match reason {
            AbortReason::Cancelled => "prepared coverage cancelled".into(),
            AbortReason::DeadlineExceeded => "prepared coverage deadline exceeded".into(),
        });
    }
    if Instant::now() >= deadline {
        return Err("prepared coverage deadline exceeded".into());
    }
    Ok(())
}

fn sqlite_error(
    error: rusqlite::Error,
    reason: &AtomicU8,
    deadline: Instant,
    abort: &dyn AbortProbe,
) -> String {
    match reason.load(Ordering::Relaxed) {
        1 => return "prepared coverage cancelled".into(),
        2 => return "prepared coverage deadline exceeded".into(),
        3 => return "prepared coverage SQLite work budget exceeded".into(),
        _ => {}
    }
    if let Some(abort) = abort.reason() {
        return match abort {
            AbortReason::Cancelled => "prepared coverage cancelled".into(),
            AbortReason::DeadlineExceeded => "prepared coverage deadline exceeded".into(),
        };
    }
    if Instant::now() >= deadline {
        return "prepared coverage deadline exceeded".into();
    }
    error.to_string()
}

fn install_progress_handler(
    db: &Connection,
    steps: Arc<AtomicU64>,
    reason: Arc<AtomicU8>,
    abort: Arc<dyn AbortProbe>,
    deadline: Instant,
    max_steps: u64,
) {
    db.progress_handler(
        PROGRESS_GRANULARITY as i32,
        Some(move || {
            if let Some(abort) = abort.reason() {
                reason.store(
                    match abort {
                        AbortReason::Cancelled => 1,
                        AbortReason::DeadlineExceeded => 2,
                    },
                    Ordering::Relaxed,
                );
                return true;
            }
            if Instant::now() >= deadline {
                reason.store(2, Ordering::Relaxed);
                return true;
            }
            let used = steps
                .fetch_add(PROGRESS_GRANULARITY, Ordering::Relaxed)
                .saturating_add(PROGRESS_GRANULARITY);
            if used > max_steps {
                reason.store(3, Ordering::Relaxed);
                true
            } else {
                false
            }
        }),
    );
}

/// A path and a separately selected binding. No current pointer, neighboring
/// file, source root, or default checkout is discovered by this reader.
pub struct PreparedCoverageSource {
    path: PathBuf,
    binding_path: PathBuf,
    limits: PreparedCoverageLimits,
}

impl PreparedCoverageSource {
    pub fn new(
        path: PathBuf,
        binding_path: PathBuf,
        limits: PreparedCoverageLimits,
    ) -> Result<Self, String> {
        limits.validate()?;
        if !path.is_absolute() || !binding_path.is_absolute() || path == binding_path {
            return Err(
                "prepared coverage requires separate absolute publication and binding paths".into(),
            );
        }
        Ok(Self {
            path,
            binding_path,
            limits,
        })
    }

    /// Visit every normalized node and relation in binary identifier order.
    /// The binding and publication are rechecked in a fresh transaction and by
    /// file/WAL identity before a complete summary is returned. The result
    /// retains the selected files and exposes a final terminal recheck guard.
    pub fn visit<F>(
        &self,
        deadline: Instant,
        abort: Arc<dyn AbortProbe>,
        mut visitor: F,
    ) -> Result<PreparedCoverageResult, String>
    where
        F: for<'row> FnMut(PreparedCoverageRow<'row>) -> Result<(), String>,
    {
        check_abort(deadline, abort.as_ref())?;
        let binding_cap = self
            .limits
            .max_binding_bytes
            .min(self.limits.max_total_bytes);
        let (binding_file, binding_state, binding, binding_bytes) =
            load_binding(&self.binding_path, binding_cap)?;
        let mut meter = CoverageMeter::new(self.limits);
        meter.charge(1, binding_bytes)?;
        let main_before = path_state(&self.path)?;
        let wal = wal_path(&self.path);
        let pinned = tos_fd_open::open_absolute_regular(&self.path, u64::MAX)
            .map_err(|error| format!("selected prepared publication unavailable: {error}"))?;
        let pinned_state = file_state(&pinned)?;
        let db = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| format!("selected prepared publication cannot be opened: {error}"))?;
        db.busy_timeout(Duration::from_millis(100))
            .map_err(|error| error.to_string())?;
        let sqlite_limit = self
            .limits
            .max_row_bytes
            .max(131_072)
            .checked_add(4_096)
            .filter(|limit| *limit <= i32::MAX as usize)
            .ok_or("prepared coverage SQLite row limit overflow")?;
        db.set_limit(
            Limit::SQLITE_LIMIT_LENGTH,
            sqlite_limit
                .try_into()
                .map_err(|_| "prepared coverage SQLite row limit overflow")?,
        )
        .map_err(|error| error.to_string())?;
        db.pragma_update(None, "query_only", true)
            .map_err(|error| error.to_string())?;
        if main_before != pinned_state
            || path_state(&self.path)? != main_before
            || path_state(&self.binding_path)? != binding_state
            || file_state(&binding_file)? != binding_state
        {
            return Err("prepared coverage selection changed while opening".into());
        }
        // Observe WAL state after SQLite has opened the selected pathname, then
        // ensure admission itself did not cross a publication generation.
        let wal_before = optional_state(&wal)?;
        db.execute_batch("BEGIN")
            .map_err(|error| error.to_string())?;

        let steps = Arc::new(AtomicU64::new(0));
        let reason = Arc::new(AtomicU8::new(0));
        install_progress_handler(
            &db,
            Arc::clone(&steps),
            Arc::clone(&reason),
            Arc::clone(&abort),
            deadline,
            self.limits.max_vm_steps,
        );

        let admission = meter.admission_limits(self.limits.admission)?;
        let admitted = PreparedReadTransaction::admit(&db, &binding, admission)
            .map_err(|error| format!("prepared coverage admission refused: {error}"))?;
        meter.charge(admitted.read_rows(), admitted.read_bytes())?;
        if path_state(&self.path)? != main_before || optional_state(&wal)? != wal_before {
            return Err("prepared coverage publication changed during initial admission".into());
        }
        let source_revision = admitted
            .top()
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            .filter(|revision| !revision.is_empty())
            .ok_or("prepared coverage selected source revision missing")?
            .to_owned();
        drop(admitted);

        let mut summary = PreparedCoverageSummary {
            source_revision: source_revision.clone(),
            nodes: 0,
            relations: 0,
            charged_rows: meter.rows,
            charged_bytes: meter.bytes,
            sqlite_vm_steps: 0,
        };
        self.scan_kind(
            &db,
            PreparedCoverageKind::Node,
            "knowledge_nodes",
            NODE_COLUMNS,
            deadline,
            abort.as_ref(),
            reason.as_ref(),
            &mut summary,
            &mut meter,
            &source_revision,
            &mut visitor,
        )?;
        self.scan_kind(
            &db,
            PreparedCoverageKind::Relation,
            "knowledge_relations",
            RELATION_COLUMNS,
            deadline,
            abort.as_ref(),
            reason.as_ref(),
            &mut summary,
            &mut meter,
            &source_revision,
            &mut visitor,
        )?;
        check_abort(deadline, abort.as_ref())?;
        db.execute_batch("COMMIT; BEGIN")
            .map_err(|error| sqlite_error(error, &reason, deadline, abort.as_ref()))?;
        let admission = meter.admission_limits(self.limits.admission)?;
        let fresh = PreparedReadTransaction::recheck_binding(&db, &binding, admission)
            .map_err(|error| format!("prepared coverage fresh binding check failed: {error}"))?;
        if fresh
            .top()
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            != Some(source_revision.as_str())
        {
            return Err("prepared coverage source revision changed during scan".into());
        }
        meter.charge(fresh.read_rows(), fresh.read_bytes())?;
        drop(fresh);
        db.execute_batch("COMMIT")
            .map_err(|error| sqlite_error(error, &reason, deadline, abort.as_ref()))?;
        check_abort(deadline, abort.as_ref())?;
        if path_state(&self.path)? != main_before
            || optional_state(&wal)? != wal_before
            || path_state(&self.binding_path)? != binding_state
            || file_state(&binding_file)? != binding_state
        {
            return Err("prepared coverage file, WAL, or binding changed during scan".into());
        }
        summary.charged_rows = meter.rows;
        summary.charged_bytes = meter.bytes;
        summary.sqlite_vm_steps = steps.load(Ordering::Relaxed);
        Ok(PreparedCoverageResult {
            summary,
            guard: PreparedCoverageGuard {
                path: self.path.clone(),
                binding_path: self.binding_path.clone(),
                wal_path: wal,
                main_state: main_before,
                binding_state,
                wal_state: wal_before,
                _publication_file: pinned,
                binding_file,
                db,
                binding,
                limits: self.limits,
                meter,
                steps,
                reason,
            },
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_kind<F>(
        &self,
        db: &Connection,
        kind: PreparedCoverageKind,
        table: &str,
        columns: &[&str],
        deadline: Instant,
        abort: &dyn AbortProbe,
        reason: &AtomicU8,
        summary: &mut PreparedCoverageSummary,
        meter: &mut CoverageMeter,
        source_revision: &str,
        visitor: &mut F,
    ) -> Result<(), String>
    where
        F: for<'row> FnMut(PreparedCoverageRow<'row>) -> Result<(), String>,
    {
        let row_limit: i64 = self
            .limits
            .max_row_bytes
            .try_into()
            .map_err(|_| "prepared coverage row limit overflow")?;
        let selections = columns
            .iter()
            .chain(std::iter::once(&"json"))
            .map(|column| {
                format!(
                    "CASE WHEN typeof({column})='text' AND length(CAST({column} AS BLOB))<=?1 THEN {column} END"
                )
            })
            .collect::<Vec<_>>();
        let sql = format!(
            "SELECT {} FROM {table} ORDER BY id COLLATE BINARY",
            selections.join(",")
        );
        let mut statement = db
            .prepare(&sql)
            .map_err(|error| sqlite_error(error, reason, deadline, abort))?;
        let mut rows = statement
            .query([row_limit])
            .map_err(|error| sqlite_error(error, reason, deadline, abort))?;
        let mut previous_id: Option<String> = None;
        while let Some(row) = rows
            .next()
            .map_err(|error| sqlite_error(error, reason, deadline, abort))?
        {
            check_abort(deadline, abort)?;
            meter.charge(1, 0)?;
            let mut indexed = Vec::with_capacity(columns.len());
            let mut row_bytes = 0usize;
            for index in 0..columns.len() + 1 {
                let value = match row.get::<_, Option<String>>(index) {
                    Ok(Some(value)) => value,
                    Ok(None) => {
                        return Err(
                            "prepared coverage carrier has invalid or oversized text".into()
                        );
                    }
                    Err(error) => return Err(sqlite_error(error, reason, deadline, abort)),
                };
                row_bytes = row_bytes
                    .checked_add(value.len())
                    .filter(|bytes| *bytes <= self.limits.max_row_bytes)
                    .ok_or("prepared coverage carrier exceeds its row byte budget")?;
                meter.charge(0, value.len())?;
                if index < columns.len() {
                    indexed.push(value);
                } else {
                    let raw = value;
                    let identifier = indexed
                        .first()
                        .ok_or("prepared coverage carrier identifier missing")?;
                    if previous_id
                        .as_deref()
                        .is_some_and(|previous| previous >= identifier.as_str())
                    {
                        return Err(
                            "prepared coverage identifiers are not unique binary order".into()
                        );
                    }
                    previous_id = Some(identifier.clone());
                    let record = parse_json(
                        raw.as_bytes(),
                        JsonMode::PublishedStrict,
                        json_limits(self.limits.max_row_bytes),
                    )
                    .map_err(|error| format!("prepared coverage carrier JSON invalid: {error}"))?
                    .into_root();
                    if record.as_object().is_none()
                        || columns.iter().zip(&indexed).any(|(column, expected)| {
                            tos_compiler::local_prepared::index_value(&record, column)
                                .ok()
                                .as_deref()
                                != Some(expected.as_str())
                        })
                    {
                        return Err("prepared coverage carrier identity differs from normalized row columns".into());
                    }
                    let emitted =
                        emit_python_compact_json(&record, json_limits(self.limits.max_row_bytes))
                            .map_err(|error| {
                            format!("prepared coverage carrier framing invalid: {error}")
                        })?;
                    if emitted.as_slice() != raw.as_bytes() {
                        return Err(
                            "prepared coverage carrier differs from emitted compact JSON".into(),
                        );
                    }
                    self.verify_row_digest(
                        db, kind, identifier, &raw, deadline, abort, reason, meter,
                    )?;
                    summary.nodes = if kind == PreparedCoverageKind::Node {
                        summary
                            .nodes
                            .checked_add(1)
                            .ok_or("prepared coverage row count overflow")?
                    } else {
                        summary.nodes
                    };
                    summary.relations = if kind == PreparedCoverageKind::Relation {
                        summary
                            .relations
                            .checked_add(1)
                            .ok_or("prepared coverage row count overflow")?
                    } else {
                        summary.relations
                    };
                    check_abort(deadline, abort)?;
                    visitor(PreparedCoverageRow {
                        kind,
                        source_revision,
                        record: &record,
                    })?;
                    check_abort(deadline, abort)?;
                }
            }
        }
        Ok(())
    }

    fn verify_row_digest(
        &self,
        db: &Connection,
        kind: PreparedCoverageKind,
        identifier: &str,
        raw: &str,
        deadline: Instant,
        abort: &dyn AbortProbe,
        reason: &AtomicU8,
        meter: &mut CoverageMeter,
    ) -> Result<(), String> {
        let kind = match kind {
            PreparedCoverageKind::Node => "node",
            PreparedCoverageKind::Relation => "relation",
        };
        let key = format!("knowledge_{kind}_digest:{identifier}");
        let mut statement = db
            .prepare("SELECT part,json_chunk FROM edge_meta WHERE key=?1 ORDER BY part LIMIT 257")
            .map_err(|error| sqlite_error(error, reason, deadline, abort))?;
        let mut rows = statement
            .query([key])
            .map_err(|error| sqlite_error(error, reason, deadline, abort))?;
        let mut digest = Vec::new();
        let mut count = 0usize;
        while let Some(row) = rows
            .next()
            .map_err(|error| sqlite_error(error, reason, deadline, abort))?
        {
            check_abort(deadline, abort)?;
            meter.charge(1, 0)?;
            if count == DIGEST_CHUNKS {
                return Err("prepared coverage carrier checksum chunk budget exceeded".into());
            }
            let part = row
                .get::<_, i64>(0)
                .map_err(|error| sqlite_error(error, reason, deadline, abort))?;
            if part != count as i64 {
                return Err("prepared coverage carrier checksum chunks are incomplete".into());
            }
            let ValueRef::Text(chunk) = row
                .get_ref(1)
                .map_err(|error| sqlite_error(error, reason, deadline, abort))?
            else {
                return Err("prepared coverage carrier checksum chunk is not text".into());
            };
            if chunk.is_empty() || digest.len().saturating_add(chunk.len()) > DIGEST_BYTES {
                return Err("prepared coverage carrier checksum exceeds its byte budget".into());
            }
            meter.charge(0, std::mem::size_of::<i64>().saturating_add(chunk.len()))?;
            digest.extend_from_slice(chunk);
            count += 1;
        }
        if count == 0 {
            return Err("prepared coverage carrier checksum missing".into());
        }
        let value = parse_json(
            &digest,
            JsonMode::PublishedStrict,
            json_limits(DIGEST_BYTES),
        )
        .map_err(|error| format!("prepared coverage carrier checksum invalid: {error}"))?
        .into_root();
        if value.as_object().is_none_or(|fields| fields.len() != 1)
            || value.object_get("sha256").and_then(JsonValue::as_str)
                != Some(Digest256::of_bytes(raw.as_bytes()).to_hex().as_str())
        {
            return Err("prepared coverage carrier checksum differs".into());
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "source-tools"))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tos_compiler::local_prepared::{
        PreparedRows, PublicationLimits, publish_prepared_rows_until,
    };

    struct NeverAbort;

    impl AbortProbe for NeverAbort {
        fn reason(&self) -> Option<AbortReason> {
            None
        }
    }

    fn text(value: &str) -> JsonValue {
        JsonValue::String(tos_foundation::JsonString::from_utf8(value))
    }

    fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (tos_foundation::JsonString::from_utf8(key), value))
                .collect(),
        )
    }

    struct Rows {
        nodes: Vec<JsonValue>,
        relations: Vec<JsonValue>,
    }

    fn node_display() -> JsonValue {
        object(vec![
            ("title", object(vec![("en", text("Claim title"))])),
            ("kind_label", object(vec![("en", text("Claim"))])),
            (
                "summary",
                object(vec![("en", text("Prepared coverage fixture claim"))]),
            ),
            (
                "provenance",
                object(vec![
                    ("title", text("source-derived")),
                    ("source_title_available", JsonValue::Bool(true)),
                    ("kind_label", text("registry-label")),
                    ("summary", text("source-derived")),
                    ("source_summary_available", JsonValue::Bool(true)),
                ]),
            ),
        ])
    }

    fn relation_display() -> JsonValue {
        object(vec![
            ("label", object(vec![("en", text("supports"))])),
            (
                "inverse_label",
                object(vec![("en", text("is supported by"))]),
            ),
            (
                "statement",
                object(vec![("en", text("The first claim supports the second."))]),
            ),
            (
                "explanation",
                object(vec![("en", text("Prepared coverage fixture relation"))]),
            ),
            (
                "provenance",
                object(vec![
                    ("label", text("source-derived")),
                    ("inverse_label", text("source-derived")),
                    ("statement", text("source-derived")),
                    ("explanation", text("source-derived")),
                    ("source_explanation_available", JsonValue::Bool(true)),
                ]),
            ),
        ])
    }

    fn node(id: &str, entity_id: &str, native_id: &str) -> JsonValue {
        object(vec![
            ("id", text(id)),
            ("entity_id", text(entity_id)),
            ("native_id", text(native_id)),
            ("source_graph", text("graph-test")),
            ("kind_id", text("claim")),
            ("type_id", text("tos.entity.claim")),
            ("content_revision", text(&"c".repeat(64))),
            (
                "source_refs",
                JsonValue::Array(vec![text("test:prepared-coverage")]),
            ),
            ("type_mapping", object(vec![("status", text("unmapped"))])),
            (
                "attributes",
                object(vec![("source_record", object(Vec::new()))]),
            ),
            ("display", node_display()),
        ])
    }

    fn relation() -> JsonValue {
        object(vec![
            ("id", text("relation-ab")),
            ("entity_id", text("relation-entity-ab")),
            ("native_id", text("native-relation-ab")),
            ("source_graph", text("graph-test")),
            ("from_id", text("node-a")),
            ("to_id", text("node-b")),
            ("predicate_id", text("supports")),
            ("relation_type_id", text("tos.relation.supports")),
            ("content_revision", text(&"d".repeat(64))),
            (
                "source_refs",
                JsonValue::Array(vec![text("test:prepared-coverage")]),
            ),
            (
                "predicate_mapping",
                object(vec![("status", text("unmapped"))]),
            ),
            (
                "attributes",
                object(vec![("source_record", object(Vec::new()))]),
            ),
            ("display", relation_display()),
        ])
    }

    impl PreparedRows for Rows {
        fn visit(
            &mut self,
            kind: &str,
            sink: &mut dyn FnMut(&JsonValue) -> tos_compiler::Result<()>,
        ) -> tos_compiler::Result<()> {
            let rows = if kind == "node" {
                &self.nodes
            } else {
                &self.relations
            };
            for row in rows {
                sink(row)?;
            }
            Ok(())
        }
    }

    fn rows() -> Rows {
        Rows {
            nodes: vec![
                node("node-a", "entity-a", "native-node-a"),
                node("node-b", "entity-b", "native-node-b"),
            ],
            relations: vec![relation()],
        }
    }

    fn publication(dir: &Path) -> (PathBuf, PathBuf, String) {
        let path = dir.join("prepared.sqlite");
        let binding_path = dir.join("owner-selected-binding.json");
        let revision = "a".repeat(64);
        let digest = "b".repeat(64);
        let header = object(vec![
            ("schema", text("tos_knowledge_graph_v1")),
            ("source_revision", text(&revision)),
            (
                "normalization_binding",
                object(vec![
                    (
                        "schema",
                        text("tos_knowledge_graph_normalization_binding_v1"),
                    ),
                    ("processor_digest", text(&digest)),
                    ("entity_registry_digest", text(&digest)),
                    ("relation_registry_digest", text(&digest)),
                    ("configuration_digest", text(&digest)),
                ]),
            ),
            (
                "authority_boundary",
                object(vec![
                    ("source_owner", text("Tree-of-Sophia")),
                    ("is_source", JsonValue::Bool(false)),
                    ("is_canon", JsonValue::Bool(false)),
                    ("writes_to_tree", JsonValue::Bool(false)),
                ]),
            ),
            ("query_properties", JsonValue::Array(Vec::new())),
        ]);
        let catalog = object(vec![
            ("schema", text("tos_knowledge_catalog_v1")),
            ("source_revision", text(&revision)),
            ("lenses", JsonValue::Array(Vec::new())),
        ]);
        let binding = publish_prepared_rows_until(
            &path,
            &header,
            &catalog,
            &mut rows(),
            PublicationLimits {
                max_bytes: 4 * 1024 * 1024,
                max_mutations: 100_000,
                max_row_bytes: 4096,
                max_metadata_bytes: 65_536,
                max_changes: 16,
                max_change_bytes: 65_536,
            },
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        fs::write(
            &binding_path,
            emit_python_compact_json(&binding, JsonLimits::default()).unwrap(),
        )
        .unwrap();
        (path, binding_path, revision)
    }

    #[test]
    fn streams_real_prepared_rows_with_binding_revision_and_rejects_budget_or_tampering() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-prepared-coverage-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        let (path, binding_path, revision) = publication(&dir);
        let source = PreparedCoverageSource::new(
            path.clone(),
            binding_path.clone(),
            PreparedCoverageLimits::default(),
        )
        .unwrap();
        let mut ids = Vec::new();
        let mut result = source
            .visit(
                Instant::now() + Duration::from_secs(10),
                Arc::new(NeverAbort),
                |row| {
                    assert_eq!(row.source_revision, revision);
                    ids.push((
                        row.kind,
                        row.record
                            .object_get("id")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_owned(),
                    ));
                    Ok(())
                },
            )
            .unwrap();
        result
            .guard
            .recheck(
                &mut result.summary,
                Instant::now() + Duration::from_secs(10),
                Arc::new(NeverAbort),
            )
            .unwrap();
        let summary = &result.summary;
        assert_eq!(summary.source_revision, revision);
        assert_eq!((summary.nodes, summary.relations), (2, 1));
        assert!(summary.charged_bytes > 0);
        assert!(summary.charged_rows > 3);
        assert!(summary.sqlite_vm_steps > 0);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], (PreparedCoverageKind::Node, "node-a".into()));
        assert_eq!(ids[1], (PreparedCoverageKind::Node, "node-b".into()));
        assert_eq!(
            ids[2],
            (PreparedCoverageKind::Relation, "relation-ab".into())
        );

        let coverage_args = vec![
            "coverage".to_owned(),
            "--prepared-read-model".to_owned(),
            path.display().to_string(),
            "--prepared-binding".to_owned(),
            binding_path.display().to_string(),
            "--rows".to_owned(),
        ];
        let mut coverage_output = Vec::new();
        let mut coverage_errors = Vec::new();
        assert_eq!(
            crate::coverage::run_if_requested(
                &coverage_args,
                &mut coverage_output,
                &mut coverage_errors,
            ),
            Some(0),
            "{}",
            String::from_utf8_lossy(&coverage_errors)
        );
        let frames = coverage_output
            .split(|byte| *byte == b'\n')
            .filter(|frame| !frame.is_empty())
            .map(|frame| serde_json::from_slice::<serde_json::Value>(frame).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 4, "two nodes, one relation and summary");
        assert!(
            frames[..3]
                .iter()
                .all(|frame| frame["schema_version"] == "tos_knowledge_coverage_row_v1")
        );
        let terminal = frames.last().unwrap();
        assert_eq!(terminal["schema_version"], "tos_knowledge_coverage_v1");
        assert_eq!(terminal["source_revision"], revision);
        assert_eq!(terminal["enumeration_complete"], true);
        assert_eq!(terminal["nodes"], 2);
        assert_eq!(terminal["relations"], 1);

        let limited = PreparedCoverageSource::new(
            path.clone(),
            binding_path.clone(),
            PreparedCoverageLimits {
                max_rows: 1,
                ..PreparedCoverageLimits::default()
            },
        )
        .unwrap();
        let error = limited
            .visit(
                Instant::now() + Duration::from_secs(10),
                Arc::new(NeverAbort),
                |_| Ok(()),
            )
            .err()
            .unwrap();
        assert!(error.contains("row budget"), "{error}");

        let mut changed_rows = rows();
        let mut changed = changed_rows.nodes.remove(0);
        let JsonValue::Object(fields) = &mut changed else {
            panic!("node row must remain an object")
        };
        fields.push((
            tos_foundation::JsonString::from_utf8("unexpected"),
            text("changed after prepared publication"),
        ));
        let raw = emit_python_compact_json(&changed, JsonLimits::default()).unwrap();
        let raw = String::from_utf8(raw).unwrap();
        let db = Connection::open(&path).unwrap();
        db.execute(
            "UPDATE knowledge_nodes SET json=?1 WHERE id=?2",
            rusqlite::params![raw, "node-a"],
        )
        .unwrap();
        drop(db);
        let error = source
            .visit(
                Instant::now() + Duration::from_secs(10),
                Arc::new(NeverAbort),
                |_| Ok(()),
            )
            .err()
            .unwrap();
        assert!(error.contains("checksum differs"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }
}
