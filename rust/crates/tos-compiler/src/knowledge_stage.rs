//! Private, disk-indexed full-knowledge staging. Source admission and the
//! filesystem spill quota are supplied by independent owner/host guards.

use crate::{
    Error, Limits, Result, SourceBinding, file_digest, safe_open, sqlite_budget, stream_digest,
};
use fs2::FileExt;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    any::Any,
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, SourceRevision};
use tos_source_store::{
    CorpusCutReader, MetadataPublicationEpoch, SourceMembershipV1, StreamedCorpusCutReaderV1,
};

// A selected descriptor permits up to 4,096 source registrations. A full
// source family can contribute several independently sealed collections.
const MAX_COLLECTIONS: usize = 16_384;
const MAX_NAME_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WritePhase {
    Create,
    SqliteOpen,
    Schema,
    Input,
    Normalized,
    Catalog,
    Search,
    Sort,
    Finalize,
}

/// The host implementation must hold an exclusive quota-backed private
/// temp/output namespace for this stage's entire lifetime, including SQLite's
/// fallback directories and rollback files. No other writer may replace a
/// stage path or write the retained inodes. `verify` must reject absent or
/// exhausted kernel-enforced quotas; a before/after size sample alone cannot
/// cap a single SQLite statement's spill. There is no permissive implementation.
pub trait StageIsolation {
    fn verify(&self, candidate: &Path, limits: StageLimits, phase: WritePhase) -> Result<()>;
}

/// A source-owner implementation checks the selected registration and retains
/// the immutable sealed cut until the final recheck. Stage hashes alone do not
/// establish source completeness or admission.
pub trait StageOwner {
    fn verify_receipt(&self, receipt: &ExactInputReceipt) -> Result<()>;
    fn recheck_sealed_cut(&self, receipt: &ExactInputReceipt) -> Result<()>;
}

#[derive(Clone, Copy, Debug)]
pub struct StageLimits {
    pub sqlite: Limits,
    pub max_temp_bytes: u64,
    pub max_seek_rows: usize,
    pub max_seek_bytes: u64,
}
impl StageLimits {
    pub(crate) fn validate(self) -> Result<()> {
        self.sqlite.validate()?;
        if self.max_temp_bytes == 0
            || self.max_seek_rows == 0
            || self.max_seek_rows > MAX_STAGE_PAGE_ROWS
            || self.max_seek_bytes == 0
            || self.max_seek_bytes > MAX_STAGE_PAGE_BYTES
        {
            return Err(Error::Budget("stage temp/seek limits must be positive"));
        }
        Ok(())
    }
}

// Existing stage page ceilings; callers still pass their narrower actual
// normalization/finalization page limits to with_write_page.
const MAX_STAGE_PAGE_ROWS: usize = 1024;
const MAX_STAGE_PAGE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputCollectionReceipt {
    pub source_graph: String,
    pub collection: String,
    pub input_role: String,
    pub adapter_profile: String,
    pub expected_count: u64,
    /// SHA-256 over sorted (length-prefixed ID, binary payload SHA-256) pairs.
    pub expected_root_sha256: String,
}

#[derive(Clone, Debug)]
pub struct ExactInputReceipt {
    pub binding: SourceBinding,
    pub collections: Vec<InputCollectionReceipt>,
}
impl ExactInputReceipt {
    pub(crate) fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        validate_input_collections(&self.collections)?;
        Ok(())
    }
}

fn validate_input_collections(collections: &[InputCollectionReceipt]) -> Result<()> {
    if collections.is_empty() || collections.len() > MAX_COLLECTIONS {
        return Err(Error::Invalid("input collection registration count"));
    }
    let mut seen = BTreeSet::new();
    for entry in collections {
        for value in [
            &entry.source_graph,
            &entry.collection,
            &entry.input_role,
            &entry.adapter_profile,
        ] {
            if value.is_empty() || value.len() > MAX_NAME_BYTES {
                return Err(Error::Invalid("input collection registration field"));
            }
        }
        Digest256::from_hex(&entry.expected_root_sha256)
            .map_err(|_| Error::Invalid("input collection root digest"))?;
        if !seen.insert((&entry.source_graph, &entry.collection)) {
            return Err(Error::Invalid("duplicate input collection registration"));
        }
    }
    Ok(())
}

/// A cold authored manifest is a source carrier, never a projected index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColdAuthoredBinding {
    revision: SourceRevision,
    membership: SourceMembershipV1,
    source_cut: String,
    epoch_token: Option<String>,
    epoch_generation: u64,
    epoch_member: Option<(Digest256, u64)>,
}
impl ColdAuthoredBinding {
    pub fn from_cut(
        cut: &CorpusCutReader,
        revision: SourceRevision,
        membership: SourceMembershipV1,
        epoch: &MetadataPublicationEpoch,
    ) -> Result<Self> {
        if cut.current().revision() != revision
            || cut
                .stream(revision)
                .map_err(|_| Error::Source("cold authored membership custody refused".into()))?
                .expectation()
                != membership
        {
            return Err(Error::Invalid("cold authored independently selected cut"));
        }
        Ok(Self {
            revision,
            membership,
            source_cut: revision.0.to_hex(),
            epoch_token: epoch.token().map(str::to_owned),
            epoch_generation: epoch.generation(),
            epoch_member: epoch.member_binding().map_err(|_| {
                Error::Source("cold authored metadata epoch binding refused".into())
            })?,
        })
    }
    /// Bind an independently selected revision to the authenticated disk index.
    /// Full stream EOF/currentness remains the owner's final-stage obligation.
    pub fn from_streamed_cut(
        cut: &StreamedCorpusCutReaderV1,
        revision: SourceRevision,
        membership: SourceMembershipV1,
        epoch: &MetadataPublicationEpoch,
    ) -> Result<Self> {
        let selected = cut
            .revision(revision)
            .map_err(|_| Error::Source("cold authored streamed custody refused".into()))?
            .ok_or(Error::Invalid("cold authored streamed revision absent"))?;
        if cut.current_revision() != revision || selected.membership != membership {
            return Err(Error::Invalid(
                "cold authored independently selected streamed cut",
            ));
        }
        Ok(Self {
            revision,
            membership,
            source_cut: revision.0.to_hex(),
            epoch_token: epoch.token().map(str::to_owned),
            epoch_generation: epoch.generation(),
            epoch_member: epoch.member_binding().map_err(|_| {
                Error::Source("cold authored metadata epoch binding refused".into())
            })?,
        })
    }
    pub fn revision(&self) -> SourceRevision {
        self.revision
    }
    pub fn membership(&self) -> SourceMembershipV1 {
        self.membership
    }
    pub fn source_cut(&self) -> &str {
        &self.source_cut
    }
    pub fn epoch_token(&self) -> Option<&str> {
        self.epoch_token.as_deref()
    }
    pub fn epoch_generation(&self) -> u64 {
        self.epoch_generation
    }
    pub fn epoch_member(&self) -> Option<(Digest256, u64)> {
        self.epoch_member
    }
    pub fn value(&self) -> serde_json::Value {
        serde_json::json!({"kind":"cold-authored-manifest-v1", "source_revision":self.revision.0.to_hex(),
            "manifest_body_sha256":self.revision.0.to_hex(), "membership_count":self.membership.count,
            "membership_sha256":self.membership.digest.to_hex(), "epoch_token":self.epoch_token,
            "epoch_generation":self.epoch_generation,
            "epoch_member":self.epoch_member.map(|(sha,bytes)|serde_json::json!({"sha256":sha.to_hex(),"bytes":bytes}))})
    }
}
#[derive(Clone, Debug)]
pub struct ColdExactInputReceipt {
    pub binding: ColdAuthoredBinding,
    pub collections: Vec<InputCollectionReceipt>,
}
impl ColdExactInputReceipt {
    fn validate(&self) -> Result<()> {
        validate_input_collections(&self.collections)
    }
}
pub trait ColdStageOwner {
    fn verify_receipt(&self, receipt: &ColdExactInputReceipt) -> Result<()>;
    fn recheck_sealed_cut(&self, receipt: &ColdExactInputReceipt) -> Result<()>;
}
/// Actual candidate input identity and adapter-reported complete EOF coverage.
/// This carrier creates no revision, publication epoch or source admission.
#[derive(Clone)]
pub struct CandidateValidationBinding<I: Copy + Eq + 'static> {
    identity: I,
    coverage: tos_validation::record_biblio_cut::SourceCutInputCoverage,
}
impl<I: Copy + Eq + 'static> CandidateValidationBinding<I> {
    pub fn from_verified_input(
        input: &dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<I>,
        expected: &I,
        coverage: tos_validation::record_biblio_cut::SourceCutInputCoverage,
        deadline: Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        let checkpoint = || {
            if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
                Err(Error::Budget("candidate input deadline/cancel"))
            } else {
                Ok(())
            }
        };
        checkpoint()?;
        if input.input_identity() != expected {
            return Err(Error::Invalid("candidate actual input identity"));
        }
        if coverage.member_count() != coverage.membership().count {
            return Err(Error::Invalid("candidate complete input coverage"));
        }
        input
            .source_input()
            .verify_current_fence(&coverage, deadline, cancelled)
            .map_err(|e| Error::Source(format!("candidate current input fence:{e:?}")))?;
        checkpoint()?;
        if input.input_identity() != expected {
            return Err(Error::Invalid("candidate actual input identity changed"));
        }
        Ok(Self {
            identity: *input.input_identity(),
            coverage,
        })
    }
    pub fn input_identity(&self) -> &I {
        &self.identity
    }
    pub fn coverage(&self) -> &tos_validation::record_biblio_cut::SourceCutInputCoverage {
        &self.coverage
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.coverage.member_count() != self.coverage.membership().count {
            return Err(Error::Invalid("candidate complete input coverage"));
        }
        Ok(())
    }
    /// Diagnostic coverage only. The opaque identity is compared in memory;
    /// these bytes are never an identity token or membership admission.
    pub(crate) fn value(&self) -> serde_json::Value {
        serde_json::json!({"kind":"candidate-validation-input-v1",
            "membership_count":self.coverage.member_count(),
            "membership_sha256":self.coverage.membership().digest.to_hex(),
            "source_bytes_read":self.coverage.source_bytes_read()})
    }
    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.identity == other.identity && self.coverage == other.coverage
    }
}
#[derive(Clone)]
pub struct CandidateExactInputReceipt<I: Copy + Eq + 'static> {
    pub binding: CandidateValidationBinding<I>,
    pub collections: Vec<InputCollectionReceipt>,
}
impl<I: Copy + Eq + 'static> CandidateExactInputReceipt<I> {
    fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        validate_input_collections(&self.collections)
    }
}
/// The actual adapter retains its source fence through computational completion.
pub trait CandidateStageOwner<I: Copy + Eq + 'static> {
    fn verify_receipt(&self, receipt: &CandidateExactInputReceipt<I>) -> Result<()>;
    fn recheck_current_input(&self, receipt: &CandidateExactInputReceipt<I>) -> Result<()>;
}
#[derive(Clone)]
struct ErasedCandidateReceipt {
    actual: Rc<dyn Any>,
    validate: fn(&dyn Any) -> Result<()>,
    collections: fn(&dyn Any) -> &[InputCollectionReceipt],
}
impl ErasedCandidateReceipt {
    fn new<I: Copy + Eq + 'static>(receipt: CandidateExactInputReceipt<I>) -> Self {
        Self {
            actual: Rc::new(receipt),
            validate: |r| {
                r.downcast_ref::<CandidateExactInputReceipt<I>>()
                    .ok_or(Error::Invalid("candidate receipt type"))?
                    .validate()
            },
            collections: |r| {
                &r.downcast_ref::<CandidateExactInputReceipt<I>>()
                    .expect("private candidate receipt type")
                    .collections
            },
        }
    }
    fn typed<I: Copy + Eq + 'static>(&self) -> Result<&CandidateExactInputReceipt<I>> {
        self.actual
            .downcast_ref()
            .ok_or(Error::Invalid("candidate receipt identity type"))
    }
}
trait ErasedCandidateOwner {
    fn verify(&self, receipt: &ErasedCandidateReceipt) -> Result<()>;
    fn recheck(&self, receipt: &ErasedCandidateReceipt) -> Result<()>;
}
struct CandidateOwnerAdapter<'a, I: Copy + Eq + 'static>(&'a dyn CandidateStageOwner<I>);
impl<I: Copy + Eq + 'static> ErasedCandidateOwner for CandidateOwnerAdapter<'_, I> {
    fn verify(&self, receipt: &ErasedCandidateReceipt) -> Result<()> {
        self.0.verify_receipt(receipt.typed::<I>()?)
    }
    fn recheck(&self, receipt: &ErasedCandidateReceipt) -> Result<()> {
        self.0.recheck_current_input(receipt.typed::<I>()?)
    }
}
#[derive(Clone)]
enum StageInputReceipt {
    Projection(ExactInputReceipt),
    Cold(ColdExactInputReceipt),
    Candidate(ErasedCandidateReceipt),
}
impl StageInputReceipt {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Projection(r) => r.validate(),
            Self::Cold(r) => r.validate(),
            Self::Candidate(r) => (r.validate)(r.actual.as_ref()),
        }
    }
    fn collections(&self) -> &[InputCollectionReceipt] {
        match self {
            Self::Projection(r) => &r.collections,
            Self::Cold(r) => &r.collections,
            Self::Candidate(r) => (r.collections)(r.actual.as_ref()),
        }
    }
}
enum StageInputOwner<'a> {
    Projection(&'a dyn StageOwner),
    Cold(&'a dyn ColdStageOwner),
    Candidate(Box<dyn ErasedCandidateOwner + 'a>),
}
impl StageInputOwner<'_> {
    fn verify_receipt(&self, receipt: &StageInputReceipt) -> Result<()> {
        match (self, receipt) {
            (Self::Projection(o), StageInputReceipt::Projection(r)) => o.verify_receipt(r),
            (Self::Cold(o), StageInputReceipt::Cold(r)) => o.verify_receipt(r),
            (Self::Candidate(o), StageInputReceipt::Candidate(r)) => o.verify(r),
            _ => Err(Error::Invalid("stage input owner kind")),
        }
    }
    fn recheck_sealed_cut(&self, receipt: &StageInputReceipt) -> Result<()> {
        match (self, receipt) {
            (Self::Projection(o), StageInputReceipt::Projection(r)) => o.recheck_sealed_cut(r),
            (Self::Cold(o), StageInputReceipt::Cold(r)) => o.recheck_sealed_cut(r),
            (Self::Candidate(o), StageInputReceipt::Candidate(r)) => o.recheck(r),
            _ => Err(Error::Invalid("stage input owner kind")),
        }
    }
}
#[derive(Clone, Debug)]
pub struct ColdStageReceipt {
    pub binding: ColdAuthoredBinding,
    pub verified_inputs: Vec<InputCollectionReceipt>,
    pub input_rows: u64,
}

pub struct InputRow<'a> {
    pub source_graph: &'a str,
    pub collection: &'a str,
    pub id: &'a str,
    pub payload: &'a [u8],
}
pub struct NodeRow<'a> {
    pub id: &'a str,
    pub source_graph: &'a str,
    pub native_id: Option<&'a str>,
    pub entity_id: Option<&'a str>,
    pub kind_id: &'a str,
    pub type_id: &'a str,
    pub source_order: i64,
    pub payload: &'a [u8],
}
pub struct RelationRow<'a> {
    pub id: &'a str,
    pub source_graph: &'a str,
    pub native_id: Option<&'a str>,
    pub from_id: &'a str,
    pub to_id: &'a str,
    pub predicate_id: &'a str,
    pub relation_type_id: &'a str,
    pub source_order: i64,
    pub payload: &'a [u8],
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageReceipt {
    pub binding: SourceBinding,
    pub source_cut: String,
    pub membership_root: String,
    pub input_collections: usize,
    pub verified_inputs: Vec<InputCollectionReceipt>,
    pub input_rows: u64,
    pub node_rows: u64,
    pub relation_rows: u64,
    pub node_root_sha256: String,
    pub relation_root_sha256: String,
    pub sqlite_sha256: String,
    pub sqlite_size_bytes: u64,
}

