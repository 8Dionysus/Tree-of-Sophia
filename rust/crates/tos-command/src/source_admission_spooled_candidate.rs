//! Private candidate membership/index workspace for the additive admission route.
//! A completed metadata import is not validation or permission to publish.
use super::source_admission::{AdmissionBatch, active, invalid};
use super::source_admission_candidate::CandidateLimits;
use super::source_admission_spooled_index::cursor_argument_state;
use super::source_admission_store::AdmissionStore;
#[path = "source_admission_logical_membership.rs"]
mod logical_membership;
use super::source_admission_v2_reader::{V2ReadSession, V2RootKind};
use rusqlite::{OptionalExtension, params};
use std::{
    cell::{Cell, RefCell},
    fs::File,
    io::{self, Write},
    mem::size_of,
    os::unix::fs::MetadataExt,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{
    MemberMetadata, PinnedSqliteAuxLimits, PinnedSqliteAuxRequest, PinnedSqliteAuxScope,
    PinnedSqliteConnection, PinnedSqliteIoBudget, PinnedSqliteSpaceBudget, ReadLimits,
    RetirementMetadata, SourceMembershipV1, StreamedCorpusCutReaderV1,
};

/// Separate finite workspace selection. These are caller resource limits, not
/// a new source validator, an admission receipt or an empirical size claim.
#[derive(Clone, Copy)]
pub(crate) struct SpoolLimits {
    pub candidate: CandidateLimits,
    pub max_row_state_bytes: usize,
    pub sqlite_cache_bytes: usize,
}

/// Actual candidate member bytes held with their exact metadata, never an
/// admitted revision or a completion grant. No constructor or owned raw escape.
pub(crate) struct CandidateMemberRead {
    raw: Vec<u8>,
    metadata: MemberMetadata,
}
impl CandidateMemberRead {
    pub(crate) fn metadata(&self) -> &MemberMetadata {
        &self.metadata
    }
    pub(crate) fn raw(&self) -> &[u8] {
        &self.raw
    }
}

impl SpoolLimits {
    /// Required BEFORE reading the existing resident bounded batch.
    pub(crate) fn bounded_batch_limits(
        self,
    ) -> io::Result<super::source_admission::AdmissionLimits> {
        let reserved = size_of::<SpoolCandidate<'_>>()
            .checked_add(self.sqlite_cache_bytes)
            .and_then(|n| n.checked_add(self.max_row_state_bytes))
            .ok_or_else(|| invalid("candidate spool baseline overflow"))?;
        let mut candidate = self.candidate;
        candidate.max_state_bytes = candidate
            .max_state_bytes
            .checked_sub(reserved)
            .ok_or_else(|| invalid("candidate spool baseline exceeds state"))?;
        candidate.bounded_batch_limits()
    }
    fn retained_batch_state(self, batch: &AdmissionBatch) -> io::Result<usize> {
        let mut retained = size_of::<SpoolCandidate<'_>>()
            .checked_add(self.sqlite_cache_bytes)
            .ok_or_else(|| invalid("candidate spool baseline overflow"))?;
        for (path, _) in &batch.updates {
            retained = retained
                .checked_add(path.capacity())
                .and_then(|n| {
                    n.checked_add(
                        16 * (size_of::<String>()
                            + size_of::<super::source_admission::SourceUpdate>()
                            + size_of::<usize>()),
                    )
                })
                .ok_or_else(|| invalid("candidate batch state overflow"))?;
        }
        for (path, row) in &batch.retirements {
            retained = retained
                .checked_add(path.capacity())
                .and_then(|n| n.checked_add(row.event_ref.as_str().len()))
                .and_then(|n| {
                    n.checked_add(
                        16 * (size_of::<String>()
                            + size_of::<super::source_admission::SourceRetirement>()
                            + size_of::<usize>()),
                    )
                })
                .ok_or_else(|| invalid("candidate batch state overflow"))?;
        }
        Ok(retained)
    }
    fn check_batch_state(self, batch: &AdmissionBatch) -> io::Result<()> {
        let retained = self.retained_batch_state(batch)?;
        if retained
            .checked_add(self.max_row_state_bytes)
            .is_none_or(|n| n > self.candidate.max_state_bytes)
        {
            return Err(invalid(
                "candidate retained batch and spool row state exceed profile",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CandidateFence {
    pub batch_sha256: Digest256,
    pub base_revision: Option<SourceRevision>,
    pub validator_sha256: Digest256,
    pub membership: SourceMembershipV1,
    pub source_bytes: u64,
    pub retirement_count: u64,
    pub retirement_digest: Digest256,
}
/// Mechanical publication receipt; semantic and rights authority remain separate.
pub(crate) struct SpooledPublicationReceipt {
    pub revision: SourceRevision,
    pub manifest_sha256: Option<Digest256>,
    pub source_artifact: Option<super::source_admission_segment_v2::SourceRevisionArtifactV2>,
    pub rootset_sha256: Option<Digest256>,
    pub manifest_bytes: Option<u64>,
    pub fence: CandidateFence,
    pub identities: u64,
    pub dependency_sources: u64,
    pub dependencies: u64,
    /// Hold through terminal or verified retained-baseline handoff. Rename does
    /// not free the surviving named manifest's physical allocation.
    pub persistent_manifest_custody: Arc<tos_source_store::PinnedSqliteSpaceReservation>,
}

/// A post-CAS refusal carries the exact candidate fence and publication
/// identity even when store durability or pointer rechecks fail afterward.
pub(crate) struct SpooledPublicationCommittedRefusal {
    pub(crate) revision: SourceRevision,
    pub(crate) manifest_sha256: Option<Digest256>,
    pub(crate) source_artifact:
        Option<super::source_admission_segment_v2::SourceRevisionArtifactV2>,
    pub(crate) rootset_sha256: Option<Digest256>,
    pub(crate) batch_sha256: Digest256,
    pub(crate) validator_sha256: Digest256,
    pub(crate) persistent_manifest_custody:
        Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    cause: io::Error,
}

impl std::fmt::Debug for SpooledPublicationCommittedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpooledPublicationCommittedRefusal")
            .field("revision", &self.revision.0.to_hex())
            .field(
                "manifest_sha256",
                &self.manifest_sha256.map(|digest| digest.to_hex()),
            )
            .field("source_artifact", &self.source_artifact)
            .field(
                "rootset_sha256",
                &self.rootset_sha256.map(|digest| digest.to_hex()),
            )
            .field("batch_sha256", &self.batch_sha256.to_hex())
            .field("validator_sha256", &self.validator_sha256.to_hex())
            .field(
                "custody_retained",
                &self.persistent_manifest_custody.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for SpooledPublicationCommittedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("spooled corpus revision committed before a post-rename refusal")
    }
}

impl std::error::Error for SpooledPublicationCommittedRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// A V2 writer left immutable physical allocations before the selector CAS.
/// The retained custody is returned with the refusal for explicit recovery.
pub(crate) struct SpooledPublicationV2Refusal {
    pub(crate) revision: SourceRevision,
    pub(crate) manifest_sha256: Option<Digest256>,
    pub(crate) source_artifact:
        Option<super::source_admission_segment_v2::SourceRevisionArtifactV2>,
    pub(crate) rootset_sha256: Option<Digest256>,
    pub(crate) batch_sha256: Digest256,
    pub(crate) validator_sha256: Digest256,
    pub(crate) persistent_store_custody:
        Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    cause: io::Error,
}

impl std::fmt::Debug for SpooledPublicationV2Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpooledPublicationV2Refusal")
            .field("revision", &self.revision.0.to_hex())
            .field(
                "manifest_sha256",
                &self.manifest_sha256.map(|digest| digest.to_hex()),
            )
            .field("source_artifact", &self.source_artifact)
            .field(
                "rootset_sha256",
                &self.rootset_sha256.map(|digest| digest.to_hex()),
            )
            .field("batch_sha256", &self.batch_sha256.to_hex())
            .field("validator_sha256", &self.validator_sha256.to_hex())
            .field(
                "store_custody_retained",
                &self.persistent_store_custody.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for SpooledPublicationV2Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("V2 publication refused before a verified selector commit")
    }
}

impl std::error::Error for SpooledPublicationV2Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Empty strict-main file in the already captured workspace. The budgeted
/// reader owns the actual main-file reservation; this factory creates neither
/// a second stage nor a second ledger and does not pre-reserve it twice.
pub(crate) struct StreamedCutWorkspace {
    pub main: File,
    pub io_budget: PinnedSqliteIoBudget,
    pub space_budget: PinnedSqliteSpaceBudget,
    pub deadline: Instant,
    pub cancelled: Arc<AtomicBool>,
}
/// Field order closes the manifest FD before releasing its shared reservation.
pub(crate) struct ManifestWorkspace {
    pub main: File,
    pub reservation: tos_source_store::PinnedSqliteSpaceReservation,
}

pub(crate) struct SpoolCandidate<'host> {
    store: &'host AdmissionStore,
    workspace: File,
    space_budget: PinnedSqliteSpaceBudget,
    base: Option<&'host StreamedCorpusCutReaderV1>,
    base_v2: Option<&'host RefCell<V2ReadSession>>,
    v2_io: Option<PinnedSqliteIoBudget>,
    batch: AdmissionBatch,
    db: PinnedSqliteConnection,
    _scope: PinnedSqliteAuxScope,
    ledger: PinnedSqliteIoBudget,
    limits: SpoolLimits,
    membership: SourceMembershipV1,
    source_bytes: u64,
    logical_reads: Cell<u64>,
    logical_writes: Cell<u64>,
    logical_read_ceiling: Cell<u64>,
    logical_write_ceiling: Cell<u64>,
    row_state_ceiling: Cell<usize>,
    retirement_count: u64,
    new_retirement_start: u64,
    failed: Cell<bool>,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
fn sql(error: rusqlite::Error) -> io::Error {
    invalid(error)
}
fn member_from_row(
    row: &rusqlite::Row<'_>,
    cap: usize,
) -> rusqlite::Result<(String, Vec<u8>, Vec<u8>, u32)> {
    let name = row.get_ref(0)?;
    let path_len = match name {
        rusqlite::types::ValueRef::Text(raw) => raw.len(),
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    if path_len
        .checked_mul(16)
        .and_then(|n| n.checked_add(1024))
        .is_none_or(|n| n > cap)
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    match (row.get_ref(1)?, row.get_ref(2)?) {
        (rusqlite::types::ValueRef::Blob(sha), rusqlite::types::ValueRef::Blob(size))
            if sha.len() == 32 && size.len() == 8 =>
        {
            ()
        }
        _ => return Err(rusqlite::Error::InvalidQuery),
    }
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}
fn bounded_text(row: &rusqlite::Row<'_>, cap: usize) -> rusqlite::Result<String> {
    match row.get_ref(0)? {
        rusqlite::types::ValueRef::Text(raw)
            if raw
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(2048))
                .is_some_and(|n| n <= cap) =>
        {
            row.get(0)
        }
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn member(row: (String, Vec<u8>, Vec<u8>, u32)) -> io::Result<MemberMetadata> {
    let (path, sha, size, mode) = row;
    let sha: [u8; 32] = sha
        .try_into()
        .map_err(|_| invalid("candidate spool digest width"))?;
    let size: [u8; 8] = size
        .try_into()
        .map_err(|_| invalid("candidate spool length width"))?;
    if !matches!(mode, 0o600 | 0o644 | 0o755) {
        return Err(invalid("candidate spool source mode"));
    }
    Ok(MemberMetadata {
        path: RelativePath::parse(&path).map_err(invalid)?,
        sha256: Digest256::from_bytes(sha),
        size_bytes: u64::from_be_bytes(size),
        mode,
    })
}
impl<'host> SpoolCandidate<'host> {
    /// Reads the exact protected canonical batch under the SAME ledger before
    /// opening any candidate/index scope. No externally parsed batch can enter
    /// the additive path through this constructor.
    pub(crate) fn prepare_from_selected_batch(
        store: &'host AdmissionStore,
        batch_path: &std::path::Path,
        input_root: &std::path::Path,
        selected_validator: Digest256,
        base: Option<&'host StreamedCorpusCutReaderV1>,
        workspace: File,
        request: PinnedSqliteAuxRequest,
        limits: SpoolLimits,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        if request.deadline != deadline || !Arc::ptr_eq(&request.cancelled, &cancelled) {
            return Err(invalid("candidate batch clock or cancellation differs"));
        }
        let batch_limits = limits.bounded_batch_limits()?;
        let batch = AdmissionBatch::read_budgeted(
            batch_path,
            input_root,
            batch_limits,
            deadline,
            &cancelled,
            &request.io_budget,
        )?;
        Self::prepare_selected_batch(
            store,
            batch,
            selected_validator,
            base,
            workspace,
            request,
            limits,
            deadline,
            cancelled,
        )
    }
    /// Continue from the same bounded canonical parser when the caller must
    /// check validator identity before creating the persistent store. The held
    /// input descriptor and private batch fields prevent arbitrary member
    /// iterators or caller-authored `AdmissionBatch` values from entering.
    pub(crate) fn prepare_selected_batch(
        store: &'host AdmissionStore,
        batch: AdmissionBatch,
        selected_validator: Digest256,
        base: Option<&'host StreamedCorpusCutReaderV1>,
        workspace: File,
        request: PinnedSqliteAuxRequest,
        limits: SpoolLimits,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        Self::prepare_selected_batch_with_v2_base(
            store,
            batch,
            selected_validator,
            base,
            None,
            None,
            workspace,
            request,
            limits,
            deadline,
            cancelled,
        )
    }

    /// Additive authenticated V2 base route. A V2 reader and its exact pointer
    /// IO ledger are borrowed from the protected invocation; the candidate
    /// builds its existing bounded SQL base indexes from held authenticated
    /// rows before native completion is minted.
    pub(crate) fn prepare_selected_batch_with_v2_base(
        store: &'host AdmissionStore,
        batch: AdmissionBatch,
        selected_validator: Digest256,
        base: Option<&'host StreamedCorpusCutReaderV1>,
        base_v2: Option<&'host RefCell<V2ReadSession>>,
        v2_io: Option<&PinnedSqliteIoBudget>,
        workspace: File,
        request: PinnedSqliteAuxRequest,
        limits: SpoolLimits,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        if !batch.shares_spooled_io_budget(&request.io_budget) {
            return Err(invalid(
                "candidate batch did not use the original IO budget",
            ));
        }
        Self::prepare(
            store,
            batch,
            selected_validator,
            base,
            base_v2,
            v2_io,
            workspace,
            request,
            limits,
            deadline,
            cancelled,
        )
    }
    /// The batch is still the existing bounded canonical batch. A later paged
    /// batch producer must retain that native parser/identity/held-input law;
    /// this method does not reinterpret arbitrary supplied member iterators.
    fn prepare(
        store: &'host AdmissionStore,
        batch: AdmissionBatch,
        selected_validator: Digest256,
        base: Option<&'host StreamedCorpusCutReaderV1>,
        base_v2: Option<&'host RefCell<V2ReadSession>>,
        v2_io: Option<&PinnedSqliteIoBudget>,
        workspace: File,
        request: PinnedSqliteAuxRequest,
        limits: SpoolLimits,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        limits.candidate.validate()?;
        limits.check_batch_state(&batch)?;
        active(deadline, &cancelled)?;
        let v2_selected_base = base_v2.map(|reader| reader.borrow().selected_revision());
        let selected_base = base
            .map(|reader| reader.current_revision())
            .or(v2_selected_base);
        if batch.validator_sha256 != selected_validator
            || (base.is_some() && base_v2.is_some())
            || selected_base.map(|revision| revision.0) != batch.base_revision
            || (base_v2.is_some() && v2_io.is_none())
            || base_v2
                .is_some_and(|reader| v2_io.is_none_or(|io| !reader.borrow().shares_io_budget(io)))
            || request.deadline != deadline
            || !Arc::ptr_eq(&request.cancelled, &cancelled)
            || base.is_some_and(|reader| {
                !reader.shares_budgeted_request(
                    &request.io_budget,
                    &request.space_budget,
                    deadline,
                    &cancelled,
                )
            })
            || limits.max_row_state_bytes == 0
            || limits.max_row_state_bytes > limits.candidate.max_state_bytes
            || limits.sqlite_cache_bytes == 0
            || limits.sqlite_cache_bytes > limits.max_row_state_bytes
        {
            return Err(invalid(
                "spooled candidate selected profile or validator differs",
            ));
        }
        if let Some(reader) = base_v2 {
            reader.borrow_mut().verify_current_fence()?;
        }
        store.check_current_budgeted(
            batch.base_revision,
            limits.candidate.reader,
            deadline,
            &cancelled,
            v2_io.unwrap_or(&request.io_budget),
        )?;
        let ledger = request.io_budget.clone();
        let space_budget = request.space_budget.clone();
        let held_workspace = workspace.try_clone()?;
        // The shared pager ledger also covers object reads/writes below. Its
        // finite maximum cannot be silently reset for an individual phase.
        let mut scope = PinnedSqliteAuxScope::new(workspace, request).map_err(invalid)?;
        let db = scope.open_connection().map_err(invalid)?;
        db.pragma_update(
            None,
            "cache_size",
            -(i64::try_from(limits.sqlite_cache_bytes.div_ceil(1024)).map_err(invalid)?),
        )
        .map_err(sql)?;
        let clock_cancel = cancelled.clone();
        db.progress_handler(1000, Some(move || active(deadline, &clock_cancel).is_err()));
        // The private preparation is one finite SQLite unit. Per-row
        // autocommit repeatedly flushes the same pages and can exhaust the
        // original shared write budget before immutable-object ingestion.
        // This transaction keeps the same pager/space/deadline accounting;
        // no prepared candidate is returned until its metered commit succeeds.
        db.execute_batch("BEGIN IMMEDIATE").map_err(sql)?;
        db.execute_batch("CREATE TABLE members(path TEXT COLLATE BINARY PRIMARY KEY,sha BLOB NOT NULL CHECK(length(sha)=32),size BLOB NOT NULL CHECK(length(size)=8),mode INTEGER NOT NULL,changed INTEGER NOT NULL,touched INTEGER NOT NULL) WITHOUT ROWID; CREATE INDEX members_changed_path ON members(path COLLATE BINARY) WHERE changed=1; CREATE TABLE retirements(ordinal INTEGER PRIMARY KEY,path TEXT,sha BLOB,event_ref TEXT,event_sha BLOB,event_size BLOB); CREATE INDEX retirements_by_path ON retirements(path COLLATE BINARY,ordinal); CREATE TABLE affected(path TEXT COLLATE BINARY PRIMARY KEY,visited INTEGER NOT NULL) WITHOUT ROWID; CREATE TABLE reverse_dependencies(target TEXT COLLATE BINARY,source TEXT COLLATE BINARY,PRIMARY KEY(target,source)) WITHOUT ROWID; CREATE INDEX reverse_dependencies_by_source ON reverse_dependencies(source COLLATE BINARY,target COLLATE BINARY); CREATE TABLE historical_ids(id TEXT COLLATE BINARY PRIMARY KEY,path TEXT NOT NULL) WITHOUT ROWID; CREATE INDEX historical_ids_by_path ON historical_ids(path COLLATE BINARY,id COLLATE BINARY); CREATE INDEX affected_queue ON affected(visited,path); CREATE TABLE v1_migration_history(revision BLOB PRIMARY KEY CHECK(length(revision)=32),raw BLOB NOT NULL CHECK(length(raw)<=65536)) WITHOUT ROWID;").map_err(sql)?;
        let batch_bytes = batch.bytes_read();
        let mut candidate = Self {
            store,
            workspace: held_workspace,
            space_budget,
            base,
            base_v2,
            v2_io: v2_io.cloned(),
            batch,
            db,
            _scope: scope,
            ledger,
            limits,
            membership: SourceMembershipV1 {
                count: 0,
                digest: Digest256::of_bytes(b""),
            },
            source_bytes: 0,
            logical_reads: Cell::new(batch_bytes),
            logical_writes: Cell::new(0),
            logical_read_ceiling: Cell::new(limits.candidate.max_read_bytes),
            logical_write_ceiling: Cell::new(limits.candidate.max_write_bytes),
            row_state_ceiling: Cell::new(limits.max_row_state_bytes),
            retirement_count: 0,
            new_retirement_start: 0,
            failed: Cell::new(true),
            deadline,
            cancelled,
        };
        candidate.import_and_prepare()?;
        candidate.running()?;
        candidate.db.execute_batch("COMMIT").map_err(sql)?;
        candidate.failed.set(false);
        candidate.tick()?;
        Ok(candidate)
    }
    fn running(&self) -> io::Result<()> {
        if let Err(error) = active(self.deadline, &self.cancelled) {
            self.failed.set(true);
            return Err(error);
        }
        if self.ledger.snapshot().failure.is_some() {
            self.failed.set(true);
            Err(invalid("candidate spool prior I/O failure"))
        } else {
            Ok(())
        }
    }
    pub(crate) fn tick(&self) -> io::Result<()> {
        if self.failed.get() {
            return Err(invalid("candidate spool is unusable"));
        }
        self.running()
    }
    fn finish_read<T>(&self, result: io::Result<T>) -> io::Result<T> {
        match result {
            Ok(value) => {
                self.running()?;
                Ok(value)
            }
            Err(error) => {
                self.failed.set(true);
                Err(error)
            }
        }
    }
    /// Actual shared raw-object and SQLite physical counters. These bytes are
    /// already charged; callers debit only their phase delta, never this snapshot again.
    pub(crate) fn io_usage(&self) -> io::Result<tos_source_store::PinnedSqliteIoSnapshot> {
        self.tick()?;
        let snapshot = self.ledger.snapshot();
        self.finish_read(Ok(snapshot))
    }
    /// Original ledger identity only; no currency, budget or admission grant.
    pub(crate) fn shares_io_budget(&self, budget: &PinnedSqliteIoBudget) -> bool {
        self.ledger.shares_with(budget)
    }
    /// Verify the actual prepared holder's workspace and original request.
    /// This comparison is not an admission or publication grant.
    pub(crate) fn verify_prepared_request(
        &self,
        workspace: &File,
        io: &PinnedSqliteIoBudget,
        space: &PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
    ) -> io::Result<()> {
        self.tick()?;
        let result = (|| {
            if !self.ledger.shares_with(io)
                || !self.space_budget.shares_with(space)
                || self.deadline != deadline
                || !Arc::ptr_eq(&self.cancelled, cancelled)
            {
                return Err(invalid("candidate prepared request differs"));
            }
            let health = self.space_budget.snapshot();
            if !health.ledger_consistent || health.allocation_anomalies != 0 {
                return Err(invalid("candidate prepared space is unusable"));
            }
            self.store.verify_layout()?;
            let identity = |file: &File| -> io::Result<(u64, u64, u32, u32)> {
                let meta = file.metadata()?;
                if !meta.is_dir()
                    || meta.uid() != unsafe { libc::geteuid() }
                    || meta.mode() & 0o7777 != 0o700
                {
                    return Err(invalid("candidate prepared workspace custody differs"));
                }
                Ok((meta.dev(), meta.ino(), meta.uid(), meta.mode()))
            };
            let own = identity(&self.workspace)?;
            if identity(workspace)? != own {
                return Err(invalid("candidate prepared workspace differs"));
            }
            self.store.verify_layout()?;
            if identity(&self.workspace)? != own || identity(workspace)? != own {
                return Err(invalid("candidate prepared workspace changed"));
            }
            let health = self.space_budget.snapshot();
            if !health.ledger_consistent || health.allocation_anomalies != 0 {
                return Err(invalid("candidate prepared space is unusable"));
            }
            Ok(())
        })();
        self.finish_read(result)
    }
    /// Declared borrowed base Rust/custody and nominal cache, separately.
    /// The reader owner excludes opaque native heap, page pins and process RSS.
    pub(crate) fn borrowed_base_declared_retained_state_bytes(&self) -> io::Result<(usize, usize)> {
        self.tick()?;
        let result = (|| match (self.base, self.base_v2) {
            (None, None) => Ok((0, 0)),
            (Some(reader), None) => {
                if !reader.shares_budgeted_request(
                    &self.ledger,
                    &self.space_budget,
                    self.deadline,
                    &self.cancelled,
                ) {
                    return Err(invalid("borrowed base original request differs"));
                }
                reader.declared_retained_state_bytes().map_err(invalid)
            }
            (None, Some(reader)) => {
                let io = self
                    .v2_io
                    .as_ref()
                    .ok_or_else(|| invalid("V2 base lacks its original IO ledger"))?;
                let reader = reader.borrow();
                if !reader.shares_io_budget(io) {
                    return Err(invalid("V2 base original IO ledger differs"));
                }
                reader.declared_retained_state_bytes().map_err(invalid)
            }
            (Some(_), Some(_)) => Err(invalid("candidate has multiple selected base readers")),
        })();
        self.finish_read(result)
    }
    /// Declared owned candidate/batch/cache state only. The borrowed base reader,
    /// SQLite allocator/page pins and process RSS require their own owner accounting.
    pub(crate) fn own_retained_state_upper_bound_bytes(&self) -> io::Result<usize> {
        self.tick()?;
        self.finish_read(self.limits.retained_batch_state(&self.batch))
    }
    /// Narrow cumulative logical ceilings without resetting usage or shared physical IO.
    pub(crate) fn restrict_remaining_io(
        &self,
        remaining_read: u64,
        remaining_write: u64,
    ) -> io::Result<()> {
        self.tick()?;
        let result = (|| {
            let reads = self
                .logical_reads
                .get()
                .checked_add(remaining_read)
                .ok_or_else(|| invalid("candidate remaining read ceiling overflow"))?;
            let writes = self
                .logical_writes
                .get()
                .checked_add(remaining_write)
                .ok_or_else(|| invalid("candidate remaining write ceiling overflow"))?;
            // External Foundation phases may consume the outer allowance.
            // Narrow the SAME physical ledger before final EOF, without reset.
            self.ledger
                .restrict_remaining_io(remaining_read, remaining_write)
                .map_err(invalid)?;
            self.logical_read_ceiling
                .set(self.logical_read_ceiling.get().min(reads));
            self.logical_write_ceiling
                .set(self.logical_write_ceiling.get().min(writes));
            Ok(())
        })();
        self.finish_read(result)
    }
    /// Total owned state includes the retained baseline. Only its checked
    /// remainder is available for transient rows; an earlier narrowing cannot grow.
    pub(crate) fn restrict_remaining_state(&self, max_owned_state_bytes: usize) -> io::Result<()> {
        self.tick()?;
        let result = (|| {
            let retained = self.limits.retained_batch_state(&self.batch)?;
            let transient = max_owned_state_bytes
                .checked_sub(retained)
                .filter(|n| *n != 0)
                .ok_or_else(|| invalid("candidate remaining state below retained baseline"))?;
            self.row_state_ceiling
                .set(self.row_state_ceiling.get().min(transient));
            Ok(())
        })();
        self.finish_read(result)
    }
    pub(crate) fn check_state(&self, n: usize) -> io::Result<()> {
        self.running()?;
        if n > self.row_state_ceiling.get() {
            self.failed.set(true);
            Err(invalid("candidate spool per-row state exceeded"))
        } else {
            Ok(())
        }
    }
    pub(crate) fn debit_read(&self, n: u64) -> io::Result<()> {
        self.running()?;
        self.ledger.charge_read(n).map_err(invalid)
    }
    pub(crate) fn debit_write(&self, n: u64) -> io::Result<()> {
        self.running()?;
        self.ledger.charge_write(n).map_err(invalid)
    }
    pub(crate) fn record_write_returned(&self, n: u64) -> io::Result<()> {
        self.running()?;
        self.finish_read(self.ledger.record_write_returned(n).map_err(invalid))
    }
    /// Account actual held-destination bytes in a bootstrap's precharged
    /// original IO reservation; no second ledger or allowance is created.
    pub(crate) fn record_bootstrap_read_returned(&self, n: u64) -> io::Result<()> {
        self.running()?;
        self.finish_read(self.ledger.record_read_returned(n).map_err(invalid))
    }
    /// A failed consuming serializer/index operation cannot reuse the candidate.
    pub(crate) fn abandon(&self) {
        self.failed.set(true);
    }
    fn reserve_logical(&self, reads: u64, writes: u64) -> io::Result<()> {
        self.running()?;
        let r = self
            .logical_reads
            .get()
            .checked_add(reads)
            .filter(|n| *n <= self.logical_read_ceiling.get())
            .ok_or_else(|| {
                self.failed.set(true);
                invalid("candidate logical read bound")
            })?;
        let w = self
            .logical_writes
            .get()
            .checked_add(writes)
            .filter(|n| *n <= self.logical_write_ceiling.get())
            .ok_or_else(|| {
                self.failed.set(true);
                invalid("candidate logical write bound")
            })?;
        self.logical_reads.set(r);
        self.logical_writes.set(w);
        Ok(())
    }
    /// FND obtains its distinct index main from the SAME captured workspace,
    /// shared physical and pager I/O ledgers, cancellation and clock. This does
    /// not select another stage or create another resource/admission grant.
    pub(crate) fn open_index_scope(
        &self,
        limits: PinnedSqliteAuxLimits,
    ) -> io::Result<PinnedSqliteAuxScope> {
        self.tick()?;
        let request = PinnedSqliteAuxRequest {
            limits,
            io_budget: self.ledger.clone(),
            space_budget: self.space_budget.clone(),
            deadline: self.deadline,
            cancelled: self.cancelled.clone(),
        };
        let result =
            (|| PinnedSqliteAuxScope::new(self.workspace.try_clone()?, request).map_err(invalid))();
        self.finish_read(result)
    }
    pub(crate) fn io_snapshot(&self) -> tos_source_store::PinnedSqliteIoSnapshot {
        self.ledger.snapshot()
    }
    pub(crate) fn create_streamed_cut_workspace_file(&self) -> io::Result<StreamedCutWorkspace> {
        self.tick()?;
        let result = (|| {
            let main = File::from(rustix::fs::openat(
                &self.workspace,
                ".",
                rustix::fs::OFlags::TMPFILE
                    | rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::from_raw_mode(0o600),
            )?);
            Ok(StreamedCutWorkspace {
                main,
                io_budget: self.ledger.clone(),
                space_budget: self.space_budget.clone(),
                deadline: self.deadline,
                cancelled: self.cancelled.clone(),
            })
        })();
        self.finish_read(result)
    }
    pub(crate) fn create_manifest_workspace_file(
        &self,
        max_allocated_bytes: u64,
    ) -> io::Result<ManifestWorkspace> {
        self.tick()?;
        let result = (|| {
            let reservation = self
                .space_budget
                .reserve(max_allocated_bytes)
                .map_err(invalid)?;
            let main = File::from(rustix::fs::openat(
                &self.workspace,
                ".",
                rustix::fs::OFlags::TMPFILE
                    | rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::from_raw_mode(0o600),
            )?);
            Ok(ManifestWorkspace { main, reservation })
        })();
        self.finish_read(result)
    }
    /// Consuming publication after the actual native validator has returned its
    /// opaque index. Success and failure both retire this candidate invocation.
    pub(crate) fn publish_validated(
        &self,
        index: &super::source_admission_spooled_index::IndexView<'_>,
        manifest_limits: super::source_admission_spooled_manifest::ManifestStreamLimits,
        max_manifest_allocated_bytes: u64,
        streamed: super::source_admission_store::StreamedPublicationRead,
    ) -> io::Result<SpooledPublicationReceipt> {
        self.tick()?;
        let result = (|| {
            if !self.ledger.shares_with(&streamed.io_budget)
                || !self.space_budget.shares_with(&streamed.space_budget)
                || !Arc::ptr_eq(&self.cancelled, &streamed.cancelled)
                || manifest_limits.max_manifest_bytes
                    > self.limits.candidate.reader.max_manifest_bytes as u64
                || max_manifest_allocated_bytes < manifest_limits.max_manifest_bytes
                || max_manifest_allocated_bytes == u64::MAX
                || streamed.max_manifest_allocated_bytes < manifest_limits.max_manifest_bytes
            {
                return Err(invalid("publication resource/profile association differs"));
            }
            index.verify_candidate()?;
            let fence = self.fence()?;
            if fence != index.fence() {
                return Err(invalid("publication validated index fence differs"));
            }
            self.verify_consumed()?;
            if index.segment_v2_budget().is_some() && self.batch.base_revision.is_some() {
                // A selected V2 base publishes only the authenticated COW
                // successor and its compact record. Do not serialize another
                // whole V1 snapshot or relabel the compact record as one.
                let build = if self.base_v2.is_some() {
                    super::source_admission_segment_v2::build_successor_rootset_v2(
                        self.store,
                        self,
                        index,
                        self.deadline,
                        &self.cancelled,
                    )
                } else {
                    super::source_admission_segment_v2::build_v1_migration_rootset_v2(
                        self.store,
                        self,
                        index,
                        self.deadline,
                        &self.cancelled,
                    )
                };
                let built = match build {
                    Ok(built) => built,
                    Err(error) => {
                        return Err(io::Error::new(
                            error.kind(),
                            SpooledPublicationV2Refusal {
                                // No successor identity exists until all four
                                // current roots and the compact record are
                                // complete. This field names the selected base;
                                // the absent artifact/rootset makes that clear.
                                revision: SourceRevision(
                                    self.batch
                                        .base_revision
                                        .ok_or_else(|| invalid("V2 successor base is absent"))?,
                                ),
                                manifest_sha256: None,
                                source_artifact: None,
                                rootset_sha256: None,
                                batch_sha256: fence.batch_sha256,
                                validator_sha256: fence.validator_sha256,
                                persistent_store_custody: self.store.v2_store_custody(),
                                cause: error,
                            },
                        ));
                    }
                };
                let revision = built.roots.current.revision;
                let source_artifact = built.roots.current.source_artifact.clone();
                let rootset_sha256 = built.sha256;
                let custody = built.tree_io.custody_reservation();
                let lock = match self
                    .store
                    .lock_for_v2_publication(self.deadline, &self.cancelled)
                {
                    Ok(lock) => lock,
                    Err(error) => {
                        return Err(io::Error::new(
                            error.kind(),
                            SpooledPublicationV2Refusal {
                                revision,
                                manifest_sha256: None,
                                source_artifact: Some(source_artifact),
                                rootset_sha256: Some(rootset_sha256),
                                batch_sha256: fence.batch_sha256,
                                validator_sha256: fence.validator_sha256,
                                persistent_store_custody: Some(custody),
                                cause: error,
                            },
                        ));
                    }
                };
                if let Err(error) = self.store.publish_v2_successor(
                    built,
                    self.limits.candidate.reader,
                    &lock,
                    self.deadline,
                    &self.cancelled,
                ) {
                    let committed = error
                        .get_ref()
                        .and_then(|source| {
                            source.downcast_ref::<
                                super::source_admission_store::V2SuccessorPublicationCommittedRefusal,
                            >()
                        })
                        .map(|refusal| {
                            (
                                refusal.revision,
                                refusal.source_artifact.clone(),
                                refusal.rootset_sha256,
                                Arc::clone(&refusal.custody),
                            )
                        });
                    if let Some((
                        committed_revision,
                        committed_artifact,
                        committed_rootset,
                        custody,
                    )) = committed
                    {
                        return Err(io::Error::new(
                            error.kind(),
                            SpooledPublicationCommittedRefusal {
                                revision: committed_revision,
                                manifest_sha256: None,
                                source_artifact: Some(committed_artifact),
                                rootset_sha256: Some(committed_rootset),
                                batch_sha256: fence.batch_sha256,
                                validator_sha256: fence.validator_sha256,
                                persistent_manifest_custody: Some(custody),
                                cause: error,
                            },
                        ));
                    }
                    return Err(io::Error::new(
                        error.kind(),
                        SpooledPublicationV2Refusal {
                            revision,
                            manifest_sha256: None,
                            source_artifact: Some(source_artifact),
                            rootset_sha256: Some(rootset_sha256),
                            batch_sha256: fence.batch_sha256,
                            validator_sha256: fence.validator_sha256,
                            persistent_store_custody: Some(custody),
                            cause: error,
                        },
                    ));
                }
                return Ok(SpooledPublicationReceipt {
                    revision,
                    manifest_sha256: None,
                    source_artifact: Some(source_artifact),
                    rootset_sha256: Some(rootset_sha256),
                    manifest_bytes: None,
                    fence,
                    identities: index.identity_count(),
                    dependency_sources: index.dependency_source_count(),
                    dependencies: index.dependency_count(),
                    persistent_manifest_custody: custody,
                });
            }
            let (_, revision) = super::source_admission_spooled_manifest::serialize(
                self,
                index,
                None,
                None,
                manifest_limits,
            )?;
            let mut output = self.create_manifest_workspace_file(max_manifest_allocated_bytes)?;
            let (manifest_bytes, manifest_sha256) =
                super::source_admission_spooled_manifest::serialize(
                    self,
                    index,
                    Some(revision),
                    Some(super::source_admission_spooled_manifest::ManifestSink {
                        file: &mut output.main,
                        reservation: &output.reservation,
                    }),
                    manifest_limits,
                )?;
            self.tick()?;
            output.main.sync_all()?;
            self.tick()?;
            index.verify_candidate()?;
            self.verify_consumed()?;
            if self.fence()? != fence {
                return Err(invalid("publication candidate changed after serialization"));
            }
            let mut rootset_sha256 = None;
            let mut v2_custody = None;
            let mut source_artifact = None;
            let publication_result = if let Some(profile) = index.segment_v2_budget() {
                if self.batch.base_revision.is_some() {
                    return Err(invalid(
                        "V2 initial writer refuses a nonempty admission base",
                    ));
                }
                source_artifact = Some(
                    super::source_admission_segment_v2::SourceRevisionArtifactV2::SnapshotV1 {
                        sha256: manifest_sha256,
                        bytes: manifest_bytes,
                    },
                );
                let lock = self
                    .store
                    .lock_for_v2_publication(self.deadline, &self.cancelled)?;
                if self
                    .store
                    .current_selection(
                        self.limits.candidate.reader,
                        self.deadline,
                        &self.cancelled,
                        Some(&profile.io),
                    )?
                    .is_some()
                {
                    return Err(invalid(
                        "V2 initial writer requires no selected corpus revision",
                    ));
                }
                let built = match super::source_admission_segment_v2::build_initial_rootset_v2(
                    self.store,
                    self,
                    index,
                    SourceRevision(revision),
                    manifest_sha256,
                    manifest_bytes,
                    fence.batch_sha256,
                    self.deadline,
                    &self.cancelled,
                ) {
                    Ok(built) => built,
                    Err(error) => {
                        return Err(io::Error::new(
                            error.kind(),
                            SpooledPublicationV2Refusal {
                                revision: SourceRevision(revision),
                                manifest_sha256: Some(manifest_sha256),
                                source_artifact: source_artifact.clone(),
                                rootset_sha256: None,
                                batch_sha256: fence.batch_sha256,
                                validator_sha256: fence.validator_sha256,
                                persistent_store_custody: self.store.v2_store_custody(),
                                cause: error,
                            },
                        ));
                    }
                };
                rootset_sha256 = Some(built.sha256);
                v2_custody = Some(built.tree_io.custody_reservation());
                self.store.publish_streamed_v2_initial(
                    self.batch.base_revision,
                    revision,
                    output.main,
                    manifest_bytes,
                    manifest_sha256,
                    self.limits.candidate.reader,
                    streamed,
                    built,
                    &lock,
                    self.deadline,
                    &self.cancelled,
                )
            } else {
                self.store.publish_streamed(
                    self.batch.base_revision,
                    revision,
                    output.main,
                    manifest_bytes,
                    manifest_sha256,
                    self.limits.candidate.reader,
                    streamed,
                    self.deadline,
                    &self.cancelled,
                )
            };
            if let Err(error) = publication_result {
                let committed = error
                    .get_ref()
                    .and_then(|source| {
                        source.downcast_ref::<
                            super::source_admission_store::StreamedPublicationCommittedRefusal,
                        >()
                    })
                    .map(|refusal| {
                        (
                            refusal.revision,
                            refusal.manifest_sha256,
                            refusal.rootset_sha256,
                            refusal.custody.clone(),
                        )
                    });
                if let Some((committed_revision, committed_manifest, committed_rootset, custody)) =
                    committed
                {
                    return Err(io::Error::new(
                        error.kind(),
                        SpooledPublicationCommittedRefusal {
                            revision: SourceRevision(committed_revision),
                            manifest_sha256: Some(committed_manifest),
                            source_artifact: source_artifact.clone(),
                            rootset_sha256: committed_rootset,
                            batch_sha256: fence.batch_sha256,
                            validator_sha256: fence.validator_sha256,
                            persistent_manifest_custody: custody,
                            cause: error,
                        },
                    ));
                }
                if index.segment_v2_budget().is_some() {
                    return Err(io::Error::new(
                        error.kind(),
                        SpooledPublicationV2Refusal {
                            revision: SourceRevision(revision),
                            manifest_sha256: Some(manifest_sha256),
                            source_artifact: source_artifact.clone(),
                            rootset_sha256,
                            batch_sha256: fence.batch_sha256,
                            validator_sha256: fence.validator_sha256,
                            persistent_store_custody: v2_custody
                                .or_else(|| self.store.v2_store_custody()),
                            cause: error,
                        },
                    ));
                }
                return Err(error);
            }
            // output.reservation survives the consumed FD call above and drops
            // only after its temporary backing has closed.
            let custody = match if index.segment_v2_budget().is_some() {
                v2_custody
                    .take()
                    .or_else(|| self.store.v2_store_custody())
                    .ok_or_else(|| invalid("V2 store custody absent after publication"))
            } else {
                self.store.streamed_manifest_custody()
            } {
                Ok(custody) => custody,
                Err(error) => {
                    return Err(io::Error::new(
                        error.kind(),
                        SpooledPublicationCommittedRefusal {
                            revision: SourceRevision(revision),
                            manifest_sha256: Some(manifest_sha256),
                            source_artifact: source_artifact.clone(),
                            rootset_sha256,
                            batch_sha256: fence.batch_sha256,
                            validator_sha256: fence.validator_sha256,
                            persistent_manifest_custody: v2_custody
                                .clone()
                                .or_else(|| self.store.v2_store_custody()),
                            cause: error,
                        },
                    ));
                }
            };
            let receipt = SpooledPublicationReceipt {
                revision: SourceRevision(revision),
                manifest_sha256: Some(manifest_sha256),
                source_artifact,
                rootset_sha256,
                manifest_bytes: Some(manifest_bytes),
                fence,
                identities: index.identity_count(),
                dependency_sources: index.dependency_source_count(),
                dependencies: index.dependency_count(),
                persistent_manifest_custody: custody,
            };
            Ok(receipt)
        })();
        self.abandon();
        result
    }
    /// Creation starts from the unchanged admitted base, never a final future
    /// authored batch whose declared members already occupy the requested homes.
    pub(crate) fn verify_creation_base(&self) -> io::Result<CandidateFence> {
        self.tick()?;
        if self.batch.base_revision.is_none()
            || !self.batch.updates.is_empty()
            || !self.batch.retirements.is_empty()
        {
            return Err(invalid(
                "creation staging requires an unchanged selected base",
            ));
        }
        self.fence()
    }
    pub(crate) fn base_revision(&self) -> Option<SourceRevision> {
        self.batch.base_revision.map(SourceRevision)
    }
    pub(crate) fn v1_base_reader(&self) -> io::Result<&StreamedCorpusCutReaderV1> {
        self.tick()?;
        let reader = self
            .base
            .ok_or_else(|| invalid("V1 migration base reader absent"))?;
        if !reader.shares_budgeted_request(
            &self.ledger,
            &self.space_budget,
            self.deadline,
            &self.cancelled,
        ) {
            return Err(invalid("V1 migration base reader request differs"));
        }
        Ok(reader)
    }
    pub(crate) fn v1_manifest_max_bytes(&self) -> io::Result<u64> {
        self.tick()?;
        u64::try_from(self.limits.candidate.reader.max_manifest_bytes)
            .map_err(|_| invalid("V1 manifest read ceiling exceeds range"))
    }
    pub(crate) fn migration_member_count_limit(&self) -> io::Result<u64> {
        self.tick()?;
        u64::try_from(self.limits.candidate.admission.max_members)
            .map_err(|_| invalid("migration member-count ceiling exceeds range"))
    }
    pub(crate) fn migration_source_byte_limit(&self) -> io::Result<u64> {
        self.tick()?;
        Ok(self.limits.candidate.admission.max_source_bytes)
    }
    pub(crate) fn migration_member_byte_limit(&self) -> io::Result<u64> {
        self.tick()?;
        Ok(self.limits.candidate.admission.max_member_bytes)
    }
    pub(crate) fn migration_history_identity_limit(&self) -> io::Result<u64> {
        self.tick()?;
        u64::try_from(self.limits.candidate.max_history_identities)
            .map_err(|_| invalid("migration identity-count ceiling exceeds range"))
    }
    pub(crate) fn migration_history_revision_limit(&self) -> io::Result<u64> {
        self.tick()?;
        u64::try_from(self.limits.candidate.max_history_revisions)
            .map_err(|_| invalid("migration revision-count ceiling exceeds range"))
    }
    pub(crate) fn migration_pointer_read_limits(&self) -> io::Result<ReadLimits> {
        self.tick()?;
        Ok(self.limits.candidate.reader)
    }
    pub(crate) fn begin_v1_migration_history(&self) -> io::Result<()> {
        self.tick()?;
        if self.base.is_none() || self.base_v2.is_some() || self.v2_io.is_none() {
            return Err(invalid("V1 migration history scratch lacks a V1 base"));
        }
        let result = self
            .db
            .execute("DELETE FROM v1_migration_history", [])
            .map(|_| ())
            .map_err(sql);
        self.finish_read(result)
    }
    pub(crate) fn stage_v1_migration_history_row(
        &self,
        revision: Digest256,
        raw: &[u8],
    ) -> io::Result<()> {
        self.tick()?;
        let result = (|| {
            if raw.is_empty()
                || raw.len()
                    > super::source_admission_segment_v2::SourceRevisionRootsV2::MAX_ENCODED_BYTES
            {
                return Err(invalid("V1 migration history row exceeds wire profile"));
            }
            let live = raw
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(4096))
                .ok_or_else(|| invalid("V1 migration history scratch state overflow"))?;
            self.check_state(live)?;
            let logical = u64::try_from(raw.len())
                .map_err(|_| invalid("V1 migration history row length exceeds range"))?
                .checked_add(32)
                .ok_or_else(|| invalid("V1 migration history logical write overflow"))?;
            self.reserve_logical(0, logical)?;
            self.db
                .execute(
                    "INSERT INTO v1_migration_history(revision,raw) VALUES(?1,?2)",
                    params![revision.as_bytes().as_slice(), raw],
                )
                .map_err(sql)?;
            Ok(())
        })();
        self.finish_read(result)
    }
    pub(crate) fn v1_migration_history_after(
        &self,
        after: Option<Digest256>,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<(Digest256, Vec<u8>)>> {
        self.tick()?;
        let result = (|| {
            self.check_state(max_owned_state_bytes)?;
            let decode = |row: &rusqlite::Row<'_>| {
                let revision = match row.get_ref(0)? {
                    rusqlite::types::ValueRef::Blob(bytes) if bytes.len() == 32 => {
                        Digest256::from_bytes(
                            bytes
                                .try_into()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        )
                    }
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let raw_len = match row.get_ref(1)? {
                    rusqlite::types::ValueRef::Blob(bytes)
                        if !bytes.is_empty()
                            && bytes.len() <= super::source_admission_segment_v2::SourceRevisionRootsV2::MAX_ENCODED_BYTES => bytes.len(),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let state = raw_len
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(4096))
                    .ok_or(rusqlite::Error::InvalidQuery)?;
                if state > max_owned_state_bytes {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Ok((revision, row.get::<_, Vec<u8>>(1)?))
            };
            let row = match after {
                Some(after) => self.db.query_row(
                    "SELECT revision,raw FROM v1_migration_history WHERE revision>?1 ORDER BY revision LIMIT 1",
                    [after.as_bytes().as_slice()],
                    decode,
                ),
                None => self.db.query_row(
                    "SELECT revision,raw FROM v1_migration_history ORDER BY revision LIMIT 1",
                    [],
                    decode,
                ),
            }
            .optional()
            .map_err(sql)?;
            if let Some((_, raw)) = &row {
                self.reserve_logical(
                    u64::try_from(raw.len())
                        .map_err(|_| invalid("V1 migration history logical read overflow"))?
                        .checked_add(32)
                        .ok_or_else(|| invalid("V1 migration history logical read overflow"))?,
                    0,
                )?;
            }
            Ok(row)
        })();
        self.finish_read(result)
    }
    pub(crate) fn v2_base_session(&self) -> io::Result<&RefCell<V2ReadSession>> {
        self.base_v2
            .ok_or_else(|| invalid("V2 base reader absent during successor build"))
    }
    pub(crate) fn v2_cursor_state_allowance(&self, maximum: usize) -> io::Result<usize> {
        self.tick()?;
        let allowance = self.row_state_ceiling.get().min(maximum);
        if allowance == 0 {
            return Err(invalid("V2 candidate cursor state allowance is empty"));
        }
        Ok(allowance)
    }
    pub(crate) fn validator_sha256(&self) -> Digest256 {
        self.batch.validator_sha256
    }
    /// The adapter inherits this exact original invocation clock and cancel
    /// object; a shorter/new clock cannot reset the candidate's shared ledger.
    pub(crate) fn matches_invocation(&self, deadline: Instant, cancelled: &AtomicBool) -> bool {
        self.deadline == deadline && std::ptr::eq(self.cancelled.as_ref(), cancelled)
    }
    pub(crate) fn membership(&self) -> SourceMembershipV1 {
        self.membership
    }
    pub(crate) fn membership_counts(&self) -> (u64, u64) {
        (self.membership.count, self.source_bytes)
    }
    fn put(&self, m: &MemberMetadata, changed: bool) -> io::Result<()> {
        self.check_state(
            m.path
                .as_str()
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(1024))
                .ok_or_else(|| invalid("candidate row state overflow"))?,
        )?;
        self.db.execute("INSERT INTO members VALUES(?1,?2,?3,?4,?5,0) ON CONFLICT(path) DO UPDATE SET sha=excluded.sha,size=excluded.size,mode=excluded.mode,changed=excluded.changed",params![m.path.as_str(),m.sha256.as_bytes().as_slice(),m.size_bytes.to_be_bytes().as_slice(),m.mode,changed]).map_err(sql)?;
        Ok(())
    }
    fn raw_member(&self, path: &RelativePath) -> io::Result<Option<MemberMetadata>> {
        self.running()?;
        self.db
            .query_row(
                "SELECT path,sha,size,mode FROM members WHERE path=?1",
                [path.as_str()],
                |row| member_from_row(row, self.row_state_ceiling.get()),
            )
            .optional()
            .map_err(sql)?
            .map(member)
            .transpose()
    }
    pub(crate) fn member(&self, path: &RelativePath) -> io::Result<Option<MemberMetadata>> {
        self.tick()?;
        self.finish_read(self.raw_member(path))
    }
    pub(crate) fn member_bounded(
        &self,
        path: &RelativePath,
        allowance: usize,
    ) -> io::Result<Option<MemberMetadata>> {
        self.tick()?;
        let result = (|| {
            self.check_state(allowance)?;
            let row = self
                .db
                .query_row(
                    "SELECT path,sha,size,mode FROM members WHERE path=?1",
                    [path.as_str()],
                    |row| member_from_row(row, allowance),
                )
                .optional()
                .map_err(sql)?;
            row.map(member).transpose()
        })();
        self.finish_read(result)
    }
    pub(crate) fn member_after_bounded(
        &self,
        after: Option<&RelativePath>,
        allowance: usize,
    ) -> io::Result<Option<MemberMetadata>> {
        self.tick()?;
        let result = (|| {
            let argument = cursor_argument_state(after.map_or(0, |path| path.as_str().len()))?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("candidate member cursor state exceeds profile"))?;
            self.raw_after_bounded(after, row_allowance)
        })();
        self.finish_read(result)
    }
    fn raw_after(&self, after: Option<&RelativePath>) -> io::Result<Option<MemberMetadata>> {
        self.raw_after_bounded(after, self.row_state_ceiling.get())
    }
    fn raw_after_bounded(
        &self,
        after: Option<&RelativePath>,
        owned_allowance: usize,
    ) -> io::Result<Option<MemberMetadata>> {
        self.running()?;
        let row = match after {
            Some(p) => self.db.query_row(
                "SELECT path,sha,size,mode FROM members WHERE path>?1 ORDER BY path LIMIT 1",
                [p.as_str()],
                |row| member_from_row(row, owned_allowance),
            ),
            None => self.db.query_row(
                "SELECT path,sha,size,mode FROM members ORDER BY path LIMIT 1",
                [],
                |row| member_from_row(row, owned_allowance),
            ),
        }
        .optional()
        .map_err(sql)?;
        row.map(member).transpose()
    }
    /// Strict descendants use the exact binary interval [directory+'/', directory+'0').
    /// Starting after the directory itself would stop incorrectly on a preceding sibling.
    pub(crate) fn member_under_after_bounded(
        &self,
        directory: &RelativePath,
        after: Option<&RelativePath>,
        allowance: usize,
    ) -> io::Result<Option<MemberMetadata>> {
        self.tick()?;
        let result = (|| {
            self.check_state(allowance)?;
            let parameters = directory
                .as_str()
                .len()
                .checked_mul(32)
                .and_then(|n| n.checked_add(after.map_or(0, |p| p.as_str().len()).checked_mul(16)?))
                .and_then(|n| n.checked_add(8192))
                .ok_or_else(|| invalid("candidate prefix parameter state overflow"))?;
            let row_allowance = allowance
                .checked_sub(parameters)
                .ok_or_else(|| invalid("candidate prefix state exceeded"))?;
            let lower = format!("{}/", directory.as_str());
            let upper = format!("{}0", directory.as_str());
            if after.is_some_and(|p| !p.as_str().starts_with(&lower)) {
                return Err(invalid("candidate prefix cursor outside scope"));
            }
            let row = match after {
                None => self.db.query_row(
                    "SELECT path,sha,size,mode FROM members WHERE path>=?1 AND path<?2 ORDER BY path LIMIT 1",
                    params![lower,upper],|row|member_from_row(row,row_allowance)),
                Some(p) => self.db.query_row(
                    "SELECT path,sha,size,mode FROM members WHERE path>?1 AND path>=?2 AND path<?3 ORDER BY path LIMIT 1",
                    params![p.as_str(),lower,upper],|row|member_from_row(row,row_allowance)),
            }.optional().map_err(sql)?;
            row.map(member).transpose()
        })();
        self.finish_read(result)
    }
    pub(crate) fn member_after(
        &self,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<MemberMetadata>> {
        self.tick()?;
        self.finish_read(self.raw_after(after))
    }
    /// Stream only member rows changed by this batch in binary path order.
    /// Base rows remain available through `member_after`; this delta cursor is
    /// for immutable V2 successor writers and grants no publication authority.
    pub(crate) fn changed_member_after(
        &self,
        after: Option<&RelativePath>,
        allowance: usize,
    ) -> io::Result<Option<MemberMetadata>> {
        self.tick()?;
        let result = (|| {
            let input = after.map_or(0, |path| path.as_str().len());
            let argument = cursor_argument_state(input)?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("changed-member cursor state exceeds profile"))?;
            let row = match after {
                Some(after) => self.db.query_row(
                    "SELECT path,sha,size,mode FROM members WHERE changed=1 AND path>?1 ORDER BY path LIMIT 1",
                    [after.as_str()],
                    |row| member_from_row(row, row_allowance),
                ),
                None => self.db.query_row(
                    "SELECT path,sha,size,mode FROM members WHERE changed=1 ORDER BY path LIMIT 1",
                    [],
                    |row| member_from_row(row, row_allowance),
                ),
            }
            .optional()
            .map_err(sql)?;
            row.map(member).transpose()
        })();
        self.finish_read(result)
    }
    /// Stream source paths removed by this batch in binary path order so a
    /// V2 current-root writer can emit exact tombstones without a flat set.
    pub(crate) fn retired_path_after(
        &self,
        after: Option<&RelativePath>,
        allowance: usize,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            let input = after.map_or(0, |path| path.as_str().len());
            let argument = cursor_argument_state(input)?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("retirement cursor state exceeds profile"))?;
            let path = match after {
                Some(after) => self.db.query_row(
                    "SELECT path FROM retirements WHERE path>?1 ORDER BY path,ordinal LIMIT 1",
                    [after.as_str()],
                    |row| bounded_text(row, row_allowance),
                ),
                None => self.db.query_row(
                    "SELECT path FROM retirements ORDER BY path,ordinal LIMIT 1",
                    [],
                    |row| bounded_text(row, row_allowance),
                ),
            }
            .optional()
            .map_err(sql)?;
            path.map(|path| RelativePath::parse(&path).map_err(invalid))
                .transpose()
        })();
        self.finish_read(result)
    }
    /// Stream only retirement paths introduced by this candidate. Historical
    /// events remain in the immutable retirement root, while these paths join
    /// changed members to drive current-root deltas.
    pub(crate) fn new_retired_path_after(
        &self,
        after: Option<&RelativePath>,
        allowance: usize,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            let input = after.map_or(0, |path| path.as_str().len());
            let argument = cursor_argument_state(input)?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("new retirement cursor state exceeds profile"))?;
            let row = match after {
                Some(after) => self.db.query_row(
                    "SELECT path FROM retirements WHERE ordinal>=?1 AND path>?2 ORDER BY path,ordinal LIMIT 1",
                    params![i64::try_from(self.new_retirement_start).map_err(invalid)?, after.as_str()],
                    |row| bounded_text(row, row_allowance),
                ),
                None => self.db.query_row(
                    "SELECT path FROM retirements WHERE ordinal>=?1 ORDER BY path,ordinal LIMIT 1",
                    [i64::try_from(self.new_retirement_start).map_err(invalid)?],
                    |row| bounded_text(row, row_allowance),
                ),
            }
            .optional()
            .map_err(sql)?;
            row.map(|path| RelativePath::parse(&path).map_err(invalid))
                .transpose()
        })();
        self.finish_read(result)
    }
    /// Merge changed members and new tombstones into one strictly path-ordered
    /// cursor. The SQL union is bounded by the indexed changed subset.
    pub(crate) fn changed_source_after(
        &self,
        after: Option<&RelativePath>,
        allowance: usize,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            let input = after.map_or(0, |path| path.as_str().len());
            let argument = cursor_argument_state(input)?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("changed-source cursor state exceeds profile"))?;
            let start = i64::try_from(self.new_retirement_start).map_err(invalid)?;
            let row = match after {
                Some(after) => self.db.query_row(
                    "SELECT path FROM (SELECT path FROM members WHERE changed=1 UNION SELECT path FROM retirements WHERE ordinal>=?1) WHERE path>?2 ORDER BY path LIMIT 1",
                    params![start, after.as_str()],
                    |row| bounded_text(row, row_allowance),
                ),
                None => self.db.query_row(
                    "SELECT path FROM (SELECT path FROM members WHERE changed=1 UNION SELECT path FROM retirements WHERE ordinal>=?1) ORDER BY path LIMIT 1",
                    [start],
                    |row| bounded_text(row, row_allowance),
                ),
            }
            .optional()
            .map_err(sql)?;
            row.map(|path| RelativePath::parse(&path).map_err(invalid))
                .transpose()
        })();
        self.finish_read(result)
    }
    pub(crate) fn clear_v2_delta_rows(&self) -> io::Result<()> {
        self.tick()?;
        let result = self
            .db
            .execute_batch("CREATE TABLE IF NOT EXISTS v2_identity_delta(id TEXT COLLATE BINARY PRIMARY KEY,path TEXT) WITHOUT ROWID; CREATE TABLE IF NOT EXISTS v2_dependency_delta(key BLOB PRIMARY KEY,value BLOB) WITHOUT ROWID; DELETE FROM v2_identity_delta; DELETE FROM v2_dependency_delta;")
            .map_err(sql);
        self.finish_read(result)
    }
    pub(crate) fn put_v2_identity_delta(
        &self,
        id: &str,
        path: Option<&RelativePath>,
    ) -> io::Result<()> {
        self.tick()?;
        let result = (|| {
            let state = id
                .len()
                .checked_add(path.map_or(0, |path| path.as_str().len()))
                .and_then(|n| n.checked_mul(16))
                .and_then(|n| n.checked_add(4096))
                .ok_or_else(|| invalid("V2 identity delta state overflow"))?;
            self.check_state(state)?;
            self.db
                .execute(
                    "INSERT INTO v2_identity_delta(id,path) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET path=excluded.path",
                    params![id, path.map(RelativePath::as_str)],
                )
                .map_err(sql)?;
            Ok(())
        })();
        self.finish_read(result)
    }
    pub(crate) fn v2_identity_delta_after(
        &self,
        after: Option<&str>,
        allowance: usize,
    ) -> io::Result<Option<(String, Option<RelativePath>)>> {
        self.tick()?;
        let result = (|| {
            let argument = cursor_argument_state(after.map_or(0, str::len))?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("V2 identity delta cursor state exceeds profile"))?;
            let row = match after {
                Some(after) => self.db.query_row(
                    "SELECT id,path FROM v2_identity_delta WHERE id>?1 ORDER BY id LIMIT 1",
                    [after],
                    |row| {
                        let id = bounded_text(row, row_allowance)?;
                        let path = match row.get_ref(1)? {
                            rusqlite::types::ValueRef::Null => None,
                            rusqlite::types::ValueRef::Text(bytes) => {
                                let state = bytes.len().checked_add(size_of::<RelativePath>());
                                if state.is_none_or(|state| state > row_allowance) {
                                    return Err(rusqlite::Error::InvalidQuery);
                                }
                                let text = std::str::from_utf8(bytes)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                                Some(
                                    RelativePath::parse(text)
                                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                )
                            }
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        };
                        Ok((id, path))
                    },
                ),
                None => self.db.query_row(
                    "SELECT id,path FROM v2_identity_delta ORDER BY id LIMIT 1",
                    [],
                    |row| {
                        let id = bounded_text(row, row_allowance)?;
                        let path = match row.get_ref(1)? {
                            rusqlite::types::ValueRef::Null => None,
                            rusqlite::types::ValueRef::Text(bytes) => {
                                let state = bytes.len().checked_add(size_of::<RelativePath>());
                                if state.is_none_or(|state| state > row_allowance) {
                                    return Err(rusqlite::Error::InvalidQuery);
                                }
                                let text = std::str::from_utf8(bytes)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                                Some(
                                    RelativePath::parse(text)
                                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                )
                            }
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        };
                        Ok((id, path))
                    },
                ),
            }
            .optional()
            .map_err(sql)?;
            Ok(row)
        })();
        self.finish_read(result)
    }
    pub(crate) fn put_v2_dependency_delta(
        &self,
        key: &[u8],
        value: Option<&[u8]>,
    ) -> io::Result<()> {
        self.tick()?;
        let result = (|| {
            let state = key
                .len()
                .checked_add(value.map_or(0, <[u8]>::len))
                .and_then(|n| n.checked_mul(8))
                .and_then(|n| n.checked_add(4096))
                .ok_or_else(|| invalid("V2 dependency delta state overflow"))?;
            self.check_state(state)?;
            self.db
                .execute(
                    "INSERT INTO v2_dependency_delta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    params![key, value],
                )
                .map_err(sql)?;
            Ok(())
        })();
        self.finish_read(result)
    }
    pub(crate) fn v2_dependency_delta_after(
        &self,
        after: Option<&[u8]>,
        allowance: usize,
    ) -> io::Result<Option<(Vec<u8>, Option<Vec<u8>>)>> {
        self.tick()?;
        let result = (|| {
            let argument = cursor_argument_state(after.map_or(0, <[u8]>::len))?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("V2 dependency delta cursor state exceeds profile"))?;
            let mut decode =
                |row: &rusqlite::Row<'_>| -> rusqlite::Result<(Vec<u8>, Option<Vec<u8>>)> {
                    let key = match row.get_ref(0)? {
                        rusqlite::types::ValueRef::Blob(bytes) if bytes.len() <= row_allowance => {
                            bytes.to_vec()
                        }
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    };
                    let value = match row.get_ref(1)? {
                        rusqlite::types::ValueRef::Null => None,
                        rusqlite::types::ValueRef::Blob(bytes) if bytes.len() <= row_allowance => {
                            Some(bytes.to_vec())
                        }
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    };
                    let state = key.len().checked_add(value.as_ref().map_or(0, Vec::len));
                    if state.is_none_or(|state| state > row_allowance) {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    Ok((key, value))
                };
            let row = match after {
                Some(after) => self.db.query_row(
                    "SELECT key,value FROM v2_dependency_delta WHERE key>?1 ORDER BY key LIMIT 1",
                    [after],
                    &mut decode,
                ),
                None => self.db.query_row(
                    "SELECT key,value FROM v2_dependency_delta ORDER BY key LIMIT 1",
                    [],
                    &mut decode,
                ),
            }
            .optional()
            .map_err(sql)?;
            Ok(row)
        })();
        self.finish_read(result)
    }
    /// Borrow one verified current member at a time. The callback's genuine
    /// retained native-parser state is supplied separately and charged together
    /// with the raw/metadata/keyset overlap BEFORE allocation. Membership is
    /// returned only after complete EOF and the same candidate fence, never as
    /// a fabricated revision or source-admission decision.
    pub(crate) fn for_each_verified_member(
        &self,
        max_member_bytes: usize,
        max_owned_state_bytes: usize,
        callback_retained_state_bytes: usize,
        visit: &mut dyn FnMut(&MemberMetadata, &[u8]) -> io::Result<()>,
    ) -> io::Result<SourceMembershipV1> {
        self.tick()?;
        let result = (|| {
            if max_member_bytes == 0 || max_owned_state_bytes > self.row_state_ceiling.get() {
                return Err(invalid("candidate full member visitor profile"));
            }
            let baseline = max_member_bytes
                .checked_mul(4)
                .and_then(|n| n.checked_add(callback_retained_state_bytes))
                .and_then(|n| n.checked_add(16384))
                .ok_or_else(|| invalid("candidate full member visitor state"))?;
            self.check_state(max_owned_state_bytes)?;
            let before = self.fence()?;
            let mut hash = Digest256Hasher::new();
            hash.update(b"tos-val-full-membership-v1\0");
            let mut count = 0u64;
            let mut source_bytes = 0u64;
            let mut after: Option<RelativePath> = None;
            loop {
                self.tick()?;
                let previous = after.as_ref().map_or(0, |p| p.as_str().len());
                let allowance = max_owned_state_bytes
                    .checked_sub(baseline)
                    .and_then(|n| n.checked_sub(previous.checked_mul(16)?))
                    .filter(|n| *n >= 1024)
                    .ok_or_else(|| invalid("candidate member metadata preallocation"))?;
                let Some(metadata) = self.raw_after_bounded(after.as_ref(), allowance)? else {
                    break;
                };
                if after
                    .as_ref()
                    .is_some_and(|p| metadata.path.as_str() <= p.as_str())
                    || metadata.size_bytes > max_member_bytes as u64
                {
                    return Err(invalid("candidate full member ordering/byte bound"));
                }
                let peak = baseline
                    .checked_add(
                        previous
                            .checked_mul(16)
                            .ok_or_else(|| invalid("candidate previous locator state"))?,
                    )
                    .and_then(|n| n.checked_add(metadata.path.as_str().len().checked_mul(32)?))
                    .filter(|n| *n <= max_owned_state_bytes)
                    .ok_or_else(|| invalid("candidate full member raw/metadata overlap"))?;
                self.check_state(peak)?;
                let raw = self.read_current_raw(metadata.path.as_str(), max_member_bytes)?;
                if raw.len() as u64 != metadata.size_bytes
                    || Digest256::of_bytes(&raw) != metadata.sha256
                {
                    return Err(invalid("candidate full member metadata/raw fixity"));
                }
                visit(&metadata, &raw)?;
                self.tick()?;
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= before.membership.count)
                    .ok_or_else(|| invalid("candidate full member count"))?;
                source_bytes = source_bytes
                    .checked_add(metadata.size_bytes)
                    .filter(|n| *n <= before.source_bytes)
                    .ok_or_else(|| invalid("candidate full member source bytes"))?;
                feed(&mut hash, &metadata);
                drop(raw);
                after = Some(metadata.path);
            }
            let membership = SourceMembershipV1 {
                count,
                digest: hash.finalize(),
            };
            if membership != before.membership
                || source_bytes != before.source_bytes
                || self.fence()? != before
            {
                return Err(invalid("candidate full member EOF/fence differs"));
            }
            self.tick()?;
            Ok(membership)
        })();
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }

    fn put_retirement(&mut self, r: &RetirementMetadata) -> io::Result<()> {
        self.running()?;
        self.check_state(
            r.path
                .as_str()
                .len()
                .checked_add(r.event_ref.as_str().len())
                .and_then(|n| n.checked_mul(4))
                .and_then(|n| n.checked_add(1024))
                .ok_or_else(|| invalid("retirement row state overflow"))?,
        )?;
        self.db
            .execute(
                "INSERT INTO retirements VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    i64::try_from(self.retirement_count).map_err(invalid)?,
                    r.path.as_str(),
                    r.sha256.as_bytes().as_slice(),
                    r.event_ref.as_str(),
                    r.event_sha256.as_bytes().as_slice(),
                    r.event_size_bytes.to_be_bytes().as_slice()
                ],
            )
            .map_err(sql)?;
        self.retirement_count = self
            .retirement_count
            .checked_add(1)
            .ok_or_else(|| invalid("retirement count overflow"))?;
        Ok(())
    }
    fn preflight_base_locator(&self, base: &StreamedCorpusCutReaderV1) -> io::Result<()> {
        // The foreign reader's VALIDATED raw-row limit bounds owned decoded
        // strings/JSON scratch and the finite simultaneous cursor clones.
        self.check_state(
            base.manifest_row_byte_limit()
                .checked_mul(64)
                .and_then(|n| n.checked_add(4096))
                .ok_or_else(|| invalid("base locator row-state overflow"))?,
        )
    }

    fn preflight_v2_base(&self) -> io::Result<&RefCell<V2ReadSession>> {
        let base = self
            .base_v2
            .ok_or_else(|| invalid("V2 base reader absent"))?;
        let io = self
            .v2_io
            .as_ref()
            .ok_or_else(|| invalid("V2 base lacks original IO ledger"))?;
        if !base.borrow().shares_io_budget(io) {
            return Err(invalid("V2 base original IO ledger differs"));
        }
        Ok(base)
    }

    fn import_v2_base(&mut self) -> io::Result<()> {
        self.db
            .execute_batch(
                "CREATE TABLE v2_history_chain(revision BLOB PRIMARY KEY CHECK(length(revision)=32),base_revision BLOB CHECK(base_revision IS NULL OR length(base_revision)=32)) WITHOUT ROWID; CREATE TABLE v2_history_seen(revision BLOB PRIMARY KEY CHECK(length(revision)=32)) WITHOUT ROWID; CREATE TABLE v2_base_identities(id TEXT COLLATE BINARY PRIMARY KEY,path TEXT NOT NULL) WITHOUT ROWID; CREATE INDEX v2_base_identities_by_path ON v2_base_identities(path COLLATE BINARY,id COLLATE BINARY); CREATE TABLE v2_base_dependencies(key BLOB PRIMARY KEY,value BLOB NOT NULL) WITHOUT ROWID;",
            )
            .map_err(sql)?;
        let base = self
            .base_v2
            .ok_or_else(|| invalid("V2 base reader absent during import"))?;
        let (
            revision,
            expected_membership,
            expected_member_count,
            expected_source_bytes,
            expected_identity_count,
            expected_dependency_sources,
            expected_dependency_count,
            expected_retirement_count,
            expected_history_count,
        ) = {
            let reader = base.borrow();
            let roots = reader.current_roots();
            if !self
                .v2_io
                .as_ref()
                .is_some_and(|io| reader.shares_io_budget(io))
            {
                return Err(invalid("V2 base original IO ledger differs"));
            }
            (
                reader.selected_revision(),
                roots.membership_v1,
                roots.member_count,
                roots.source_bytes,
                roots.identity_count,
                roots.dependency_source_count,
                roots.dependency_count,
                roots.retirement_count,
                reader.retained_history_count(),
            )
        };
        if expected_history_count > self.limits.candidate.max_history_revisions as u64 {
            return Err(invalid("candidate V2 history revision bound"));
        }
        let max_members =
            u64::try_from(self.limits.candidate.admission.max_members).map_err(invalid)?;
        let max_history_identities =
            u64::try_from(self.limits.candidate.max_history_identities).map_err(invalid)?;
        let max_retirements =
            u64::try_from(self.limits.candidate.reader.max_manifest_entries).map_err(invalid)?;
        let maximum_dependency_count = expected_member_count
            .checked_mul(expected_member_count.saturating_sub(1))
            .ok_or_else(|| invalid("V2 base dependency structural bound overflow"))?;
        if expected_member_count > max_members
            || expected_source_bytes > self.limits.candidate.admission.max_source_bytes
            || expected_identity_count > max_history_identities
            || expected_dependency_sources > expected_member_count
            || expected_dependency_count > maximum_dependency_count
            || expected_retirement_count > max_retirements
        {
            return Err(invalid("V2 base root counts exceed candidate profile"));
        }

        let mut after: Option<Vec<u8>> = None;
        let mut count = 0u64;
        let mut bytes = 0u64;
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-val-full-membership-v1\0");
        while let Some(row) = base.borrow_mut().next_row_after(
            revision,
            V2RootKind::Members,
            None,
            None,
            after.as_deref(),
        )? {
            self.running()?;
            self.check_state(v2_row_cursor_state_upper_bound(
                row.key.len(),
                row.value.len(),
                after.as_ref().map_or(0, Vec::len),
            )?)?;
            let member = decode_v2_member(&row.key, &row.value)?;
            if member.size_bytes > self.limits.candidate.admission.max_member_bytes {
                return Err(invalid("V2 base member exceeds selected source ceiling"));
            }
            self.put(&member, false)?;
            feed(&mut hash, &member);
            count = count
                .checked_add(1)
                .filter(|n| *n <= expected_member_count)
                .ok_or_else(|| invalid("V2 base member count exceeded"))?;
            bytes = bytes
                .checked_add(member.size_bytes)
                .ok_or_else(|| invalid("V2 base member bytes overflow"))?;
            after = Some(row.key);
        }
        let membership = SourceMembershipV1 {
            count,
            digest: hash.finalize(),
        };
        if count != expected_member_count
            || bytes != expected_source_bytes
            || membership != expected_membership
        {
            return Err(invalid("V2 base member EOF or membership differs"));
        }

        let mut retirement_after: Option<Vec<u8>> = None;
        let mut retirement_count = 0u64;
        while let Some(row) = base.borrow_mut().next_row_after(
            revision,
            V2RootKind::Retirements,
            None,
            None,
            retirement_after.as_deref(),
        )? {
            self.running()?;
            self.check_state(v2_row_cursor_state_upper_bound(
                row.key.len(),
                row.value.len(),
                retirement_after.as_ref().map_or(0, Vec::len),
            )?)?;
            let (ordinal, retirement) =
                decode_v2_retirement(&row.key, &row.value, self.row_state_ceiling.get())?;
            if ordinal != retirement_count {
                return Err(invalid("V2 base retirement ordinals are not contiguous"));
            }
            if retirement.path == retirement.event_ref
                || retirement.event_size_bytes > self.limits.candidate.admission.max_member_bytes
            {
                return Err(invalid("V2 base retirement tuple exceeds selected profile"));
            }
            self.put_retirement(&retirement)?;
            retirement_count = retirement_count
                .checked_add(1)
                .filter(|n| *n <= expected_retirement_count)
                .ok_or_else(|| invalid("V2 base retirement count exceeded"))?;
            retirement_after = Some(row.key);
        }
        if retirement_count != expected_retirement_count {
            return Err(invalid("V2 base retirement EOF differs"));
        }

        let mut dependency_after: Option<Vec<u8>> = None;
        let mut dependency_count = 0u64;
        let mut dependency_source_count = 0u64;
        while let Some(row) = base.borrow_mut().next_row_after(
            revision,
            V2RootKind::Dependencies,
            None,
            None,
            dependency_after.as_deref(),
        )? {
            self.running()?;
            self.check_state(v2_row_cursor_state_upper_bound(
                row.key.len(),
                row.value.len(),
                dependency_after.as_ref().map_or(0, Vec::len),
            )?)?;
            let (source, target) = decode_v2_dependency(&row.key, &row.value)?;
            if source == target
                || self.raw_member(&source)?.is_none()
                || self.raw_member(&target)?.is_none()
            {
                return Err(invalid(
                    "V2 base dependency endpoint is not a current member",
                ));
            }
            if dependency_after
                .as_deref()
                .and_then(|key| key.split(|byte| *byte == 0).next())
                != Some(source.as_str().as_bytes())
            {
                dependency_source_count = dependency_source_count
                    .checked_add(1)
                    .filter(|n| *n <= expected_dependency_sources)
                    .ok_or_else(|| invalid("V2 base dependency source count exceeded"))?;
            }
            self.db
                .execute(
                    "INSERT INTO reverse_dependencies VALUES(?1,?2)",
                    params![target.as_str(), source.as_str()],
                )
                .map_err(sql)?;
            self.check_state(4096)?;
            self.db
                .execute(
                    "INSERT INTO v2_base_dependencies(key,value) VALUES(?1,?2)",
                    params![row.key.as_slice(), row.value.as_slice()],
                )
                .map_err(sql)?;
            dependency_count = dependency_count
                .checked_add(1)
                .filter(|n| *n <= expected_dependency_count)
                .ok_or_else(|| invalid("V2 base dependency count exceeded"))?;
            dependency_after = Some(row.key);
        }
        if dependency_source_count != expected_dependency_sources
            || dependency_count != expected_dependency_count
        {
            return Err(invalid("V2 base dependency EOF differs"));
        }

        let mut history_after = None;
        let mut history_count = 0u64;
        let mut identity_visits = 0u64;
        let mut current_identity_count = 0u64;
        let mut current_revision_seen = false;
        loop {
            let next = base.borrow_mut().next_history_roots_after(
                history_after
                    .as_ref()
                    .map(|revision: &SourceRevision| &revision.0.as_bytes()[..]),
                self.row_state_ceiling.get(),
            )?;
            let Some((roots, _raw_bytes)) = next else {
                break;
            };
            self.running()?;
            // Reserve the bounded chain-row operation before copying even the
            // fixed-width optional base revision out of the decoded history
            // row. The reader's result allowance and this candidate's row
            // allowance are distinct caller-owned peaks.
            self.check_state(4096)?;
            let history_revision = roots.revision;
            let base_revision = roots.base_revision.map(|base| base.0.as_bytes().to_vec());
            let expected_identities = roots.identity_count;
            let is_current = history_revision == revision;
            let remaining_identity_bound = max_history_identities
                .checked_sub(identity_visits)
                .ok_or_else(|| invalid("V2 base history identity bound underflow"))?;
            if expected_identities > remaining_identity_bound {
                return Err(invalid("V2 base history identity count exceeds profile"));
            }
            self.db
                .execute(
                    "INSERT INTO v2_history_chain(revision,base_revision) VALUES(?1,?2)",
                    params![
                        history_revision.0.as_bytes().as_slice(),
                        base_revision.as_deref()
                    ],
                )
                .map_err(sql)?;
            drop(roots);
            history_count = history_count
                .checked_add(1)
                .filter(|n| *n <= expected_history_count)
                .ok_or_else(|| invalid("V2 base history count exceeded"))?;

            let mut id_after: Option<Vec<u8>> = None;
            let mut observed_identities = 0u64;
            while let Some(row) = base.borrow_mut().next_row_after(
                history_revision,
                V2RootKind::Identities,
                None,
                None,
                id_after.as_deref(),
            )? {
                self.running()?;
                self.check_state(v2_row_cursor_state_upper_bound(
                    row.key.len(),
                    row.value.len(),
                    id_after.as_ref().map_or(0, Vec::len),
                )?)?;
                let (id, path) = decode_v2_identity(&row.key, &row.value)?;
                if is_current && self.raw_member(&path)?.is_none() {
                    return Err(invalid(
                        "V2 current identity points outside current membership",
                    ));
                }
                if is_current {
                    self.check_state(4096)?;
                    self.db
                        .execute(
                            "INSERT INTO v2_base_identities(id,path) VALUES(?1,?2)",
                            params![id, path.as_str()],
                        )
                        .map_err(sql)?;
                }
                identity_visits = identity_visits
                    .checked_add(1)
                    .filter(|n| *n <= self.limits.candidate.max_history_identities as u64)
                    .ok_or_else(|| invalid("candidate V2 historical identity bound"))?;
                let old: Option<String> = self
                    .db
                    .query_row(
                        "SELECT path FROM historical_ids WHERE id=?1",
                        [&id],
                        |row| bounded_text(row, self.row_state_ceiling.get()),
                    )
                    .optional()
                    .map_err(sql)?;
                if old.as_ref().is_some_and(|old| old != path.as_str()) {
                    return Err(invalid("historical identity ownership conflict"));
                }
                self.db
                    .execute(
                        "INSERT OR IGNORE INTO historical_ids VALUES(?1,?2)",
                        params![id, path.as_str()],
                    )
                    .map_err(sql)?;
                observed_identities = observed_identities
                    .checked_add(1)
                    .ok_or_else(|| invalid("V2 identity EOF count overflow"))?;
                id_after = Some(row.key);
            }
            if observed_identities != expected_identities {
                return Err(invalid("V2 historical identity EOF differs"));
            }
            if is_current {
                current_revision_seen = true;
                current_identity_count = observed_identities;
            }
            history_after = Some(history_revision);
        }
        if history_count != expected_history_count
            || !current_revision_seen
            || current_identity_count != expected_identity_count
        {
            return Err(invalid("V2 base history or current identity EOF differs"));
        }
        let mut chain_revision = revision;
        let mut chain_count = 0u64;
        loop {
            self.check_state(4096)?;
            let inserted = self
                .db
                .execute(
                    "INSERT OR IGNORE INTO v2_history_seen(revision) VALUES(?1)",
                    [chain_revision.0.as_bytes().as_slice()],
                )
                .map_err(sql)?;
            if inserted != 1 {
                return Err(invalid("V2 base history contains a cycle"));
            }
            chain_count = chain_count
                .checked_add(1)
                .filter(|n| *n <= expected_history_count)
                .ok_or_else(|| invalid("V2 base history chain exceeds root count"))?;
            let base_revision: Option<Option<Vec<u8>>> = self
                .db
                .query_row(
                    "SELECT base_revision FROM v2_history_chain WHERE revision=?1",
                    [chain_revision.0.as_bytes().as_slice()],
                    |row| match row.get_ref(0)? {
                        rusqlite::types::ValueRef::Null => Ok(None),
                        rusqlite::types::ValueRef::Blob(bytes) if bytes.len() == 32 => {
                            Ok(Some(bytes.to_vec()))
                        }
                        _ => Err(rusqlite::Error::InvalidQuery),
                    },
                )
                .optional()
                .map_err(sql)?;
            let Some(base_revision) = base_revision else {
                return Err(invalid("V2 base history ancestor is absent"));
            };
            let Some(base_revision) = base_revision else {
                break;
            };
            let digest_bytes: [u8; 32] = base_revision.as_slice().try_into().map_err(invalid)?;
            chain_revision = SourceRevision(Digest256::from_bytes(digest_bytes));
        }
        if chain_count != expected_history_count {
            return Err(invalid("V2 base history contains unreachable revisions"));
        }
        base.borrow().verify_current_fence()?;
        Ok(())
    }

    fn import_and_prepare(&mut self) -> io::Result<()> {
        if let Some(base) = self.base {
            self.preflight_base_locator(base)?;
            if base.revision_count() > self.limits.candidate.max_history_revisions as u64 {
                return Err(invalid("candidate history revision bound"));
            }
            let revision = base.current_revision();
            let expected = base
                .revision(revision)
                .map_err(invalid)?
                .ok_or_else(|| invalid("base metadata absent"))?;
            let mut after = None;
            let mut count = 0u64;
            let mut hash = Digest256Hasher::new();
            hash.update(b"tos-val-full-membership-v1\0");
            while let Some(m) = base
                .member_after(revision, after.as_ref())
                .map_err(invalid)?
            {
                self.running()?;
                self.put(&m, false)?;
                feed(&mut hash, &m);
                count = count
                    .checked_add(1)
                    .ok_or_else(|| invalid("base member count overflow"))?;
                after = Some(m.path);
            }
            if (SourceMembershipV1 {
                count,
                digest: hash.finalize(),
            }) != expected.membership
            {
                return Err(invalid("base EOF membership differs"));
            }
            for i in 0..expected.retirement_count {
                let r = base
                    .retirement_at(revision, i)
                    .map_err(invalid)?
                    .ok_or_else(|| invalid("base retirement absent"))?;
                self.put_retirement(&r)?;
            }
            if base.revision_count() > self.limits.candidate.max_history_revisions as u64 {
                return Err(invalid("candidate history revision bound"));
            }
            let mut identity_visits = 0u64;
            for ordinal in 0..base.revision_count() {
                let history = base
                    .revision_at(ordinal)
                    .map_err(invalid)?
                    .ok_or_else(|| invalid("candidate historical revision absent"))?;
                let mut id_after = None;
                let mut observed = 0u64;
                while let Some((id, path)) = base
                    .identity_after(history.revision, id_after.as_deref())
                    .map_err(invalid)?
                {
                    self.running()?;
                    self.check_state(
                        id.len()
                            .checked_add(path.as_str().len())
                            .and_then(|n| n.checked_mul(16))
                            .and_then(|n| n.checked_add(1024))
                            .ok_or_else(|| invalid("history row state overflow"))?,
                    )?;
                    identity_visits = identity_visits
                        .checked_add(1)
                        .filter(|n| *n <= self.limits.candidate.max_history_identities as u64)
                        .ok_or_else(|| invalid("candidate historical identity bound"))?;
                    let old: Option<String> = self
                        .db
                        .query_row("SELECT path FROM historical_ids WHERE id=?1", [&id], |r| {
                            bounded_text(r, self.row_state_ceiling.get())
                        })
                        .optional()
                        .map_err(sql)?;
                    if old.as_ref().is_some_and(|old| old != path.as_str()) {
                        return Err(invalid("historical identity ownership conflict"));
                    }
                    self.db
                        .execute(
                            "INSERT OR IGNORE INTO historical_ids VALUES(?1,?2)",
                            params![id, path.as_str()],
                        )
                        .map_err(sql)?;
                    observed = observed
                        .checked_add(1)
                        .ok_or_else(|| invalid("history EOF count overflow"))?;
                    id_after = Some(id);
                }
                if observed != history.identity_count {
                    return Err(invalid("historical identity EOF differs"));
                }
            }
            let mut source_after = None;
            while let Some(source) = base
                .dependency_source_after(revision, source_after.as_ref())
                .map_err(invalid)?
            {
                let mut target_after = None;
                while let Some(target) = base
                    .dependency_after(revision, &source, target_after.as_ref())
                    .map_err(invalid)?
                {
                    self.running()?;
                    self.db
                        .execute(
                            "INSERT INTO reverse_dependencies VALUES(?1,?2)",
                            params![target.as_str(), source.as_str()],
                        )
                        .map_err(sql)?;
                    target_after = Some(target);
                }
                source_after = Some(source);
            }
        } else if self.base_v2.is_some() {
            self.import_v2_base()?;
        }
        self.new_retirement_start = self.retirement_count;
        // Plan complete membership before the first immutable object ingest.
        for (path, u) in &self.batch.updates {
            self.running()?;
            let path = RelativePath::parse(path).map_err(invalid)?;
            let m = MemberMetadata {
                path,
                sha256: u.sha256,
                size_bytes: u.size_bytes,
                mode: u.mode,
            };
            let old = self.raw_member(&m.path)?;
            let changed = old.as_ref().is_none_or(|old| {
                old.sha256 != m.sha256 || old.size_bytes != m.size_bytes || old.mode != m.mode
            });
            self.put(&m, changed)?;
            if changed {
                self.db
                    .execute(
                        "INSERT OR IGNORE INTO affected VALUES(?1,0)",
                        [m.path.as_str()],
                    )
                    .map_err(sql)?;
            }
        }
        // Only this pre-existing bounded batch is resident; base/manifest/history
        // cardinality is never copied into a Vec. Charge its row clones first.
        let clone_state = self
            .batch
            .retirements
            .iter()
            .try_fold(0usize, |sum, (p, r)| {
                sum.checked_add(p.len() + r.event_ref.as_str().len() + 256)
                    .ok_or_else(|| invalid("retirement batch state overflow"))
            })?;
        self.check_state(clone_state)?;
        let retirements: Vec<_> = self
            .batch
            .retirements
            .iter()
            .map(|(p, r)| (p.clone(), r.event_ref.clone(), r.event_sha256))
            .collect();
        for (path, event_ref, event_sha) in retirements {
            self.running()?;
            let path = RelativePath::parse(&path).map_err(invalid)?;
            if event_ref == path || self.batch.retirements.contains_key(event_ref.as_str()) {
                return Err(invalid("retirement event is removed by same batch"));
            }
            let target = self
                .raw_member(&path)?
                .ok_or_else(|| invalid("retirement needs existing path"))?;
            let event = self
                .raw_member(&event_ref)?
                .ok_or_else(|| invalid("retirement event missing"))?;
            if event.sha256 != event_sha {
                return Err(invalid("retirement event digest differs"));
            }
            let duplicate=self.db.query_row("SELECT 1 FROM retirements WHERE path=?1 AND sha=?2 AND event_ref=?3 AND event_sha=?4 LIMIT 1",params![path.as_str(),target.sha256.as_bytes().as_slice(),event_ref.as_str(),event_sha.as_bytes().as_slice()],|_|Ok(())).optional().map_err(sql)?.is_some();
            if duplicate {
                return Err(invalid("retirement event already exists in history"));
            }
            self.put_retirement(&RetirementMetadata {
                path: path.clone(),
                sha256: target.sha256,
                event_ref,
                event_sha256: event_sha,
                event_size_bytes: event.size_bytes,
            })?;
            self.db
                .execute("DELETE FROM members WHERE path=?1", [path.as_str()])
                .map_err(sql)?;
            self.db
                .execute(
                    "INSERT OR IGNORE INTO affected VALUES(?1,0)",
                    [path.as_str()],
                )
                .map_err(sql)?;
        }
        let mut after = None;
        let mut count = 0u64;
        let mut bytes = 0u64;
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-val-full-membership-v1\0");
        while let Some(m) = self.raw_after(after.as_ref())? {
            if m.size_bytes > self.limits.candidate.admission.max_member_bytes {
                return Err(invalid("candidate member byte bound exceeded"));
            }
            count = count
                .checked_add(1)
                .filter(|n| *n <= self.limits.candidate.admission.max_members as u64)
                .ok_or_else(|| invalid("candidate member count exceeded"))?;
            bytes = bytes
                .checked_add(m.size_bytes)
                .filter(|n| *n <= self.limits.candidate.admission.max_source_bytes)
                .ok_or_else(|| invalid("candidate source bytes exceeded"))?;
            feed(&mut hash, &m);
            after = Some(m.path);
        }
        self.membership = SourceMembershipV1 {
            count,
            digest: hash.finalize(),
        };
        self.source_bytes = bytes;
        // The fixed point uses indexed incoming edges and a disk-backed queue.
        loop {
            self.running()?;
            let next: Option<String> = self
                .db
                .query_row(
                    "SELECT path FROM affected WHERE visited=0 ORDER BY path LIMIT 1",
                    [],
                    |r| bounded_text(r, self.row_state_ceiling.get()),
                )
                .optional()
                .map_err(sql)?;
            let Some(path) = next else { break };
            self.db.execute("INSERT OR IGNORE INTO affected SELECT source,0 FROM reverse_dependencies WHERE target=?1",[&path]).map_err(sql)?;
            self.db
                .execute("UPDATE affected SET visited=1 WHERE path=?1", [&path])
                .map_err(sql)?;
        }
        // Reserve the COMPLETE immutable-object work before the first ingest.
        // Pager work continues to debit this same ledger independently.
        let mut object_reads = 0u64;
        let mut object_writes = 0u64;
        for u in self.batch.updates.values() {
            object_reads = object_reads
                .checked_add(
                    u.size_bytes
                        .checked_mul(3)
                        .ok_or_else(|| invalid("candidate read charge overflow"))?,
                )
                .ok_or_else(|| invalid("candidate read charge overflow"))?;
            object_writes = object_writes
                .checked_add(u.size_bytes)
                .ok_or_else(|| invalid("candidate write charge overflow"))?;
        }
        for i in self.new_retirement_start..self.retirement_count {
            let r = self
                .retirement_at_raw(i)?
                .ok_or_else(|| invalid("new retirement absent"))?;
            let size = match self.batch.updates.get(r.path.as_str()) {
                Some(update) if update.sha256 == r.sha256 => update.size_bytes,
                _ => self.store.object_size(
                    r.sha256,
                    self.limits.candidate.admission.max_member_bytes,
                    self.deadline,
                    &self.cancelled,
                )?,
            };
            object_reads = object_reads
                .checked_add(size)
                .and_then(|n| n.checked_add(r.event_size_bytes))
                .ok_or_else(|| invalid("retirement read charge overflow"))?;
        }
        self.reserve_logical(object_reads, object_writes)?;
        for (path, u) in &self.batch.updates {
            let mut input = self
                .batch
                .open_update(path, self.deadline, &self.cancelled)?;
            self.store.ingest_accounted(
                &mut input,
                u.size_bytes,
                u.sha256,
                self.deadline,
                &self.cancelled,
                &|n| self.debit_read(n),
                &|n| self.debit_write(n),
                &|n| self.ledger.record_read_returned(n).map_err(invalid),
                &|n| self.ledger.record_write_returned(n).map_err(invalid),
            )?;
        }
        self.store.sync_objects(self.deadline, &self.cancelled)?;
        for i in self.new_retirement_start..self.retirement_count {
            let r = self
                .retirement_at_raw(i)?
                .ok_or_else(|| invalid("new retirement absent"))?;
            let size = self.store.object_size(
                r.sha256,
                self.limits.candidate.admission.max_member_bytes,
                self.deadline,
                &self.cancelled,
            )?;
            self.store.verify_object_accounted(
                r.sha256,
                size,
                self.deadline,
                &self.cancelled,
                &|n| self.debit_read(n),
                &|_| Ok(()),
                &|n| self.ledger.record_read_returned(n).map_err(invalid),
                &|n| self.ledger.record_write_returned(n).map_err(invalid),
            )?;
            self.store.verify_object_accounted(
                r.event_sha256,
                r.event_size_bytes,
                self.deadline,
                &self.cancelled,
                &|n| self.debit_read(n),
                &|_| Ok(()),
                &|n| self.ledger.record_read_returned(n).map_err(invalid),
                &|n| self.ledger.record_write_returned(n).map_err(invalid),
            )?;
        }
        self.running()
    }
    pub(crate) fn affected_after(
        &self,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            let path: Option<String> = match after {
                Some(p) => self.db.query_row(
                    "SELECT path FROM affected WHERE path>?1 ORDER BY path LIMIT 1",
                    [p.as_str()],
                    |r| bounded_text(r, self.row_state_ceiling.get()),
                ),
                None => {
                    self.db
                        .query_row("SELECT path FROM affected ORDER BY path LIMIT 1", [], |r| {
                            bounded_text(r, self.row_state_ceiling.get())
                        })
                }
            }
            .optional()
            .map_err(sql)?;
            path.map(|p| RelativePath::parse(&p).map_err(invalid))
                .transpose()
        })();
        self.finish_read(result)
    }
    fn selected(&self, path: &str) -> io::Result<MemberMetadata> {
        let path = RelativePath::parse(path).map_err(invalid)?;
        self.member(&path)?
            .ok_or_else(|| invalid("read outside candidate membership"))
    }
    fn mark_read(&self, path: &str) -> io::Result<()> {
        self.db
            .execute("UPDATE members SET touched=1 WHERE path=?1", [path])
            .map_err(sql)?;
        Ok(())
    }
    pub(crate) fn read(&self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        self.tick()?;
        self.failed.set(true);
        let r = self.read_current_raw(path, cap);
        if r.is_ok() {
            self.failed.set(false);
        }
        r
    }
    // Shared actual object read body; outer operations own sticky failure state.
    fn read_current_raw(&self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        let m = self
            .raw_member(&RelativePath::parse(path).map_err(invalid)?)?
            .ok_or_else(|| invalid("read outside candidate membership"))?;
        if m.size_bytes > cap as u64 {
            return Err(invalid("candidate read exceeds bound"));
        }
        self.check_state(usize::try_from(m.size_bytes).map_err(invalid)?)?;
        self.reserve_logical(m.size_bytes, 0)?;
        let raw = self.store.read_object_accounted(
            m.sha256,
            m.size_bytes,
            cap,
            self.deadline,
            &self.cancelled,
            &|n| self.debit_read(n),
            &|_| Ok(()),
            &|n| self.ledger.record_read_returned(n).map_err(invalid),
            &|n| self.ledger.record_write_returned(n).map_err(invalid),
        )?;
        self.mark_read(path)?;
        self.running()?;
        Ok(raw)
    }
    /// Bound the real metadata/raw overlap BEFORE member allocation/read. The
    /// native caller additionally charges its own live parsed-row/kernel state.
    pub(crate) fn read_member_bound(
        &self,
        path: &RelativePath,
        cap: usize,
        max_owned_state_bytes: usize,
    ) -> io::Result<CandidateMemberRead> {
        self.tick()?;
        self.failed.set(true);
        let result = (|| {
            let peak = cap
                .checked_mul(4)
                .and_then(|n| n.checked_add(path.as_str().len().checked_mul(16)?))
                .and_then(|n| n.checked_add(8192))
                .ok_or_else(|| invalid("candidate bound member retained state overflow"))?;
            if cap == 0 || peak > max_owned_state_bytes {
                return Err(invalid("candidate bound member owned allowance"));
            }
            self.check_state(max_owned_state_bytes)?;
            let metadata = self
                .raw_member(path)?
                .ok_or_else(|| invalid("bound read outside candidate"))?;
            if metadata.size_bytes > cap as u64 {
                return Err(invalid("candidate bound member byte profile"));
            }
            let raw = self.read_current_raw(path.as_str(), cap)?;
            if raw.len() as u64 != metadata.size_bytes
                || Digest256::of_bytes(&raw) != metadata.sha256
            {
                return Err(invalid("candidate bound member raw fixity"));
            }
            self.running()?;
            Ok(CandidateMemberRead { raw, metadata })
        })();
        if result.is_ok() {
            self.failed.set(false);
        }
        result
    }
    pub(crate) fn verify(&self, path: &str) -> io::Result<()> {
        self.tick()?;
        self.failed.set(true);
        let r = (|| {
            let m = self
                .raw_member(&RelativePath::parse(path).map_err(invalid)?)?
                .ok_or_else(|| invalid("verify outside candidate membership"))?;
            self.reserve_logical(m.size_bytes, 0)?;
            self.store.verify_object_accounted(
                m.sha256,
                m.size_bytes,
                self.deadline,
                &self.cancelled,
                &|n| self.debit_read(n),
                &|_| Ok(()),
                &|n| self.ledger.record_read_returned(n).map_err(invalid),
                &|n| self.ledger.record_write_returned(n).map_err(invalid),
            )?;
            self.mark_read(path)?;
            self.running()
        })();
        if r.is_ok() {
            self.failed.set(false)
        }
        r
    }
    pub(crate) fn copy(&self, path: &str, sink: &mut dyn Write) -> io::Result<()> {
        self.tick()?;
        self.failed.set(true);
        let r = (|| {
            let m = self
                .raw_member(&RelativePath::parse(path).map_err(invalid)?)?
                .ok_or_else(|| invalid("copy outside candidate membership"))?;
            self.reserve_logical(m.size_bytes, 0)?;
            self.store.copy_object_accounted(
                m.sha256,
                m.size_bytes,
                sink,
                self.deadline,
                &self.cancelled,
                &|n| self.debit_read(n),
                &|n| self.debit_write(n),
                &|n| self.ledger.record_read_returned(n).map_err(invalid),
                &|n| self.ledger.record_write_returned(n).map_err(invalid),
            )?;
            self.mark_read(path)?;
            self.running()
        })();
        if r.is_ok() {
            self.failed.set(false)
        }
        r
    }
    fn retirement_at_raw_bounded(
        &self,
        i: u64,
        allowance: usize,
    ) -> io::Result<Option<RetirementMetadata>> {
        self.running()?;
        let row = self
            .db
            .query_row(
                "SELECT path,sha,event_ref,event_sha,event_size FROM retirements WHERE ordinal=?1",
                [i64::try_from(i).map_err(invalid)?],
                |r| {
                    let length = |value: rusqlite::types::ValueRef<'_>| match value {
                        rusqlite::types::ValueRef::Text(raw) => Ok(raw.len()),
                        _ => Err(rusqlite::Error::InvalidQuery),
                    };
                    if length(r.get_ref(0)?)?
                        .checked_add(length(r.get_ref(2)?)?)
                        .and_then(|n| n.checked_mul(16))
                        .and_then(|n| n.checked_add(2048))
                        .is_none_or(|n| n > allowance)
                    {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    for (column, width) in [(1, 32), (3, 32), (4, 8)] {
                        match r.get_ref(column)? {
                            rusqlite::types::ValueRef::Blob(raw) if raw.len() == width => (),
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        }
                    }
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, Vec<u8>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(sql)?;
        row.map(|(path, sha, event, event_sha, size)| {
            Ok(RetirementMetadata {
                path: RelativePath::parse(&path).map_err(invalid)?,
                sha256: Digest256::from_bytes(
                    sha.try_into()
                        .map_err(|_| invalid("retired digest width"))?,
                ),
                event_ref: RelativePath::parse(&event).map_err(invalid)?,
                event_sha256: Digest256::from_bytes(
                    event_sha
                        .try_into()
                        .map_err(|_| invalid("event digest width"))?,
                ),
                event_size_bytes: u64::from_be_bytes(
                    size.try_into().map_err(|_| invalid("event size width"))?,
                ),
            })
        })
        .transpose()
    }
    fn retirement_at_raw(&self, i: u64) -> io::Result<Option<RetirementMetadata>> {
        self.retirement_at_raw_bounded(i, self.row_state_ceiling.get())
    }
    pub(crate) fn retirement_at(&self, i: u64) -> io::Result<Option<RetirementMetadata>> {
        self.tick()?;
        self.finish_read(self.retirement_at_raw(i))
    }
    pub(crate) fn retirement_at_bounded(
        &self,
        i: u64,
        allowance: usize,
    ) -> io::Result<Option<RetirementMetadata>> {
        self.tick()?;
        let result = (|| {
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(size_of::<u64>())
                .ok_or_else(|| invalid("candidate retirement cursor state exceeds profile"))?;
            self.retirement_at_raw_bounded(i, row_allowance)
        })();
        self.finish_read(result)
    }
    pub(crate) fn retirement_count(&self) -> u64 {
        self.retirement_count
    }
    pub(crate) fn new_retirement_at(&self, i: u64) -> io::Result<Option<RetirementMetadata>> {
        self.tick()?;
        let result = (|| {
            self.retirement_at_raw(
                self.new_retirement_start
                    .checked_add(i)
                    .ok_or_else(|| invalid("retirement ordinal overflow"))?,
            )
        })();
        self.finish_read(result)
    }
    pub(crate) fn new_retirement_at_bounded(
        &self,
        i: u64,
        allowance: usize,
    ) -> io::Result<Option<RetirementMetadata>> {
        let ordinal = self
            .new_retirement_start
            .checked_add(i)
            .ok_or_else(|| invalid("retirement ordinal overflow"))?;
        self.retirement_at_bounded(ordinal, allowance)
    }
    pub(crate) fn new_retirement_count(&self) -> io::Result<u64> {
        self.retirement_count
            .checked_sub(self.new_retirement_start)
            .ok_or_else(|| invalid("candidate retirement range regressed"))
    }
    /// Current-base identity rows are copied into this bounded candidate's
    /// SQLite workspace during authenticated base import. The COW writer can
    /// merge them with the completed native identity cursor without spending
    /// the V2 read session's already-consumed tree-node allowance again.
    pub(crate) fn v2_base_identity_after(
        &self,
        after: Option<&str>,
        allowance: usize,
    ) -> io::Result<Option<(String, RelativePath)>> {
        self.tick()?;
        let result = (|| {
            let argument = cursor_argument_state(after.map_or(0, str::len))?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("V2 base identity cursor state exceeds profile"))?;
            let row = match after {
                Some(after) => self.db.query_row(
                    "SELECT id,path FROM v2_base_identities WHERE id>?1 ORDER BY id LIMIT 1",
                    [after],
                    |row| {
                        let id = bounded_text(row, row_allowance)?;
                        let path = match row.get_ref(1)? {
                            rusqlite::types::ValueRef::Text(bytes)
                                if bytes
                                    .len()
                                    .checked_mul(16)
                                    .and_then(|n| n.checked_add(2048))
                                    .is_some_and(|n| n <= row_allowance) =>
                            {
                                row.get::<_, String>(1)?
                            }
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        };
                        Ok((id, path))
                    },
                ),
                None => self.db.query_row(
                    "SELECT id,path FROM v2_base_identities ORDER BY id LIMIT 1",
                    [],
                    |row| {
                        let id = bounded_text(row, row_allowance)?;
                        let path = match row.get_ref(1)? {
                            rusqlite::types::ValueRef::Text(bytes)
                                if bytes
                                    .len()
                                    .checked_mul(16)
                                    .and_then(|n| n.checked_add(2048))
                                    .is_some_and(|n| n <= row_allowance) =>
                            {
                                row.get::<_, String>(1)?
                            }
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        };
                        Ok((id, path))
                    },
                ),
            }
            .optional()
            .map_err(sql)?;
            row.map(|(id, path)| Ok((id, RelativePath::parse(&path).map_err(invalid)?)))
                .transpose()
        })();
        self.finish_read(result)
    }
    /// Exact reverse lookup into the authenticated selected-base identities.
    /// The path index keeps a changed-member identity delta proportional to
    /// the identities attached to changed paths rather than rescanning all IDs.
    pub(crate) fn v2_base_identity_for_path_after(
        &self,
        path: &RelativePath,
        after_id: Option<&str>,
        allowance: usize,
    ) -> io::Result<Option<String>> {
        self.tick()?;
        let result = (|| {
            let argument_bytes = path
                .as_str()
                .len()
                .checked_add(after_id.map_or(0, str::len))
                .ok_or_else(|| invalid("V2 base identity cursor argument overflow"))?;
            let argument = cursor_argument_state(argument_bytes)?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("V2 base identity cursor exceeds profile"))?;
            match after_id {
                Some(after) => self.db.query_row(
                    "SELECT id FROM v2_base_identities INDEXED BY v2_base_identities_by_path WHERE path=?1 AND id>?2 ORDER BY path,id LIMIT 1",
                    params![path.as_str(), after],
                    |row| bounded_text(row, row_allowance),
                ),
                None => self.db.query_row(
                    "SELECT id FROM v2_base_identities INDEXED BY v2_base_identities_by_path WHERE path=?1 ORDER BY path,id LIMIT 1",
                    [path.as_str()],
                    |row| bounded_text(row, row_allowance),
                ),
            }
            .optional()
            .map_err(sql)
        })();
        self.finish_read(result)
    }
    /// Authenticated current-base dependency edges saved during import, in
    /// the exact bytewise key order consumed by V2 dependency COW updates.
    pub(crate) fn v2_base_dependency_after(
        &self,
        after: Option<&[u8]>,
        allowance: usize,
    ) -> io::Result<Option<(Vec<u8>, Vec<u8>)>> {
        self.tick()?;
        let result = (|| {
            let argument = cursor_argument_state(after.map_or(0, <[u8]>::len))?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("V2 base dependency cursor state exceeds profile"))?;
            let mut decode = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(Vec<u8>, Vec<u8>)> {
                let key = match row.get_ref(0)? {
                    rusqlite::types::ValueRef::Blob(bytes) if bytes.len() <= row_allowance => {
                        bytes.to_vec()
                    }
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let value = match row.get_ref(1)? {
                    rusqlite::types::ValueRef::Blob(bytes) if bytes.len() <= row_allowance => {
                        bytes.to_vec()
                    }
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                if key
                    .len()
                    .checked_add(value.len())
                    .is_none_or(|bytes| bytes > row_allowance)
                {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Ok((key, value))
            };
            match after {
                Some(after) => self.db.query_row(
                    "SELECT key,value FROM v2_base_dependencies WHERE key>?1 ORDER BY key LIMIT 1",
                    [after],
                    &mut decode,
                ),
                None => self.db.query_row(
                    "SELECT key,value FROM v2_base_dependencies ORDER BY key LIMIT 1",
                    [],
                    &mut decode,
                ),
            }
            .optional()
            .map_err(sql)
        })();
        self.finish_read(result)
    }
    pub(crate) fn base_identity_after(
        &self,
        after: Option<&str>,
    ) -> io::Result<Option<(String, RelativePath)>> {
        self.tick()?;
        let result = (|| {
            if let Some(base) = self.base {
                self.preflight_base_locator(base)?;
                return base
                    .identity_after(base.current_revision(), after)
                    .map_err(invalid);
            }
            if let Some(base) = self.base_v2 {
                let base = self.preflight_v2_base()?;
                let cursor_bytes = after.map_or(0, str::len);
                self.check_state(v2_row_state_upper_bound(cursor_bytes, 0)?)?;
                let revision = base.borrow().selected_revision();
                let row = base.borrow_mut().next_row_after(
                    revision,
                    V2RootKind::Identities,
                    None,
                    None,
                    after.map(str::as_bytes),
                )?;
                return row
                    .map(|row| {
                        self.check_state(v2_row_cursor_state_upper_bound(
                            row.key.len(),
                            row.value.len(),
                            cursor_bytes,
                        )?)?;
                        decode_v2_identity(&row.key, &row.value)
                    })
                    .transpose();
            }
            Ok(None)
        })();
        self.finish_read(result)
    }
    pub(crate) fn base_dependency_source_after(
        &self,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            if let Some(base) = self.base {
                self.preflight_base_locator(base)?;
                return base
                    .dependency_source_after(base.current_revision(), after)
                    .map_err(invalid);
            }
            if self.base_v2.is_some() {
                let argument = after.map_or(0, |path| path.as_str().len());
                self.check_state(v2_row_state_upper_bound(argument, 0)?)?;
                let row_allowance = self
                    .row_state_ceiling
                    .get()
                    .checked_sub(cursor_argument_state(argument)?)
                    .ok_or_else(|| invalid("V2 dependency source result exceeds profile"))?;
                let source: Option<String> = match after {
                    Some(after) => self.db.query_row(
                        "SELECT source FROM reverse_dependencies WHERE source>?1 ORDER BY source LIMIT 1",
                        [after.as_str()],
                        |row| bounded_text(row, row_allowance),
                    ),
                    None => self.db.query_row(
                        "SELECT source FROM reverse_dependencies ORDER BY source LIMIT 1",
                        [],
                        |row| bounded_text(row, row_allowance),
                    ),
                }
                .optional()
                .map_err(sql)?;
                return source
                    .map(|source| RelativePath::parse(&source).map_err(invalid))
                    .transpose();
            }
            Ok(None)
        })();
        self.finish_read(result)
    }
    pub(crate) fn base_dependency_after(
        &self,
        source: &RelativePath,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        if self.base_v2.is_some() {
            return self.base_dependency_target_after(source, after, self.row_state_ceiling.get());
        }
        let result = (|| {
            if let Some(base) = self.base {
                self.preflight_base_locator(base)?;
                return base
                    .dependency_after(base.current_revision(), source, after)
                    .map_err(invalid);
            }
            Ok(None)
        })();
        self.finish_read(result)
    }
    /// Unique historical identity ownership imported from every authenticated
    /// ancestor with its exact identity EOF. This is data, not completion.
    pub(crate) fn historical_identity_after(
        &self,
        after: Option<&str>,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<(String, RelativePath)>> {
        self.tick()?;
        let result = (|| {
            let input_bytes = after.map_or(0, str::len);
            if input_bytes > self.row_state_ceiling.get() {
                return Err(invalid("historical cursor input bound"));
            }
            let parameters = input_bytes
                .checked_mul(16)
                .and_then(|n| n.checked_add(1024))
                .ok_or_else(|| invalid("historical cursor parameter state"))?;
            self.check_state(parameters)?;
            let decode = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(String, RelativePath)> {
                let id = match row.get_ref(0)? {
                    rusqlite::types::ValueRef::Text(bytes) => bytes,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let path = match row.get_ref(1)? {
                    rusqlite::types::ValueRef::Text(bytes) => bytes,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let output = id
                    .len()
                    .checked_add(path.len())
                    .and_then(|n| n.checked_mul(16))
                    .and_then(|n| {
                        n.checked_add(size_of::<String>() + size_of::<RelativePath>() + 1024)
                    })
                    .filter(|n| *n <= max_owned_state_bytes)
                    .ok_or(rusqlite::Error::InvalidQuery)?;
                if id.len() > self.row_state_ceiling.get()
                    || path.len() > self.row_state_ceiling.get()
                    || std::str::from_utf8(id).is_err()
                    || std::str::from_utf8(path).is_err()
                {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                self.check_state(
                    parameters
                        .checked_add(output)
                        .ok_or(rusqlite::Error::InvalidQuery)?,
                )
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let id = std::str::from_utf8(id).map_err(|_| rusqlite::Error::InvalidQuery)?;
                let path = std::str::from_utf8(path).map_err(|_| rusqlite::Error::InvalidQuery)?;
                let path = RelativePath::parse(path).map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok((id.to_owned(), path))
            };
            let row = match after {
                None => self.db.query_row(
                    "SELECT id,path FROM historical_ids ORDER BY id LIMIT 1",
                    [],
                    decode,
                ),
                Some(after) => self.db.query_row(
                    "SELECT id,path FROM historical_ids WHERE id>?1 ORDER BY id LIMIT 1",
                    [after],
                    decode,
                ),
            }
            .optional()
            .map_err(sql)?;
            if row
                .as_ref()
                .is_some_and(|(id, _)| after.is_some_and(|after| id.as_str() <= after))
            {
                return Err(invalid("historical cursor order"));
            }
            Ok(row)
        })();
        self.finish_read(result)
    }
    pub(crate) fn historical_identity_path(&self, id: &str) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            let path: Option<String> = self
                .db
                .query_row("SELECT path FROM historical_ids WHERE id=?1", [id], |r| {
                    bounded_text(r, self.row_state_ceiling.get())
                })
                .optional()
                .map_err(sql)?;
            path.map(|path| RelativePath::parse(&path).map_err(invalid))
                .transpose()
        })();
        self.finish_read(result)
    }
    /// Reverse lookup the old identity rows owned by one changed member path.
    /// The dedicated SQLite order is finite and avoids rescanning all retained
    /// identities for every warm source update.
    pub(crate) fn historical_identity_for_path_after(
        &self,
        path: &RelativePath,
        after_id: Option<&str>,
        allowance: usize,
    ) -> io::Result<Option<String>> {
        self.tick()?;
        let result = (|| {
            let argument_bytes = path
                .as_str()
                .len()
                .checked_add(after_id.map_or(0, str::len))
                .ok_or_else(|| invalid("historical path cursor argument overflow"))?;
            let argument = cursor_argument_state(argument_bytes)?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("historical path cursor state exceeds profile"))?;
            match after_id {
                Some(after) => self.db.query_row(
                    "SELECT id FROM historical_ids WHERE path=?1 AND id>?2 ORDER BY id LIMIT 1",
                    params![path.as_str(), after],
                    |row| bounded_text(row, row_allowance),
                ),
                None => self.db.query_row(
                    "SELECT id FROM historical_ids WHERE path=?1 ORDER BY id LIMIT 1",
                    [path.as_str()],
                    |row| bounded_text(row, row_allowance),
                ),
            }
            .optional()
            .map_err(sql)
        })();
        self.finish_read(result)
    }
    /// Stream base dependency targets for one source path in binary order.
    /// The reverse-adjacency table has a source-order index for warm COW deltas.
    pub(crate) fn base_dependency_target_after(
        &self,
        source: &RelativePath,
        after_target: Option<&RelativePath>,
        allowance: usize,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            let argument_bytes = source
                .as_str()
                .len()
                .checked_add(after_target.map_or(0, |path| path.as_str().len()))
                .ok_or_else(|| invalid("base dependency cursor argument overflow"))?;
            let argument = cursor_argument_state(argument_bytes)?;
            self.check_state(allowance)?;
            let row_allowance = allowance
                .checked_sub(argument)
                .ok_or_else(|| invalid("base dependency cursor state exceeds profile"))?;
            let target = match after_target {
                Some(after) => self.db.query_row(
                    "SELECT target FROM reverse_dependencies WHERE source=?1 AND target>?2 ORDER BY target LIMIT 1",
                    params![source.as_str(), after.as_str()],
                    |row| bounded_text(row, row_allowance),
                ),
                None => self.db.query_row(
                    "SELECT target FROM reverse_dependencies WHERE source=?1 ORDER BY target LIMIT 1",
                    [source.as_str()],
                    |row| bounded_text(row, row_allowance),
                ),
            }
            .optional()
            .map_err(sql)?;
            target
                .map(|target| RelativePath::parse(&target).map_err(invalid))
                .transpose()
        })();
        self.finish_read(result)
    }
    pub(crate) fn base_identity_path(&self, id: &str) -> io::Result<Option<RelativePath>> {
        self.base_identity_path_bounded(id, self.row_state_ceiling.get())
    }
    /// Native global-index callback supplies its remaining owned-row allowance.
    /// The store must inspect borrowed SQL text BEFORE allocating a RelativePath.
    pub(crate) fn base_identity_path_bounded(
        &self,
        id: &str,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<RelativePath>> {
        self.tick()?;
        let result = (|| {
            if max_owned_state_bytes == 0 || max_owned_state_bytes > self.row_state_ceiling.get() {
                return Err(invalid("candidate base identity owned allowance"));
            }
            self.check_state(max_owned_state_bytes)?;
            if let Some(base) = self.base {
                self.preflight_base_locator(base)?;
            }
            if let Some(base) = self.base {
                return base
                    .identity_path_bounded(base.current_revision(), id, max_owned_state_bytes)
                    .map_err(invalid);
            }
            if let Some(base) = self.base_v2 {
                let base = self.preflight_v2_base()?;
                let mut reader = base.borrow_mut();
                let maximum = reader.identity_lookup_result_state_upper_bound()?;
                if maximum > max_owned_state_bytes {
                    return Err(invalid("V2 base identity result exceeds caller allowance"));
                }
                let revision = reader.selected_revision();
                return reader.identity_path(revision, id);
            }
            Ok(None)
        })();
        self.finish_read(result)
    }
    pub(crate) fn fence(&self) -> io::Result<CandidateFence> {
        self.tick()?;
        let (retirement_count, retirement_digest) = self.retirement_fence()?;
        let fence = CandidateFence {
            batch_sha256: self.batch.batch_sha256,
            base_revision: self.base_revision(),
            validator_sha256: self.validator_sha256(),
            membership: self.membership,
            source_bytes: self.source_bytes,
            retirement_count,
            retirement_digest,
        };
        self.tick()?;
        Ok(fence)
    }
    pub(crate) fn batch_sha256(&self) -> Digest256 {
        self.batch.batch_sha256
    }
    /// Exact native public ledger rows, length-framed in original retained order.
    /// This private candidate fence is not a source-store revision or authority.
    pub(crate) fn retirement_fence(&self) -> io::Result<(u64, Digest256)> {
        self.tick()?;
        self.failed.set(true);
        let result = (|| {
            let mut hash = Digest256Hasher::new();
            for ordinal in 0..self.retirement_count {
                let row = self
                    .retirement_at_raw(ordinal)?
                    .ok_or_else(|| invalid("retirement fence row absent"))?;
                self.check_state(
                    row.path
                        .as_str()
                        .len()
                        .checked_add(row.event_ref.as_str().len())
                        .and_then(|n| n.checked_mul(16))
                        .and_then(|n| n.checked_add(2048))
                        .ok_or_else(|| invalid("retirement fence state overflow"))?,
                )?;
                let value = serde_json::json!({"path":row.path.as_str(),"sha256":row.sha256.to_hex(),"event_ref":row.event_ref.as_str(),"event_sha256":row.event_sha256.to_hex(),"event_size_bytes":row.event_size_bytes});
                let raw = super::source_admission_candidate::canonical(
                    &value,
                    self.limits.candidate.reader.json,
                )?;
                hash.update(&(raw.len() as u64).to_be_bytes());
                hash.update(&raw);
            }
            self.running()?;
            Ok((self.retirement_count, hash.finalize()))
        })();
        if result.is_ok() {
            self.failed.set(false)
        }
        result
    }
    /// Same maintained changed/read member recheck before pointer publication.
    pub(crate) fn verify_consumed(&self) -> io::Result<()> {
        self.tick()?;
        self.failed.set(true);
        let result = (|| {
            let mut after = None;
            loop {
                self.running()?;
                let row=match &after {
                    Some(path)=>self.db.query_row("SELECT path,sha,size,mode FROM members WHERE (touched=1 OR changed=1) AND path>?1 ORDER BY path LIMIT 1",[path],|row|member_from_row(row,self.row_state_ceiling.get())),
                    None=>self.db.query_row("SELECT path,sha,size,mode FROM members WHERE touched=1 OR changed=1 ORDER BY path LIMIT 1",[],|row|member_from_row(row,self.row_state_ceiling.get()))
                }.optional().map_err(sql)?;
                let Some(row) = row else { break };
                let row = member(row)?;
                self.reserve_logical(row.size_bytes, 0)?;
                self.store.verify_object_accounted(
                    row.sha256,
                    row.size_bytes,
                    self.deadline,
                    &self.cancelled,
                    &|n| self.debit_read(n),
                    &|_| Ok(()),
                    &|n| self.ledger.record_read_returned(n).map_err(invalid),
                    &|n| self.ledger.record_write_returned(n).map_err(invalid),
                )?;
                after = Some(row.path.as_str().to_owned());
            }
            self.running()
        })();
        if result.is_ok() {
            self.failed.set(false)
        }
        result
    }
}
// Shared CMD membership framing for the retained V1 migration and candidate.
pub(crate) fn feed(hash: &mut Digest256Hasher, m: &MemberMetadata) {
    hash.update(&(m.path.as_str().len() as u64).to_be_bytes());
    hash.update(m.path.as_str().as_bytes());
    hash.update(&m.size_bytes.to_be_bytes());
    hash.update(m.sha256.as_bytes());
}

