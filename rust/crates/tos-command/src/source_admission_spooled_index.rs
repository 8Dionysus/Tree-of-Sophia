//! Disk-backed storage for the maintained native source identity/link kernel.
//!
//! This module owns no admission law. `source_admission_index::build_index_into`
//! supplies the same parser, schema, duplicate, reference and edge predicates
//! used by the resident `Index`; this adapter only stores resulting rows in
//! the candidate's shared-budget SQLite scope. A cursor view can be minted
//! only after the native validator presents its private completion token.

use crate::{
    source_admission_candidate_records::CandidateRecordsInput,
    source_admission_candidate_schema,
    source_admission_index::{
        self, AdmissionIndexBackend, BaseIdentityPath, CandidateIndexInput, FreshIndexRowsWriter,
        IndexLimits, NativeSemanticStep, SchemaCheck,
    },
    source_admission_spooled_candidate::{CandidateFence, SpoolCandidate},
    source_foundation_admission::NativeAdmissionComplete,
};
use rusqlite::{OptionalExtension, params};
use std::{io, sync::Arc, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, RelativePath};
use tos_source_store::{
    PinnedSqliteAuxLimits, PinnedSqliteAuxScope, PinnedSqliteConnection, PinnedSqliteIoBudget,
    SourceMembershipV1,
};
use tos_validation::{
    source_cut::CutPreparedSchemaExecutionBinding,
    source_foundation_records::SourceFoundationRecordsStreamedReport,
};

#[path = "source_admission_spooled_records.rs"]
mod records_store;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn sql(_: rusqlite::Error) -> io::Error {
    invalid("native source index storage refused")
}