/// Provisional derived row roots for the full-model seal. `finish` repeats
/// these scans after its owner cut recheck; this is not source admission.
#[derive(Clone, Debug)]
pub(crate) struct CoreRoots {
    pub nodes: u64,
    pub relations: u64,
    pub node_sha256: String,
    pub relation_sha256: String,
}

#[derive(Clone, Debug)]
pub struct SeekRow {
    pub id: String,
    pub source_graph: String,
    pub source_order: Option<i64>,
    pub payload: Vec<u8>,
    pub payload_sha256: String,
}

#[derive(Clone, Debug)]
pub struct ScanPage {
    pub rows: Vec<SeekRow>,
    /// Pass this as `after_id` for the next page. `None` means exhausted.
    pub next_id: Option<String>,
}

/// Budget ledger used by public operations. Native capture and its borrowed
/// query callback share an atomic ledger so the same held capture can be lent
/// across a Send disclosure lease and by disposable public D1 staging.
#[derive(Clone)]
enum PublicWorkLedger {
    Shared {
        used: Arc<AtomicU64>,
        cancelled: Arc<AtomicBool>,
    },
}

pub struct KnowledgeStage<'a> {
    candidate: PathBuf,
    inode: (u64, u64),
    lease_path: PathBuf,
    lease_inode: (u64, u64),
    lease: Option<fs::File>,
    db: Option<Connection>,
    vm_used: Option<Arc<AtomicU64>>,
    limits: StageLimits,
    raw_input_max_bytes: usize,
    receipt: StageInputReceipt,
    registrations: BTreeMap<String, BTreeSet<String>>,
    owner: StageInputOwner<'a>,
    isolation: Option<&'a dyn StageIsolation>,
    public_build: bool,
    public_deadline: Option<Instant>,
    total_rows: u64,
    work_bytes: u64,
    public_work: Option<(PublicWorkLedger, u64)>,
    raw_read_budget: Option<(Cell<u64>, u64, Cell<bool>)>,
    poisoned: bool,
    write_page: Option<WritePageCharge>,
    keep: bool,
    selected_full: bool,
    closed_input_rows: Option<u64>,
    fresh_selected: Option<PathBuf>,
}

struct WritePageCharge {
    rows: u64,
    bytes: u64,
    max_rows: u64,
    max_bytes: u64,
}

impl<'a> KnowledgeStage<'a> {
    pub(crate) fn public_build(&self) -> bool {
        self.public_build
    }
    pub(crate) fn registered_source(&self, source_graph: &str) -> bool {
        if matches!(&self.receipt, StageInputReceipt::Candidate(_)) {
            return self
                .receipt
                .collections()
                .iter()
                .any(|entry| entry.source_graph == source_graph);
        }
        self.registrations.contains_key(source_graph)
    }

    fn registered(&self, source_graph: &str, collection: &str) -> bool {
        if matches!(&self.receipt, StageInputReceipt::Candidate(_)) {
            return self
                .receipt
                .collections()
                .iter()
                .any(|entry| entry.source_graph == source_graph && entry.collection == collection);
        }
        self.registrations
            .get(source_graph)
            .is_some_and(|collections| collections.contains(collection))
    }

