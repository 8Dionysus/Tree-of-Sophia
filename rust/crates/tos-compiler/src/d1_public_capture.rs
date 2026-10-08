//! Exact, disposable capture of the three allowlisted public graph projections
//! and selected original-carrier files. Rows live on disk in source encounter
//! order. This is a read-model input, never a source, rights, canon or
//! installed-current grant.

use crate::{Error, Limits, Result, legacy::decode_partition_part, safe_open, sqlite_budget};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File},
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, emit_python_compact_json,
    emit_python_compact_json_with_state_budget_and_visits_and_check, parse_json,
    parse_json_with_state_budget_and_check,
};

const CORPUS: &str = "corpus";
const PHILOSOPHY: &str = "philosophy";
const CLAIMS: &str = "bibliographic";
const PHILOSOPHY_AUDIT_RELATIVE: &str =
    "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json";
const MAX_ROOT_BYTES: u64 = 256 * 1024;
pub(crate) const MAX_ROW_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_HEADER_BYTES: usize = 2 * 1024 * 1024;
const MAX_INDEX_BYTES: usize = 128 * 1024;
const MAX_PART_BYTES: usize = 8 * 1024 * 1024;
const STORED_OVERHEAD: usize = 65536;

#[derive(Clone, Copy, Debug)]
pub struct PublicCaptureLimits {
    pub max_input_bytes: u64,
    pub max_rows: u64,
    pub max_staging_bytes: u64,
    pub max_work_bytes: u64,
    pub max_sql_vm_steps: u64,
    pub sqlite_cache_kib: u32,
}

/// Exact seven-path selector inherited from Reference Core construction.
/// Five files form the whole graph/catalog source identity; the two optional
/// files retain their selected-path presence for isolated header/evidence
/// calls. Paths are inputs, never copied or re-rooted under `root`.
#[derive(Clone, Debug)]
pub struct PublicCaptureInputPaths {
    pub index_path: PathBuf,
    pub philosophy_graph_projection_path: PathBuf,
    pub bibliographic_graph_path: PathBuf,
    pub entity_type_registry_path: PathBuf,
    pub relation_type_registry_path: PathBuf,
    pub philosophy_post_planting_audit_path: PathBuf,
    pub evidence_projection_path: PathBuf,
}

/// One original source carrier needed by a lower-level Core operation. These
/// profiles deliberately omit unrelated graph roles and registries so an
/// index or bibliography read still works on a partial source root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCaptureRole {
    Corpus,
    Philosophy,
    Bibliographic,
    PhilosophyAudit,
}

/// Existing selected-profile choice, not a new semantic capture domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCaptureProfile {
    Carrier(RuntimeCaptureRole),
    Whole,
}

/// One serial owner's original budgets. Absolute ceilings never reset between
/// selected roles and WholeRoot; only creation_work_allowance is a remainder.
/// The state callback includes all simultaneously held caller/transport owners.
pub struct RuntimeCaptureOwnedBudget<'a> {
    pub remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
    pub original_work: Arc<AtomicU64>,
    pub original_work_limit: u64,
    pub creation_work_allowance: u64,
    pub original_sql_vm: Arc<AtomicU64>,
    pub original_sql_vm_limit: u64,
    pub original_sqlite_heap: Arc<sqlite_budget::DedicatedSessionSqliteHeap>,
    pub max_creation_json_visits: usize,
    /// This call cutoff narrows construction; the owner deadline remains the
    /// original session deadline supplied to the constructor.
    pub creation_deadline: Instant,
}

/// Initialized empty by the caller; debit this on success AND error, then
/// terminate on error. A failed parse retains its admitted visit ceiling.
#[derive(Default, Clone, Copy, Debug)]
pub struct RuntimeCaptureCreationUsage {
    pub json_visits: usize,
}

fn check_capture_active(
    cancelled: Option<&std::sync::atomic::AtomicBool>,
    deadline: Instant,
) -> Result<()> {
    if cancelled.is_some_and(|value| value.load(std::sync::atomic::Ordering::Relaxed)) {
        return Err(Error::Budget("public D1 capture cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("public D1 capture deadline"));
    }
    Ok(())
}

impl PublicCaptureInputPaths {
    /// Maintained runtime source selector; no filesystem discovery.
    pub fn runtime(root: &Path) -> Self {
        Self {
            index_path: root.join("ToS/derived-exports/tos_corpus_index.min.json"),
            philosophy_graph_projection_path: root
                .join("ToS/derived-exports/philosophy_graph_projection.min.json"),
            bibliographic_graph_path: root
                .join("ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"),
            entity_type_registry_path: root
                .join("ToS/doctrine/semantic-interchange/entity-types.v1.json"),
            relation_type_registry_path: root
                .join("ToS/doctrine/semantic-interchange/relation-types.v1.json"),
            philosophy_post_planting_audit_path: root.join(
                "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
            ),
            evidence_projection_path: root
                .join("ToS/derived-exports/epistemic_evidence_projection.min.json"),
        }
    }

    pub(crate) fn selected_paths(&self) -> [(&Path, bool); 7] {
        [
            (&self.index_path, true),
            (&self.philosophy_graph_projection_path, true),
            (&self.bibliographic_graph_path, true),
            (&self.entity_type_registry_path, true),
            (&self.relation_type_registry_path, true),
            (&self.philosophy_post_planting_audit_path, false),
            (&self.evidence_projection_path, false),
        ]
    }

    fn validate(&self) -> Result<()> {
        for (path, _) in self.selected_paths() {
            if !path.is_absolute() || path.to_str().is_none() {
                return Err(Error::Invalid("selected public D1 input path"));
            }
        }
        Ok(())
    }

    fn core_path(&self, relative: &str) -> Option<&Path> {
        match relative {
            "ToS/derived-exports/tos_corpus_index.min.json" => Some(&self.index_path),
            "ToS/derived-exports/philosophy_graph_projection.min.json" => {
                Some(&self.philosophy_graph_projection_path)
            }
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json" => {
                Some(&self.bibliographic_graph_path)
            }
            "ToS/doctrine/semantic-interchange/entity-types.v1.json" => {
                Some(&self.entity_type_registry_path)
            }
            "ToS/doctrine/semantic-interchange/relation-types.v1.json" => {
                Some(&self.relation_type_registry_path)
            }
            _ => None,
        }
    }

    fn optional_path(&self, relative: &str) -> Option<&Path> {
        match relative {
            "ToS/derived-exports/epistemic_evidence_projection.min.json" => {
                Some(&self.evidence_projection_path)
            }
            "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json" => {
                Some(&self.philosophy_post_planting_audit_path)
            }
            _ => None,
        }
    }
}

impl PublicCaptureLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_input_bytes == 0
            || self.max_rows == 0
            || self.max_staging_bytes == 0
            || self.max_work_bytes == 0
            || self.max_sql_vm_steps == 0
            || self.sqlite_cache_kib == 0
        {
            return Err(Error::Budget("public D1 capture limits"));
        }
        Ok(())
    }
    pub(crate) fn sqlite(self) -> Limits {
        Limits {
            max_rows: self.max_rows,
            max_row_bytes: MAX_ROW_BYTES,
            max_output_bytes: self.max_staging_bytes,
            max_work_bytes: self.max_work_bytes,
            sqlite_cache_kib: self.sqlite_cache_kib,
            max_sql_vm_steps: self.max_sql_vm_steps,
        }
    }
}

/// A private source-byte snapshot. No publication is possible from this
/// capture without the full build's final input recheck and output completion.
/// Opaque per-capture identity. Session budget Arcs are deliberately not this
/// identity: different captures may share that same original ledger.
pub(crate) struct ControlledCaptureIdentity {
    token: Arc<()>,
}
impl ControlledCaptureIdentity {
    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.token, &other.token)
    }
}

#[derive(Clone, Copy)]
struct CaptureOperation {
    deadline: Instant,
    work_limit: u64,
    vm_limit: u64,
    phase_limited: bool,
}

pub struct PublicCapture {
    root: PathBuf,
    prepared_profile: bool,
    evidence_profile: bool,
    runtime_capture_role: Option<RuntimeCaptureRole>,
    prepared_state: Vec<(PathBuf, PathBuf, u64, u64, i64, i64, i64, i64)>,
    path: PathBuf,
    inode: (u64, u64),
    sources: Vec<SourceFile>,
    partitioned: bool,
    file_state: CaptureFileState,
    family_seal: Mutex<FamilyPreparationSeal>,
    pub rows: u64,
    work_bytes: Arc<AtomicU64>,
    max_work_bytes: u64,
    deadline: Instant,
    operation_deadline: Mutex<Option<CaptureOperation>>,
    limits: PublicCaptureLimits,
    vm_used: Arc<AtomicU64>,
    shared_vm: bool,
    sqlite_heap: Option<Arc<sqlite_budget::DedicatedSessionSqliteHeap>>,
    controlled_identity: Option<Arc<()>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

type CaptureFileState = (u64, u64, u64, i64, i64, i64, i64);
enum FamilyPreparationSeal {
    Initial,
    Preparing(std::thread::ThreadId),
    Prepared(CaptureFileState),
    Failed,
}
struct FamilyPreparationGuard<'a> {
    capture: &'a PublicCapture,
    complete: bool,
}
impl Drop for FamilyPreparationGuard<'_> {
    fn drop(&mut self) {
        if !self.complete {
            if let Ok(mut seal) = self.capture.family_seal.lock() {
                *seal = FamilyPreparationSeal::Failed;
            }
        }
    }
}
fn capture_file_state(metadata: &fs::Metadata) -> Result<CaptureFileState> {
    if !metadata.file_type().is_file() {
        return Err(Error::Invalid("public D1 private capture replaced"));
    }
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}

enum SourceOrigin {
    File(PathBuf),
    Compiled(&'static [u8]),
}
struct SourceFile {
    label: String,
    origin: SourceOrigin,
    digest: Option<Digest256>,
    len: u64,
}

struct PendingCapture<'a> {
    path: &'a Path,
    complete: bool,
}
impl Drop for PendingCapture<'_> {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        for suffix in ["-journal", "-wal", "-shm", ""] {
            let mut name = self.path.as_os_str().to_os_string();
            name.push(suffix);
            let _ = fs::remove_file(PathBuf::from(name));
        }
    }
}

// Maintained prepare selects these five carriers, including their original
// symlink resolution and path/mtime/size/inode/ctime currentness observations.
const PREPARED_INPUTS: [&str; 5] = [
    "ToS/derived-exports/tos_corpus_index.min.json",
    "ToS/derived-exports/philosophy_graph_projection.min.json",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
];
fn prepared_state(root: &Path) -> Result<Vec<(PathBuf, PathBuf, u64, u64, i64, i64, i64, i64)>> {
    PREPARED_INPUTS
        .iter()
        .map(|name| {
            let selected = root.join(name);
            let resolved = fs::canonicalize(&selected)?;
            let m = fs::metadata(&selected)?;
            Ok((
                selected,
                resolved,
                m.len(),
                m.ino(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            ))
        })
        .collect()
}
fn profile_open(path: &Path, cap: u64, prepared: bool) -> Result<File> {
    if prepared {
        safe_open::open_regular(&fs::canonicalize(path)?, cap)
    } else {
        safe_open::open_regular(path, cap)
    }
}

#[track_caller]
fn checked_add(work: &AtomicU64, bytes: usize, limit: u64) -> Result<()> {
    let mut current = work.load(std::sync::atomic::Ordering::Acquire);
    loop {
        let Some(next) = current
            .checked_add(bytes as u64)
            .filter(|value| *value <= limit)
        else {
            let caller = std::panic::Location::caller();
            eprintln!(
                "Native capture work refused at {}:{}: used={} requested={} limit={}",
                caller.file(),
                caller.line(),
                current,
                bytes,
                limit
            );
            return Err(Error::Budget("public D1 capture work"));
        };
        match work.compare_exchange_weak(
            current,
            next,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        ) {
            Ok(_) => return Ok(()),
            Err(value) => current = value,
        }
    }
}

pub(crate) fn json(raw: &[u8], cap: usize) -> Result<JsonValue> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("public D1 JSON limits"))?;
    Ok(parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(foundation_json_error)?
        .into_root())
}

pub(crate) fn compact(value: &JsonValue, cap: usize) -> Result<Vec<u8>> {
    emit_python_compact_json(
        value,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("public D1 JSON output"))?,
    )
    .map_err(foundation_json_error)
}

const FOUNDATION_JSON_DIAGNOSTICS: &[&str] = &[
    "runtime carrier compact cutoff/cancellation",
    "runtime carrier compact work overflow",
    "runtime carrier compact original work",
    "runtime carrier compact visits overflow",
    "runtime carrier creation cutoff/cancellation",
    "JSON limits must be positive",
    "JSON parser state budget exceeded",
    "JSON structural budget exceeded",
    "expected JSON value",
    "expected array comma or close",
    "expected object comma or close",
    "invalid JSON literal",
    "unexpected JSON token",
    "unterminated JSON string",
    "incomplete JSON escape",
    "short Unicode escape",
    "invalid Unicode escape",
    "invalid JSON escape",
    "unescaped control in JSON string",
    "invalid JSON string",
    "object key must be string",
    "duplicate decoded JSON member",
    "leading zero",
    "missing integer digits",
    "missing fraction digits",
    "missing exponent digits",
    "integer digit budget exceeded",
    "nonfinite or unrepresentable float",
    "trailing JSON input",
    "JSON input is not UTF-8",
    "JSON byte budget exceeded",
    "JSON output byte budget exceeded",
    "JSON output structural budget exceeded",
    "JSON visit counter overflow",
    "JSON writer budget exceeded",
    "JSON array is too large",
    "canonical writer state/visit budget",
    "JSON output key is not a Unicode scalar string",
    "canonical UTF-8 cannot encode a lone surrogate",
    "number lexeme and kind disagree",
    "float lexeme is invalid",
    "JSON retained storage overflow",
    "JSON retained container storage overflow",
    "JSON string retained storage overflow",
];

fn foundation_json_error(error: tos_foundation::FoundationError) -> Error {
    // Foundation JSON diagnostics on these parse/write paths are fixed
    // mechanical messages. Keep only known strings so a future Foundation
    // detail cannot carry source text, provider text, or a path across the
    // compiler error boundary. The code and optional numeric byte offset stay
    // typed and bounded even for an unknown detail.
    let message = FOUNDATION_JSON_DIAGNOSTICS
        .iter()
        .copied()
        .find(|known| *known == error.detail)
        .unwrap_or("unclassified Foundation JSON error");
    Error::FoundationJson {
        code: error.code,
        message,
        byte_offset: error.byte_offset,
    }
}

#[cfg(test)]
mod foundation_json_error_tests {
    use super::*;

    #[test]
    fn preserves_known_cause_and_redacts_unknown_detail_through_cold_bridge() {
        let known = foundation_json_error(
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "JSON output byte budget exceeded",
            )
            .at(17),
        );
        assert_eq!(
            known.to_string(),
            "Foundation JSON budget_exceeded at byte 17: JSON output byte budget exceeded"
        );
        assert_eq!(
            crate::ColdOperationFailure::from(known).to_string(),
            "Foundation JSON budget_exceeded at byte 17: JSON output byte budget exceeded"
        );

        let unknown = foundation_json_error(tos_foundation::FoundationError::new(
            tos_foundation::FoundationErrorCode::BudgetExceeded,
            "private provider text /srv/private/source.json",
        ));
        let rendered = unknown.to_string();
        assert_eq!(
            rendered,
            "Foundation JSON budget_exceeded: unclassified Foundation JSON error"
        );
        assert!(!rendered.contains("private provider text"));
        assert!(!rendered.contains("/srv/private/source.json"));
    }
}

fn selected_rows(role: &str, collection: &str) -> bool {
    match role {
        "evidence-corpus" => matches!(collection, "nodes" | "relation_edges"),
        "evidence-philosophy" => collection == "views",
        CORPUS => matches!(
            collection,
            "diagnostics"
                | "nodes"
                | "resources"
                | "manifests"
                | "branches"
                | "graph_views"
                | "relation_edges"
                | "relation_packs"
                | "source_navigation/nodes"
                | "source_navigation/edges"
                | "source_navigation/rights"
        ),
        PHILOSOPHY => matches!(
            collection,
            "nodes" | "edges" | "clusters" | "views" | "review_packets" | "graph_layers"
        ),
        CLAIMS => matches!(
            collection,
            "nodes" | "edges" | "claim_traces" | "input_digests"
        ),
        _ => false,
    }
}