fn bounded_text(row: &rusqlite::Row<'_>, column: usize, cap: usize) -> rusqlite::Result<String> {
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

fn bounded_text_pair(
    row: &rusqlite::Row<'_>,
    first: usize,
    second: usize,
    cap: usize,
) -> rusqlite::Result<(String, String)> {
    let first_len = match row.get_ref(first)? {
        rusqlite::types::ValueRef::Text(raw) => raw.len(),
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let second_len = match row.get_ref(second)? {
        rusqlite::types::ValueRef::Text(raw) => raw.len(),
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    if first_len
        .checked_add(second_len)
        // SQLite text, the retained identity, and the path wrapper coexist
        // while the cursor result is constructed; reserve all three copies.
        .and_then(|bytes| bytes.checked_mul(32))
        .and_then(|bytes| bytes.checked_add(4096))
        .is_none_or(|bytes| bytes > cap)
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok((row.get(first)?, row.get(second)?))
}

fn text_pair_state_upper_bound(first: usize, second: usize) -> io::Result<usize> {
    first
        .checked_add(second)
        .and_then(|bytes| bytes.checked_mul(32))
        .and_then(|bytes| bytes.checked_add(4096))
        .ok_or_else(|| invalid("native source row text state overflow"))
}

fn text_length(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<usize> {
    let value: i64 = row.get(column)?;
    usize::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn text_lengths(row: &rusqlite::Row<'_>) -> rusqlite::Result<(usize, usize)> {
    Ok((text_length(row, 0)?, text_length(row, 1)?))
}

fn bounded_u64(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(column)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn path_text(path: &RelativePath, cap: usize) -> io::Result<()> {
    let bytes = path
        .as_str()
        .len()
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_add(2048))
        .ok_or_else(|| invalid("native source index row state overflow"))?;
    if bytes > cap {
        return Err(invalid("native source index row state exceeds profile"));
    }
    Ok(())
}

pub(super) fn cursor_argument_state(bytes: usize) -> io::Result<usize> {
    if bytes == 0 {
        return Ok(0);
    }
    bytes
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_add(2048))
        .ok_or_else(|| invalid("native source index cursor argument state overflow"))
}

fn query_pragma_i64(db: &PinnedSqliteConnection, pragma: &'static str) -> io::Result<i64> {
    db.query_row(pragma, [], |row| row.get(0)).map_err(sql)
}

fn query_pragma_text(db: &PinnedSqliteConnection, pragma: &'static str) -> io::Result<String> {
    db.query_row(pragma, [], |row| match row.get_ref(0)? {
        rusqlite::types::ValueRef::Text(raw) if raw.len() <= 128 => row.get(0),
        _ => Err(rusqlite::Error::InvalidQuery),
    })
    .map_err(sql)
}

fn set_and_verify_connection_policy(
    db: &PinnedSqliteConnection,
    limits: SpoolIndexLimits,
) -> io::Result<()> {
    // The strict auxiliary VFS already limits the main inode to this caller's
    // reserved ceiling. Check the selected SQLite policy before any DDL, then
    // read back the cache size after rounding down to whole KiB.
    if limits.sqlite.main_logical_bytes == 0
        || limits.sqlite.main_allocated_bytes == 0
        || limits.cache_bytes < 1024
    {
        return Err(invalid("native source index SQLite ceiling is too small"));
    }
    db.pragma_update(None, "journal_mode", "OFF").map_err(sql)?;
    db.pragma_update(None, "synchronous", 0).map_err(sql)?;
    db.pragma_update(None, "temp_store", "FILE").map_err(sql)?;
    db.pragma_update(None, "mmap_size", 0).map_err(sql)?;
    let cache_kib = limits.cache_bytes / 1024;
    let cache_kib = i64::try_from(cache_kib)
        .map_err(|_| invalid("native source index cache profile exceeds range"))?;
    db.pragma_update(None, "cache_size", -cache_kib)
        .map_err(sql)?;

    let journal = query_pragma_text(db, "PRAGMA journal_mode")?;
    let synchronous = query_pragma_i64(db, "PRAGMA synchronous")?;
    let temp_store = query_pragma_i64(db, "PRAGMA temp_store")?;
    let mmap = query_pragma_i64(db, "PRAGMA mmap_size")?;
    let cache_readback = query_pragma_i64(db, "PRAGMA cache_size")?;
    let page_size = query_pragma_i64(db, "PRAGMA page_size")?;
    let expected_cache_kib = -cache_kib;
    if !journal.eq_ignore_ascii_case("off")
        || synchronous != 0
        || temp_store != 1
        || mmap != 0
        || cache_readback != expected_cache_kib
        || page_size <= 0
        || u64::try_from(page_size)
            .ok()
            .is_none_or(|page| page > limits.sqlite.main_allocated_bytes)
        || u64::try_from(cache_readback.unsigned_abs())
            .ok()
            .and_then(|kib| kib.checked_mul(1024))
            .is_none_or(|bytes| bytes > limits.cache_bytes as u64)
    {
        return Err(invalid("native source index SQLite policy changed"));
    }
    Ok(())
}

fn feed_membership(hash: &mut Digest256Hasher, path: &RelativePath, size: u64, sha: Digest256) {
    hash.update(&(path.as_str().len() as u64).to_be_bytes());
    hash.update(path.as_str().as_bytes());
    hash.update(&size.to_be_bytes());
    hash.update(sha.as_bytes());
}

/// SQLite and transient row bounds come from the enclosing native invocation
/// profile. No limit here grants a separate disk, I/O, clock or cancellation
/// budget; `SpoolCandidate::open_index_scope` joins its existing ledgers.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct SpoolIndexLimits {
    pub(crate) sqlite: PinnedSqliteAuxLimits,
    pub(crate) cache_bytes: usize,
    pub(crate) max_row_state_bytes: usize,
}

/// Evidence that the current-only Records/Item callback returned its sealed
/// report over this exact candidate index sink. This boundary intentionally
/// carries no native-admission authority: source selection, schema/render,
/// history, global identity/link checks, and final grammar rechecks remain
/// later obligations of the enclosing validator.
pub(crate) struct CandidateRecordsReportVerified {
    fence: CandidateFence,
    sink_identity: Arc<()>,
    membership: SourceMembershipV1,
    prepared_schema: CutPreparedSchemaExecutionBinding,
    selected_member_bytes: u64,
    record_source_bytes_read: u64,
    record_issue_count: usize,
    item_issue_count: usize,
    manifest_item_id_count: usize,
}

/// Logical retained state for the private callback boundary and its stable
/// Arc control block. The enclosing source invocation reserves this together
/// with the sink before constructing the report; this is not an RSS estimate.
pub(crate) const CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES: usize =
    std::mem::size_of::<CandidateRecordsReportVerified>() + 2 * std::mem::size_of::<usize>();

impl CandidateRecordsReportVerified {
    pub(crate) fn fence(&self) -> CandidateFence {
        self.fence
    }

    pub(crate) fn membership(&self) -> SourceMembershipV1 {
        self.membership
    }

    pub(crate) fn prepared_schema(&self) -> CutPreparedSchemaExecutionBinding {
        self.prepared_schema
    }

    pub(crate) fn selected_member_bytes(&self) -> u64 {
        self.selected_member_bytes
    }

    pub(crate) fn record_source_bytes_read(&self) -> u64 {
        self.record_source_bytes_read
    }

    pub(crate) fn record_issue_count(&self) -> usize {
        self.record_issue_count
    }

    pub(crate) fn item_issue_count(&self) -> usize {
        self.item_issue_count
    }

    pub(crate) fn manifest_item_id_count(&self) -> usize {
        self.manifest_item_id_count
    }
}

#[derive(Clone, Copy)]
enum FreshPairTable {
    Record,
    Claim,
    Semantic,
}

trait CandidateFenceSource {
    fn verify_prepared_request(
        &self,
        workspace: &std::fs::File,
        io: &PinnedSqliteIoBudget,
        space: &tos_source_store::PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
    ) -> io::Result<()>;
    fn shares_io_budget(&self, budget: &PinnedSqliteIoBudget) -> bool;
    fn tick(&self) -> io::Result<()>;
    fn matches_invocation(
        &self,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> bool;
    fn check_state(&self, bytes: usize) -> io::Result<()>;
    fn fence(&self) -> io::Result<CandidateFence>;
    fn member(&self, path: &RelativePath) -> io::Result<Option<tos_source_store::MemberMetadata>>;
    fn member_after(
        &self,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<tos_source_store::MemberMetadata>>;
    fn read(&self, path: &str, cap: usize) -> io::Result<Vec<u8>>;
    fn verify(&self, path: &str) -> io::Result<()>;
    fn base_identity_path_bounded(
        &self,
        id: &str,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<RelativePath>>;
}

impl CandidateFenceSource for SpoolCandidate<'_> {
    fn verify_prepared_request(
        &self,
        workspace: &std::fs::File,
        io: &PinnedSqliteIoBudget,
        space: &tos_source_store::PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
    ) -> io::Result<()> {
        SpoolCandidate::verify_prepared_request(self, workspace, io, space, deadline, cancelled)
    }
    fn shares_io_budget(&self, budget: &PinnedSqliteIoBudget) -> bool {
        SpoolCandidate::shares_io_budget(self, budget)
    }
    fn tick(&self) -> io::Result<()> {
        SpoolCandidate::tick(self)
    }

    fn matches_invocation(
        &self,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> bool {
        SpoolCandidate::matches_invocation(self, deadline, cancelled)
    }

    fn check_state(&self, bytes: usize) -> io::Result<()> {
        SpoolCandidate::check_state(self, bytes)
    }

    fn fence(&self) -> io::Result<CandidateFence> {
        SpoolCandidate::fence(self)
    }

    fn member(&self, path: &RelativePath) -> io::Result<Option<tos_source_store::MemberMetadata>> {
        SpoolCandidate::member(self, path)
    }

    fn member_after(
        &self,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<tos_source_store::MemberMetadata>> {
        SpoolCandidate::member_after(self, after)
    }

    fn read(&self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        SpoolCandidate::read(self, path, cap)
    }

    fn verify(&self, path: &str) -> io::Result<()> {
        SpoolCandidate::verify(self, path)
    }

    fn base_identity_path_bounded(
        &self,
        id: &str,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<RelativePath>> {
        SpoolCandidate::base_identity_path_bounded(self, id, max_owned_state_bytes)
    }
}

fn verify_coverage(
    candidate: &dyn CandidateFenceSource,
    expected: CandidateFence,
    row_limit: usize,
) -> io::Result<()> {
    candidate.tick()?;
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-val-full-membership-v1\0");
    let mut count = 0u64;
    let mut source_bytes = 0u64;
    let mut after = None;
    while let Some(member) = candidate.member_after(after.as_ref())? {
        candidate.tick()?;
        path_text(&member.path, row_limit)?;
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid("native source membership count overflow"))?;
        source_bytes = source_bytes
            .checked_add(member.size_bytes)
            .ok_or_else(|| invalid("native source membership byte count overflow"))?;
        feed_membership(&mut hash, &member.path, member.size_bytes, member.sha256);
        after = Some(member.path);
    }
    let membership = tos_source_store::SourceMembershipV1 {
        count,
        digest: hash.finalize(),
    };
    candidate.tick()?;
    if membership != expected.membership || source_bytes != expected.source_bytes {
        return Err(invalid(
            "native source candidate membership did not reach its fenced EOF",
        ));
    }
    Ok(())
}

struct SpoolInput<'a> {
    candidate: &'a dyn CandidateFenceSource,
    json: JsonLimits,
    json_state_bytes: usize,
    row_state_limit: usize,
}

impl CandidateIndexInput for SpoolInput<'_> {
    fn tick(&mut self) -> io::Result<()> {
        self.candidate.tick()
    }

    fn check_row_state(&self, bytes: usize) -> io::Result<()> {
        self.candidate.check_state(bytes)
    }

    fn row_state_limit(&self) -> usize {
        self.row_state_limit
    }

    fn member(&mut self, path: &str) -> io::Result<bool> {
        self.candidate.tick()?;
        let Ok(path) = RelativePath::parse(path) else {
            return Ok(false);
        };
        let found = self.candidate.member(&path)?.is_some();
        self.candidate.tick()?;
        Ok(found)
    }

    fn member_after(&mut self, after: Option<&str>) -> io::Result<Option<String>> {
        self.candidate.tick()?;
        let after = after
            .map(RelativePath::parse)
            .transpose()
            .map_err(|_| invalid("native source member cursor path is invalid"))?;
        let member = self.candidate.member_after(after.as_ref())?;
        self.candidate.tick()?;
        Ok(member.map(|member| member.path.as_str().to_owned()))
    }

    fn member_size(&mut self, path: &str) -> io::Result<u64> {
        let path = RelativePath::parse(path)
            .map_err(|_| invalid("native source member path is invalid"))?;
        self.candidate
            .member(&path)?
            .map(|member| member.size_bytes)
            .ok_or_else(|| invalid("native source member disappeared"))
    }

    fn member_digest(&mut self, path: &str) -> io::Result<Digest256> {
        let path = RelativePath::parse(path)
            .map_err(|_| invalid("native source member path is invalid"))?;
        self.candidate
            .member(&path)?
            .map(|member| member.sha256)
            .ok_or_else(|| invalid("native source member disappeared"))
    }

    fn read_raw(&mut self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        self.candidate.read(path, cap)
    }

    fn verify_member(&mut self, path: &str) -> io::Result<()> {
        self.candidate.verify(path)
    }

    fn json_limits(&self) -> JsonLimits {
        self.json
    }

    fn json_state_bytes(&self) -> usize {
        self.json_state_bytes
    }
}

/// Private mutable backend retained through full FND completion. Calling the
/// shared index kernel alone cannot expose rows to the manifest writer.
pub(crate) struct IndexSink<'candidate> {
    candidate: &'candidate dyn CandidateFenceSource,
    fence: CandidateFence,
    sink_identity: Arc<()>,
    row_limit: usize,
    fresh_sealed: bool,
    fresh_record_rows: u64,
    fresh_claim_rows: u64,
    fresh_semantic_rows: u64,
    records_collection_ordinals: [u64; 16],
    record_observation_count: u64,
    record_schema_diagnostic_count: u64,
    native_index_built: bool,
    selected_profile: SpoolIndexLimits,
    db: PinnedSqliteConnection,
    _scope: PinnedSqliteAuxScope,
}

impl<'candidate> IndexSink<'candidate> {
    pub(crate) fn open(
        candidate: &'candidate SpoolCandidate<'_>,
        limits: SpoolIndexLimits,
    ) -> io::Result<Self> {
        if limits.cache_bytes == 0
            || limits.cache_bytes > limits.max_row_state_bytes
            || limits.max_row_state_bytes == 0
            || limits.max_row_state_bytes == usize::MAX
        {
            return Err(invalid("native source index profile is invalid"));
        }
        candidate.tick()?;
        let fence = candidate.fence()?;
        candidate.check_state(limits.max_row_state_bytes)?;
        verify_coverage(candidate, fence, limits.max_row_state_bytes)?;
        // A retained Arc gives report tokens a stable, non-address-reuse
        // identity for this exact DB/scope. The enclosing invocation profile
        // must include this small identity handle in its retained-state ceiling.
        let sink_identity_state = std::mem::size_of::<Arc<()>>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .ok_or_else(|| invalid("native index scope identity state overflow"))?;
        candidate.check_state(sink_identity_state)?;
        let sink_identity = Arc::new(());
        let mut scope = candidate.open_index_scope(limits.sqlite)?;
        let db = scope
            .open_connection()
            .map_err(|_| invalid("native source index shared-budget SQLite open refused"))?;
        set_and_verify_connection_policy(&db, limits)?;
        db.execute_batch(
            "CREATE TABLE identities(id TEXT COLLATE BINARY PRIMARY KEY,path TEXT NOT NULL) WITHOUT ROWID;\
             CREATE INDEX identities_by_path ON identities(path COLLATE BINARY,id COLLATE BINARY);\
             CREATE TABLE dependencies(source TEXT COLLATE BINARY NOT NULL,target TEXT COLLATE BINARY NOT NULL,PRIMARY KEY(source,target)) WITHOUT ROWID;\
             CREATE INDEX dependencies_reverse_order ON dependencies(target COLLATE BINARY,source COLLATE BINARY);\
             CREATE TABLE fresh_record_rows(ordinal INTEGER PRIMARY KEY,id TEXT NOT NULL,source_ref TEXT NOT NULL);\
             CREATE TABLE fresh_claim_rows(ordinal INTEGER PRIMARY KEY,id TEXT NOT NULL,source_ref TEXT NOT NULL);\
             CREATE TABLE fresh_semantic_rows(ordinal INTEGER PRIMARY KEY,id TEXT COLLATE BINARY NOT NULL,path TEXT NOT NULL);\
             CREATE INDEX fresh_semantic_order ON fresh_semantic_rows(id COLLATE BINARY,ordinal);\
             CREATE TABLE sf_rows(collection INTEGER NOT NULL,seq INTEGER NOT NULL,key1 TEXT COLLATE BINARY NOT NULL,key2 TEXT COLLATE BINARY NOT NULL,payload BLOB NOT NULL,aux BLOB NOT NULL,state_bytes INTEGER NOT NULL,PRIMARY KEY(collection,seq)) WITHOUT ROWID;\
             CREATE UNIQUE INDEX sf_rows_keyed_unique ON sf_rows(collection,key1 COLLATE BINARY,key2 COLLATE BINARY) WHERE collection IN (0,1,2,4,5,6,7,8);\
             CREATE INDEX sf_rows_order ON sf_rows(collection,seq);\
             CREATE INDEX sf_rows_key_order ON sf_rows(collection,key1 COLLATE BINARY,key2 COLLATE BINARY);\
             CREATE TABLE sf_current_paths(path TEXT PRIMARY KEY COLLATE BINARY,record_id TEXT NOT NULL COLLATE BINARY,record_count INTEGER NOT NULL,schema_matches INTEGER NOT NULL,artifact_scope INTEGER NOT NULL,artifact_visited INTEGER NOT NULL) WITHOUT ROWID;\
             CREATE INDEX sf_unvisited_artifact_paths ON sf_current_paths(artifact_visited,path COLLATE BINARY) WHERE artifact_scope=1;\
             CREATE TABLE sf_candidate_artifact_schema_proofs(path TEXT PRIMARY KEY COLLATE BINARY,record_count INTEGER NOT NULL,target_diagnostic_count INTEGER NOT NULL,member_sha256_hex TEXT,member_size_bytes BLOB,diagnostic_unit_sha256_hex TEXT,diagnostic_report_sha256_hex TEXT,invalid INTEGER NOT NULL) WITHOUT ROWID;\
             CREATE TABLE sf_facts(collection INTEGER NOT NULL,ordinal INTEGER NOT NULL,key1 TEXT COLLATE BINARY NOT NULL,payload BLOB NOT NULL,state_bytes INTEGER NOT NULL,PRIMARY KEY(collection,ordinal)) WITHOUT ROWID;\
             CREATE INDEX sf_facts_key_order ON sf_facts(collection,key1 COLLATE BINARY,ordinal);",
        )
        .map_err(sql)?;
        candidate.tick()?;
        Ok(Self {
            candidate,
            fence,
            sink_identity,
            row_limit: limits.max_row_state_bytes,
            selected_profile: limits,
            fresh_sealed: false,
            fresh_record_rows: 0,
            fresh_claim_rows: 0,
            fresh_semantic_rows: 0,
            records_collection_ordinals: [0; 16],
            record_observation_count: 0,
            record_schema_diagnostic_count: 0,
            native_index_built: false,
            db,
            _scope: scope,
        })
    }

    /// Run the existing candidate-fenced Records+Item receiver against this
    /// exact SQLite sink and actual prepared candidate schema worker. This
    /// report does not stand in for candidate catalog/render phases or the
    /// remaining whole native admission callback.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn inspect_candidate_records(
        &mut self,
        input: &CandidateRecordsInput<'_, '_>,
        worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<CandidateFence>,
        limits: tos_validation::source_foundation_records::SourceFoundationRecordsLimits,
        require_local_payloads: bool,
        cancelled: &AtomicBool,
        record_executor: &mut tos_validation::record_biblio_cut::BiblioRecordExecutor,
        physical_facts: &tos_validation::source_foundation_discovery::SourcePhysicalFacts,
        payloads: &mut impl tos_validation::source_cut::CutPayloadReader,
        fact_budget: tos_validation::record_biblio_cut::SourceCutRecordFactBudget,
        page_budget: tos_validation::source_foundation_records::SourceFoundationRecordsPageBudget,
        retained_state_bytes: usize,
        max_operation_state_bytes: usize,
    ) -> io::Result<CandidateRecordsReportVerified> {
        self.with_candidate_records_report(
            input,
            worker,
            limits,
            require_local_payloads,
            cancelled,
            record_executor,
            physical_facts,
            payloads,
            fact_budget,
            page_budget,
            retained_state_bytes,
            max_operation_state_bytes,
            0,
            |_, _, _, _, _| Ok(()),
        )
        .map(|(verified, ())| verified)
    }

    /// Keep the genuine stored report and its read-only index borrowed through
    /// the caller's bounded dependent owner phases. The caller cannot return a
    /// report/store borrow: only its owned result escapes after report drop.
    /// Fresh catalog rows may be written only after this method returns.
    /// `callback_state_bytes` is an inclusive simultaneous bound for callback
    /// captures/workspace and the owned result, including its heap allocations.
    /// Its inline result header is checked here before any report read; actual
    /// owner report cost remains part of the caller's whole operation ledger.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_candidate_records_report<
        T,
        P: tos_validation::source_cut::CutPayloadReader,
    >(
        &mut self,
        input: &CandidateRecordsInput<'_, '_>,
        worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<CandidateFence>,
        limits: tos_validation::source_foundation_records::SourceFoundationRecordsLimits,
        require_local_payloads: bool,
        cancelled: &AtomicBool,
        record_executor: &mut tos_validation::record_biblio_cut::BiblioRecordExecutor,
        physical_facts: &tos_validation::source_foundation_discovery::SourcePhysicalFacts,
        payloads: &mut P,
        fact_budget: tos_validation::record_biblio_cut::SourceCutRecordFactBudget,
        page_budget: tos_validation::source_foundation_records::SourceFoundationRecordsPageBudget,
        retained_state_bytes: usize,
        max_operation_state_bytes: usize,
        callback_state_bytes: usize,
        receive: impl FnOnce(
            &SourceFoundationRecordsStreamedReport<'_, CandidateFence>,
            &CandidateRecordsReportVerified,
            &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<CandidateFence>,
            &mut tos_validation::record_biblio_cut::BiblioRecordExecutor,
            &mut P,
        ) -> io::Result<T>,
    ) -> io::Result<(CandidateRecordsReportVerified, T)> {
        let candidate = self.candidate;
        let fence = self.fence;
        if callback_state_bytes == usize::MAX || callback_state_bytes < std::mem::size_of::<T>() {
            input.abandon();
            return Err(invalid(
                "candidate dependent callback state omits owned result",
            ));
        }
        let retained = retained_state_bytes
            .checked_add(callback_state_bytes)
            .and_then(|bytes| {
                bytes.checked_add(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)
            })
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Arc<()>>()))
            .filter(|bytes| *bytes <= max_operation_state_bytes)
            .ok_or_else(|| invalid("candidate dependent callback state exceeds operation"))?;
        if let Err(error) = candidate.check_state(retained) {
            input.abandon();
            return Err(error);
        }
        // Clone the stable scope handle before lending the store mutably to
        // the kernel. Its allocation has already been admitted above.
        let sink_identity = Arc::clone(&self.sink_identity);
        let result = (|| {
            let deadline = limits.operation.deadline;
            let expected_schema = worker.prepared_execution_binding();
            let report = source_admission_candidate_schema::inspect_candidate_records_stored(
                input,
                worker,
                limits,
                require_local_payloads,
                cancelled,
                record_executor,
                physical_facts,
                payloads,
                fact_budget,
                page_budget,
                self,
                retained,
                max_operation_state_bytes,
            )
            .map_err(|_| invalid("candidate Records/Item receiver refused"))?;
            // This report was constructed directly by the maintained receiver
            // over this exact mutable store loan. No externally supplied
            // report or reconstructed private report constructor enters here.
            let verified = Self::verify_records_report_bound(
                candidate,
                fence,
                &sink_identity,
                &report,
                expected_schema,
                deadline,
                cancelled,
            )?;
            let value = receive(&report, &verified, worker, record_executor, payloads)?;
            candidate.tick()?;
            if candidate.fence()? != fence {
                return Err(invalid(
                    "candidate changed during dependent Records callback",
                ));
            }
            drop(report);
            Ok((verified, value))
        })();
        if result.is_err() {
            input.abandon();
        }
        result
    }

    pub(crate) fn build(
        &mut self,
        records: &CandidateRecordsReportVerified,
        limits: IndexLimits,
        json: JsonLimits,
        json_state_bytes: usize,
        schemas: &mut SchemaCheck<'_>,
    ) -> io::Result<usize> {
        self.candidate.tick()?;
        if records.fence != self.fence
            || !Arc::ptr_eq(&records.sink_identity, &self.sink_identity)
            || records.membership != self.fence.membership
            || records.selected_member_bytes != self.fence.source_bytes
            || self.candidate.fence()? != self.fence
        {
            return Err(invalid(
                "native source records report belongs to another candidate fence",
            ));
        }
        if self.fresh_sealed {
            return Err(invalid("native source row spool already consumed"));
        }
        self.validate_fresh_tables()?;
        self.fresh_sealed = true;
        let candidate = self.candidate;
        let mut input = SpoolInput {
            candidate,
            json,
            json_state_bytes,
            row_state_limit: self.row_limit,
        };
        let mut base_identity = |id: &str, max_state_bytes: usize| {
            candidate
                .base_identity_path_bounded(id, max_state_bytes)
                .map(|path| path.map(BaseIdentityPath::Streamed))
        };
        let retained = source_admission_index::build_index_into(
            &mut input,
            &mut base_identity,
            limits,
            self,
            schemas,
        )?;
        self.native_index_built = true;
        Ok(retained)
    }

    /// Bind the opaque VAL report to this sink only after the Records/Item
    /// kernel's full source traversal, currentness checks, and post-Item fence
    /// verification returned successfully. The report is not a claim that the
    /// candidate has no issues or that the remaining native callback passed.
    pub(crate) fn verify_records_report(
        &self,
        report: &SourceFoundationRecordsStreamedReport<'_, CandidateFence>,
        expected_schema: CutPreparedSchemaExecutionBinding,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<CandidateRecordsReportVerified> {
        if !report.index().is_backed_by(self) {
            return Err(invalid("native Records report belongs to another store"));
        }
        Self::verify_records_report_bound(
            self.candidate,
            self.fence,
            &self.sink_identity,
            report,
            expected_schema,
            deadline,
            cancelled,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn verify_records_report_bound(
        candidate: &dyn CandidateFenceSource,
        fence: CandidateFence,
        sink_identity: &Arc<()>,
        report: &SourceFoundationRecordsStreamedReport<'_, CandidateFence>,
        expected_schema: CutPreparedSchemaExecutionBinding,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<CandidateRecordsReportVerified> {
        if !candidate.matches_invocation(deadline, cancelled) {
            return Err(invalid(
                "native source records report invocation differs from candidate",
            ));
        }
        candidate.tick()?;
        let before = candidate.fence()?;
        if before != fence
            || report.input_identity() != &fence
            || *report.source_membership() != fence.membership
            || report.cost().selected_current_member_bytes != fence.source_bytes
        {
            return Err(invalid(
                "native source records report is not bound to this candidate index sink",
            ));
        }
        let schema = report
            .candidate_schema_identity()
            .ok_or_else(|| invalid("candidate Records report omitted prepared schema identity"))?;
        if schema.prepared_execution_binding() != expected_schema
            || schema.profile() != expected_schema.schema_profile
            || schema.schema_set_digest() != expected_schema.schema_set_sha256
        {
            return Err(invalid(
                "candidate Records report differs from the prepared schema worker",
            ));
        }
        let record_source_bytes_read = report.record_usage().source_bytes_read;
        let selected_member_bytes = report.cost().selected_current_member_bytes;
        let record_issue_count = report.record_usage().observed_issue_count;
        let item_issue_count = report.items().issue_count;
        let manifest_item_id_count = report.items().manifest_item_id_count;
        candidate.check_state(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)?;
        candidate.tick()?;
        if !candidate.matches_invocation(deadline, cancelled) || candidate.fence()? != before {
            return Err(invalid(
                "native source candidate changed while binding the Records report",
            ));
        }
        Ok(CandidateRecordsReportVerified {
            fence: before,
            sink_identity: Arc::clone(sink_identity),
            membership: *report.source_membership(),
            prepared_schema: expected_schema,
            selected_member_bytes,
            record_source_bytes_read,
            record_issue_count,
            item_issue_count,
            manifest_item_id_count,
        })
    }

    fn validate_fresh_tables(&self) -> io::Result<()> {
        self.candidate.tick()?;
        let records = row_count(&self.db, "SELECT COUNT(*) FROM fresh_record_rows")?;
        let claims = row_count(&self.db, "SELECT COUNT(*) FROM fresh_claim_rows")?;
        let semantic = row_count(&self.db, "SELECT COUNT(*) FROM fresh_semantic_rows")?;
        self.candidate.tick()?;
        if records != self.fresh_record_rows
            || claims != self.fresh_claim_rows
            || semantic != self.fresh_semantic_rows
        {
            return Err(invalid("fresh native catalog row spool is incomplete"));
        }
        Ok(())
    }

    fn push_fresh_pair(&mut self, table: FreshPairTable, id: &str, value: &str) -> io::Result<()> {
        if self.fresh_sealed {
            return Err(invalid("native source row spool is sealed"));
        }
        let row_state = text_pair_state_upper_bound(id.len(), value.len())?;
        self.candidate.check_state(row_state)?;
        let (ordinal, insert) = match table {
            FreshPairTable::Record => (
                self.fresh_record_rows,
                "INSERT INTO fresh_record_rows(ordinal,id,source_ref) VALUES(?1,?2,?3)",
            ),
            FreshPairTable::Claim => (
                self.fresh_claim_rows,
                "INSERT INTO fresh_claim_rows(ordinal,id,source_ref) VALUES(?1,?2,?3)",
            ),
            FreshPairTable::Semantic => (
                self.fresh_semantic_rows,
                "INSERT INTO fresh_semantic_rows(ordinal,id,path) VALUES(?1,?2,?3)",
            ),
        };
        let ordinal = i64::try_from(ordinal)
            .map_err(|_| invalid("native source row ordinal exceeds SQLite range"))?;
        self.candidate.tick()?;
        self.db
            .execute(insert, params![ordinal, id, value])
            .map_err(sql)?;
        self.candidate.tick()?;
        match table {
            FreshPairTable::Record => {
                self.fresh_record_rows = self
                    .fresh_record_rows
                    .checked_add(1)
                    .ok_or_else(|| invalid("fresh record source row count overflow"))?;
            }
            FreshPairTable::Claim => {
                self.fresh_claim_rows = self
                    .fresh_claim_rows
                    .checked_add(1)
                    .ok_or_else(|| invalid("fresh claim source row count overflow"))?;
            }
            FreshPairTable::Semantic => {
                self.fresh_semantic_rows = self
                    .fresh_semantic_rows
                    .checked_add(1)
                    .ok_or_else(|| invalid("fresh semantic source row count overflow"))?;
            }
        }
        Ok(())
    }

    fn pair_lengths_at(
        &self,
        table: FreshPairTable,
        ordinal: u64,
    ) -> io::Result<Option<(usize, usize)>> {
        let query = match table {
            FreshPairTable::Record => {
                "SELECT length(CAST(id AS BLOB)),length(CAST(source_ref AS BLOB)) FROM fresh_record_rows WHERE ordinal=?1"
            }
            FreshPairTable::Claim => {
                "SELECT length(CAST(id AS BLOB)),length(CAST(source_ref AS BLOB)) FROM fresh_claim_rows WHERE ordinal=?1"
            }
            FreshPairTable::Semantic => {
                "SELECT length(CAST(id AS BLOB)),length(CAST(path AS BLOB)) FROM fresh_semantic_rows WHERE ordinal=?1"
            }
        };
        let ordinal = i64::try_from(ordinal)
            .map_err(|_| invalid("native source row ordinal exceeds SQLite range"))?;
        self.candidate.tick()?;
        let lengths = self
            .db
            .query_row(query, [ordinal], text_lengths)
            .optional()
            .map_err(sql)?;
        self.candidate.tick()?;
        Ok(lengths)
    }

    fn pair_at(
        &self,
        table: FreshPairTable,
        ordinal: u64,
        expected_lengths: (usize, usize),
    ) -> io::Result<Option<(String, String)>> {
        let query = match table {
            FreshPairTable::Record => {
                "SELECT id,source_ref FROM fresh_record_rows WHERE ordinal=?1"
            }
            FreshPairTable::Claim => "SELECT id,source_ref FROM fresh_claim_rows WHERE ordinal=?1",
            FreshPairTable::Semantic => "SELECT id,path FROM fresh_semantic_rows WHERE ordinal=?1",
        };
        let ordinal = i64::try_from(ordinal)
            .map_err(|_| invalid("native source row ordinal exceeds SQLite range"))?;
        self.candidate.check_state(text_pair_state_upper_bound(
            expected_lengths.0,
            expected_lengths.1,
        )?)?;
        self.candidate.tick()?;
        let pair = self
            .db
            .query_row(query, [ordinal], |row| {
                bounded_text_pair(row, 0, 1, self.row_limit)
            })
            .optional()
            .map_err(sql)?;
        self.candidate.tick()?;
        if pair.as_ref().is_some_and(|(first, second)| {
            first.len() != expected_lengths.0 || second.len() != expected_lengths.1
        }) {
            return Err(invalid("native source row changed during cursor read"));
        }
        Ok(pair)
    }

    fn semantic_next_lengths(
        &self,
        after_id: Option<&str>,
        after_ordinal: i64,
    ) -> io::Result<Option<(usize, usize, i64)>> {
        let query = match after_id {
            Some(_) => {
                "SELECT length(CAST(id AS BLOB)),length(CAST(path AS BLOB)),ordinal FROM fresh_semantic_rows WHERE (id COLLATE BINARY,ordinal)>(?1 COLLATE BINARY,?2) ORDER BY id COLLATE BINARY,ordinal LIMIT 1"
            }
            None => {
                "SELECT length(CAST(id AS BLOB)),length(CAST(path AS BLOB)),ordinal FROM fresh_semantic_rows ORDER BY id COLLATE BINARY,ordinal LIMIT 1"
            }
        };
        self.candidate.tick()?;
        let next = match after_id {
            Some(after) => self
                .db
                .query_row(query, params![after, after_ordinal], |row| {
                    Ok((text_length(row, 0)?, text_length(row, 1)?, row.get(2)?))
                })
                .optional(),
            None => self
                .db
                .query_row(query, [], |row| {
                    Ok((text_length(row, 0)?, text_length(row, 1)?, row.get(2)?))
                })
                .optional(),
        }
        .map_err(sql)?;
        self.candidate.tick()?;
        Ok(next)
    }

    fn semantic_pair_at(
        &self,
        ordinal: i64,
        expected_lengths: (usize, usize),
    ) -> io::Result<(String, String)> {
        self.candidate.check_state(text_pair_state_upper_bound(
            expected_lengths.0,
            expected_lengths.1,
        )?)?;
        self.candidate.tick()?;
        let pair = self
            .db
            .query_row(
                "SELECT id,path FROM fresh_semantic_rows WHERE ordinal=?1",
                [ordinal],
                |row| bounded_text_pair(row, 0, 1, self.row_limit),
            )
            .map_err(sql)?;
        self.candidate.tick()?;
        if pair.0.len() != expected_lengths.0 || pair.1.len() != expected_lengths.1 {
            return Err(invalid(
                "native semantic source row changed during cursor read",
            ));
        }
        Ok(pair)
    }

    /// Only the full native validator owns this token, and calls this after its
    /// complete source/history/schema callback and final source rechecks.
    pub(crate) fn finish(
        self,
        complete: NativeAdmissionComplete,
    ) -> io::Result<IndexView<'candidate>> {
        self.candidate.verify_prepared_request(
            complete.original_workspace(),
            complete.original_io(),
            complete.original_space(),
            complete.deadline(),
            complete.cancelled(),
        )?;
        let records = complete.records();
        if records.fence != self.fence
            || records.membership != self.fence.membership
            || records.selected_member_bytes != self.fence.source_bytes
            || !Arc::ptr_eq(&records.sink_identity, &self.sink_identity)
            || complete.index_profile() != self.selected_profile
            || !self.candidate.shares_io_budget(complete.original_io())
        {
            return Err(invalid(
                "native completion belongs to another candidate or index scope",
            ));
        }
        if !self.fresh_sealed || !self.native_index_built {
            return Err(invalid("native source index operation is incomplete"));
        }
        self.candidate.tick()?;
        if self.candidate.fence()? != self.fence {
            return Err(invalid("native source candidate fence changed"));
        }
        verify_coverage(self.candidate, self.fence, self.row_limit)?;
        let identity_count = row_count(&self.db, "SELECT COUNT(*) FROM identities")?;
        let dependency_count = row_count(&self.db, "SELECT COUNT(*) FROM dependencies")?;
        let dependency_source_count = row_count(
            &self.db,
            "SELECT COUNT(*) FROM (SELECT source FROM dependencies GROUP BY source)",
        )?;
        self.candidate.tick()?;
        if self.candidate.fence()? != self.fence {
            return Err(invalid("native source candidate fence changed"));
        }
        Ok(IndexView {
            candidate: self.candidate,
            fence: self.fence,
            complete,
            identity_count,
            dependency_source_count,
            dependency_count,
            row_limit: self.row_limit,
            db: self.db,
            _scope: self._scope,
        })
    }
}

impl FreshIndexRowsWriter for IndexSink<'_> {
    fn push_record(&mut self, id: &str, source_ref: &str) -> io::Result<()> {
        self.push_fresh_pair(FreshPairTable::Record, id, source_ref)
    }

    fn push_claim(&mut self, id: &str, source_ref: &str) -> io::Result<()> {
        self.push_fresh_pair(FreshPairTable::Claim, id, source_ref)
    }

    fn push_native_semantic(&mut self, id: &str, path: &str) -> io::Result<()> {
        self.push_fresh_pair(FreshPairTable::Semantic, id, path)
    }
}

impl AdmissionIndexBackend for IndexSink<'_> {
    fn retains_index_rows(&self) -> bool {
        false
    }

    fn source_record_count(&self) -> io::Result<u64> {
        Ok(self.fresh_record_rows)
    }

    fn source_claim_count(&self) -> io::Result<u64> {
        Ok(self.fresh_claim_rows)
    }

    fn source_native_semantic_count(&self) -> io::Result<u64> {
        self.candidate.tick()?;
        let count = row_count(
            &self.db,
            "SELECT COUNT(*) FROM (SELECT id FROM fresh_semantic_rows GROUP BY id)",
        )?;
        self.candidate.tick()?;
        Ok(count)
    }

    fn source_native_semantic_row_count(&self) -> io::Result<u64> {
        self.candidate.tick()?;
        let count = row_count(&self.db, "SELECT COUNT(*) FROM fresh_semantic_rows")?;
        self.candidate.tick()?;
        Ok(count)
    }

    fn for_each_source_record(
        &mut self,
        visit: &mut dyn FnMut(&mut dyn AdmissionIndexBackend, &str, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        let expected = self.fresh_record_rows;
        let candidate = self.candidate;
        for ordinal in 0..expected {
            let lengths = self
                .pair_lengths_at(FreshPairTable::Record, ordinal)?
                .ok_or_else(|| invalid("fresh record row spool ended before its count"))?;
            candidate.check_state(text_pair_state_upper_bound(lengths.0, lengths.1)?)?;
            let (id, source_ref) = self
                .pair_at(FreshPairTable::Record, ordinal, lengths)?
                .ok_or_else(|| invalid("fresh record row spool changed during read"))?;
            candidate.tick()?;
            visit(self, &id, &source_ref)?;
            candidate.tick()?;
        }
        candidate.tick()?;
        let observed = row_count(&self.db, "SELECT COUNT(*) FROM fresh_record_rows")?;
        candidate.tick()?;
        if observed != expected {
            return Err(invalid("fresh record row spool did not reach EOF"));
        }
        Ok(())
    }

    fn for_each_source_claim(
        &mut self,
        visit: &mut dyn FnMut(&mut dyn AdmissionIndexBackend, &str, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        let expected = self.fresh_claim_rows;
        let candidate = self.candidate;
        for ordinal in 0..expected {
            let lengths = self
                .pair_lengths_at(FreshPairTable::Claim, ordinal)?
                .ok_or_else(|| invalid("fresh claim row spool ended before its count"))?;
            candidate.check_state(text_pair_state_upper_bound(lengths.0, lengths.1)?)?;
            let (id, source_ref) = self
                .pair_at(FreshPairTable::Claim, ordinal, lengths)?
                .ok_or_else(|| invalid("fresh claim row spool changed during read"))?;
            candidate.tick()?;
            visit(self, &id, &source_ref)?;
            candidate.tick()?;
        }
        candidate.tick()?;
        let observed = row_count(&self.db, "SELECT COUNT(*) FROM fresh_claim_rows")?;
        candidate.tick()?;
        if observed != expected {
            return Err(invalid("fresh claim row spool did not reach EOF"));
        }
        Ok(())
    }

    fn for_each_source_native_semantic(
        &mut self,
        visit: &mut dyn for<'row> FnMut(NativeSemanticStep<'row>) -> io::Result<()>,
    ) -> io::Result<()> {
        let expected_rows = self.fresh_semantic_rows;
        let candidate = self.candidate;
        let mut after_id: Option<String> = None;
        let mut after_ordinal = -1i64;
        let mut seen = 0u64;
        while seen < expected_rows {
            let cursor_id_bytes = after_id.as_ref().map_or(0, String::len);
            let (id_bytes, path_bytes, ordinal) = self
                .semantic_next_lengths(after_id.as_deref(), after_ordinal)?
                .ok_or_else(|| invalid("fresh semantic row spool ended before its count"))?;
            visit(NativeSemanticStep::Preflight {
                cursor_id_bytes,
                id_bytes,
                path_bytes,
            })?;
            let (id, path) = self.semantic_pair_at(ordinal, (id_bytes, path_bytes))?;
            candidate.tick()?;
            let group_first = after_id.as_deref() != Some(id.as_str());
            visit(NativeSemanticStep::Row {
                backend: self,
                cursor_id_bytes,
                id: &id,
                path: &path,
                group_first,
            })?;
            candidate.tick()?;
            after_id = Some(id);
            after_ordinal = ordinal;
            seen = seen
                .checked_add(1)
                .ok_or_else(|| invalid("fresh semantic row count overflow"))?;
        }
        candidate.tick()?;
        let observed = row_count(&self.db, "SELECT COUNT(*) FROM fresh_semantic_rows")?;
        candidate.tick()?;
        if observed != expected_rows {
            return Err(invalid("fresh semantic row spool did not reach EOF"));
        }
        Ok(())
    }

    fn identity_path(
        &mut self,
        id: &str,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<String>> {
        self.candidate.check_state(max_owned_state_bytes)?;
        let result = self
            .db
            .query_row("SELECT path FROM identities WHERE id=?1", [id], |row| {
                bounded_text(row, 0, max_owned_state_bytes)
            })
            .optional()
            .map_err(sql)?;
        self.candidate.tick()?;
        Ok(result)
    }

    fn insert_identity(&mut self, id: &str, path: &str) -> io::Result<()> {
        let state = id
            .len()
            .checked_add(path.len())
            .and_then(|bytes| bytes.checked_mul(16))
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| invalid("native source identity insert state overflow"))?;
        self.candidate.check_state(state)?;
        self.db
            .execute(
                "INSERT INTO identities(id,path) VALUES(?1,?2)",
                params![id, path],
            )
            .map_err(sql)?;
        self.candidate.tick()
    }

    fn dependency_source_exists(&mut self, source: &str) -> io::Result<bool> {
        self.candidate.check_state(
            source
                .len()
                .checked_mul(16)
                .and_then(|bytes| bytes.checked_add(4096))
                .ok_or_else(|| invalid("native dependency lookup state overflow"))?,
        )?;
        let result = self
            .db
            .query_row(
                "SELECT 1 FROM dependencies WHERE source=?1 LIMIT 1",
                [source],
                |_| Ok(()),
            )
            .optional()
            .map_err(sql)?
            .is_some();
        self.candidate.tick()?;
        Ok(result)
    }

    fn dependency_exists(&mut self, source: &str, target: &str) -> io::Result<bool> {
        let state = source
            .len()
            .checked_add(target.len())
            .and_then(|bytes| bytes.checked_mul(16))
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| invalid("native dependency lookup state overflow"))?;
        self.candidate.check_state(state)?;
        let result = self
            .db
            .query_row(
                "SELECT 1 FROM dependencies WHERE source=?1 AND target=?2",
                params![source, target],
                |_| Ok(()),
            )
            .optional()
            .map_err(sql)?
            .is_some();
        self.candidate.tick()?;
        Ok(result)
    }

    fn insert_dependency(&mut self, source: &str, target: &str) -> io::Result<()> {
        let state = source
            .len()
            .checked_add(target.len())
            .and_then(|bytes| bytes.checked_mul(16))
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| invalid("native dependency insert state overflow"))?;
        self.candidate.check_state(state)?;
        self.db
            .execute(
                "INSERT INTO dependencies(source,target) VALUES(?1,?2)",
                params![source, target],
            )
            .map_err(sql)?;
        self.candidate.tick()
    }
}

fn row_count(db: &PinnedSqliteConnection, query: &'static str) -> io::Result<u64> {
    let count = db
        .query_row(query, [], |row| bounded_u64(row, 0))
        .map_err(sql)?;
    Ok(count)
}

/// Opaque native-validator result. The SQLite connection and its budget lease
/// remain alive until the held source serializer has consumed every cursor.
pub(crate) struct IndexView<'candidate> {
    complete: NativeAdmissionComplete,
    candidate: &'candidate dyn CandidateFenceSource,
    fence: CandidateFence,
    identity_count: u64,
    dependency_source_count: u64,
    dependency_count: u64,
    row_limit: usize,
    db: PinnedSqliteConnection,
    _scope: PinnedSqliteAuxScope,
}

