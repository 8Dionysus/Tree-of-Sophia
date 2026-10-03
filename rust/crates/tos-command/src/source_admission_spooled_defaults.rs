//! Candidate-bound default-district stores over the invocation's shared SQLite
//! scope.  This module keeps Records, paths, bibliography events, default
//! events and claims in their owning bounded projections; it does not grant
//! source-admission authority.

use crate::{
    source_admission_candidate_records::CandidateRecordsInput,
    source_admission_spooled_candidate::{CandidateFence, SpoolCandidate},
    source_admission_spooled_index::SpoolIndexLimits,
};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use std::{borrow::Cow, cell::Cell, io, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::{PinnedSqliteAuxScope, PinnedSqliteConnection, SourceMembershipV1};
use tos_validation::{
    biblio_rules::{
        BiblioClaim, SourceFoundationBiblioClaimSink, SourceFoundationBiblioEventSink,
        SourceFoundationBiblioManifestSink, SourceFoundationBiblioQueryBudget,
        SourceFoundationBiblioStoredSink,
    },
    item_rules::ItemRefusal,
    record_biblio_cut::{SourceCutInput, SourceCutInputWithIdentity},
    source_foundation_default_rules::{
        SourceFoundationDefaultClaims, SourceFoundationDefaultEventLookup,
        SourceFoundationDefaultEventStore, SourceFoundationDefaultEventStoreCost,
        SourceFoundationDefaultPaths, SourceFoundationDefaultRecordsLookup,
        SourceFoundationDefaultStoredLimits,
    },
    source_foundation_discovery::{
        DiscoveryDigestCache, DiscoveryDigestCacheCost, DiscoveryEventSummaryNamespace,
        DiscoveryEventSummaryStore, DiscoveryEventSummaryStoreCost, DiscoveryRunSummary,
        DiscoveryRunSummaryStore, DiscoveryRunSummaryStoreCost, DiscoverySchemaRequestStore,
        DiscoverySchemaRequestStoreCost, DiscoverySeenIdNamespace, DiscoverySeenIds,
        SchemaRequest as DiscoverySchemaRequest,
    },
    source_foundation_records::{
        SourceFoundationRecordsCollection as RecordsCollection,
        SourceFoundationRecordsStoredFact as StoredFact, SourceFoundationRecordsStreamedReport,
    },
};

const EVENT_CODEC_VERSION: i64 = 1;
const CLAIM_CODEC_VERSION: i64 = 1;

fn source_refusal() -> ItemRefusal {
    ItemRefusal::Source("candidate default store refused".into())
}

fn sql_refusal(_: rusqlite::Error) -> ItemRefusal {
    source_refusal()
}

fn invalid(_: io::Error) -> ItemRefusal {
    source_refusal()
}

fn storage_error(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn checked_add(left: usize, right: usize) -> Result<usize, ItemRefusal> {
    left.checked_add(right).ok_or(ItemRefusal::Budget)
}

fn estimate_string_state(value: &str) -> Result<usize, ItemRefusal> {
    value
        .len()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(size_of::<String>() + 32))
        .ok_or(ItemRefusal::Budget)
}

fn estimate_value_state(value: &Value) -> Result<usize, ItemRefusal> {
    fn walk(value: &Value, depth: usize) -> Result<usize, ItemRefusal> {
        if depth > 128 {
            return Err(ItemRefusal::Budget);
        }
        let mut bytes = size_of::<Value>();
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                bytes = checked_add(bytes, 32)?;
            }
            Value::String(text) => {
                bytes = checked_add(bytes, estimate_string_state(text)?)?;
            }
            Value::Array(values) => {
                let capacity = values
                    .len()
                    .checked_mul(size_of::<Value>() * 2)
                    .and_then(|bytes| bytes.checked_add(64))
                    .ok_or(ItemRefusal::Budget)?;
                bytes = checked_add(bytes, capacity)?;
                for child in values {
                    bytes = checked_add(bytes, walk(child, depth + 1)?)?;
                }
            }
            Value::Object(values) => {
                let nodes = values
                    .len()
                    .checked_mul(size_of::<(String, Value)>() + 96)
                    .ok_or(ItemRefusal::Budget)?;
                bytes = checked_add(bytes, nodes)?;
                for (key, child) in values {
                    bytes = checked_add(bytes, estimate_string_state(key)?)?;
                    bytes = checked_add(bytes, walk(child, depth + 1)?)?;
                }
            }
        }
        Ok(bytes)
    }
    walk(value, 0)
}

fn json_state_upper_bound(raw_bytes: usize) -> Result<usize, ItemRefusal> {
    // Before serde allocates a decoded Value, reserve for short scalar nodes,
    // object-map entries, string/key storage, and the decoder stack. This is a
    // deliberately conservative byte bound over the already-read JSON bytes.
    raw_bytes
        .checked_mul(128)
        .and_then(|bytes| bytes.checked_add(8192))
        .ok_or(ItemRefusal::Budget)
}

struct JsonCounter {
    bytes: usize,
    limit: usize,
}