    pub(crate) fn exact_receipt(&self) -> Result<&ExactInputReceipt> {
        match &self.receipt {
            StageInputReceipt::Projection(r) => Ok(r),
            StageInputReceipt::Cold(_) => Err(Error::Invalid(
                "cold authored stage is not projection input",
            )),
            StageInputReceipt::Candidate(_) => {
                Err(Error::Invalid("candidate stage is not projection input"))
            }
        }
    }
    pub(crate) fn cold_receipt(&self) -> Result<&ColdExactInputReceipt> {
        match &self.receipt {
            StageInputReceipt::Cold(r) => Ok(r),
            StageInputReceipt::Projection(_) => Err(Error::Invalid(
                "projection stage is not cold authored input",
            )),
            StageInputReceipt::Candidate(_) => {
                Err(Error::Invalid("candidate stage is not cold authored input"))
            }
        }
    }
    pub(crate) fn input_collections(&self) -> &[InputCollectionReceipt] {
        self.receipt.collections()
    }
    pub fn candidate_receipt<I: Copy + Eq + 'static>(
        &self,
    ) -> Result<&CandidateExactInputReceipt<I>> {
        if self.poisoned {
            return Err(Error::Invalid("candidate stage poisoned"));
        }
        match &self.receipt {
            StageInputReceipt::Candidate(r) => r.typed::<I>(),
            _ => Err(Error::Invalid("stage is not candidate validation input")),
        }
    }
    pub(crate) fn input_source_cut(&self) -> Result<&str> {
        match &self.receipt {
            StageInputReceipt::Projection(r) => Ok(&r.binding.source_cut),
            StageInputReceipt::Cold(r) => Ok(r.binding.source_cut()),
            StageInputReceipt::Candidate(_) => {
                Err(Error::Invalid("candidate input has no source cut"))
            }
        }
    }

    pub(crate) fn poison(&mut self) {
        self.poisoned = true;
    }
    pub(crate) fn mark_selected_full(&mut self) -> Result<()> {
        let result = if self.public_build
            || self.poisoned
            || self.write_page.is_some()
            || self.selected_full
        {
            Err(Error::Invalid("selected full-model stage state"))
        } else {
            self.selected_full = true;
            Ok(())
        };
        self.poisoned |= result.is_err();
        result
    }

    pub fn create(
        candidate: &Path,
        limits: StageLimits,
        receipt: ExactInputReceipt,
        owner: &'a dyn StageOwner,
        isolation: &'a dyn StageIsolation,
    ) -> Result<Self> {
        Self::create_inner(
            candidate,
            limits,
            StageInputReceipt::Projection(receipt),
            StageInputOwner::Projection(owner),
            Some(isolation),
            None,
            None,
            None,
        )
    }

    pub fn create_cold(
        candidate: &Path,
        limits: StageLimits,
        receipt: ColdExactInputReceipt,
        owner: &'a dyn ColdStageOwner,
        isolation: &'a dyn StageIsolation,
    ) -> Result<Self> {
        Self::create_inner(
            candidate,
            limits,
            StageInputReceipt::Cold(receipt),
            StageInputOwner::Cold(owner),
            Some(isolation),
            None,
            None,
            None,
        )
    }

    /// Bind the existing connection-wide VM counter and progress callback to
    /// the caller's absolute operation deadline. Cold input remains a cold
    /// receipt; no public-build or projection authority is selected here.
    pub fn create_cold_until(
        candidate: &Path,
        limits: StageLimits,
        receipt: ColdExactInputReceipt,
        owner: &'a dyn ColdStageOwner,
        isolation: &'a dyn StageIsolation,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_cold_until_with_input_cap(
            candidate,
            limits,
            receipt,
            owner,
            isolation,
            deadline,
            limits.sqlite.max_row_bytes,
        )
    }
    /// Select an authenticated cold raw-input ceiling independently of the
    /// unchanged normalized/projection row ceiling. All cumulative counters,
    /// isolation and the original absolute deadline remain shared.
    pub fn create_cold_until_with_input_cap(
        candidate: &Path,
        limits: StageLimits,
        receipt: ColdExactInputReceipt,
        owner: &'a dyn ColdStageOwner,
        isolation: &'a dyn StageIsolation,
        deadline: Instant,
        max_input_bytes: usize,
    ) -> Result<Self> {
        if max_input_bytes == 0
            || max_input_bytes as u64 > MAX_STAGE_PAGE_BYTES
            || max_input_bytes as u128 > limits.sqlite.max_work_bytes as u128
        {
            return Err(Error::Budget("cold stage raw input ceiling"));
        }
        if Instant::now() >= deadline {
            return Err(Error::Budget("cold stage deadline"));
        }
        let mut stage = Self::create_inner(
            candidate,
            limits,
            StageInputReceipt::Cold(receipt),
            StageInputOwner::Cold(owner),
            Some(isolation),
            Some(Arc::new(AtomicU64::new(0))),
            None,
            Some(deadline),
        )?;
        stage.raw_input_max_bytes = max_input_bytes;
        Ok(stage)
    }

    /// Private computational candidate staging with the same quota, deadline,
    /// counters and raw input ceiling. No cold or projection binding is minted.
    pub fn create_candidate_until_with_input_cap<I: Copy + Eq + 'static>(
        candidate: &Path,
        limits: StageLimits,
        receipt: CandidateExactInputReceipt<I>,
        owner: &'a dyn CandidateStageOwner<I>,
        isolation: &'a dyn StageIsolation,
        deadline: Instant,
        max_input_bytes: usize,
    ) -> Result<Self> {
        if max_input_bytes == 0
            || max_input_bytes as u64 > MAX_STAGE_PAGE_BYTES
            || max_input_bytes as u128 > limits.sqlite.max_work_bytes as u128
        {
            return Err(Error::Budget("candidate stage raw input ceiling"));
        }
        if Instant::now() >= deadline {
            return Err(Error::Budget("candidate stage deadline"));
        }
        let mut stage = Self::create_inner(
            candidate,
            limits,
            StageInputReceipt::Candidate(ErasedCandidateReceipt::new(receipt)),
            StageInputOwner::Candidate(Box::new(CandidateOwnerAdapter(owner))),
            Some(isolation),
            Some(Arc::new(AtomicU64::new(0))),
            None,
            Some(deadline),
        )?;
        stage.raw_input_max_bytes = max_input_bytes;
        Ok(stage)
    }

    /// Check the actual candidate owner without repeating the input-row census.
    pub(crate) fn recheck_candidate_owner<I: Copy + Eq + 'static>(&mut self) -> Result<()> {
        let result = (|| {
            self.candidate_receipt::<I>()?;
            self.check(WritePhase::Finalize)?;
            self.owner.recheck_sealed_cut(&self.receipt)?;
            self.check(WritePhase::Finalize)
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// Recheck this computational candidate's actual owner fence and the
    /// existing complete input collection roots. No output receipt is issued.
    pub fn verify_candidate_inputs<I: Copy + Eq + 'static>(&mut self) -> Result<u64> {
        let result = (|| {
            self.candidate_receipt::<I>()?;
            if self.write_page.is_some() || !self.db().is_autocommit() {
                return Err(Error::Invalid("candidate stage pending write"));
            }
            self.check(WritePhase::Finalize)?;
            self.owner.recheck_sealed_cut(&self.receipt)?;
            let rows = self.verified_input_rows()?;
            self.recheck_candidate_owner::<I>()?;
            Ok(rows)
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// Complete a computational cold candidate. No selected projection or SQLite
    /// export receipt is emitted; disposal retains existing private-stage cleanup.
    pub fn finish_cold(mut self) -> Result<ColdStageReceipt> {
        let result = (|| {
            self.cold_receipt()?;
            if self.poisoned || self.write_page.is_some() || !self.db().is_autocommit() {
                return Err(Error::Invalid("cold stage poisoned or pending write"));
            }
            self.owner.recheck_sealed_cut(&self.receipt)?;
            let rows = self.verified_input_rows()?;
            self.check(WritePhase::Finalize)?;
            let receipt = self.cold_receipt()?;
            Ok(ColdStageReceipt {
                binding: receipt.binding.clone(),
                verified_inputs: receipt.collections.clone(),
                input_rows: rows,
            })
        })();
        self.poisoned |= result.is_err();
        result
    }

    pub(crate) fn create_captured_native_snapshot(
        candidate: &Path,
        limits: StageLimits,
        receipt: ExactInputReceipt,
        owner: &'a dyn StageOwner,
        isolation: &'a dyn StageIsolation,
        vm_used: Arc<AtomicU64>,
        work_used: Arc<AtomicU64>,
        cancelled: Arc<AtomicBool>,
        max_work_bytes: u64,
        deadline: Instant,
    ) -> Result<Self> {
        if receipt.binding.owner_profile != "tos-native-projection-snapshot-v1" {
            return Err(Error::Invalid("native snapshot stage profile"));
        }
        Self::create_inner(
            candidate,
            limits,
            StageInputReceipt::Projection(receipt),
            StageInputOwner::Projection(owner),
            Some(isolation),
            Some(vm_used),
            Some((
                PublicWorkLedger::Shared {
                    used: work_used,
                    cancelled,
                },
                max_work_bytes,
            )),
            Some(deadline),
        )
    }

    /// Disposable public-output staging. Its local inode/lease and SQLite
    /// limits are not a kernel aggregate-spill quota or selected admission.
    /// Only the compiler's full public D1 builder may invoke this entry.
    pub(crate) fn create_public_build(
        candidate: &Path,
        limits: StageLimits,
        receipt: ExactInputReceipt,
        owner: &'a dyn StageOwner,
        vm_used: Arc<AtomicU64>,
        work_used: Arc<AtomicU64>,
        cancelled: Arc<AtomicBool>,
        max_work_bytes: u64,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_inner(
            candidate,
            limits,
            StageInputReceipt::Projection(receipt),
            StageInputOwner::Projection(owner),
            None,
            Some(vm_used),
            Some((
                PublicWorkLedger::Shared {
                    used: work_used,
                    cancelled,
                },
                max_work_bytes,
            )),
            Some(deadline),
        )
    }

    fn create_inner(
        candidate: &Path,
        limits: StageLimits,
        receipt: StageInputReceipt,
        owner: StageInputOwner<'a>,
        isolation: Option<&'a dyn StageIsolation>,
        shared_vm_used: Option<Arc<AtomicU64>>,
        public_work: Option<(PublicWorkLedger, u64)>,
        public_deadline: Option<Instant>,
    ) -> Result<Self> {
        limits.validate()?;
        receipt.validate()?;
        let mut registrations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        // Candidate registrations borrow the already receipted collection
        // strings; do not create an independently unpriced map/string clone.
        if !matches!(&receipt, StageInputReceipt::Candidate(_)) {
            for entry in receipt.collections() {
                registrations
                    .entry(entry.source_graph.clone())
                    .or_default()
                    .insert(entry.collection.clone());
            }
        }
        owner.verify_receipt(&receipt)?;
        let parent = candidate
            .parent()
            .ok_or(Error::Invalid("stage candidate parent"))?;
        if !parent.is_dir() || parent.is_symlink() || candidate.exists() || candidate.is_symlink() {
            return Err(Error::Invalid("stage candidate path"));
        }
        if sqlite_sidecar_paths(candidate)
            .iter()
            .any(|path| path.exists() || path.is_symlink())
        {
            return Err(Error::Invalid("stage SQLite sidecar path exists"));
        }
        let fresh = fresh_selected_path(candidate);
        if fresh.exists() || fresh.is_symlink() {
            return Err(Error::Invalid("stage fresh selected path exists"));
        }
        let lease_path = lease_path(candidate)?;
        if lease_path.exists() || lease_path.is_symlink() {
            return Err(Error::Invalid("stage lease exists"));
        }
        if let Some(isolation) = isolation {
            isolation.verify(candidate, limits, WritePhase::Create)?;
        }
        // Receipt and host verification may consume the remaining time. Do
        // not create a lease or candidate after that selected deadline.
        if public_deadline.is_some_and(|limit| Instant::now() >= limit) {
            return Err(Error::Budget(if isolation.is_none() {
                "public D1 build deadline"
            } else {
                "cold stage deadline"
            }));
        }
        let mut lease = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lease_path)?;
        if lease.try_lock_exclusive().is_err() {
            let _ = fs::remove_file(&lease_path);
            return Err(Error::Invalid("stage lease busy"));
        }
        let lease_metadata = lease.metadata()?;
        let file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(candidate)
        {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_file(&lease_path);
                return Err(Error::Io(error));
            }
        };
        let metadata = file.metadata()?;
        drop(file);
        let mut stage = Self {
            candidate: candidate.to_owned(),
            inode: (metadata.dev(), metadata.ino()),
            lease_path,
            lease_inode: (lease_metadata.dev(), lease_metadata.ino()),
            lease: Some(lease),
            db: None,
            vm_used: None,
            limits,
            receipt,
            registrations,
            owner,
            isolation,
            raw_input_max_bytes: limits.sqlite.max_row_bytes,
            public_build: isolation.is_none(),
            public_deadline,
            total_rows: 0,
            work_bytes: 0,
            public_work,
            raw_read_budget: None,
            poisoned: false,
            write_page: None,
            keep: false,
            selected_full: false,
            closed_input_rows: None,
            fresh_selected: None,
        };
        let lease = stage.lease.as_mut().expect("stage lease open");
        write!(
            lease,
            "tos-knowledge-stage-v1 {} {}\n",
            metadata.dev(),
            metadata.ino()
        )?;
        lease.sync_all()?;
        fs::File::open(parent)?.sync_all()?;
        stage.check(WritePhase::SqliteOpen)?;
        let db = Connection::open(candidate)?;
        stage.db = Some(db);
        stage.vm_used = Some(if let Some(used) = shared_vm_used {
            sqlite_budget::configure_with_counter_until(
                stage.db(),
                limits.sqlite,
                Arc::clone(&used),
                public_deadline.ok_or(Error::Invalid("public D1 deadline absent"))?,
            )?;
            used
        } else {
            sqlite_budget::configure(stage.db(), limits.sqlite)?
        });
        // This must precede every TEMP page allocation, including reading its
        // page geometry for the disposable public-build page cap.
        configure_stage_temp_reclamation(stage.db())?;
        if stage.public_build {
            let page_size: u64 = stage
                .db()
                .query_row("PRAGMA temp.page_size", [], |row| row.get(0))?;
            if page_size == 0 {
                return Err(Error::Invalid("public D1 TEMP page size"));
            }
            let pages = limits.max_temp_bytes / page_size;
            if pages == 0 || pages > i64::MAX as u64 {
                return Err(Error::Budget("public D1 TEMP page cap"));
            }
            let applied: i64 = stage.db().query_row(
                &format!("PRAGMA temp.max_page_count={pages}"),
                [],
                |row| row.get(0),
            )?;
            if applied <= 0 || applied as u64 > pages {
                return Err(Error::Invalid("public D1 TEMP page cap unavailable"));
            }
        }
        stage.check(WritePhase::Schema)?;
        stage.db().execute_batch(SCHEMA)?;
        stage.check(WritePhase::Schema)?;
        Ok(stage)
    }

    fn db(&self) -> &Connection {
        self.db.as_ref().expect("stage database open")
    }
    fn check(&self, phase: WritePhase) -> Result<()> {
        if self
            .public_deadline
            .is_some_and(|limit| Instant::now() >= limit)
        {
            return Err(Error::Budget(if self.public_build {
                "public D1 build deadline"
            } else {
                "cold stage deadline"
            }));
        }
        Self::check_isolation(
            self.isolation,
            &self.candidate,
            self.inode,
            &self.lease_path,
            self.lease_inode,
            self.limits,
            phase,
        )
    }
    fn check_isolation(
        isolation: Option<&dyn StageIsolation>,
        candidate: &Path,
        inode: (u64, u64),
        lease_path: &Path,
        lease_inode: (u64, u64),
        limits: StageLimits,
        phase: WritePhase,
    ) -> Result<()> {
        let file = fs::symlink_metadata(candidate)?;
        let lease = fs::symlink_metadata(lease_path)?;
        if !file.file_type().is_file()
            || (file.dev(), file.ino()) != inode
            || !lease.file_type().is_file()
            || (lease.dev(), lease.ino()) != lease_inode
        {
            return Err(Error::Invalid("stage private inode/lease changed"));
        }
        if let Some(isolation) = isolation {
            isolation.verify(candidate, limits, phase)?;
        }
        Ok(())
    }
    /// A bounded producer may add catalog/search tables and indexed joins to
    /// this private database. The caller must keep its own row/byte budgets;
    /// the stage holds SQLite VM/page/cache limits. Admitted native stages also
    /// retain their host isolation guard; the disposable public-build profile
    /// has no host quota and does not claim aggregate disk-spill isolation.
    /// Any callback or quota failure poisons the stage, so `finish` refuses it.
    pub(crate) fn with_connection<T>(
        &mut self,
        phase: WritePhase,
        f: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        if self.poisoned {
            return Err(Error::Invalid("stage poisoned by prior failure"));
        }
        let mut result = (|| {
            self.check(phase)?;
            let value =
                f(self.db.as_mut().expect("stage database open")).map_err(|error| match error {
                    Error::Sql(error) => Error::SqlitePhase { phase, error },
                    other => other,
                })?;
            self.check(phase)?;
            Ok(value)
        })();
        // Attribute a budget refusal only when this operation's actual
        // cumulative VM counter reached its configured guard. No reason-text
        // match, counter reset, new query, or post-failure cleanup can hide it.
        let used_steps = self
            .vm_used
            .as_ref()
            .map_or(0, |used| used.load(Ordering::Relaxed));
        let max_steps = sqlite_budget::effective_vm_cap(self.limits.sqlite);
        if matches!(&result, Err(Error::Budget(_))) && used_steps >= max_steps {
            result = Err(Error::SqliteVmBudget {
                phase,
                used_steps,
                max_steps,
            });
        }
        self.poisoned |= result.is_err();
        result
    }

    /// A document transaction keeps the same bounded isolation-check cadence
    /// while borrowing the connection. Attribution and poison stay in the
    /// existing with_connection path; the callback cannot replace its guard.
    pub(crate) fn with_connection_checks<T>(
        &mut self,
        phase: WritePhase,
        f: impl FnOnce(&mut Connection, &dyn Fn() -> Result<()>) -> Result<T>,
    ) -> Result<T> {
        let isolation = self.isolation;
        let candidate = self.candidate.clone();
        let inode = self.inode;
        let lease_path = self.lease_path.clone();
        let lease_inode = self.lease_inode;
        let limits = self.limits;
        self.with_connection(phase, |db| {
            f(db, &|| {
                Self::check_isolation(
                    isolation,
                    &candidate,
                    inode,
                    &lease_path,
                    lease_inode,
                    limits,
                    phase,
                )
            })
        })
    }

    /// One already-bounded normalization/finalization page. The closure uses
    /// the same Stage methods and connection, so its reads see earlier writes
    /// in this page. Previous committed pages remain independent. A caller
    /// may not commit an ignored row error: every Stage failure poisons it.
    pub(crate) fn with_write_page<T>(
        &mut self,
        phase: WritePhase,
        max_rows: usize,
        max_bytes: u64,
        f: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let result = (|| {
            if self.poisoned || self.write_page.is_some() {
                return Err(Error::Invalid("stage write page unavailable"));
            }
            if !matches!(phase, WritePhase::Normalized | WritePhase::Finalize) {
                return Err(Error::Invalid("stage write page phase"));
            }
            if max_rows == 0
                || max_rows > MAX_STAGE_PAGE_ROWS
                || max_bytes == 0
                || max_bytes > MAX_STAGE_PAGE_BYTES
            {
                return Err(Error::Budget("stage write page bounds"));
            }
            self.check(phase)?;
            if !self.db().is_autocommit() {
                return Err(Error::Invalid("stage write page nested transaction"));
            }
            self.db()
                .execute_batch("BEGIN IMMEDIATE")
                .map_err(|error| Error::SqlitePhase { phase, error })?;
            self.write_page = Some(WritePageCharge {
                rows: 0,
                bytes: 0,
                max_rows: max_rows as u64,
                max_bytes,
            });
            let page = (|| {
                let value = f(self)?;
                if self.poisoned || self.db().is_autocommit() {
                    return Err(Error::Invalid("stage write page failed or closed"));
                }
                self.check(phase)?;
                Ok(value)
            })();
            let value = match page {
                Ok(value) => value,
                Err(error) => {
                    // SQLite may already have aborted this transaction. Cleanup
                    // must not replace the failure that poisoned this page.
                    if !self.db().is_autocommit() {
                        let _ = self.db().execute_batch("ROLLBACK");
                    }
                    self.write_page = None;
                    return Err(error);
                }
            };
            if let Err(error) = self.db().execute_batch("COMMIT") {
                if !self.db().is_autocommit() {
                    let _ = self.db().execute_batch("ROLLBACK");
                }
                self.write_page = None;
                return Err(Error::SqlitePhase { phase, error });
            }
            self.write_page = None;
            self.check(phase)?;
            Ok(value)
        })();
        self.poisoned |= result.is_err();
        result
    }

    fn require_open_inputs(&self) -> Result<()> {
        if self.poisoned {
            return Err(Error::Invalid("stage poisoned by prior failure"));
        }
        if self.closed_input_rows.is_some() {
            return Err(Error::Invalid("stage original input already closed"));
        }
        Ok(())
    }

    fn verified_input_rows(&self) -> Result<u64> {
        self.check(WritePhase::Sort)?;
        let mut input_rows = 0u64;
        for entry in self.receipt.collections() {
            let (count, root) = input_root(self.db(), entry)?;
            self.check(WritePhase::Sort)?;
            if count != entry.expected_count || root != entry.expected_root_sha256 {
                return Err(Error::Invalid("input collection count/root mismatch"));
            }
            input_rows = input_rows
                .checked_add(count)
                .ok_or(Error::Budget("input rows"))?;
        }
        Ok(input_rows)
    }

    /// End the actual normalization/original-capture phase. Only the private
    /// verified count is retained; roots remain in the immutable exact receipt.
    /// Full-component writers cannot re-open ingestion after this transition.
    pub(crate) fn close_inputs_for_full_components(&mut self) -> Result<()> {
        let result = (|| {
            self.require_open_inputs()?;
            if self.write_page.is_some() {
                return Err(Error::Invalid("stage close input inside write page"));
            }
            self.owner.recheck_sealed_cut(&self.receipt)?;
            let rows = self.verified_input_rows()?;
            self.with_connection(WritePhase::Finalize, |db| {
                db.execute_batch("PRAGMA secure_delete=ON; DROP TABLE raw_records")?;
                Ok(())
            })?;
            self.closed_input_rows = Some(rows);
            Ok(())
        })();
        self.poisoned |= result.is_err();
        result
    }

    pub(crate) fn core_roots(&mut self) -> Result<CoreRoots> {
        self.with_connection(WritePhase::Sort, |db| {
            let (nodes, node_sha256) = output_root(db, "knowledge_nodes")?;
            let (relations, relation_sha256) = output_root(db, "knowledge_relations")?;
            Ok(CoreRoots {
                nodes,
                relations,
                node_sha256,
                relation_sha256,
            })
        })
    }

    /// Disposable-public completion checks the same private transaction and
    /// custody state without returning a StageReceipt or permitting selection.
    pub(crate) fn complete_public_build(&mut self, nodes: u64, relations: u64) -> Result<()> {
        let result = (|| {
            if !self.public_build
                || self.poisoned
                || self.write_page.is_some()
                || self.selected_full
                || self.closed_input_rows.is_none()
                || !self.db().is_autocommit()
            {
                return Err(Error::Invalid("public D1 stage completion state"));
            }
            self.owner.recheck_sealed_cut(&self.receipt)?;
            self.with_connection(WritePhase::Finalize, |db| {
                let actual_nodes: u64 = db.query_row("SELECT count(*) FROM knowledge_nodes",[],|row|row.get(0))?;
                let actual_relations: u64 = db.query_row("SELECT count(*) FROM knowledge_relations",[],|row|row.get(0))?;
                if actual_nodes!=nodes || actual_relations!=relations {
                    return Err(Error::Invalid("public D1 completed row counts"));
                }
                let dangling:Option<i64>=db.query_row("SELECT 1 FROM knowledge_relations r WHERE NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.from_id) OR NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.to_id) LIMIT 1",[],|row|row.get(0)).optional()?;
                if dangling.is_some(){return Err(Error::Invalid("public D1 relation endpoint absent"));}
                Ok(())
            })
        })();
        self.poisoned |= result.is_err();
        result
    }
    pub(crate) fn charge(&mut self, payload: &[u8]) -> Result<()> {
        self.charge_with_row_cap(payload, self.limits.sqlite.max_row_bytes)
    }
    fn charge_raw_input(&mut self, payload: &[u8]) -> Result<()> {
        self.charge_with_row_cap(payload, self.raw_input_max_bytes)
    }
    fn charge_with_row_cap(&mut self, payload: &[u8], max_row_bytes: usize) -> Result<()> {
        let result = (|| {
            if payload.len() > max_row_bytes {
                return Err(Error::Budget("stage row bytes"));
            }
            self.total_rows = self
                .total_rows
                .checked_add(1)
                .ok_or(Error::Budget("stage rows"))?;
            if self.total_rows > self.limits.sqlite.max_rows {
                return Err(Error::Budget("stage rows"));
            }
            self.work_bytes = self
                .work_bytes
                .checked_add(payload.len() as u64)
                .ok_or(Error::Budget("stage work bytes"))?;
            if self.work_bytes > self.limits.sqlite.max_work_bytes {
                return Err(Error::Budget("stage work bytes"));
            }
            self.charge_public_work(payload.len() as u64)?;
            self.charge_write_page(1, payload.len() as u64)?;
            Ok(())
        })();
        self.poisoned |= result.is_err();
        result
    }
    fn charge_write_page(&mut self, rows: u64, bytes: u64) -> Result<()> {
        if let Some(page) = self.write_page.as_mut() {
            page.rows = page
                .rows
                .checked_add(rows)
                .ok_or(Error::Budget("stage write page rows"))?;
            page.bytes = page
                .bytes
                .checked_add(bytes)
                .ok_or(Error::Budget("stage write page bytes"))?;
            if page.rows > page.max_rows || page.bytes > page.max_bytes {
                return Err(Error::Budget("stage write page rows/bytes"));
            }
        }
        Ok(())
    }
    /// Charge a disk-backed external-sort copy before the SQL statement that
    /// writes final rows. A later failure poisons the stage and removes it.
    pub(crate) fn charge_materialized(&mut self, rows: u64, bytes: u64) -> Result<()> {
        let result = (|| {
            self.total_rows = self
                .total_rows
                .checked_add(rows)
                .ok_or(Error::Budget("stage rows"))?;
            self.work_bytes = self
                .work_bytes
                .checked_add(bytes)
                .ok_or(Error::Budget("stage work bytes"))?;
            if self.total_rows > self.limits.sqlite.max_rows
                || self.work_bytes > self.limits.sqlite.max_work_bytes
            {
                return Err(Error::Budget("stage materialized rows/work bytes"));
            }
            self.charge_public_work(bytes)?;
            self.charge_write_page(rows, bytes)?;
            Ok(())
        })();
        self.poisoned |= result.is_err();
        result
    }
    fn charge_public_work(&self, bytes: u64) -> Result<()> {
        self.check_public_work_active()?;
        if let Some((used, limit)) = &self.public_work {
            match used {
                PublicWorkLedger::Shared { used, .. } => {
                    let mut current = used.load(Ordering::Acquire);
                    loop {
                        self.check_public_work_active()?;
                        let next = current
                            .checked_add(bytes)
                            .filter(|next| *next <= *limit)
                            .ok_or(Error::Budget("public D1 build work bytes"))?;
                        match used.compare_exchange_weak(
                            current,
                            next,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => break,
                            Err(observed) => current = observed,
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn check_public_work_active(&self) -> Result<()> {
        let deadline_expired = self
            .public_deadline
            .is_some_and(|limit| Instant::now() >= limit);
        let cancelled = self.public_work.as_ref().is_some_and(|(ledger, _)| {
            matches!(
                ledger,
                PublicWorkLedger::Shared { cancelled, .. }
                    if cancelled.load(Ordering::Acquire)
            )
        });
        if deadline_expired || cancelled {
            return Err(Error::Budget(if self.public_build {
                "public D1 build deadline"
            } else {
                "cold stage deadline/cancel"
            }));
        }
        Ok(())
    }
    pub fn ingest_input(&mut self, row: InputRow<'_>) -> Result<()> {
        let result = self.ingest_input_inner(row);
        self.poisoned |= result.is_err();
        result
    }
    /// The existing row/byte ceilings for an atomic raw-input chunk.
    pub(crate) fn input_batch_limits(&self) -> (usize, u64) {
        (self.limits.max_seek_rows, self.limits.max_seek_bytes)
    }
    /// The existing validated private-stage page ceiling, distinct from the
    /// caller's narrower input seek limit and its actual output page budget.
    pub(crate) fn write_page_limits(&self) -> (usize, u64) {
        (MAX_STAGE_PAGE_ROWS, MAX_STAGE_PAGE_BYTES)
    }
    /// Atomic finite input chunk using the existing seek row/byte ceilings.
    /// Borrowed original rows are not copied or granted new source custody.
    /// Failure poisons the private stage and rolls back the current chunk.
    pub fn ingest_input_batch(&mut self, rows: &[InputRow<'_>]) -> Result<()> {
        let result = (|| {
            self.require_open_inputs()?;
            if self.write_page.is_some() {
                return Err(Error::Invalid("stage input batch inside write page"));
            }
            if rows.is_empty() || rows.len() > self.limits.max_seek_rows {
                return Err(Error::Budget("stage input chunk rows"));
            }
            let mut bytes = 0usize;
            for row in rows {
                bytes = bytes
                    .checked_add(row.payload.len())
                    .filter(|n| *n as u64 <= self.limits.max_seek_bytes)
                    .ok_or(Error::Budget("stage input chunk bytes"))?;
                if !self.registered(row.source_graph, row.collection) {
                    return Err(Error::Invalid("unregistered input collection"));
                }
                valid_id(row.id)?;
                self.charge_raw_input(row.payload)?;
            }
            self.check(WritePhase::Input)?;
            let tx = self
                .db
                .as_mut()
                .expect("stage database open")
                .transaction()?;
            for row in rows {
                let digest = Digest256::of_bytes(row.payload);
                tx.execute(
                    "INSERT INTO raw_records VALUES (?1,?2,?3,?4,?5,?6)",
                    params![
                        row.source_graph,
                        row.collection,
                        row.id,
                        row.payload.len() as i64,
                        digest.as_bytes().as_slice(),
                        row.payload
                    ],
                )?;
            }
            tx.commit()?;
            self.check(WritePhase::Input)
        })();
        self.poisoned |= result.is_err();
        result
    }
    fn ingest_input_inner(&mut self, row: InputRow<'_>) -> Result<()> {
        self.require_open_inputs()?;
        if self.write_page.is_some() {
            return Err(Error::Invalid("stage input inside write page"));
        }
        if !self.registered(row.source_graph, row.collection) {
            return Err(Error::Invalid("unregistered input collection"));
        }
        valid_id(row.id)?;
        self.charge_raw_input(row.payload)?;
        self.check(WritePhase::Input)?;
        let digest = Digest256::of_bytes(row.payload);
        self.db().execute(
            "INSERT INTO raw_records VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                row.source_graph,
                row.collection,
                row.id,
                row.payload.len() as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        self.check(WritePhase::Input)?;
        Ok(())
    }
    pub fn insert_node(&mut self, row: NodeRow<'_>) -> Result<()> {
        let result = self.insert_node_inner(row);
        self.poisoned |= result.is_err();
        result
    }
    fn insert_node_inner(&mut self, row: NodeRow<'_>) -> Result<()> {
        if !self.registered_source(row.source_graph) {
            return Err(Error::Invalid("unregistered node source"));
        }
        for value in [row.id, row.kind_id, row.type_id] {
            valid_id(value)?;
        }
        for value in [row.native_id, row.entity_id].into_iter().flatten() {
            valid_id(value)?;
        }
        if row.source_order < 0 {
            return Err(Error::Invalid("negative source order"));
        }
        self.charge(row.payload)?;
        self.check(WritePhase::Normalized)?;
        let digest = Digest256::of_bytes(row.payload);
        self.db().execute(
            "INSERT INTO knowledge_nodes VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                row.id,
                row.source_graph,
                row.native_id,
                row.entity_id,
                row.kind_id,
                row.type_id,
                row.source_order,
                row.payload.len() as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        self.check(WritePhase::Normalized)?;
        Ok(())
    }
    pub fn insert_relation(&mut self, row: RelationRow<'_>) -> Result<()> {
        let result = self.insert_relation_inner(row);
        self.poisoned |= result.is_err();
        result
    }
    fn insert_relation_inner(&mut self, row: RelationRow<'_>) -> Result<()> {
        if !self.registered_source(row.source_graph) {
            return Err(Error::Invalid("unregistered relation source"));
        }
        for value in [
            row.id,
            row.from_id,
            row.to_id,
            row.predicate_id,
            row.relation_type_id,
        ] {
            valid_id(value)?;
        }
        if let Some(native_id) = row.native_id {
            valid_id(native_id)?;
        }
        if row.source_order < 0 {
            return Err(Error::Invalid("negative source order"));
        }
        self.charge(row.payload)?;
        self.check(WritePhase::Normalized)?;
        let digest = Digest256::of_bytes(row.payload);
        self.db().execute(
            "INSERT INTO knowledge_relations VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                row.id,
                row.source_graph,
                row.native_id,
                row.from_id,
                row.to_id,
                row.predicate_id,
                row.relation_type_id,
                row.source_order,
                row.payload.len() as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        self.check(WritePhase::Normalized)?;
        Ok(())
    }

    /// Borrow the existing stage under a cumulative raw-input read ceiling.
    /// Charges happen before payload copies; ignored read refusals still poison
    /// the observation. The original SQLite, deadline and isolation guards stay.
    pub fn with_raw_input_read_budget<T>(
        &mut self,
        max_bytes: u64,
        observe: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if self.poisoned || self.raw_read_budget.is_some() || max_bytes == u64::MAX {
            return Err(Error::Invalid("stage raw observation budget unavailable"));
        }
        self.check(WritePhase::Catalog)?;
        self.raw_read_budget = Some((Cell::new(0), max_bytes, Cell::new(false)));
        let mut result = observe(self);
        let failed = self
            .raw_read_budget
            .take()
            .map_or(true, |(_, _, failed)| failed.get());
        if failed {
            result = Err(Error::Budget("stage raw observation read bytes"));
        }
        if result.is_ok() {
            result = self.check(WritePhase::Catalog).and(result);
        }
        self.poisoned |= result.is_err();
        result
    }

    fn charge_raw_observation_read(&self, bytes: u64) -> Result<()> {
        if let Some((used, cap, failed)) = &self.raw_read_budget {
            if let Err(error) = self.check(WritePhase::Catalog) {
                failed.set(true);
                return Err(error);
            }
            if failed.get() {
                return Err(Error::Budget("stage raw observation read bytes"));
            }
            match used.get().checked_add(bytes).filter(|next| *next <= *cap) {
                Some(next) => used.set(next),
                None => {
                    failed.set(true);
                    return Err(Error::Budget("stage raw observation read bytes"));
                }
            }
        }
        Ok(())
    }

    /// Indexed exact ID seek. One row is transferred only after its actual
    /// length predicate passes; returned bytes and digest are verified.
    pub fn raw_by_id(
        &self,
        source_graph: &str,
        collection: &str,
        id: &str,
    ) -> Result<Option<SeekRow>> {
        self.require_open_inputs()?;
        if !self.registered(source_graph, collection) {
            return Err(Error::Invalid("unregistered input collection"));
        }
        valid_id(id)?;
        if self.raw_read_budget.is_some() {
            let length = self.db().query_row(
                "SELECT payload_len,length(payload) FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3",
                params![source_graph, collection, id],
                |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?)),
            ).optional()?;
            if let Some((declared, actual)) = length {
                if declared != actual || actual > self.raw_input_max_bytes as u64 {
                    return Err(Error::Invalid("stage raw observation input length"));
                }
                self.charge_raw_observation_read(actual)?;
            }
        }
        let row = self
            .db()
            .query_row(
                "SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records
             WHERE source_graph=?1 AND collection=?2 AND id=?3
               AND length(payload)<=?4 AND payload_len=length(payload)",
                params![
                    source_graph,
                    collection,
                    id,
                    self.raw_input_max_bytes as i64
                ],
                read_seek_row,
            )
            .optional()?;
        row.map(|row| verify_seek_row(row, self.raw_input_max_bytes))
            .transpose()
    }

    /// Ordered raw input page using the `(source_graph,collection,id)` primary
    /// index. The page is bounded by both row and total payload bytes, so the
    /// caller can normalize it after the immutable stage borrow ends.
    pub fn scan_input(
        &self,
        source_graph: &str,
        collection: &str,
        after_id: Option<&str>,
        max_rows: usize,
    ) -> Result<ScanPage> {
        self.require_open_inputs()?;
        if !self.registered(source_graph, collection) {
            return Err(Error::Invalid("unregistered input collection"));
        }
        if let Some(id) = after_id {
            valid_id(id)?;
        }
        if max_rows == 0 || max_rows > self.limits.max_seek_rows {
            return Err(Error::Budget("stage seek rows"));
        }
        let sql = if after_id.is_some() {
            "SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records
             WHERE source_graph=?1 AND collection=?2 AND id>?3
               AND length(payload)<=?4 AND payload_len=length(payload)
             ORDER BY id LIMIT ?5"
        } else {
            "SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records
             WHERE source_graph=?1 AND collection=?2
               AND length(payload)<=?3 AND payload_len=length(payload)
             ORDER BY id LIMIT ?4"
        };
        let mut statement = self.db().prepare(sql)?;
        let lookahead = (max_rows + 1) as i64;
        let mut rows = if let Some(id) = after_id {
            statement.query(params![
                source_graph,
                collection,
                id,
                self.raw_input_max_bytes as i64,
                lookahead
            ])?
        } else {
            statement.query(params![
                source_graph,
                collection,
                self.raw_input_max_bytes as i64,
                lookahead
            ])?
        };
        let mut page = Vec::new();
        let mut bytes = 0u64;
        let mut has_more = false;
        while let Some(row) = rows.next()? {
            if page.len() == max_rows {
                has_more = true;
                break;
            }
            if self.raw_read_budget.is_some() {
                let bytes = match row.get_ref(3)? {
                    rusqlite::types::ValueRef::Blob(raw) => raw.len() as u64,
                    _ => return Err(Error::Invalid("stage raw observation input blob")),
                };
                self.charge_raw_observation_read(bytes)?;
            }
            let item = verify_seek_row(read_seek_row(row)?, self.raw_input_max_bytes)?;
            let next_bytes = bytes
                .checked_add(item.payload.len() as u64)
                .ok_or(Error::Budget("stage seek bytes"))?;
            if next_bytes > self.limits.max_seek_bytes {
                if page.is_empty() {
                    return Err(Error::Budget("stage seek bytes"));
                }
                has_more = true;
                break;
            }
            bytes = next_bytes;
            page.push(item);
        }
        let next_id = if has_more {
            page.last().map(|row| row.id.clone())
        } else {
            None
        };
        Ok(ScanPage {
            rows: page,
            next_id,
        })
    }

    /// Bounded index walk in deterministic `(source_order,id)` order.
    pub fn outgoing(
        &self,
        from_id: &str,
        after_order: i64,
        max_rows: usize,
        sink: &mut dyn FnMut(SeekRow) -> Result<()>,
    ) -> Result<usize> {
        valid_id(from_id)?;
        self.seek_indexed(
            "SELECT id,source_graph,source_order,payload,payload_sha256 FROM knowledge_relations
              WHERE from_id=?1 AND source_order>?2 AND length(payload)<=?3
                AND payload_len=length(payload)
              ORDER BY source_order,id LIMIT ?4",
            from_id,
            after_order,
            max_rows,
            sink,
        )
    }
    pub fn incoming(
        &self,
        to_id: &str,
        after_order: i64,
        max_rows: usize,
        sink: &mut dyn FnMut(SeekRow) -> Result<()>,
    ) -> Result<usize> {
        valid_id(to_id)?;
        self.seek_indexed(
            "SELECT id,source_graph,source_order,payload,payload_sha256 FROM knowledge_relations
              WHERE to_id=?1 AND source_order>?2 AND length(payload)<=?3
                AND payload_len=length(payload)
              ORDER BY source_order,id LIMIT ?4",
            to_id,
            after_order,
            max_rows,
            sink,
        )
    }
    fn seek_indexed(
        &self,
        sql: &str,
        endpoint: &str,
        after_order: i64,
        max_rows: usize,
        sink: &mut dyn FnMut(SeekRow) -> Result<()>,
    ) -> Result<usize> {
        if max_rows == 0 || max_rows > self.limits.max_seek_rows {
            return Err(Error::Budget("stage seek rows"));
        }
        let mut statement = self.db().prepare(sql)?;
        let mut rows = statement.query(params![
            endpoint,
            after_order,
            self.limits.sqlite.max_row_bytes as i64,
            max_rows as i64
        ])?;
        let mut count = 0usize;
        let mut bytes = 0u64;
        while let Some(row) = rows.next()? {
            let item = verify_seek_row(read_seek_row(row)?, self.limits.sqlite.max_row_bytes)?;
            bytes = bytes
                .checked_add(item.payload.len() as u64)
                .ok_or(Error::Budget("stage seek bytes"))?;
            if bytes > self.limits.max_seek_bytes {
                return Err(Error::Budget("stage seek bytes"));
            }
            sink(item)?;
            count += 1;
        }
        Ok(count)
    }

    pub fn finish(mut self) -> Result<StageReceipt> {
        self.exact_receipt()?;
        if self.public_build {
            return Err(Error::Invalid("public D1 stage has no selected finish"));
        }
        if self.poisoned || self.write_page.is_some() || !self.db().is_autocommit() {
            return Err(Error::Invalid("stage poisoned by prior failed row"));
        }
        self.owner.recheck_sealed_cut(&self.receipt)?;
        let input_rows = match self.closed_input_rows {
            Some(rows) => {
                self.check(WritePhase::Sort)?;
                rows
            }
            None => self.verified_input_rows()?,
        };
        let (node_rows, node_root) = output_root(self.db(), "knowledge_nodes")?;
        self.check(WritePhase::Sort)?;
        let (relation_rows, relation_root) = output_root(self.db(), "knowledge_relations")?;
        self.check(WritePhase::Sort)?;
        let dangling: Option<String> = self
            .db()
            .query_row(
                "SELECT r.id FROM knowledge_relations r
             WHERE NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.from_id)
                OR NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.to_id)
             LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if dangling.is_some() {
            return Err(Error::Invalid("stage relation endpoint absent"));
        }
        self.check(WritePhase::Sort)?;
        let mut selected_file = None;
        if self.selected_full {
            crate::knowledge_navigation_original::verify_stage(&mut self, None)?;
            crate::knowledge_philosophy_original::verify_stage(&mut self, None)?;
            crate::knowledge_corpus_original::verify_stage(&mut self, None)?;
            self.check(WritePhase::Finalize)?;
            preflight_selected_vacuum(self.db(), &self.candidate, self.inode, self.limits)?;
            // Owner input is removed from the private stage only after exact
            // root checks. VACUUM INTO then creates a different SQLite inode
            // containing the allowlisted logical tables; the private stage
            // inode is never the selected artifact.
            if self.closed_input_rows.is_none() {
                self.db()
                    .execute_batch("PRAGMA secure_delete=ON; DROP TABLE raw_records")?;
            }
            selected_table_closure(self.db())?;
            self.check(WritePhase::Finalize)?;
            let fresh = fresh_selected_path(&self.candidate);
            self.isolation
                .ok_or(Error::Invalid("selected stage isolation absent"))?
                .verify(&fresh, self.limits, WritePhase::Finalize)?;
            let fresh_utf8 = fresh
                .to_str()
                .ok_or(Error::Invalid("stage fresh selected path encoding"))?;
            self.fresh_selected = Some(fresh.clone());
            self.db().execute("VACUUM INTO ?1", [fresh_utf8])?;
            self.check(WritePhase::Finalize)?;
            fs::set_permissions(&fresh, fs::Permissions::from_mode(0o600))?;
            let pinned = safe_open::open_regular(&fresh, self.limits.sqlite.max_output_bytes)?;
            verify_fresh_selected(
                &fresh,
                &pinned,
                self.limits.sqlite,
                Arc::clone(self.vm_used.as_ref().expect("stage VM counter")),
            )?;
            selected_file = Some(pinned);
        }
        self.check(WritePhase::Finalize)?;
        if self
            .db()
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))?
            != "ok"
        {
            return Err(Error::Invalid("stage SQLite integrity"));
        }
        self.check(WritePhase::Finalize)?;
        self.owner.recheck_sealed_cut(&self.receipt)?;
        let db = self.db.take().expect("stage database open");
        db.close().map_err(|(_, e)| Error::Sql(e))?;
        let output_path = self.fresh_selected.as_deref().unwrap_or(&self.candidate);
        let (sqlite_sha256, sqlite_size_bytes) = if let Some(pinned) = selected_file.as_ref() {
            let mut digest_file = pinned.try_clone()?;
            digest_file.rewind()?;
            stream_digest(&mut digest_file)?
        } else {
            file_digest(output_path)?
        };
        if sqlite_size_bytes > self.limits.sqlite.max_output_bytes {
            return Err(Error::Budget("stage final output bytes"));
        }
        if let Some(pinned) = selected_file.as_ref() {
            pinned.sync_all()?;
        } else {
            fs::File::open(output_path)?.sync_all()?;
        }
        if let Some(fresh) = self.fresh_selected.clone() {
            let old = fs::symlink_metadata(&self.candidate)?;
            let new = fs::symlink_metadata(&fresh)?;
            let pinned = selected_file
                .as_ref()
                .ok_or(Error::Invalid("fresh selected file not retained"))?
                .metadata()?;
            if !old.file_type().is_file()
                || (old.dev(), old.ino()) != self.inode
                || !new.file_type().is_file()
                || (new.dev(), new.ino()) != (pinned.dev(), pinned.ino())
                || new.len() != sqlite_size_bytes
                || old.uid() != new.uid()
            {
                return Err(Error::Invalid("selected stage inode changed"));
            }
            cleanup_sqlite_sidecars(&self.candidate, old.uid())?;
            // Persist the replacement inode under the existing locked lease
            // before rename. A crash on either side of rename then leaves a
            // recoverable exact old/new inode, never an unqualified path.
            self.mark_replacement((new.dev(), new.ino()))?;
            fs::rename(&fresh, &self.candidate)?;
            self.inode = (new.dev(), new.ino());
            self.fresh_selected = None;
            let installed = fs::symlink_metadata(&self.candidate)?;
            if !installed.file_type().is_file()
                || (installed.dev(), installed.ino()) != self.inode
                || installed.len() != sqlite_size_bytes
            {
                return Err(Error::Invalid("selected installed inode changed"));
            }
        }
        fs::File::open(self.candidate.parent().expect("stage parent"))?.sync_all()?;
        self.remove_lease()?;
        self.keep = true;
        Ok(StageReceipt {
            binding: self.exact_receipt()?.binding.clone(),
            source_cut: self.exact_receipt()?.binding.source_cut.clone(),
            membership_root: self.exact_receipt()?.binding.membership_root.clone(),
            input_collections: self.receipt.collections().len(),
            verified_inputs: self.receipt.collections().to_vec(),
            input_rows,
            node_rows,
            relation_rows,
            node_root_sha256: node_root,
            relation_root_sha256: relation_root,
            sqlite_sha256,
            sqlite_size_bytes,
        })
    }
}

pub(crate) fn configure_stage_temp_reclamation(db: &Connection) -> Result<()> {
    db.execute_batch("PRAGMA temp.auto_vacuum=INCREMENTAL")?;
    let mode: i64 = db.query_row("PRAGMA temp.auto_vacuum", [], |row| row.get(0))?;
    if mode != 2 {
        return Err(Error::Invalid("stage TEMP reclamation mode"));
    }
    Ok(())
}

/// VACUUM builds a second database while the current one is still present.
/// Reserve two current-file equivalents in the declared private temp budget
/// before dropping owner input or starting that rebuild. The host isolation
/// guard remains responsible for enforcing the actual peak across temp,
/// rollback, fallback paths and output files; this arithmetic is an early
/// refusal, not a filesystem quota implementation.
fn preflight_selected_vacuum(
    db: &Connection,
    candidate: &Path,
    inode: (u64, u64),
    limits: StageLimits,
) -> Result<()> {
    let page_count: i64 = db.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let page_size: i64 = db.query_row("PRAGMA page_size", [], |row| row.get(0))?;
    let (page_count, page_size) = (
        u64::try_from(page_count).map_err(|_| Error::Invalid("selected page count"))?,
        u64::try_from(page_size).map_err(|_| Error::Invalid("selected page size"))?,
    );
    if page_count == 0 || page_size == 0 {
        return Err(Error::Invalid("selected SQLite page geometry"));
    }
    let database_bytes = page_count
        .checked_mul(page_size)
        .ok_or(Error::Budget("selected SQLite page bytes"))?;
    let rebuild_reserve = database_bytes
        .checked_mul(2)
        .ok_or(Error::Budget("selected VACUUM rebuild reserve"))?;
    let metadata = fs::symlink_metadata(candidate)?;
    if !metadata.file_type().is_file()
        || (metadata.dev(), metadata.ino()) != inode
        || metadata.len() != database_bytes
    {
        return Err(Error::Invalid("selected SQLite file/page mismatch"));
    }
    if database_bytes > limits.sqlite.max_output_bytes || rebuild_reserve > limits.max_temp_bytes {
        return Err(Error::Budget("selected VACUUM output/temp reserve"));
    }
    Ok(())
}

pub(crate) fn selected_table_closure(db: &Connection) -> Result<()> {
    const TABLES: &[&str] = &[
        "metadata",
        "graph_header",
        "knowledge_nodes",
        "knowledge_relations",
        "source_scope",
        "search_documents",
        "search_posting_blocks",
        "search_gram_stats",
        "catalog_index_meta",
        "catalog_facet_fields",
        "catalog_facets",
        "catalog_routes",
        "catalog_source_counts",
    ];
    let philosophy_original = crate::knowledge_philosophy_original::present(db)?;
    let corpus_original = crate::knowledge_corpus_original::present(db)?;
    let philosophy_tables = [
        crate::knowledge_philosophy_original::META_TABLE,
        crate::knowledge_philosophy_original::ROW_TABLE,
    ];
    let navigation_original = crate::knowledge_navigation_original::present(db)?;
    let navigation_tables = [
        crate::knowledge_navigation_original::META_TABLE,
        crate::knowledge_navigation_original::ROW_TABLE,
        crate::knowledge_navigation_original::MEMBER_TABLE,
    ];
    let mut statement = db.prepare(
        "SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN name ELSE NULL END
         FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let mut rows = statement.query([])?;
    let mut seen = std::collections::BTreeSet::new();
    while let Some(row) = rows.next()? {
        let name: Option<String> = row.get(0)?;
        let Some(name) = name else {
            return Err(Error::Budget("selected knowledge table name bytes"));
        };
        if !(TABLES.contains(&name.as_str())
            || navigation_original && navigation_tables.contains(&name.as_str())
            || philosophy_original && philosophy_tables.contains(&name.as_str())
            || corpus_original
                && [
                    crate::knowledge_corpus_original::META_TABLE,
                    crate::knowledge_corpus_original::ROW_TABLE,
                ]
                .contains(&name.as_str()))
            || !seen.insert(name)
        {
            return Err(Error::Invalid("unexpected selected knowledge table"));
        }
    }
    if seen.len()
        != TABLES.len()
            + if navigation_original { 3 } else { 0 }
            + if philosophy_original { 2 } else { 0 }
            + if corpus_original { 2 } else { 0 }
    {
        return Err(Error::Invalid("missing selected knowledge table"));
    }
    const EXPLICIT_INDEXES: &[(&str, &str)] = &[
        (
            "knowledge_nodes_source_order",
            "CREATE INDEX knowledge_nodes_source_order ON knowledge_nodes(source_graph,source_order,id)",
        ),
        (
            "knowledge_nodes_kind",
            "CREATE INDEX knowledge_nodes_kind ON knowledge_nodes(kind_id,source_order)",
        ),
        (
            "knowledge_nodes_entity",
            "CREATE INDEX knowledge_nodes_entity ON knowledge_nodes(entity_id,source_order,id)",
        ),
        (
            "knowledge_nodes_native",
            "CREATE INDEX knowledge_nodes_native ON knowledge_nodes(native_id,source_order,id)",
        ),
        (
            "knowledge_nodes_entity_id",
            "CREATE INDEX knowledge_nodes_entity_id ON knowledge_nodes(entity_id,id)",
        ),
        (
            "knowledge_relations_native",
            "CREATE INDEX knowledge_relations_native ON knowledge_relations(native_id,source_order,id)",
        ),
        (
            "knowledge_relations_source_order",
            "CREATE INDEX knowledge_relations_source_order ON knowledge_relations(source_graph,source_order,id)",
        ),
        (
            "knowledge_relations_from",
            "CREATE INDEX knowledge_relations_from ON knowledge_relations(from_id,source_order,id)",
        ),
        (
            "knowledge_relations_to",
            "CREATE INDEX knowledge_relations_to ON knowledge_relations(to_id,source_order,id)",
        ),
        (
            "knowledge_relations_from_id",
            "CREATE INDEX knowledge_relations_from_id ON knowledge_relations(from_id,id)",
        ),
        (
            "knowledge_relations_to_id",
            "CREATE INDEX knowledge_relations_to_id ON knowledge_relations(to_id,id)",
        ),
        (
            "knowledge_relations_predicate",
            "CREATE INDEX knowledge_relations_predicate ON knowledge_relations(predicate_id,source_order)",
        ),
        (
            "search_document_filter",
            "CREATE INDEX search_document_filter ON search_documents(kind,source_graph,kind_id,predicate_id,position)",
        ),
    ];
    let mut statement = db.prepare(
        "SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN name ELSE NULL END,
                CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=1024 THEN sql ELSE NULL END
         FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name",
    )?;
    let mut rows = statement.query([])?;
    let mut indexes = BTreeSet::new();
    while let Some(row) = rows.next()? {
        let name: Option<String> = row.get(0)?;
        let sql: Option<String> = row.get(1)?;
        let (Some(name), Some(sql)) = (name, sql) else {
            return Err(Error::Budget("selected knowledge schema text bytes"));
        };
        if !(EXPLICIT_INDEXES
            .iter()
            .any(|(expected_name, expected_sql)| name == *expected_name && sql == *expected_sql)
            || corpus_original
                && crate::knowledge_corpus_original::INDEXES
                    .iter()
                    .any(|(n, d)| name == *n && sql == *d))
            || !indexes.insert(name)
        {
            return Err(Error::Invalid("unexpected selected knowledge index"));
        }
    }
    if indexes.len()
        != EXPLICIT_INDEXES.len()
            + if corpus_original {
                crate::knowledge_corpus_original::INDEXES.len()
            } else {
                0
            }
    {
        return Err(Error::Invalid("missing selected knowledge index"));
    }
    let extra: Option<i64> = db
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type NOT IN ('table','index') LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if extra.is_some() {
        return Err(Error::Invalid(
            "unexpected selected knowledge schema object",
        ));
    }
    Ok(())
}

fn verify_fresh_selected(
    path: &Path,
    pinned: &fs::File,
    limits: Limits,
    used: Arc<AtomicU64>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let opened = pinned.metadata()?;
    if !metadata.file_type().is_file()
        || (metadata.dev(), metadata.ino()) != (opened.dev(), opened.ino())
        || opened.len() > limits.max_output_bytes
    {
        return Err(Error::Budget("fresh selected SQLite bytes/type"));
    }
    if sqlite_sidecar_paths(path)
        .iter()
        .any(|sidecar| sidecar.exists() || sidecar.is_symlink())
    {
        return Err(Error::Invalid("fresh selected SQLite sidecar"));
    }
    let db = tos_source_store::PinnedSqliteConnection::open_readonly_immutable(pinned)
        .map_err(|error| Error::Source(error.to_string()))?;
    sqlite_budget::install_progress(&db, limits, used);
    db.pragma_update(None, "cache_size", -(limits.sqlite_cache_kib as i64))?;
    db.execute_batch("PRAGMA temp_store=FILE")?;
    crate::knowledge_selected::verify_schema(&db)?;
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let freelist: u64 = db.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
    if integrity != "ok" || freelist != 0 {
        return Err(Error::Invalid("fresh selected SQLite integrity/pages"));
    }
    db.close().map_err(|(_, error)| Error::Sql(error))?;
    Ok(())
}

impl Drop for KnowledgeStage<'_> {
    fn drop(&mut self) {
        self.db.take();
        if !self.keep {
            if let Some(fresh) = self.fresh_selected.as_ref() {
                if let (Ok(metadata), Some(lease)) =
                    (fs::symlink_metadata(fresh), self.lease.as_ref())
                {
                    if metadata.file_type().is_file()
                        && lease
                            .metadata()
                            .is_ok_and(|lease| lease.uid() == metadata.uid())
                    {
                        let _ = cleanup_sqlite_sidecars(fresh, metadata.uid());
                        let _ = fs::remove_file(fresh);
                    }
                }
            }
            if let Ok(metadata) = fs::symlink_metadata(&self.candidate) {
                if metadata.file_type().is_file() && (metadata.dev(), metadata.ino()) == self.inode
                {
                    let _ = cleanup_sqlite_sidecars(&self.candidate, metadata.uid());
                    let _ = fs::remove_file(&self.candidate);
                }
            }
            let _ = self.remove_lease();
        }
    }
}

impl KnowledgeStage<'_> {
    fn mark_replacement(&mut self, replacement: (u64, u64)) -> Result<()> {
        let metadata = fs::symlink_metadata(&self.lease_path)?;
        if !metadata.file_type().is_file() || (metadata.dev(), metadata.ino()) != self.lease_inode {
            return Err(Error::Invalid("stage lease path changed"));
        }
        let lease = self
            .lease
            .as_mut()
            .ok_or(Error::Invalid("stage lease absent"))?;
        lease.seek(SeekFrom::End(0))?;
        writeln!(
            lease,
            "tos-knowledge-stage-new-inode-v1 {} {}",
            replacement.0, replacement.1
        )?;
        lease.sync_all()?;
        Ok(())
    }

    fn remove_lease(&mut self) -> Result<()> {
        if let Ok(metadata) = fs::symlink_metadata(&self.lease_path) {
            if !metadata.file_type().is_file()
                || (metadata.dev(), metadata.ino()) != self.lease_inode
            {
                return Err(Error::Invalid("stage lease path changed"));
            }
            fs::remove_file(&self.lease_path)?;
            fs::File::open(
                self.lease_path
                    .parent()
                    .ok_or(Error::Invalid("stage lease parent"))?,
            )?
            .sync_all()?;
        }
        self.lease.take();
        Ok(())
    }
}

fn lease_path(candidate: &Path) -> Result<PathBuf> {
    let name = candidate
        .file_name()
        .ok_or(Error::Invalid("stage candidate filename"))?;
    let mut name = name.to_os_string();
    name.push(".stage-lease");
    Ok(candidate.with_file_name(name))
}

fn fresh_selected_path(candidate: &Path) -> PathBuf {
    let mut path = candidate.as_os_str().to_os_string();
    path.push(".fresh-selected");
    PathBuf::from(path)
}

fn sqlite_sidecar_paths(candidate: &Path) -> [PathBuf; 3] {
    ["-journal", "-wal", "-shm"].map(|suffix| {
        let mut path = candidate.as_os_str().to_os_string();
        path.push(suffix);
        PathBuf::from(path)
    })
}
fn cleanup_sqlite_sidecars(candidate: &Path, uid: u32) -> Result<()> {
    for path in sqlite_sidecar_paths(candidate) {
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.file_type().is_file() || metadata.uid() != uid {
                    return Err(Error::Invalid("stage SQLite sidecar changed"));
                }
                fs::remove_file(path)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::Io(error)),
        }
    }
    Ok(())
}

/// Recover one abandoned private candidate after the owner has established
/// that its producer process is gone. An active lock or changed inode refuses;
/// this never touches selected models or arbitrary neighboring files.
pub fn reap_abandoned_private_stage(candidate: &Path) -> Result<bool> {
    let lease_path = lease_path(candidate)?;
    if !lease_path.exists() && !lease_path.is_symlink() {
        return Ok(false);
    }
    let mut lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&lease_path)?;
    if !lease.metadata()?.file_type().is_file() {
        return Err(Error::Invalid("stage lease type"));
    }
    lease
        .try_lock_exclusive()
        .map_err(|_| Error::Invalid("stage producer still active"))?;
    let mut marker = String::new();
    lease.seek(SeekFrom::Start(0))?;
    if lease.metadata()?.len() > 192 {
        return Err(Error::Invalid("stage lease marker bytes"));
    }
    Read::by_ref(&mut lease)
        .take(192)
        .read_to_string(&mut marker)?;
    let mut lines = marker.lines();
    let mut fields = lines
        .next()
        .ok_or(Error::Invalid("stage lease marker"))?
        .split_whitespace();
    if fields.next() != Some("tos-knowledge-stage-v1") {
        return Err(Error::Invalid("stage lease marker"));
    }
    let dev: u64 = fields
        .next()
        .ok_or(Error::Invalid("stage lease device"))?
        .parse()
        .map_err(|_| Error::Invalid("stage lease device"))?;
    let ino: u64 = fields
        .next()
        .ok_or(Error::Invalid("stage lease inode"))?
        .parse()
        .map_err(|_| Error::Invalid("stage lease inode"))?;
    if fields.next().is_some() {
        return Err(Error::Invalid("stage lease marker trailing data"));
    }
    let metadata = fs::symlink_metadata(candidate)?;
    if !metadata.file_type().is_file() {
        return Err(Error::Invalid("stage candidate changed since lease"));
    }
    let renamed = (metadata.dev(), metadata.ino()) != (dev, ino);
    if renamed {
        let mut replacement = lines
            .next()
            .ok_or(Error::Invalid("stage replacement lease absent"))?
            .split_whitespace();
        if replacement.next() != Some("tos-knowledge-stage-new-inode-v1") {
            return Err(Error::Invalid("stage replacement lease marker"));
        }
        let new_dev: u64 = replacement
            .next()
            .ok_or(Error::Invalid("stage replacement device"))?
            .parse()
            .map_err(|_| Error::Invalid("stage replacement device"))?;
        let new_ino: u64 = replacement
            .next()
            .ok_or(Error::Invalid("stage replacement inode"))?
            .parse()
            .map_err(|_| Error::Invalid("stage replacement inode"))?;
        if replacement.next().is_some()
            || lines.next().is_some()
            || (metadata.dev(), metadata.ino()) != (new_dev, new_ino)
        {
            return Err(Error::Invalid("stage replacement inode differs"));
        }
    }
    let fresh = fresh_selected_path(candidate);
    match fs::symlink_metadata(&fresh) {
        Ok(output) => {
            if renamed {
                return Err(Error::Invalid("renamed stage also has fresh output"));
            }
            if !output.file_type().is_file() || output.uid() != metadata.uid() {
                return Err(Error::Invalid("abandoned fresh selected path changed"));
            }
            cleanup_sqlite_sidecars(&fresh, output.uid())?;
            fs::remove_file(&fresh)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::Io(error)),
    }
    cleanup_sqlite_sidecars(candidate, metadata.uid())?;
    fs::remove_file(candidate)?;
    fs::remove_file(&lease_path)?;
    FileExt::unlock(&lease)?;
    fs::File::open(candidate.parent().ok_or(Error::Invalid("stage parent"))?)?.sync_all()?;
    Ok(true)
}

fn valid_id(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES {
        return Err(Error::Invalid("stage ID bytes"));
    }
    Ok(())
}
fn root_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}
fn input_root(db: &Connection, entry: &InputCollectionReceipt) -> Result<(u64, String)> {
    let mut statement = db.prepare(
        "SELECT id,payload_sha256 FROM raw_records
      WHERE source_graph=?1 AND collection=?2 ORDER BY id",
    )?;
    let mut rows = statement.query(params![entry.source_graph, entry.collection])?;
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let digest: Vec<u8> = row.get(1)?;
        if digest.len() != 32 {
            return Err(Error::Invalid("stage payload digest size"));
        }
        root_item(&mut hash, &id, &digest);
        count = count.checked_add(1).ok_or(Error::Budget("input rows"))?;
    }
    Ok((count, hash.finalize().to_hex()))
}
fn output_root(db: &Connection, table: &str) -> Result<(u64, String)> {
    let sql = match table {
        "knowledge_nodes" => {
            "SELECT id,source_graph,source_order,payload_sha256 FROM knowledge_nodes ORDER BY source_graph,id"
        }
        "knowledge_relations" => {
            "SELECT id,source_graph,source_order,payload_sha256 FROM knowledge_relations ORDER BY source_graph,id"
        }
        _ => return Err(Error::Invalid("unknown stage output table")),
    };
    let mut statement = db.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let order: i64 = row.get(2)?;
        let digest: Vec<u8> = row.get(3)?;
        if digest.len() != 32 || order < 0 || order as u64 != count {
            return Err(Error::Invalid("stage output source order/digest"));
        }
        root_item(&mut hash, &id, &digest);
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("stage output rows"))?;
    }
    Ok((count, hash.finalize().to_hex()))
}
fn read_seek_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SeekRow> {
    let digest: Vec<u8> = row.get(4)?;
    Ok(SeekRow {
        id: row.get(0)?,
        source_graph: row.get(1)?,
        source_order: row.get(2)?,
        payload: row.get(3)?,
        payload_sha256: digest.iter().map(|b| format!("{b:02x}")).collect(),
    })
}
fn verify_seek_row(row: SeekRow, max_row_bytes: usize) -> Result<SeekRow> {
    if row.payload.len() > max_row_bytes {
        return Err(Error::Budget("stage seek row bytes"));
    }
    if Digest256::of_bytes(&row.payload).to_hex() != row.payload_sha256 {
        return Err(Error::Invalid("stage seek payload digest"));
    }
    Ok(row)
}