fn v2_row_state_upper_bound(key_bytes: usize, value_bytes: usize) -> io::Result<usize> {
    key_bytes
        .checked_add(value_bytes)
        .and_then(|bytes| bytes.checked_mul(16))
        .and_then(|bytes| bytes.checked_add(1024))
        .ok_or_else(|| invalid("V2 base row state overflow"))
}

fn v2_row_cursor_state_upper_bound(
    key_bytes: usize,
    value_bytes: usize,
    prior_cursor_bytes: usize,
) -> io::Result<usize> {
    let retained_key_bytes = key_bytes
        .checked_add(prior_cursor_bytes)
        .ok_or_else(|| invalid("V2 base row cursor state overflow"))?;
    v2_row_state_upper_bound(retained_key_bytes, value_bytes)
}

fn decode_v2_member(key: &[u8], value: &[u8]) -> io::Result<MemberMetadata> {
    if value.len() != 44 {
        return Err(invalid("V2 member tuple length differs"));
    }
    let path = std::str::from_utf8(key).map_err(invalid)?;
    let sha256 = Digest256::from_bytes(value[..32].try_into().map_err(invalid)?);
    let size_bytes = u64::from_be_bytes(value[32..40].try_into().map_err(invalid)?);
    let mode = u32::from_le_bytes(value[40..44].try_into().map_err(invalid)?);
    if !matches!(mode, 0o600 | 0o644 | 0o755) {
        return Err(invalid("V2 member source mode differs"));
    }
    Ok(MemberMetadata {
        path: RelativePath::parse(path).map_err(invalid)?,
        sha256,
        size_bytes,
        mode,
    })
}