fn valid_top_level_collection(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Logical creation-state ownership. The outer callback already retains the
/// original request/Driver/Stage/transport owners. Every live constructor
/// allocation is additive here; guards release only actually dropped locals.
/// This draft is not connected until all maintained creation paths use it.
pub(crate) struct CreationState<'a> {
    remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
    retained: Cell<usize>,
    persistent: Cell<usize>,
    json_visits: Cell<usize>,
    max_json_visits: usize,
    sql_vm: Arc<AtomicU64>,
    sql_vm_limit: u64,
    sqlite_heap: Arc<sqlite_budget::DedicatedSessionSqliteHeap>,
    work: Arc<AtomicU64>,
    work_limit: u64,
    deadline: Instant,
    cancelled: &'a std::sync::atomic::AtomicBool,
    cancelled_handle: Arc<std::sync::atomic::AtomicBool>,
    // Only model/read state borrows its authentic retained capture phase owner.
    capture_owner: Option<&'a PublicCapture>,
}
pub(crate) struct CreationStateHold<'a, 'budget> {
    owner: &'a CreationState<'budget>,
    admitted: usize,
}
impl<'budget> CreationState<'budget> {
    /// Borrow the dedicated runtime's original owners for one operation.
    /// The caller owns simultaneous model/transport/DTO state and records JSON
    /// usage on both outcomes before terminating on error; no counter resets.
    pub(crate) fn from_runtime_owned_budget<'a>(
        budget: &'a crate::knowledge_payload_read::RuntimeKnowledgeOwnedBudget<'a>,
    ) -> Result<CreationState<'a>> {
        if budget.operation_deadline > budget.owner_deadline || budget.remaining_json_visits == 0 {
            return Err(Error::Invalid(
                "runtime payload original lifetime/JSON context",
            ));
        }
        check_capture_active(Some(budget.cancelled.as_ref()), budget.operation_deadline)?;
        if budget
            .original_work
            .load(std::sync::atomic::Ordering::Acquire)
            > budget.original_work_limit
            || budget
                .original_sql_vm
                .load(std::sync::atomic::Ordering::Acquire)
                > budget.original_sql_vm_limit
        {
            return Err(Error::Budget("runtime payload original counter exhausted"));
        }
        // Same maintained depth-96 parser frame admission as the model factory.
        // Existing connections and process heap remain held in the caller's
        // remaining callback; this owner adds only its parser/control frame.
        let frame =
            (std::mem::size_of::<serde_json::Value>() + std::mem::size_of::<JsonValue>() + 512)
                .checked_mul(97)
                .and_then(|n| n.checked_add(std::mem::size_of::<CreationState<'_>>()))
                .ok_or(Error::Budget("runtime payload parser frame state"))?;
        (budget.remaining_after_retained)(frame)?;
        budget.original_sqlite_heap.verify_current()?;
        Ok(CreationState {
            remaining_after_retained: budget.remaining_after_retained,
            retained: Cell::new(frame),
            persistent: Cell::new(0),
            json_visits: Cell::new(0),
            max_json_visits: budget.remaining_json_visits,
            sql_vm: Arc::clone(budget.original_sql_vm),
            sql_vm_limit: budget.original_sql_vm_limit,
            sqlite_heap: Arc::clone(budget.original_sqlite_heap),
            work: Arc::clone(budget.original_work),
            work_limit: budget.original_work_limit,
            deadline: budget.operation_deadline,
            cancelled: budget.cancelled.as_ref(),
            cancelled_handle: Arc::clone(budget.cancelled),
            capture_owner: None,
        })
    }
    pub(crate) fn remaining_json_visits(&self) -> Result<usize> {
        self.active()?;
        self.max_json_visits
            .checked_sub(self.json_visits.get())
            .filter(|n| *n > 0)
            .ok_or(Error::Budget("owned original remaining JSON visits"))
    }
    pub(crate) fn debit_json_visits(&self, used: usize) -> Result<()> {
        let next = self
            .json_visits
            .get()
            .checked_add(used)
            .ok_or(Error::Budget("owned original JSON usage overflow"))?;
        self.json_visits.set(next);
        if next > self.max_json_visits {
            return Err(Error::Budget("owned original JSON usage"));
        }
        self.active()
    }
    pub(crate) fn json_visits(&self) -> usize {
        self.json_visits.get()
    }
    pub(crate) fn active(&self) -> Result<()> {
        self.remaining(0).map(|_| ())
    }
    pub(crate) fn sql_vm_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.sql_vm)
    }
    pub(crate) fn operation_deadline(&self) -> Instant {
        self.deadline
    }
    pub(crate) fn cancellation_handle(&self) -> Arc<std::sync::atomic::AtomicBool> {
        Arc::clone(&self.cancelled_handle)
    }
    pub(crate) fn sql_vm_limit(&self) -> u64 {
        self.capture_owner.map_or(self.sql_vm_limit, |owner| {
            owner
                .active_vm_limit()
                .map_or(0, |limit| limit.min(self.sql_vm_limit))
        })
    }
    pub(crate) fn heap(&self) -> &Arc<sqlite_budget::DedicatedSessionSqliteHeap> {
        &self.sqlite_heap
    }
    #[track_caller]
    pub(crate) fn json<'a>(&'a self, raw: &[u8], cap: usize) -> Result<CreationJson<'a>> {
        creation_json(self, raw, cap)
    }
    /// Strict grammar is parsed once. Decoded text and key ownership move from
    /// the metered Foundation tree into serde under the same operation budget;
    /// original parser visitor usage persists on success and failure.
    #[track_caller]
    pub(crate) fn serde_owned(&self, raw: &[u8], cap: usize) -> Result<serde_json::Value> {
        let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("owned model serde limits"))?;
        self.serde_owned_with_limits(raw, limits)
    }
    #[track_caller]
    pub(crate) fn with_foundation_owned_with_limits<T>(
        &self,
        raw: &[u8],
        limits: JsonLimits,
        operation: impl FnOnce(&JsonValue) -> Result<T>,
    ) -> Result<T> {
        let document = creation_json_with_limits(self, raw, limits)?;
        let result = operation(&document);
        drop(document);
        result
    }
    #[track_caller]
    pub(crate) fn foundation_owned_with_limits(
        &self,
        raw: &[u8],
        limits: JsonLimits,
    ) -> Result<JsonValue> {
        creation_json_with_limits(self, raw, limits)?.into_retained_root()
    }
    #[track_caller]
    pub(crate) fn serde_owned_with_limits(
        &self,
        raw: &[u8],
        limits: JsonLimits,
    ) -> Result<serde_json::Value> {
        let (value, hold) = self.serde_scoped_with_limits(raw, limits)?;
        hold.into_persistent()?;
        Ok(value)
    }
    #[track_caller]
    pub(crate) fn serde_scoped_with_limits<'s>(
        &'s self,
        raw: &[u8],
        limits: JsonLimits,
    ) -> Result<(serde_json::Value, CreationStateHold<'s, 'budget>)> {
        let document = creation_json_with_limits(self, raw, limits)?;
        // The strict Foundation parser already owns decoded strings, exact
        // number lexemes and insertion order. Transfer them into the consumer
        // representation; retain the original bytes separately at their owner.
        let extra = self.serde_transfer_workspace_upper(&document)?;
        let mut output_hold = self.hold(extra)?;
        let CreationJson { value, _hold } = document;
        let mut input_hold = _hold.ok_or(Error::Invalid("strict transfer input hold absent"))?;
        let value = self.transfer_foundation_value(value, 0)?;
        // Both guards were continuously charged during conversion. Move the
        // original string/number custody into the result guard before releasing
        // old container geometry. No live value receives a temporary refund.
        output_hold.admitted = output_hold.admitted.checked_add(input_hold.admitted)
            .ok_or(Error::Budget("strict value custody transfer overflow"))?;
        input_hold.admitted = 0;
        drop(input_hold);
        output_hold.finish_value_construction(&value)?;
        Ok((value, output_hold))
    }
    fn serde_transfer_workspace_upper(&self, value: &JsonValue) -> Result<usize> {
        fn add(left: usize, right: usize) -> Result<usize> {
            left.checked_add(right).ok_or(Error::Budget("strict value conversion state"))
        }
        fn walk(state: &CreationState<'_>, value: &JsonValue, depth: usize) -> Result<usize> {
            state.charge_work(1)?;
            if depth > 96 { return Err(Error::Budget("strict value conversion depth")); }
            match value {
                // UTF8 strings and object keys are moved from the admitted
                // Foundation tree, so conversion allocates no replacement text.
                JsonValue::Null | JsonValue::Bool(_) | JsonValue::String(_) => Ok(0),
                JsonValue::Number(n) => n.lexeme.len().checked_mul(4)
                    .and_then(|n| n.checked_add(128))
                    .ok_or(Error::Budget("strict numeric conversion state")),
                JsonValue::Array(rows) => {
                    let mut bytes = rows.len().checked_mul(std::mem::size_of::<serde_json::Value>())
                        .ok_or(Error::Budget("strict array conversion state"))?;
                    for row in rows { bytes = add(bytes, walk(state, row, depth + 1)?)?; }
                    Ok(bytes)
                }
                JsonValue::Object(fields) => {
                    let mut bytes = crate::knowledge_normalization::serde_object_slots_upper(fields.len())?;
                    for (_, value) in fields { bytes = add(bytes, walk(state, value, depth + 1)?)?; }
                    Ok(bytes)
                }
            }
        }
        let frames = 97usize.checked_mul(std::mem::size_of::<JsonValue>() + std::mem::size_of::<serde_json::Value>() + 512)
            .ok_or(Error::Budget("strict conversion frame state"))?;
        add(frames, walk(self, value, 0)?)
    }
    fn transfer_foundation_value(&self, value: JsonValue, depth: usize) -> Result<serde_json::Value> {
        self.charge_work(1)?;
        if depth > 96 { return Err(Error::Budget("strict value conversion depth")); }
        use serde_json::Value as V;
        Ok(match value {
            JsonValue::Null => V::Null,
            JsonValue::Bool(v) => V::Bool(v),
            JsonValue::String(v) => V::String(v.into_utf8().ok_or(Error::Invalid("strict serde unpaired surrogate"))?),
            JsonValue::Number(n) => {
                self.charge_work(n.lexeme.len())?;
                V::Number(n.lexeme.parse().map_err(|_| Error::Invalid("strict serde number conversion"))?)
            }
            JsonValue::Array(rows) => {
                let mut output = Vec::with_capacity(rows.len());
                for row in rows { output.push(self.transfer_foundation_value(row, depth + 1)?); }
                V::Array(output)
            }
            JsonValue::Object(fields) => {
                let mut output = serde_json::Map::with_capacity(fields.len());
                for (key, value) in fields {
                    let key = key.into_utf8().ok_or(Error::Invalid("strict serde unpaired key surrogate"))?;
                    self.charge_work(key.len())?;
                    if output.insert(key, self.transfer_foundation_value(value, depth + 1)?).is_some() {
                        return Err(Error::Invalid("strict conversion duplicate key"));
                    }
                }
                V::Object(output)
            }
        })
    }

    #[track_caller]
    pub(crate) fn with_serde_owned_with_limits<T>(
        &self,
        raw: &[u8],
        limits: JsonLimits,
        operation: impl FnOnce(&serde_json::Value) -> Result<T>,
    ) -> Result<T> {
        let (value, hold) = self.serde_scoped_with_limits(raw, limits)?;
        let result = operation(&value);
        drop(value);
        drop(hold);
        result
    }
    #[track_caller]
    pub(crate) fn with_serde_owned_value_with_limits<T>(
        &self,
        raw: &[u8],
        limits: JsonLimits,
        operation: impl FnOnce(serde_json::Value) -> Result<T>,
    ) -> Result<T> {
        let (value, hold) = self.serde_scoped_with_limits(raw, limits)?;
        // Internal wrappers keep the moved value inside the callback; any
        // returned output has independently admitted ownership.
        let result = operation(value);
        drop(hold);
        result
    }
    /// Recheck only composition-sensitive limits of an already strictly parsed
    /// tree. Keys, numeric lexemes and UTF8 were admitted with its constituents.
    pub(crate) fn check_serde_structure(&self, value: &serde_json::Value, limits: JsonLimits) -> Result<()> {
        fn walk(state: &CreationState<'_>, value: &serde_json::Value, limits: JsonLimits,
                depth: usize, visited: &mut usize) -> Result<()> {
            state.charge_work(1)?;
            if depth > limits.max_depth.min(96) || *visited >= limits.max_visits
                || state.json_visits.get() >= state.max_json_visits {
                return Err(Error::Budget("owned composed JSON structure"));
            }
            *visited += 1;
            state.json_visits.set(state.json_visits.get() + 1);
            match value {
                serde_json::Value::Array(rows) => {
                    for row in rows { walk(state, row, limits, depth + 1, visited)?; }
                }
                serde_json::Value::Object(fields) => {
                    for row in fields.values() { walk(state, row, limits, depth + 1, visited)?; }
                }
                _ => {}
            }
            Ok(())
        }
        let _frames = self.hold(97 * 128)?;
        walk(self, value, limits, 0, &mut 0)
    }
    #[cfg(test)]
    fn decode_serde_raw(&self, raw: &[u8]) -> Result<serde_json::Value> {
        // The original raw input is borrowed. Every byte supplied to serde is
        // byte-work/cutoff checked; no independent parser clock/counter.
        struct CheckedRead<'a, 'budget> {
            raw: &'a [u8],
            at: usize,
            owner: &'a CreationState<'budget>,
        }
        impl Read for CheckedRead<'_, '_> {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                self.owner
                    .remaining(0)
                    .map_err(|_| io::Error::other("owned model decode cutoff"))?;
                let count = output.len().min(self.raw.len() - self.at);
                checked_add(&self.owner.work, count, self.owner.work_limit)
                    .map_err(|_| io::Error::other("owned model decode byte work"))?;
                output[..count].copy_from_slice(&self.raw[self.at..self.at + count]);
                self.at += count;
                Ok(count)
            }
        }
        let input = CheckedRead {
            raw,
            at: 0,
            owner: self,
        };
        let mut decoder = serde_json::Deserializer::from_reader(input);
        let value = CheckedValueSeed {
            creation_read: None,
        }
        .deserialize(&mut decoder)
        .map_err(|_| Error::Invalid("owned model strict serde decode"))?;
        decoder
            .end()
            .map_err(|_| Error::Invalid("owned model serde trailing input"))?;
        self.remaining(0)?;
        Ok(value)
    }
    pub(crate) fn observed_work_bytes(&self) -> u64 {
        self.work.load(std::sync::atomic::Ordering::Acquire)
    }
    #[track_caller]
    pub(crate) fn charge_work(&self, bytes: usize) -> Result<()> {
        self.remaining(0)?;
        let limit = self.capture_owner.map_or(Ok(self.work_limit), |owner| {
            owner
                .active_work_limit()
                .map(|limit| limit.min(self.work_limit))
        })?;
        checked_add(&self.work, bytes, limit)
    }
    pub(crate) fn encode_json<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
        cap: usize,
    ) -> Result<Vec<u8>> {
        let (raw, hold) = self.json_encoded_result(value, cap)?;
        hold.into_persistent()?;
        Ok(raw)
    }
    pub(crate) fn with_json_encoded<T: serde::Serialize + ?Sized, O>(
        &self,
        value: &T,
        cap: usize,
        operation: impl FnOnce(&[u8]) -> Result<O>,
    ) -> Result<O> {
        let (raw, hold) = self.json_encoded_result(value, cap)?;
        let result = operation(&raw);
        drop(raw);
        drop(hold);
        result
    }
    /// A verified carrier already declares its exact logical length. Emit once
    /// into that admitted capacity, then check length before exposing bytes.
    /// The consumer still authenticates the logical digest under this owner.
    pub(crate) fn with_json_encoded_exact<T: serde::Serialize + ?Sized, O>(
        &self,
        value: &T,
        exact_len: usize,
        cap: usize,
        operation: impl FnOnce(&[u8]) -> Result<O>,
    ) -> Result<O> {
        let (raw, hold) = self.json_encoded_exact_result(value, exact_len, cap)?;
        let result = operation(&raw);
        drop(raw);
        drop(hold);
        result
    }
    fn json_encoded_result<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
        cap: usize,
    ) -> Result<(Vec<u8>, CreationStateHold<'_, 'budget>)> {
        struct Count<'a, 'budget> {
            owner: &'a CreationState<'budget>,
            len: usize,
            cap: usize,
        }
        impl Write for Count<'_, '_> {
            fn write(&mut self, raw: &[u8]) -> io::Result<usize> {
                self.owner
                    .charge_work(raw.len())
                    .map_err(|_| io::Error::other("owned model count work"))?;
                self.len = self.len.checked_add(raw.len())
                    .filter(|n| *n <= self.cap)
                    .ok_or_else(|| io::Error::other("owned model encode bytes"))?;
                Ok(raw.len())
            }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let mut count = Count { owner: self, len: 0, cap };
        serde_json::to_writer(&mut count, value)
            .map_err(|_| Error::Budget("owned model encode count"))?;
        self.json_encoded_exact_result(value, count.len, cap)
    }
    fn json_encoded_exact_result<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
        exact_len: usize,
        cap: usize,
    ) -> Result<(Vec<u8>, CreationStateHold<'_, 'budget>)> {
        if exact_len == 0 || exact_len > cap {
            return Err(Error::Budget("owned model exact encode bytes"));
        }
        let hold = self.hold(exact_len)?;
        struct Emit<'a, 'budget> {
            owner: &'a CreationState<'budget>,
            raw: Vec<u8>,
            cap: usize,
        }
        impl Write for Emit<'_, '_> {
            fn write(&mut self, raw: &[u8]) -> io::Result<usize> {
                self.owner.charge_work(raw.len())
                    .map_err(|_| io::Error::other("owned model emit work"))?;
                self.raw.len().checked_add(raw.len())
                    .filter(|n| *n <= self.cap)
                    .ok_or_else(|| io::Error::other("owned model changed encoded size"))?;
                self.raw.extend_from_slice(raw);
                Ok(raw.len())
            }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let mut raw = Vec::new();
        raw.try_reserve_exact(exact_len)
            .map_err(|_| Error::Budget("owned model encode allocation"))?;
        if raw.capacity() != exact_len {
            return Err(Error::Budget("owned model encode allocation capacity"));
        }
        let mut writer = Emit { owner: self, raw, cap: exact_len };
        serde_json::to_writer(&mut writer, value)
            .map_err(|_| Error::Budget("owned model encode emit"))?;
        if writer.raw.len() != exact_len {
            return Err(Error::Invalid("owned model encode count changed"));
        }
        self.active()?;
        Ok((writer.raw, hold))
    }
    pub(crate) fn encode_canonical<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
        cap: usize,
    ) -> Result<Vec<u8>> {
        let raw = self.encode_json(value, cap)?;
        let document = self.json(&raw, cap)?;
        let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("owned model canonical limits"))?;
        self.encode_foundation_canonical_with_limits(&document, limits)
    }
    pub(crate) fn with_foundation_compact_bytes<T>(
        &self,
        value: &JsonValue,
        cap: usize,
        source_bytes: usize,
        operation: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        let bytes = compact_with_owned_state(self, value, cap, source_bytes)?;
        operation(&bytes)
    }
    pub(crate) fn clone_foundation(&self, value: &JsonValue) -> Result<JsonValue> {
        let upper = value
            .retained_storage_bytes()
            .map_err(|_| Error::Budget("owned Foundation clone state"))?;
        self.retain(upper)?;
        self.charge_work(upper)?;
        Ok(value.clone())
    }
    pub(crate) fn encode_foundation_canonical_with_limits(
        &self,
        value: &JsonValue,
        limits: JsonLimits,
    ) -> Result<Vec<u8>> {
        let encoded = self.foundation_canonical_result(value, limits)?;
        self.retain(encoded.capacity())?;
        Ok(encoded)
    }
    pub(crate) fn with_foundation_canonical_bytes<T>(
        &self,
        value: &JsonValue,
        limits: JsonLimits,
        operation: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        let encoded = self.foundation_canonical_result(value, limits)?;
        let hold = self.hold(encoded.capacity())?;
        let result = operation(&encoded);
        drop(encoded);
        drop(hold);
        result
    }
    fn foundation_canonical_result(
        &self,
        value: &JsonValue,
        mut limits: JsonLimits,
    ) -> Result<Vec<u8>> {
        let before = self.json_visits.get();
        limits.max_visits = limits.max_visits.min(
            self.max_json_visits
                .checked_sub(before)
                .filter(|n| *n > 0)
                .ok_or(Error::Budget("owned model canonical JSON visits"))?,
        );
        limits.max_depth = limits.max_depth.min(96);
        limits.max_integer_digits = limits.max_integer_digits.min(4096);
        let available = self.remaining(0)?;
        let mut check = || {
            self.remaining(0).map(|_| ()).map_err(|_| {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "owned model canonical cutoff",
                )
            })
        };
        let mut admit = |bytes: usize, visits: usize| {
            let failure = || {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "owned model canonical work/visits",
                )
            };
            let total = self
                .json_visits
                .get()
                .checked_add(visits)
                .filter(|n| *n <= self.max_json_visits)
                .ok_or_else(failure)?;
            self.charge_work(bytes.checked_add(visits).ok_or_else(failure)?)
                .map_err(|_| failure())?;
            self.json_visits.set(total);
            Ok(())
        };
        let (encoded, used) =
            tos_foundation::canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                value,
                tos_foundation::CanonicalProfile::SourceRecordDigestV1,
                limits,
                available,
                &mut check,
                &mut admit,
            )
            .map_err(|_| Error::Budget("owned model canonical admission"))?;
        if self.json_visits.get().checked_sub(before) != Some(used) {
            return Err(Error::Invalid("owned model canonical visit accounting"));
        }
        Ok(encoded)
    }
    pub(crate) fn value_clone_state_upper_bound(&self, value: &serde_json::Value) -> Result<usize> {
        fn count(
            state: &CreationState<'_>,
            value: &serde_json::Value,
            depth: usize,
        ) -> Result<usize> {
            if depth > 96 {
                return Err(Error::Budget("owned model clone depth"));
            }
            state.charge_work(std::mem::size_of::<serde_json::Value>())?;
            let mut bytes = match value {
                serde_json::Value::Null | serde_json::Value::Bool(_) => 0,
                serde_json::Value::Number(v) => v.as_str().len(),
                serde_json::Value::String(v) => v.len(),
                serde_json::Value::Array(values) => values
                    .len()
                    .checked_mul(std::mem::size_of::<serde_json::Value>())
                    .ok_or(Error::Budget("owned model clone array"))?,
                serde_json::Value::Object(fields) => {
                    crate::knowledge_normalization::serde_object_slots_upper(fields.len())?
                }
            };
            match value {
                serde_json::Value::Array(values) => {
                    for item in values {
                        bytes = bytes
                            .checked_add(count(state, item, depth + 1)?)
                            .ok_or(Error::Budget("owned model clone geometry"))?;
                    }
                }
                serde_json::Value::Object(fields) => {
                    for (key, item) in fields {
                        state.charge_work(key.len())?;
                        let child = count(state, item, depth + 1)?;
                        bytes = bytes
                            .checked_add(key.len())
                            .and_then(|n| n.checked_add(child))
                            .ok_or(Error::Budget("owned model clone geometry"))?;
                    }
                }
                serde_json::Value::String(v) => state.charge_work(v.len())?,
                serde_json::Value::Number(v) => state.charge_work(v.as_str().len())?,
                _ => {}
            }
            Ok(bytes)
        }
        count(self, value, 0)
    }
    fn clone_value_mode(
        &self,
        value: &serde_json::Value,
        persistent: bool,
    ) -> Result<serde_json::Value> {
        fn admit(state: &CreationState<'_>, persistent: bool, bytes: usize) -> Result<()> {
            if persistent {
                state.retain(bytes)
            } else {
                state.remaining(0).map(|_| ())
            }
        }
        fn copy(
            state: &CreationState<'_>,
            value: &serde_json::Value,
            depth: usize,
            persistent: bool,
        ) -> Result<serde_json::Value> {
            if depth > 96 {
                return Err(Error::Budget("owned model clone depth"));
            }
            state.charge_work(std::mem::size_of::<serde_json::Value>())?;
            Ok(match value {
                serde_json::Value::Null => serde_json::Value::Null,
                serde_json::Value::Bool(v) => serde_json::Value::Bool(*v),
                serde_json::Value::Number(v) => {
                    admit(state, persistent, v.as_str().len())?;
                    state.charge_work(v.as_str().len())?;
                    serde_json::Value::Number(v.clone())
                }
                serde_json::Value::String(v) => {
                    admit(state, persistent, v.len())?;
                    state.charge_work(v.len())?;
                    serde_json::Value::String(v.clone())
                }
                serde_json::Value::Array(values) => {
                    // Exact-reserved clone vector; old input remains accounted.
                    admit(
                        state,
                        persistent,
                        values
                            .len()
                            .checked_mul(std::mem::size_of::<serde_json::Value>())
                            .ok_or(Error::Budget("owned model clone array"))?,
                    )?;
                    let mut output = Vec::with_capacity(values.len());
                    for item in values {
                        output.push(copy(state, item, depth + 1, persistent)?);
                    }
                    serde_json::Value::Array(output)
                }
                serde_json::Value::Object(fields) => {
                    admit(
                        state,
                        persistent,
                        crate::knowledge_normalization::serde_object_slots_upper(fields.len())?,
                    )?;
                    let mut output = serde_json::Map::new();
                    for (key, item) in fields {
                        admit(state, persistent, key.len())?;
                        state.charge_work(key.len())?;
                        output.insert(key.clone(), copy(state, item, depth + 1, persistent)?);
                    }
                    serde_json::Value::Object(output)
                }
            })
        }
        let output = copy(self, value, 0, persistent)?;
        self.remaining(0)?;
        Ok(output)
    }
    pub(crate) fn clone_value(&self, value: &serde_json::Value) -> Result<serde_json::Value> {
        self.clone_value_mode(value, true)
    }
    pub(crate) fn remaining(&self, prospective: usize) -> Result<usize> {
        let deadline = match self.capture_owner {
            Some(capture) => self.deadline.min(capture.active_deadline()?),
            None => self.deadline,
        };
        check_capture_active(Some(self.cancelled), deadline)?;
        let total = self
            .retained
            .get()
            .checked_add(prospective)
            .ok_or(Error::Budget("runtime carrier creation state overflow"))?;
        (self.remaining_after_retained)(total)
    }
    pub(crate) fn retain(&self, bytes: usize) -> Result<()> {
        self.remaining(bytes)?;
        self.retained.set(
            self.retained
                .get()
                .checked_add(bytes)
                .ok_or(Error::Budget("runtime persistent creation state"))?,
        );
        self.persistent.set(
            self.persistent
                .get()
                .checked_add(bytes)
                .ok_or(Error::Budget("runtime persistent creation state"))?,
        );
        Ok(())
    }
    fn transfer_persistent_to_capture(&self) -> Result<()> {
        // Only after the maintained constructor returned its actual capture:
        // ownership is then counted by retained_state_upper_bound, not twice.
        let bytes = self.persistent.replace(0);
        self.retained.set(
            self.retained
                .get()
                .checked_sub(bytes)
                .ok_or(Error::Budget("runtime persistent capture transfer"))?,
        );
        Ok(())
    }
    #[track_caller]
    pub(crate) fn hold<'owner>(
        &'owner self,
        bytes: usize,
    ) -> Result<CreationStateHold<'owner, 'budget>> {
        if let Err(error) = self.remaining(bytes) {
            let site = std::panic::Location::caller();
            eprintln!(
                "Native state hold refused at {}:{}: retained={} additional={}",
                site.file(),
                site.line(),
                self.retained.get(),
                bytes,
            );
            return Err(error);
        }
        let retained = self
            .retained
            .get()
            .checked_add(bytes)
            .ok_or(Error::Budget("runtime carrier creation state overflow"))?;
        self.retained.set(retained);
        Ok(CreationStateHold {
            owner: self,
            admitted: bytes,
        })
    }
}
// Controlled Linux route: explicit fixed getdents buffer, bounded names and
// exact Vec capacity avoid std::read_dir/opendir's hidden native heap.
fn controlled_ledger_workspace_upper() -> usize {
    32768 + 4096 * (2 * std::mem::size_of::<std::ffi::OsString>() + 255)
}
fn controlled_fence_workspace_upper(whole: bool) -> usize {
    // Existing source_digest stack, SQL row/statement Rust locals, bounded
    // pathname conversions. SQLite connection/cache live in the shared pool.
    65536
        + 4 * 8194
        + 2 * std::mem::size_of::<Connection>()
        + std::mem::size_of::<sqlite_budget::SharedVmWindow>()
        + 2 * std::mem::size_of::<(
            Arc<AtomicU64>,
            Arc<std::sync::atomic::AtomicBool>,
            Instant,
            u64,
        )>()
        + if whole {
            controlled_ledger_workspace_upper()
        } else {
            0
        }
}
#[cfg(target_os = "linux")]
fn controlled_ledger_names(
    path: &Path,
    mut observe: impl FnMut(usize) -> Result<()>,
) -> Result<Vec<std::ffi::OsString>> {
    use std::os::{
        fd::AsRawFd,
        unix::{ffi::OsStringExt, fs::OpenOptionsExt},
    };
    observe(0)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let mut names = Vec::with_capacity(4096);
    let mut buffer = [0u8; 32768];
    loop {
        observe(0)?;
        let count = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                file.as_raw_fd(),
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if count < 0 {
            return Err(io::Error::last_os_error().into());
        }
        if count == 0 {
            break;
        }
        let count = usize::try_from(count).map_err(|_| Error::Invalid("ledger dirent count"))?;
        if count > buffer.len() {
            return Err(Error::Invalid("ledger dirent buffer"));
        }
        let mut at = 0usize;
        while at < count {
            observe(0)?;
            if count - at < 20 {
                return Err(Error::Invalid("ledger dirent header"));
            }
            let len = u16::from_ne_bytes([buffer[at + 16], buffer[at + 17]]) as usize;
            if len < 20 || len > count - at {
                return Err(Error::Invalid("ledger dirent length"));
            }
            let bytes = &buffer[at + 19..at + len];
            let end = bytes
                .iter()
                .position(|b| *b == 0)
                .ok_or(Error::Invalid("ledger dirent termination"))?;
            let name = &bytes[..end];
            if name != b"." && name != b".." {
                if names.len() == 4096 || name.len() > 255 {
                    return Err(Error::Budget("public D1 ledger membership"));
                }
                observe(name.len())?;
                names.push(std::ffi::OsString::from_vec(name.to_vec()));
            }
            at += len;
        }
    }
    observe(0)?;
    names.sort();
    observe(0)?;
    Ok(names)
}

#[cfg(not(target_os = "linux"))]
fn controlled_ledger_names(
    _path: &Path,
    _observe: impl FnMut(usize) -> Result<()>,
) -> Result<Vec<std::ffi::OsString>> {
    Err(Error::Invalid("controlled native session requires Linux"))
}

// Same maintained path choice, admitted before PathBuf/String ownership.
fn creation_source_path(
    root: &Path,
    selected: Option<&Path>,
    relative: &str,
    creation: Option<&CreationState<'_>>,
) -> Result<PathBuf> {
    if let Some(state) = creation {
        let bytes = match selected {
            Some(path) => path.as_os_str().as_encoded_bytes().len(),
            None => root
                .as_os_str()
                .as_encoded_bytes()
                .len()
                .checked_add(relative.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Error::Budget("runtime source path width"))?,
        };
        // Joined path Vec growth/reallocation and safe_open temporary pathname,
        // plus the independently retained source label. Kept until capture
        // transfer; no release of a source that remains authenticated/live.
        state.retain(
            bytes
                .max(8)
                .checked_mul(4)
                .and_then(|n| n.checked_add(relative.len()))
                .ok_or(Error::Budget("runtime source path state"))?,
        )?;
    }
    Ok(selected
        .map(Path::to_owned)
        .unwrap_or_else(|| root.join(relative)))
}

impl CreationStateHold<'_, '_> {
    /// End constructor scratch ownership, retaining only the returned value's
    /// bounded heap geometry. This can release an admitted peak, never grow it.
    pub(crate) fn finish_value_construction(&mut self, value: &serde_json::Value) -> Result<()> {
        let heap = crate::knowledge_normalization::serde_retained_heap_upper_with_check(
            value, 0, &mut || self.owner.charge_work(1),
        )?;
        let upper = std::mem::size_of::<serde_json::Value>().checked_add(heap)
            .ok_or(Error::Budget("constructed value retained state overflow"))?;
        // Both are conservative bounds. Opaque Number storage can give the
        // retained walk a looser bound than the original decoder admission;
        // retain that already proved peak instead of minting more capacity.
        let retained = self.admitted.min(upper);
        let released = self.admitted - retained;
        let remaining = self.owner.retained.get().checked_sub(released)
            .ok_or(Error::Budget("constructed value state transfer mismatch"))?;
        self.owner.active()?;
        self.owner.retained.set(remaining);
        self.admitted = retained;
        Ok(())
    }

    /// Transfer already admitted state to the capture lifetime without another
    /// reservation or a transient refund while the returned value stays live.
    fn into_persistent(mut self) -> Result<()> {
        let persistent = self.owner.persistent.get().checked_add(self.admitted)
            .ok_or(Error::Budget("runtime persistent value transfer overflow"))?;
        self.owner.active()?;
        self.owner.persistent.set(persistent);
        self.admitted = 0;
        Ok(())
    }
}

impl Drop for CreationStateHold<'_, '_> {
    fn drop(&mut self) {
        // Every hold belongs to this one serial constructor. Keep the ledger
        // saturated on an internal mismatch instead of granting extra room.
        self.owner.retained.set(
            self.owner
                .retained
                .get()
                .checked_sub(self.admitted)
                .unwrap_or(usize::MAX),
        );
    }
}

// serde_json 1.0.151 IoRead appends to raw_buffer after its inner Read
// returns. Admit buffer growth before delivering that byte. The serde scratch
// Vec survives individual values. Conservatively keep the admitted raw/string
// envelope through each owned value. Reset only after that value is dropped;
// persistent typed strings/containers remain charged through this source.
struct CreationReadState<'a, 'budget> {
    owner: &'a CreationState<'budget>,
    stream_read: Cell<usize>,
    scratch_capacity: Cell<usize>,
    admitted: Cell<usize>,
    tree_admitted: Cell<usize>,
}
impl CreationReadState<'_, '_> {
    fn buffer_capacity(bytes: usize) -> Result<usize> {
        if bytes == 0 {
            return Ok(0);
        }
        bytes
            .max(8)
            .checked_next_power_of_two()
            .ok_or(Error::Budget("runtime carrier serde buffer capacity"))
    }
    fn replace_admission(&self, bytes: usize) -> Result<()> {
        let old = self.admitted.get();
        if bytes > old {
            let extra = bytes - old;
            self.owner.remaining(extra)?;
            let total = self
                .owner
                .retained
                .get()
                .checked_add(extra)
                .ok_or(Error::Budget("runtime carrier serde state overflow"))?;
            self.owner.retained.set(total);
        } else {
            let total = self
                .owner
                .retained
                .get()
                .checked_sub(old - bytes)
                .ok_or(Error::Budget("runtime carrier serde state mismatch"))?;
            self.owner.retained.set(total);
        }
        self.admitted.set(bytes);
        Ok(())
    }
    fn admit_tree(&self, bytes: usize) -> Result<()> {
        self.owner.remaining(bytes)?;
        self.owner.retained.set(
            self.owner
                .retained
                .get()
                .checked_add(bytes)
                .ok_or(Error::Budget("runtime carrier typed tree state"))?,
        );
        self.tree_admitted.set(
            self.tree_admitted
                .get()
                .checked_add(bytes)
                .ok_or(Error::Budget("runtime carrier typed tree state"))?,
        );
        Ok(())
    }
    fn begin_value(&self) {
        // Called only after the preceding owned RawValue/field callback has
        // dropped. Persistent decoded keys/header strings are charged to the
        // typed tree pool; serde's scratch high-water remains admitted.
        self.stream_read.set(0);
    }
    fn before_read(&self, requested: usize) -> Result<()> {
        // The already-reserved creation allowance is one serial work domain.
        // Include requested bytes even on EOF/refusal, before serde executes.
        checked_add(&self.owner.work, requested, self.owner.work_limit)?;
        let next = self
            .stream_read
            .get()
            .checked_add(requested)
            .ok_or(Error::Budget("runtime carrier serde bytes overflow"))?;
        // IoRead can reuse one previously peeked byte without another Read.
        let capacity = Self::buffer_capacity(
            next.checked_add(1)
                .ok_or(Error::Budget("runtime carrier serde lookahead"))?,
        )?;
        let scratch = self.scratch_capacity.get().max(capacity);
        // Named owners: persistent de::scratch, IoRead::raw_buffer growth
        // including old/new storage overlap, and String::into_boxed_str output.
        // CheckedValueSeed uses the latter envelope for its two key/string
        // copies; typed tree/container slots are admitted separately by seeds.
        let raw_storage = capacity
            .checked_mul(3)
            .ok_or(Error::Budget("runtime carrier serde raw state"))?;
        // Additional named UTF8 owners across this source: retained header
        // strings, root seen keys, ordinal keys, and current key/collection
        // copies. Each originates from at most the delivered source prefix.
        let key_storage = capacity
            .checked_mul(5)
            .ok_or(Error::Budget("runtime carrier serde key state"))?;
        let admitted = scratch
            .checked_add(raw_storage)
            .and_then(|n| n.checked_add(key_storage))
            // serde arbitrary_precision's private number map key is synthetic.
            .and_then(|n| n.checked_add(128))
            .ok_or(Error::Budget("runtime carrier serde total state"))?;
        self.replace_admission(admitted)?;
        self.scratch_capacity.set(scratch);
        self.stream_read.set(next);
        Ok(())
    }
}
impl Drop for CreationReadState<'_, '_> {
    fn drop(&mut self) {
        self.owner.retained.set(
            self.owner
                .retained
                .get()
                .checked_sub(self.admitted.get().saturating_add(self.tree_admitted.get()))
                .unwrap_or(usize::MAX),
        );
    }
}

pub(crate) struct CreationJson<'a> {
    value: JsonValue,
    _hold: Option<CreationStateHold<'a, 'a>>,
}
impl CreationJson<'_> {
    fn into_retained_root(self) -> Result<JsonValue> {
        let Self { value, _hold } = self;
        let hold = _hold.ok_or(Error::Invalid("owned Foundation document hold absent"))?;
        hold.owner.remaining(0)?;
        hold.owner.persistent.set(
            hold.owner
                .persistent
                .get()
                .checked_add(hold.admitted)
                .ok_or(Error::Budget("owned Foundation hold transfer"))?,
        );
        // The same already admitted storage remains live. Only its lifetime
        // changes from temporary guard to the producer phase; no double grant.
        std::mem::forget(hold);
        Ok(value)
    }
}
impl std::ops::Deref for CreationJson<'_> {
    type Target = JsonValue;
    fn deref(&self) -> &JsonValue {
        &self.value
    }
}
impl std::ops::DerefMut for CreationJson<'_> {
    fn deref_mut(&mut self) -> &mut JsonValue {
        &mut self.value
    }
}

pub(crate) fn foundation_scoped<'a>(
    raw: &[u8],
    cap: usize,
    owner: Option<&'a CreationState<'a>>,
) -> Result<CreationJson<'a>> {
    match owner {
        Some(owner) => creation_json(owner, raw, cap),
        None => Ok(CreationJson {
            value: json(raw, cap)?,
            _hold: None,
        }),
    }
}

fn compact_with_owned_state<'a>(
    owner: &'a CreationState<'a>,
    value: &JsonValue,
    cap: usize,
    source_bytes: usize,
) -> Result<CreationBytes<'a>> {
    let before = owner.json_visits.get();
    let allowance = owner
        .max_json_visits
        .checked_sub(before)
        .filter(|value| *value > 0)
        .ok_or(Error::Budget("runtime carrier compact visits"))?
        .min(1_000_000)
        .min(
            source_bytes
                .checked_mul(6)
                .and_then(|n| n.checked_add(2))
                .ok_or(Error::Budget("runtime carrier compact visit bound"))?,
        );
    // Two passes each visit values, keys and numeric-lexeme validation;
    // every such token originates in the bounded original input bytes.
    // PythonPublishedCompact cannot expand an original token beyond this
    // finite bound: quoted UTF-16 units need <=6 bytes, finite binary64
    // shortest spelling fits 32 bytes, integer digits retain their input
    // width, and container punctuation is already present in source bytes.
    let output_cap = source_bytes
        .checked_mul(32)
        .and_then(|n| n.checked_add(2))
        .ok_or(Error::Budget("runtime carrier compact token bound"))?
        .min(cap);
    let limits = JsonLimits::new(output_cap, 96, allowance, 4096)
        .map_err(|_| Error::Budget("runtime carrier compact limits"))?;
    let available = owner.remaining(0)?;
    let mut check = || {
        check_capture_active(Some(owner.cancelled), owner.deadline).map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "runtime carrier compact cutoff/cancellation",
            )
        })
    };
    let work = &owner.work;
    let limit = owner.work_limit;
    let mut admit = |bytes: usize, visits: usize| {
        let amount = bytes.checked_add(visits).ok_or_else(|| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "runtime carrier compact work overflow",
            )
        })?;
        checked_add(work, amount, limit).map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "runtime carrier compact original work",
            )
        })?;
        owner
            .json_visits
            .set(owner.json_visits.get().checked_add(visits).ok_or_else(|| {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "runtime carrier compact visits overflow",
                )
            })?);
        Ok(())
    };
    let (bytes, visits) = emit_python_compact_json_with_state_budget_and_visits_and_check(
        value, limits, available, &mut check, &mut admit,
    )
    .map_err(foundation_json_error)?;
    owner.json_visits.set(
        before
            .checked_add(visits)
            .ok_or(Error::Budget("runtime carrier compact visits"))?,
    );
    let hold = owner.hold(bytes.capacity())?;
    Ok(CreationBytes {
        bytes,
        _hold: Some(hold),
    })
}

struct CreationBytes<'a> {
    bytes: Vec<u8>,
    _hold: Option<CreationStateHold<'a, 'a>>,
}
impl std::ops::Deref for CreationBytes<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

struct CaptureWriter<'a> {
    creation: Option<&'a CreationState<'a>>,
    creation_read: Option<Rc<CreationReadState<'a, 'a>>>,
    db: &'a Connection,
    role: &'static str,
    row_count: &'a mut u64,
    work: &'a AtomicU64,
    limits: PublicCaptureLimits,
    ordinals: Vec<(String, u64)>,
    ordinal_slots_admitted: usize,
    read_budget: Rc<Cell<usize>>,
    deadline: Instant,
    cancelled: Option<&'a std::sync::atomic::AtomicBool>,
    dynamic_philosophy: bool,
    page_rows: usize,
    page_bytes: usize,
}

// serde_json's RawValue owns a row before the row callback can inspect it.
// Bound bytes delivered to that allocation, including punctuation/whitespace.
// The underlying file reader has a fixed 64 KiB buffer.
struct CaptureReader<'a, R> {
    creation_read: Option<Rc<CreationReadState<'a, 'a>>>,
    inner: R,
    remaining: Rc<Cell<usize>>,
    deadline: Instant,
    cancelled: Option<&'a std::sync::atomic::AtomicBool>,
}
impl<R: Read> Read for CaptureReader<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self
            .cancelled
            .is_some_and(|value| value.load(std::sync::atomic::Ordering::Relaxed))
            || Instant::now() >= self.deadline
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "public D1 capture cancelled or expired",
            ));
        }
        if output.is_empty() {
            return Ok(0);
        }
        let left = self.remaining.get();
        if left == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "public D1 JSON value read bound",
            ));
        }
        let available = output.len().min(left);
        if let Some(state) = &self.creation_read {
            state
                .before_read(available)
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        let n = self.inner.read(&mut output[..available])?;
        self.remaining.set(left - n);
        Ok(n)
    }
}

fn json_field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value
        .as_object()?
        .iter()
        .find(|(key, _)| key.as_str() == Some(name))
        .map(|(_, value)| value)
}