impl io::Write for JsonCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|next| *next <= self.limit)
            .ok_or_else(|| io::Error::other("bounded JSON size exceeded"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn json_len<T: serde::Serialize + ?Sized>(value: &T, limit: usize) -> Result<usize, ItemRefusal> {
    let mut writer = JsonCounter { bytes: 0, limit };
    serde_json::to_writer(&mut writer, value).map_err(|_| ItemRefusal::Budget)?;
    Ok(writer.bytes)
}

fn encoded_json<T: serde::Serialize + ?Sized>(
    value: &T,
    expected_bytes: usize,
    max_state_bytes: usize,
) -> Result<Vec<u8>, ItemRefusal> {
    let workspace = expected_bytes
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(size_of::<Vec<u8>>() + 1024))
        .ok_or(ItemRefusal::Budget)?;
    if workspace > max_state_bytes {
        return Err(ItemRefusal::Budget);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_bytes)
        .map_err(|_| ItemRefusal::Budget)?;
    serde_json::to_writer(&mut bytes, value).map_err(|_| ItemRefusal::Budget)?;
    if bytes.len() != expected_bytes {
        return Err(source_refusal());
    }
    Ok(bytes)
}

fn bounded_row_text(
    row: &rusqlite::Row<'_>,
    column: usize,
    cap: usize,
) -> rusqlite::Result<String> {
    match row.get_ref(column)? {
        rusqlite::types::ValueRef::Text(raw)
            if raw
                .len()
                .checked_mul(16)
                .and_then(|bytes| bytes.checked_add(2048))
                .is_some_and(|bytes| bytes <= cap) =>
        {
            row.get(column)
        }
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn row_text_state(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<usize> {
    match row.get_ref(column)? {
        rusqlite::types::ValueRef::Text(raw) => raw
            .len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or(rusqlite::Error::InvalidQuery),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn bounded_row_text_precharged(
    context: ProviderContext<'_, '_, '_>,
    row: &rusqlite::Row<'_>,
    column: usize,
    cap: usize,
) -> rusqlite::Result<String> {
    let state = row_text_state(row, column)?;
    context
        .active_state(state)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    bounded_row_text(row, column, cap)
}

fn checked_u64_blob(bytes: &[u8]) -> Result<u64, ItemRefusal> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| source_refusal())?;
    Ok(u64::from_be_bytes(bytes))
}

fn usize_u64(value: usize) -> Result<u64, ItemRefusal> {
    u64::try_from(value).map_err(|_| ItemRefusal::Budget)
}

fn check_candidate(
    candidate: &SpoolCandidate<'_>,
    fence: CandidateFence,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    if !candidate.matches_invocation(deadline, cancelled) {
        candidate.abandon();
        return Err(source_refusal());
    }
    candidate.tick().map_err(invalid)?;
    if candidate.fence().map_err(invalid)? != fence {
        candidate.abandon();
        return Err(source_refusal());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ProviderContext<'candidate, 'host, 'cancel> {
    candidate: &'candidate SpoolCandidate<'host>,
    fence: CandidateFence,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    row_state_limit: usize,
    operation_state_limit: usize,
    page_budget: tos_validation::source_foundation_records::SourceFoundationRecordsPageBudget,
    max_scan_rows: u64,
}

impl ProviderContext<'_, '_, '_> {
    fn check(&self) -> Result<(), ItemRefusal> {
        check_candidate(self.candidate, self.fence, self.deadline, self.cancelled)
    }

    fn row_state(&self, bytes: usize) -> Result<(), ItemRefusal> {
        if bytes > self.row_state_limit || bytes > self.operation_state_limit {
            return Err(ItemRefusal::Budget);
        }
        self.candidate.check_state(bytes).map_err(invalid)
    }

    fn add_scan_rows(&self, scanned: &Cell<u64>, rows: usize) -> Result<(), ItemRefusal> {
        let next = scanned
            .get()
            .checked_add(usize_u64(rows)?)
            .filter(|next| self.max_scan_rows != 0 && *next <= self.max_scan_rows)
            .ok_or(ItemRefusal::Budget)?;
        scanned.set(next);
        Ok(())
    }

    fn active_state(&self, bytes: usize) -> Result<(), ItemRefusal> {
        if bytes > self.operation_state_limit {
            return Err(ItemRefusal::Budget);
        }
        self.candidate.check_state(bytes).map_err(invalid)
    }
}

/// Repeatable candidate metadata traversal. It retains only scalar accounting;
/// each call reaches actual metadata EOF on the original input and checks the
/// same CandidateFence again.
pub(crate) struct CandidateDefaultPaths<'candidate, 'input, 'host, 'cancel, 'budget> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    input: &'input CandidateRecordsInput<'candidate, 'host>,
    max_paths_per_scan: u64,
    max_path_bytes_per_scan: u64,
    path_rows: &'budget Cell<u64>,
}

impl<'candidate, 'input, 'host, 'cancel, 'budget>
    CandidateDefaultPaths<'candidate, 'input, 'host, 'cancel, 'budget>
{
    pub(crate) fn new(
        candidate: &'candidate SpoolCandidate<'host>,
        input: &'input CandidateRecordsInput<'candidate, 'host>,
        page_budget: tos_validation::source_foundation_records::SourceFoundationRecordsPageBudget,
        max_paths_per_scan: u64,
        max_path_bytes_per_scan: u64,
        max_scan_rows: u64,
        operation_state_limit: usize,
        path_rows: &'budget Cell<u64>,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let fence = candidate.fence().map_err(invalid)?;
        if max_paths_per_scan == 0
            || max_path_bytes_per_scan == 0
            || max_scan_rows == 0
            || input.input_identity() != &fence
        {
            return Err(ItemRefusal::Budget);
        }
        input.verify_invocation(deadline, cancelled)?;
        check_candidate(candidate, fence, deadline, cancelled)?;
        Ok(Self {
            context: ProviderContext {
                candidate,
                fence,
                deadline,
                cancelled,
                row_state_limit: operation_state_limit,
                operation_state_limit,
                page_budget,
                max_scan_rows,
            },
            input,
            max_paths_per_scan,
            max_path_bytes_per_scan,
            path_rows,
        })
    }

    pub(crate) fn verify_eof(&self) -> Result<(), ItemRefusal> {
        self.for_each_path(&mut |_| Ok(()))
    }

    fn contains_path(&self, target: &str) -> Result<bool, ItemRefusal> {
        self.context.check()?;
        self.input
            .verify_invocation(self.context.deadline, self.context.cancelled)?;
        let path_state = target
            .len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(8192))
            .ok_or(ItemRefusal::Budget)?;
        self.context.active_state(path_state)?;
        let allowance = self
            .context
            .operation_state_limit
            .checked_sub(path_state)
            .filter(|bytes| *bytes != 0)
            .ok_or(ItemRefusal::Budget)?;
        let path = RelativePath::parse(target).map_err(|_| source_refusal())?;
        self.context.add_scan_rows(self.path_rows, 1)?;
        let found = self
            .context
            .candidate
            .member_bounded(&path, allowance)
            .map_err(invalid)?;
        drop(path);
        self.input
            .verify_invocation(self.context.deadline, self.context.cancelled)?;
        self.context.check()?;
        if let Some(metadata) = found {
            if metadata.path.as_str() != target {
                self.context.candidate.abandon();
                return Err(source_refusal());
            }
            let row_state = metadata
                .path
                .as_str()
                .len()
                .checked_mul(16)
                .and_then(|bytes| bytes.checked_add(2048))
                .ok_or(ItemRefusal::Budget)?;
            self.context
                .active_state(checked_add(path_state, row_state)?)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn traverse(
        &self,
        mut visit: impl FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        use tos_validation::record_biblio_cut::SourceCutInput;
        self.context.check()?;
        self.input
            .verify_invocation(self.context.deadline, self.context.cancelled)?;
        let mut count = 0u64;
        let mut bytes = 0u64;
        let mut observed_source_bytes = 0u64;
        let mut previous: Option<String> = None;
        let result = self.input.source_input().for_each_current_member_meta(
            self.context.deadline,
            self.context.cancelled,
            &mut |meta| {
                self.context.check()?;
                let path = meta.path;
                if previous.as_deref().is_some_and(|prior| prior >= path) {
                    return Err(source_refusal());
                }
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= self.max_paths_per_scan)
                    .ok_or(ItemRefusal::Budget)?;
                bytes = bytes
                    .checked_add(usize_u64(path.len())?)
                    .filter(|n| *n <= self.max_path_bytes_per_scan)
                    .ok_or(ItemRefusal::Budget)?;
                observed_source_bytes = observed_source_bytes
                    .checked_add(meta.size_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                self.context.add_scan_rows(self.path_rows, 1)?;
                let previous_state = previous
                    .as_deref()
                    .map(estimate_string_state)
                    .transpose()?
                    .unwrap_or(0);
                self.context
                    .active_state(checked_add(previous_state, estimate_string_state(path)?)?)?;
                visit(path)?;
                previous = Some(path.to_owned());
                Ok(())
            },
        );
        result?;
        self.input
            .verify_invocation(self.context.deadline, self.context.cancelled)?;
        self.context.check()?;
        let (expected_count, expected_bytes) = self.context.candidate.membership_counts();
        if count != expected_count
            || observed_source_bytes != expected_bytes
            || self.context.fence.source_bytes != expected_bytes
        {
            self.context.candidate.abandon();
            return Err(source_refusal());
        }
        Ok(())
    }
}

impl SourceFoundationDefaultPaths for CandidateDefaultPaths<'_, '_, '_, '_, '_> {
    fn contains(&self, target: &str) -> Result<bool, ItemRefusal> {
        self.contains_path(target)
    }

    fn contains_with_checkpoint(
        &self,
        target: &str,
        checkpoint: &mut dyn FnMut() -> Result<(), ItemRefusal>,
    ) -> Result<bool, ItemRefusal> {
        checkpoint()?;
        let found = self.contains_path(target)?;
        checkpoint()?;
        Ok(found)
    }

    fn for_each_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.traverse(visit)
    }
}

#[derive(Clone, Copy)]
struct EventProjectionState {
    unique_ids: u64,
    observation_rows: u64,
    last_source_ordinal: Option<u64>,
    merged_json_bytes: usize,
    workspace_peak_bytes: usize,
    retained_state_bytes: usize,
}

impl EventProjectionState {
    fn fresh() -> Self {
        Self {
            unique_ids: 0,
            observation_rows: 0,
            last_source_ordinal: None,
            merged_json_bytes: 2,
            workspace_peak_bytes: 0,
            retained_state_bytes: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct ClaimProjectionState {
    inserted_rows: u64,
    last_ordinal: Option<u64>,
    workspace_peak_bytes: usize,
}

impl ClaimProjectionState {
    fn fresh() -> Self {
        Self {
            inserted_rows: 0,
            last_ordinal: None,
            workspace_peak_bytes: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct ManifestProjectionState {
    observations: u64,
    unique_ids: u64,
    workspace_peak_bytes: usize,
}

/// One cumulative cap covers every Biblio SQL statement in this candidate
/// scope. The Biblio owner may narrow the caller-selected phase cap once its
/// ItemLimits are known; neither binding nor provider loans reset usage.
struct BiblioQueryBudget {
    selected_limit: u64,
    bound_limit: Cell<Option<u64>>,
    used: Cell<u64>,
}

impl BiblioQueryBudget {
    fn new(selected_limit: u64) -> Result<Self, io::Error> {
        if selected_limit == 0 || selected_limit == u64::MAX {
            return Err(storage_error("candidate Biblio SQL budget invalid"));
        }
        Ok(Self {
            selected_limit,
            bound_limit: Cell::new(None),
            used: Cell::new(0),
        })
    }

    fn bind(&self, owner_limit: u64) -> Result<(), ItemRefusal> {
        if owner_limit == 0 || owner_limit == u64::MAX {
            return Err(ItemRefusal::Budget);
        }
        let limit = self
            .bound_limit
            .get()
            .map(|bound| bound.min(owner_limit))
            .unwrap_or(owner_limit.min(self.selected_limit));
        self.bound_limit.set(Some(limit));
        if self.used.get() > limit {
            return Err(ItemRefusal::Budget);
        }
        Ok(())
    }

    fn charge(&self) -> Result<(), ItemRefusal> {
        let limit = self.bound_limit.get().ok_or_else(source_refusal)?;
        let next = self
            .used
            .get()
            .checked_add(1)
            .filter(|next| *next <= limit)
            .ok_or(ItemRefusal::Budget)?;
        self.used.set(next);
        Ok(())
    }
}

impl ManifestProjectionState {
    fn fresh() -> Self {
        Self {
            observations: 0,
            unique_ids: 0,
            workspace_peak_bytes: 0,
        }
    }
}

/// One actual defaults scope owns both incompatible event views and the
/// original-order Biblio claims. Its auxiliary VFS, inode, I/O, space,
/// deadline and cancellation authorities all come from the candidate.
pub(crate) struct SpoolDefaultStore<'candidate, 'host> {
    candidate: &'candidate SpoolCandidate<'host>,
    fence: CandidateFence,
    limits: SpoolIndexLimits,
    db: PinnedSqliteConnection,
    _scope: PinnedSqliteAuxScope,
    default_events: EventProjectionState,
    biblio_events: EventProjectionState,
    claims: ClaimProjectionState,
    biblio_manifests: ManifestProjectionState,
    max_biblio_event_json_bytes: usize,
    max_operation_state_bytes: usize,
    default_event_json_limit: Option<usize>,
    default_events_folded: bool,
    scan_rows: Cell<u64>,
    path_rows: Cell<u64>,
    claim_live_state: Cell<usize>,
    scan_limit: Cell<Option<u64>>,
    biblio_query_budget: BiblioQueryBudget,
}

fn pragma_i64(db: &PinnedSqliteConnection, pragma: &'static str) -> io::Result<i64> {
    db.query_row(pragma, [], |row| row.get(0))
        .map_err(|_| storage_error("candidate defaults SQLite policy query refused"))
}

fn pragma_text(db: &PinnedSqliteConnection, pragma: &'static str) -> io::Result<String> {
    db.query_row(pragma, [], |row| match row.get_ref(0)? {
        rusqlite::types::ValueRef::Text(raw) if raw.len() <= 128 => row.get(0),
        _ => Err(rusqlite::Error::InvalidQuery),
    })
    .map_err(|_| storage_error("candidate defaults SQLite policy query refused"))
}

fn set_connection_policy(db: &PinnedSqliteConnection, limits: SpoolIndexLimits) -> io::Result<()> {
    if limits.sqlite.main_logical_bytes == 0
        || limits.sqlite.main_allocated_bytes == 0
        || limits.cache_bytes < 1024
        || limits.cache_bytes > limits.max_row_state_bytes
        || limits.max_row_state_bytes == 0
        || limits.max_row_state_bytes == usize::MAX
    {
        return Err(storage_error("candidate defaults SQLite profile invalid"));
    }
    db.pragma_update(None, "journal_mode", "OFF")
        .map_err(|_| storage_error("candidate defaults journal policy refused"))?;
    db.pragma_update(None, "synchronous", 0)
        .map_err(|_| storage_error("candidate defaults synchronous policy refused"))?;
    db.pragma_update(None, "temp_store", "FILE")
        .map_err(|_| storage_error("candidate defaults temp-store policy refused"))?;
    db.pragma_update(None, "mmap_size", 0)
        .map_err(|_| storage_error("candidate defaults mmap policy refused"))?;
    let cache_kib = i64::try_from(limits.cache_bytes / 1024)
        .map_err(|_| storage_error("candidate defaults cache profile exceeds range"))?;
    if cache_kib <= 0 {
        return Err(storage_error("candidate defaults cache profile is empty"));
    }
    db.pragma_update(None, "cache_size", -cache_kib)
        .map_err(|_| storage_error("candidate defaults cache policy refused"))?;
    let journal = pragma_text(db, "PRAGMA journal_mode")?;
    let synchronous = pragma_i64(db, "PRAGMA synchronous")?;
    let temp_store = pragma_i64(db, "PRAGMA temp_store")?;
    let mmap = pragma_i64(db, "PRAGMA mmap_size")?;
    let cache_readback = pragma_i64(db, "PRAGMA cache_size")?;
    let page_size = pragma_i64(db, "PRAGMA page_size")?;
    let expected_cache = -cache_kib;
    if !journal.eq_ignore_ascii_case("off")
        || synchronous != 0
        || temp_store != 1
        || mmap != 0
        || cache_readback != expected_cache
        || page_size <= 0
        || u64::try_from(page_size)
            .ok()
            .is_none_or(|bytes| bytes > limits.sqlite.main_allocated_bytes)
        || u64::try_from(cache_readback.unsigned_abs())
            .ok()
            .and_then(|kib| kib.checked_mul(1024))
            .is_none_or(|bytes| bytes > limits.cache_bytes as u64)
    {
        return Err(storage_error("candidate defaults SQLite policy changed"));
    }
    Ok(())
}

impl<'candidate, 'host> SpoolDefaultStore<'candidate, 'host> {
    pub(crate) fn open(
        candidate: &'candidate SpoolCandidate<'host>,
        limits: SpoolIndexLimits,
        max_biblio_query_rows: u64,
        max_biblio_event_json_bytes: usize,
        max_operation_state_bytes: usize,
    ) -> io::Result<Self> {
        if max_biblio_query_rows == 0
            || max_biblio_query_rows == u64::MAX
            || max_biblio_event_json_bytes < 2
            || max_operation_state_bytes == 0
            || max_operation_state_bytes == usize::MAX
            || max_operation_state_bytes > limits.max_row_state_bytes
        {
            return Err(storage_error("candidate defaults state profile invalid"));
        }
        let biblio_query_budget = BiblioQueryBudget::new(max_biblio_query_rows)?;
        candidate.tick()?;
        let fence = candidate.fence()?;
        candidate.check_state(size_of::<Self>())?;
        let mut scope = candidate.open_index_scope(limits.sqlite)?;
        let db = scope
            .open_connection()
            .map_err(|_| storage_error("candidate defaults SQLite open refused"))?;
        set_connection_policy(&db, limits)?;
        db.execute_batch(
            "CREATE TABLE default_events(\
                 slot BLOB NOT NULL PRIMARY KEY CHECK(length(slot)=8),\
                 id TEXT NOT NULL COLLATE BINARY UNIQUE,\
                 value BLOB NOT NULL\
             ) WITHOUT ROWID;\
             CREATE TABLE sf_discovery_seen_ids(\
                 namespace TEXT NOT NULL COLLATE BINARY CHECK(namespace IN ('artifact','composite','composite-representation','event','discovery-event','representation-file','schema-location','payload-observation')),\
                 key TEXT NOT NULL COLLATE BINARY,\
                 first_path TEXT NOT NULL COLLATE BINARY CHECK(length(first_path)>0),\
                 PRIMARY KEY(namespace,key)\
             ) WITHOUT ROWID;\
             CREATE TABLE sf_discovery_run_summaries(\
                 path TEXT NOT NULL COLLATE BINARY PRIMARY KEY CHECK(length(path)>0),\
                 value BLOB NOT NULL CHECK(length(value)>0)\
             ) WITHOUT ROWID;\
             CREATE TABLE sf_discovery_event_summaries(\
                 ordinal BLOB NOT NULL PRIMARY KEY CHECK(length(ordinal)=8),\
                 namespace TEXT NOT NULL COLLATE BINARY CHECK(namespace IN ('boundary','discovery')),\
                 id TEXT NOT NULL COLLATE BINARY CHECK(length(id)>0),\
                 location TEXT NOT NULL COLLATE BINARY CHECK(length(location)>0),\
                 value BLOB NOT NULL CHECK(length(value)>0),\
                 owner_insert INTEGER NOT NULL CHECK(owner_insert IN (0,1))\
             ) WITHOUT ROWID;\
             CREATE INDEX sf_discovery_events_by_key ON sf_discovery_event_summaries(namespace,id,ordinal);\
             CREATE INDEX sf_discovery_events_by_owner_order ON sf_discovery_event_summaries(owner_insert,ordinal);\
             CREATE TABLE sf_discovery_schema_requests(\
                 ordinal BLOB NOT NULL PRIMARY KEY CHECK(length(ordinal)=8),\
                 before_issue BLOB NOT NULL CHECK(length(before_issue)=8),\
                 location TEXT NOT NULL COLLATE BINARY CHECK(length(location)>0),\
                 contract TEXT NOT NULL COLLATE BINARY CHECK(length(contract)>0),\
                 document BLOB NOT NULL CHECK(length(document)>0)\
             ) WITHOUT ROWID;\
             CREATE TABLE sf_discovery_digests(\
                 path TEXT NOT NULL COLLATE BINARY PRIMARY KEY CHECK(length(path)>0),\
                 sha256 TEXT NOT NULL COLLATE BINARY CHECK(length(sha256)=64)\
             ) WITHOUT ROWID;\
             CREATE TABLE biblio_events(\
                 slot BLOB NOT NULL PRIMARY KEY CHECK(length(slot)=8),\
                 id TEXT NOT NULL COLLATE BINARY UNIQUE,\
                 last_ordinal BLOB NOT NULL CHECK(length(last_ordinal)=8),\
                 observation_count BLOB NOT NULL CHECK(length(observation_count)=8),\
                 value BLOB NOT NULL\
             ) WITHOUT ROWID;\
             CREATE TABLE biblio_claims(\
                 ordinal BLOB NOT NULL PRIMARY KEY CHECK(length(ordinal)=8),\
                 path TEXT NOT NULL COLLATE BINARY,\
                 line BLOB NOT NULL CHECK(length(line)=8),\
                 claim_id TEXT COLLATE BINARY,\
                 raw_sha256 TEXT NOT NULL,\
                 native INTEGER NOT NULL CHECK(native IN (0,1)),\
                 value BLOB NOT NULL\
             ) WITHOUT ROWID;\
             CREATE TABLE biblio_manifests(\
                 slot BLOB NOT NULL PRIMARY KEY CHECK(length(slot)=8),\
                 id TEXT NOT NULL COLLATE BINARY UNIQUE,\
                 observation_count BLOB NOT NULL CHECK(length(observation_count)=8),\
                 edition TEXT NOT NULL COLLATE BINARY\
             ) WITHOUT ROWID;\
             CREATE INDEX biblio_claim_location ON biblio_claims(\
                 path COLLATE BINARY,line,ordinal);\
             CREATE INDEX biblio_claim_id_order ON biblio_claims(\
                 claim_id COLLATE BINARY,ordinal);",
        )
        .map_err(|_| storage_error("candidate defaults SQLite schema refused"))?;
        candidate.tick()?;
        Ok(Self {
            candidate,
            fence,
            limits,
            db,
            _scope: scope,
            default_events: EventProjectionState::fresh(),
            biblio_events: EventProjectionState::fresh(),
            claims: ClaimProjectionState::fresh(),
            biblio_manifests: ManifestProjectionState::fresh(),
            max_biblio_event_json_bytes,
            max_operation_state_bytes,
            default_event_json_limit: None,
            default_events_folded: false,
            scan_rows: Cell::new(0),
            path_rows: Cell::new(0),
            claim_live_state: Cell::new(0),
            scan_limit: Cell::new(None),
            biblio_query_budget,
        })
    }

    fn bind_scan_limit(&self, max_scan_rows: u64) -> Result<(), ItemRefusal> {
        if max_scan_rows == 0 {
            return Err(ItemRefusal::Budget);
        }
        match self.scan_limit.get() {
            Some(existing) if existing != max_scan_rows => Err(source_refusal()),
            Some(_) => Ok(()),
            None => {
                self.scan_limit.set(Some(max_scan_rows));
                Ok(())
            }
        }
    }

    fn context<'a>(
        &self,
        page_budget: tos_validation::source_foundation_records::SourceFoundationRecordsPageBudget,
        max_scan_rows: u64,
        operation_state_limit: usize,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<ProviderContext<'candidate, 'host, 'a>, ItemRefusal> {
        if operation_state_limit == 0
            || operation_state_limit > self.limits.max_row_state_bytes
            || operation_state_limit == usize::MAX
            || page_budget.max_state_bytes.get() > operation_state_limit
            || page_budget.max_cursor_bytes.get() > operation_state_limit
            || page_budget.max_rows.get() == 0
            || max_scan_rows == 0
        {
            return Err(ItemRefusal::Budget);
        }
        self.bind_scan_limit(max_scan_rows)?;
        check_candidate(self.candidate, self.fence, deadline, cancelled)?;
        Ok(ProviderContext {
            candidate: self.candidate,
            fence: self.fence,
            deadline,
            cancelled,
            row_state_limit: self.limits.max_row_state_bytes,
            operation_state_limit,
            page_budget,
            max_scan_rows,
        })
    }

    fn report_matches(
        &self,
        records: &SourceFoundationRecordsStreamedReport<'_, CandidateFence>,
    ) -> Result<(), ItemRefusal> {
        if records.input_identity() != &self.fence
            || *records.source_membership() != self.fence.membership
            || records.cost().selected_current_member_bytes != self.fence.source_bytes
        {
            self.candidate.abandon();
            return Err(source_refusal());
        }
        Ok(())
    }

    pub(crate) fn fold_record_events(
        &mut self,
        records: &SourceFoundationRecordsStreamedReport<'_, CandidateFence>,
        stored_limits: SourceFoundationDefaultStoredLimits,
        max_event_json_bytes: usize,
        max_operation_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if self.default_events_folded
            || max_event_json_bytes < 2
            || max_operation_state_bytes > self.max_operation_state_bytes
        {
            self.candidate.abandon();
            return Err(source_refusal());
        }
        self.report_matches(records)?;
        let context = self.context(
            stored_limits.page_budget,
            stored_limits.max_scan_rows,
            max_operation_state_bytes,
            deadline,
            cancelled,
        )?;
        if !self.event_table_empty("default_events")? {
            self.candidate.abandon();
            return Err(source_refusal());
        }
        let candidate = self.candidate;
        let scan_rows = &self.scan_rows;
        let mut events = DefaultEventsProvider {
            context,
            db: &self.db,
            scan_rows,
            limits: self.limits,
            state: &mut self.default_events,
            active_page_state: Cell::new(0),
            json_ceiling: max_event_json_bytes,
        };
        let mut after = None;
        loop {
            context.check()?;
            let page_bytes = stored_limits
                .page_budget
                .max_state_bytes
                .get()
                .checked_add(stored_limits.page_budget.max_cursor_bytes.get())
                .ok_or(ItemRefusal::Budget)?;
            if page_bytes > max_operation_state_bytes {
                return Err(ItemRefusal::Budget);
            }
            context.row_state(stored_limits.page_budget.max_state_bytes.get())?;
            let page = records.index().page(
                RecordsCollection::SourceEventInsertions,
                after.as_ref(),
                stored_limits.page_budget,
                deadline,
                cancelled,
            )?;
            context.check()?;
            context.add_scan_rows(scan_rows, page.rows.len())?;
            events.active_page_state.set(page.charged_state_bytes);
            for row in &page.rows {
                context.check()?;
                match row {
                    StoredFact::SourceEventInsertion((id, value)) => events.insert_event(
                        id,
                        value,
                        max_event_json_bytes,
                        max_operation_state_bytes,
                    )?,
                    _ => {
                        candidate.abandon();
                        return Err(source_refusal());
                    }
                }
            }
            events.active_page_state.set(0);
            after = page.next_cursor;
            if after.is_none() {
                break;
            }
        }
        context.check()?;
        drop(events);
        self.default_event_json_limit = Some(max_event_json_bytes);
        self.default_events_folded = true;
        Ok(())
    }

    fn event_table_empty(&self, table: &'static str) -> Result<bool, ItemRefusal> {
        self.candidate.tick().map_err(invalid)?;
        let sql = match table {
            "default_events" => "SELECT COUNT(*) FROM default_events",
            "biblio_events" => "SELECT COUNT(*) FROM biblio_events",
            "biblio_claims" => "SELECT COUNT(*) FROM biblio_claims",
            "biblio_manifests" => "SELECT COUNT(*) FROM biblio_manifests",
            _ => return Err(source_refusal()),
        };
        let count: i64 = self
            .db
            .query_row(sql, [], |row| row.get(0))
            .map_err(sql_refusal)?;
        self.candidate.tick().map_err(invalid)?;
        Ok(count == 0)
    }

    pub(crate) fn default_event_cost(&self) -> SourceFoundationDefaultEventStoreCost {
        SourceFoundationDefaultEventStoreCost {
            retained_state_bytes: self.default_events.retained_state_bytes,
            workspace_state_bytes: self.default_events.workspace_peak_bytes,
            merged_event_json_bytes: self.default_events.merged_json_bytes,
        }
    }

    pub(crate) fn biblio_event_count(&self) -> u64 {
        self.biblio_events.observation_rows
    }

    pub(crate) fn biblio_claim_count(&self) -> u64 {
        self.claims.inserted_rows
    }
}

fn row_blob<'a>(row: &'a rusqlite::Row<'_>, column: usize) -> rusqlite::Result<&'a [u8]> {
    match row.get_ref(column)? {
        rusqlite::types::ValueRef::Blob(bytes) => Ok(bytes),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn count_from_sql(value: i64) -> Result<u64, ItemRefusal> {
    u64::try_from(value).map_err(|_| source_refusal())
}

fn decode_value(
    context: ProviderContext<'_, '_, '_>,
    raw: &[u8],
    additional_state: usize,
) -> Result<Value, ItemRefusal> {
    let upper = json_state_upper_bound(raw.len())?;
    let precharge = checked_add(upper, additional_state)?;
    context.active_state(precharge)?;
    let value: Value = serde_json::from_slice(raw).map_err(|_| source_refusal())?;
    let actual = checked_add(estimate_value_state(&value)?, additional_state)?;
    context.active_state(actual)?;
    Ok(value)
}

fn value_from_row(
    context: ProviderContext<'_, '_, '_>,
    row: &rusqlite::Row<'_>,
    column: usize,
    max_json_bytes: usize,
    additional_state: usize,
) -> Result<Value, ItemRefusal> {
    let raw = row_blob(row, column).map_err(sql_refusal)?;
    if raw.is_empty() || raw.len() > max_json_bytes {
        return Err(source_refusal());
    }
    decode_value(context, raw, additional_state)
}

fn event_json_addition(
    id_json_bytes: usize,
    value_json_bytes: usize,
    is_new: bool,
) -> Result<usize, ItemRefusal> {
    let separator = usize::from(is_new); // comma before every entry except the first
    id_json_bytes
        .checked_add(1) // colon
        .and_then(|bytes| bytes.checked_add(value_json_bytes))
        .and_then(|bytes| bytes.checked_add(separator))
        .ok_or(ItemRefusal::Budget)
}

fn check_event_operation(
    context: ProviderContext<'_, '_, '_>,
    page_state: usize,
    retained_state: usize,
    row_workspace: usize,
) -> Result<(), ItemRefusal> {
    let total = checked_add(page_state, retained_state)?;
    let total = checked_add(total, row_workspace)?;
    context.active_state(total)
}

fn next_event_json_total(
    old_total: usize,
    old_value_bytes: Option<usize>,
    id_json_bytes: usize,
    new_value_bytes: usize,
    unique_ids: u64,
    limit: usize,
) -> Result<usize, ItemRefusal> {
    let next = if let Some(old) = old_value_bytes {
        old_total
            .checked_sub(old)
            .and_then(|base| base.checked_add(new_value_bytes))
            .ok_or(ItemRefusal::Budget)?
    } else {
        old_total
            .checked_add(event_json_addition(
                id_json_bytes,
                new_value_bytes,
                unique_ids != 0,
            )?)
            .ok_or(ItemRefusal::Budget)?
    };
    if next > limit {
        return Err(ItemRefusal::Budget);
    }
    Ok(next)
}

fn query_event_value(
    context: ProviderContext<'_, '_, '_>,
    db: &PinnedSqliteConnection,
    table: &'static str,
    id: &str,
    json_limit: usize,
    biblio_budget: Option<&BiblioQueryBudget>,
) -> Result<Option<Value>, ItemRefusal> {
    context.check()?;
    if table == "biblio_events" {
        biblio_budget.ok_or_else(source_refusal)?.charge()?;
    }
    let query = match table {
        "default_events" => "SELECT id,value FROM default_events WHERE id=?1",
        "biblio_events" => "SELECT id,value FROM biblio_events WHERE id=?1",
        _ => return Err(source_refusal()),
    };
    let result = db
        .query_row(query, [id], |row| {
            let id_text =
                bounded_row_text_precharged(context, row, 0, context.operation_state_limit)?;
            if id_text != id {
                return Err(rusqlite::Error::InvalidQuery);
            }
            value_from_row(
                context,
                row,
                1,
                json_limit,
                estimate_string_state(&id_text).map_err(|_| rusqlite::Error::InvalidQuery)?,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)
        })
        .optional()
        .map_err(sql_refusal)?;
    context.check()?;
    Ok(result)
}

fn event_for_each(
    context: ProviderContext<'_, '_, '_>,
    db: &PinnedSqliteConnection,
    table: &'static str,
    json_limit: usize,
    biblio_budget: Option<&BiblioQueryBudget>,
    visit: &mut dyn FnMut(&str, &Value) -> Result<(), ItemRefusal>,
) -> Result<(), ItemRefusal> {
    let mut after: Option<String> = None;
    loop {
        context.check()?;
        if table == "biblio_events" {
            biblio_budget.ok_or_else(source_refusal)?.charge()?;
        }
        let row = match table {
            "default_events" => db.query_row(
                "SELECT id,value FROM default_events WHERE (?1 IS NULL OR id>?1) ORDER BY id COLLATE BINARY LIMIT 1",
                [after.as_deref()],
                |row| {
                    let id = bounded_row_text_precharged(context, row, 0, context.operation_state_limit)?;
                    let id_state = estimate_string_state(&id).map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let value = value_from_row(context, row, 1, json_limit, id_state)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok((id, value))
                },
            ),
            "biblio_events" => db.query_row(
                "SELECT id,value FROM biblio_events WHERE (?1 IS NULL OR id>?1) ORDER BY id COLLATE BINARY LIMIT 1",
                [after.as_deref()],
                |row| {
                    let id = bounded_row_text_precharged(context, row, 0, context.operation_state_limit)?;
                    let id_state = estimate_string_state(&id).map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let value = value_from_row(context, row, 1, json_limit, id_state)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok((id, value))
                },
            ),
            _ => return Err(source_refusal()),
        }
        .optional()
        .map_err(sql_refusal)?;
        context.check()?;
        let Some((id, value)) = row else {
            return Ok(());
        };
        let state = checked_add(estimate_string_state(&id)?, estimate_value_state(&value)?)?;
        context.active_state(state)?;
        visit(&id, &value)?;
        after = Some(id);
    }
}

struct DefaultEventsProvider<'a, 'candidate, 'host, 'cancel> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    scan_rows: &'a Cell<u64>,
    limits: SpoolIndexLimits,
    state: &'a mut EventProjectionState,
    active_page_state: Cell<usize>,
    json_ceiling: usize,
}

impl DefaultEventsProvider<'_, '_, '_, '_> {
    fn insertion_workspace(
        &self,
        id: &str,
        value: &Value,
        json_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        let id_state = estimate_string_state(id)?;
        let value_state = estimate_value_state(value)?;
        let encoded_state = json_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<u8>>() + 1024))
            .ok_or(ItemRefusal::Budget)?;
        checked_add(checked_add(id_state, value_state)?, encoded_state)
    }

    fn value_length(&self, id: &str) -> Result<Option<usize>, ItemRefusal> {
        self.context.check()?;
        let bytes: Option<i64> = self
            .db
            .query_row(
                "SELECT length(value) FROM default_events WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_refusal)?;
        self.context.check()?;
        bytes
            .map(|n| usize::try_from(n).map_err(|_| source_refusal()))
            .transpose()
    }
}

impl SourceFoundationDefaultEventLookup for DefaultEventsProvider<'_, '_, '_, '_> {
    fn event(&self, id: &str) -> Result<Option<Cow<'_, Value>>, ItemRefusal> {
        let id_state = estimate_string_state(id)?;
        query_event_value(
            self.context,
            self.db,
            "default_events",
            id,
            self.limits.max_row_state_bytes,
            None,
        )
        .map(|value| value.map(Cow::Owned))
        .and_then(|value| {
            if value.is_some() {
                self.context.active_state(id_state)?;
            }
            Ok(value)
        })
    }

    fn event_contains(&self, id: &str) -> Result<bool, ItemRefusal> {
        let workspace = estimate_string_state(id)?
            .checked_add(256)
            .ok_or(ItemRefusal::Budget)?;
        self.context.row_state(workspace)?;
        self.context.add_scan_rows(self.scan_rows, 1)?;
        self.context.check()?;
        let mut statement = self
            .db
            .prepare("SELECT id FROM default_events WHERE id=?1")
            .map_err(sql_refusal)?;
        let mut rows = statement.query([id]).map_err(sql_refusal)?;
        let found = if let Some(row) = rows.next().map_err(sql_refusal)? {
            match row.get_ref(0).map_err(sql_refusal)? {
                rusqlite::types::ValueRef::Text(raw) if raw == id.as_bytes() => true,
                _ => return Err(source_refusal()),
            }
        } else {
            false
        };
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        self.context.check()?;
        Ok(found)
    }

    fn for_each_event(
        &self,
        visit: &mut dyn FnMut(&str, &Value) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        event_for_each(
            self.context,
            self.db,
            "default_events",
            self.limits.max_row_state_bytes,
            None,
            visit,
        )
    }
}

impl SourceFoundationDefaultEventStore for DefaultEventsProvider<'_, '_, '_, '_> {
    fn insert_event(
        &mut self,
        id: &str,
        value: &Value,
        max_json_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        self.context.check()?;
        if max_state_bytes == 0
            || max_state_bytes > self.context.operation_state_limit
            || max_json_bytes < 2
            || max_json_bytes > self.json_ceiling
        {
            return Err(ItemRefusal::Budget);
        }
        let json_bytes = json_len(value, max_json_bytes)?;
        let workspace = self.insertion_workspace(id, value, json_bytes)?;
        check_event_operation(
            self.context,
            self.active_page_state.get(),
            self.state.retained_state_bytes,
            workspace,
        )?;
        if workspace > max_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        let old_bytes = self.value_length(id)?;
        let id_json_bytes = json_len(&id, max_json_bytes)?;
        let merged = next_event_json_total(
            self.state.merged_json_bytes,
            old_bytes,
            id_json_bytes,
            json_bytes,
            self.state.unique_ids,
            max_json_bytes,
        )?;
        let encoded = encoded_json(value, json_bytes, max_state_bytes)?;
        let ordinal = self.state.observation_rows;
        let next_observations = ordinal.checked_add(1).ok_or(ItemRefusal::Budget)?;
        let slot = ordinal.to_be_bytes();
        self.context.check()?;
        if old_bytes.is_some() {
            self.db
                .execute(
                    "UPDATE default_events SET value=?2 WHERE id=?1",
                    params![id, encoded],
                )
                .map_err(sql_refusal)?;
        } else {
            self.db
                .execute(
                    "INSERT INTO default_events(slot,id,value) VALUES(?1,?2,?3)",
                    params![slot.as_slice(), id, encoded],
                )
                .map_err(sql_refusal)?;
            self.state.unique_ids = self
                .state
                .unique_ids
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        self.context.check()?;
        self.state.observation_rows = next_observations;
        self.state.last_source_ordinal = Some(ordinal);
        self.state.merged_json_bytes = merged;
        let simultaneous_workspace = checked_add(self.active_page_state.get(), workspace)?;
        self.state.workspace_peak_bytes =
            self.state.workspace_peak_bytes.max(simultaneous_workspace);
        Ok(())
    }

    fn cost(&self) -> Result<SourceFoundationDefaultEventStoreCost, ItemRefusal> {
        self.context.check()?;
        let retained_state_bytes = self.state.retained_state_bytes;
        let workspace_state_bytes = self.state.workspace_peak_bytes;
        Ok(SourceFoundationDefaultEventStoreCost {
            retained_state_bytes,
            workspace_state_bytes,
            merged_event_json_bytes: self.state.merged_json_bytes,
        })
    }

    fn event_lookup(&self) -> &dyn SourceFoundationDefaultEventLookup {
        self
    }
}

struct CandidateDiscoverySeenIds<'a, 'candidate, 'host, 'cancel, 'budget> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    scan_rows: &'budget Cell<u64>,
}

impl CandidateDiscoverySeenIds<'_, '_, '_, '_, '_> {
    fn preflight(
        &self,
        namespace: DiscoverySeenIdNamespace,
        id: &str,
        first_path: Option<&str>,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        if max_state_bytes == 0 || max_state_bytes > self.context.operation_state_limit {
            return Err(ItemRefusal::Budget);
        }
        let input_bytes = namespace
            .storage_key()
            .len()
            .checked_add(id.len())
            .and_then(|bytes| bytes.checked_add(first_path.map_or(0, str::len)))
            .ok_or(ItemRefusal::Budget)?;
        let workspace = input_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<(String, String, String)>() + 256))
            .ok_or(ItemRefusal::Budget)?;
        if workspace > max_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.context.row_state(workspace)?;
        self.context.check()?;
        Ok(workspace)
    }

    fn verify_existing_first_path(
        &self,
        namespace: DiscoverySeenIdNamespace,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        let workspace = self.preflight(namespace, id, None, max_state_bytes)?;
        self.context.add_scan_rows(self.scan_rows, 1)?;
        let mut statement = self.db.prepare(
            "SELECT length(first_path) FROM sf_discovery_seen_ids WHERE namespace=?1 AND key=?2",
        )
        .map_err(sql_refusal)?;
        let mut rows = statement
            .query(params![namespace.storage_key(), id])
            .map_err(sql_refusal)?;
        let first_path_len = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| row.get::<_, i64>(0).map_err(sql_refusal))
            .transpose()?
            .ok_or_else(source_refusal)?;
        if first_path_len <= 0 || rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        self.context.check()?;
        Ok(workspace)
    }
}

impl DiscoverySeenIds for CandidateDiscoverySeenIds<'_, '_, '_, '_, '_> {
    fn contains(
        &mut self,
        namespace: DiscoverySeenIdNamespace,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal> {
        let workspace = self.preflight(namespace, id, None, max_state_bytes)?;
        self.context.add_scan_rows(self.scan_rows, 1)?;
        let mut statement = self
            .db
            .prepare(
                "SELECT length(first_path) FROM sf_discovery_seen_ids WHERE namespace=?1 AND key=?2",
            )
            .map_err(sql_refusal)?;
        let mut rows = statement
            .query(params![namespace.storage_key(), id])
            .map_err(sql_refusal)?;
        let found = if let Some(row) = rows.next().map_err(sql_refusal)? {
            if row.get::<_, i64>(0).map_err(sql_refusal)? <= 0 {
                return Err(source_refusal());
            }
            true
        } else {
            false
        };
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        self.context.check()?;
        Ok((found, workspace))
    }

    fn remember_first(
        &mut self,
        namespace: DiscoverySeenIdNamespace,
        id: &str,
        first_path: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal> {
        if first_path.is_empty() {
            return Err(source_refusal());
        }
        let workspace = self.preflight(namespace, id, Some(first_path), max_state_bytes)?;
        self.context.add_scan_rows(self.scan_rows, 1)?;
        let changed = self
            .db
            .execute(
                "INSERT OR IGNORE INTO sf_discovery_seen_ids(namespace,key,first_path) VALUES(?1,?2,?3)",
                params![namespace.storage_key(), id, first_path],
            )
            .map_err(sql_refusal)?;
        self.context.check()?;
        if changed == 1 {
            return Ok((true, workspace));
        }
        if changed != 0 {
            return Err(source_refusal());
        }
        let verify_workspace = self.verify_existing_first_path(namespace, id, max_state_bytes)?;
        Ok((false, workspace.max(verify_workspace)))
    }
}

struct CandidateDiscoveryRunSummaries<'a, 'candidate, 'host, 'cancel, 'budget> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    scan_rows: &'budget Cell<u64>,
    observation_rows: u64,
    unique_paths: u64,
    serialized_summary_write_bytes: u64,
    serialized_summary_read_bytes: u64,
    scan_row_operations: u64,
    workspace_peak_bytes: usize,
    finished: bool,
}

/// Invocation-local codec for a derived Discovery lookup projection. These
/// bytes are scratch rebuilt from the current source cut; they are not a
/// serialized owner record or an admission proof.
#[derive(serde::Serialize, serde::Deserialize)]
struct CandidateDiscoveryRunSummaryStoredV1 {
    version: u8,
    target_kind: String,
    known_refs: std::collections::BTreeSet<String>,
    captured_acquisitions: std::collections::BTreeSet<(String, String, String, u64)>,
}

#[derive(serde::Serialize)]
struct CandidateDiscoveryRunSummaryCodecV1<'a> {
    version: u8,
    target_kind: &'a str,
    known_refs: &'a std::collections::BTreeSet<String>,
    captured_acquisitions: &'a std::collections::BTreeSet<(String, String, String, u64)>,
}

impl<'a> From<&'a DiscoveryRunSummary> for CandidateDiscoveryRunSummaryCodecV1<'a> {
    fn from(summary: &DiscoveryRunSummary) -> Self {
        Self {
            version: 1,
            target_kind: &summary.target_kind,
            known_refs: &summary.known_refs,
            captured_acquisitions: &summary.captured_acquisitions,
        }
    }
}