fn decode_v2_identity(key: &[u8], value: &[u8]) -> io::Result<(String, RelativePath)> {
    let id = std::str::from_utf8(key).map_err(invalid)?;
    if id.is_empty() {
        return Err(invalid("V2 identity key is empty"));
    }
    let path = std::str::from_utf8(value).map_err(invalid)?;
    Ok((id.to_owned(), RelativePath::parse(path).map_err(invalid)?))
}

fn decode_v2_dependency(key: &[u8], value: &[u8]) -> io::Result<(RelativePath, RelativePath)> {
    let delimiter = key
        .iter()
        .position(|byte| *byte == 0)
        .filter(|index| *index > 0 && *index + 1 < key.len())
        .ok_or_else(|| invalid("V2 dependency key framing differs"))?;
    if value.len() < 8 {
        return Err(invalid("V2 dependency value is truncated"));
    }
    let source_len = u32::from_be_bytes(value[..4].try_into().map_err(invalid)?) as usize;
    let source_end = 4usize
        .checked_add(source_len)
        .ok_or_else(|| invalid("V2 dependency source length overflow"))?;
    let target_len_end = source_end
        .checked_add(4)
        .ok_or_else(|| invalid("V2 dependency target offset overflow"))?;
    let source_bytes = value
        .get(4..source_end)
        .ok_or_else(|| invalid("V2 dependency source is truncated"))?;
    let target_len_bytes = value
        .get(source_end..target_len_end)
        .ok_or_else(|| invalid("V2 dependency target length is truncated"))?;
    let target_len = u32::from_be_bytes(target_len_bytes.try_into().map_err(invalid)?) as usize;
    let target_end = target_len_end
        .checked_add(target_len)
        .ok_or_else(|| invalid("V2 dependency target length overflow"))?;
    let target_bytes = value
        .get(target_len_end..target_end)
        .filter(|_| target_end == value.len())
        .ok_or_else(|| invalid("V2 dependency target framing differs"))?;
    let key_source = &key[..delimiter];
    let key_target = &key[delimiter + 1..];
    if source_bytes != key_source || target_bytes != key_target {
        return Err(invalid("V2 dependency key/value binding differs"));
    }
    let source = std::str::from_utf8(source_bytes).map_err(invalid)?;
    let target = std::str::from_utf8(target_bytes).map_err(invalid)?;
    Ok((
        RelativePath::parse(source).map_err(invalid)?,
        RelativePath::parse(target).map_err(invalid)?,
    ))
}