#[track_caller]
fn creation_json<'a>(
    owner: &'a CreationState<'a>,
    raw: &[u8],
    cap: usize,
) -> Result<CreationJson<'a>> {
    creation_json_with_limits(
        owner,
        raw,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("runtime carrier creation JSON limits"))?,
    )
}
#[track_caller]
fn creation_json_with_limits<'a>(
    owner: &'a CreationState<'a>,
    raw: &[u8],
    requested: JsonLimits,
) -> Result<CreationJson<'a>> {
    let before = owner.json_visits.get();
    let allowance = owner
        .max_json_visits
        .checked_sub(before)
        .filter(|value| *value > 0)
        .ok_or(Error::Budget("runtime carrier creation JSON visits"))?
        .min(1_000_000)
        .min(requested.max_visits)
        .min(raw.len().saturating_add(1));
    let limits = JsonLimits::new(
        requested.max_bytes.min(raw.len().max(1)),
        requested.max_depth.min(96),
        allowance,
        requested.max_integer_digits.min(4096),
    )
    .map_err(|_| Error::Budget("runtime carrier creation JSON limits"))?;
    // The parser admits actual bytes and visits before traversing them under
    // this same owner. A maximum node allowance is a limit, not work already
    // performed. Errors retain every admitted prefix instead of refunding it.
    let available = owner.remaining(0)?;
    let mut admit = |bytes: usize, visits: usize| {
        let failure = || tos_foundation::FoundationError::new(
            tos_foundation::FoundationErrorCode::BudgetExceeded,
            "runtime carrier creation parse work/visits",
        );
        let total = owner.json_visits.get().checked_add(visits)
            .filter(|n| *n <= owner.max_json_visits).ok_or_else(failure)?;
        owner.charge_work(bytes.checked_add(visits).ok_or_else(failure)?)
            .map_err(|_| failure())?;
        owner.json_visits.set(total);
        Ok(())
    };
    let mut check = || {
        check_capture_active(Some(owner.cancelled), owner.deadline).map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "runtime carrier creation cutoff/cancellation",
            )
        })
    };
    let site = std::panic::Location::caller();
    let parsed = tos_foundation::parse_json_with_state_budget_and_admission(
        raw,
        JsonMode::PublishedStrict,
        limits,
        available,
        &mut check,
        &mut admit,
    )
    .map_err(|error| {
        eprintln!(
            "Native JSON parse refused at {}:{}: input_bytes={} available_state={} retained_state={}",
            site.file(), site.line(), raw.len(), available, owner.retained.get(),
        );
        foundation_json_error(error)
    })?;
    if owner.json_visits.get().checked_sub(before) != Some(parsed.visits()) {
        return Err(Error::Invalid("owned model parse visit accounting"));
    }
    let value = parsed.into_root();
    let bytes = value
        .retained_storage_bytes()
        .map_err(foundation_json_error)?;
    let hold = owner.hold(bytes)?;
    Ok(CreationJson {
        value,
        _hold: Some(hold),
    })
}

impl<'a> CaptureWriter<'a> {
    fn json_owned(&mut self, raw: &[u8], cap: usize) -> Result<CreationJson<'a>> {
        match self.creation {
            Some(owner) => creation_json(owner, raw, cap),
            None => Ok(CreationJson {
                value: json(raw, cap)?,
                _hold: None,
            }),
        }
    }
    fn compact_owned(
        &mut self,
        value: &JsonValue,
        cap: usize,
        source_bytes: usize,
    ) -> Result<CreationBytes<'a>> {
        match self.creation {
            Some(owner) => compact_with_owned_state(owner, value, cap, source_bytes),
            None => Ok(CreationBytes {
                bytes: compact(value, cap)?,
                _hold: None,
            }),
        }
    }
    fn page_step(&mut self, bytes: usize) -> Result<()> {
        check_capture_active(self.cancelled, self.deadline)?;
        self.page_rows = self
            .page_rows
            .checked_add(1)
            .ok_or(Error::Budget("public D1 capture page rows"))?;
        self.page_bytes = self
            .page_bytes
            .checked_add(bytes)
            .ok_or(Error::Budget("public D1 capture page bytes"))?;
        if self.page_rows >= 128 || self.page_bytes >= 4 * 1024 * 1024 {
            self.db.execute_batch("COMMIT; BEGIN IMMEDIATE")?;
            self.page_rows = 0;
            self.page_bytes = 0;
        }
        Ok(())
    }
    fn collection(&mut self, collection: &str, kind: &str) -> Result<()> {
        check_capture_active(self.cancelled, self.deadline)?;
        checked_add(self.work, collection.len(), self.limits.max_work_bytes)?;
        let source_navigation_object =
            self.role == CORPUS && collection == "source_navigation" && kind == "object";
        let dynamic = self.dynamic_philosophy
            && self.role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if (!selected_rows(self.role, collection) && !source_navigation_object && !dynamic)
            || (!matches!(kind, "array" | "mapping") && !source_navigation_object)
        {
            return Err(Error::Invalid("public D1 collection declaration"));
        }
        self.db.execute(
            "INSERT INTO capture_collections(role,collection,kind) VALUES (?1,?2,?3)",
            params![self.role, collection, kind],
        )?;
        Ok(())
    }
    fn row(
        &mut self,
        collection: &str,
        raw: &[u8],
        source_key: Option<&str>,
        order: &[String],
        allow_non_object: bool,
    ) -> Result<()> {
        check_capture_active(self.cancelled, self.deadline)?;
        let dynamic = self.dynamic_philosophy
            && self.role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if !selected_rows(self.role, collection) && !dynamic {
            return Err(Error::Invalid("public D1 collection outside fixed input"));
        }
        if raw.len() > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 row bytes"));
        }
        checked_add(self.work, raw.len(), self.limits.max_work_bytes)?;
        let value = self.json_owned(raw, MAX_ROW_BYTES)?;
        if !allow_non_object && value.as_object().is_none() {
            return Err(Error::Invalid("public D1 row object"));
        }
        let ordinal = self
            .ordinals
            .iter()
            .find(|(name, _)| name == collection)
            .map(|(_, value)| *value)
            .unwrap_or(0);
        if source_key.is_some_and(|key| key.is_empty() || key.len() > 4096) || order.len() > 2 {
            return Err(Error::Budget("public D1 capture row key/order"));
        }
        let _key_hold = self
            .creation
            .map(|state| state.hold(source_key.map(str::len).unwrap_or(20)))
            .transpose()?;
        let source_key = source_key
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{:020}", ordinal));
        if source_key.is_empty() || source_key.len() > 4096 || order.len() > 2 {
            return Err(Error::Budget("public D1 capture row key/order"));
        }
        let encoded = self.compact_owned(&value, MAX_ROW_BYTES, raw.len())?;
        let changed = self.db.execute(
            "INSERT INTO capture_rows(role,collection,source_key,ord,sort0,sort1,json,sha256) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![self.role, collection, source_key, ordinal as i64, order.first().map(String::as_str).unwrap_or(""), order.get(1).map(String::as_str).unwrap_or(""), &*encoded, Digest256::of_bytes(&encoded).as_bytes().as_slice()],
        )?;
        if changed != 1 {
            return Err(Error::Invalid("public D1 capture insertion"));
        }
        let next = ordinal
            .checked_add(1)
            .ok_or(Error::Budget("public D1 ordinal"))?;
        if let Some((_, value)) = self
            .ordinals
            .iter_mut()
            .find(|(name, _)| name == collection)
        {
            *value = next;
        } else {
            if self.creation.is_some() {
                // Vec geometric old/new slots, UTF8 copies were admitted
                // before the serde reader delivered their source characters.
                let upper = std::mem::size_of::<(String, u64)>()
                    .checked_mul(
                        self.ordinals
                            .len()
                            .checked_add(1)
                            .and_then(|n| n.max(4).checked_mul(4))
                            .ok_or(Error::Budget("runtime ordinal slots"))?,
                    )
                    .ok_or(Error::Budget("runtime ordinal slots"))?;
                let delta = upper
                    .checked_sub(self.ordinal_slots_admitted)
                    .ok_or(Error::Budget("runtime ordinal state"))?;
                if let Some(state) = &self.creation_read {
                    state.admit_tree(delta)?;
                } else if let Some(state) = self.creation {
                    state.retain(delta + collection.len())?;
                }
                self.ordinal_slots_admitted = upper;
            }
            self.ordinals.push((collection.to_owned(), next));
        }
        *self.row_count = self
            .row_count
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_rows)
            .ok_or(Error::Budget("public D1 capture rows"))?;
        checked_add(
            self.work,
            encoded.len() + source_key.len() + order.iter().map(String::len).sum::<usize>(),
            self.limits.max_work_bytes,
        )?;
        self.page_step(encoded.len())?;
        Ok(())
    }

    fn header(&mut self, path: &str, raw: &[u8]) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("public D1 capture deadline"));
        }
        if raw.len() > MAX_HEADER_BYTES {
            return Err(Error::Budget("public D1 header bytes"));
        }
        let value = self.json_owned(raw, MAX_HEADER_BYTES)?;
        let encoded = self.compact_owned(&value, MAX_HEADER_BYTES, raw.len())?;
        checked_add(
            self.work,
            raw.len() + encoded.len(),
            self.limits.max_work_bytes,
        )?;
        self.db.execute(
            "INSERT INTO capture_headers(role,path,json) VALUES (?1,?2,?3)",
            params![self.role, path, &*encoded],
        )?;
        self.page_step(raw.len())?;
        Ok(())
    }
}

struct RowsSeed<'a, 'b> {
    writer: &'a mut CaptureWriter<'b>,
    collection: String,
}

impl<'de> DeserializeSeed<'de> for RowsSeed<'_, '_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        struct RowsVisitor<'a, 'b> {
            writer: &'a mut CaptureWriter<'b>,
            collection: String,
        }
        impl<'de> Visitor<'de> for RowsVisitor<'_, '_> {
            type Value = ();
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a public projection row array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
                self.writer
                    .collection(&self.collection, "array")
                    .map_err(A::Error::custom)?;
                loop {
                    if let Some(state) = &self.writer.creation_read {
                        state.begin_value();
                    }
                    self.writer.read_budget.set(MAX_ROW_BYTES + 65536);
                    let Some(raw) = seq.next_element::<Box<RawValue>>()? else {
                        break;
                    };
                    let allow_non_object = self.writer.dynamic_philosophy;
                    self.writer
                        .row(
                            &self.collection,
                            raw.get().as_bytes(),
                            None,
                            &[],
                            allow_non_object,
                        )
                        .map_err(A::Error::custom)?;
                }
                Ok(())
            }
        }
        deserializer.deserialize_seq(RowsVisitor {
            writer: self.writer,
            collection: self.collection,
        })
    }
}

struct PhilosophyMemberSeed<'a, 'b> {
    writer: &'a mut CaptureWriter<'b>,
    collection: String,
}

struct HeaderCount<'a> {
    bytes: usize,
    owner: &'a CreationState<'a>,
}
impl Write for HeaderCount<'_> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.owner
            .remaining(0)
            .map_err(|_| io::Error::other("runtime header cutoff/cancellation"))?;
        let next = self
            .bytes
            .checked_add(input.len())
            .filter(|n| *n <= MAX_HEADER_BYTES)
            .ok_or_else(|| io::Error::other("runtime header encoded bound"))?;
        self.bytes = next;
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// The pinned arbitrary_precision deserializer delivers decimal/large-integer
// lexemes through its synthetic Number map. Match stock Value's NumberFromString
// conversion; ordinary object handling keeps its strict duplicate-key rule.
struct CheckedNumberLexemeSeed<'a> {
    creation_read: Option<Rc<CreationReadState<'a, 'a>>>,
}
impl<'de> DeserializeSeed<'de> for CheckedNumberLexemeSeed<'_> {
    type Value = serde_json::Number;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        struct NumberVisitor<'a> {
            state: Option<Rc<CreationReadState<'a, 'a>>>,
        }
        impl<'de> Visitor<'de> for NumberVisitor<'_> {
            type Value = serde_json::Number;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("string containing a number")
            }
            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<Self::Value, E> {
                if let Some(state) = &self.state {
                    state.admit_tree(value.len()).map_err(E::custom)?;
                }
                value.parse::<serde_json::Number>().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(NumberVisitor {
            state: self.creation_read,
        })
    }
}

struct CheckedValueSeed<'a> {
    creation_read: Option<Rc<CreationReadState<'a, 'a>>>,
}

impl<'de> DeserializeSeed<'de> for CheckedValueSeed<'_> {
    type Value = serde_json::Value;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        struct CheckedValueVisitor<'a> {
            creation_read: Option<Rc<CreationReadState<'a, 'a>>>,
        }

        impl<'de> Visitor<'de> for CheckedValueVisitor<'_> {
            type Value = serde_json::Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a strict JSON value without duplicate object keys")
            }

            fn visit_bool<E: serde::de::Error>(
                self,
                value: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Bool(value))
            }

            fn visit_i64<E: serde::de::Error>(
                self,
                value: i64,
            ) -> std::result::Result<Self::Value, E> {
                if let Some(state) = &self.creation_read {
                    state.admit_tree(32).map_err(E::custom)?;
                }
                Ok(serde_json::Value::Number(value.into()))
            }

            fn visit_u64<E: serde::de::Error>(
                self,
                value: u64,
            ) -> std::result::Result<Self::Value, E> {
                if let Some(state) = &self.creation_read {
                    state.admit_tree(32).map_err(E::custom)?;
                }
                Ok(serde_json::Value::Number(value.into()))
            }

            fn visit_f64<E: serde::de::Error>(
                self,
                value: f64,
            ) -> std::result::Result<Self::Value, E> {
                if let Some(state) = &self.creation_read {
                    state.admit_tree(32).map_err(E::custom)?;
                }
                serde_json::Number::from_f64(value)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| E::custom("non-finite philosophy projection number"))
            }

            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<Self::Value, E> {
                if let Some(state) = &self.creation_read {
                    state.admit_tree(value.len()).map_err(E::custom)?;
                }
                Ok(serde_json::Value::String(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(
                self,
                value: String,
            ) -> std::result::Result<Self::Value, E> {
                if let Some(state) = &self.creation_read {
                    state.admit_tree(value.capacity()).map_err(E::custom)?;
                }
                Ok(serde_json::Value::String(value))
            }

            fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Null)
            }

            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Null)
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                let mut admitted = 0usize;
                loop {
                    if let Some(state) = &self.creation_read {
                        let upper = crate::knowledge_normalization::serde_array_slots_upper(
                            values
                                .len()
                                .checked_add(1)
                                .ok_or_else(|| A::Error::custom("typed array slots overflow"))?,
                        )
                        .map_err(A::Error::custom)?;
                        state
                            .admit_tree(
                                upper.checked_sub(admitted).ok_or_else(|| {
                                    A::Error::custom("typed array state mismatch")
                                })?,
                            )
                            .map_err(A::Error::custom)?;
                        admitted = upper;
                    }
                    let Some(value) = seq.next_element_seed(CheckedValueSeed {
                        creation_read: self.creation_read.as_ref().map(Rc::clone),
                    })?
                    else {
                        break;
                    };
                    values.push(value);
                }
                Ok(serde_json::Value::Array(values))
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut fields = serde_json::Map::new();
                let mut admitted = 0usize;
                // The resulting Map already owns each decoded key. Checking it
                // before insertion avoids an additional duplicate-key set.
                while let Some(name) = map.next_key::<String>()? {
                    if let Some(state) = &self.creation_read {
                        state
                            .admit_tree(name.capacity())
                            .map_err(A::Error::custom)?;
                    }
                    if fields.is_empty() && name == "$serde_json::private::Number" {
                        return map
                            .next_value_seed(CheckedNumberLexemeSeed {
                                creation_read: self.creation_read.as_ref().map(Rc::clone),
                            })
                            .map(serde_json::Value::Number);
                    }
                    if fields.contains_key(&name) {
                        return Err(A::Error::custom(
                            "duplicate philosophy projection object key",
                        ));
                    }
                    if let Some(state) = &self.creation_read {
                        let upper = crate::knowledge_normalization::serde_object_slots_upper(
                            fields
                                .len()
                                .checked_add(1)
                                .ok_or_else(|| A::Error::custom("typed object slots overflow"))?,
                        )
                        .map_err(A::Error::custom)?;
                        state
                            .admit_tree(
                                upper.checked_sub(admitted).ok_or_else(|| {
                                    A::Error::custom("typed object state mismatch")
                                })?,
                            )
                            .map_err(A::Error::custom)?;
                        admitted = upper;
                    }
                    let value = map.next_value_seed(CheckedValueSeed {
                        creation_read: self.creation_read.as_ref().map(Rc::clone),
                    })?;
                    fields.insert(name, value);
                }
                Ok(serde_json::Value::Object(fields))
            }
        }

        deserializer.deserialize_any(CheckedValueVisitor {
            creation_read: self.creation_read,
        })
    }
}

impl<'de> DeserializeSeed<'de> for PhilosophyMemberSeed<'_, '_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        struct MemberVisitor<'a, 'b> {
            writer: &'a mut CaptureWriter<'b>,
            collection: String,
        }

        impl MemberVisitor<'_, '_> {
            fn header<E: serde::de::Error>(
                &mut self,
                value: serde_json::Value,
            ) -> std::result::Result<(), E> {
                if let Some(owner) = self.writer.creation {
                    // Count the unchanged serde representation without a
                    // growing byte buffer. Admit both passes on original work.
                    checked_add(
                        self.writer.work,
                        MAX_HEADER_BYTES
                            .checked_mul(2)
                            .ok_or_else(|| E::custom("runtime header work overflow"))?,
                        self.writer.limits.max_work_bytes,
                    )
                    .map_err(E::custom)?;
                    owner.remaining(0).map_err(E::custom)?;
                    let mut counter = HeaderCount { bytes: 0, owner };
                    serde_json::to_writer(&mut counter, &value).map_err(E::custom)?;
                    owner.remaining(0).map_err(E::custom)?;
                    let _raw_hold = owner.hold(counter.bytes).map_err(E::custom)?;
                    let mut raw = Vec::with_capacity(counter.bytes);
                    serde_json::to_writer(&mut raw, &value).map_err(E::custom)?;
                    if raw.len() != counter.bytes {
                        return Err(E::custom("runtime header count changed"));
                    }
                    owner.remaining(0).map_err(E::custom)?;
                    self.writer
                        .header(&self.collection, &raw)
                        .map_err(E::custom)
                } else {
                    let raw = serde_json::to_vec(&value).map_err(E::custom)?;
                    self.writer
                        .header(&self.collection, &raw)
                        .map_err(E::custom)
                }
            }
        }

        impl<'de> Visitor<'de> for MemberVisitor<'_, '_> {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a philosophy projection member")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
                self.writer
                    .collection(&self.collection, "array")
                    .map_err(A::Error::custom)?;
                loop {
                    if let Some(state) = &self.writer.creation_read {
                        state.begin_value();
                    }
                    self.writer.read_budget.set(MAX_ROW_BYTES + 65536);
                    let Some(raw) = seq.next_element::<Box<RawValue>>()? else {
                        break;
                    };
                    self.writer
                        .row(&self.collection, raw.get().as_bytes(), None, &[], true)
                        .map_err(A::Error::custom)?;
                }
                Ok(())
            }

            fn visit_map<A: MapAccess<'de>>(mut self, map: A) -> std::result::Result<(), A::Error> {
                let value = CheckedValueSeed {
                    creation_read: self.writer.creation_read.as_ref().map(Rc::clone),
                }
                .deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                self.header(value)
            }

            fn visit_bool<E: serde::de::Error>(
                mut self,
                value: bool,
            ) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Bool(value))
            }

            fn visit_i64<E: serde::de::Error>(mut self, value: i64) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Number(value.into()))
            }

            fn visit_u64<E: serde::de::Error>(mut self, value: u64) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Number(value.into()))
            }

            fn visit_f64<E: serde::de::Error>(mut self, value: f64) -> std::result::Result<(), E> {
                let number = serde_json::Number::from_f64(value)
                    .ok_or_else(|| E::custom("non-finite philosophy projection number"))?;
                self.header(serde_json::Value::Number(number))
            }

            fn visit_str<E: serde::de::Error>(mut self, value: &str) -> std::result::Result<(), E> {
                self.header(serde_json::Value::String(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(
                mut self,
                value: String,
            ) -> std::result::Result<(), E> {
                self.header(serde_json::Value::String(value))
            }

            fn visit_none<E: serde::de::Error>(mut self) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Null)
            }

            fn visit_unit<E: serde::de::Error>(mut self) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Null)
            }

            fn visit_some<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> std::result::Result<(), D::Error> {
                deserializer.deserialize_any(self)
            }

            fn visit_newtype_struct<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> std::result::Result<(), D::Error> {
                deserializer.deserialize_any(self)
            }
        }

        deserializer.deserialize_any(MemberVisitor {
            writer: self.writer,
            collection: self.collection,
        })
    }
}

struct ObjectSeed<'a, 'b> {
    writer: &'a mut CaptureWriter<'b>,
    prefix: &'static str,
}

impl<'de> DeserializeSeed<'de> for ObjectSeed<'_, '_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        struct ObjectVisitor<'a, 'b> {
            writer: &'a mut CaptureWriter<'b>,
            prefix: &'static str,
        }
        impl<'de> Visitor<'de> for ObjectVisitor<'_, '_> {
            type Value = ();
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a public projection object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
                if self.prefix == "source_navigation" && self.writer.role == CORPUS {
                    self.writer
                        .collection("source_navigation", "object")
                        .map_err(A::Error::custom)?;
                }
                let mut seen: Vec<String> = Vec::new();
                let mut seen_slots = 0usize;
                loop {
                    if let Some(state) = &self.writer.creation_read {
                        state.begin_value();
                    }
                    self.writer.read_budget.set(MAX_HEADER_BYTES + 65536);
                    let Some(key) = map.next_key::<String>()? else {
                        break;
                    };
                    if key.is_empty() || key.len() > 4096 {
                        return Err(A::Error::custom("public D1 member key bytes"));
                    }
                    checked_add(
                        self.writer.work,
                        key.len(),
                        self.writer.limits.max_work_bytes,
                    )
                    .map_err(A::Error::custom)?;
                    if self.writer.creation_read.is_some() {
                        checked_add(
                            self.writer.work,
                            key.len()
                                .checked_mul(seen.len())
                                .ok_or_else(|| A::Error::custom("member comparison work"))?,
                            self.writer.limits.max_work_bytes,
                        )
                        .map_err(A::Error::custom)?;
                    }
                    if seen.iter().any(|previous| previous == &key) {
                        return Err(A::Error::custom("duplicate public projection member"));
                    }
                    if let Some(state) = &self.writer.creation_read {
                        let upper = seen
                            .len()
                            .checked_add(1)
                            .and_then(|n| n.max(4).checked_mul(4))
                            .and_then(|n| n.checked_mul(std::mem::size_of::<String>()))
                            .ok_or_else(|| A::Error::custom("member key slots"))?;
                        state
                            .admit_tree(
                                upper
                                    .checked_sub(seen_slots)
                                    .ok_or_else(|| A::Error::custom("member key state"))?,
                            )
                            .map_err(A::Error::custom)?;
                        seen_slots = upper;
                    }
                    if let Some(state) = &self.writer.creation_read {
                        state
                            .admit_tree(
                                key.len()
                                    .checked_mul(3)
                                    .and_then(|n| n.checked_add(self.prefix.len() + 1))
                                    .ok_or_else(|| A::Error::custom("member retained key bytes"))?,
                            )
                            .map_err(A::Error::custom)?;
                    }
                    seen.push(key.clone());
                    let collection = if self.prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}/{}", self.prefix, key)
                    };
                    if let Some(state) = &self.writer.creation_read {
                        state.begin_value();
                    }
                    if self.writer.role.starts_with("evidence-")
                        && !selected_rows(self.writer.role, &collection)
                        && collection != "schema_version"
                    {
                        self.writer.read_budget.set(self.writer.limits.max_input_bytes.min(usize::MAX as u64) as usize);
                        map.next_value::<serde::de::IgnoredAny>()?;
                        self.writer.read_budget.set(MAX_HEADER_BYTES + 65536);
                    } else if self.prefix.is_empty()
                        && key == "source_navigation"
                        && self.writer.role == CORPUS
                    {
                        map.next_value_seed(ObjectSeed {
                            writer: self.writer,
                            prefix: "source_navigation",
                        })?;
                    } else if collection != "input_digests"
                        && selected_rows(self.writer.role, &collection)
                    {
                        map.next_value_seed(RowsSeed {
                            writer: self.writer,
                            collection,
                        })?;
                    } else if self.writer.dynamic_philosophy && self.prefix.is_empty() {
                        map.next_value_seed(PhilosophyMemberSeed {
                            writer: self.writer,
                            collection,
                        })?;
                    } else {
                        let raw: Box<RawValue> = map.next_value()?;
                        self.writer
                            .header(&collection, raw.get().as_bytes())
                            .map_err(A::Error::custom)?;
                    }
                }
                Ok(())
            }
        }
        deserializer.deserialize_map(ObjectVisitor {
            writer: self.writer,
            prefix: self.prefix,
        })
    }
}

pub(crate) fn source_digest(
    file: &mut File,
    cap: u64,
    mut charge: impl FnMut(usize) -> Result<()>,
) -> Result<(Digest256, u64)> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        total = total
            .checked_add(size as u64)
            .filter(|n| *n <= cap)
            .ok_or(Error::Budget("public D1 source bytes"))?;
        charge(size)?;
        hash.update(&buffer[..size]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok((hash.finalize(), total))
}

struct CreationSerde<'a> {
    value: serde_json::Value,
    _hold: Option<CreationStateHold<'a, 'a>>,
}
impl std::ops::Deref for CreationSerde<'_> {
    type Target = serde_json::Value;
    fn deref(&self) -> &serde_json::Value {
        &self.value
    }
}
fn strict_value_owned<'a>(
    raw: &[u8],
    cap: usize,
    creation: Option<&'a CreationState<'a>>,
) -> Result<CreationSerde<'a>> {
    let Some(owner) = creation else {
        return Ok(CreationSerde {
            value: strict_value(raw, cap)?,
            _hold: None,
        });
    };
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("runtime carrier strict value limits"))?;
    let (value, hold) = owner.serde_scoped_with_limits(raw, limits)?;
    Ok(CreationSerde { value, _hold: Some(hold) })

}

fn strict_value(raw: &[u8], cap: usize) -> Result<serde_json::Value> {
    json(raw, cap)?;
    serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))
}

fn field<'a>(value: &'a serde_json::Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("public projection descriptor field"))
}

fn number(value: &serde_json::Value, name: &str) -> Result<u64> {
    value
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .ok_or(Error::Invalid("public projection descriptor count"))
}

fn policy(
    role: &str,
    collection: &str,
) -> Option<(&'static [&'static str], &'static [&'static str], bool)> {
    let role = match role {
        "evidence-corpus" => CORPUS,
        "evidence-philosophy" => PHILOSOPHY,
        other => other,
    };
    let entry = match (role, collection) {
        (CORPUS, "diagnostics") => (&[][..], &[][..], false),
        (CORPUS, "nodes") => (&["node_id"][..], &["source_path"][..], false),
        (CORPUS, "resources") | (CORPUS, "manifests") => (&["path"][..], &["path"][..], false),
        (CORPUS, "relation_packs") => (&["pack_id"][..], &["path"][..], false),
        (CORPUS, "relation_edges") => (
            &["pack_id", "edge_id"][..],
            &["pack_id", "edge_id"][..],
            false,
        ),
        (CORPUS, "source_navigation/nodes") => (&["node_id"][..], &["node_id"][..], false),
        (CORPUS, "source_navigation/edges") => (&["edge_id"][..], &["edge_id"][..], false),
        (CORPUS, "source_navigation/rights") => (&["rights_id"][..], &["rights_id"][..], false),
        (PHILOSOPHY, "nodes") => (&["node_id"][..], &["node_id"][..], false),
        (PHILOSOPHY, "edges") => (&["edge_id"][..], &["edge_id"][..], false),
        (PHILOSOPHY, "clusters") => (&["cluster_id"][..], &["cluster_id"][..], false),
        (PHILOSOPHY, "views") => (&["view_id"][..], &["order", "view_id"][..], false),
        (CLAIMS, "nodes") => (&["node_id"][..], &["node_id"][..], false),
        (CLAIMS, "edges") => (&["edge_id"][..], &["edge_id"][..], false),
        (CLAIMS, "claim_traces") => (&["claim_ref"][..], &["claim_ref"][..], false),
        (CLAIMS, "input_digests") => (&[][..], &[][..], true),
        _ => return None,
    };
    Some(entry)
}

fn order_value(value: Option<&JsonValue>) -> Result<String> {
    match value {
        None => Ok("1:".to_owned()),
        Some(JsonValue::Null) => Ok("1:None".to_owned()),
        Some(JsonValue::Bool(value)) => Ok(format!("1:{value}")),
        Some(JsonValue::String(value)) => Ok(format!(
            "1:{}",
            value
                .as_str()
                .ok_or(Error::Invalid("public projection order string"))?
        )),
        Some(JsonValue::Number(number)) if number.lexeme.chars().all(|c| c.is_ascii_digit()) => {
            let decimal = number.lexeme.as_str();
            Ok(format!("0:{:020}:{decimal}", decimal.len()))
        }
        Some(JsonValue::Number(number)) if number.lexeme == "-0" => {
            Ok("0:00000000000000000001:0".to_owned())
        }
        Some(JsonValue::Number(number))
            if number.lexeme.starts_with('-')
                && number.lexeme[1..].bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            Ok(format!("1:{}", number.lexeme))
        }
        _ => Err(Error::Invalid("public projection order value")),
    }
}

fn partition_order(row: &JsonValue, fields: &[String], max_bytes: u64) -> Result<Vec<String>> {
    let mut result = Vec::with_capacity(fields.len());
    let mut total = 0u64;
    for field in fields {
        let order = order_value(json_field(row, field.as_str()))?;
        total = total
            .checked_add(order.len() as u64)
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or(Error::Budget("public projection order bytes"))?;
        result.push(order);
    }
    Ok(result)
}