impl IndexView<'_> {
    /// Declared retained Rust and nominal SQLite-cache state while publication
    /// consumes the completed index. Opaque SQLite allocator pages and process
    /// RSS remain under the enclosing native operation's existing external
    /// memory prerequisite.
    pub(crate) fn declared_retained_state_bytes(&self) -> io::Result<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.complete.index_profile().cache_bytes)
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or_else(|| invalid("native index retained state overflow"))
    }

    /// The actual per-row allowance used by this retained SQLite index while
    /// producing owned identity and dependency rows for the V2 tree writer.
    pub(crate) fn writer_row_state_limit(&self) -> usize {
        self.row_limit
    }

    pub(crate) fn publication_epoch(&self) -> &tos_source_store::MetadataPublicationEpoch {
        self.complete.original_epoch()
    }
    pub(crate) fn prepared_schema(&self) -> CutPreparedSchemaExecutionBinding {
        self.complete.records().prepared_schema()
    }
    /// Invocation-selected persistent V2 IO/allocation limits carried only by
    /// the genuine native completion witness. This is a budget profile, not a
    /// source or publication permission.
    pub(crate) fn segment_v2_budget(
        &self,
    ) -> Option<&super::source_foundation_admission::NativeSegmentV2Budget> {
        self.complete.segment_v2()
    }
    /// Bind a publication receiver to this genuinely sealed scope. A base-only
    /// candidate cannot stand in for a different pending merged membership.
    pub(crate) fn verify_publication_receiver(
        &self,
        candidate: &SpoolCandidate<'_>,
        selected_index: SpoolIndexLimits,
    ) -> io::Result<()> {
        if candidate.fence()? != self.fence
            || !candidate.shares_io_budget(self.complete.original_io())
            || selected_index != self.complete.index_profile()
        {
            return Err(invalid(
                "native publication receiver differs from completed candidate",
            ));
        }
        self.verify_candidate()
    }

    pub(crate) fn fence(&self) -> CandidateFence {
        self.fence
    }

    pub(crate) fn identity_count(&self) -> u64 {
        self.identity_count
    }

    pub(crate) fn dependency_source_count(&self) -> u64 {
        self.dependency_source_count
    }

    pub(crate) fn dependency_count(&self) -> u64 {
        self.dependency_count
    }

    pub(crate) fn verify_candidate(&self) -> io::Result<()> {
        self.candidate.verify_prepared_request(
            self.complete.original_workspace(),
            self.complete.original_io(),
            self.complete.original_space(),
            self.complete.deadline(),
            self.complete.cancelled(),
        )?;
        self.candidate.tick()?;
        if self.complete.records().fence() != self.fence
            || self.complete.records().membership() != self.fence.membership
            || !self.candidate.shares_io_budget(self.complete.original_io())
        {
            return Err(invalid("native completed index binding changed"));
        }
        if self.candidate.fence()? != self.fence {
            return Err(invalid("native source candidate fence changed"));
        }
        verify_coverage(self.candidate, self.fence, self.row_limit)
    }

    pub(crate) fn identities_after(
        &self,
        after: Option<&str>,
    ) -> io::Result<Option<(String, RelativePath)>> {
        self.candidate.tick()?;
        // The previous key remains live while SQLite constructs the returned
        // (id,path) strings and RelativePath. Subtract its bound before asking
        // rusqlite for owned text, and let the row converter enforce the
        // remaining allowance before allocating either returned String.
        let argument_state = cursor_argument_state(after.map_or(0, str::len))?;
        self.candidate.check_state(self.row_limit)?;
        let returned_row_limit = self
            .row_limit
            .checked_sub(argument_state)
            .ok_or_else(|| invalid("native identity cursor state exceeds profile"))?;
        let row = match after {
            Some(after) => self.db.query_row(
                "SELECT id,path FROM identities WHERE id>?1 ORDER BY id LIMIT 1",
                [after],
                |row| bounded_text_pair(row, 0, 1, returned_row_limit),
            ),
            None => self.db.query_row(
                "SELECT id,path FROM identities ORDER BY id LIMIT 1",
                [],
                |row| bounded_text_pair(row, 0, 1, returned_row_limit),
            ),
        }
        .optional()
        .map_err(sql)?;
        let row = row
            .map(|(id, path)| {
                Ok((
                    id,
                    RelativePath::parse(&path)
                        .map_err(|_| invalid("native index identity path is invalid"))?,
                ))
            })
            .transpose()?;
        self.candidate.tick()?;
        Ok(row)
    }

    /// Reverse-cursor identities owned by one changed source path. Native
    /// identity validation has already completed; this is bounded writer input
    /// for the corresponding authenticated V2 successor delta.
    pub(crate) fn identity_for_path_after(
        &self,
        path: &RelativePath,
        after_id: Option<&str>,
    ) -> io::Result<Option<String>> {
        self.candidate.tick()?;
        path_text(path, self.row_limit)?;
        let argument_bytes = path
            .as_str()
            .len()
            .checked_add(after_id.map_or(0, str::len))
            .ok_or_else(|| invalid("native identity path cursor argument overflow"))?;
        let argument = cursor_argument_state(argument_bytes)?;
        self.candidate.check_state(self.row_limit)?;
        let result_limit = self
            .row_limit
            .checked_sub(argument)
            .ok_or_else(|| invalid("native identity path cursor state exceeds profile"))?;
        let id = match after_id {
            Some(after) => self.db.query_row(
                "SELECT id FROM identities WHERE path=?1 AND id>?2 ORDER BY id LIMIT 1",
                params![path.as_str(), after],
                |row| bounded_text(row, 0, result_limit),
            ),
            None => self.db.query_row(
                "SELECT id FROM identities WHERE path=?1 ORDER BY id LIMIT 1",
                [path.as_str()],
                |row| bounded_text(row, 0, result_limit),
            ),
        }
        .optional()
        .map_err(sql)?;
        self.candidate.tick()?;
        Ok(id)
    }

    pub(crate) fn dependency_source_after(
        &self,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<RelativePath>> {
        self.candidate.tick()?;
        let argument_state = cursor_argument_state(after.map_or(0, |path| path.as_str().len()))?;
        self.candidate.check_state(self.row_limit)?;
        let returned_row_limit = self
            .row_limit
            .checked_sub(argument_state)
            .ok_or_else(|| invalid("native dependency cursor state exceeds profile"))?;
        let source = match after {
            Some(after) => self.db.query_row(
                "SELECT source FROM dependencies WHERE source>?1 ORDER BY source LIMIT 1",
                [after.as_str()],
                |row| bounded_text(row, 0, returned_row_limit),
            ),
            None => self.db.query_row(
                "SELECT source FROM dependencies ORDER BY source LIMIT 1",
                [],
                |row| bounded_text(row, 0, returned_row_limit),
            ),
        }
        .optional()
        .map_err(sql)?;
        let source = source
            .map(|source| {
                RelativePath::parse(&source)
                    .map_err(|_| invalid("native dependency source path is invalid"))
            })
            .transpose()?;
        self.candidate.tick()?;
        Ok(source)
    }

    /// Stream the actual maintained edge table in the chosen native key order.
    /// The tuple is always (source,target); no native row or completion is minted.
    pub(crate) fn dependency_pair_after(
        &self,
        direction: source_admission_index::NativeDependencyDirectionV1,
        after: Option<(&RelativePath, &RelativePath)>,
    ) -> io::Result<Option<(RelativePath, RelativePath)>> {
        self.candidate.tick()?;
        let argument_bytes = match after {
            Some((source, target)) => {
                path_text(source, self.row_limit)?;
                path_text(target, self.row_limit)?;
                source
                    .as_str()
                    .len()
                    .checked_add(target.as_str().len())
                    .ok_or_else(|| invalid("native dependency pair cursor argument overflow"))?
            }
            None => 0,
        };
        let argument_state = cursor_argument_state(argument_bytes)?;
        self.candidate.check_state(self.row_limit)?;
        let returned_row_limit = self
            .row_limit
            .checked_sub(argument_state)
            .ok_or_else(|| invalid("native dependency pair cursor state exceeds profile"))?;
        use source_admission_index::NativeDependencyDirectionV1::{Forward, Reverse};
        // Both orders have a maintained SQLite index. The pinned connection
        // accounts reverse-index insertion pages in the SAME IO/space budget.
        let row = match (direction, after) {
            (Forward, Some((source, target))) => self.db.query_row(
                "SELECT source,target FROM dependencies WHERE (source,target)>(?1,?2) ORDER BY source,target LIMIT 1",
                params![source.as_str(),target.as_str()],
                |row| bounded_text_pair(row,0,1,returned_row_limit)),
            (Reverse, Some((source, target))) => self.db.query_row(
                "SELECT source,target FROM dependencies INDEXED BY dependencies_reverse_order WHERE (target,source)>(?1,?2) ORDER BY target,source LIMIT 1",
                params![target.as_str(),source.as_str()],
                |row| bounded_text_pair(row,0,1,returned_row_limit)),
            (Forward, None) => self.db.query_row(
                "SELECT source,target FROM dependencies ORDER BY source,target LIMIT 1", [],
                |row| bounded_text_pair(row,0,1,returned_row_limit)),
            (Reverse, None) => self.db.query_row(
                "SELECT source,target FROM dependencies INDEXED BY dependencies_reverse_order ORDER BY target,source LIMIT 1", [],
                |row| bounded_text_pair(row,0,1,returned_row_limit)),
        }.optional().map_err(sql)?;
        let pair = row
            .map(|(source, target)| {
                Ok((
                    RelativePath::parse(&source)
                        .map_err(|_| invalid("native dependency source path is invalid"))?,
                    RelativePath::parse(&target)
                        .map_err(|_| invalid("native dependency target path is invalid"))?,
                ))
            })
            .transpose()?;
        self.candidate.tick()?;
        Ok(pair)
    }

    pub(crate) fn dependency_after(
        &self,
        source: &RelativePath,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<RelativePath>> {
        self.candidate.tick()?;
        path_text(source, self.row_limit)?;
        let argument_bytes = source
            .as_str()
            .len()
            .checked_add(after.map_or(0, |path| path.as_str().len()))
            .ok_or_else(|| invalid("native dependency cursor argument overflow"))?;
        let argument_state = cursor_argument_state(argument_bytes)?;
        self.candidate.check_state(self.row_limit)?;
        let returned_row_limit = self
            .row_limit
            .checked_sub(argument_state)
            .ok_or_else(|| invalid("native dependency cursor state exceeds profile"))?;
        let row = match after {
            Some(after) => self.db.query_row(
                "SELECT target FROM dependencies WHERE source=?1 AND target>?2 ORDER BY target LIMIT 1",
                params![source.as_str(), after.as_str()],
                |row| bounded_text(row, 0, returned_row_limit),
            ),
            None => self.db.query_row(
                "SELECT target FROM dependencies WHERE source=?1 ORDER BY target LIMIT 1",
                [source.as_str()],
                |row| bounded_text(row, 0, returned_row_limit),
            ),
        }
        .optional()
        .map_err(sql)?;
        let target = row
            .map(|target| {
                RelativePath::parse(&target)
                    .map_err(|_| invalid("native dependency target path is invalid"))
            })
            .transpose()?;
        self.candidate.tick()?;
        Ok(target)
    }
}