impl TryFrom<CandidateDiscoveryRunSummaryStoredV1> for DiscoveryRunSummary {
    type Error = ItemRefusal;

    fn try_from(codec: CandidateDiscoveryRunSummaryStoredV1) -> Result<Self, Self::Error> {
        if codec.version != 1 {
            return Err(source_refusal());
        }
        Ok(Self {
            target_kind: codec.target_kind,
            known_refs: codec.known_refs,
            captured_acquisitions: codec.captured_acquisitions,
        })
    }
}

impl CandidateDiscoveryRunSummaries<'_, '_, '_, '_, '_> {
    fn path_workspace(path: &str) -> Result<usize, ItemRefusal> {
        estimate_string_state(path)?
            .checked_add(size_of::<(String, Vec<u8>)>() + 512)
            .ok_or(ItemRefusal::Budget)
    }

    fn encoded_summary_workspace(path: &str, encoded_bytes: usize) -> Result<usize, ItemRefusal> {
        encoded_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(Self::path_workspace(path).ok()?))
            .ok_or(ItemRefusal::Budget)
    }

    fn decoded_summary_workspace(path: &str, encoded_bytes: usize) -> Result<usize, ItemRefusal> {
        json_state_upper_bound(encoded_bytes)?
            .checked_add(encoded_bytes.checked_mul(2).ok_or(ItemRefusal::Budget)?)
            .and_then(|bytes| bytes.checked_add(Self::path_workspace(path).ok()?))
            .ok_or(ItemRefusal::Budget)
    }

    fn preflight(&self, workspace: usize, max_state_bytes: usize) -> Result<(), ItemRefusal> {
        if max_state_bytes == 0
            || max_state_bytes > self.context.operation_state_limit
            || workspace > max_state_bytes
        {
            return Err(ItemRefusal::Budget);
        }
        self.context.row_state(workspace)?;
        self.context.check()
    }

    fn charge_scan_rows(&mut self, rows: usize) -> Result<(), ItemRefusal> {
        self.context.add_scan_rows(self.scan_rows, rows)?;
        self.scan_row_operations = self
            .scan_row_operations
            .checked_add(usize_u64(rows)?)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
}