fn partition_record_key(value: &JsonValue, fields: &[String]) -> Result<String> {
    if fields.len() == 1 {
        return json_field(value, fields[0].as_str())
            .and_then(JsonValue::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .map(str::to_owned)
            .ok_or(Error::Invalid("public projection record identity"));
    }
    let mut keys = Vec::with_capacity(fields.len());
    let mut encoded_len = 2usize;
    for field in fields {
        let key = json_field(value, field.as_str())
            .and_then(JsonValue::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .ok_or(Error::Invalid("public projection record identity"))?;
        if keys.len() != 0 {
            encoded_len = encoded_len
                .checked_add(1)
                .ok_or(Error::Budget("public projection record identity bytes"))?;
        }
        encoded_len = encoded_len
            .checked_add(2)
            .filter(|bytes| *bytes <= 4096)
            .ok_or(Error::Budget("public projection record identity bytes"))?;
        for byte in key.as_bytes() {
            let escaped = match byte {
                b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
                0x00..=0x1f => 6,
                _ => 1,
            };
            encoded_len = encoded_len
                .checked_add(escaped)
                .filter(|bytes| *bytes <= 4096)
                .ok_or(Error::Budget("public projection record identity bytes"))?;
        }
        keys.push(key);
    }
    if keys.len() == 1 {
        return Ok(keys[0].to_owned());
    }
    serde_json::to_string(&keys).map_err(|e| Error::Source(e.to_string()))
}

// Forecast only the key/order policy outputs and transient expected-policy
// serde values. The descriptor/root tree is borrowed, never cloned here.
fn partition_policy_state_upper(
    role: &str,
    collection: &str,
    spec: &serde_json::Value,
    dynamic: bool,
) -> Result<usize> {
    let mut fields = 0usize;
    let mut bytes = 0usize;
    let mut add = |field: &str| -> Result<()> {
        fields = fields
            .checked_add(1)
            .ok_or(Error::Budget("partition policy slots"))?;
        bytes = bytes
            .checked_add(field.len())
            .ok_or(Error::Budget("partition policy strings"))?;
        Ok(())
    };
    if dynamic {
        for name in ["key_field", "order_fields"] {
            match spec.get(name) {
                Some(serde_json::Value::String(value)) => add(value)?,
                Some(serde_json::Value::Array(values)) => {
                    for value in values {
                        add(value
                            .as_str()
                            .ok_or(Error::Invalid("partition policy field"))?)?;
                    }
                }
                _ => (),
            }
        }
    } else {
        let (keys, order, _) = policy(role, collection).ok_or(Error::Invalid(
            "public projection collection outside owner policy",
        ))?;
        for field in keys.iter().chain(order.iter()) {
            add(field)?;
        }
    }
    // key/order/effective-order vectors plus expected serde arrays. Each
    // independently geometric owner admits its minimum nonzero capacity.
    let slots = fields
        .max(4)
        .checked_mul(4)
        .and_then(|n| {
            n.checked_mul(
                3 * std::mem::size_of::<String>() + 2 * std::mem::size_of::<serde_json::Value>(),
            )
        })
        .ok_or(Error::Budget("partition policy containers"))?;
    slots
        .checked_add(
            bytes
                .checked_mul(5)
                .ok_or(Error::Budget("partition policy clones"))?,
        )
        .ok_or(Error::Budget("partition policy state"))
}

fn partition_collection_policy(
    role: &str,
    collection: &str,
    spec: &serde_json::Value,
    dynamic_philosophy: bool,
) -> Result<(Vec<String>, Vec<String>, bool)> {
    let object = spec
        .as_object()
        .ok_or(Error::Invalid("public projection collection spec"))?;
    if object.len() != 3
        || !["key_field", "order_fields", "root"]
            .iter()
            .all(|field| object.contains_key(*field))
    {
        return Err(Error::Invalid("public projection collection spec"));
    }
    if dynamic_philosophy {
        let (key_fields, mapping) = match spec.get("key_field") {
            Some(serde_json::Value::Null) => (Vec::new(), true),
            Some(serde_json::Value::String(field)) if !field.is_empty() => {
                (vec![field.clone()], false)
            }
            Some(serde_json::Value::Array(fields)) => {
                let fields = fields
                    .iter()
                    .map(|field| {
                        field
                            .as_str()
                            .filter(|field| !field.is_empty())
                            .map(str::to_owned)
                            .ok_or(Error::Invalid("public projection key field"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                (fields, false)
            }
            _ => return Err(Error::Invalid("public projection key field")),
        };
        let declared_order = spec
            .get("order_fields")
            .and_then(serde_json::Value::as_array)
            .ok_or(Error::Invalid("public projection ordering fields"))?
            .iter()
            .map(|field| {
                field
                    .as_str()
                    .filter(|field| !field.is_empty())
                    .map(str::to_owned)
                    .ok_or(Error::Invalid("public projection ordering field"))
            })
            .collect::<Result<Vec<_>>>()?;
        if key_fields.is_empty() && !mapping && !declared_order.is_empty() {
            return Err(Error::Invalid("public projection positional ordering"));
        }
        let effective_order = if mapping || key_fields.is_empty() {
            Vec::new()
        } else if declared_order.is_empty() {
            key_fields.clone()
        } else {
            declared_order
        };
        return Ok((key_fields, effective_order, mapping));
    }

    let (key_fields, order_fields, mapping) = policy(role, collection).ok_or(Error::Invalid(
        "public projection collection outside owner policy",
    ))?;
    let expected_key = if mapping {
        serde_json::Value::Null
    } else if key_fields.is_empty() {
        serde_json::json!([])
    } else if key_fields.len() == 1 {
        serde_json::json!(key_fields[0])
    } else {
        serde_json::json!(key_fields)
    };
    let expected_order = serde_json::json!(order_fields);
    if spec.get("key_field") != Some(&expected_key)
        || spec.get("order_fields") != Some(&expected_order)
    {
        return Err(Error::Invalid("public projection owner ordering policy"));
    }
    Ok((
        key_fields.iter().map(|field| (*field).to_owned()).collect(),
        order_fields
            .iter()
            .map(|field| (*field).to_owned())
            .collect(),
        mapping,
    ))
}

fn encode_order_tuple(order: &[String], max_bytes: usize) -> Result<String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let capacity = order.iter().try_fold(0usize, |total, field| {
        field
            .len()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(1))
            .and_then(|bytes| total.checked_add(bytes))
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or(Error::Budget("public projection order bytes"))
    })?;
    let mut encoded = String::with_capacity(capacity);
    for field in order {
        for byte in field.as_bytes() {
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
        // Hex contains no exclamation mark, and the terminator sorts before
        // another encoded byte. This preserves lexicographic tuple order.
        encoded.push('!');
    }
    Ok(encoded)
}

fn capture_part(
    root_path: &Path,
    descriptor: &serde_json::Value,
    prefix: &str,
    writer: &mut CaptureWriter<'_>,
    collection: &str,
    key_fields: &[String],
    order_fields: &[String],
    mapping: bool,
    collection_count: u64,
) -> Result<u64> {
    let descriptor_fields = [
        "kind",
        "prefix",
        "path",
        "sha256",
        "size_bytes",
        "decoded_bytes",
        "decoded_sha256",
        "count",
    ];
    let members = descriptor
        .as_object()
        .ok_or(Error::Invalid("public projection descriptor"))?;
    if members.len() != descriptor_fields.len()
        || !descriptor_fields
            .iter()
            .all(|field| members.contains_key(*field))
    {
        return Err(Error::Invalid("public projection descriptor shape"));
    }
    let kind = field(descriptor, "kind")?;
    if kind != "index" && kind != "data" {
        return Err(Error::Invalid("public projection part kind"));
    }
    if field(descriptor, "prefix")? != prefix
        || prefix.len() > 64
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Invalid("public projection part prefix"));
    }
    let stored_sha = field(descriptor, "sha256")?;
    let decoded_sha = field(descriptor, "decoded_sha256")?;
    Digest256::from_hex(stored_sha).map_err(|_| Error::Invalid("public projection part digest"))?;
    Digest256::from_hex(decoded_sha)
        .map_err(|_| Error::Invalid("public projection decoded digest"))?;
    let stored_len = usize::try_from(number(descriptor, "size_bytes")?)
        .map_err(|_| Error::Budget("public projection stored bytes"))?;
    let decoded_len = usize::try_from(number(descriptor, "decoded_bytes")?)
        .map_err(|_| Error::Budget("public projection decoded bytes"))?;
    let cap = if kind == "index" {
        MAX_INDEX_BYTES
    } else {
        MAX_PART_BYTES
    };
    if stored_len > cap + STORED_OVERHEAD || decoded_len > cap {
        return Err(Error::Budget("public projection part bytes"));
    }
    let stem = root_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or(Error::Invalid("public projection stem"))?;
    let suffix = if kind == "index" {
        ".index.json"
    } else {
        ".jsonl.gz"
    };
    let _namespace_hold = writer
        .creation
        .map(|state| {
            let relative = stem
                .len()
                .checked_add(6 + 2 + 1 + 64 + suffix.len())
                .ok_or(Error::Budget("runtime part namespace width"))?;
            let path = root_path
                .as_os_str()
                .as_encoded_bytes()
                .len()
                .checked_add(relative)
                .ok_or(Error::Budget("runtime part path width"))?;
            state.hold(
                path.max(8)
                    .checked_mul(4)
                    .and_then(|n| {
                        relative
                            .max(8)
                            .checked_mul(4)
                            .and_then(|bytes| n.checked_add(bytes))
                            .and_then(|n| n.checked_add(2 * 64 + 64))
                    })
                    .ok_or(Error::Budget("runtime part namespace state"))?,
            )
        })
        .transpose()?;
    let relative = format!("{stem}.parts/{}/{}{}", &stored_sha[..2], stored_sha, suffix);
    if field(descriptor, "path")? != relative {
        return Err(Error::Invalid("public projection part namespace"));
    }
    let path = root_path
        .parent()
        .ok_or(Error::Invalid("public projection parent"))?
        .join(relative);
    let mut file = safe_open::open_regular(&path, stored_len as u64)?;
    let (actual_sha, actual_len) = source_digest(&mut file, stored_len as u64, |n| {
        if Instant::now() >= writer.deadline {
            return Err(Error::Budget("public D1 capture deadline"));
        }
        checked_add(writer.work, n, writer.limits.max_work_bytes)
    })?;
    if actual_len != stored_len as u64 || actual_sha.to_hex() != stored_sha {
        return Err(Error::Invalid("public projection part changed"));
    }
    let _part_bytes_hold = writer
        .creation
        .map(|state| {
            // Frozen stored bytes, output Vec geometric/sentinel overlap and
            // the existing decoder owner's pinned native backend workspace.
            let decoded_peak = decoded_len
                .checked_add(1)
                .and_then(|n| n.max(32).checked_mul(3))
                .ok_or(Error::Budget("runtime decoded part state"))?;
            state.hold(
                stored_len
                    .checked_add(decoded_peak)
                    .and_then(|n| {
                        n.checked_add(crate::legacy::partition_decoder_workspace_upper(kind).ok()?)
                    })
                    .ok_or(Error::Budget("runtime part byte state"))?,
            )
        })
        .transpose()?;
    let mut stored = vec![0u8; stored_len];
    file.read_exact(&mut stored)?;
    let mut eof = [0u8; 1];
    if file.read(&mut eof)? != 0 || stored.len() != stored_len {
        return Err(Error::Invalid("public projection part length changed"));
    }
    checked_add(writer.work, decoded_len, writer.limits.max_work_bytes)?;
    let decoded = decode_partition_part(
        &stored,
        kind,
        stored_len,
        decoded_len,
        stored_sha,
        decoded_sha,
    )?;
    writer.db.execute(
        "INSERT OR IGNORE INTO capture_sources(path,sha256,size_bytes) VALUES (?1,?2,?3)",
        params![
            path.to_string_lossy().as_ref(),
            actual_sha.as_bytes().as_slice(),
            actual_len as i64
        ],
    )?;
    let expected = number(descriptor, "count")?;
    if kind == "index" {
        let index = strict_value_owned(&decoded, MAX_INDEX_BYTES, writer.creation)?;
        let index_members = index
            .as_object()
            .ok_or(Error::Invalid("public projection index object"))?;
        if index_members.len() != 4
            || !["schema_version", "prefix", "count", "children"]
                .iter()
                .all(|field| index_members.contains_key(*field))
        {
            return Err(Error::Invalid("public projection index shape"));
        }
        if field(&index, "schema_version")? != "tos_projection_partition_index_v1"
            || field(&index, "prefix")? != prefix
            || number(&index, "count")? != expected
        {
            return Err(Error::Invalid("public projection partition index"));
        }
        let children = index
            .get("children")
            .and_then(serde_json::Value::as_object)
            .filter(|children| !children.is_empty() && prefix.len() < 64)
            .ok_or(Error::Invalid("public projection partition children"))?;
        let mut total = 0u64;
        for (digit, child) in children {
            if digit.len() != 1
                || !digit
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(Error::Invalid("public projection partition digit"));
            }
            total = total
                .checked_add(capture_part(
                    root_path,
                    child,
                    &format!("{prefix}{digit}"),
                    writer,
                    collection,
                    key_fields,
                    order_fields,
                    mapping,
                    collection_count,
                )?)
                .ok_or(Error::Budget("public projection partition count"))?;
        }
        if total != expected {
            return Err(Error::Invalid("public projection partition count"));
        }
        return Ok(total);
    }
    if !decoded.is_empty() && decoded.last() != Some(&b'\n') {
        return Err(Error::Invalid("public projection line feed"));
    }
    let body = if decoded.is_empty() {
        &decoded[..]
    } else {
        &decoded[..decoded.len() - 1]
    };
    let mut count = 0u64;
    // The previous key survives the line's parse/order/output temporaries.
    // Keep its bounded owner charged through replacement and final drop.
    let _previous_hold = writer.creation.map(|state| state.hold(4096)).transpose()?;
    let mut previous: Option<String> = None;
    for line in body.split(|b| *b == b'\n').filter(|_| !decoded.is_empty()) {
        if line.is_empty() {
            return Err(Error::Invalid("public projection empty row"));
        }
        let record = writer.json_owned(line, MAX_ROW_BYTES)?;
        let object = record
            .as_object()
            .ok_or(Error::Invalid("public projection row object"))?;
        if object.len() != 2 {
            return Err(Error::Invalid("public projection row shape"));
        }
        let key = json_field(&record, "key")
            .and_then(JsonValue::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .ok_or(Error::Invalid("public projection row key"))?;
        if previous.as_deref().is_some_and(|prior| key <= prior)
            || !Digest256::of_bytes(key.as_bytes())
                .to_hex()
                .starts_with(prefix)
        {
            return Err(Error::Invalid("public projection row order/placement"));
        }
        let value =
            json_field(&record, "value").ok_or(Error::Invalid("public projection row value"))?;
        let _order_hold = writer
            .creation
            .map(|state| {
                let mut string_upper = 0usize;
                for field in order_fields {
                    let width = match json_field(value, field) {
                        Some(JsonValue::String(text)) => text
                            .as_str()
                            .ok_or(Error::Invalid("public projection order string"))?
                            .len(),
                        Some(JsonValue::Number(number)) => number.lexeme.len(),
                        _ => 5,
                    };
                    // Existing formatting emits <=source width+24, including
                    // integer width prefix; geometric old/new String storage.
                    string_upper = string_upper
                        .checked_add(
                            width
                                .checked_add(24)
                                .and_then(|n| n.max(8).checked_mul(4))
                                .ok_or(Error::Budget("runtime order string state"))?,
                        )
                        .ok_or(Error::Budget("runtime order state"))?;
                }
                let slots = order_fields
                    .len()
                    .max(1)
                    .checked_mul(4 * std::mem::size_of::<String>())
                    .and_then(|n| n.checked_add(key_fields.len() * std::mem::size_of::<&str>()))
                    .ok_or(Error::Budget("runtime order vector state"))?;
                state.hold(
                    string_upper
                        .checked_mul(3)
                        .and_then(|n| n.checked_add(slots))
                        .and_then(|n| n.checked_add(2 * 4096 + 64))
                        .ok_or(Error::Budget("runtime order simultaneous state"))?,
                )
            })
            .transpose()?;
        previous = Some(key.to_owned());
        let normalized: &JsonValue = if mapping {
            &record
        } else {
            if key_fields.is_empty() {
                if key.len() != 20
                    || !key.bytes().all(|b| b.is_ascii_digit())
                    || key
                        .parse::<u64>()
                        .map_err(|_| Error::Invalid("public projection position"))?
                        >= collection_count
                {
                    return Err(Error::Invalid("public projection position"));
                }
            } else if partition_record_key(value, key_fields)? != key {
                return Err(Error::Invalid("public projection row identity"));
            }
            value
        };
        let encoded = writer.compact_owned(normalized, MAX_ROW_BYTES, line.len())?;
        let order = if key_fields.is_empty() {
            vec![key.to_owned()]
        } else {
            partition_order(
                value,
                order_fields,
                writer
                    .limits
                    .max_work_bytes
                    .saturating_sub(writer.work.load(std::sync::atomic::Ordering::Acquire)),
            )?
        };
        let order = if writer.dynamic_philosophy {
            if mapping || key_fields.is_empty() {
                vec![key.to_owned()]
            } else {
                vec![
                    encode_order_tuple(
                        &order,
                        usize::try_from(writer.limits.max_work_bytes.saturating_sub(
                            writer.work.load(std::sync::atomic::Ordering::Acquire),
                        ))
                        .unwrap_or(usize::MAX),
                    )?,
                    Digest256::of_bytes(key.as_bytes()).to_hex(),
                ]
            }
        } else {
            order
        };
        let allow_non_object = writer.dynamic_philosophy && key_fields.is_empty() && !mapping;
        writer.row(collection, &encoded, Some(key), &order, allow_non_object)?;
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("public projection row count"))?;
    }
    if count != expected {
        return Err(Error::Invalid("public projection leaf count"));
    }
    Ok(count)
}

fn capture_partitioned(root_path: &Path, raw: &[u8], writer: &mut CaptureWriter<'_>) -> Result<()> {
    let root = strict_value_owned(raw, MAX_ROOT_BYTES as usize, writer.creation)?;
    let object = root
        .as_object()
        .ok_or(Error::Invalid("public projection manifest object"))?;
    if object.len() != 5
        || ![
            "schema_version",
            "logical_schema",
            "header",
            "limits",
            "collections",
        ]
        .iter()
        .all(|key| object.contains_key(*key))
        || field(&root, "schema_version")? != "tos_partitioned_projection_v1"
    {
        return Err(Error::Invalid("public projection manifest shape"));
    }
    let schema = field(&root, "logical_schema")?;
    let expected_schema = match writer.role {
        CORPUS | "evidence-corpus" => "tos_corpus_index_v1",
        PHILOSOPHY | "evidence-philosophy" => {
            if writer.dynamic_philosophy
                && matches!(
                    schema,
                    "tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2"
                )
            {
                schema
            } else {
                "tos_philosophy_graph_projection_v2"
            }
        }
        CLAIMS => "tos_source_witness_bibliographic_graph_v1",
        _ => return Err(Error::Invalid("public projection role")),
    };
    if schema != expected_schema || root["header"]["schema_version"].as_str() != Some(schema) {
        return Err(Error::Invalid("public projection logical schema"));
    }
    let limits = root
        .get("limits")
        .and_then(serde_json::Value::as_object)
        .ok_or(Error::Invalid("public projection limits"))?;
    if limits.len() != 4
        || limits.get("root_bytes").and_then(serde_json::Value::as_u64) != Some(MAX_ROOT_BYTES)
        || limits
            .get("index_bytes")
            .and_then(serde_json::Value::as_u64)
            != Some(MAX_INDEX_BYTES as u64)
        || limits.get("part_bytes").and_then(serde_json::Value::as_u64)
            != Some(MAX_PART_BYTES as u64)
        || limits.get("key_bytes").and_then(serde_json::Value::as_u64) != Some(4096)
    {
        return Err(Error::Invalid("public projection limit profile"));
    }
    let exact = writer.json_owned(raw, MAX_ROOT_BYTES as usize)?;
    let header = json_field(&exact, "header")
        .and_then(JsonValue::as_object)
        .ok_or(Error::Invalid("public projection header"))?;
    for (key, value) in header {
        let name = key
            .as_str()
            .ok_or(Error::Invalid("public projection header key"))?;
        if writer.role.starts_with("evidence-") && name != "schema_version" {
            continue;
        }
        if name == "source_navigation" && writer.role == CORPUS {
            let navigation = value
                .as_object()
                .ok_or(Error::Invalid("public projection navigation header"))?;
            for (nested, nested_value) in navigation {
                let nested = nested
                    .as_str()
                    .ok_or(Error::Invalid("public projection navigation key"))?;
                let encoded = writer.compact_owned(nested_value, MAX_HEADER_BYTES, raw.len())?;
                writer.header(&format!("source_navigation/{nested}"), &encoded)?;
            }
        } else {
            let encoded = writer.compact_owned(value, MAX_HEADER_BYTES, raw.len())?;
            writer.header(name, &encoded)?;
        }
    }
    let collections = root
        .get("collections")
        .and_then(serde_json::Value::as_object)
        .filter(|collections| !collections.is_empty())
        .ok_or(Error::Invalid("public projection collections"))?;
    let required: &[&str] = if writer.dynamic_philosophy {
        &[]
    } else {
        match writer.role {
            "evidence-corpus" => &["nodes", "relation_edges"],
            "evidence-philosophy" => &["views"],
            CORPUS => &[
                "nodes",
                "resources",
                "manifests",
                "relation_packs",
                "relation_edges",
                "source_navigation/nodes",
                "source_navigation/edges",
                "source_navigation/rights",
            ],
            PHILOSOPHY => &["nodes", "edges", "clusters", "views"],
            CLAIMS => &["nodes", "edges", "claim_traces", "input_digests"],
            _ => return Err(Error::Invalid("public projection role")),
        }
    };
    if !required.iter().all(|name| collections.contains_key(*name)) {
        return Err(Error::Invalid(
            "public projection missing maintained collection",
        ));
    }
    for (name, spec) in collections {
        if writer.dynamic_philosophy && !valid_top_level_collection(name) {
            return Err(Error::Invalid("partitioned philosophy collection name"));
        }
        if writer.role.starts_with("evidence-") && !selected_rows(writer.role, name) {
            continue;
        }
        let _policy_hold = writer
            .creation
            .map(|state| {
                state.hold(partition_policy_state_upper(
                    writer.role,
                    name,
                    spec,
                    writer.dynamic_philosophy,
                )?)
            })
            .transpose()?;
        let (key_fields, order_fields, mapping) =
            partition_collection_policy(writer.role, name, spec, writer.dynamic_philosophy)?;
        let header_collision: Option<i64> = writer
            .db
            .query_row(
                "SELECT 1 FROM capture_headers WHERE role=?1 AND path=?2",
                params![writer.role, name],
                |row| row.get(0),
            )
            .optional()?;
        if header_collision.is_some() {
            return Err(Error::Invalid("partitioned collection overlaps header"));
        }
        writer.collection(name, if mapping { "mapping" } else { "array" })?;
        let descriptor = spec
            .get("root")
            .ok_or(Error::Invalid("public projection collection root"))?;
        let count = number(descriptor, "count")?;
        let found = capture_part(
            root_path,
            descriptor,
            "",
            writer,
            name,
            &key_fields,
            &order_fields,
            mapping,
            count,
        )?;
        if found != count {
            return Err(Error::Invalid("public projection collection count"));
        }
    }
    Ok(())
}

/// Compiled runtime companions owned by capture; census uses this same selection.
pub(crate) fn runtime_companions() -> impl Iterator<Item = (&'static str, &'static [u8])> {
    const INPUTS: &[(&str, &[u8])] = &[
        (
            "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
            include_bytes!(
                "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
            ),
        ),
        (
            "access/contracts/knowledge-api.v1.json",
            include_bytes!("../../../../access/contracts/knowledge-api.v1.json"),
        ),
        (
            "access/contracts/knowledge-graph.v1.schema.json",
            include_bytes!("../../../../access/contracts/knowledge-graph.v1.schema.json"),
        ),
        (
            "access/contracts/knowledge-search-indexed.v2.schema.json",
            include_bytes!("../../../../access/contracts/knowledge-search-indexed.v2.schema.json"),
        ),
        (
            "access/contracts/readable-context.v1.schema.json",
            include_bytes!("../../../../access/contracts/readable-context.v1.schema.json"),
        ),
        (
            "access/contracts/lens-spec.v1.schema.json",
            include_bytes!("../../../../access/contracts/lens-spec.v1.schema.json"),
        ),
        (
            "access/contracts/lens-result.v1.schema.json",
            include_bytes!("../../../../access/contracts/lens-result.v1.schema.json"),
        ),
        (
            "access/contracts/temporal-comparison-request.v1.schema.json",
            include_bytes!(
                "../../../../access/contracts/temporal-comparison-request.v1.schema.json"
            ),
        ),
        (
            "access/contracts/temporal-comparison-result.v1.schema.json",
            include_bytes!(
                "../../../../access/contracts/temporal-comparison-result.v1.schema.json"
            ),
        ),
        (
            "access/contracts/source-read.v1.schema.json",
            include_bytes!("../../../../access/contracts/source-read.v1.schema.json"),
        ),
        (
            "access/contracts/exploration-request.v1.schema.json",
            include_bytes!("../../../../access/contracts/exploration-request.v1.schema.json"),
        ),
        (
            "access/contracts/exploration-result.v1.schema.json",
            include_bytes!("../../../../access/contracts/exploration-result.v1.schema.json"),
        ),
        (
            "access/contracts/exploration-request.v2.schema.json",
            include_bytes!("../../../../access/contracts/exploration-request.v2.schema.json"),
        ),
        (
            "access/contracts/exploration-result.v2.schema.json",
            include_bytes!("../../../../access/contracts/exploration-result.v2.schema.json"),
        ),
        (
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            include_bytes!("../../../../ToS/contracts/semantic-entity-type-registry.schema.json"),
        ),
        (
            "ToS/contracts/semantic-relation-type-registry.schema.json",
            include_bytes!("../../../../ToS/contracts/semantic-relation-type-registry.schema.json"),
        ),
    ];
    INPUTS.iter().copied()
}

impl PublicCapture {
    /// Logical owned residency: inline owner, actual container capacities and
    /// owned path/string buffers. Shared cancellation belongs to the caller;
    /// borrowed views do not charge this capture again. Allocator bookkeeping
    /// and RSS remain covered by the original hard resource owner.
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        let mut bytes = std::mem::size_of::<Self>();
        if self.controlled_identity.is_some() {
            // Arc<()> has two atomic count words and no payload allocation.
            bytes = bytes
                .checked_add(2 * std::mem::size_of::<usize>())
                .ok_or(Error::Budget("controlled capture identity residency"))?;
        }
        if self.sqlite_heap.is_some() {
            bytes = bytes
                .checked_add(controlled_fence_workspace_upper(
                    self.runtime_capture_role.is_none(),
                ))
                .ok_or(Error::Budget("controlled capture fence state"))?;
        }
        let mut add = |amount: usize| -> Result<()> {
            bytes = bytes
                .checked_add(amount)
                .ok_or(Error::Budget("capture retained state overflow"))?;
            Ok(())
        };
        add(self.root.capacity())?;
        add(self.path.capacity())?;
        if self.sqlite_heap.is_none() {
            // Preserve the compatibility route's historical forecast. In the
            // dedicated route SQLite residency is the one retained backend
            // pool; physical database bytes are owned disk, not a second heap.
            add(usize::try_from(self.file_state.2)
                .map_err(|_| Error::Budget("capture retained database size"))?)?;
        }
        add(self
            .prepared_state
            .capacity()
            .checked_mul(std::mem::size_of::<(
                PathBuf,
                PathBuf,
                u64,
                u64,
                i64,
                i64,
                i64,
                i64,
            )>())
            .ok_or(Error::Budget("capture prepared state overflow"))?)?;
        for (selected, resolved, ..) in &self.prepared_state {
            add(selected.capacity())?;
            add(resolved.capacity())?;
        }
        add(self
            .sources
            .capacity()
            .checked_mul(std::mem::size_of::<SourceFile>())
            .ok_or(Error::Budget("capture source state overflow"))?)?;
        for source in &self.sources {
            add(source.label.capacity())?;
            if let SourceOrigin::File(path) = &source.origin {
                add(path.capacity())?;
            }
        }
        // These two counters are created and owned by this capture. Their Arc
        // aliases in views/receipts do not introduce another payload allocation.
        add(std::mem::size_of::<AtomicU64>()
            .checked_mul(2)
            .ok_or(Error::Budget("capture counter state overflow"))?)?;
        Ok(bytes)
    }

    pub fn create(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_profile(
            root, staging, limits, deadline, false, false, false, None, None, None,
        )
    }

    /// Five maintained prepare input roles. Software contracts are compiled
    /// code companions; optional public-release inputs are not prepare inputs.
    pub(crate) fn create_prepared(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_profile(
            root, staging, limits, deadline, true, false, false, None, None, None,
        )
    }
    /// Evidence Lens opens only views and canon node/edge collections.
    pub(crate) fn create_evidence(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_profile(
            root, staging, limits, deadline, false, true, false, None, None, None,
        )
    }

    /// Evidence Lens capture using the same selected Core input paths and
    /// caller-owned operation token. Its own source and route refs remain
    /// rooted at `root`; only the selected corpus/philosophy projections are
    /// redirected by this exact constructor.
    pub(crate) fn create_evidence_selected(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        selected.validate()?;
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        Self::create_profile(
            root,
            staging,
            limits,
            deadline,
            false,
            true,
            false,
            Some(selected),
            Some(cancelled),
            None,
        )
    }
    /// Existing Evidence-only selected profile under the parent's actual
    /// owned construction state. No fresh work/VM/JSON allowance is minted.
    pub(crate) fn create_evidence_selected_with_owned_state(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        staging: &Path,
        mut limits: PublicCaptureLimits,
        owner_deadline: Instant,
        state: &CreationState<'_>,
    ) -> Result<Self> {
        if state.operation_deadline() > owner_deadline {
            return Err(Error::Invalid(
                "owned Evidence cutoff exceeds owner deadline",
            ));
        }
        state.active()?;
        selected.validate()?;
        state.heap().verify_current()?;
        if state.work_limit > limits.max_work_bytes || state.sql_vm_limit > limits.max_sql_vm_steps
        {
            return Err(Error::Budget("owned Evidence original phase intersection"));
        }
        limits.max_work_bytes = state.work_limit;
        limits.max_sql_vm_steps = state.sql_vm_limit;
        let mut capture = Self::create_profile_owned(
            root,
            staging,
            limits,
            state.operation_deadline(),
            false,
            true,
            false,
            Some(selected),
            Some(state.cancellation_handle()),
            None,
            Some(state),
        )?;
        state.active()?;
        // Constructor SQL connection has dropped. Later openings use the
        // original owner lifetime while operations narrow independently.
        capture.deadline = owner_deadline;
        Ok(capture)
    }

    /// Runtime projection data belongs to the selected source root; executable
    /// contract/vocabulary companions belong to this exact compiled producer.
    pub fn create_runtime(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        Self::create_runtime_selected(
            root,
            &PublicCaptureInputPaths::runtime(root),
            staging,
            limits,
            deadline,
            cancelled,
        )
    }

    /// Capture Reference Core's exact seven selected paths. The five graph
    /// inputs are required. The selected audit and Evidence Lens projection
    /// are represented even when absent, so existence queries cannot silently
    /// fall back to a different root-relative file.
    pub fn create_runtime_selected(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        selected.validate()?;
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        let capture = Self::create_profile(
            root,
            staging,
            limits,
            deadline,
            false,
            false,
            true,
            Some(selected),
            Some(Arc::clone(&cancelled)),
            None,
        )?;
        let total = capture
            .retained_input_members()?
            .iter()
            .try_fold(0u64, |n, (_, _, len)| {
                n.checked_add(*len)
                    .filter(|bytes| *bytes <= limits.max_input_bytes)
                    .ok_or(Error::Budget("runtime capture aggregate source bytes"))
            })?;
        if total == 0 {
            return Err(Error::Invalid("runtime capture empty input"));
        }
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        Ok(capture)
    }

    /// Capture one selected original carrier without opening other graph roles,
    /// registries, evidence files or the public ledger. The audit role reads
    /// only its exact selected optional file. The callback below supplies the
    /// same pre/post capture fence as a full completed snapshot.
    pub fn create_runtime_carrier_selected(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        role: RuntimeCaptureRole,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        selected.validate()?;
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        let capture = Self::create_profile(
            root,
            staging,
            limits,
            deadline,
            false,
            false,
            true,
            Some(selected),
            Some(Arc::clone(&cancelled)),
            Some(role),
        )?;
        let selected_label = match role {
            RuntimeCaptureRole::Corpus => "ToS/derived-exports/tos_corpus_index.min.json",
            RuntimeCaptureRole::Philosophy => {
                "ToS/derived-exports/philosophy_graph_projection.min.json"
            }
            RuntimeCaptureRole::Bibliographic => {
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"
            }
            RuntimeCaptureRole::PhilosophyAudit => PHILOSOPHY_AUDIT_RELATIVE,
        };
        if !capture
            .sources
            .iter()
            .any(|source| source.label == selected_label && source.digest.is_some())
        {
            return Err(Error::Invalid("runtime carrier selected input absent"));
        }
        let total =
            capture
                .retained_input_members()?
                .iter()
                .try_fold(0u64, |total, (_, _, len)| {
                    total
                        .checked_add(*len)
                        .filter(|bytes| *bytes <= limits.max_input_bytes)
                        .ok_or(Error::Budget("runtime carrier aggregate source bytes"))
                })?;
        if total == 0 && role != RuntimeCaptureRole::PhilosophyAudit {
            return Err(Error::Invalid("runtime carrier capture empty input"));
        }
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        Ok(capture)
    }
    /// Create one serial lazy carrier against the caller's original shared work
    /// owner. The creation allowance is held before any source hashing or SQL
    /// work. A failed creation consumes that allowance; callers must terminate.
    /// Successful creation settles only the unused local allowance, then all
    /// capture reads and source fences debit the same original atomic owner.
    /// This is the work seam only; the caller must also admit creation state.
    pub fn create_runtime_carrier_selected_with_shared_work(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        role: RuntimeCaptureRole,
        staging: &Path,
        mut limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
        original_work: Arc<AtomicU64>,
        original_work_limit: u64,
        creation_work_allowance: u64,
    ) -> Result<Self> {
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        if creation_work_allowance == 0
            || creation_work_allowance > limits.max_work_bytes
            || limits.max_work_bytes > original_work_limit
        {
            return Err(Error::Budget("runtime carrier creation work allowance"));
        }
        let mut observed = original_work.load(std::sync::atomic::Ordering::Acquire);
        loop {
            let reserved = observed
                .checked_add(creation_work_allowance)
                .filter(|value| *value <= original_work_limit)
                .ok_or(Error::Budget("runtime carrier original creation work"))?;
            match original_work.compare_exchange_weak(
                observed,
                reserved,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(current) => observed = current,
            }
            check_capture_active(Some(cancelled.as_ref()), deadline)?;
        }
        limits.max_work_bytes = creation_work_allowance;
        let mut capture = Self::create_runtime_carrier_selected(
            root, selected, role, staging, limits, deadline, cancelled,
        )?;
        let actual = capture
            .work_bytes
            .load(std::sync::atomic::Ordering::Acquire);
        // The maintained constructor's own work guard proves actual <= allowance.
        // Keep the reservation on any inconsistency rather than lowering usage.
        let unused = creation_work_allowance
            .checked_sub(actual)
            .ok_or(Error::Budget("runtime carrier creation work settlement"))?;
        original_work.fetch_sub(unused, std::sync::atomic::Ordering::AcqRel);
        capture.work_bytes = original_work;
        capture.max_work_bytes = original_work_limit;
        capture.limits.max_work_bytes = original_work_limit;
        Ok(capture)
    }

    /// Controlled serial construction over the existing carrier or whole
    /// selected profile. This shares the original work/VM domains; usage is
    /// returned even when construction refuses. Source/state implementation
    /// remains an integration draft until every preallocation seam is joined.
    pub fn create_runtime_selected_with_owned_budget(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        profile: RuntimeCaptureProfile,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
        budget: RuntimeCaptureOwnedBudget<'_>,
        usage: &mut RuntimeCaptureCreationUsage,
    ) -> Result<Self> {
        Self::create_selected_with_owned_budget(
            root,
            Some(selected),
            true,
            profile,
            staging,
            limits,
            deadline,
            cancelled,
            budget,
            usage,
        )
    }

    /// Full public-site capture from the maintained public input route, under
    /// the original dedicated process heap, state, work and VM owners.
    pub(crate) fn create_public_with_owned_budget(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
        budget: RuntimeCaptureOwnedBudget<'_>,
        usage: &mut RuntimeCaptureCreationUsage,
    ) -> Result<Self> {
        Self::create_selected_with_owned_budget(
            root,
            None,
            false,
            RuntimeCaptureProfile::Whole,
            staging,
            limits,
            deadline,
            cancelled,
            budget,
            usage,
        )
    }

    /// Runtime full-root capture under the original dedicated native process.
    /// This preserves the maintained runtime input profile without requiring
    /// selected-path aliases or constructing a new resource domain.
    pub fn create_runtime_with_owned_budget(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
        budget: RuntimeCaptureOwnedBudget<'_>,
        usage: &mut RuntimeCaptureCreationUsage,
    ) -> Result<Self> {
        Self::create_selected_with_owned_budget(
            root,
            None,
            true,
            RuntimeCaptureProfile::Whole,
            staging,
            limits,
            deadline,
            cancelled,
            budget,
            usage,
        )
    }

    fn create_selected_with_owned_budget(
        root: &Path,
        selected: Option<&PublicCaptureInputPaths>,
        runtime_profile: bool,
        profile: RuntimeCaptureProfile,
        staging: &Path,
        mut limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
        budget: RuntimeCaptureOwnedBudget<'_>,
        usage: &mut RuntimeCaptureCreationUsage,
    ) -> Result<Self> {
        if usage.json_visits != 0 {
            return Err(Error::Invalid("runtime creation usage must start empty"));
        }
        if budget.creation_deadline > deadline {
            return Err(Error::Invalid(
                "runtime creation cutoff exceeds owner deadline",
            ));
        }
        let creation_deadline = budget.creation_deadline;
        check_capture_active(Some(cancelled.as_ref()), creation_deadline)?;
        if budget.creation_work_allowance == 0
            || budget.creation_work_allowance > limits.max_work_bytes
            || limits.max_work_bytes > budget.original_work_limit
            || limits.max_sql_vm_steps != budget.original_sql_vm_limit
            || budget.max_creation_json_visits == 0
        {
            return Err(Error::Budget("runtime creation original budgets"));
        }
        // A serial creator reserves one allowance before hashing/SQL/parsing.
        // Refusal retains it; only successful creation settles unused work.
        checked_add(
            &budget.original_work,
            usize::try_from(budget.creation_work_allowance)
                .map_err(|_| Error::Budget("runtime creation work width"))?,
            budget.original_work_limit,
        )?;
        limits.max_work_bytes = budget.creation_work_allowance;
        // Named fixed owners before allocating the local work Arc or the
        // stream's BufReader/Rc workspace. The recursive serde envelope is the
        // existing normalization owner's same depth-96 admission geometry.
        let frame = std::mem::size_of::<serde_json::Value>()
            .checked_add(std::mem::size_of::<JsonValue>())
            .and_then(|n| n.checked_add(512))
            .and_then(|n| n.checked_mul(97))
            .ok_or(Error::Budget("runtime creation frame state"))?;
        let fixed = 65536usize
            .checked_mul(2)
            .and_then(|n| n.checked_add(frame))
            .and_then(|n| n.checked_add(std::mem::size_of::<CreationState<'_>>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<CaptureWriter<'_>>()))
            .and_then(|n| {
                n.checked_add(
                    std::mem::size_of::<CreationReadState<'_, '_>>()
                        + 6 * std::mem::size_of::<usize>()
                        + std::mem::size_of::<AtomicU64>(),
                )
            })
            .ok_or(Error::Budget("runtime creation fixed state"))?;
        (budget.remaining_after_retained)(fixed)?;
        let state = CreationState {
            remaining_after_retained: budget.remaining_after_retained,
            retained: Cell::new(fixed),
            persistent: Cell::new(0),
            json_visits: Cell::new(0),
            max_json_visits: budget.max_creation_json_visits,
            sql_vm: Arc::clone(&budget.original_sql_vm),
            sql_vm_limit: budget.original_sql_vm_limit,
            sqlite_heap: Arc::clone(&budget.original_sqlite_heap),
            work: Arc::new(AtomicU64::new(0)),
            work_limit: budget.creation_work_allowance,
            deadline: creation_deadline,
            cancelled: cancelled.as_ref(),
            cancelled_handle: Arc::clone(&cancelled),
            capture_owner: None,
        };
        let result = (|| {
            state.sqlite_heap.verify_current()?;
            if let Some(selected) = selected {
                selected.validate()?;
            }
            let role = match profile {
                RuntimeCaptureProfile::Carrier(role) => Some(role),
                RuntimeCaptureProfile::Whole => None,
            };
            let capture = Self::create_profile_owned(
                root,
                staging,
                limits,
                creation_deadline,
                false,
                false,
                runtime_profile,
                selected,
                Some(Arc::clone(&cancelled)),
                role,
                Some(&state),
            )?;
            if let Some(role) = role {
                let label = match role {
                    RuntimeCaptureRole::Corpus => "ToS/derived-exports/tos_corpus_index.min.json",
                    RuntimeCaptureRole::Philosophy => {
                        "ToS/derived-exports/philosophy_graph_projection.min.json"
                    }
                    RuntimeCaptureRole::Bibliographic => {
                        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"
                    }
                    RuntimeCaptureRole::PhilosophyAudit => PHILOSOPHY_AUDIT_RELATIVE,
                };
                if !capture
                    .sources
                    .iter()
                    .any(|source| source.label == label && source.digest.is_some())
                {
                    return Err(Error::Invalid("runtime carrier selected input absent"));
                }
            }
            let total = capture.retained_input_bytes_borrowed(limits.max_input_bytes)?;
            if total == 0 && role != Some(RuntimeCaptureRole::PhilosophyAudit) {
                return Err(Error::Invalid("runtime capture empty input"));
            }
            // The callback includes caller-held state; retained capture is
            // still live here and must fit before it can be returned.
            state.transfer_persistent_to_capture()?;
            state.remaining(capture.retained_state_upper_bound()?)?;
            check_capture_active(Some(cancelled.as_ref()), creation_deadline)?;
            Ok(capture)
        })();
        usage.json_visits = state.json_visits.get();
        let mut capture = result?;
        let actual = state.work.load(std::sync::atomic::Ordering::Acquire);
        let unused = budget
            .creation_work_allowance
            .checked_sub(actual)
            .ok_or(Error::Budget("runtime creation work settlement"))?;
        // Final call-cutoff guard precedes settlement and lifetime promotion.
        // create_profile_owned dropped its construction connection; future
        // connections install shared SQL progress against self.deadline.
        check_capture_active(Some(cancelled.as_ref()), creation_deadline)?;
        budget
            .original_work
            .fetch_sub(unused, std::sync::atomic::Ordering::AcqRel);
        capture.deadline = deadline;
        capture.work_bytes = budget.original_work;
        capture.max_work_bytes = budget.original_work_limit;
        capture.limits.max_work_bytes = budget.original_work_limit;
        Ok(capture)
    }

    pub(crate) fn retained_input_length(&self, label: &str) -> Result<usize> {
        if let Some(raw) = self.prepared_software_input(label) {
            return Ok(raw.len());
        }
        self.check_custody()?;
        if let Some(source) = self
            .sources
            .iter()
            .find(|source| source.label == label && source.digest.is_some())
        {
            return usize::try_from(source.len)
                .map_err(|_| Error::Budget("owned model retained input width"));
        }
        // The retained reader also owns partition members. Obtain their
        // selected length before admitting/materializing the raw payload.
        let path = self.retained_member_path(label)?;
        let db = self.read_db()?;
        let len: Option<u64> = db
            .query_row(
                "SELECT size_bytes FROM capture_sources WHERE path=?1",
                [path
                    .to_str()
                    .ok_or(Error::Invalid("captured runtime path UTF8"))?],
                |row| row.get(0),
            )
            .optional()?;
        usize::try_from(len.ok_or(Error::Invalid("owned model retained input absent"))?)
            .map_err(|_| Error::Budget("owned model retained input width"))
    }

    fn retained_member_path(&self, label: &str) -> Result<PathBuf> {
        let selected = Path::new(label);
        if selected.is_absolute() {
            Ok(selected.to_owned())
        } else {
            let relative = tos_foundation::RelativePath::parse(label)
                .map_err(|_| Error::Invalid("captured runtime member path"))?;
            Ok(self.root.join(relative.as_str()))
        }
    }

    /// Existing allocation/JSON admission owner reused by the Whole producer.
    /// The capture must already belong to the dedicated controlled session;
    /// work and SQL are the original live counters, never fresh phase ledgers.
    pub(crate) fn model_creation_state<'a>(
        &'a self,
        remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
        heap: &Arc<sqlite_budget::DedicatedSessionSqliteHeap>,
        max_json_visits: usize,
        creation_deadline: Instant,
    ) -> Result<CreationState<'a>> {
        self.model_creation_state_with_phase(
            remaining_after_retained,
            heap,
            max_json_visits,
            creation_deadline,
            None,
        )
    }

    /// Immutable owner ceiling for restoring the same shared SQL hook after
    /// final operation fences; this does not admit work or mutate any counter.
    pub(crate) fn original_sql_vm_limit(&self) -> u64 {
        self.limits.max_sql_vm_steps
    }

    /// Borrow the authentic original capture for one serial operation. Derive
    /// its absolute phase intersections once, before frame admission or owner
    /// Arc copies. The capture/session ceilings and counters never change.
    pub(crate) fn model_creation_state_for_operation<'a>(
        &'a self,
        remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
        heap: &Arc<sqlite_budget::DedicatedSessionSqliteHeap>,
        max_json_visits: usize,
        creation_deadline: Instant,
        phase_work_bytes: u64,
        phase_sql_vm_steps: u64,
    ) -> Result<CreationState<'a>> {
        self.model_creation_state_with_phase(
            remaining_after_retained,
            heap,
            max_json_visits,
            creation_deadline,
            Some((phase_work_bytes, phase_sql_vm_steps)),
        )
    }

    fn model_creation_state_with_phase<'a>(
        &'a self,
        remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
        heap: &Arc<sqlite_budget::DedicatedSessionSqliteHeap>,
        max_json_visits: usize,
        creation_deadline: Instant,
        phase: Option<(u64, u64)>,
    ) -> Result<CreationState<'a>> {
        if !self.shared_vm || creation_deadline > self.deadline || max_json_visits == 0 {
            return Err(Error::Invalid("owned model original capture context"));
        }
        let (work_limit, sql_vm_limit) = if let Some((work_bytes, vm_steps)) = phase {
            if work_bytes == 0 || vm_steps == 0 {
                return Err(Error::Budget("owned model operation phase allowance"));
            }
            let work_used = self.work_bytes.load(Ordering::Acquire);
            let vm_used = self.vm_used.load(Ordering::Acquire);
            if work_used > self.max_work_bytes || vm_used > self.limits.max_sql_vm_steps {
                return Err(Error::Budget("owned model original counter exhausted"));
            }
            let work_phase = work_used
                .checked_add(work_bytes)
                .ok_or(Error::Budget("owned model phase work overflow"))?;
            let vm_phase = vm_used
                .checked_add(vm_steps)
                .ok_or(Error::Budget("owned model phase VM overflow"))?;
            (
                self.max_work_bytes.min(work_phase),
                self.limits.max_sql_vm_steps.min(vm_phase),
            )
        } else {
            (self.max_work_bytes, self.limits.max_sql_vm_steps)
        };
        let actual = self
            .sqlite_heap
            .as_ref()
            .ok_or(Error::Invalid("owned model heap absent"))?;
        if !Arc::ptr_eq(actual, heap) {
            return Err(Error::Invalid("owned model original heap identity"));
        }
        actual.verify_current()?;
        check_capture_active(Some(self.cancelled.as_ref()), creation_deadline)?;
        let frame = (std::mem::size_of::<serde_json::Value>() + std::mem::size_of::<JsonValue>() + 512)
            .checked_mul(97).and_then(|n| n.checked_add(std::mem::size_of::<CreationState<'_>>()))
            .and_then(|n| n.checked_add(3 * tos_source_store::PinnedSqliteConnection::immutable_retained_rust_state_upper_bound()))
            .and_then(|n| n.checked_add(3 * (self.path.as_os_str().as_encoded_bytes().len() + 1
                + std::mem::size_of::<rusqlite::Statement<'_>>())))
            .ok_or(Error::Budget("owned model parser frame state"))?;
        remaining_after_retained(frame)?;
        Ok(CreationState {
            remaining_after_retained,
            retained: Cell::new(frame),
            persistent: Cell::new(0),
            json_visits: Cell::new(0),
            max_json_visits,
            sql_vm: Arc::clone(&self.vm_used),
            sql_vm_limit,
            sqlite_heap: Arc::clone(actual),
            work: Arc::clone(&self.work_bytes),
            work_limit,
            deadline: creation_deadline,
            cancelled: self.cancelled.as_ref(),
            cancelled_handle: Arc::clone(&self.cancelled),
            capture_owner: Some(self),
        })
    }

    fn create_profile(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        prepared_profile: bool,
        evidence_profile: bool,
        runtime_profile: bool,
        selected_paths: Option<&PublicCaptureInputPaths>,
        cancelled: Option<Arc<std::sync::atomic::AtomicBool>>,
        runtime_capture_role: Option<RuntimeCaptureRole>,
    ) -> Result<Self> {
        Self::create_profile_owned(
            root,
            staging,
            limits,
            deadline,
            prepared_profile,
            evidence_profile,
            runtime_profile,
            selected_paths,
            cancelled,
            runtime_capture_role,
            None,
        )
    }

    fn create_profile_owned<'a>(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        prepared_profile: bool,
        evidence_profile: bool,
        runtime_profile: bool,
        selected_paths: Option<&PublicCaptureInputPaths>,
        cancelled: Option<Arc<std::sync::atomic::AtomicBool>>,
        runtime_capture_role: Option<RuntimeCaptureRole>,
        creation: Option<&'a CreationState<'a>>,
    ) -> Result<Self> {
        limits.validate()?;
        let cancelled =
            cancelled.unwrap_or_else(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));
        let cancelled_ref = Some(cancelled.as_ref());
        let prepared_state = if prepared_profile {
            prepared_state(root)?
        } else {
            Vec::new()
        };
        check_capture_active(cancelled_ref, deadline)?;
        if let Some(state) = creation {
            // Before path syscalls, rusqlite path CString and failure cleanup
            // suffix copies, not only before the final returned owner clones.
            state.retain(
                root.as_os_str()
                    .as_encoded_bytes()
                    .len()
                    .checked_add(staging.as_os_str().as_encoded_bytes().len())
                    .and_then(|n| n.max(8).checked_mul(4))
                    .ok_or(Error::Budget("runtime capture root/path state"))?,
            )?;
            state.retain(controlled_fence_workspace_upper(
                runtime_capture_role.is_none(),
            ))?;
            state.retain(
                2 * std::mem::size_of::<usize>() + std::mem::size_of::<Option<Arc<()>>>(),
            )?;
        }
        // Allocate once after authentic creation state admission. None legacy
        // captures do not gain an issuer token or a detached controlled route.
        let controlled_identity = creation.map(|_| Arc::new(()));
        if staging.exists() || staging.is_symlink() {
            return Err(Error::Invalid("public D1 capture staging must be fresh"));
        }
        let mut pending = PendingCapture {
            path: staging,
            complete: false,
        };
        // Reserve the original shared VM quantum before opening the connection.
        if let Some(state) = creation {
            state.sqlite_heap.verify_current()?;
        }
        let shared_window = creation
            .map(|state| {
                sqlite_budget::SharedVmWindow::reserve(
                    Arc::clone(&state.sql_vm),
                    state.sql_vm_limit,
                )
            })
            .transpose()?;
        let vm_used = creation
            .map(|state| Arc::clone(&state.sql_vm))
            .unwrap_or_else(|| Arc::new(AtomicU64::new(0)));
        // The family writer reopens only an owned private 0600 inode. SQLite's
        // implicit creation mode depends on ambient umask, so create this
        // disposable capture explicitly before handing it to SQLite.
        {
            let _inode_hold = creation
                .map(|state| state.hold(std::mem::size_of::<File>()))
                .transpose()?;
            let inode = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(staging)?;
            inode.set_permissions(fs::Permissions::from_mode(0o600))?;
            drop(inode);
        }
        let mut db = Connection::open(staging)?;
        if let Some(window) = shared_window {
            window.install(&db, deadline, Arc::clone(&cancelled));
        } else {
            sqlite_budget::install_progress_until(
                &db,
                limits.sqlite(),
                Arc::clone(&vm_used),
                deadline,
            );
        }
        db.execute_batch("PRAGMA page_size=4096; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE;")?;
        let pages = limits.max_staging_bytes / 4096;
        if pages < 16 || pages > i64::MAX as u64 {
            return Err(Error::Budget("public D1 capture page bound"));
        }
        let main_pages: i64 =
            db.query_row(&format!("PRAGMA max_page_count={pages}"), [], |row| {
                row.get(0)
            })?;
        let temp_pages: i64 =
            db.query_row(&format!("PRAGMA temp.max_page_count={pages}"), [], |row| {
                row.get(0)
            })?;
        if main_pages != pages as i64 || temp_pages != pages as i64 {
            return Err(Error::Budget("public D1 capture page admission"));
        }
        db.execute_batch("CREATE TABLE capture_rows(role TEXT NOT NULL,collection TEXT NOT NULL,source_key TEXT NOT NULL,ord INTEGER NOT NULL,sort0 TEXT NOT NULL,sort1 TEXT NOT NULL,json BLOB NOT NULL,sha256 BLOB NOT NULL,PRIMARY KEY(role,collection,source_key)) WITHOUT ROWID; CREATE INDEX capture_rows_order ON capture_rows(role,collection,sort0,sort1,source_key); CREATE TABLE capture_collections(role TEXT NOT NULL,collection TEXT NOT NULL,kind TEXT NOT NULL,PRIMARY KEY(role,collection)) WITHOUT ROWID; CREATE TABLE capture_headers(role TEXT NOT NULL,path TEXT NOT NULL,json BLOB NOT NULL,PRIMARY KEY(role,path)) WITHOUT ROWID; CREATE TABLE capture_sources(path TEXT PRIMARY KEY,sha256 BLOB NOT NULL,size_bytes INTEGER NOT NULL) WITHOUT ROWID;")?;
        let mut rows = 0u64;
        let work_bytes = creation
            .map(|state| Arc::clone(&state.work))
            .unwrap_or_else(|| Arc::new(AtomicU64::new(0)));
        if let Some(state) = creation {
            // Three main roles, one audit, eighteen contracts, two optional
            // companions plus the maintained bounded4096 public-ledger
            // entries. Geometric source-vector old/new slots, no payload copy.
            state.retain(
                std::mem::size_of::<SourceFile>()
                    .checked_mul(
                        4 * if runtime_capture_role.is_some() {
                            1
                        } else {
                            4120
                        },
                    )
                    .ok_or(Error::Budget("runtime source slot state"))?,
            )?;
        }
        let mut sources = Vec::new();
        let mut corpus_partitioned = None;
        let mut claims_partitioned = None;
        for (role, relative) in [
            (CORPUS, "ToS/derived-exports/tos_corpus_index.min.json"),
            (
                PHILOSOPHY,
                "ToS/derived-exports/philosophy_graph_projection.min.json",
            ),
            (
                CLAIMS,
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            ),
        ] {
            if runtime_capture_role.is_some_and(|selected_role| {
                !matches!(
                    (selected_role, role),
                    (RuntimeCaptureRole::Corpus, CORPUS)
                        | (RuntimeCaptureRole::Philosophy, PHILOSOPHY)
                        | (RuntimeCaptureRole::Bibliographic, CLAIMS)
                )
            }) {
                continue;
            }
            if evidence_profile && role == CLAIMS {
                continue;
            }
            let role = if evidence_profile {
                if role == CORPUS {
                    "evidence-corpus"
                } else {
                    "evidence-philosophy"
                }
            } else {
                role
            };
            let path = creation_source_path(
                root,
                selected_paths.and_then(|selected| selected.core_path(relative)),
                relative,
                creation,
            )?;
            let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(cancelled_ref, deadline)?;
                checked_add(&work_bytes, n, limits.max_work_bytes)
            })?;
            check_capture_active(cancelled_ref, deadline)?;
            db.execute_batch("BEGIN IMMEDIATE")?;
            let creation_read = creation.map(|owner| {
                Rc::new(CreationReadState {
                    owner,
                    stream_read: Cell::new(0),
                    scratch_capacity: Cell::new(0),
                    admitted: Cell::new(0),
                    tree_admitted: Cell::new(0),
                })
            });
            let mut writer = CaptureWriter {
                creation,
                creation_read,
                db: &db,
                role,
                row_count: &mut rows,
                work: &work_bytes,
                limits,
                ordinals: Vec::new(),
                ordinal_slots_admitted: 0,
                read_budget: Rc::new(Cell::new(MAX_HEADER_BYTES + 65536)),
                deadline,
                cancelled: cancelled_ref,
                dynamic_philosophy: role == PHILOSOPHY
                    && runtime_capture_role == Some(RuntimeCaptureRole::Philosophy),
                page_rows: 0,
                page_bytes: 0,
            };
            let mut partitioned = false;
            if len <= MAX_ROOT_BYTES {
                let _raw_hold = creation.map(|state| state.hold(len as usize)).transpose()?;
                let mut raw = vec![0u8; len as usize];
                (&mut file).read_exact(&mut raw)?;
                let mut eof = [0u8; 1];
                if file.read(&mut eof)? != 0 || raw.len() as u64 != len {
                    return Err(Error::Invalid("public D1 source changed during capture"));
                }
                file.seek(SeekFrom::Start(0))?;
                check_capture_active(cancelled_ref, deadline)?;
                let first = strict_value_owned(&raw, MAX_ROOT_BYTES as usize, creation)?;
                check_capture_active(cancelled_ref, deadline)?;
                if first
                    .get("schema_version")
                    .and_then(serde_json::Value::as_str)
                    == Some("tos_partitioned_projection_v1")
                {
                    capture_partitioned(&path, &raw, &mut writer)?;
                    partitioned = true;
                }
            }
            if !partitioned {
                let buffered = BufReader::with_capacity(64 * 1024, &mut file);
                let bounded = CaptureReader {
                    creation_read: writer.creation_read.as_ref().map(Rc::clone),
                    inner: buffered,
                    remaining: Rc::clone(&writer.read_budget),
                    deadline,
                    cancelled: cancelled_ref,
                };
                let mut deserializer = serde_json::Deserializer::from_reader(bounded);
                ObjectSeed {
                    writer: &mut writer,
                    prefix: "",
                }
                .deserialize(&mut deserializer)
                .map_err(|e| Error::Source(e.to_string()))?;
                writer.read_budget.set(MAX_HEADER_BYTES + 65536);
                deserializer
                    .end()
                    .map_err(|e| Error::Source(e.to_string()))?;
            }
            let _schema_hold = creation.map(|state| state.hold(4096)).transpose()?;
            let stored_schema: Option<Vec<u8>> = db
                .query_row(
                    "SELECT CASE WHEN length(json)<=4096 THEN json ELSE NULL END FROM capture_headers WHERE role=?1 AND path='schema_version'",
                    [role],
                    |row| row.get(0),
                )
                .optional()?;
            let expected_schema = match role {
                CORPUS | "evidence-corpus" => "tos_corpus_index_v1",
                PHILOSOPHY | "evidence-philosophy" => "tos_philosophy_graph_projection_v2",
                CLAIMS => "tos_source_witness_bibliographic_graph_v1",
                _ => return Err(Error::Invalid("public D1 source role")),
            };
            let schema = stored_schema
                .as_deref()
                .map(|raw| writer.json_owned(raw, 4096))
                .transpose()?;
            if runtime_capture_role == Some(RuntimeCaptureRole::Philosophy) {
                if !matches!(
                    schema.as_ref().and_then(|value| value.as_str()),
                    Some(
                        "tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2"
                    )
                ) {
                    return Err(Error::Invalid("runtime philosophy source schema"));
                }
            } else if prepared_profile {
                if role == PHILOSOPHY
                    && !matches!(
                        schema.as_ref().and_then(|value| value.as_str()),
                        Some(
                            "tos_philosophy_graph_projection_v1"
                                | "tos_philosophy_graph_projection_v2"
                        )
                    )
                {
                    return Err(Error::Invalid("prepared philosophy source schema"));
                }
            } else if schema.as_ref().and_then(|value| value.as_str()) != Some(expected_schema) {
                return Err(Error::Invalid("public D1 source schema"));
            }
            if matches!(role, CORPUS | "evidence-corpus") {
                corpus_partitioned = Some(partitioned);
            }
            if role == CLAIMS {
                claims_partitioned = Some(partitioned);
            }
            drop(writer);
            db.execute_batch("COMMIT")?;
            check_capture_active(cancelled_ref, deadline)?;
            sources.push(SourceFile {
                label: relative.to_owned(),
                origin: SourceOrigin::File(path),
                digest: Some(digest),
                len,
            });
        }
        if runtime_capture_role == Some(RuntimeCaptureRole::PhilosophyAudit) {
            let selected = selected_paths
                .and_then(|selected| selected.optional_path(PHILOSOPHY_AUDIT_RELATIVE))
                .ok_or(Error::Invalid("selected philosophy audit path absent"))?;
            let path =
                creation_source_path(root, Some(selected), PHILOSOPHY_AUDIT_RELATIVE, creation)?;
            let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(cancelled_ref, deadline)?;
                checked_add(&work_bytes, n, limits.max_work_bytes)
            })?;
            check_capture_active(cancelled_ref, deadline)?;
            sources.push(SourceFile {
                label: PHILOSOPHY_AUDIT_RELATIVE.to_owned(),
                origin: SourceOrigin::File(path),
                digest: Some(digest),
                len,
            });
        }
        if !prepared_profile
            && !evidence_profile
            && runtime_capture_role.is_none()
            && corpus_partitioned != claims_partitioned
        {
            return Err(Error::Invalid("public D1 coupled projection storage mode"));
        }
        for relative in [
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        ]
        .into_iter()
        .chain(runtime_companions().map(|(path, _)| path))
        {
            if evidence_profile || runtime_capture_role.is_some() {
                continue;
            }
            if prepared_profile
                && !matches!(
                    relative,
                    "ToS/doctrine/semantic-interchange/entity-types.v1.json"
                        | "ToS/doctrine/semantic-interchange/relation-types.v1.json"
                )
            {
                continue;
            }
            if runtime_profile {
                if let Some((_, raw)) = runtime_companions().find(|(path, _)| *path == relative) {
                    if raw.len() as u64 > limits.max_input_bytes {
                        return Err(Error::Budget("runtime compiled companion bytes"));
                    }
                    checked_add(&work_bytes, raw.len(), limits.max_work_bytes)?;
                    if let Some(state) = creation {
                        state.retain(relative.len())?;
                    }
                    sources.push(SourceFile {
                        label: relative.to_owned(),
                        origin: SourceOrigin::Compiled(raw),
                        digest: Some(Digest256::of_bytes(raw)),
                        len: raw.len() as u64,
                    });
                    continue;
                }
            }
            let path = creation_source_path(
                root,
                selected_paths.and_then(|selected| selected.core_path(relative)),
                relative,
                creation,
            )?;
            let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(cancelled_ref, deadline)?;
                checked_add(&work_bytes, n, limits.max_work_bytes)
            })?;
            check_capture_active(cancelled_ref, deadline)?;
            sources.push(SourceFile {
                label: relative.to_owned(),
                origin: SourceOrigin::File(path),
                digest: Some(digest),
                len,
            });
        }
        if !prepared_profile && !evidence_profile && runtime_capture_role.is_none() {
            for relative in [
                "ToS/derived-exports/epistemic_evidence_projection.min.json",
                "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
            ] {
                let path = creation_source_path(
                    root,
                    selected_paths.and_then(|selected| selected.optional_path(relative)),
                    relative,
                    creation,
                )?;
                if path.exists() || path.is_symlink() {
                    let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
                    let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                        check_capture_active(cancelled_ref, deadline)?;
                        checked_add(&work_bytes, n, limits.max_work_bytes)
                    })?;
                    check_capture_active(cancelled_ref, deadline)?;
                    sources.push(SourceFile {
                        label: relative.to_owned(),
                        origin: SourceOrigin::File(path),
                        digest: Some(digest),
                        len,
                    });
                } else {
                    sources.push(SourceFile {
                        label: relative.to_owned(),
                        origin: SourceOrigin::File(path),
                        digest: None,
                        len: 0,
                    });
                }
            }
            let ledger_relative = "ToS/source-witnesses/access-requests/public-ledger";
            let ledger = creation_source_path(root, None, ledger_relative, creation)?;
            if ledger.exists() {
                if ledger.is_symlink() || !ledger.is_dir() {
                    return Err(Error::Invalid("public D1 ledger directory"));
                }
                let mut names = if creation.is_some() {
                    controlled_ledger_names(&ledger, |bytes| {
                        check_capture_active(cancelled_ref, deadline)?;
                        checked_add(&work_bytes, bytes, limits.max_work_bytes)
                    })?
                } else {
                    Vec::new()
                };
                if creation.is_none() {
                    for entry in std::fs::read_dir(&ledger)? {
                        if names.len() == 4096 {
                            return Err(Error::Budget("public D1 ledger membership"));
                        }
                        let name = entry?.file_name();
                        checked_add(
                            &work_bytes,
                            name.as_encoded_bytes().len(),
                            limits.max_work_bytes,
                        )?;
                        names.push(name);
                    }
                }
                if creation.is_none() {
                    names.sort();
                }
                for name in names {
                    let name = name
                        .to_str()
                        .ok_or(Error::Invalid("public D1 ledger filename"))?;
                    if !name.ends_with(".access-request.json") {
                        continue;
                    }
                    let path = creation_source_path(&ledger, None, name, creation)?;
                    if let Some(state) = creation {
                        state.retain(ledger_relative.len() + name.len() + 1)?;
                    }
                    let mut file = safe_open::open_regular(&path, 256_000)?;
                    let (digest, len) = source_digest(&mut file, 256_000, |n| {
                        check_capture_active(cancelled_ref, deadline)?;
                        checked_add(&work_bytes, n, limits.max_work_bytes)
                    })?;
                    check_capture_active(cancelled_ref, deadline)?;
                    sources.push(SourceFile {
                        label: format!("{ledger_relative}/{name}"),
                        origin: SourceOrigin::File(path),
                        digest: Some(digest),
                        len,
                    });
                }
            }
        }
        if std::fs::metadata(staging)?.len() > limits.max_staging_bytes {
            return Err(Error::Budget("public D1 capture physical bytes"));
        }
        check_capture_active(cancelled_ref, deadline)?;
        drop(db);
        let metadata = fs::symlink_metadata(staging)?;
        if !metadata.file_type().is_file() {
            return Err(Error::Invalid("public D1 capture inode"));
        }
        pending.complete = true;
        Ok(Self {
            root: root.to_owned(),
            prepared_profile,
            evidence_profile,
            runtime_capture_role,
            prepared_state,
            path: staging.to_owned(),
            inode: (metadata.dev(), metadata.ino()),
            file_state: (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ),
            family_seal: Mutex::new(FamilyPreparationSeal::Initial),
            sources,
            partitioned: corpus_partitioned == Some(true)
                || (prepared_profile && claims_partitioned == Some(true)),
            rows,
            work_bytes,
            max_work_bytes: limits.max_work_bytes,
            deadline,
            operation_deadline: Mutex::new(None),
            limits,
            vm_used,
            shared_vm: creation.is_some(),
            sqlite_heap: creation.map(|state| Arc::clone(&state.sqlite_heap)),
            controlled_identity,
            cancelled,
        })
    }

    /// Issued only from the actual controlled capture and its authentic state.
    /// The recipient holds this token while its verified model remains live.
    pub(crate) fn controlled_identity(
        &self,
        state: &CreationState<'_>,
    ) -> Result<ControlledCaptureIdentity> {
        state.active()?;
        let heap = self
            .sqlite_heap
            .as_ref()
            .ok_or(Error::Invalid("capture controlled owner absent"))?;
        if !Arc::ptr_eq(heap, &state.sqlite_heap)
            || !Arc::ptr_eq(&self.work_bytes, &state.work)
            || !Arc::ptr_eq(&self.vm_used, &state.sql_vm)
            || !Arc::ptr_eq(&self.cancelled, &state.cancelled_handle)
        {
            return Err(Error::Invalid("capture controlled state owner differs"));
        }
        let token = self
            .controlled_identity
            .as_ref()
            .ok_or(Error::Invalid("capture controlled identity absent"))?;
        let _result_hold = state.hold(std::mem::size_of::<Result<ControlledCaptureIdentity>>())?;
        state.retain(std::mem::size_of::<ControlledCaptureIdentity>())?;
        state.charge_work(std::mem::size_of::<ControlledCaptureIdentity>())?;
        Ok(ControlledCaptureIdentity {
            token: Arc::clone(token),
        })
    }
    /// Borrowed comparison does not issue another token or accumulate a fresh
    /// retention on every query. Original source/currentness disclosure fences
    /// remain the caller's genuine capture-owner operations.
    pub(crate) fn matches_controlled_identity(
        &self,
        identity: &ControlledCaptureIdentity,
    ) -> Result<bool> {
        check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
        let token = self
            .controlled_identity
            .as_ref()
            .ok_or(Error::Invalid("capture controlled identity absent"))?;
        Ok(Arc::ptr_eq(token, &identity.token))
    }

    /// Narrow one serial controlled operation without replacing the retained
    /// capture lifetime or any original counter. SQL connections opened inside
    /// the operation must be dropped before its callback returns.
    pub(crate) fn with_owned_operation_deadline<T>(
        &self,
        deadline: Instant,
        operation: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        self.with_owned_operation_deadline_and_limits(deadline, None, operation)
    }

    /// Phase limits are deltas against the original monotonic counters. They
    /// narrow this serial operation without resetting or refunding prior work.
    pub(crate) fn with_owned_operation_deadline_and_limits<T>(
        &self,
        deadline: Instant,
        phase_limits: Option<(u64, u64)>,
        operation: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        if !self.shared_vm || deadline > self.deadline {
            return Err(Error::Invalid("controlled capture operation deadline"));
        }
        check_capture_active(Some(self.cancelled.as_ref()), deadline)?;
        {
            let mut active = self
                .operation_deadline
                .lock()
                .map_err(|_| Error::Invalid("capture operation deadline poisoned"))?;
            if active.is_some() {
                return Err(Error::Invalid("overlapping controlled capture operation"));
            }
            let (work_limit, vm_limit) = match phase_limits {
                Some((work, vm)) => (
                    self.work_bytes
                        .load(std::sync::atomic::Ordering::Acquire)
                        .checked_add(work)
                        .ok_or(Error::Budget("capture phase work overflow"))?
                        .min(self.max_work_bytes),
                    self.vm_used
                        .load(std::sync::atomic::Ordering::Acquire)
                        .checked_add(vm)
                        .ok_or(Error::Budget("capture phase VM overflow"))?
                        .min(self.limits.max_sql_vm_steps),
                ),
                None => (self.max_work_bytes, self.limits.max_sql_vm_steps),
            };
            *active = Some(CaptureOperation {
                deadline,
                work_limit,
                vm_limit,
                phase_limited: phase_limits.is_some(),
            });
        }
        struct Restore<'a>(&'a Mutex<Option<CaptureOperation>>);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                match self.0.lock() {
                    Ok(mut value) => *value = None,
                    Err(poisoned) => *poisoned.into_inner() = None,
                }
            }
        }
        let _restore = Restore(&self.operation_deadline);
        let result = operation();
        if result.is_ok() {
            check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
            if self.work_bytes.load(std::sync::atomic::Ordering::Acquire)
                > self.active_work_limit()?
                || self.vm_used.load(std::sync::atomic::Ordering::Acquire)
                    > self.active_vm_limit()?
            {
                return Err(Error::Budget("capture operation final work/VM fence"));
            }
        }
        result
    }

    /// End the verified cold scan's work/VM intersection while keeping this
    /// operation's original cutoff and monotonic counters through QRY/close.
    /// The caller reinstalls its SQL progress hook with the original ceiling
    /// before the independently budgeted query begins.
    pub(crate) fn finish_owned_operation_phase_limits(&self) -> Result<()> {
        let mut active = self
            .operation_deadline
            .lock()
            .map_err(|_| Error::Invalid("capture operation deadline poisoned"))?;
        let phase = active
            .as_mut()
            .filter(|phase| phase.phase_limited)
            .ok_or(Error::Invalid("capture limited operation phase absent"))?;
        check_capture_active(Some(self.cancelled.as_ref()), phase.deadline)?;
        if self.work_bytes.load(std::sync::atomic::Ordering::Acquire) > phase.work_limit
            || self.vm_used.load(std::sync::atomic::Ordering::Acquire) > phase.vm_limit
        {
            return Err(Error::Budget("capture cold phase final work/VM fence"));
        }
        phase.work_limit = self.max_work_bytes;
        phase.vm_limit = self.limits.max_sql_vm_steps;
        phase.phase_limited = false;
        Ok(())
    }

    pub(crate) fn active_deadline(&self) -> Result<Instant> {
        let active = self
            .operation_deadline
            .lock()
            .map_err(|_| Error::Invalid("capture operation deadline poisoned"))?;
        Ok(active.map_or(self.deadline, |phase| phase.deadline.min(self.deadline)))
    }

    fn active_work_limit(&self) -> Result<u64> {
        let active = self
            .operation_deadline
            .lock()
            .map_err(|_| Error::Invalid("capture operation deadline poisoned"))?;
        Ok(active.map_or(self.max_work_bytes, |phase| phase.work_limit))
    }

    fn active_vm_limit(&self) -> Result<u64> {
        let active = self
            .operation_deadline
            .lock()
            .map_err(|_| Error::Invalid("capture operation deadline poisoned"))?;
        Ok(active.map_or(self.limits.max_sql_vm_steps, |phase| phase.vm_limit))
    }

    pub fn charge_work(&self, bytes: u64) -> Result<()> {
        check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
        if Instant::now() >= self.active_deadline()? {
            return Err(Error::Budget("public D1 build deadline"));
        }
        let limit = self.active_work_limit()?;
        let mut current = self.work_bytes.load(std::sync::atomic::Ordering::Acquire);
        loop {
            let next = current
                .checked_add(bytes)
                .filter(|value| *value <= limit)
                .ok_or(Error::Budget("public D1 build work bytes"))?;
            match self.work_bytes.compare_exchange_weak(
                current,
                next,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(observed) => current = observed,
            }
            check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
        }
    }

    /// Lend the exact selected original carrier under this capture's pre/post
    /// source fence. A partial-role capture intentionally has no whole Core
    /// `source_revision`; the view reports that absence explicitly.
    pub fn with_captured_carriers<'a, T>(
        &'a self,
        consume: impl for<'view> FnOnce(
            &'view crate::native_snapshot::CompletedCaptureCarriers<'a>,
        ) -> Result<T>,
    ) -> Result<T> {
        if self.runtime_capture_role.is_none() {
            return Err(Error::Invalid("selected carrier profile required"));
        }
        self.verify_captured_inputs()?;
        let view = crate::native_snapshot::CompletedCaptureCarriers::from_selected_capture(self);
        let result = consume(&view);
        let current = self.verify_captured_inputs();
        match current {
            Err(error) => Err(error),
            Ok(()) => result,
        }
    }

    /// Borrow only the present audit member already owned by this exact capture.
    /// The path is provenance metadata; bytes still pass through `read_input`.
    pub(crate) fn selected_philosophy_audit_path(&self) -> Result<&Path> {
        self.check_custody()?;
        let source = self
            .sources
            .iter()
            .find(|source| source.label == PHILOSOPHY_AUDIT_RELATIVE)
            .filter(|source| source.digest.is_some())
            .ok_or(Error::Invalid("selected philosophy audit input absent"))?;
        match &source.origin {
            SourceOrigin::File(path) => Ok(path),
            SourceOrigin::Compiled(_) => {
                Err(Error::Invalid("selected philosophy audit file required"))
            }
        }
    }

    pub(crate) fn read_selected_philosophy_audit(&self, cap: usize) -> Result<Vec<u8>> {
        self.selected_philosophy_audit_path()?;
        self.read_input(PHILOSOPHY_AUDIT_RELATIVE, cap)?
            .ok_or(Error::Invalid("selected philosophy audit input absent"))
    }

    pub fn work_bytes(&self) -> u64 {
        self.work_bytes.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn vm_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.vm_used)
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn work_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.work_bytes)
    }

    pub(crate) fn max_work_bytes(&self) -> u64 {
        self.max_work_bytes
    }

    pub(crate) fn cancellation(&self) -> &std::sync::atomic::AtomicBool {
        self.cancelled.as_ref()
    }

    pub(crate) fn cancellation_handle(&self) -> Arc<std::sync::atomic::AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    pub(crate) fn runtime_input_paths_owned(
        &self,
        state: &CreationState<'_>,
    ) -> Result<PublicCaptureInputPaths> {
        let mut paths: [Option<&PathBuf>; 7] = [None; 7];
        let mut bytes = std::mem::size_of::<PublicCaptureInputPaths>();
        for (index, label) in [
            "ToS/derived-exports/tos_corpus_index.min.json",
            "ToS/derived-exports/philosophy_graph_projection.min.json",
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
            "ToS/derived-exports/epistemic_evidence_projection.min.json",
        ]
        .iter()
        .enumerate()
        {
            for source in &self.sources {
                state.active()?;
                state.charge_work(
                    source
                        .label
                        .len()
                        .checked_add(label.len())
                        .ok_or(Error::Budget("owned runtime label comparison"))?,
                )?;
                if source.label != *label {
                    continue;
                }
                let SourceOrigin::File(path) = &source.origin else {
                    return Err(Error::Invalid("runtime selected input path absent"));
                };
                state.charge_work(path.as_os_str().len())?;
                bytes = bytes
                    .checked_add(path.as_os_str().len())
                    .ok_or(Error::Budget("owned runtime selected paths"))?;
                paths[index] = Some(path);
                break;
            }
            if paths[index].is_none() {
                return Err(Error::Invalid("runtime selected input path absent"));
            }
        }
        state.retain(bytes)?;
        Ok(PublicCaptureInputPaths {
            index_path: paths[0]
                .ok_or(Error::Invalid("runtime selected input path absent"))?
                .clone(),
            philosophy_graph_projection_path: paths[1]
                .ok_or(Error::Invalid("runtime selected input path absent"))?
                .clone(),
            bibliographic_graph_path: paths[2]
                .ok_or(Error::Invalid("runtime selected input path absent"))?
                .clone(),
            entity_type_registry_path: paths[3]
                .ok_or(Error::Invalid("runtime selected input path absent"))?
                .clone(),
            relation_type_registry_path: paths[4]
                .ok_or(Error::Invalid("runtime selected input path absent"))?
                .clone(),
            philosophy_post_planting_audit_path: paths[5]
                .ok_or(Error::Invalid("runtime selected input path absent"))?
                .clone(),
            evidence_projection_path: paths[6]
                .ok_or(Error::Invalid("runtime selected input path absent"))?
                .clone(),
        })
    }

    pub(crate) fn runtime_input_paths(&self) -> Result<PublicCaptureInputPaths> {
        fn selected_path(capture: &PublicCapture, label: &str) -> Result<PathBuf> {
            capture
                .sources
                .iter()
                .find(|source| source.label == label)
                .and_then(|source| match &source.origin {
                    SourceOrigin::File(path) => Some(path.clone()),
                    SourceOrigin::Compiled(_) => None,
                })
                .ok_or(Error::Invalid("runtime selected input path absent"))
        }
        Ok(PublicCaptureInputPaths {
            index_path: selected_path(self, "ToS/derived-exports/tos_corpus_index.min.json")?,
            philosophy_graph_projection_path: selected_path(
                self,
                "ToS/derived-exports/philosophy_graph_projection.min.json",
            )?,
            bibliographic_graph_path: selected_path(
                self,
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            )?,
            entity_type_registry_path: selected_path(
                self,
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            )?,
            relation_type_registry_path: selected_path(
                self,
                "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            )?,
            philosophy_post_planting_audit_path: selected_path(
                self,
                "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
            )?,
            evidence_projection_path: selected_path(
                self,
                "ToS/derived-exports/epistemic_evidence_projection.min.json",
            )?,
        })
    }

    fn expected_capture_file_state(&self) -> Result<CaptureFileState> {
        let seal = self
            .family_seal
            .lock()
            .map_err(|_| Error::Invalid("public D1 family seal poisoned"))?;
        match &*seal {
            FamilyPreparationSeal::Initial => Ok(self.file_state),
            FamilyPreparationSeal::Prepared(state) => Ok(*state),
            FamilyPreparationSeal::Preparing(_) => {
                Err(Error::Invalid("public D1 family preparation incomplete"))
            }
            FamilyPreparationSeal::Failed => {
                Err(Error::Invalid("public D1 family preparation failed"))
            }
        }
    }
    /// The one sanctioned mutation of this private capture: the maintained
    /// family-reference preparation. A failed/partial preparation permanently
    /// poisons the capture; arbitrary writers cannot refresh its stamp.
    pub(crate) fn prepare_family_rows_once(&self, limits: PublicCaptureLimits) -> Result<()> {
        self.prepare_family_rows_once_with_owned_state(limits, None)
    }

    pub(crate) fn prepare_family_rows_once_with_owned_state(
        &self,
        limits: PublicCaptureLimits,
        state: Option<&CreationState<'_>>,
    ) -> Result<()> {
        let _held_workspace = match state {
            Some(state) => Some(
                state.hold(
                    self.path
                        .as_os_str()
                        .len()
                        .checked_mul(4)
                        .and_then(|n| {
                            n.checked_add(
                                std::mem::size_of::<File>()
                                    + 4 * std::mem::size_of::<fs::Metadata>(),
                            )
                        })
                        .ok_or(Error::Budget("owned family held path workspace"))?,
                )?,
            ),
            None => None,
        };
        let deadline = self.active_deadline()?;
        self.capture_identity()?;
        self.verify_captured_inputs()?;
        let held = safe_open::open_regular(&self.path, self.limits.max_staging_bytes)?;
        let initial = capture_file_state(&held.metadata()?)?;
        if initial != self.capture_identity()? {
            return Err(Error::Invalid("public D1 family initial custody"));
        }
        {
            let mut seal = self
                .family_seal
                .lock()
                .map_err(|_| Error::Invalid("public D1 family seal poisoned"))?;
            if !matches!(*seal, FamilyPreparationSeal::Initial) {
                return Err(Error::Invalid(
                    "public D1 family preparation already attempted",
                ));
            }
            *seal = FamilyPreparationSeal::Preparing(std::thread::current().id());
        }
        let mut guard = FamilyPreparationGuard {
            capture: self,
            complete: false,
        };
        // The maintained SQL owner closes all statements/connections before
        // returning. Only this exact operation may establish the successor.
        match state {
            Some(state) => crate::d1_public_graph::prepare_family_rows_owned_unsealed(
                self, limits, &held, state,
            )?,
            None => crate::d1_public_graph::prepare_family_rows_unsealed(self, limits, &held)?,
        };
        check_capture_active(Some(self.cancelled.as_ref()), deadline)?;
        held.sync_all()?;
        self.verify_captured_inputs()?;
        let after = capture_file_state(&held.metadata()?)?;
        let named = capture_file_state(&fs::symlink_metadata(&self.path)?)?;
        if after != named || (after.0, after.1) != self.inode || after.2 > limits.max_staging_bytes
        {
            return Err(Error::Invalid("public D1 family successor custody"));
        }
        check_capture_active(Some(self.cancelled.as_ref()), deadline)?;
        {
            let mut seal = self
                .family_seal
                .lock()
                .map_err(|_| Error::Invalid("public D1 family seal poisoned"))?;
            if !matches!(&*seal,FamilyPreparationSeal::Preparing(owner) if *owner==std::thread::current().id())
            {
                return Err(Error::Invalid("public D1 family successor transition"));
            }
            *seal = FamilyPreparationSeal::Prepared(after);
        }
        // All fallible identity/source/cancel fences precede publishing the
        // prepared seal. No fallible step can escape with Prepared on Err.
        guard.complete = true;
        Ok(())
    }

    pub fn check_custody(&self) -> Result<()> {
        check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
        {
            let seal = self
                .family_seal
                .lock()
                .map_err(|_| Error::Invalid("public D1 family seal poisoned"))?;
            match &*seal {
                FamilyPreparationSeal::Failed => {
                    return Err(Error::Invalid("public D1 family preparation failed"));
                }
                FamilyPreparationSeal::Preparing(owner)
                    if *owner != std::thread::current().id() =>
                {
                    return Err(Error::Invalid("public D1 family preparation in progress"));
                }
                _ => {}
            }
        }
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.file_type().is_file() || (metadata.dev(), metadata.ino()) != self.inode {
            return Err(Error::Invalid("public D1 private capture replaced"));
        }
        self.charge_work(0)
    }

    pub(crate) fn capture_identity(&self) -> Result<(u64, u64, u64, i64, i64, i64, i64)> {
        self.check_custody()?;
        let metadata = fs::symlink_metadata(&self.path)?;
        let state = (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        );
        if !metadata.file_type().is_file() || state != self.expected_capture_file_state()? {
            return Err(Error::Invalid("public D1 private capture changed"));
        }
        Ok(state)
    }

    pub(crate) fn read_db(&self) -> Result<Connection> {
        self.check_custody()?;
        if let Some(heap) = &self.sqlite_heap {
            heap.verify_current()?;
        }
        let shared_window = if self.shared_vm {
            Some(sqlite_budget::SharedVmWindow::reserve(
                Arc::clone(&self.vm_used),
                self.active_vm_limit()?,
            )?)
        } else {
            None
        };
        let db = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        if let Some(window) = shared_window {
            window.install(&db, self.active_deadline()?, Arc::clone(&self.cancelled));
        } else {
            sqlite_budget::install_progress_until(
                &db,
                self.limits.sqlite(),
                Arc::clone(&self.vm_used),
                self.active_deadline()?,
            );
        }
        db.pragma_update(None, "cache_size", -(self.limits.sqlite_cache_kib as i64))?;
        Ok(db)
    }

    pub(crate) fn write_family_db(
        &self,
        held: &File,
    ) -> Result<tos_source_store::PinnedSqliteConnection> {
        let deadline = self.active_deadline()?;
        self.check_custody()?;
        {
            let seal = self
                .family_seal
                .lock()
                .map_err(|_| Error::Invalid("public D1 family seal poisoned"))?;
            if !matches!(&*seal,FamilyPreparationSeal::Preparing(owner) if *owner==std::thread::current().id())
            {
                return Err(Error::Invalid(
                    "public D1 family writer outside sanctioned transition",
                ));
            }
        }
        let before = capture_file_state(&held.metadata()?)?;
        if (before.0, before.1) != self.inode
            || before != capture_file_state(&fs::symlink_metadata(&self.path)?)?
        {
            return Err(Error::Invalid("public D1 family writer custody"));
        }
        if let Some(heap) = &self.sqlite_heap {
            heap.verify_current()?;
        }
        let shared_window = if self.shared_vm {
            Some(sqlite_budget::SharedVmWindow::reserve(
                Arc::clone(&self.vm_used),
                self.active_vm_limit()?,
            )?)
        } else {
            None
        };
        let db=tos_source_store::PinnedSqliteConnection::open_private_capture_for_family_preparation_with_setup(held, |db| {
            if let Some(window)=shared_window {
                window.install(db,deadline,Arc::clone(&self.cancelled));
            } else {
                sqlite_budget::install_progress_until(db,self.limits.sqlite(),Arc::clone(&self.vm_used),deadline);
            }
        }).map_err(|error| match error.code {
            tos_source_store::StoreErrorCode::BudgetExceeded => Error::Budget(error.detail),
            _ => Error::Invalid(error.detail),
        })?;
        db.pragma_update(None, "cache_size", -(self.limits.sqlite_cache_kib as i64))?;
        Ok(db)
    }

    pub(crate) fn write_db(&self) -> Result<Connection> {
        self.check_custody()?;
        if let Some(heap) = &self.sqlite_heap {
            heap.verify_current()?;
        }
        let shared_window = if self.shared_vm {
            Some(sqlite_budget::SharedVmWindow::reserve(
                Arc::clone(&self.vm_used),
                self.active_vm_limit()?,
            )?)
        } else {
            None
        };
        let db = Connection::open(&self.path)?;
        if let Some(window) = shared_window {
            window.install(&db, self.active_deadline()?, Arc::clone(&self.cancelled));
        } else {
            sqlite_budget::install_progress_until(
                &db,
                self.limits.sqlite(),
                Arc::clone(&self.vm_used),
                self.active_deadline()?,
            );
        }
        db.pragma_update(None, "cache_size", -(self.limits.sqlite_cache_kib as i64))?;
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE",
        )?;
        Ok(db)
    }

    pub fn verify_inputs(&self, limits: PublicCaptureLimits) -> Result<()> {
        check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
        self.check_custody()?;
        if self.prepared_profile && prepared_state(&self.root)? != self.prepared_state {
            return Err(Error::Invalid("prepared source state changed"));
        }
        if limits.max_work_bytes != self.max_work_bytes {
            return Err(Error::Invalid("public D1 changed work budget"));
        }
        for source in &self.sources {
            let SourceOrigin::File(path) = &source.origin else {
                continue;
            };
            if source.digest.is_none() {
                if path.exists() || path.is_symlink() {
                    return Err(Error::Invalid(
                        "public D1 optional source appeared during build",
                    ));
                }
                continue;
            }
            let mut file = profile_open(path, limits.max_input_bytes, self.prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
                self.charge_work(n as u64)
            })?;
            if len != source.len || Some(digest) != source.digest {
                return Err(Error::Invalid("public D1 source changed during build"));
            }
        }
        if !self.prepared_profile && !self.evidence_profile && self.runtime_capture_role.is_none() {
            let ledger = self
                .root
                .join("ToS/source-witnesses/access-requests/public-ledger");
            if self.sqlite_heap.is_some() {
                let names = if ledger.exists() {
                    if ledger.is_symlink() || !ledger.is_dir() {
                        return Err(Error::Invalid("public D1 ledger changed"));
                    }
                    controlled_ledger_names(&ledger, |bytes| self.charge_work(bytes as u64))?
                } else {
                    Vec::new()
                };
                let mut found = 0usize;
                for name in &names {
                    let name = name
                        .to_str()
                        .ok_or(Error::Invalid("public D1 ledger filename"))?;
                    if !name.ends_with(".access-request.json") {
                        continue;
                    }
                    if !self.sources.iter().any(|source| {
                        source
                            .label
                            .strip_prefix("ToS/source-witnesses/access-requests/public-ledger/")
                            == Some(name)
                    }) {
                        return Err(Error::Invalid("public D1 ledger membership changed"));
                    }
                    found += 1;
                }
                if found
                    != self
                        .sources
                        .iter()
                        .filter(|source| {
                            source
                                .label
                                .starts_with("ToS/source-witnesses/access-requests/public-ledger/")
                        })
                        .count()
                {
                    return Err(Error::Invalid("public D1 ledger membership changed"));
                }
            } else {
                let mut current = BTreeSet::new();
                if ledger.exists() {
                    if ledger.is_symlink() || !ledger.is_dir() {
                        return Err(Error::Invalid("public D1 ledger changed"));
                    }
                    for entry in std::fs::read_dir(&ledger)? {
                        check_capture_active(
                            Some(self.cancelled.as_ref()),
                            self.active_deadline()?,
                        )?;
                        if current.len() >= 4096 {
                            return Err(Error::Budget("public D1 ledger membership"));
                        }
                        let entry = entry?;
                        let name = entry.file_name();
                        self.charge_work(name.as_encoded_bytes().len() as u64)?;
                        let name = name
                            .to_str()
                            .ok_or(Error::Invalid("public D1 ledger filename"))?;
                        if name.ends_with(".access-request.json") {
                            current.insert(name.to_owned());
                        }
                    }
                }
                let captured = self
                    .sources
                    .iter()
                    .filter_map(|source| {
                        source
                            .label
                            .strip_prefix("ToS/source-witnesses/access-requests/public-ledger/")
                    })
                    .map(str::to_owned)
                    .collect::<BTreeSet<_>>();
                if current != captured {
                    return Err(Error::Invalid("public D1 ledger membership changed"));
                }
            }
        }
        let db = self.read_db()?;
        let mut statement =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let path = row
                .get_ref(0)?
                .as_str()
                .map_err(|_| Error::Invalid("public D1 SQL text column"))?;
            let digest = row
                .get_ref(1)?
                .as_blob()
                .map_err(|_| Error::Invalid("public D1 SQL blob column"))?;
            if self.sqlite_heap.is_some() && (path.len() > 8194 || digest.len() != 32) {
                return Err(Error::Budget("controlled captured source metadata"));
            }
            let len: u64 = row.get(2)?;
            let mut file = safe_open::open_regular(Path::new(&path), limits.max_input_bytes)?;
            let (actual, actual_len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)?;
                self.charge_work(n as u64)
            })?;
            if actual_len != len || actual.as_bytes().as_slice() != digest {
                return Err(Error::Invalid("public D1 part changed during build"));
            }
        }
        check_capture_active(Some(self.cancelled.as_ref()), self.active_deadline()?)
    }

    pub(crate) fn verify_captured_inputs(&self) -> Result<()> {
        self.verify_inputs(self.limits)
    }

    /// Read only an exact member already selected by this retained capture,
    /// including partition members. No ambient relative-path read is admitted.
    pub fn read_retained_input(&self, label: &str, cap: usize) -> Result<Vec<u8>> {
        self.check_custody()?;
        if self.prepared_software_input(label).is_some()
            || self
                .sources
                .iter()
                .any(|source| source.label == label && source.digest.is_some())
        {
            return self
                .read_input(label, cap)?
                .ok_or(Error::Invalid("captured runtime member absent"));
        }
        let path = self.retained_member_path(label)?;
        let db = self.read_db()?;
        let part: Option<(Vec<u8>, u64)> = db
            .query_row(
                "SELECT sha256,size_bytes FROM capture_sources WHERE path=?1",
                [path
                    .to_str()
                    .ok_or(Error::Invalid("captured runtime path UTF8"))?],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (expected, len) = part.ok_or(Error::Invalid("captured runtime member absent"))?;
        if len > cap as u64 {
            return Err(Error::Budget("captured runtime member bytes"));
        }
        self.charge_work(len)?;
        let file = safe_open::open_regular(&path, cap as u64)?;
        let mut raw = Vec::with_capacity(len as usize);
        file.take(cap as u64 + 1).read_to_end(&mut raw)?;
        if raw.len() as u64 != len
            || Digest256::of_bytes(&raw).as_bytes().as_slice() != expected.as_slice()
        {
            return Err(Error::Invalid("captured runtime part changed"));
        }
        Ok(raw)
    }

    // Validate the original aggregate closure without materializing a second
    // BTreeMap/Vec of all member names during controlled construction.
    fn retained_input_bytes_borrowed(&self, cap: u64) -> Result<u64> {
        self.check_custody()?;
        let mut total = 0u64;
        for source in &self.sources {
            if source.digest.is_some() {
                total = total
                    .checked_add(source.len)
                    .filter(|n| *n <= cap)
                    .ok_or(Error::Budget("runtime capture aggregate source bytes"))?;
            }
        }
        let db = self.read_db()?;
        let mut statement =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = statement.query([])?;
        let mut count = 0usize;
        while let Some(row) = rows.next()? {
            count = count
                .checked_add(1)
                .filter(|n| *n <= 65536)
                .ok_or(Error::Budget("captured runtime member count"))?;
            let path_ref = row.get_ref(0)?;
            let path = path_ref
                .as_str()
                .map_err(|_| Error::Invalid("captured runtime member UTF8"))?;
            let digest_ref = row.get_ref(1)?;
            let digest = digest_ref
                .as_blob()
                .map_err(|_| Error::Invalid("captured runtime member digest"))?;
            if digest.len() != 32 {
                return Err(Error::Invalid("captured runtime member digest"));
            }
            let len: u64 = row.get(2)?;
            self.charge_work(path.len() as u64 + 40)?;
            let member_path = Path::new(path);
            let relative = member_path
                .strip_prefix(&self.root)
                .unwrap_or(member_path)
                .to_str()
                .ok_or(Error::Invalid("captured runtime member UTF8"))?;
            if let Some(source) = self
                .sources
                .iter()
                .find(|source| source.label == relative && source.digest.is_some())
            {
                if source.len != len
                    || source.digest.as_ref().map(|d| d.as_bytes().as_slice()) != Some(digest)
                {
                    return Err(Error::Invalid("captured runtime duplicate member differs"));
                }
            } else {
                total = total
                    .checked_add(len)
                    .filter(|n| *n <= cap)
                    .ok_or(Error::Budget("runtime capture aggregate source bytes"))?;
            }
        }
        Ok(total)
    }

    /// Exact retained closure from the same capture, including authenticated
    /// partition members and typed compiled companions. Optional absence stays
    /// absent; no source tree enumeration is used.
    pub fn retained_input_members(&self) -> Result<Vec<(String, Digest256, u64)>> {
        self.check_custody()?;
        let mut members = BTreeMap::new();
        for source in &self.sources {
            if let Some(digest) = source.digest {
                members.insert(source.label.clone(), (digest, source.len));
            }
        }
        let db = self.read_db()?;
        let mut statement =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            if members.len() >= 65536 {
                return Err(Error::Budget("captured runtime member count"));
            }
            let path: String = row.get(0)?;
            let raw_sha: Vec<u8> = row.get(1)?;
            let len: u64 = row.get(2)?;
            self.charge_work(path.len() as u64 + 40)?;
            let member_path = Path::new(&path);
            let relative = member_path
                .strip_prefix(&self.root)
                .unwrap_or(member_path)
                .to_str()
                .ok_or(Error::Invalid("captured runtime member UTF8"))?
                .to_owned();
            let bytes: [u8; 32] = raw_sha
                .try_into()
                .map_err(|_| Error::Invalid("captured runtime member digest"))?;
            let sha = Digest256::from_bytes(bytes);
            if let Some(previous) = members.insert(relative, (sha, len)) {
                if previous != (sha, len) {
                    return Err(Error::Invalid("captured runtime duplicate member differs"));
                }
            }
        }
        Ok(members
            .into_iter()
            .map(|(path, (sha, len))| (path, sha, len))
            .collect())
    }

    pub(crate) fn retained_input_members_owned(
        &self,
        state: &CreationState<'_>,
    ) -> Result<Vec<(String, Digest256, u64)>> {
        self.check_custody()?;
        let mut members = BTreeMap::<String, (Digest256, u64)>::new();
        let node = 11 * std::mem::size_of::<(String, (Digest256, u64))>()
            + 16 * std::mem::size_of::<usize>();
        for source in &self.sources {
            if let Some(digest) = source.digest {
                if let Some(existing) = members.get_mut(&source.label) {
                    *existing = (digest, source.len);
                } else {
                    state.retain(
                        node.checked_add(source.label.len())
                            .ok_or(Error::Budget("owned retained member state"))?,
                    )?;
                    members.insert(source.label.clone(), (digest, source.len));
                }
            }
        }
        let _stmt_hold = state.hold(
            tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(
            ),
        )?;
        let db = self.read_db()?;
        let mut stmt =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            state.active()?;
            if members.len() >= 65536 {
                return Err(Error::Budget("captured runtime member count"));
            }
            let path = row
                .get_ref(0)?
                .as_str()
                .map_err(|_| Error::Invalid("public D1 SQL text column"))?;
            let digest = row
                .get_ref(1)?
                .as_blob()
                .map_err(|_| Error::Invalid("public D1 SQL blob column"))?;
            let bytes: u64 = row.get(2)?;
            self.charge_work(path.len() as u64 + 40)?;
            let relative = Path::new(path)
                .strip_prefix(&self.root)
                .unwrap_or(Path::new(path))
                .to_str()
                .ok_or(Error::Invalid("captured runtime member UTF8"))?;
            let raw: [u8; 32] = digest
                .try_into()
                .map_err(|_| Error::Invalid("captured runtime member digest"))?;
            let digest = Digest256::from_bytes(raw);
            if let Some(previous) = members.get(relative) {
                if *previous != (digest, bytes) {
                    return Err(Error::Invalid("captured runtime duplicate member differs"));
                }
            } else {
                state.retain(
                    node.checked_add(relative.len())
                        .ok_or(Error::Budget("owned retained member state"))?,
                )?;
                members.insert(relative.to_owned(), (digest, bytes));
            }
        }
        state.retain(
            members
                .len()
                .checked_mul(std::mem::size_of::<(String, Digest256, u64)>())
                .ok_or(Error::Budget("owned retained member output slots"))?,
        )?;
        let mut output = Vec::with_capacity(members.len());
        output.extend(
            members
                .into_iter()
                .map(|(path, (digest, bytes))| (path, digest, bytes)),
        );
        Ok(output)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Capture the exact five-file physical source-state tuple used by the
    /// public Core graph API. The digest fence is checked on both sides of
    /// stat collection so this tuple cannot describe a different input cut.
    pub(crate) fn core_source_state_owned(
        &self,
        state: &CreationState<'_>,
    ) -> Result<Vec<(String, i64, u64, u64, i64)>> {
        if self.runtime_capture_role.is_some() {
            return Err(Error::Invalid(
                "whole Core source state requires all five inputs",
            ));
        }
        type StateRow = (String, i64, u64, u64, i64);
        let returned = 5usize
            .checked_mul(std::mem::size_of::<StateRow>() + 8194)
            .ok_or(Error::Budget("owned Core source-state result"))?;
        state.retain(returned)?;
        let _workspace = state.hold(
            returned
                .checked_add(3 * 8194)
                .and_then(|n| n.checked_add(controlled_fence_workspace_upper(true)))
                .ok_or(Error::Budget("owned Core source-state workspace"))?,
        )?;
        self.core_source_state()
    }

    pub fn core_source_state(&self) -> Result<Vec<(String, i64, u64, u64, i64)>> {
        if self.runtime_capture_role.is_some() {
            return Err(Error::Invalid(
                "whole Core source state requires all five inputs",
            ));
        }
        fn collect(capture: &PublicCapture) -> Result<Vec<(String, i64, u64, u64, i64)>> {
            let mut states = Vec::with_capacity(5);
            for label in [
                "ToS/derived-exports/tos_corpus_index.min.json",
                "ToS/derived-exports/philosophy_graph_projection.min.json",
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
                "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            ] {
                let source = capture
                    .sources
                    .iter()
                    .find(|source| source.label == label && source.digest.is_some())
                    .ok_or(Error::Invalid("public D1 core source-state member"))?;
                let SourceOrigin::File(path) = &source.origin else {
                    return Err(Error::Invalid("public D1 core source-state origin"));
                };
                let resolved = fs::canonicalize(path)?;
                let metadata = fs::metadata(path)?;
                let mtime_ns = metadata
                    .mtime()
                    .checked_mul(1_000_000_000)
                    .and_then(|value| value.checked_add(metadata.mtime_nsec()))
                    .ok_or(Error::Budget("public D1 source mtime"))?;
                let ctime_ns = metadata
                    .ctime()
                    .checked_mul(1_000_000_000)
                    .and_then(|value| value.checked_add(metadata.ctime_nsec()))
                    .ok_or(Error::Budget("public D1 source ctime"))?;
                if metadata.len() != source.len {
                    return Err(Error::Invalid("public D1 source-state size changed"));
                }
                states.push((
                    resolved
                        .to_str()
                        .ok_or(Error::Invalid("public D1 source-state path UTF8"))?
                        .to_owned(),
                    mtime_ns,
                    metadata.len(),
                    metadata.ino(),
                    ctime_ns,
                ));
            }
            Ok(states)
        }
        let before = collect(self)?;
        self.verify_inputs(self.limits)?;
        let after = collect(self)?;
        if before != after {
            return Err(Error::Invalid("public D1 core source state changed"));
        }
        Ok(after)
    }

    /// Physical identity of the complete selected capture closure, including
    /// partition members which do not appear in Reference Core's five-path
    /// public `source_state` tuple. The returned order is canonical absolute
    /// path order; a source edit may change metadata while preserving this
    /// path set, but adding/removing a captured member changes the vector.
    pub(crate) fn capture_source_state_owned(
        &self,
        budget: &CreationState<'_>,
    ) -> Result<Vec<(String, i64, u64, u64, i64)>> {
        self.verify_inputs(self.limits)?;
        let db = self.read_db()?;
        let (row_count, path_bytes): (u64, u64) = db.query_row(
            "SELECT count(*),coalesce(sum(length(CAST(path AS BLOB))),0) FROM capture_sources",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let source_bytes = self.sources.iter().try_fold(0usize, |total, source| {
            if source.digest.is_some()
                && let SourceOrigin::File(path) = &source.origin
            {
                total
                    .checked_add(path.as_os_str().len())
                    .ok_or(Error::Budget("owned source path bytes"))
            } else {
                Ok(total)
            }
        })?;
        let path_bytes = usize::try_from(path_bytes)
            .ok()
            .and_then(|n| n.checked_add(source_bytes))
            .ok_or(Error::Budget("owned source-state path bytes"))?;
        let maximum = usize::try_from(row_count)
            .ok()
            .and_then(|n| n.checked_add(self.sources.len()))
            .ok_or(Error::Budget("owned capture source-state rows"))?;
        type StateRow = (String, i64, u64, u64, i64);
        budget.retain(
            maximum
                .checked_mul(std::mem::size_of::<StateRow>())
                .ok_or(Error::Budget("owned capture source-state result"))?,
        )?;
        let _paths_hold = budget.hold(
            maximum
                .checked_mul(2 * std::mem::size_of::<PathBuf>())
                .and_then(|n| n.checked_add(path_bytes))
                .and_then(|n| n.checked_add(controlled_fence_workspace_upper(true)))
                .ok_or(Error::Budget("owned capture source-state workspace"))?,
        )?;
        let mut paths = Vec::with_capacity(maximum);
        for source in &self.sources {
            if source.digest.is_some()
                && let SourceOrigin::File(path) = &source.origin
            {
                if path.as_os_str().len() > 8194 {
                    return Err(Error::Budget("owned capture source path"));
                }
                paths.push(path.clone());
            }
        }
        let mut stmt = db.prepare("SELECT path FROM capture_sources ORDER BY path")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            budget.active()?;
            let path = row
                .get_ref(0)?
                .as_str()
                .map_err(|_| Error::Invalid("public D1 SQL text column"))?;
            if paths.len() >= maximum || path.len() > 8194 {
                return Err(Error::Budget("owned capture source paths"));
            }
            self.charge_work(path.len() as u64)?;
            paths.push(PathBuf::from(path));
        }
        drop(rows);
        drop(stmt);
        drop(db);
        budget.charge_work(
            paths
                .len()
                .checked_mul(std::mem::size_of::<PathBuf>())
                .ok_or(Error::Budget("owned source-state sort work"))?,
        )?;
        paths.sort();
        paths.dedup();
        let mut output = Vec::with_capacity(maximum);
        for path in paths {
            budget.active()?;
            let resolved = fs::canonicalize(&path)?;
            let metadata = fs::metadata(&path)?;
            let name = resolved
                .to_str()
                .ok_or(Error::Invalid("public D1 capture source path UTF8"))?;
            if name.len() > 8194 {
                return Err(Error::Budget("owned resolved source path"));
            }
            let mtime = metadata
                .mtime()
                .checked_mul(1_000_000_000)
                .and_then(|n| n.checked_add(metadata.mtime_nsec()))
                .ok_or(Error::Budget("public D1 capture source mtime"))?;
            let ctime = metadata
                .ctime()
                .checked_mul(1_000_000_000)
                .and_then(|n| n.checked_add(metadata.ctime_nsec()))
                .ok_or(Error::Budget("public D1 capture source ctime"))?;
            budget.retain(name.len())?;
            output.push((
                name.to_owned(),
                mtime,
                metadata.len(),
                metadata.ino(),
                ctime,
            ));
        }
        budget.charge_work(
            output
                .len()
                .checked_mul(std::mem::size_of::<StateRow>())
                .ok_or(Error::Budget("owned source-state final sort work"))?,
        )?;
        output.sort_by(|a, b| a.0.cmp(&b.0));
        self.verify_inputs(self.limits)?;
        Ok(output)
    }

    pub fn capture_source_state(&self) -> Result<Vec<(String, i64, u64, u64, i64)>> {
        self.verify_inputs(self.limits)?;
        let mut paths = BTreeSet::new();
        for source in &self.sources {
            if source.digest.is_some()
                && let SourceOrigin::File(path) = &source.origin
            {
                paths.insert(path.clone());
            }
        }
        let db = self.read_db()?;
        let mut statement = db.prepare("SELECT path FROM capture_sources ORDER BY path")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            self.charge_work(path.len() as u64)?;
            paths.insert(PathBuf::from(path));
        }
        let mut state = Vec::with_capacity(paths.len());
        for path in paths {
            let resolved = fs::canonicalize(&path)?;
            let metadata = fs::metadata(&path)?;
            let mtime_ns = metadata
                .mtime()
                .checked_mul(1_000_000_000)
                .and_then(|value| value.checked_add(metadata.mtime_nsec()))
                .ok_or(Error::Budget("public D1 capture source mtime"))?;
            let ctime_ns = metadata
                .ctime()
                .checked_mul(1_000_000_000)
                .and_then(|value| value.checked_add(metadata.ctime_nsec()))
                .ok_or(Error::Budget("public D1 capture source ctime"))?;
            state.push((
                resolved
                    .to_str()
                    .ok_or(Error::Invalid("public D1 capture source path UTF8"))?
                    .to_owned(),
                mtime_ns,
                metadata.len(),
                metadata.ino(),
                ctime_ns,
            ));
        }
        state.sort_by(|left, right| left.0.cmp(&right.0));
        self.verify_inputs(self.limits)?;
        Ok(state)
    }

    pub fn source_digest(&self, label: &str) -> Result<Digest256> {
        self.sources
            .iter()
            .find(|source| source.label == label)
            .and_then(|source| source.digest)
            .ok_or(Error::Invalid("public D1 source digest absent"))
    }

    pub(crate) fn source_labels(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|source| source.digest.is_some())
            .map(|source| source.label.as_str())
            .collect()
    }

    pub(crate) fn public_ledger_labels(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|source| {
                source.digest.is_some()
                    && source
                        .label
                        .starts_with("ToS/source-witnesses/access-requests/public-ledger/")
                    && source.label.ends_with(".access-request.json")
            })
            .map(|source| source.label.as_str())
            .collect()
    }

    /// Measured public inputs for the existing completion manifest. Part rows
    /// were admitted by the source-owned partition descriptors during capture;
    /// only their bounded root is exported, never private absolute paths.
    /// Verify the existing public D1 completion pair against this captured
    /// source. This grants no selected reader authority and creates no output.
    /// Call before and after the local comparison using the same capture.
    pub fn verify_public_completion(
        &self,
        runtime: &Path,
        dist: &Path,
        limits: PublicCaptureLimits,
        manifest_bytes: usize,
    ) -> Result<(serde_json::Value, Vec<u8>)> {
        use std::io::Read;
        self.verify_inputs(limits)?;
        if manifest_bytes == 0 || manifest_bytes > MAX_HEADER_BYTES + 1 {
            return Err(Error::Budget("public D1 verifier manifest bytes"));
        }
        let read_marker = |path: &Path| -> Result<Vec<u8>> {
            let mut file = tos_fd_open::open_absolute_regular(path, manifest_bytes as u64)
                .map_err(|_| Error::Invalid("public D1 completion marker"))?;
            let size = file.metadata()?.len();
            if size == 0 || size > manifest_bytes as u64 {
                return Err(Error::Budget("public D1 completion marker bytes"));
            }
            self.charge_work(
                size.checked_mul(3)
                    .ok_or(Error::Budget("public D1 verifier work"))?,
            )?;
            let mut raw = Vec::with_capacity(size as usize);
            std::io::Read::by_ref(&mut file)
                .take(size + 1)
                .read_to_end(&mut raw)?;
            if raw.len() as u64 != size {
                return Err(Error::Invalid("public D1 completion marker changed"));
            }
            Ok(raw)
        };
        let raw = read_marker(&runtime.join("manifest.json"))?;
        if raw != read_marker(&dist.join("__edge/build-manifest.json"))? {
            return Err(Error::Invalid("public D1 completion markers differ"));
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?;
        let revision = manifest["data_revision"]
            .as_str()
            .filter(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            .ok_or(Error::Invalid("public D1 completion revision"))?;
        if manifest["schema"] != "tos_cloudflare_edge_build_v1"
            || manifest["read_model_schema"] != "tos_cloudflare_edge_read_model_v9"
        {
            return Err(Error::Invalid("public D1 completion schema"));
        }
        for (name, field) in [
            ("read-model.sql", "sql_bytes"),
            ("read-model.rows.json", "baseline_bytes"),
        ] {
            let size = manifest[field]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or(Error::Invalid("public D1 completed file size"))?;
            let file = tos_fd_open::open_absolute_regular(&runtime.join(name), size)
                .map_err(|_| Error::Invalid("public D1 completed file"))?;
            if file.metadata()?.len() != size {
                return Err(Error::Invalid("public D1 completed file size"));
            }
            if name == "read-model.rows.json" {
                let mut prefix = Vec::with_capacity(160);
                (&file).take(160).read_to_end(&mut prefix)?;
                let count = prefix.len();
                let expected = format!(
                    "{{\"schema\":\"tos_cloudflare_edge_read_model_v9\",\"revision\":\"{revision}\""
                );
                if !prefix.starts_with(expected.as_bytes()) {
                    return Err(Error::Invalid("public D1 baseline revision"));
                }
                self.charge_work(count as u64)?;
            }
        }
        if manifest["public_input_binding"] != self.manifest_input_binding(manifest_bytes)? {
            return Err(Error::Invalid("public D1 measured input binding differs"));
        }
        self.check_custody()?;
        Ok((manifest, raw))
    }

    pub(crate) fn manifest_input_binding(&self, max_bytes: usize) -> Result<serde_json::Value> {
        #[derive(serde::Serialize)]
        struct Entry<'a> {
            path: &'a str,
            size_bytes: Option<u64>,
            sha256: Option<String>,
        }
        struct Count {
            len: usize,
            max: usize,
            exceeded: bool,
        }
        impl Write for Count {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let Some(next) = self.len.checked_add(bytes.len()).filter(|n| *n <= self.max)
                else {
                    self.exceeded = true;
                    return Err(io::Error::other("public D1 manifest input binding bytes"));
                };
                self.len = next;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        self.check_custody()?;
        if max_bytes == 0 || max_bytes > MAX_HEADER_BYTES {
            return Err(Error::Budget("public D1 manifest input binding bytes"));
        }
        const PREFIX: &[u8] = b"{\"sources\":[";
        let pointer_bytes = self
            .sources
            .len()
            .checked_mul(std::mem::size_of::<&SourceFile>())
            .ok_or(Error::Budget("public D1 manifest input state"))?;
        if PREFIX
            .len()
            .checked_add(self.sources.len().saturating_mul(4))
            .is_none_or(|minimum| minimum > max_bytes)
        {
            return Err(Error::Budget("public D1 manifest input binding bytes"));
        }
        self.charge_work(pointer_bytes as u64)?;
        let mut sources = self.sources.iter().collect::<Vec<_>>();
        sources.sort_by(|a, b| a.label.cmp(&b.label));
        if sources
            .windows(2)
            .any(|pair| pair[0].label == pair[1].label)
        {
            return Err(Error::Invalid("public D1 duplicate input label"));
        }
        let mut count = Count {
            len: PREFIX.len(),
            max: max_bytes,
            exceeded: false,
        };
        let mut ledger = Digest256Hasher::new();
        ledger.update(b"tos-public-ledger-membership-v1\0");
        let mut ledger_count = 0u64;
        for (index, source) in sources.iter().enumerate() {
            self.charge_work(source.label.len() as u64 + 64)?;
            if source.digest.is_some()
                && source
                    .label
                    .starts_with("ToS/source-witnesses/access-requests/public-ledger/")
                && source.label.ends_with(".access-request.json")
            {
                ledger.update(&(source.label.len() as u64).to_be_bytes());
                ledger.update(source.label.as_bytes());
                ledger_count = ledger_count
                    .checked_add(1)
                    .ok_or(Error::Budget("public D1 ledger membership count"))?;
            }
            if index != 0 {
                count
                    .write_all(b",")
                    .map_err(|_| Error::Budget("public D1 manifest input binding bytes"))?;
            }
            let entry = Entry {
                path: &source.label,
                size_bytes: source.digest.map(|_| source.len),
                sha256: source.digest.map(|digest| digest.to_hex()),
            };
            serde_json::to_writer(&mut count, &entry).map_err(|e| {
                if count.exceeded {
                    Error::Budget("public D1 manifest input binding bytes")
                } else {
                    Error::Source(e.to_string())
                }
            })?;
        }
        let db = self.read_db()?;
        let mut stmt =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = stmt.query([])?;
        let mut parts = Digest256Hasher::new();
        parts.update(b"tos-public-part-closure-v1\0");
        let mut part_count = 0u64;
        while let Some(row) = rows.next()? {
            let absolute: String = row.get(0)?;
            let sha: Vec<u8> = row.get(1)?;
            let size: i64 = row.get(2)?;
            let label = Path::new(&absolute)
                .strip_prefix(&self.root)
                .ok()
                .and_then(Path::to_str)
                .filter(|path| !path.is_empty())
                .ok_or(Error::Invalid("public D1 part logical path"))?;
            if sha.len() != 32 || size < 0 {
                return Err(Error::Invalid("public D1 part binding row"));
            }
            self.charge_work(label.len() as u64 + sha.len() as u64 + 16)?;
            parts.update(&(label.len() as u64).to_be_bytes());
            parts.update(label.as_bytes());
            parts.update(&(size as u64).to_be_bytes());
            parts.update(&sha);
            part_count = part_count
                .checked_add(1)
                .ok_or(Error::Budget("public D1 part binding count"))?;
        }
        let suffix = serde_json::json!({
            "schema":"tos_public_input_binding_v1",
            "public_ledger":{"count":ledger_count,"paths_sha256":ledger.finalize().to_hex()},
            "partitioned":self.partitioned,
            "partition_parts":{"count":part_count,"root_sha256":parts.finalize().to_hex()},
        });
        let suffix = serde_json::to_vec(&suffix).map_err(|e| Error::Source(e.to_string()))?;
        for piece in [
            b"],".as_slice(),
            &suffix[1..suffix.len() - 1],
            b"}".as_slice(),
        ] {
            count
                .write_all(piece)
                .map_err(|_| Error::Budget("public D1 manifest input binding bytes"))?;
        }
        // One exact count admits the raw bytes, bounded parsed Value state and
        // both serialization passes before any growing buffer is allocated.
        let resident = (count.len as u64)
            .checked_mul(4)
            .and_then(|n| n.checked_add((sources.len() as u64).checked_mul(512)?))
            .ok_or(Error::Budget("public D1 manifest input state"))?;
        self.charge_work(resident)?;
        let mut raw = Vec::with_capacity(count.len);
        raw.extend_from_slice(PREFIX);
        for (index, source) in sources.iter().enumerate() {
            if index != 0 {
                raw.push(b',');
            }
            let entry = Entry {
                path: &source.label,
                size_bytes: source.digest.map(|_| source.len),
                sha256: source.digest.map(|digest| digest.to_hex()),
            };
            serde_json::to_writer(&mut raw, &entry).map_err(|e| Error::Source(e.to_string()))?;
        }
        raw.extend_from_slice(b"],");
        raw.extend_from_slice(&suffix[1..suffix.len() - 1]);
        raw.push(b'}');
        if raw.len() != count.len {
            return Err(Error::Invalid("public D1 manifest input serialized size"));
        }
        serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))
    }

    /// Bind the complete allowlisted public snapshot by stable logical labels.
    /// These digests were measured on capture and are independently rechecked
    /// before completion; transient private paths never enter the revision.
    pub(crate) fn update_revision_sources(&self, hash: &mut Digest256Hasher) -> Result<()> {
        self.check_custody()?;
        for source in &self.sources {
            self.charge_work(source.label.len() as u64 + 41)?;
            hash.update(&(source.label.len() as u64).to_be_bytes());
            hash.update(source.label.as_bytes());
            hash.update(&source.len.to_be_bytes());
            if let Some(digest) = source.digest {
                hash.update(&[1]);
                hash.update(digest.as_bytes());
            } else {
                hash.update(&[0]);
            }
        }
        Ok(())
    }

    pub fn partitioned(&self) -> bool {
        self.partitioned
    }

    fn prepared_software_input(&self, label: &str) -> Option<&'static [u8]> {
        if !self.prepared_profile {
            return None;
        }
        match label {
            "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json" => Some(include_bytes!(
                "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
            )),
            "ToS/contracts/semantic-entity-type-registry.schema.json" => Some(include_bytes!(
                "../../../../ToS/contracts/semantic-entity-type-registry.schema.json"
            )),
            "ToS/contracts/semantic-relation-type-registry.schema.json" => Some(include_bytes!(
                "../../../../ToS/contracts/semantic-relation-type-registry.schema.json"
            )),
            _ => None,
        }
    }

    pub fn read_input(&self, label: &str, cap: usize) -> Result<Option<Vec<u8>>> {
        if let Some(raw) = self.prepared_software_input(label) {
            if raw.len() > cap {
                return Err(Error::Budget("prepared software companion bytes"));
            }
            self.charge_work(raw.len() as u64)?;
            return Ok(Some(raw.to_vec()));
        }
        self.check_custody()?;
        let source = self
            .sources
            .iter()
            .find(|source| source.label == label)
            .ok_or(Error::Invalid("public D1 input outside exact closure"))?;
        let Some(expected) = source.digest else {
            return Ok(None);
        };
        if source.len > cap as u64 {
            return Err(Error::Budget("public D1 input bytes"));
        }
        self.charge_work(source.len)?;
        let mut file = match &source.origin {
            SourceOrigin::Compiled(raw) => return Ok(Some(raw.to_vec())),
            SourceOrigin::File(path) => profile_open(path, cap as u64, self.prepared_profile)?,
        };
        let mut raw = Vec::with_capacity(source.len as usize);
        file.take(cap as u64 + 1).read_to_end(&mut raw)?;
        if raw.len() as u64 != source.len || Digest256::of_bytes(&raw) != expected {
            return Err(Error::Invalid("public D1 input changed"));
        }
        Ok(Some(raw))
    }

    /// The maintained partitioned compiler binds exactly five logical roots;
    /// part hashes are recursively committed by each manifest root. This is
    /// a source revision, not a publication or selected-current receipt.
    pub fn partitioned_source_revision(&self) -> Result<String> {
        if !self.partitioned {
            return Err(Error::Invalid("public D1 partitioned revision mode"));
        }
        let mut bindings = serde_json::Map::new();
        for label in [
            "ToS/derived-exports/tos_corpus_index.min.json",
            "ToS/derived-exports/philosophy_graph_projection.min.json",
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        ] {
            let source = self
                .sources
                .iter()
                .find(|source| source.label == label)
                .ok_or(Error::Invalid("public D1 revision input"))?;
            let digest = source
                .digest
                .ok_or(Error::Invalid("public D1 revision missing input"))?;
            bindings.insert(label.to_owned(), serde_json::Value::String(digest.to_hex()));
        }
        let value =
            serde_json::json!({"compiler":"tos_offline_knowledge_v2","snapshot_bindings":bindings});
        let raw = serde_json::to_vec(&value).map_err(|e| Error::Source(e.to_string()))?;
        let value = json(&raw, MAX_HEADER_BYTES)?;
        let canonical = tos_foundation::canonical_bytes_v1(
            &value,
            tos_foundation::CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::new(MAX_HEADER_BYTES, 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("public D1 revision JSON"))?,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        Ok(Digest256::of_bytes(&canonical).to_hex())
    }

    fn stable_role(&self, hash: &mut Digest256Hasher, role: &str, prefix: &str) -> Result<()> {
        use crate::knowledge_normalization::{stable_digest_value, write_len, write_string};
        let db = self.read_db()?;
        let mut keys = BTreeSet::new();
        let mut stmt =
            db.prepare("SELECT path FROM capture_headers WHERE role=?1 ORDER BY path")?;
        for item in stmt.query_map([role], |row| row.get::<_, String>(0))? {
            let item = item?;
            self.charge_work(item.len() as u64)?;
            if prefix.is_empty() {
                if let Some((root, _)) = item.split_once('/') {
                    keys.insert(root.to_owned());
                } else {
                    keys.insert(item);
                }
            } else if let Some(nested) = item
                .strip_prefix(prefix)
                .and_then(|path| path.strip_prefix('/'))
            {
                keys.insert(nested.to_owned());
            }
        }
        let mut stmt = db.prepare(
            "SELECT collection FROM capture_collections WHERE role=?1 ORDER BY collection",
        )?;
        for item in stmt.query_map([role], |row| row.get::<_, String>(0))? {
            let item = item?;
            self.charge_work(item.len() as u64)?;
            if prefix.is_empty() {
                if let Some((root, _)) = item.split_once('/') {
                    keys.insert(root.to_owned());
                } else {
                    keys.insert(item);
                }
            } else if let Some(nested) = item
                .strip_prefix(prefix)
                .and_then(|path| path.strip_prefix('/'))
            {
                keys.insert(nested.to_owned());
            }
        }
        hash.update(b"o");
        write_len(hash, keys.len());
        hash.update(b"{");
        for key in keys {
            write_string(hash, &key);
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}/{key}")
            };
            if path == "source_navigation" {
                self.stable_role(hash, role, "source_navigation")?;
                continue;
            }
            let header: Option<Vec<u8>> = db
                .query_row(
                    "SELECT json FROM capture_headers WHERE role=?1 AND path=?2",
                    params![role, path],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(raw) = header {
                self.charge_work(raw.len() as u64)?;
                let value: serde_json::Value = strict_value(&raw, MAX_HEADER_BYTES)?;
                stable_digest_value(&value, hash)?;
                continue;
            }
            let count: u64 = db.query_row(
                "SELECT count(*) FROM capture_rows WHERE role=?1 AND collection=?2",
                params![role, path],
                |row| row.get(0),
            )?;
            hash.update(b"a");
            write_len(hash, count as usize);
            hash.update(b"[");
            let actual = self.visit_rows(role, &path, |_, raw| {
                let value: serde_json::Value = strict_value(raw, MAX_ROW_BYTES)?;
                stable_digest_value(&value, hash)
            })?;
            if actual != count {
                return Err(Error::Invalid("public D1 stable revision rows"));
            }
            hash.update(b"]");
        }
        hash.update(b"}");
        Ok(())
    }

    /// The legacy public graph uses Python's full logical-value stable digest.
    /// Emit its container framing over the captured disk rows in owner order,
    /// never rebuilding the 61/30/10 MiB objects in memory.
    pub fn legacy_source_revision(&self) -> Result<String> {
        if self.partitioned {
            return Err(Error::Invalid("public D1 legacy revision mode"));
        }
        self.core_source_revision()
    }

    /// Python Core's exact whole-source revision framing, computed over the
    /// retained logical role documents even when physical rows came from
    /// partitioned projection members.
    pub fn core_source_revision(&self) -> Result<String> {
        if self.runtime_capture_role.is_some() {
            return Err(Error::Invalid(
                "whole Core revision requires all five inputs",
            ));
        }
        use crate::knowledge_normalization::{stable_digest_value, write_len, write_string};
        let mut hash = Digest256Hasher::new();
        hash.update(b"o");
        write_len(&mut hash, 5);
        hash.update(b"{");
        for (name, role) in [
            ("bibliographic_claims", Some(CLAIMS)),
            ("corpus", Some(CORPUS)),
            ("entity_type_registry", None),
            ("philosophy", Some(PHILOSOPHY)),
            ("relation_type_registry", None),
        ] {
            write_string(&mut hash, name);
            match role {
                Some(role) => self.stable_role(&mut hash, role, "")?,
                None => {
                    let label = if name == "entity_type_registry" {
                        "ToS/doctrine/semantic-interchange/entity-types.v1.json"
                    } else {
                        "ToS/doctrine/semantic-interchange/relation-types.v1.json"
                    };
                    let raw = self
                        .read_input(label, 4 * 1024 * 1024)?
                        .ok_or(Error::Invalid("public D1 missing registry"))?;
                    let value: serde_json::Value = strict_value(&raw, 4 * 1024 * 1024)?;
                    stable_digest_value(&value, &mut hash)?;
                }
            }
        }
        hash.update(b"}");
        Ok(hash.finalize().to_hex())
    }

    pub fn header(&self, role: &str, path: &str) -> Result<JsonValue> {
        self.check_custody()?;
        let db = self.read_db()?;
        let raw: Vec<u8> = db.query_row(
            "SELECT json FROM capture_headers WHERE role=?1 AND path=?2",
            params![role, path],
            |row| row.get(0),
        )?;
        self.charge_work(raw.len() as u64)?;
        json(&raw, MAX_HEADER_BYTES)
    }

    /// Reconstruct only the bounded non-row header of one captured projection.
    /// Collection values stay in the disk index and never enter this object.
    pub(crate) fn header_object(
        &self,
        role: &str,
        prefix: &str,
        max_bytes: usize,
    ) -> Result<serde_json::Value> {
        self.check_custody()?;
        // The caller supplies an envelope ceiling; the header's own row and
        // aggregate ceiling remains in force even when that envelope is larger.
        let max_bytes = max_bytes.min(MAX_HEADER_BYTES);
        if max_bytes == 0 {
            return Err(Error::Budget("public D1 header object bytes"));
        }
        let db = self.read_db()?;
        let mut statement = db.prepare(
            "SELECT path,length(json),CASE WHEN length(json) <= ?2 THEN json END
               FROM capture_headers WHERE role=?1 ORDER BY path",
        )?;
        let mut rows = statement.query(params![role, max_bytes as i64])?;
        let mut fields = serde_json::Map::new();
        let mut total = 2usize;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let dynamic_philosophy = self.runtime_capture_role
                == Some(RuntimeCaptureRole::Philosophy)
                && role == PHILOSOPHY;
            let Some(name) = (if prefix.is_empty() {
                (dynamic_philosophy || !path.contains('/')).then_some(path.as_str())
            } else {
                path.strip_prefix(prefix)
                    .and_then(|name| name.strip_prefix('/'))
            }) else {
                continue;
            };
            if name.is_empty() || (!dynamic_philosophy && name.contains('/')) {
                return Err(Error::Invalid("public D1 nested header path"));
            }
            let declared: i64 = row.get(1)?;
            if declared <= 0 || declared as usize > max_bytes {
                return Err(Error::Budget("public D1 header object bytes"));
            }
            // Bound the JSON representation before SQLite copies the BLOB or
            // the serde map allocates its decoded subtree. Six bytes per key
            // byte is a conservative upper bound for JSON escaping.
            let field_bytes = name
                .len()
                .checked_mul(6)
                .and_then(|bytes| bytes.checked_add(3))
                .and_then(|bytes| bytes.checked_add(declared as usize))
                .and_then(|bytes| bytes.checked_add(usize::from(!fields.is_empty())))
                .ok_or(Error::Budget("public D1 header object bytes"))?;
            total = total
                .checked_add(field_bytes)
                .filter(|total| *total <= max_bytes)
                .ok_or(Error::Budget("public D1 header object bytes"))?;
            let raw: Option<Vec<u8>> = row.get(2)?;
            let raw = raw.ok_or(Error::Budget("public D1 header object bytes"))?;
            if raw.len() != declared as usize {
                return Err(Error::Invalid("public D1 header length changed"));
            }
            self.charge_work((path.len() + raw.len()) as u64)?;
            let value = strict_value(&raw, max_bytes)?;
            if fields.insert(name.to_owned(), value).is_some() {
                return Err(Error::Invalid("public D1 duplicate header path"));
            }
        }
        Ok(serde_json::Value::Object(fields))
    }

    pub(crate) fn header_object_owned(
        &self,
        role: &str,
        prefix: &str,
        cap: usize,
        state: &CreationState<'_>,
    ) -> Result<serde_json::Value> {
        let mut fields = serde_json::Map::new();
        let mut slots = 0usize;
        self.visit_header_fields(role, prefix, cap, |name, raw| {
            let next = crate::knowledge_normalization::serde_object_slots_upper(fields.len() + 1)?;
            state.retain(
                next.checked_sub(slots)
                    .and_then(|n| n.checked_add(name.len()))
                    .ok_or(Error::Budget("owned model header map state"))?,
            )?;
            slots = next;
            let value = state.serde_owned(raw, cap)?;
            if fields.insert(name.to_owned(), value).is_some() {
                return Err(Error::Invalid("public D1 duplicate header path"));
            }
            Ok(())
        })?;
        state.remaining(0)?;
        Ok(serde_json::Value::Object(fields))
    }

    /// Borrow authenticated header fields; no decoded serde DOM or payload copy.
    pub(crate) fn visit_header_fields(
        &self,
        role: &str,
        prefix: &str,
        max_bytes: usize,
        mut sink: impl FnMut(&str, &[u8]) -> Result<()>,
    ) -> Result<()> {
        self.check_custody()?;
        let cap = max_bytes.min(MAX_HEADER_BYTES);
        if cap == 0 {
            return Err(Error::Budget("public D1 header bytes"));
        }
        let db = self.read_db()?;
        let mut statement = db.prepare("SELECT path,length(json),CASE WHEN length(json)<=?2 THEN json END FROM capture_headers WHERE role=?1 ORDER BY path")?;
        let mut rows = statement.query(params![role, cap as i64])?;
        let mut total = 2usize;
        let mut count = 0usize;
        while let Some(row) = rows.next()? {
            self.check_custody()?;
            let path = row
                .get_ref(0)?
                .as_str()
                .map_err(|_| Error::Invalid("public D1 header path"))?;
            let dynamic = self.runtime_capture_role == Some(RuntimeCaptureRole::Philosophy)
                && role == PHILOSOPHY;
            let name = if prefix.is_empty() {
                if dynamic || !path.contains('/') {
                    Some(path)
                } else {
                    None
                }
            } else {
                path.strip_prefix(prefix).and_then(|s| s.strip_prefix('/'))
            };
            let Some(name) = name else {
                continue;
            };
            if name.is_empty() || (!dynamic && name.contains('/')) {
                return Err(Error::Invalid("public D1 nested header path"));
            }
            let declared: i64 = row.get(1)?;
            if declared <= 0 || declared as usize > cap {
                return Err(Error::Budget("public D1 header bytes"));
            }
            total = name
                .len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(3))
                .and_then(|n| n.checked_add(declared as usize))
                .and_then(|n| n.checked_add(usize::from(count > 0)))
                .and_then(|n| total.checked_add(n))
                .filter(|n| *n <= cap)
                .ok_or(Error::Budget("public D1 header bytes"))?;
            let raw = row
                .get_ref(2)?
                .as_blob()
                .map_err(|_| Error::Budget("public D1 header bytes"))?;
            if raw.len() != declared as usize {
                return Err(Error::Invalid("public D1 header length changed"));
            }
            self.charge_work((path.len() + raw.len()) as u64)?;
            sink(name, raw)?;
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("public D1 header fields"))?;
        }
        self.check_custody()
    }
    /// Original configured SQLite cache plus one bounded row and control workspace.
    pub(crate) fn carrier_reader_workspace(&self) -> Result<usize> {
        (if self.sqlite_heap.is_some() {
            0
        } else {
            self.limits.sqlite_cache_kib as usize
        })
        .checked_mul(1024)
        .and_then(|n| n.checked_add(MAX_ROW_BYTES))
        .and_then(|n| n.checked_add(65536))
        .ok_or(Error::Budget("public D1 carrier workspace"))
    }
    /// Borrow names from the held capture cursor instead of retaining a second inventory.
    pub(crate) fn visit_collection_names(
        &self,
        role: &str,
        mut sink: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        self.check_custody()?;
        if self.runtime_capture_role != Some(RuntimeCaptureRole::Philosophy) || role != PHILOSOPHY {
            return Err(Error::Invalid("dynamic philosophy carrier required"));
        }
        let db = self.read_db()?;
        let mut statement = db.prepare(
            "SELECT collection FROM capture_collections WHERE role=?1 ORDER BY collection",
        )?;
        let mut rows = statement.query([role])?;
        let mut count = 0usize;
        while let Some(row) = rows.next()? {
            self.check_custody()?;
            if count >= 4096 {
                return Err(Error::Budget("public D1 captured collection names"));
            }
            let name = row
                .get_ref(0)?
                .as_str()
                .map_err(|_| Error::Invalid("public D1 collection name"))?;
            if !valid_top_level_collection(name) {
                return Err(Error::Invalid(
                    "public D1 captured philosophy collection name",
                ));
            }
            self.charge_work(name.len() as u64)?;
            sink(name)?;
            count += 1;
        }
        self.check_custody()
    }

    /// Declared presence and container kind from the authenticated capture.
    /// An absent collection remains absent; callers must not invent empty rows.
    pub(crate) fn captured_collection_kind(
        &self,
        role: &str,
        collection: &str,
    ) -> Result<Option<String>> {
        self.check_custody()?;
        let dynamic_philosophy = self.runtime_capture_role == Some(RuntimeCaptureRole::Philosophy)
            && role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if !selected_rows(role, collection)
            && !dynamic_philosophy
            && !(role == CORPUS && collection == "source_navigation")
        {
            return Err(Error::Invalid("public D1 collection outside fixed input"));
        }
        let db = self.read_db()?;
        use rusqlite::OptionalExtension;
        let kind: Option<String> = db
            .query_row(
                "SELECT kind FROM capture_collections WHERE role=?1 AND collection=?2",
                params![role, collection],
                |row| {
                    let kind = row
                        .get_ref(0)?
                        .as_str()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    if kind.len() > 7 {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    Ok(kind.to_owned())
                },
            )
            .optional()?;
        if let Some(value) = &kind {
            self.charge_work(value.len() as u64)?;
        }
        Ok(kind)
    }

    pub(crate) fn captured_collection_names(&self, role: &str) -> Result<Vec<String>> {
        self.check_custody()?;
        if self.runtime_capture_role != Some(RuntimeCaptureRole::Philosophy) || role != PHILOSOPHY {
            return Err(Error::Invalid("dynamic philosophy carrier required"));
        }
        let db = self.read_db()?;
        let mut statement = db.prepare(
            "SELECT collection FROM capture_collections WHERE role=?1 ORDER BY collection",
        )?;
        let mut rows = statement.query([role])?;
        let mut names = Vec::new();
        while let Some(row) = rows.next()? {
            if names.len() >= 4096 {
                return Err(Error::Budget("public D1 captured collection names"));
            }
            let name: String = row.get(0)?;
            if !valid_top_level_collection(&name) {
                return Err(Error::Invalid(
                    "public D1 captured philosophy collection name",
                ));
            }
            self.charge_work(name.len() as u64)?;
            names.push(name);
        }
        Ok(names)
    }

    /// Physical part order is a hash traversal. This cursor restores the
    /// owner's declared logical order from the private disk index; each row is
    /// checked against the digest recorded at capture before it is exposed.
    pub(crate) fn captured_row_count_owned(
        &self,
        role: &str,
        collection: &str,
        state: &CreationState<'_>,
    ) -> Result<u64> {
        let _hold = state.hold(
            tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(
            ),
        )?;
        let db = self.read_db()?;
        let count: i64 = db.query_row(
            "SELECT count(*) FROM capture_rows WHERE role=?1 AND collection=?2",
            rusqlite::params![role, collection],
            |r| r.get(0),
        )?;
        state.active()?;
        u64::try_from(count).map_err(|_| Error::Invalid("captured owned row count"))
    }
    pub fn visit_rows(
        &self,
        role: &str,
        collection: &str,
        sink: impl FnMut(u64, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        self.visit_rows_ordered(role, collection, false, sink)
    }

    /// Exact captured array order for original components whose ordinal is
    /// part of the receipt. Published/query ordering remains visit_rows().
    pub(crate) fn visit_original_rows(
        &self,
        role: &str,
        collection: &str,
        sink: impl FnMut(u64, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        self.visit_rows_ordered(role, collection, true, sink)
    }

    fn visit_rows_ordered(
        &self,
        role: &str,
        collection: &str,
        original: bool,
        mut sink: impl FnMut(u64, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        self.check_custody()?;
        let dynamic_philosophy = self.runtime_capture_role == Some(RuntimeCaptureRole::Philosophy)
            && role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if !selected_rows(role, collection) && !dynamic_philosophy {
            return Err(Error::Invalid("public D1 collection outside fixed input"));
        }
        let db = self.read_db()?;
        let sql = if original {
            "SELECT CASE WHEN length(json)<=8388608 THEN json END,sha256 FROM capture_rows WHERE role=?1 AND collection=?2 ORDER BY ord,source_key"
        } else {
            "SELECT CASE WHEN length(json)<=8388608 THEN json END,sha256 FROM capture_rows WHERE role=?1 AND collection=?2 ORDER BY sort0,sort1,source_key"
        };
        let mut statement = db.prepare(sql)?;
        let mut rows = statement.query(params![role, collection])?;
        let mut count = 0u64;
        while let Some(row) = rows.next()? {
            let raw = row
                .get_ref(0)?
                .as_blob()
                .map_err(|_| Error::Budget("public D1 captured row bytes"))?;
            self.charge_work(raw.len() as u64)?;
            let expected = row
                .get_ref(1)?
                .as_blob()
                .map_err(|_| Error::Invalid("public D1 captured row digest"))?;
            if raw.len() > MAX_ROW_BYTES
                || Digest256::of_bytes(&raw).as_bytes().as_slice() != expected
            {
                return Err(Error::Invalid("public D1 captured row mismatch"));
            }
            sink(count, &raw)?;
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("public D1 visit rows"))?;
        }
        Ok(count)
    }
}


#[cfg(test)]
mod construction_phase_tests {
    use super::*;
    use crate::knowledge_payload_read::RuntimeKnowledgeOwnedBudget;
    use std::{sync::atomic::Ordering, time::Duration};

    #[test]
    fn construction_peak_transfers_only_live_value_state() {
        const CHILD: &str = "TOS_CONSTRUCTION_PHASE_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            // The real dedicated initializer owns process-global SQLite state.
            // Isolate it so this test does not change the rest of the suite.
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "d1_public_capture::construction_phase_tests::construction_peak_transfers_only_live_value_state", "--nocapture"])
                .env(CHILD, "1").output().unwrap();
            assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let remaining = |bytes: usize| (4 * 1024 * 1024usize).checked_sub(bytes)
            .ok_or(Error::Budget("test original state"));
        let heap = sqlite_budget::DedicatedSessionSqliteHeap::establish(
            1024 * 1024, &remaining, deadline, &cancelled,
        ).unwrap();
        let work = Arc::new(AtomicU64::new(0));
        let vm = Arc::new(AtomicU64::new(0));
        let budget = RuntimeKnowledgeOwnedBudget {
            remaining_after_retained: &remaining, original_work: &work,
            original_work_limit: 1_000_000, original_sql_vm: &vm,
            original_sql_vm_limit: 1_000_000, original_sqlite_heap: &heap,
            remaining_json_visits: 10_000, owner_deadline: deadline,
            operation_deadline: deadline, cancelled: &cancelled,
        };
        let state = CreationState::from_runtime_owned_budget(&budget).unwrap();
        let baseline = state.retained.get();
        let mut peak = state.hold(2 * 1024 * 1024).unwrap();
        let mut text = String::with_capacity(96 * 1024);
        text.push_str(&"x".repeat(64 * 1024));
        let value = serde_json::Value::String(text);
        assert!(state.hold(3 * 1024 * 1024).is_err());
        peak.finish_value_construction(&value).unwrap();
        assert_eq!(peak.admitted, std::mem::size_of::<serde_json::Value>() + 96 * 1024);
        assert_eq!(state.retained.get(), baseline + peak.admitted);
        assert!(work.load(Ordering::Acquire) > 0);
        drop(state.hold(3 * 1024 * 1024).unwrap());
        drop(value); drop(peak);
        assert_eq!(state.retained.get(), baseline);

        let raw = br#"{"z":["\u0416",1.2300],"a":{"b":true}}"#;
        let limits = JsonLimits::new(8192, 96, 10_000, 4096).unwrap();
        let (decoded, hold) = state.serde_scoped_with_limits(raw, limits).unwrap();
        assert_eq!(decoded["z"][0].as_str(), Some("Ж"));
        assert_eq!(decoded["z"][1].as_number().unwrap().as_str(), "1.2300");
        assert_eq!(decoded.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>(), ["z", "a"]);
        assert_eq!(state.retained.get(), baseline + hold.admitted);
        drop(decoded); drop(hold);
        assert_eq!(state.retained.get(), baseline);
        // A conservative Number upper bound must not introduce a new refusal.
        let large_number = "9".repeat(256);
        let (number, number_hold) = state.serde_scoped_with_limits(large_number.as_bytes(), limits).unwrap();
        assert_eq!(number.as_number().unwrap().as_str(), large_number);
        drop(number); drop(number_hold);
        assert_eq!(state.retained.get(), baseline);
        // Compare the existing strict serde decoder for exact numeric spelling,
        // Unicode, insertion order and nested arrays. It is not a runtime path.
        for raw in [
            br#"[-0,0,1.2300,1e+3,1E-3,18446744073709551616,-9223372036854775809]"#.as_slice(),
            br#"{"z":"\uD83D\uDE00","a":[null,true,false,"\u0416"],"empty":{}}"#.as_slice(),
        ] {
            let previous = state.decode_serde_raw(raw).unwrap();
            let (next, hold) = state.serde_scoped_with_limits(raw, limits).unwrap();
            assert_eq!(serde_json::to_vec(&next).unwrap(), serde_json::to_vec(&previous).unwrap());
            drop(next); drop(hold);
            assert_eq!(state.retained.get(), baseline);
        }
        for raw in [br#"{"a":1,"\u0061":2}"#.as_slice(), br#"{} []"#.as_slice(), br#""\ud800""#.as_slice()] {
            assert!(state.serde_scoped_with_limits(raw, limits).is_err());
            assert_eq!(state.retained.get(), baseline);
        }
        let long = serde_json::to_vec(&serde_json::json!({"text": "x".repeat(8192),"n":1.2300})).unwrap();
        let mut old_work = 0;
        // Recreate the former grammar/geometry/second-parse sequence once.
        let before_work = work.load(Ordering::Acquire);
        let old_document = creation_json_with_limits(&state, &long, JsonLimits::new(16384,96,10000,4096).unwrap()).unwrap();
        state.charge_work(long.len()*2).unwrap();
        let _ = crate::knowledge_normalization::serde_input_workspace_upper(&old_document, long.len()).unwrap();
        drop(old_document);
        let old = state.decode_serde_raw(&long).unwrap();
        old_work += work.load(Ordering::Acquire)-before_work;
        let before_work = work.load(Ordering::Acquire);
        let (next, hold) = state.serde_scoped_with_limits(&long, JsonLimits::new(16384,96,10000,4096).unwrap()).unwrap();
        let new_work = work.load(Ordering::Acquire)-before_work;
        assert_eq!(next,old);
        assert!(new_work < old_work, "single parse must remove real work: old={old_work} new={new_work}");
        eprintln!("strict conversion measured work old={old_work} new={new_work}");
        drop(next);drop(hold);drop(old);
        assert_eq!(state.retained.get(),baseline);
        // Exact-length carriers eliminate only the count walk. The bytes and
        // their custody remain identical, including every refused callback.
        let value: serde_json::Value = serde_json::from_slice(&long).unwrap();
        let expected = serde_json::to_vec(&value).unwrap();
        let before = work.load(Ordering::Acquire);
        state.with_json_encoded(&value, 16384, |bytes| {
            assert_eq!(bytes, expected);
            assert!(state.retained.get() >= baseline + expected.len());
            Ok(())
        }).unwrap();
        let counted_work = work.load(Ordering::Acquire) - before;
        assert_eq!(state.retained.get(), baseline);
        let before = work.load(Ordering::Acquire);
        state.with_json_encoded_exact(&value, expected.len(), 16384, |bytes| {
            assert_eq!(bytes, expected);
            assert!(state.retained.get() >= baseline + expected.len());
            Ok(())
        }).unwrap();
        let exact_work = work.load(Ordering::Acquire) - before;
        assert_eq!(counted_work, 2 * exact_work);
        assert_eq!(exact_work, expected.len() as u64);
        assert_eq!(state.retained.get(), baseline);
        for declared in [0, expected.len() - 1, expected.len() + 1, 16385] {
            let mut called = false;
            assert!(state.with_json_encoded_exact(&value, declared, 16384, |_| {
                called = true; Ok(())
            }).is_err());
            assert!(!called);
            assert_eq!(state.retained.get(), baseline);
        }
        assert!(state.with_json_encoded_exact(&value, expected.len(), 16384, |_| {
            Err::<(), _>(Error::Invalid("consumer refused"))
        }).is_err());
        assert_eq!(state.retained.get(), baseline);
        eprintln!("exact JSON emission work counted={counted_work} exact={exact_work}");
        // Parse work follows actual UTF8 bytes and grammar nodes. A large
        // string is one node; a partial invalid array still consumes its paid
        // prefix. The caller cannot reset work/visits after a refusal.
        let parse_raw = br#"{"text":"long scalar string","values":[1,2,3]}"#;
        let before_work = work.load(Ordering::Acquire);
        let before_visits = state.json_visits.get();
        let parsed = creation_json_with_limits(&state, parse_raw, limits).unwrap();
        let used = state.json_visits.get() - before_visits;
        assert_eq!(used, 6);
        assert_eq!(work.load(Ordering::Acquire) - before_work, (parse_raw.len() + used) as u64);
        drop(parsed);
        assert_eq!(state.retained.get(), baseline);
        let before_work = work.load(Ordering::Acquire);
        let before_visits = state.json_visits.get();
        let broken = br#"[1,2,"#;
        assert!(creation_json_with_limits(&state, broken, limits).is_err());
        assert!(state.json_visits.get() > before_visits);
        assert!(work.load(Ordering::Acquire) >= before_work + broken.len() as u64);
        assert_eq!(state.retained.get(), baseline);
        // Distinct original owner with a byte-tight work cap cannot execute
        // even the first grammar node after its UTF8 span uses the budget.
        let limited_work = Arc::new(AtomicU64::new(0));
        let limited_budget = RuntimeKnowledgeOwnedBudget {
            original_work: &limited_work, original_work_limit: parse_raw.len() as u64,
            ..budget
        };
        let limited_state = CreationState::from_runtime_owned_budget(&limited_budget).unwrap();
        assert!(creation_json_with_limits(&limited_state, parse_raw, limits).is_err());
        assert_eq!(limited_work.load(Ordering::Acquire), parse_raw.len() as u64);
        assert_eq!(limited_state.json_visits.get(), 0);
        assert_eq!(limited_state.retained.get(), 0);
        drop(limited_state);
        let persistent = state.serde_owned_with_limits(raw, limits).unwrap();
        assert!(state.persistent.get() > 0);
        assert_eq!(state.retained.get(), baseline + state.persistent.get());
        drop(persistent);
        state.transfer_persistent_to_capture().unwrap();
        assert_eq!(state.retained.get(), baseline);

        let mut interrupted = state.hold(1024).unwrap();
        let before = state.retained.get();
        cancelled.store(true, Ordering::Release);
        assert!(interrupted.finish_value_construction(&serde_json::Value::Null).is_err());
        assert_eq!(state.retained.get(), before);
        cancelled.store(false, Ordering::Release);
        work.store(budget.original_work_limit, Ordering::Release);
        assert!(interrupted.finish_value_construction(&serde_json::Value::Null).is_err());
        assert_eq!(state.retained.get(), before);
        drop(interrupted);
        assert_eq!(state.retained.get(), baseline);
    }
}