const SCHEMA: &str = r#"
CREATE TABLE raw_records(
 source_graph TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL,
 PRIMARY KEY(source_graph,collection,id)) WITHOUT ROWID;
CREATE TABLE knowledge_nodes(
 id TEXT PRIMARY KEY,source_graph TEXT NOT NULL,native_id TEXT,entity_id TEXT,
 kind_id TEXT NOT NULL,type_id TEXT NOT NULL,source_order INTEGER NOT NULL UNIQUE,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL) WITHOUT ROWID;
CREATE INDEX knowledge_nodes_source_order ON knowledge_nodes(source_graph,source_order,id);
CREATE INDEX knowledge_nodes_kind ON knowledge_nodes(kind_id,source_order);
CREATE INDEX knowledge_nodes_entity ON knowledge_nodes(entity_id,source_order,id);
CREATE INDEX knowledge_nodes_native ON knowledge_nodes(native_id,source_order,id);
CREATE INDEX knowledge_nodes_entity_id ON knowledge_nodes(entity_id,id);
CREATE TABLE knowledge_relations(
 id TEXT PRIMARY KEY,source_graph TEXT NOT NULL,native_id TEXT,
 from_id TEXT NOT NULL,to_id TEXT NOT NULL,predicate_id TEXT NOT NULL,
 relation_type_id TEXT NOT NULL,source_order INTEGER NOT NULL UNIQUE,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL) WITHOUT ROWID;