impl DiscoveryRunSummaryStore for CandidateDiscoveryRunSummaries<'_, '_, '_, '_, '_> {
    fn insert_summary(
        &mut self,
        path: &str,
        summary: &DiscoveryRunSummary,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        if self.finished || path.is_empty() {
            return Err(source_refusal());
        }
        self.preflight(Self::path_workspace(path)?, max_state_bytes)?;
        RelativePath::new(path).map_err(|_| source_refusal())?;
        self.context.check()?;
        let codec = CandidateDiscoveryRunSummaryCodecV1::from(summary);
        let encoded_bytes = json_len(&codec, max_state_bytes)?;
        self.context.check()?;
        if encoded_bytes == 0 {
            return Err(source_refusal());
        }
        let workspace = Self::encoded_summary_workspace(path, encoded_bytes)?;
        self.preflight(workspace, max_state_bytes)?;
        let value = encoded_json(&codec, encoded_bytes, max_state_bytes)?;
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let inserted = self
            .db
            .execute(
                "INSERT OR IGNORE INTO sf_discovery_run_summaries(path,value) VALUES(?1,?2)",
                params![path, &value],
            )
            .map_err(sql_refusal)?;
        self.context.check()?;
        match inserted {
            1 => {
                self.unique_paths = self
                    .unique_paths
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
            0 => {
                self.charge_scan_rows(1)?;
                let updated = self
                    .db
                    .execute(
                        "UPDATE sf_discovery_run_summaries SET value=?2 WHERE path=?1",
                        params![path, &value],
                    )
                    .map_err(sql_refusal)?;
                if updated != 1 {
                    return Err(source_refusal());
                }
                self.context.check()?;
            }
            _ => return Err(source_refusal()),
        }
        self.observation_rows = self
            .observation_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.serialized_summary_write_bytes = self
            .serialized_summary_write_bytes
            .checked_add(u64::try_from(value.len()).map_err(|_| ItemRefusal::Budget)?)
            .ok_or(ItemRefusal::Budget)?;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok(workspace)
    }

    fn lookup_summary(
        &mut self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<DiscoveryRunSummary>, usize), ItemRefusal> {
        if self.finished || path.is_empty() {
            return Err(source_refusal());
        }
        let probe_workspace = Self::path_workspace(path)?;
        self.preflight(probe_workspace, max_state_bytes)?;
        RelativePath::new(path).map_err(|_| source_refusal())?;
        self.charge_scan_rows(1)?;
        let mut statement = self
            .db
            .prepare("SELECT length(value) FROM sf_discovery_run_summaries WHERE path=?1")
            .map_err(sql_refusal)?;
        let mut rows = statement.query(params![path]).map_err(sql_refusal)?;
        let encoded_bytes = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| row.get::<_, i64>(0).map_err(sql_refusal))
            .transpose()?;
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        let Some(encoded_bytes) = encoded_bytes else {
            self.workspace_peak_bytes = self.workspace_peak_bytes.max(probe_workspace);
            return Ok((None, probe_workspace));
        };
        let encoded_bytes = usize::try_from(encoded_bytes).map_err(|_| source_refusal())?;
        if encoded_bytes == 0 {
            return Err(source_refusal());
        }
        let workspace = Self::decoded_summary_workspace(path, encoded_bytes)?;
        self.preflight(workspace, max_state_bytes)?;
        self.charge_scan_rows(1)?;
        let mut statement = self
            .db
            .prepare("SELECT value FROM sf_discovery_run_summaries WHERE path=?1")
            .map_err(sql_refusal)?;
        let mut rows = statement.query(params![path]).map_err(sql_refusal)?;
        let value = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| match row.get_ref(0)? {
                rusqlite::types::ValueRef::Blob(raw) if raw.len() == encoded_bytes => {
                    Ok(raw.to_vec())
                }
                _ => Err(rusqlite::Error::InvalidQuery),
            })
            .transpose()
            .map_err(sql_refusal)?
            .ok_or_else(source_refusal)?;
        self.serialized_summary_read_bytes = self
            .serialized_summary_read_bytes
            .checked_add(u64::try_from(value.len()).map_err(|_| ItemRefusal::Budget)?)
            .ok_or(ItemRefusal::Budget)?;
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        let codec = serde_json::from_slice::<CandidateDiscoveryRunSummaryStoredV1>(&value)
            .map_err(|_| source_refusal())?;
        let summary = DiscoveryRunSummary::try_from(codec)?;
        let canonical_codec = CandidateDiscoveryRunSummaryCodecV1::from(&summary);
        let canonical_len = json_len(&canonical_codec, encoded_bytes)?;
        let canonical = encoded_json(&canonical_codec, canonical_len, max_state_bytes)?;
        if canonical.as_slice() != value.as_slice() {
            return Err(source_refusal());
        }
        self.context.check()?;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok((Some(summary), workspace))
    }

    fn finish(
        &mut self,
        expected_rows: u64,
        max_state_bytes: usize,
    ) -> Result<DiscoveryRunSummaryStoreCost, ItemRefusal> {
        if self.finished || self.observation_rows != expected_rows {
            return Err(source_refusal());
        }
        let workspace = size_of::<i64>() + 256;
        self.preflight(workspace, max_state_bytes)?;
        let count_scan_rows = usize::try_from(self.unique_paths)
            .map_err(|_| ItemRefusal::Budget)?
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.charge_scan_rows(count_scan_rows)?;
        let mut statement = self
            .db
            .prepare("SELECT count(*) FROM sf_discovery_run_summaries")
            .map_err(sql_refusal)?;
        let mut rows = statement.query([]).map_err(sql_refusal)?;
        let actual_count = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| row.get::<_, i64>(0).map_err(sql_refusal))
            .transpose()?
            .ok_or_else(source_refusal)?;
        if actual_count < 0
            || u64::try_from(actual_count).map_err(|_| source_refusal())? != self.unique_paths
            || rows.next().map_err(sql_refusal)?.is_some()
        {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        self.finished = true;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok(DiscoveryRunSummaryStoreCost {
            observation_rows: self.observation_rows,
            unique_paths: self.unique_paths,
            serialized_summary_write_bytes: self.serialized_summary_write_bytes,
            serialized_summary_read_bytes: self.serialized_summary_read_bytes,
            workspace_state_bytes: self.workspace_peak_bytes,
            scan_row_operations: self.scan_row_operations,
        })
    }
}

struct CandidateDiscoveryEventSummaries<'a, 'candidate, 'host, 'cancel, 'budget> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    scan_rows: &'budget Cell<u64>,
    observation_rows: u64,
    owner_insertion_rows: u64,
    serialized_write_bytes: u64,
    serialized_read_bytes: u64,
    scan_row_operations: u64,
    workspace_peak_bytes: usize,
    finished: bool,
    drained: bool,
}

impl CandidateDiscoveryEventSummaries<'_, '_, '_, '_, '_> {
    fn workspace_for_stored_value(
        id_bytes: usize,
        location_bytes: usize,
        encoded_value_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        let id_state = id_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<String>() + 32))
            .ok_or(ItemRefusal::Budget)?;
        let location_state = location_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<String>() + 32))
            .ok_or(ItemRefusal::Budget)?;
        id_state
            .checked_add(location_state)
            .and_then(|bytes| bytes.checked_add(encoded_value_bytes))
            .and_then(|bytes| bytes.checked_add(json_state_upper_bound(encoded_value_bytes).ok()?))
            .and_then(|bytes| bytes.checked_add(size_of::<Value>() + 512))
            .ok_or(ItemRefusal::Budget)
    }

    fn preflight(&self, workspace: usize, max_state_bytes: usize) -> Result<(), ItemRefusal> {
        if max_state_bytes == 0
            || max_state_bytes > self.context.operation_state_limit
            || workspace > max_state_bytes
        {
            return Err(ItemRefusal::Budget);
        }
        self.context.row_state(workspace)?;
        self.context.check()
    }

    fn charge_scan_rows(&mut self, rows: usize) -> Result<(), ItemRefusal> {
        self.context.add_scan_rows(self.scan_rows, rows)?;
        self.scan_row_operations = self
            .scan_row_operations
            .checked_add(usize_u64(rows)?)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn cost(&self) -> DiscoveryEventSummaryStoreCost {
        DiscoveryEventSummaryStoreCost {
            observation_rows: self.observation_rows,
            owner_insertion_rows: self.owner_insertion_rows,
            serialized_write_bytes: self.serialized_write_bytes,
            serialized_read_bytes: self.serialized_read_bytes,
            workspace_state_bytes: self.workspace_peak_bytes,
            scan_row_operations: self.scan_row_operations,
        }
    }
}