fn take_v2_retirement_bytes<'a>(
    raw: &'a [u8],
    offset: &mut usize,
    length: usize,
) -> io::Result<&'a [u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid("V2 retirement offset overflow"))?;
    let bytes = raw
        .get(*offset..end)
        .ok_or_else(|| invalid("V2 retirement tuple is truncated"))?;
    *offset = end;
    Ok(bytes)
}

fn take_v2_retirement_path<'a>(raw: &'a [u8], offset: &mut usize) -> io::Result<&'a [u8]> {
    let length = u32::from_be_bytes(
        take_v2_retirement_bytes(raw, offset, 4)?
            .try_into()
            .map_err(invalid)?,
    ) as usize;
    take_v2_retirement_bytes(raw, offset, length)
}

fn decode_v2_retirement(
    key: &[u8],
    value: &[u8],
    max_row_state_bytes: usize,
) -> io::Result<(u64, RetirementMetadata)> {
    if key.len() != 8 || v2_row_state_upper_bound(key.len(), value.len())? > max_row_state_bytes {
        return Err(invalid("V2 retirement row exceeds selected profile"));
    }
    let ordinal = u64::from_be_bytes(key.try_into().map_err(invalid)?);
    let mut offset = 0usize;
    let path =
        std::str::from_utf8(take_v2_retirement_path(value, &mut offset)?).map_err(invalid)?;
    let sha256 = Digest256::from_bytes(
        take_v2_retirement_bytes(value, &mut offset, 32)?
            .try_into()
            .map_err(invalid)?,
    );
    let event_ref =
        std::str::from_utf8(take_v2_retirement_path(value, &mut offset)?).map_err(invalid)?;
    let event_sha256 = Digest256::from_bytes(
        take_v2_retirement_bytes(value, &mut offset, 32)?
            .try_into()
            .map_err(invalid)?,
    );
    let event_size_bytes = u64::from_be_bytes(
        take_v2_retirement_bytes(value, &mut offset, 8)?
            .try_into()
            .map_err(invalid)?,
    );
    if offset != value.len() {
        return Err(invalid("V2 retirement tuple has trailing bytes"));
    }
    Ok((
        ordinal,
        RetirementMetadata {
            path: RelativePath::parse(path).map_err(invalid)?,
            sha256,
            event_ref: RelativePath::parse(event_ref).map_err(invalid)?,
            event_sha256,
            event_size_bytes,
        },
    ))
}