CREATE INDEX knowledge_relations_source_order ON knowledge_relations(source_graph,source_order,id);
CREATE INDEX knowledge_relations_native ON knowledge_relations(native_id,source_order,id);
CREATE INDEX knowledge_relations_from ON knowledge_relations(from_id,source_order,id);
CREATE INDEX knowledge_relations_to ON knowledge_relations(to_id,source_order,id);
CREATE INDEX knowledge_relations_from_id ON knowledge_relations(from_id,id);
CREATE INDEX knowledge_relations_to_id ON knowledge_relations(to_id,id);
CREATE INDEX knowledge_relations_predicate ON knowledge_relations(predicate_id,source_order);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    const RAW_ROOT: &str = "4a6512ce1f0842dc9246eb059c186dfb2b08bd6f2515f000474c792273958ca4";
    const EMPTY_ROOT: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    struct Owner {
        checks: AtomicUsize,
    }
    impl StageOwner for Owner {
        fn verify_receipt(&self, receipt: &ExactInputReceipt) -> Result<()> {
            if receipt.collections[0].adapter_profile != "fixture-adapter-v1" {
                return Err(Error::Invalid("fixture adapter"));
            }
            self.checks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> Result<()> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    struct TestQuota {
        calls: AtomicUsize,
        deny: bool,
    }
    impl StageIsolation for TestQuota {
        fn verify(&self, _: &Path, limits: StageLimits, _: WritePhase) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.deny || limits.max_temp_bytes == 0 {
                return Err(Error::Budget("test host temp quota"));
            }
            Ok(())
        }
    }
    struct ToggleQuota {
        deny: AtomicBool,
    }
    impl StageIsolation for ToggleQuota {
        fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> Result<()> {
            if self.deny.load(Ordering::SeqCst) {
                Err(Error::Budget("late stage quota"))
            } else {
                Ok(())
            }
        }
    }
    fn limits() -> StageLimits {
        StageLimits {
            sqlite: Limits::default(),
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 8,
            max_seek_bytes: 1024,
        }
    }
    fn exact_receipt(root: &str) -> ExactInputReceipt {
        ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "sealed-cut-7".into(),
                through_commit_seq: 7,
                membership_root: "0".repeat(64),
                index_generation: "gen-1".into(),
                route_map_version: "routes-1".into(),
                reader_abi: "reader-1".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: vec![
                InputCollectionReceipt {
                    source_graph: "fixture.graph".into(),
                    collection: "fixture/raw".into(),
                    input_role: "fixture".into(),
                    adapter_profile: "fixture-adapter-v1".into(),
                    expected_count: 1,
                    expected_root_sha256: root.into(),
                },
                InputCollectionReceipt {
                    source_graph: "fixture.graph".into(),
                    collection: "fixture/empty".into(),
                    input_role: "fixture".into(),
                    adapter_profile: "fixture-adapter-v1".into(),
                    expected_count: 0,
                    expected_root_sha256: EMPTY_ROOT.into(),
                },
            ],
        }
    }

    #[test]
    fn registration_index_accepts_more_than_legacy_collection_ceiling() {
        let candidate = stage_path("many-collections");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut receipt = exact_receipt(RAW_ROOT);
        for index in 0..300 {
            receipt.collections.push(InputCollectionReceipt {
                source_graph: "fixture.graph".into(),
                collection: format!("extra/{index}"),
                input_role: "fixture".into(),
                adapter_profile: "fixture-adapter-v1".into(),
                expected_count: 0,
                expected_root_sha256: EMPTY_ROOT.into(),
            });
        }
        let mut stage =
            KnowledgeStage::create(&candidate, limits(), receipt, &owner, &quota).unwrap();
        assert!(stage.registered("fixture.graph", "extra/299"));
        assert!(!stage.registered("fixture.graph", "extra/300"));
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "extra/299",
                id: "row.1",
                payload: b"row",
            })
            .unwrap();
        drop(stage);
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }
    fn stage_path(label: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-knowledge-stage-{label}-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir.join("candidate.sqlite3")
    }
    fn ingest_fixture(stage: &mut KnowledgeStage<'_>) {
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage
            .insert_node(NodeRow {
                id: "node.1",
                source_graph: "fixture.graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind.1",
                type_id: "type.1",
                source_order: 0,
                payload: b"node",
            })
            .unwrap();
        stage
            .insert_relation(RelationRow {
                id: "rel.1",
                source_graph: "fixture.graph",
                native_id: None,
                from_id: "node.1",
                to_id: "node.1",
                predicate_id: "pred.1",
                relation_type_id: "reltype.1",
                source_order: 0,
                payload: b"relation",
            })
            .unwrap();
    }

    #[test]
    fn atomic_input_chunk_rolls_back_duplicate_and_poison_cleans_private_stage() {
        let candidate = stage_path("input-chunk-rollback");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        let row = || InputRow {
            source_graph: "fixture.graph",
            collection: "fixture/raw",
            id: "raw.1",
            payload: b"raw",
        };
        assert!(stage.ingest_input_batch(&[row(), row()]).is_err());
        assert_eq!(
            stage
                .db()
                .query_row("SELECT count(*) FROM raw_records", [], |r| r
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert!(stage.ingest_input_batch(&[row()]).is_err());
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn bounded_write_page_keeps_prior_chunk_and_refuses_failed_stage() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let candidate = stage_path("normalized-page");
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage.with_write_page(WritePhase::Normalized, 2, 64, |stage| {
            assert_eq!(
                stage.raw_by_id("fixture.graph", "fixture/raw", "raw.1")?.unwrap().payload,
                b"raw"
            );
            stage.insert_node(NodeRow {
                id: "node.1", source_graph: "fixture.graph", native_id: None,
                entity_id: None, kind_id: "kind.1", type_id: "type.1",
                source_order: 0, payload: b"node",
            })?;
            stage.insert_relation(RelationRow {
                id: "rel.1", source_graph: "fixture.graph", native_id: None,
                from_id: "node.1", to_id: "node.1", predicate_id: "pred.1",
                relation_type_id: "reltype.1", source_order: 0, payload: b"relation",
            })?;
            let endpoints = stage.with_connection(WritePhase::Normalized, |db| {
                Ok(db.query_row("SELECT count(*) FROM knowledge_relations r JOIN knowledge_nodes n ON n.id=r.from_id WHERE r.id='rel.1'", [], |row| row.get::<_, u64>(0))?)
            })?;
            assert_eq!(endpoints, 1);
            Ok(())
        }).unwrap();
        let vm_before = stage.vm_used.as_ref().unwrap().load(Ordering::Relaxed);
        let rows_before = stage.total_rows;
        assert!(
            stage
                .with_write_page(WritePhase::Normalized, 2, 64, |stage| {
                    stage.insert_node(NodeRow {
                        id: "node.2",
                        source_graph: "fixture.graph",
                        native_id: None,
                        entity_id: None,
                        kind_id: "kind.1",
                        type_id: "type.1",
                        source_order: 1,
                        payload: b"node",
                    })?;
                    // The caller's ignored SQL error still poisons this page.
                    let _ = stage.insert_node(NodeRow {
                        id: "node.1",
                        source_graph: "fixture.graph",
                        native_id: None,
                        entity_id: None,
                        kind_id: "kind.1",
                        type_id: "type.1",
                        source_order: 2,
                        payload: b"node",
                    });
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(
            stage
                .db()
                .query_row("SELECT count(*) FROM knowledge_nodes", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            1
        );
        assert!(stage.vm_used.as_ref().unwrap().load(Ordering::Relaxed) >= vm_before);
        assert!(stage.total_rows > rows_before);
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();

        let candidate = stage_path("normalized-page-cap");
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        assert!(
            stage
                .with_write_page(WritePhase::Normalized, 1, 8, |stage| {
                    stage.insert_node(NodeRow {
                        id: "node.1",
                        source_graph: "fixture.graph",
                        native_id: None,
                        entity_id: None,
                        kind_id: "kind.1",
                        type_id: "type.1",
                        source_order: 0,
                        payload: b"node",
                    })?;
                    stage.insert_node(NodeRow {
                        id: "node.2",
                        source_graph: "fixture.graph",
                        native_id: None,
                        entity_id: None,
                        kind_id: "kind.1",
                        type_id: "type.1",
                        source_order: 1,
                        payload: b"node",
                    })?;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(
            stage
                .db()
                .query_row("SELECT count(*) FROM knowledge_nodes", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();

        let candidate = stage_path("normalized-page-nested");
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        assert!(
            stage
                .with_write_page(WritePhase::Normalized, 1, 8, |stage| {
                    stage.insert_node(NodeRow {
                        id: "node.1",
                        source_graph: "fixture.graph",
                        native_id: None,
                        entity_id: None,
                        kind_id: "kind.1",
                        type_id: "type.1",
                        source_order: 0,
                        payload: b"node",
                    })?;
                    let _ = stage.with_write_page(WritePhase::Normalized, 1, 8, |_| Ok(()));
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(
            stage
                .db()
                .query_row("SELECT count(*) FROM knowledge_nodes", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();

        let candidate = stage_path("normalized-page-late-guard");
        let late = ToggleQuota {
            deny: AtomicBool::new(false),
        };
        let mut stage =
            KnowledgeStage::create(&candidate, limits(), exact_receipt(RAW_ROOT), &owner, &late)
                .unwrap();
        assert!(
            stage
                .with_write_page(WritePhase::Normalized, 1, 8, |stage| {
                    stage.insert_node(NodeRow {
                        id: "node.1",
                        source_graph: "fixture.graph",
                        native_id: None,
                        entity_id: None,
                        kind_id: "kind.1",
                        type_id: "type.1",
                        source_order: 0,
                        payload: b"node",
                    })?;
                    late.deny.store(true, Ordering::SeqCst);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(
            stage
                .db()
                .query_row("SELECT count(*) FROM knowledge_nodes", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn exact_owner_root_and_indexed_seek_yield_private_complete_stage() {
        let candidate = stage_path("complete");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        ingest_fixture(&mut stage);
        assert_eq!(
            stage
                .raw_by_id("fixture.graph", "fixture/raw", "raw.1")
                .unwrap()
                .unwrap()
                .payload,
            b"raw"
        );
        let page = stage
            .scan_input("fixture.graph", "fixture/raw", None, 1)
            .unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(
            page.rows[0].payload_sha256,
            Digest256::of_bytes(b"raw").to_hex()
        );
        assert!(page.next_id.is_none());
        assert!(
            stage
                .scan_input("fixture.graph", "fixture/raw", Some("raw.1"), 1)
                .unwrap()
                .rows
                .is_empty()
        );
        let empty = stage
            .scan_input("fixture.graph", "fixture/empty", None, 1)
            .unwrap();
        assert!(empty.rows.is_empty() && empty.next_id.is_none());
        let mut ids = Vec::new();
        assert_eq!(
            stage
                .outgoing("node.1", -1, 2, &mut |row| {
                    ids.push(row.id);
                    Ok(())
                })
                .unwrap(),
            1
        );
        assert_eq!(ids, ["rel.1"]);
        stage.close_inputs_for_full_components().unwrap();
        assert!(
            stage
                .raw_by_id("fixture.graph", "fixture/raw", "raw.1")
                .is_err()
        );
        assert!(
            stage
                .scan_input("fixture.graph", "fixture/raw", None, 1)
                .is_err()
        );
        assert_eq!(
            stage
                .db()
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE name='raw_records'",
                    [],
                    |row| row.get::<_, u64>(0)
                )
                .unwrap(),
            0
        );
        let receipt = stage.finish().unwrap();
        assert_eq!(
            (
                receipt.input_collections,
                receipt.input_rows,
                receipt.node_rows,
                receipt.relation_rows
            ),
            (2, 1, 1, 1)
        );
        assert_eq!(receipt.source_cut, "sealed-cut-7");
        assert!(candidate.is_file());
        assert_eq!(owner.checks.load(Ordering::SeqCst), 4);
        assert!(quota.calls.load(Ordering::SeqCst) >= 6);
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn omitted_input_and_ignored_insert_error_cannot_finish() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let omitted = stage_path("omitted");
        let stage =
            KnowledgeStage::create(&omitted, limits(), exact_receipt(RAW_ROOT), &owner, &quota)
                .unwrap();
        assert!(stage.finish().is_err());
        assert!(!omitted.exists());
        fs::remove_dir_all(omitted.parent().unwrap()).unwrap();

        let omitted = stage_path("omitted-transition");
        let mut stage =
            KnowledgeStage::create(&omitted, limits(), exact_receipt(RAW_ROOT), &owner, &quota)
                .unwrap();
        assert!(stage.close_inputs_for_full_components().is_err());
        assert!(stage.finish().is_err());
        assert!(!omitted.exists());
        fs::remove_dir_all(omitted.parent().unwrap()).unwrap();

        let duplicate = stage_path("duplicate");
        let mut stage = KnowledgeStage::create(
            &duplicate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        assert!(
            stage
                .ingest_input(InputRow {
                    source_graph: "fixture.graph",
                    collection: "fixture/raw",
                    id: "raw.1",
                    payload: b"raw"
                })
                .is_err()
        );
        assert!(stage.finish().is_err());
        assert!(!duplicate.exists());
        fs::remove_dir_all(duplicate.parent().unwrap()).unwrap();
    }

    #[test]
    fn owner_root_and_quota_gate_fail_before_candidate_admission() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let wrong = stage_path("root");
        let mut stage =
            KnowledgeStage::create(&wrong, limits(), exact_receipt(EMPTY_ROOT), &owner, &quota)
                .unwrap();
        ingest_fixture(&mut stage);
        assert!(stage.finish().is_err());
        assert!(!wrong.exists());
        fs::remove_dir_all(wrong.parent().unwrap()).unwrap();

        let denied = stage_path("quota");
        let denied_quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: true,
        };
        assert!(
            KnowledgeStage::create(
                &denied,
                limits(),
                exact_receipt(RAW_ROOT),
                &owner,
                &denied_quota
            )
            .is_err()
        );
        assert!(!denied.exists());
        fs::remove_dir_all(denied.parent().unwrap()).unwrap();
    }

    #[test]
    fn selected_vacuum_refuses_unadmitted_rebuild_before_private_drop() {
        let candidate = stage_path("vacuum-reserve");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut tight = limits();
        tight.max_temp_bytes = 1;
        let mut stage =
            KnowledgeStage::create(&candidate, tight, exact_receipt(RAW_ROOT), &owner, &quota)
                .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage.mark_selected_full().unwrap();
        let result = stage.finish();
        assert!(matches!(
            result,
            Err(Error::Budget("selected VACUUM output/temp reserve"))
        ));
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn noncontiguous_source_order_is_rejected_after_external_sort() {
        let candidate = stage_path("order");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage
            .insert_node(NodeRow {
                id: "node.1",
                source_graph: "fixture.graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind.1",
                type_id: "type.1",
                source_order: 1,
                payload: b"node",
            })
            .unwrap();
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn endpoint_closure_and_composer_failure_cannot_finish() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let candidate = stage_path("endpoint");
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage
            .insert_node(NodeRow {
                id: "node.1",
                source_graph: "fixture.graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind.1",
                type_id: "type.1",
                source_order: 0,
                payload: b"node",
            })
            .unwrap();
        stage
            .insert_relation(RelationRow {
                id: "rel.1",
                source_graph: "fixture.graph",
                native_id: None,
                from_id: "node.1",
                to_id: "node.absent",
                predicate_id: "pred.1",
                relation_type_id: "reltype.1",
                source_order: 0,
                payload: b"relation",
            })
            .unwrap();
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();

        let candidate = stage_path("composer");
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        ingest_fixture(&mut stage);
        assert!(
            stage
                .with_connection::<()>(WritePhase::Catalog, |_| Err(Error::Invalid(
                    "catalog failure"
                )))
                .is_err()
        );
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn abandoned_exact_private_stage_is_reaped_but_live_stage_refuses() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let live = stage_path("live");
        let stage =
            KnowledgeStage::create(&live, limits(), exact_receipt(RAW_ROOT), &owner, &quota)
                .unwrap();
        assert!(reap_abandoned_private_stage(&live).is_err());
        drop(stage);
        assert!(!live.exists());
        fs::remove_dir_all(live.parent().unwrap()).unwrap();

        let orphan = stage_path("orphan");
        fs::write(&orphan, b"private interrupted stage").unwrap();
        let metadata = fs::metadata(&orphan).unwrap();
        fs::write(
            lease_path(&orphan).unwrap(),
            format!(
                "tos-knowledge-stage-v1 {} {}\n",
                metadata.dev(),
                metadata.ino()
            ),
        )
        .unwrap();
        assert!(reap_abandoned_private_stage(&orphan).unwrap());
        assert!(!orphan.exists());
        assert!(!lease_path(&orphan).unwrap().exists());
        fs::remove_dir_all(orphan.parent().unwrap()).unwrap();

        // The same recovery route also recognizes a replacement inode that
        // was recorded and renamed before the producer died.
        let renamed = stage_path("renamed");
        fs::write(&renamed, b"old private stage").unwrap();
        let old = fs::metadata(&renamed).unwrap();
        let fresh = fresh_selected_path(&renamed);
        fs::write(&fresh, b"new selected candidate").unwrap();
        let new = fs::metadata(&fresh).unwrap();
        fs::write(
            lease_path(&renamed).unwrap(),
            format!(
                "tos-knowledge-stage-v1 {} {}\ntos-knowledge-stage-new-inode-v1 {} {}\n",
                old.dev(),
                old.ino(),
                new.dev(),
                new.ino()
            ),
        )
        .unwrap();
        fs::rename(&fresh, &renamed).unwrap();
        assert!(reap_abandoned_private_stage(&renamed).unwrap());
        assert!(!renamed.exists());
        assert!(!lease_path(&renamed).unwrap().exists());
        fs::remove_dir_all(renamed.parent().unwrap()).unwrap();
    }
}