impl DiscoveryEventSummaryStore for CandidateDiscoveryEventSummaries<'_, '_, '_, '_, '_> {
    fn record_event(
        &mut self,
        namespace: DiscoveryEventSummaryNamespace,
        id: &str,
        location: &str,
        value: &Value,
        insert_into_owner_map: bool,
        max_serialized_value_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        if self.finished || id.is_empty() || location.is_empty() || max_serialized_value_bytes < 2 {
            return Err(source_refusal());
        }
        let encoded_value_bytes = json_len(value, max_serialized_value_bytes)?;
        if encoded_value_bytes == 0 {
            return Err(source_refusal());
        }
        let workspace = estimate_string_state(id)?
            .checked_add(estimate_string_state(location)?)
            .and_then(|bytes| bytes.checked_add(estimate_value_state(value).ok()?))
            .and_then(|bytes| {
                bytes.checked_add(
                    encoded_value_bytes
                        .checked_mul(2)?
                        .checked_add(size_of::<Vec<u8>>() + size_of::<[u8; 8]>() + 1024)?,
                )
            })
            .ok_or(ItemRefusal::Budget)?;
        self.preflight(workspace, max_state_bytes)?;
        let encoded = encoded_json(value, encoded_value_bytes, max_state_bytes)?;
        let ordinal = self.observation_rows.to_be_bytes();
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let inserted = self
            .db
            .execute(
                "INSERT INTO sf_discovery_event_summaries(ordinal,namespace,id,location,value,owner_insert) VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    ordinal.as_slice(),
                    namespace.storage_key(),
                    id,
                    location,
                    &encoded,
                    i64::from(insert_into_owner_map),
                ],
            )
            .map_err(sql_refusal)?;
        self.context.check()?;
        if inserted != 1 {
            return Err(source_refusal());
        }
        self.observation_rows = self
            .observation_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        if insert_into_owner_map {
            self.owner_insertion_rows = self
                .owner_insertion_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        let logical_bytes = id
            .len()
            .checked_add(location.len())
            .and_then(|bytes| bytes.checked_add(encoded.len()))
            .ok_or(ItemRefusal::Budget)?;
        self.serialized_write_bytes = self
            .serialized_write_bytes
            .checked_add(usize_u64(logical_bytes)?)
            .ok_or(ItemRefusal::Budget)?;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok(workspace)
    }

    fn lookup_event(
        &mut self,
        namespace: DiscoveryEventSummaryNamespace,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<(String, Value)>, usize), ItemRefusal> {
        if self.finished {
            return Err(source_refusal());
        }
        let probe_workspace = estimate_string_state(id)?
            .checked_add(size_of::<([u8; 8], i64, i64)>() + 256)
            .ok_or(ItemRefusal::Budget)?;
        self.preflight(probe_workspace, max_state_bytes)?;
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let mut statement = self.db.prepare(
            "SELECT ordinal,length(location),length(value) FROM sf_discovery_event_summaries WHERE namespace=?1 AND id=?2 ORDER BY ordinal DESC LIMIT 1",
        ).map_err(sql_refusal)?;
        let mut rows = statement
            .query(params![namespace.storage_key(), id])
            .map_err(sql_refusal)?;
        let metadata = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| {
                let ordinal = row_blob(row, 0)?.to_vec();
                let location_bytes = row.get::<_, i64>(1)?;
                let value_bytes = row.get::<_, i64>(2)?;
                Ok((ordinal, location_bytes, value_bytes))
            })
            .transpose()
            .map_err(sql_refusal)?;
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        let Some((ordinal, location_bytes, value_bytes)) = metadata else {
            self.workspace_peak_bytes = self.workspace_peak_bytes.max(probe_workspace);
            return Ok((None, probe_workspace));
        };
        let location_bytes = usize::try_from(location_bytes).map_err(|_| source_refusal())?;
        let value_bytes = usize::try_from(value_bytes).map_err(|_| source_refusal())?;
        if ordinal.len() != 8 || location_bytes == 0 || value_bytes == 0 {
            return Err(source_refusal());
        }
        let workspace = probe_workspace.max(Self::workspace_for_stored_value(
            id.len(),
            location_bytes,
            value_bytes,
        )?);
        self.preflight(workspace, max_state_bytes)?;
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let mut statement = self.db.prepare(
            "SELECT namespace,id,location,value FROM sf_discovery_event_summaries WHERE ordinal=?1",
        ).map_err(sql_refusal)?;
        let mut rows = statement.query([ordinal.as_slice()]).map_err(sql_refusal)?;
        let stored = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| {
                let stored_namespace = bounded_row_text(row, 0, 32)?;
                let stored_id = bounded_row_text(row, 1, id.len())?;
                let location = bounded_row_text(row, 2, location_bytes)?;
                let raw = row_blob(row, 3)?;
                if stored_namespace != namespace.storage_key()
                    || stored_id != id
                    || location.len() != location_bytes
                    || raw.len() != value_bytes
                {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Ok((location, raw.to_vec()))
            })
            .transpose()
            .map_err(sql_refusal)?
            .ok_or_else(source_refusal)?;
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        let (location, raw) = stored;
        let value = serde_json::from_slice::<Value>(&raw).map_err(|_| source_refusal())?;
        if !value.is_object() || value.get("event_id").and_then(Value::as_str) != Some(id) {
            return Err(source_refusal());
        }
        self.serialized_read_bytes = self
            .serialized_read_bytes
            .checked_add(usize_u64(
                id.len()
                    .checked_add(location.len())
                    .and_then(|bytes| bytes.checked_add(raw.len()))
                    .ok_or(ItemRefusal::Budget)?,
            )?)
            .ok_or(ItemRefusal::Budget)?;
        self.context.check()?;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok((Some((location, value)), workspace))
    }

    fn finish(
        &mut self,
        expected_observation_rows: u64,
        expected_owner_insertion_rows: u64,
        max_state_bytes: usize,
    ) -> Result<DiscoveryEventSummaryStoreCost, ItemRefusal> {
        if self.finished
            || self.observation_rows != expected_observation_rows
            || self.owner_insertion_rows != expected_owner_insertion_rows
        {
            return Err(source_refusal());
        }
        let workspace = size_of::<(i64, i64)>() + 256;
        self.preflight(workspace, max_state_bytes)?;
        let scan_rows = usize::try_from(self.observation_rows)
            .map_err(|_| ItemRefusal::Budget)?
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.charge_scan_rows(scan_rows)?;
        let mut statement = self
            .db
            .prepare(
                "SELECT count(*),COALESCE(sum(owner_insert),0) FROM sf_discovery_event_summaries",
            )
            .map_err(sql_refusal)?;
        let mut rows = statement.query([]).map_err(sql_refusal)?;
        let actual = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| {
                Ok((
                    row.get::<_, i64>(0).map_err(sql_refusal)?,
                    row.get::<_, i64>(1).map_err(sql_refusal)?,
                ))
            })
            .transpose()?
            .ok_or_else(source_refusal)?;
        if actual.0 < 0
            || actual.1 < 0
            || u64::try_from(actual.0).map_err(|_| source_refusal())? != self.observation_rows
            || u64::try_from(actual.1).map_err(|_| source_refusal())? != self.owner_insertion_rows
            || rows.next().map_err(sql_refusal)?.is_some()
        {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        self.finished = true;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok(self.cost())
    }

    fn for_each_owner_insertion(
        &mut self,
        remaining_state_bytes: &mut dyn FnMut() -> Result<usize, ItemRefusal>,
        visit: &mut dyn FnMut(&str, &Value, usize) -> Result<(), ItemRefusal>,
    ) -> Result<DiscoveryEventSummaryStoreCost, ItemRefusal> {
        if !self.finished || self.drained {
            return Err(source_refusal());
        }
        let mut after: Option<Vec<u8>> = None;
        let mut visited = 0u64;
        loop {
            let metadata_workspace = size_of::<([u8; 8], i64, i64, i64)>() + 256;
            let available = remaining_state_bytes()?;
            self.preflight(metadata_workspace, available)?;
            self.context.check()?;
            self.charge_scan_rows(1)?;
            let sql = if after.is_some() {
                "SELECT ordinal,length(id),length(location),length(value) FROM sf_discovery_event_summaries WHERE owner_insert=1 AND ordinal>?1 ORDER BY ordinal LIMIT 1"
            } else {
                "SELECT ordinal,length(id),length(location),length(value) FROM sf_discovery_event_summaries WHERE owner_insert=1 ORDER BY ordinal LIMIT 1"
            };
            let mut statement = self.db.prepare(sql).map_err(sql_refusal)?;
            let mut rows = if let Some(ordinal) = after.as_deref() {
                statement.query([ordinal]).map_err(sql_refusal)?
            } else {
                statement.query([]).map_err(sql_refusal)?
            };
            let metadata = rows
                .next()
                .map_err(sql_refusal)?
                .map(|row| {
                    Ok((
                        row_blob(row, 0)?.to_vec(),
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .transpose()
                .map_err(sql_refusal)?;
            if rows.next().map_err(sql_refusal)?.is_some() {
                return Err(source_refusal());
            }
            drop(rows);
            drop(statement);
            self.context.check()?;
            let Some((ordinal, id_bytes, location_bytes, value_bytes)) = metadata else {
                break;
            };
            let id_bytes = usize::try_from(id_bytes).map_err(|_| source_refusal())?;
            let location_bytes = usize::try_from(location_bytes).map_err(|_| source_refusal())?;
            let value_bytes = usize::try_from(value_bytes).map_err(|_| source_refusal())?;
            if ordinal.len() != 8 || id_bytes == 0 || location_bytes == 0 || value_bytes == 0 {
                return Err(source_refusal());
            }
            let workspace =
                Self::workspace_for_stored_value(id_bytes, location_bytes, value_bytes)?;
            let available = remaining_state_bytes()?;
            self.preflight(workspace, available)?;
            self.context.check()?;
            self.charge_scan_rows(1)?;
            let mut statement = self.db.prepare(
                "SELECT id,location,value FROM sf_discovery_event_summaries WHERE ordinal=?1 AND owner_insert=1",
            ).map_err(sql_refusal)?;
            let mut rows = statement.query([ordinal.as_slice()]).map_err(sql_refusal)?;
            let stored = rows
                .next()
                .map_err(sql_refusal)?
                .map(|row| {
                    let id = bounded_row_text(row, 0, id_bytes)?;
                    let location = bounded_row_text(row, 1, location_bytes)?;
                    let raw = row_blob(row, 2)?;
                    if id.len() != id_bytes
                        || location.len() != location_bytes
                        || raw.len() != value_bytes
                    {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    Ok((id, location, raw.to_vec()))
                })
                .transpose()
                .map_err(sql_refusal)?
                .ok_or_else(source_refusal)?;
            if rows.next().map_err(sql_refusal)?.is_some() {
                return Err(source_refusal());
            }
            drop(rows);
            drop(statement);
            let (id, location, raw) = stored;
            let value = serde_json::from_slice::<Value>(&raw).map_err(|_| source_refusal())?;
            if !value.is_object()
                || value.get("event_id").and_then(Value::as_str) != Some(id.as_str())
            {
                return Err(source_refusal());
            }
            self.serialized_read_bytes = self
                .serialized_read_bytes
                .checked_add(usize_u64(
                    id.len()
                        .checked_add(location.len())
                        .and_then(|bytes| bytes.checked_add(raw.len()))
                        .ok_or(ItemRefusal::Budget)?,
                )?)
                .ok_or(ItemRefusal::Budget)?;
            visit(&id, &value, workspace)?;
            visited = visited.checked_add(1).ok_or(ItemRefusal::Budget)?;
            self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
            after = Some(ordinal);
        }
        if visited != self.owner_insertion_rows {
            return Err(source_refusal());
        }
        self.context.check()?;
        self.drained = true;
        Ok(self.cost())
    }
}

struct CandidateDiscoverySchemaRequests<'a, 'candidate, 'host, 'cancel, 'budget> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    scan_rows: &'budget Cell<u64>,
    observation_rows: u64,
    read_rows: u64,
    serialized_write_bytes: u64,
    serialized_read_bytes: u64,
    scan_row_operations: u64,
    workspace_peak_bytes: usize,
    max_document_bytes: Option<usize>,
    last_before_issue: Option<usize>,
    last_read_before_issue: Option<usize>,
    cursor_ordinal: Option<u64>,
    expected_rows: Option<u64>,
    direct_issue_count: Option<usize>,
    finished: bool,
    drained: bool,
}

impl CandidateDiscoverySchemaRequests<'_, '_, '_, '_, '_> {
    fn row_text_state(bytes: usize) -> Result<usize, ItemRefusal> {
        bytes
            .checked_mul(16)
            .and_then(|state| state.checked_add(2048))
            .ok_or(ItemRefusal::Budget)
    }

    fn request_workspace(
        location_bytes: usize,
        contract_bytes: usize,
        document_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        Self::row_text_state(location_bytes)?
            .checked_add(Self::row_text_state(contract_bytes)?)
            .and_then(|state| state.checked_add(json_state_upper_bound(document_bytes).ok()?))
            .and_then(|state| {
                state.checked_add(
                    document_bytes
                        .checked_mul(2)?
                        .checked_add(size_of::<Vec<u8>>() + size_of::<SchemaRequest>() + 1024)?,
                )
            })
            .ok_or(ItemRefusal::Budget)
    }

    fn preflight(&self, workspace: usize, max_state_bytes: usize) -> Result<(), ItemRefusal> {
        if max_state_bytes == 0
            || max_state_bytes > self.context.operation_state_limit
            || workspace > max_state_bytes
        {
            return Err(ItemRefusal::Budget);
        }
        self.context.row_state(workspace)?;
        self.context.check()
    }

    fn charge_scan_rows(&mut self, rows: usize) -> Result<(), ItemRefusal> {
        self.context.add_scan_rows(self.scan_rows, rows)?;
        self.scan_row_operations = self
            .scan_row_operations
            .checked_add(usize_u64(rows)?)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn cost(&self) -> DiscoverySchemaRequestStoreCost {
        DiscoverySchemaRequestStoreCost {
            observation_rows: self.observation_rows,
            serialized_write_bytes: self.serialized_write_bytes,
            serialized_read_bytes: self.serialized_read_bytes,
            workspace_state_bytes: self.workspace_peak_bytes,
            scan_row_operations: self.scan_row_operations,
        }
    }

    fn verify_drained(&self) -> Result<(), ItemRefusal> {
        if !self.finished
            || !self.drained
            || self.expected_rows != Some(self.observation_rows)
            || self.read_rows != self.observation_rows
        {
            return Err(source_refusal());
        }
        self.context.check()
    }
}

impl DiscoverySchemaRequestStore for CandidateDiscoverySchemaRequests<'_, '_, '_, '_, '_> {
    fn record_request(
        &mut self,
        before_issue: usize,
        location: &str,
        contract: &str,
        document: &Value,
        max_document_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        if self.finished
            || location.is_empty()
            || location.len() > 4096
            || contract.is_empty()
            || contract.len() > 4096
            || max_document_bytes == 0
            || self
                .last_before_issue
                .is_some_and(|previous| previous > before_issue)
            || self
                .max_document_bytes
                .is_some_and(|limit| limit != max_document_bytes)
        {
            return Err(source_refusal());
        }
        let base_workspace = Self::row_text_state(location.len())?
            .checked_add(Self::row_text_state(contract.len())?)
            .and_then(|state| state.checked_add(estimate_value_state(document).ok()?))
            .and_then(|state| state.checked_add(size_of::<SchemaRequest>() + 512))
            .ok_or(ItemRefusal::Budget)?;
        self.preflight(base_workspace, max_state_bytes)?;
        let document_bytes = json_len(document, max_document_bytes)?;
        if document_bytes == 0 {
            return Err(source_refusal());
        }
        let workspace = Self::request_workspace(location.len(), contract.len(), document_bytes)?;
        self.preflight(workspace, max_state_bytes)?;
        let encoded = encoded_json(document, document_bytes, max_state_bytes)?;
        let ordinal = self.observation_rows.to_be_bytes();
        let before_issue = usize_u64(before_issue)?.to_be_bytes();
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let inserted = self
            .db
            .execute(
                "INSERT INTO sf_discovery_schema_requests(ordinal,before_issue,location,contract,document) VALUES(?1,?2,?3,?4,?5)",
                params![
                    ordinal.as_slice(),
                    before_issue.as_slice(),
                    location,
                    contract,
                    encoded.as_slice(),
                ],
            )
            .map_err(sql_refusal)?;
        self.context.check()?;
        if inserted != 1 {
            return Err(source_refusal());
        }
        self.observation_rows = self
            .observation_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.serialized_write_bytes = self
            .serialized_write_bytes
            .checked_add(usize_u64(encoded.len())?)
            .ok_or(ItemRefusal::Budget)?;
        self.max_document_bytes = Some(max_document_bytes);
        self.last_before_issue = Some(
            usize::try_from(u64::from_be_bytes(before_issue)).map_err(|_| ItemRefusal::Budget)?,
        );
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok(workspace)
    }

    fn finish(
        &mut self,
        expected_rows: u64,
        direct_issue_count: usize,
        max_state_bytes: usize,
    ) -> Result<DiscoverySchemaRequestStoreCost, ItemRefusal> {
        if self.finished
            || self.observation_rows != expected_rows
            || self
                .last_before_issue
                .is_some_and(|ordinal| ordinal > direct_issue_count)
            || (expected_rows > 0) != self.max_document_bytes.is_some()
        {
            return Err(source_refusal());
        }
        let workspace = size_of::<(i64, [u8; 8], [u8; 8])>() + 256;
        self.preflight(workspace, max_state_bytes)?;
        let count_scan_rows = usize::try_from(expected_rows)
            .map_err(|_| ItemRefusal::Budget)?
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.charge_scan_rows(count_scan_rows)?;
        let mut statement = self
            .db
            .prepare(
                "SELECT count(*),min(ordinal),max(ordinal),max(before_issue) FROM sf_discovery_schema_requests",
            )
            .map_err(sql_refusal)?;
        let mut rows = statement.query([]).map_err(sql_refusal)?;
        let actual = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| {
                let count = row.get::<_, i64>(0).map_err(sql_refusal)?;
                let min = if count == 0 {
                    None
                } else {
                    Some(row_blob(row, 1).map_err(sql_refusal)?.to_vec())
                };
                let max = if count == 0 {
                    None
                } else {
                    Some(row_blob(row, 2).map_err(sql_refusal)?.to_vec())
                };
                let last_issue = if count == 0 {
                    None
                } else {
                    Some(row_blob(row, 3).map_err(sql_refusal)?.to_vec())
                };
                Ok((count, min, max, last_issue))
            })
            .transpose()?
            .ok_or_else(source_refusal)?;
        let expected_last_ordinal = expected_rows.checked_sub(1);
        let expected_last_issue = self
            .last_before_issue
            .map(usize_u64)
            .transpose()?
            .map(u64::to_be_bytes)
            .map(|bytes| bytes.to_vec());
        if rows.next().map_err(sql_refusal)?.is_some()
            || actual.0 < 0
            || u64::try_from(actual.0).map_err(|_| source_refusal())? != expected_rows
            || actual.1.as_deref().map(checked_u64_blob).transpose()?
                != Some(0).filter(|_| expected_rows > 0)
            || actual.2.as_deref().map(checked_u64_blob).transpose()? != expected_last_ordinal
            || actual.3 != expected_last_issue
        {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        self.expected_rows = Some(expected_rows);
        self.direct_issue_count = Some(direct_issue_count);
        self.finished = true;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok(self.cost())
    }

    fn next_request(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<DiscoverySchemaRequest>, usize), ItemRefusal> {
        if !self.finished || self.drained || self.read_rows > self.observation_rows {
            return Err(source_refusal());
        }
        let metadata_workspace = size_of::<([u8; 8], i64, i64, i64)>() + 256;
        self.preflight(metadata_workspace, max_state_bytes)?;
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let sql = if self.cursor_ordinal.is_some() {
            "SELECT ordinal,length(location),length(contract),length(document) FROM sf_discovery_schema_requests WHERE ordinal>?1 ORDER BY ordinal LIMIT 1"
        } else {
            "SELECT ordinal,length(location),length(contract),length(document) FROM sf_discovery_schema_requests ORDER BY ordinal LIMIT 1"
        };
        let mut statement = self.db.prepare(sql).map_err(sql_refusal)?;
        let mut rows = if let Some(after) = self.cursor_ordinal {
            statement
                .query([after.to_be_bytes().as_slice()])
                .map_err(sql_refusal)?
        } else {
            statement.query([]).map_err(sql_refusal)?
        };
        let metadata = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| {
                Ok((
                    row_blob(row, 0)?.to_vec(),
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .transpose()
            .map_err(sql_refusal)?;
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        let Some((ordinal, location_bytes, contract_bytes, document_bytes)) = metadata else {
            if self.read_rows != self.observation_rows {
                return Err(source_refusal());
            }
            self.drained = true;
            self.workspace_peak_bytes = self.workspace_peak_bytes.max(metadata_workspace);
            return Ok((None, metadata_workspace));
        };
        let ordinal = checked_u64_blob(&ordinal)?;
        let expected_ordinal = self.read_rows;
        let location_bytes = usize::try_from(location_bytes).map_err(|_| source_refusal())?;
        let contract_bytes = usize::try_from(contract_bytes).map_err(|_| source_refusal())?;
        let document_bytes = usize::try_from(document_bytes).map_err(|_| source_refusal())?;
        let max_document_bytes = self.max_document_bytes.ok_or_else(source_refusal)?;
        if ordinal != expected_ordinal
            || self
                .cursor_ordinal
                .is_some_and(|previous| ordinal <= previous)
            || location_bytes == 0
            || location_bytes > 4096
            || contract_bytes == 0
            || contract_bytes > 4096
            || document_bytes == 0
            || document_bytes > max_document_bytes
        {
            return Err(source_refusal());
        }
        let workspace = metadata_workspace.max(Self::request_workspace(
            location_bytes,
            contract_bytes,
            document_bytes,
        )?);
        self.preflight(workspace, max_state_bytes)?;
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let mut statement = self.db.prepare(
            "SELECT before_issue,location,contract,document FROM sf_discovery_schema_requests WHERE ordinal=?1",
        ).map_err(sql_refusal)?;
        let mut rows = statement
            .query([ordinal.to_be_bytes().as_slice()])
            .map_err(sql_refusal)?;
        let stored = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| {
                let before_issue = row_blob(row, 0)?.to_vec();
                let location = bounded_row_text(row, 1, workspace)?;
                let contract = bounded_row_text(row, 2, workspace)?;
                let document = match row.get_ref(3)? {
                    rusqlite::types::ValueRef::Blob(raw) if raw.len() == document_bytes => {
                        raw.to_vec()
                    }
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                if before_issue.len() != 8
                    || location.len() != location_bytes
                    || contract.len() != contract_bytes
                {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Ok((before_issue, location, contract, document))
            })
            .transpose()
            .map_err(sql_refusal)?
            .ok_or_else(source_refusal)?;
        if rows.next().map_err(sql_refusal)?.is_some() {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        let (before_issue, location, contract, raw) = stored;
        let before_issue =
            usize::try_from(checked_u64_blob(&before_issue)?).map_err(|_| source_refusal())?;
        let direct_issue_count = self.direct_issue_count.ok_or_else(source_refusal)?;
        if before_issue > direct_issue_count
            || self
                .last_read_before_issue
                .is_some_and(|previous| previous > before_issue)
        {
            return Err(source_refusal());
        }
        let document = serde_json::from_slice::<Value>(&raw).map_err(|_| source_refusal())?;
        self.serialized_read_bytes = self
            .serialized_read_bytes
            .checked_add(usize_u64(raw.len())?)
            .ok_or(ItemRefusal::Budget)?;
        self.read_rows = self.read_rows.checked_add(1).ok_or(ItemRefusal::Budget)?;
        self.last_read_before_issue = Some(before_issue);
        self.cursor_ordinal = Some(ordinal);
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        self.context.check()?;
        Ok((
            Some(DiscoverySchemaRequest {
                before_issue,
                location,
                contract,
                document,
            }),
            workspace,
        ))
    }

    fn cost(&self) -> DiscoverySchemaRequestStoreCost {
        CandidateDiscoverySchemaRequests::cost(self)
    }
}

struct CandidateDiscoveryDigestCache<'a, 'candidate, 'host, 'cancel, 'budget> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    scan_rows: &'budget Cell<u64>,
    observation_rows: u64,
    unique_paths: u64,
    serialized_write_bytes: u64,
    serialized_read_bytes: u64,
    scan_row_operations: u64,
    max_path_bytes: usize,
    workspace_peak_bytes: usize,
    finished: bool,
}

impl CandidateDiscoveryDigestCache<'_, '_, '_, '_, '_> {
    fn row_text_state(bytes: usize) -> Result<usize, ItemRefusal> {
        bytes
            .checked_mul(16)
            .and_then(|state| state.checked_add(2048))
            .ok_or(ItemRefusal::Budget)
    }

    fn valid_digest(value: &str) -> bool {
        Digest256::from_hex(value)
            .map(|digest| digest.to_hex() == value)
            .unwrap_or(false)
    }

    fn preflight(&self, workspace: usize, max_state_bytes: usize) -> Result<(), ItemRefusal> {
        if self.finished
            || max_state_bytes == 0
            || max_state_bytes > self.context.operation_state_limit
            || workspace > max_state_bytes
        {
            return Err(source_refusal());
        }
        self.context.row_state(workspace)?;
        self.context.check()
    }

    fn charge_scan_rows(&mut self, rows: usize) -> Result<(), ItemRefusal> {
        self.context.add_scan_rows(self.scan_rows, rows)?;
        self.scan_row_operations = self
            .scan_row_operations
            .checked_add(usize_u64(rows)?)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn cost(&self) -> DiscoveryDigestCacheCost {
        DiscoveryDigestCacheCost {
            observation_rows: self.observation_rows,
            unique_paths: self.unique_paths,
            serialized_write_bytes: self.serialized_write_bytes,
            serialized_read_bytes: self.serialized_read_bytes,
            workspace_state_bytes: self.workspace_peak_bytes,
            scan_row_operations: self.scan_row_operations,
        }
    }
}

impl DiscoveryDigestCache for CandidateDiscoveryDigestCache<'_, '_, '_, '_, '_> {
    fn lookup_digest(
        &mut self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize), ItemRefusal> {
        let workspace = Self::row_text_state(path.len())?
            .checked_add(Self::row_text_state(64)?)
            .and_then(|state| state.checked_add(size_of::<(String, String)>() + 256))
            .ok_or(ItemRefusal::Budget)?;
        if path.is_empty() {
            return Err(source_refusal());
        }
        self.preflight(workspace, max_state_bytes)?;
        self.context.check()?;
        self.charge_scan_rows(1)?;
        let mut statement = self
            .db
            .prepare("SELECT sha256 FROM sf_discovery_digests WHERE path=?1")
            .map_err(sql_refusal)?;
        let mut rows = statement.query([path]).map_err(sql_refusal)?;
        let value = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| bounded_row_text(row, 0, workspace))
            .transpose()
            .map_err(sql_refusal)?;
        if rows.next().map_err(sql_refusal)?.is_some()
            || value
                .as_deref()
                .is_some_and(|digest| !Self::valid_digest(digest))
        {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        if let Some(value) = &value {
            self.serialized_read_bytes = self
                .serialized_read_bytes
                .checked_add(usize_u64(value.len())?)
                .ok_or(ItemRefusal::Budget)?;
        }
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok((value, workspace))
    }

    fn remember_digest(
        &mut self,
        path: &str,
        digest: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal> {
        let workspace = Self::row_text_state(path.len())?
            .checked_add(Self::row_text_state(digest.len())?)
            .and_then(|state| state.checked_add(size_of::<(String, String)>() + 256))
            .ok_or(ItemRefusal::Budget)?;
        if path.is_empty() || !Self::valid_digest(digest) {
            return Err(source_refusal());
        }
        self.preflight(workspace, max_state_bytes)?;
        self.context.check()?;
        // Reserve both the insert and exact-key readback before either SQL
        // operation; an existing path is accepted only with the same digest.
        self.charge_scan_rows(2)?;
        let inserted = self
            .db
            .execute(
                "INSERT OR IGNORE INTO sf_discovery_digests(path,sha256) VALUES(?1,?2)",
                params![path, digest],
            )
            .map_err(sql_refusal)?;
        if inserted > 1 {
            return Err(source_refusal());
        }
        self.context.check()?;
        let mut statement = self
            .db
            .prepare("SELECT sha256 FROM sf_discovery_digests WHERE path=?1")
            .map_err(sql_refusal)?;
        let mut rows = statement.query([path]).map_err(sql_refusal)?;
        let stored = rows
            .next()
            .map_err(sql_refusal)?
            .map(|row| bounded_row_text(row, 0, workspace))
            .transpose()
            .map_err(sql_refusal)?
            .ok_or_else(source_refusal)?;
        if rows.next().map_err(sql_refusal)?.is_some() || stored != digest {
            return Err(source_refusal());
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        self.observation_rows = self
            .observation_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        if inserted == 1 {
            self.unique_paths = self
                .unique_paths
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            self.serialized_write_bytes = self
                .serialized_write_bytes
                .checked_add(usize_u64(
                    path.len()
                        .checked_add(digest.len())
                        .ok_or(ItemRefusal::Budget)?,
                )?)
                .ok_or(ItemRefusal::Budget)?;
        }
        self.serialized_read_bytes = self
            .serialized_read_bytes
            .checked_add(usize_u64(stored.len())?)
            .ok_or(ItemRefusal::Budget)?;
        self.max_path_bytes = self.max_path_bytes.max(path.len());
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok((inserted == 1, workspace))
    }

    fn finish(
        &mut self,
        expected_observation_rows: u64,
        expected_unique_paths: u64,
        max_state_bytes: usize,
    ) -> Result<DiscoveryDigestCacheCost, ItemRefusal> {
        if self.finished
            || self.observation_rows != expected_observation_rows
            || self.unique_paths != expected_unique_paths
        {
            return Err(source_refusal());
        }
        let path_state = Self::row_text_state(self.max_path_bytes.max(1))?;
        let digest_state = Self::row_text_state(64)?;
        let workspace = path_state
            .checked_mul(2)
            .and_then(|state| state.checked_add(digest_state))
            .and_then(|state| state.checked_add(size_of::<(String, String)>() + 512))
            .ok_or(ItemRefusal::Budget)?;
        self.preflight(workspace, max_state_bytes)?;
        let audit_rows = usize::try_from(expected_unique_paths)
            .map_err(|_| ItemRefusal::Budget)?
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.charge_scan_rows(audit_rows)?;
        self.context.check()?;
        let mut statement = self
            .db
            .prepare("SELECT path,sha256 FROM sf_discovery_digests ORDER BY path COLLATE BINARY")
            .map_err(sql_refusal)?;
        let mut rows = statement.query([]).map_err(sql_refusal)?;
        let mut visited = 0u64;
        let mut previous_path: Option<String> = None;
        while let Some(row) = rows.next().map_err(sql_refusal)? {
            if visited % 128 == 0 {
                self.context.check()?;
            }
            if visited >= expected_unique_paths {
                return Err(source_refusal());
            }
            let path = bounded_row_text(row, 0, workspace).map_err(sql_refusal)?;
            let digest = bounded_row_text(row, 1, workspace).map_err(sql_refusal)?;
            if path.is_empty()
                || path.len() > self.max_path_bytes
                || previous_path
                    .as_deref()
                    .is_some_and(|prior| prior >= path.as_str())
                || !Self::valid_digest(&digest)
            {
                return Err(source_refusal());
            }
            self.serialized_read_bytes = self
                .serialized_read_bytes
                .checked_add(usize_u64(
                    path.len()
                        .checked_add(digest.len())
                        .ok_or(ItemRefusal::Budget)?,
                )?)
                .ok_or(ItemRefusal::Budget)?;
            previous_path = Some(path);
            visited = visited.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
        drop(rows);
        drop(statement);
        self.context.check()?;
        if visited != expected_unique_paths {
            return Err(source_refusal());
        }
        self.finished = true;
        self.workspace_peak_bytes = self.workspace_peak_bytes.max(workspace);
        Ok(self.cost())
    }

    fn verify_finished(&self) -> Result<(), ItemRefusal> {
        if !self.finished {
            return Err(source_refusal());
        }
        self.context.check()
    }

    fn cost(&self) -> DiscoveryDigestCacheCost {
        CandidateDiscoveryDigestCache::cost(self)
    }
}

struct BiblioEventsProvider<'a, 'candidate, 'host, 'cancel> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    state: &'a mut EventProjectionState,
    max_json_bytes: usize,
    max_state_bytes: usize,
    query_budget: &'a BiblioQueryBudget,
}

impl SourceFoundationDefaultEventLookup for BiblioEventsProvider<'_, '_, '_, '_> {
    fn event(&self, id: &str) -> Result<Option<Cow<'_, Value>>, ItemRefusal> {
        query_event_value(
            self.context,
            self.db,
            "biblio_events",
            id,
            self.max_json_bytes,
            Some(self.query_budget),
        )
        .map(|value| value.map(Cow::Owned))
    }

    fn for_each_event(
        &self,
        visit: &mut dyn FnMut(&str, &Value) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        event_for_each(
            self.context,
            self.db,
            "biblio_events",
            self.max_json_bytes,
            Some(self.query_budget),
            visit,
        )
    }
}

impl SourceFoundationBiblioEventSink for BiblioEventsProvider<'_, '_, '_, '_> {
    fn insert_biblio_event(
        &mut self,
        ordinal: u64,
        id: &str,
        value: &Value,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if !self
            .context
            .candidate
            .matches_invocation(deadline, cancelled)
        {
            self.context.candidate.abandon();
            return Err(source_refusal());
        }
        self.context.check()?;
        if self
            .state
            .last_source_ordinal
            .is_some_and(|prior| ordinal <= prior)
        {
            self.context.candidate.abandon();
            return Err(source_refusal());
        }
        let value_json_bytes = json_len(value, self.max_json_bytes)?;
        let value_state = estimate_value_state(value)?;
        let id_state = estimate_string_state(id)?;
        let encoded_state = value_json_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<u8>>() + 1024))
            .ok_or(ItemRefusal::Budget)?;
        let workspace = checked_add(checked_add(value_state, id_state)?, encoded_state)?;
        check_event_operation(self.context, 0, self.state.retained_state_bytes, workspace)?;
        if workspace > self.max_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        let previous: Option<([u8; 8], [u8; 8], usize)> = self
            .query_budget
            .charge()
            .and_then(|()| {
                self.db
                    .query_row(
                        "SELECT slot,observation_count,length(value) FROM biblio_events WHERE id=?1",
                        [id],
                        |row| {
                            let slot: [u8; 8] = row_blob(row, 0)?
                                .try_into()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?;
                            let count: [u8; 8] = row_blob(row, 1)?
                                .try_into()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?;
                            let value_bytes: i64 = row.get(2)?;
                            let value_bytes = usize::try_from(value_bytes)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?;
                            Ok((slot, count, value_bytes))
                        },
                    )
                    .optional()
                    .map_err(sql_refusal)
            })?;
        self.context.check()?;
        let previous_count = previous
            .as_ref()
            .map(|(_, bytes, _)| u64::from_be_bytes(*bytes));
        let observation_count = previous_count
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        let id_json_bytes = json_len(&id, self.max_json_bytes)?;
        let merged_json_bytes = next_event_json_total(
            self.state.merged_json_bytes,
            previous.as_ref().map(|(_, _, value_bytes)| *value_bytes),
            id_json_bytes,
            value_json_bytes,
            self.state.unique_ids,
            self.max_json_bytes,
        )?;
        let encoded = encoded_json(value, value_json_bytes, self.max_state_bytes)?;
        let ordinal_bytes = ordinal.to_be_bytes();
        let slot = previous
            .as_ref()
            .map(|(slot, _, _)| slot.as_slice())
            .unwrap_or(ordinal_bytes.as_slice());
        let last_ordinal = ordinal.to_be_bytes();
        let count_bytes = observation_count.to_be_bytes();
        self.context.check()?;
        self.query_budget.charge()?;
        self.db
            .execute(
                "INSERT INTO biblio_events(slot,id,last_ordinal,observation_count,value) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET last_ordinal=excluded.last_ordinal,observation_count=excluded.observation_count,value=excluded.value",
                params![slot, id, last_ordinal.as_slice(), count_bytes.as_slice(), encoded],
            )
            .map_err(sql_refusal)?;
        self.context.check()?;
        if previous.is_none() {
            self.state.unique_ids = self
                .state
                .unique_ids
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        self.state.observation_rows = self
            .state
            .observation_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.state.last_source_ordinal = Some(ordinal);
        self.state.merged_json_bytes = merged_json_bytes;
        self.state.workspace_peak_bytes = self.state.workspace_peak_bytes.max(workspace);
        Ok(())
    }

    fn biblio_event_observation_count(&self, id: &str) -> Result<u64, ItemRefusal> {
        self.context.check()?;
        self.context.active_state(size_of::<[u8; 8]>())?;
        self.query_budget.charge()?;
        let count: Option<[u8; 8]> = self
            .db
            .query_row(
                "SELECT observation_count FROM biblio_events WHERE id=?1",
                [id],
                |row| {
                    row_blob(row, 0)?
                        .try_into()
                        .map_err(|_| rusqlite::Error::InvalidQuery)
                },
            )
            .optional()
            .map_err(sql_refusal)?;
        self.context.check()?;
        Ok(count.map(u64::from_be_bytes).unwrap_or(0))
    }

    fn biblio_event_lookup(&self) -> &dyn SourceFoundationDefaultEventLookup {
        self
    }
}

fn decode_claim_row(
    context: ProviderContext<'_, '_, '_>,
    row: &rusqlite::Row<'_>,
    max_state_bytes: usize,
    live_state_bytes: usize,
) -> Result<(u64, BiblioClaim), ItemRefusal> {
    let ordinal = checked_u64_blob(row_blob(row, 0).map_err(sql_refusal)?)?;
    let path_ref = match row.get_ref(1).map_err(sql_refusal)? {
        rusqlite::types::ValueRef::Text(value) => value,
        _ => return Err(source_refusal()),
    };
    let line = usize::try_from(checked_u64_blob(row_blob(row, 2).map_err(sql_refusal)?)?)
        .map_err(|_| source_refusal())?;
    let digest_ref = match row.get_ref(3).map_err(sql_refusal)? {
        rusqlite::types::ValueRef::Text(value) => value,
        _ => return Err(source_refusal()),
    };
    let native = match row.get(4).map_err(sql_refusal)? {
        0_i64 => false,
        1_i64 => true,
        _ => return Err(source_refusal()),
    };
    let raw_value = row_blob(row, 5).map_err(sql_refusal)?;
    let text_state = |length: usize| -> Result<usize, ItemRefusal> {
        length
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or(ItemRefusal::Budget)
    };
    let strings_state = checked_add(text_state(path_ref.len())?, text_state(digest_ref.len())?)?;
    let upper = json_state_upper_bound(raw_value.len())?;
    let precharge = checked_add(
        live_state_bytes,
        checked_add(
            checked_add(strings_state, upper)?,
            size_of::<BiblioClaim>() + size_of::<u64>(),
        )?,
    )?;
    if raw_value.is_empty() || precharge > max_state_bytes {
        return Err(ItemRefusal::Budget);
    }
    context.active_state(precharge)?;
    let path = std::str::from_utf8(path_ref)
        .map_err(|_| source_refusal())?
        .to_owned();
    let raw_sha256 = std::str::from_utf8(digest_ref)
        .map_err(|_| source_refusal())?
        .to_owned();
    let value: Value = serde_json::from_slice(raw_value).map_err(|_| source_refusal())?;
    let claim = BiblioClaim {
        path,
        line,
        value,
        raw_sha256,
        native,
    };
    let actual = checked_add(
        live_state_bytes,
        checked_add(
            checked_add(
                estimate_string_state(&claim.path)?,
                estimate_string_state(&claim.raw_sha256)?,
            )?,
            checked_add(
                estimate_value_state(&claim.value)?,
                size_of::<BiblioClaim>(),
            )?,
        )?,
    )?;
    if actual > max_state_bytes {
        return Err(ItemRefusal::Budget);
    }
    context.active_state(checked_add(actual, size_of::<u64>())?)?;
    Ok((ordinal, claim))
}

struct ClaimsProvider<'a, 'candidate, 'host, 'cancel> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    state: &'a mut ClaimProjectionState,
    max_state_bytes: usize,
    query_budget: &'a BiblioQueryBudget,
    live_state: &'a Cell<usize>,
}

impl ClaimsProvider<'_, '_, '_, '_> {
    fn with_live_claim<T>(
        &self,
        bytes: usize,
        callback: impl FnOnce() -> Result<T, ItemRefusal>,
    ) -> Result<T, ItemRefusal> {
        let previous = self.live_state.get();
        let next = checked_add(previous, bytes)?;
        self.context.active_state(next)?;
        self.live_state.set(next);
        let result = callback();
        self.live_state.set(previous);
        result
    }

    fn next_claim(
        &self,
        after: Option<u64>,
        location: Option<(&str, usize)>,
        descending: bool,
    ) -> Result<Option<(u64, BiblioClaim)>, ItemRefusal> {
        self.context.check()?;
        self.query_budget.charge()?;
        let line = location.map(|(_, line)| line_u64_blob(line)).transpose()?;
        let result = match (location, descending) {
            (None, false) => self.db.query_row(
                "SELECT ordinal,path,line,raw_sha256,native,value FROM biblio_claims WHERE (?1 IS NULL OR ordinal>?1) ORDER BY ordinal LIMIT 1",
                [after.map(u64::to_be_bytes).as_ref().map(|value| value.as_slice())],
                |row| decode_claim_row(self.context, row, self.max_state_bytes, self.live_state.get()).map_err(|_| rusqlite::Error::InvalidQuery),
            ),
            (None, true) => self.db.query_row(
                "SELECT ordinal,path,line,raw_sha256,native,value FROM biblio_claims WHERE (?1 IS NULL OR ordinal<?1) ORDER BY ordinal DESC LIMIT 1",
                [after.map(u64::to_be_bytes).as_ref().map(|value| value.as_slice())],
                |row| decode_claim_row(self.context, row, self.max_state_bytes, self.live_state.get()).map_err(|_| rusqlite::Error::InvalidQuery),
            ),
            (Some((path, _)), false) => self.db.query_row(
                "SELECT ordinal,path,line,raw_sha256,native,value FROM biblio_claims WHERE path=?1 AND line=?2 AND (?3 IS NULL OR ordinal>?3) ORDER BY ordinal LIMIT 1",
                params![path, line.as_deref(), after.map(u64::to_be_bytes).as_ref().map(|value| value.as_slice())],
                |row| decode_claim_row(self.context, row, self.max_state_bytes, self.live_state.get()).map_err(|_| rusqlite::Error::InvalidQuery),
            ),
            (Some((path, _)), true) => self.db.query_row(
                "SELECT ordinal,path,line,raw_sha256,native,value FROM biblio_claims WHERE path=?1 AND line=?2 AND (?3 IS NULL OR ordinal<?3) ORDER BY ordinal DESC LIMIT 1",
                params![path, line.as_deref(), after.map(u64::to_be_bytes).as_ref().map(|value| value.as_slice())],
                |row| decode_claim_row(self.context, row, self.max_state_bytes, self.live_state.get()).map_err(|_| rusqlite::Error::InvalidQuery),
            ),
        }
        .optional()
        .map_err(sql_refusal)?;
        self.context.check()?;
        Ok(result)
    }

    fn count_query(
        &self,
        sql: &'static str,
        params: impl rusqlite::Params,
    ) -> Result<u64, ItemRefusal> {
        self.context.check()?;
        self.query_budget.charge()?;
        let count: i64 = self
            .db
            .query_row(sql, params, |row| row.get(0))
            .map_err(sql_refusal)?;
        self.context.check()?;
        count_from_sql(count)
    }

    fn find_claim_id(&self, id: &str) -> Result<Option<(u64, BiblioClaim)>, ItemRefusal> {
        self.context.check()?;
        self.query_budget.charge()?;
        let found = self.db.query_row(
            "SELECT ordinal,path,line,raw_sha256,native,value FROM biblio_claims WHERE claim_id=?1 ORDER BY ordinal LIMIT 1",
            [id],
            |row| decode_claim_row(self.context, row, self.max_state_bytes, self.live_state.get()).map_err(|_| rusqlite::Error::InvalidQuery),
        ).optional().map_err(sql_refusal)?;
        self.context.check()?;
        Ok(found)
    }

    fn insert(&mut self, ordinal: u64, claim: &BiblioClaim) -> Result<(), ItemRefusal> {
        self.context.check()?;
        if self
            .state
            .last_ordinal
            .is_some_and(|previous| ordinal <= previous)
        {
            self.context.candidate.abandon();
            return Err(source_refusal());
        }
        let value_bytes = json_len(&claim.value, self.max_state_bytes)?;
        let value_state = estimate_value_state(&claim.value)?;
        let path_state = estimate_string_state(&claim.path)?;
        let digest_state = estimate_string_state(&claim.raw_sha256)?;
        let encoded_state = value_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<u8>>() + 1024))
            .ok_or(ItemRefusal::Budget)?;
        let workspace = checked_add(
            checked_add(checked_add(value_state, path_state)?, digest_state)?,
            checked_add(encoded_state, size_of::<BiblioClaim>())?,
        )?;
        check_event_operation(self.context, 0, 0, workspace)?;
        if workspace > self.max_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        let claim_id = claim.value.get("claim_id").and_then(Value::as_str);
        let encoded = encoded_json(&claim.value, value_bytes, self.max_state_bytes)?;
        let ordinal_bytes = ordinal.to_be_bytes();
        let line_bytes = usize_u64(claim.line)?.to_be_bytes();
        self.context.check()?;
        self.query_budget.charge()?;
        self.db.execute(
            "INSERT INTO biblio_claims(ordinal,path,line,claim_id,raw_sha256,native,value) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![ordinal_bytes.as_slice(), claim.path, line_bytes.as_slice(), claim_id, claim.raw_sha256, if claim.native { 1_i64 } else { 0_i64 }, encoded],
        ).map_err(sql_refusal)?;
        self.context.check()?;
        self.state.inserted_rows = self
            .state
            .inserted_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.state.last_ordinal = Some(ordinal);
        self.state.workspace_peak_bytes = self.state.workspace_peak_bytes.max(workspace);
        Ok(())
    }
}

fn line_u64_blob(line: usize) -> Result<[u8; 8], ItemRefusal> {
    Ok(usize_u64(line)?.to_be_bytes())
}

impl SourceFoundationDefaultClaims for ClaimsProvider<'_, '_, '_, '_> {
    fn claim_by_id(&self, id: &str) -> Result<Option<(u64, Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.find_claim_id(id)
            .map(|claim| claim.map(|(ordinal, claim)| (ordinal, Cow::Owned(claim))))
    }

    fn for_each_claim(
        &self,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let mut after = None;
        loop {
            let Some((ordinal, claim)) = self.next_claim(after, None, false)? else {
                break;
            };
            self.with_live_claim(
                checked_add(estimate_claim_state(&claim)?, size_of::<u64>())?,
                || visit(ordinal, &claim),
            )?;
            after = Some(ordinal);
        }
        Ok(())
    }

    fn first_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.next_claim(None, Some((path, line)), false)
            .map(|claim| claim.map(|(ordinal, claim)| (ordinal, Cow::Owned(claim))))
    }

    fn last_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.next_claim(None, Some((path, line)), true)
            .map(|claim| claim.map(|(ordinal, claim)| (ordinal, Cow::Owned(claim))))
    }

    fn claim_count_for_path(&self, path: &str) -> Result<u64, ItemRefusal> {
        self.count_query("SELECT COUNT(*) FROM biblio_claims WHERE path=?1", [path])
    }

    fn distinct_nonzero_claim_lines_for_path(&self, path: &str) -> Result<u64, ItemRefusal> {
        let zero = 0_u64.to_be_bytes();
        self.count_query(
            "SELECT COUNT(DISTINCT line) FROM biblio_claims WHERE path=?1 AND line>?2",
            params![path, zero.as_slice()],
        )
    }

    fn for_each_claim_at(
        &self,
        path: &str,
        line: usize,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let mut after = None;
        loop {
            let Some((ordinal, claim)) = self.next_claim(after, Some((path, line)), false)? else {
                break;
            };
            self.with_live_claim(
                checked_add(estimate_claim_state(&claim)?, size_of::<u64>())?,
                || visit(ordinal, &claim),
            )?;
            after = Some(ordinal);
        }
        Ok(())
    }
}

impl SourceFoundationBiblioClaimSink for ClaimsProvider<'_, '_, '_, '_> {
    fn insert_biblio_claim(
        &mut self,
        ordinal: u64,
        claim: &BiblioClaim,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if !self
            .context
            .candidate
            .matches_invocation(deadline, cancelled)
        {
            self.context.candidate.abandon();
            return Err(source_refusal());
        }
        self.insert(ordinal, claim)
    }
}

fn estimate_claim_state(claim: &BiblioClaim) -> Result<usize, ItemRefusal> {
    checked_add(
        checked_add(
            checked_add(
                size_of::<BiblioClaim>(),
                estimate_string_state(&claim.path)?,
            )?,
            estimate_string_state(&claim.raw_sha256)?,
        )?,
        estimate_value_state(&claim.value)?,
    )
}

struct CandidateDefaultRecords<'report, 'index, 'candidate, 'host, 'cancel, 'budget> {
    report: &'report SourceFoundationRecordsStreamedReport<'index, CandidateFence>,
    context: ProviderContext<'candidate, 'host, 'cancel>,
    scan_rows: &'budget Cell<u64>,
    lookup_state_limit: usize,
}

impl CandidateDefaultRecords<'_, '_, '_, '_, '_, '_> {
    fn for_collection(
        &self,
        collection: RecordsCollection,
        mut visit: impl FnMut(&StoredFact, usize) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let mut after = None;
        loop {
            self.context.check()?;
            let page = self.report.index().page(
                collection,
                after.as_ref(),
                self.context.page_budget,
                self.context.deadline,
                self.context.cancelled,
            )?;
            self.context.check()?;
            if page.charged_state_bytes > self.context.page_budget.max_state_bytes.get() {
                return Err(ItemRefusal::Budget);
            }
            self.context
                .add_scan_rows(self.scan_rows, page.rows.len())?;
            for fact in &page.rows {
                self.context.check()?;
                visit(fact, page.charged_state_bytes)?;
            }
            after = page.next_cursor;
            if after.is_none() {
                return self.context.check();
            }
        }
    }

    fn current_record_owned(&self, id: &str) -> Result<Option<BiblioCurrentRecord>, ItemRefusal> {
        self.context.check()?;
        let max_state =
            std::num::NonZeroUsize::new(self.lookup_state_limit).ok_or(ItemRefusal::Budget)?;
        let found = self.report.index().lookup_current_record(
            id,
            max_state,
            self.context.deadline,
            self.context.cancelled,
        )?;
        self.context.check()?;
        let Some(found) = found else {
            return Ok(None);
        };
        let state = estimate_record_state(&found.record)?;
        self.context.active_state(state)?;
        Ok(Some(found.record))
    }

    fn sorted_current_records(
        &self,
        mut visit: impl FnMut(&str, &BiblioCurrentRecord) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let mut after: Option<String> = None;
        loop {
            self.context.check()?;
            let page = self.report.index().current_records_by_id_page(
                after.as_deref(),
                self.context.page_budget,
                self.context.deadline,
                self.context.cancelled,
            )?;
            self.context.check()?;
            if page.charged_state_bytes > self.context.operation_state_limit {
                return Err(ItemRefusal::Budget);
            }
            self.context
                .add_scan_rows(self.scan_rows, page.rows.len())?;
            self.context.active_state(page.charged_state_bytes)?;
            for (id, record) in &page.rows {
                self.context.check()?;
                visit(id, record)?;
            }
            let Some(next) = page.next_after_id else {
                return self.context.check();
            };
            if after.as_deref().is_some_and(|prior| next.as_str() <= prior) {
                self.context.candidate.abandon();
                return Err(source_refusal());
            }
            after = Some(next);
        }
    }

    fn record_for_path(
        &self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<BiblioCurrentRecord>, usize), ItemRefusal> {
        self.context.check()?;
        let workspace_state = estimate_string_state(path)?
            .checked_add(256)
            .ok_or(ItemRefusal::Budget)?;
        let max_state = max_state_bytes.min(self.lookup_state_limit);
        if workspace_state > max_state {
            return Err(ItemRefusal::Budget);
        }
        self.context.row_state(workspace_state)?;
        let max_state = std::num::NonZeroUsize::new(max_state).ok_or(ItemRefusal::Budget)?;
        self.context.add_scan_rows(self.scan_rows, 1)?;
        let found = self.report.index().lookup_current_record_by_path(
            path,
            max_state,
            self.context.deadline,
            self.context.cancelled,
        )?;
        self.context.check()?;
        let Some(found) = found else {
            self.context.active_state(workspace_state)?;
            return Ok((None, workspace_state));
        };
        if found.record.path != path {
            self.context.candidate.abandon();
            return Err(source_refusal());
        }
        let charged_state_bytes = found.charged_state_bytes.max(workspace_state);
        self.context.active_state(charged_state_bytes)?;
        Ok((Some(found.record), charged_state_bytes))
    }
}

fn estimate_record_state(record: &BiblioCurrentRecord) -> Result<usize, ItemRefusal> {
    checked_add(
        checked_add(
            size_of::<BiblioCurrentRecord>(),
            estimate_string_state(&record.path)?,
        )?,
        checked_add(
            estimate_string_state(&record.kind)?,
            estimate_value_state(&record.value)?,
        )?,
    )
}

impl SourceFoundationDefaultRecordsLookup for CandidateDefaultRecords<'_, '_, '_, '_, '_, '_> {
    fn current_record(
        &self,
        id: &str,
    ) -> Result<Option<Cow<'_, BiblioCurrentRecord>>, ItemRefusal> {
        self.current_record_owned(id)
            .map(|record| record.map(Cow::Owned))
    }

    fn record_by_path(
        &self,
        path: &str,
    ) -> Result<Option<Cow<'_, BiblioCurrentRecord>>, ItemRefusal> {
        self.record_for_path(path, self.lookup_state_limit)
            .map(|(record, _)| record.map(Cow::Owned))
    }

    fn record_by_path_with_state_budget(
        &self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<Cow<'_, BiblioCurrentRecord>>, usize), ItemRefusal> {
        self.record_for_path(path, max_state_bytes)
            .map(|(record, charged)| (record.map(Cow::Owned), charged))
    }

    fn item_edition(&self, id: &str) -> Result<Option<Cow<'_, str>>, ItemRefusal> {
        self.context.check()?;
        let max_state =
            std::num::NonZeroUsize::new(self.lookup_state_limit).ok_or(ItemRefusal::Budget)?;
        let found = self.report.index().lookup_item_edition(
            id,
            max_state,
            self.context.deadline,
            self.context.cancelled,
        )?;
        self.context.check()?;
        if let Some(found) = found {
            self.context.active_state(found.charged_state_bytes)?;
            Ok(Some(Cow::Owned(found.embodiment_ref)))
        } else {
            Ok(None)
        }
    }

    fn rights_contains(&self, id: &str) -> Result<bool, ItemRefusal> {
        self.context.check()?;
        self.context.active_state(estimate_string_state(id)?)?;
        let found = self.report.index().contains_rights_id(
            id,
            self.context.deadline,
            self.context.cancelled,
        )?;
        self.context.check()?;
        Ok(found)
    }

    fn file_contains(&self, item: &Value, file: &Value) -> Result<bool, ItemRefusal> {
        let (Some(item), Some(file)) = (item.as_str(), file.as_str()) else {
            return Ok(false);
        };
        self.context.check()?;
        let found = self.report.index().contains_item_file_membership(
            file,
            item,
            self.context.deadline,
            self.context.cancelled,
        )?;
        self.context.check()?;
        Ok(found)
    }

    fn file_sha256(&self, file: &Value) -> Result<Option<Cow<'_, Value>>, ItemRefusal> {
        let Some(file_id) = file.as_str() else {
            return Ok(None);
        };
        self.context.check()?;
        let max_state =
            std::num::NonZeroUsize::new(self.lookup_state_limit).ok_or(ItemRefusal::Budget)?;
        let found = self.report.index().lookup_file_descriptor(
            file_id,
            max_state,
            self.context.deadline,
            self.context.cancelled,
        )?;
        self.context.check()?;
        let Some(found) = found else {
            return Ok(None);
        };
        let state = estimate_value_state(&found.sha256)?;
        self.context.active_state(state)?;
        Ok(Some(Cow::Owned(found.sha256)))
    }

    fn for_each_current_record(
        &self,
        visit: &mut dyn FnMut(&str, &BiblioCurrentRecord) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.sorted_current_records(visit)
    }

    fn for_each_profile_kind(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.for_collection(RecordsCollection::UsedDeclaredProfileKinds, |fact, _| {
            let StoredFact::UsedDeclaredProfileKind(kind) = fact else {
                return Err(source_refusal());
            };
            visit(kind)
        })
    }
}

struct BiblioStoredProvider<'a, 'candidate, 'host, 'cancel> {
    events: BiblioEventsProvider<'a, 'candidate, 'host, 'cancel>,
    claims: ClaimsProvider<'a, 'candidate, 'host, 'cancel>,
    manifests: BiblioManifestProvider<'a, 'candidate, 'host, 'cancel>,
}

impl SourceFoundationDefaultEventLookup for BiblioStoredProvider<'_, '_, '_, '_> {
    fn event(&self, id: &str) -> Result<Option<Cow<'_, Value>>, ItemRefusal> {
        self.events.event(id)
    }

    fn for_each_event(
        &self,
        visit: &mut dyn FnMut(&str, &Value) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.events.for_each_event(visit)
    }
}

impl SourceFoundationBiblioEventSink for BiblioStoredProvider<'_, '_, '_, '_> {
    fn insert_biblio_event(
        &mut self,
        ordinal: u64,
        id: &str,
        value: &Value,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.events
            .insert_biblio_event(ordinal, id, value, deadline, cancelled)
    }

    fn biblio_event_observation_count(&self, id: &str) -> Result<u64, ItemRefusal> {
        self.events.biblio_event_observation_count(id)
    }

    fn biblio_event_lookup(&self) -> &dyn SourceFoundationDefaultEventLookup {
        &self.events
    }
}

impl SourceFoundationDefaultClaims for BiblioStoredProvider<'_, '_, '_, '_> {
    fn claim_by_id(&self, id: &str) -> Result<Option<(u64, Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.claims.claim_by_id(id)
    }

    fn for_each_claim(
        &self,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.claims.for_each_claim(visit)
    }

    fn first_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.claims.first_claim_at(path, line)
    }

    fn last_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.claims.last_claim_at(path, line)
    }

    fn claim_count_for_path(&self, path: &str) -> Result<u64, ItemRefusal> {
        self.claims.claim_count_for_path(path)
    }

    fn distinct_nonzero_claim_lines_for_path(&self, path: &str) -> Result<u64, ItemRefusal> {
        self.claims.distinct_nonzero_claim_lines_for_path(path)
    }

    fn for_each_claim_at(
        &self,
        path: &str,
        line: usize,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.claims.for_each_claim_at(path, line, visit)
    }
}

impl SourceFoundationBiblioClaimSink for BiblioStoredProvider<'_, '_, '_, '_> {
    fn insert_biblio_claim(
        &mut self,
        ordinal: u64,
        claim: &BiblioClaim,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.claims
            .insert_biblio_claim(ordinal, claim, deadline, cancelled)
    }
}

struct BiblioManifestProvider<'a, 'candidate, 'host, 'cancel> {
    context: ProviderContext<'candidate, 'host, 'cancel>,
    db: &'a PinnedSqliteConnection,
    state: &'a mut ManifestProjectionState,
    max_state_bytes: usize,
    query_budget: &'a BiblioQueryBudget,
}

impl BiblioManifestProvider<'_, '_, '_, '_> {
    fn edition_for_id(&self, id: &str) -> Result<Option<String>, ItemRefusal> {
        self.context.check()?;
        self.query_budget.charge()?;
        let edition = self
            .db
            .query_row(
                "SELECT edition FROM biblio_manifests WHERE id=?1",
                [id],
                |row| bounded_row_text_precharged(self.context, row, 0, self.max_state_bytes),
            )
            .optional()
            .map_err(sql_refusal)?;
        self.context.check()?;
        if let Some(edition) = &edition {
            self.context.active_state(estimate_string_state(edition)?)?;
        }
        Ok(edition)
    }
}

impl tos_validation::biblio_rules::SourceFoundationBiblioManifestSink
    for BiblioManifestProvider<'_, '_, '_, '_>
{
    fn observe_biblio_manifest(
        &mut self,
        id: &str,
        edition: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        if !self
            .context
            .candidate
            .matches_invocation(deadline, cancelled)
        {
            self.context.candidate.abandon();
            return Err(source_refusal());
        }
        self.context.check()?;
        let id_state = estimate_string_state(id)?;
        let edition_state = estimate_string_state(edition)?;
        let workspace = checked_add(id_state, edition_state)?;
        check_event_operation(self.context, 0, 0, workspace)?;
        if workspace > self.max_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        let previous: Option<[u8; 8]> = self.query_budget.charge().and_then(|()| {
            self.db
                .query_row(
                    "SELECT observation_count FROM biblio_manifests WHERE id=?1",
                    [id],
                    |row| {
                        row_blob(row, 0)?
                            .try_into()
                            .map_err(|_| rusqlite::Error::InvalidQuery)
                    },
                )
                .optional()
                .map_err(sql_refusal)
        })?;
        self.context.check()?;
        let duplicate = previous.is_some();
        let count = previous
            .map(|count| u64::from_be_bytes(count))
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        let ordinal = self.state.observations;
        let slot = ordinal.to_be_bytes();
        let count_bytes = count.to_be_bytes();
        self.context.check()?;
        self.query_budget.charge()?;
        self.db
            .execute(
                "INSERT INTO biblio_manifests(slot,id,observation_count,edition) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET observation_count=excluded.observation_count,edition=excluded.edition",
                params![slot.as_slice(), id, count_bytes.as_slice(), edition],
            )
            .map_err(sql_refusal)?;
        self.context.check()?;
        self.state.observations = self
            .state
            .observations
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        if !duplicate {
            self.state.unique_ids = self
                .state
                .unique_ids
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        self.state.workspace_peak_bytes = self.state.workspace_peak_bytes.max(workspace);
        Ok(duplicate)
    }

    fn for_each_biblio_manifest(
        &self,
        visit: &mut dyn FnMut(&str, &str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let mut after: Option<[u8; 8]> = None;
        loop {
            self.context.check()?;
            self.query_budget.charge()?;
            let next = self
                .db
                .query_row(
                    "SELECT slot,id,edition FROM biblio_manifests WHERE (?1 IS NULL OR slot>?1) ORDER BY slot LIMIT 1",
                    [after.as_ref().map(|slot| slot.as_slice())],
                    |row| {
                        let slot: [u8; 8] = row_blob(row, 0)?
                            .try_into()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        let strings_state = row_text_state(row, 1)?
                            .checked_add(row_text_state(row, 2)?)
                            .ok_or(rusqlite::Error::InvalidQuery)?;
                        self.context
                            .active_state(strings_state)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        let id = bounded_row_text(row, 1, self.max_state_bytes)?;
                        let edition = bounded_row_text(row, 2, self.max_state_bytes)?;
                        Ok((slot, id, edition))
                    },
                )
                .optional()
                .map_err(sql_refusal)?;
            self.context.check()?;
            let Some((slot, id, edition)) = next else {
                return Ok(());
            };
            self.context.active_state(checked_add(
                estimate_string_state(&id)?,
                estimate_string_state(&edition)?,
            )?)?;
            visit(&id, &edition)?;
            after = Some(slot);
        }
    }

    fn biblio_manifest_edition(&self, id: &str) -> Result<Option<Cow<'_, str>>, ItemRefusal> {
        self.edition_for_id(id)
            .map(|edition| edition.map(Cow::Owned))
    }
}

impl SourceFoundationBiblioManifestSink for BiblioStoredProvider<'_, '_, '_, '_> {
    fn observe_biblio_manifest(
        &mut self,
        id: &str,
        edition: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.manifests
            .observe_biblio_manifest(id, edition, deadline, cancelled)
    }

    fn for_each_biblio_manifest(
        &self,
        visit: &mut dyn FnMut(&str, &str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.manifests.for_each_biblio_manifest(visit)
    }

    fn biblio_manifest_edition(&self, id: &str) -> Result<Option<Cow<'_, str>>, ItemRefusal> {
        self.manifests.biblio_manifest_edition(id)
    }
}

impl SourceFoundationBiblioQueryBudget for BiblioStoredProvider<'_, '_, '_, '_> {
    fn bind_biblio_query_budget(&mut self, max_row_operations: u64) -> Result<(), ItemRefusal> {
        self.events.context.check()?;
        self.events.query_budget.bind(max_row_operations)?;
        self.events.context.check()
    }
}

impl<'candidate, 'host> SpoolDefaultStore<'candidate, 'host> {
    /// Lend the same invocation-bound indexes to Biblio and default kernels.
    /// The callback result must be owned; no report, page, provider, or Cow
    /// backed by the live SQLite scope escapes this method.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_providers<R>(
        &mut self,
        records: &SourceFoundationRecordsStreamedReport<'_, CandidateFence>,
        input: &CandidateRecordsInput<'candidate, 'host>,
        stored_limits: SourceFoundationDefaultStoredLimits,
        max_operation_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: impl FnOnce(
            &dyn SourceFoundationDefaultRecordsLookup,
            &dyn SourceFoundationDefaultPaths,
            &mut dyn SourceFoundationDefaultEventStore,
            &mut dyn DiscoverySeenIds,
            &mut dyn DiscoveryRunSummaryStore,
            &mut dyn DiscoveryEventSummaryStore,
            &mut dyn DiscoverySchemaRequestStore,
            &mut dyn DiscoveryDigestCache,
            &mut dyn SourceFoundationBiblioStoredSink,
        ) -> Result<R, ItemRefusal>,
    ) -> Result<R, ItemRefusal> {
        let result = (|| {
            self.report_matches(records)?;
            if !self.default_events_folded
                || max_operation_state_bytes == 0
                || max_operation_state_bytes > self.max_operation_state_bytes
                || input.input_identity() != &self.fence
            {
                return Err(source_refusal());
            }
            input.verify_invocation(deadline, cancelled)?;
            let context = self.context(
                stored_limits.page_budget,
                stored_limits.max_scan_rows,
                max_operation_state_bytes,
                deadline,
                cancelled,
            )?;
            context.check()?;
            let scan_rows = &self.scan_rows;
            let candidate = self.candidate;
            let records_provider = CandidateDefaultRecords {
                report: records,
                context,
                scan_rows,
                lookup_state_limit: max_operation_state_bytes,
            };
            let expected_members = self.fence.membership.count.max(1);
            let max_paths_per_scan = expected_members;
            let max_path_bytes_per_scan = usize_u64(max_operation_state_bytes)?
                .checked_mul(stored_limits.max_scan_rows)
                .filter(|bytes| *bytes != 0)
                .ok_or(ItemRefusal::Budget)?;
            let path_rows = &self.path_rows;
            let paths = CandidateDefaultPaths::new(
                candidate,
                input,
                stored_limits.page_budget,
                max_paths_per_scan,
                max_path_bytes_per_scan,
                stored_limits.max_scan_rows,
                max_operation_state_bytes,
                path_rows,
                deadline,
                cancelled,
            )?;
            let mut default_events = DefaultEventsProvider {
                context,
                db: &self.db,
                scan_rows,
                limits: self.limits,
                state: &mut self.default_events,
                active_page_state: Cell::new(0),
                json_ceiling: self.default_event_json_limit.ok_or_else(source_refusal)?,
            };
            let mut discovery_seen_ids = CandidateDiscoverySeenIds {
                context,
                db: &self.db,
                scan_rows,
            };
            let mut discovery_run_summaries = CandidateDiscoveryRunSummaries {
                context,
                db: &self.db,
                scan_rows,
                observation_rows: 0,
                unique_paths: 0,
                serialized_summary_write_bytes: 0,
                serialized_summary_read_bytes: 0,
                scan_row_operations: 0,
                workspace_peak_bytes: 0,
                finished: false,
            };
            let mut discovery_event_summaries = CandidateDiscoveryEventSummaries {
                context,
                db: &self.db,
                scan_rows,
                observation_rows: 0,
                owner_insertion_rows: 0,
                serialized_write_bytes: 0,
                serialized_read_bytes: 0,
                scan_row_operations: 0,
                workspace_peak_bytes: 0,
                finished: false,
                drained: false,
            };
            let mut discovery_schema_requests = CandidateDiscoverySchemaRequests {
                context,
                db: &self.db,
                scan_rows,
                observation_rows: 0,
                read_rows: 0,
                serialized_write_bytes: 0,
                serialized_read_bytes: 0,
                scan_row_operations: 0,
                workspace_peak_bytes: 0,
                max_document_bytes: None,
                last_before_issue: None,
                last_read_before_issue: None,
                cursor_ordinal: None,
                expected_rows: None,
                direct_issue_count: None,
                finished: false,
                drained: false,
            };
            let mut discovery_digest_cache = CandidateDiscoveryDigestCache {
                context,
                db: &self.db,
                scan_rows,
                observation_rows: 0,
                unique_paths: 0,
                serialized_write_bytes: 0,
                serialized_read_bytes: 0,
                scan_row_operations: 0,
                max_path_bytes: 0,
                workspace_peak_bytes: 0,
                finished: false,
            };
            let mut biblio = BiblioStoredProvider {
                events: BiblioEventsProvider {
                    context,
                    db: &self.db,
                    state: &mut self.biblio_events,
                    max_json_bytes: self.max_biblio_event_json_bytes,
                    max_state_bytes: max_operation_state_bytes,
                    query_budget: &self.biblio_query_budget,
                },
                claims: ClaimsProvider {
                    context,
                    db: &self.db,
                    state: &mut self.claims,
                    max_state_bytes: max_operation_state_bytes,
                    query_budget: &self.biblio_query_budget,
                    live_state: &self.claim_live_state,
                },
                manifests: BiblioManifestProvider {
                    context,
                    db: &self.db,
                    state: &mut self.biblio_manifests,
                    max_state_bytes: max_operation_state_bytes,
                    query_budget: &self.biblio_query_budget,
                },
            };
            let value = callback(
                &records_provider,
                &paths,
                &mut default_events,
                &mut discovery_seen_ids,
                &mut discovery_run_summaries,
                &mut discovery_event_summaries,
                &mut discovery_schema_requests,
                &mut discovery_digest_cache,
                &mut biblio,
            )?;
            discovery_schema_requests.verify_drained()?;
            discovery_digest_cache.verify_finished()?;
            drop(biblio);
            drop(discovery_digest_cache);
            drop(discovery_schema_requests);
            drop(discovery_event_summaries);
            drop(discovery_run_summaries);
            drop(discovery_seen_ids);
            drop(default_events);
            paths.verify_eof()?;
            input.verify_invocation(deadline, cancelled)?;
            self.report_matches(records)?;
            context.check()?;
            Ok(value)
        })();
        if result.is_err() {
            self.candidate.abandon();
        }
        result
    }
}
