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
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
    },
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
    pub fn validate(self) -> Result<()> {
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

// Field order is intentional: row/page payloads drop before their admission
// guards on success, error or unwind. This is a temporary page, not a refund
// of any independently retained normalized output created by its consumer.
struct OwnedInputPage<'a> {
    page: ScanPage,
    row_holds: Vec<crate::d1_public_capture::CreationStateHold<'a, 'a>>,
    container_hold: crate::d1_public_capture::CreationStateHold<'a, 'a>,
}

/// An exact source row whose input admission follows the borrowed row's actual
/// lifetime. Dropping this reader releases only its own input, never output
/// retained independently by the consumer.
pub(crate) struct ScopedRawRow<'a> {
    owned: Option<OwnedInputPage<'a>>,
    legacy: Option<SeekRow>,
}
impl ScopedRawRow<'_> {
    pub(crate) fn as_ref(&self) -> Option<&SeekRow> {
        match &self.owned {
            Some(page) => page.page.rows.first(),
            None => self.legacy.as_ref(),
        }
    }
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

/// Existing snapshot phase owner: the callback already includes capture,
/// original SQLite pool and every other live model/session owner. This holder
/// admits only Stage Rust allocations; it is not a separate state grant.
pub(crate) struct NativeStageOwnedBudget<'a> {
    pub remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
    pub heap: &'a Arc<sqlite_budget::DedicatedSessionSqliteHeap>,
    pub creation_state: &'a crate::d1_public_capture::CreationState<'a>,
    pub cancelled: Arc<AtomicBool>,
    pub original_sql_limit: u64,
    retained_rust_bytes: usize,
}
impl NativeStageOwnedBudget<'_> {
    fn admit(&self, extra: usize) -> Result<usize> {
        self.heap.verify_current()?;
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Error::Budget("owned native stage cancelled"));
        }
        // The full Stage owner was transferred into CreationState before
        // construction. Other producer allocations see that SAME retention;
        // adding it again here would double-charge this alias.
        (self.remaining_after_retained)(extra)
    }
}

/// Physical layout selected only by a genuine producer/verified model ABI.
/// Source carrier sharing never shares row authority, identity or ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgePayloadLayout {
    InlineV1,
    CarrierOnceV1,
    CarrierOnceV2,
    CarrierOnceV3,
    CarrierOnceV4,
}
impl KnowledgePayloadLayout {
    pub const fn uses_carriers(self) -> bool {
        matches!(
            self,
            Self::CarrierOnceV1 | Self::CarrierOnceV2 | Self::CarrierOnceV3 | Self::CarrierOnceV4
        )
    }
    pub const fn packed_bytes(self) -> bool {
        matches!(
            self,
            Self::CarrierOnceV2 | Self::CarrierOnceV3 | Self::CarrierOnceV4
        )
    }
    pub const fn dictionary_bytes(self) -> bool {
        matches!(self, Self::CarrierOnceV3 | Self::CarrierOnceV4)
    }
    pub(crate) const fn dictionary_capacity(self) -> usize {
        if matches!(self, Self::CarrierOnceV4) {
            crate::knowledge_byte_codec::DICTIONARY_WINDOW_BYTES
        } else {
            crate::knowledge_byte_codec::DICTIONARY_BYTES
        }
    }
    /// Format selection follows an already authenticated model ABI. Callers
    /// retain their independent whole-model and component compatibility checks.
    pub fn from_model_abi(abi: &str) -> Self {
        match abi {
            tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1 => {
                Self::CarrierOnceV1
            }
            tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2 => {
                Self::CarrierOnceV2
            }
            tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V3 => {
                Self::CarrierOnceV3
            }
            tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4 => {
                Self::CarrierOnceV4
            }
            _ => Self::InlineV1,
        }
    }
    pub const fn carrier_model_abi(self) -> Option<&'static str> {
        match self {
            Self::InlineV1 => None,
            Self::CarrierOnceV4 => {
                Some(tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4)
            }
            Self::CarrierOnceV3 => {
                Some(tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V3)
            }
            Self::CarrierOnceV1 => {
                Some(tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1)
            }
            Self::CarrierOnceV2 => {
                Some(tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2)
            }
        }
    }
    pub(crate) fn physical_bound(self, logical_bound: usize) -> Result<usize> {
        if self.packed_bytes() {
            crate::knowledge_byte_codec::stored_bound(logical_bound)
        } else {
            Ok(logical_bound)
        }
    }
    pub(crate) fn verify_physical_length(
        self,
        stored: &[u8],
        expected_bytes: usize,
        max_bytes: usize,
    ) -> Result<()> {
        if self.dictionary_bytes() && crate::knowledge_byte_codec::is_dictionary_frame(stored) {
            crate::knowledge_byte_codec::dictionary_frame_metadata(
                stored,
                Some(expected_bytes),
                max_bytes,
            )?;
        } else if self.packed_bytes() {
            crate::knowledge_byte_codec::frame_metadata(stored, Some(expected_bytes), max_bytes)?;
        } else if expected_bytes == 0
            || expected_bytes > max_bytes
            || stored.len() != expected_bytes
        {
            return Err(Error::Invalid("legacy physical payload length"));
        }
        Ok(())
    }
    pub(crate) fn read_dictionary<'s, 'b>(
        self,
        db: &Connection,
        state: &'s crate::d1_public_capture::CreationState<'b>,
        stored: &[u8],
        max_bytes: usize,
    ) -> Result<Option<crate::knowledge_byte_dictionary::OwnedDictionary<'s, 'b>>> {
        if self.dictionary_bytes() {
            crate::knowledge_byte_dictionary::read_selected(db, state, stored, max_bytes, self)
        } else {
            Ok(None)
        }
    }
    pub(crate) fn with_sql_decoded<T>(
        self,
        db: &Connection,
        state: &crate::d1_public_capture::CreationState<'_>,
        stored: &[u8],
        expected: Option<usize>,
        max_bytes: usize,
        consume: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        let dictionary = self.read_dictionary(db, state, stored, max_bytes)?;
        self.with_decoded_dictionary(
            state,
            stored,
            dictionary.as_ref().map(|d| d.verified()).transpose()?,
            expected,
            max_bytes,
            consume,
        )
    }
    pub(crate) fn with_decoded_dictionary<T>(
        self,
        state: &crate::d1_public_capture::CreationState<'_>,
        stored: &[u8],
        dictionary: Option<crate::knowledge_byte_dictionary::VerifiedDictionary<'_>>,
        expected: Option<usize>,
        max_bytes: usize,
        consume: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        if self.dictionary_bytes() && crate::knowledge_byte_codec::is_dictionary_frame(stored) {
            crate::knowledge_byte_codec::with_dictionary_decoded(
                state,
                stored,
                dictionary.ok_or(Error::Invalid("selected byte dictionary absent"))?,
                expected,
                max_bytes,
                consume,
            )
        } else {
            if dictionary.is_some() {
                return Err(Error::Invalid("unexpected byte dictionary"));
            }
            self.with_decoded(state, stored, expected, max_bytes, consume)
        }
    }
    pub(crate) fn with_encoded_dictionary<T>(
        self,
        state: &crate::d1_public_capture::CreationState<'_>,
        dictionary: Option<crate::knowledge_byte_dictionary::VerifiedDictionary<'_>>,
        raw: &[u8],
        max_bytes: usize,
        consume: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        match dictionary {
            Some(dictionary) if self.dictionary_bytes() => {
                crate::knowledge_byte_codec::with_dictionary_encoded(
                    state, raw, dictionary, max_bytes, consume,
                )
            }
            Some(_) => Err(Error::Invalid(
                "dictionary encoder incompatible with model ABI",
            )),
            None => self.with_encoded(state, raw, max_bytes, consume),
        }
    }
    pub(crate) fn with_decoded<T>(
        self,
        state: &crate::d1_public_capture::CreationState<'_>,
        stored: &[u8],
        expected_bytes: Option<usize>,
        max_bytes: usize,
        consume: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        if self.packed_bytes() {
            crate::knowledge_byte_codec::with_decoded(
                state,
                stored,
                expected_bytes,
                max_bytes,
                consume,
            )
        } else {
            if stored.is_empty()
                || stored.len() > max_bytes
                || expected_bytes.is_some_and(|n| n != stored.len())
            {
                return Err(Error::Invalid("legacy physical payload length"));
            }
            consume(stored)
        }
    }
    pub(crate) fn with_encoded<T>(
        self,
        state: &crate::d1_public_capture::CreationState<'_>,
        raw: &[u8],
        max_bytes: usize,
        consume: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        if self.packed_bytes() {
            crate::knowledge_byte_codec::with_encoded(state, raw, max_bytes, consume)
        } else {
            consume(raw)
        }
    }
}

pub(crate) const CARRIER_NORMALIZED_COLUMNS_DDL: &str = r#"
ALTER TABLE knowledge_nodes ADD COLUMN payload_codec INTEGER NOT NULL DEFAULT 0 CHECK(payload_codec IN (0,1));
ALTER TABLE knowledge_nodes ADD COLUMN source_packet_sha256 BLOB CHECK((payload_codec=0 AND source_packet_sha256 IS NULL) OR (payload_codec=1 AND typeof(source_packet_sha256)='blob' AND length(source_packet_sha256)=32));
ALTER TABLE knowledge_relations ADD COLUMN payload_codec INTEGER NOT NULL DEFAULT 0 CHECK(payload_codec IN (0,1));
ALTER TABLE knowledge_relations ADD COLUMN source_packet_sha256 BLOB CHECK((payload_codec=0 AND source_packet_sha256 IS NULL) OR (payload_codec=1 AND typeof(source_packet_sha256)='blob' AND length(source_packet_sha256)=32));
"#;

pub const KNOWLEDGE_CARRIER_ONCE_MODEL_ABI: &str =
    tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4;

/// An exact byte reference issued by the retaining Stage; not source authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactSourceCarrierRef {
    packet_len: u64,
    packet_sha256: Digest256,
}
impl ExactSourceCarrierRef {
    pub fn packet_len(&self) -> u64 {
        self.packet_len
    }
    pub fn packet_sha256(&self) -> &Digest256 {
        &self.packet_sha256
    }
}

// Large opaque packets belong in table leaf pages. WITHOUT ROWID stores the
// whole packet in an index b-tree, spilling ordinary source rows much earlier;
// the separate narrow digest index preserves exact-key lookup without that
// amplification. Keep the former primary key's non-null requirement explicit.
pub(crate) const SOURCE_CARRIER_DDL: &str = r#"
CREATE TABLE knowledge_source_carriers(
 packet_sha256 BLOB PRIMARY KEY NOT NULL CHECK(length(packet_sha256)=32),
 packet_len INTEGER NOT NULL CHECK(packet_len>0),
 packet BLOB NOT NULL CHECK(length(packet) BETWEEN 18 AND packet_len+17));
"#;

// Field order is part of the ownership law: both byte buffers drop before
// the state hold on normal return, early refusal, or unwinding.
struct NormalizedPayloadRead<'s, 'budget> {
    codec: i64,
    logical_len: usize,
    digest: Digest256,
    raw: Vec<u8>,
    source: Vec<u8>,
    source_len: usize,
    _hold: crate::d1_public_capture::CreationStateHold<'s, 'budget>,
}

/// A successful codec1 read inside this Stage's BEGIN IMMEDIATE page, usable
/// only within that page and until its next SQL write. The transaction excludes
/// other writers; its generation and change counter detect local writes or
/// completed pages. CAS still binds the row and logical revision.
#[derive(Clone, Copy)]
pub(crate) struct SourceCarrierReadReceipt {
    stage_inode: (u64, u64),
    sql_changes: u64,
    page_generation: u64,
    relation: bool,
    previous: Digest256,
    source_len: usize,
    source_digest: Digest256,
}

/// One admitted representation of the already authenticated logical row.
/// The typed tree must be consumed synchronously inside its reader callback.
pub(crate) enum NormalizedLogical<'a> {
    Bytes(&'a [u8]),
    Value {
        value: serde_json::Value,
        logical_len: usize,
        digest: Digest256,
        source_receipt: Option<SourceCarrierReadReceipt>,
    },
}
impl NormalizedLogical<'_> {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Bytes(raw) => raw.len(),
            Self::Value { logical_len, .. } => *logical_len,
        }
    }
}

/// Metadata is authoritative scalar row data held through the payload scope.
/// `semantic_key` is node kind_id or relation predicate_id.
pub(crate) struct NormalizedRowMetadata<'row> {
    pub id: &'row str,
    pub source_graph: &'row str,
    pub native_id: Option<&'row str>,
    pub semantic_key: &'row str,
    pub source_order: i64,
    pub logical_digest: Digest256,
    pub raw_input_present: bool,
}
struct NormalizedCursorRow<'s, 'budget> {
    id: String,
    source_graph: String,
    native_id: Option<String>,
    semantic_key: String,
    source_order: i64,
    logical_digest: Digest256,
    raw_input_present: bool,
    _hold: crate::d1_public_capture::CreationStateHold<'s, 'budget>,
}

fn read_normalized_cursor_row<'s, 'budget>(
    state: &'s crate::d1_public_capture::CreationState<'budget>,
    row: &rusqlite::Row<'_>,
    after_order: Option<i64>,
) -> Result<NormalizedCursorRow<'s, 'budget>> {
    fn charged_text<'row>(
        state: &crate::d1_public_capture::CreationState<'_>,
        row: &'row rusqlite::Row<'_>,
        index: usize,
    ) -> Result<&'row str> {
        let raw = match row.get_ref(index)? {
            rusqlite::types::ValueRef::Text(raw) => raw,
            _ => return Err(Error::Invalid("normalized cursor text type/bound")),
        };
        state.charge_work(raw.len())?;
        std::str::from_utf8(raw).map_err(|_| Error::Invalid("normalized cursor UTF8"))
    }
    let source_order: i64 = row.get(0)?;
    let id = charged_text(state, row, 1)?;
    let graph = charged_text(state, row, 2)?;
    let native = match row.get_ref(3)? {
        rusqlite::types::ValueRef::Null => None,
        rusqlite::types::ValueRef::Text(v) => {
            state.charge_work(v.len())?;
            Some(
                std::str::from_utf8(v)
                    .map_err(|_| Error::Invalid("normalized cursor native UTF8"))?,
            )
        }
        _ => return Err(Error::Invalid("normalized cursor native type")),
    };
    let semantic = charged_text(state, row, 4)?;
    let sha = row
        .get_ref(5)?
        .as_blob()
        .map_err(|_| Error::Invalid("normalized cursor digest type/bound"))?;
    let native_valid: bool = row.get(6)?;
    let raw_input_present: bool = row.get(7)?;
    if !native_valid
        || source_order < 0
        || after_order.is_some_and(|after| source_order <= after)
        || sha.len() != 32
    {
        return Err(Error::Invalid("normalized cursor metadata differs"));
    }
    let held = id
        .len()
        .checked_add(graph.len())
        .and_then(|n| n.checked_add(native.map_or(0, str::len)))
        .and_then(|n| n.checked_add(semantic.len()))
        .and_then(|n| n.checked_add(std::mem::size_of::<NormalizedCursorRow<'_, '_>>()))
        .ok_or(Error::Budget("normalized cursor metadata state"))?;
    let hold = state.hold(held)?;
    state.charge_work(held)?;
    let mut digest = [0u8; 32];
    digest.copy_from_slice(sha);
    Ok(NormalizedCursorRow {
        id: id.to_owned(),
        source_graph: graph.to_owned(),
        native_id: native.map(str::to_owned),
        semantic_key: semantic.to_owned(),
        source_order,
        logical_digest: Digest256::from_bytes(digest),
        raw_input_present,
        _hold: hold,
    })
}

/// Physical staging is selected by the factory, independently of provenance
/// and the normalized payload ABI. Both profiles retain the caller's byte caps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StageStorageProfile {
    Persistent,
    NativeProjection,
}
impl StageStorageProfile {
    fn geometry(self) -> (&'static std::ffi::CStr, i64, i64) {
        match self {
            Self::Persistent => (
                c"PRAGMA main.page_size=4096; PRAGMA temp.page_size=4096",
                4096,
                4096,
            ),
            Self::NativeProjection => (
                c"PRAGMA main.page_size=16384; PRAGMA temp.page_size=16384",
                16384,
                16384,
            ),
        }
    }
}

pub(crate) struct PreparationSchema {
    pub(crate) main: &'static str,
    pub(crate) temporary: &'static str,
}

// One authored table/index definition, two static physical schemas. Indexes
// follow their table's schema under SQLite's CREATE INDEX rules.
macro_rules! preparation_statement {
    (main, table, $sql:literal) => {
        concat!("CREATE TABLE ", $sql, ";\n")
    };
    (temporary, table, $sql:literal) => {
        concat!("CREATE TEMP TABLE ", $sql, ";\n")
    };
    ($placement:ident, index, $sql:literal) => {
        concat!("CREATE INDEX ", $sql, ";\n")
    };
}
pub(crate) use preparation_statement;
macro_rules! preparation_schema {
    ($($kind:ident $sql:literal),+ $(,)?) => {
        $crate::knowledge_stage::PreparationSchema {
            main: concat!($($crate::knowledge_stage::preparation_statement!(main, $kind, $sql)),+),
            temporary: concat!($($crate::knowledge_stage::preparation_statement!(temporary, $kind, $sql)),+),
        }
    };
}
pub(crate) use preparation_schema;

pub struct KnowledgeStage<'a> {
    candidate: PathBuf,
    inode: (u64, u64),
    lease_path: PathBuf,
    lease_inode: (u64, u64),
    lease: Option<fs::File>,
    db: Option<Connection>,
    vm_used: Option<Arc<AtomicU64>>,
    controlled: Option<NativeStageOwnedBudget<'a>>,
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
    write_page_generation: u64,
    keep: bool,
    selected_full: bool,
    payload_layout: KnowledgePayloadLayout,
    storage_profile: StageStorageProfile,
    closed_input_rows: Option<u64>,
    fresh_selected: Option<PathBuf>,
}

#[derive(Debug)]
struct WritePageCharge {
    rows: u64,
    bytes: u64,
    max_rows: u64,
    max_bytes: u64,
    representation_rows: u64,
    representation_bytes: u64,
    max_representation_rows: u64,
    max_representation_bytes: u64,
}

impl<'a> KnowledgeStage<'a> {
    pub(crate) fn owned_creation_state(
        &self,
    ) -> Option<&'a crate::d1_public_capture::CreationState<'a>> {
        self.controlled.as_ref().map(|budget| budget.creation_state)
    }

    /// Retain one late normalization page while logical bytes, exact source
    /// packets and the caller's bounded output coexist. Codec reads keep their
    /// own transient holds; this covers only the copies escaping each callback.
    pub(crate) fn hold_normalized_page(
        &self,
        rows: usize,
        row_bytes: usize,
        extra_output_bytes_per_row: usize,
    ) -> Result<Option<crate::d1_public_capture::CreationStateHold<'a, 'a>>> {
        let Some(state) = self.owned_creation_state() else {
            return Ok(None);
        };
        if rows == 0 || rows > 1024 || row_bytes == 0 || row_bytes > 8 * 1024 * 1024 {
            return Err(Error::Budget("normalized late page limits"));
        }
        let bytes = row_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(5 * 4096 + 512))
            .and_then(|n| n.checked_add(extra_output_bytes_per_row))
            .and_then(|n| n.checked_mul(rows))
            .ok_or(Error::Budget("normalized late page state"))?;
        Ok(Some(state.hold(bytes)?))
    }

    /// Price only a planning/read traversal; physical rows/bytes are charged
    /// separately by their materialization owners, using the same original caps.
    pub(crate) fn charge_preparation_work(&mut self, bytes: u64) -> Result<()> {
        let result = (|| {
            if self.poisoned {
                return Err(Error::Invalid("Stage preparation poisoned"));
            }
            self.check(WritePhase::Normalized)?;
            let next = self
                .work_bytes
                .checked_add(bytes)
                .filter(|n| *n <= self.limits.sqlite.max_work_bytes)
                .ok_or(Error::Budget("Stage preparation work bytes"))?;
            self.charge_public_work(bytes)?;
            self.work_bytes = next;
            self.check(WritePhase::Normalized)?;
            Ok(())
        })();
        self.poisoned |= result.is_err();
        result
    }

    pub fn payload_layout(&self) -> KnowledgePayloadLayout {
        self.payload_layout
    }

    /// Select explicit static DDL for the storage profile. Provenance never
    /// selects physical placement, and schema text is not rewritten at runtime.
    pub(crate) fn create_preparation_tables(&mut self, schema: PreparationSchema) -> Result<()> {
        let sql = match self.storage_profile {
            StageStorageProfile::Persistent => schema.main,
            StageStorageProfile::NativeProjection => schema.temporary,
        };
        let result = (|| {
            if sql.len() > 64 * 1024 || sql.is_empty() {
                return Err(Error::Budget("preparation schema bytes"));
            }
            self.with_connection(WritePhase::Schema, |db| {
                db.execute_batch(sql)?;
                Ok(())
            })
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// Only the native full producer may opt in, before the first row. Other
    /// factories and existing callers retain their original inline layout.
    pub(crate) fn enable_carrier_once_layout(
        &mut self,
        layout: KnowledgePayloadLayout,
    ) -> Result<()> {
        let result = (|| {
            if self.poisoned
                || !matches!(
                    layout,
                    KnowledgePayloadLayout::CarrierOnceV2
                        | KnowledgePayloadLayout::CarrierOnceV3
                        | KnowledgePayloadLayout::CarrierOnceV4
                )
                || self.exact_receipt()?.binding.owner_profile
                    != "tos-native-projection-snapshot-v1"
                || self.storage_profile != StageStorageProfile::NativeProjection
                || self.total_rows != 0
                || self.closed_input_rows.is_some()
                || self.write_page.is_some()
                || !self.db().is_autocommit()
                || self.payload_layout != KnowledgePayloadLayout::InlineV1
            {
                return Err(Error::Invalid(
                    "carrier layout requires pristine native owner Stage",
                ));
            }
            self.check(WritePhase::Schema)?;
            let state = self.owned_creation_state().ok_or(Error::Invalid(
                "carrier layout requires same owned creation state",
            ))?;
            stage_batch_owned(self.db(), CARRIER_NORMALIZED_SCHEMA_C, state)?;
            self.charge_public_work(SOURCE_CARRIER_DDL.len() as u64)?;
            self.db().execute_batch(SOURCE_CARRIER_DDL)?;
            self.charge_public_work(CARRIER_NORMALIZED_COLUMNS_DDL.len() as u64)?;
            self.db().execute_batch(CARRIER_NORMALIZED_COLUMNS_DDL)?;
            self.check(WritePhase::Schema)?;
            if layout.dictionary_bytes() {
                self.charge_public_work(crate::knowledge_byte_dictionary::ddl(layout).len() as u64)?;
                self.db()
                    .execute_batch(crate::knowledge_byte_dictionary::ddl(layout))?;
                self.create_preparation_tables(
                    crate::knowledge_byte_dictionary::preparation_schema(layout),
                )?;
                for (_, drop_sql, create_sql) in COMPACT_ORDER_INDEXES {
                    self.charge_public_work((drop_sql.len() + create_sql.len()) as u64)?;
                    self.db().execute_batch(drop_sql)?;
                    self.db().execute_batch(create_sql)?;
                }
            }
            self.payload_layout = layout;
            Ok(())
        })();
        self.poisoned |= result.is_err();
        result
    }

    fn prepare_byte_dictionary(
        &mut self,
        kind: &str,
        graph: &str,
        raw: &[u8],
    ) -> Result<Option<crate::knowledge_byte_dictionary::OwnedDictionary<'a, 'a>>> {
        if !self.payload_layout.dictionary_bytes() {
            return Ok(None);
        }
        let state = self
            .owned_creation_state()
            .ok_or(Error::Invalid("dictionary producer owner absent"))?;
        let (dictionary, rows, bytes) = crate::knowledge_byte_dictionary::prepare_selected(
            self.db(),
            state,
            kind,
            graph,
            raw,
            self.payload_layout,
        )?;
        self.charge_representation(rows, bytes)?;
        Ok(dictionary)
    }

    /// Retain exact opaque packet bytes once. A digest hit must have identical
    /// bytes; no semantic JSON equality can share a different physical packet.
    /// Both a new carrier row and its bytes use the existing Stage/page ledger.
    pub fn retain_exact_source_carrier(&mut self, packet: &[u8]) -> Result<ExactSourceCarrierRef> {
        self.retain_exact_source_carrier_for_family("source", packet)
    }
    pub(crate) fn retain_exact_source_carrier_for_family(
        &mut self,
        family: &str,
        packet: &[u8],
    ) -> Result<ExactSourceCarrierRef> {
        let result = self
            .retain_exact_source_carrier_inner(family, packet)
            .map_err(|error| self.annotate_sqlite_full(WritePhase::Normalized, error));
        self.poisoned |= result.is_err();
        result
    }
    fn retain_exact_source_carrier_inner(
        &mut self,
        family: &str,
        packet: &[u8],
    ) -> Result<ExactSourceCarrierRef> {
        if self.poisoned {
            return Err(Error::Invalid("source carrier Stage poisoned"));
        }
        if !self.payload_layout.uses_carriers() {
            return Err(Error::Invalid("exact source carrier layout inactive"));
        }
        if packet.is_empty() || packet.len() > self.limits.sqlite.max_row_bytes {
            return Err(Error::Budget("exact source carrier packet bytes"));
        }
        self.check(WritePhase::Normalized)?;
        self.charge_public_work(packet.len() as u64)?;
        let digest = Digest256::of_bytes(packet);
        let layout = self.payload_layout;
        let state = self
            .owned_creation_state()
            .ok_or(Error::Invalid("source carrier same owner state absent"))?;
        // Read existing storage only under its selected logical/physical caps.
        let found = {
            let mut statement = self.db().prepare(
                "SELECT packet_len,CASE WHEN typeof(packet)='blob' AND packet_len=?2 AND length(packet)<=?2+17 THEN packet END FROM knowledge_source_carriers WHERE packet_sha256=?1",
            )?;
            let mut rows = statement.query(params![&digest.as_bytes()[..], packet.len() as i64])?;
            if let Some(row) = rows.next()? {
                let declared: i64 = row.get(0)?;
                let stored = row.get_ref(1)?.as_blob().map_err(|_| {
                    Error::Invalid("source carrier stored packet type or length differs")
                })?;
                self.charge_public_work(packet.len() as u64)?;
                if declared != packet.len() as i64 {
                    return Err(Error::Invalid("source carrier logical length differs"));
                }
                layout.with_sql_decoded(
                    self.db(),
                    state,
                    stored,
                    Some(packet.len()),
                    self.limits.sqlite.max_row_bytes,
                    |raw| {
                        state.charge_work(raw.len())?;
                        if raw != packet {
                            return Err(Error::Invalid(
                                "source carrier digest collision or stored bytes differ",
                            ));
                        }
                        Ok(())
                    },
                )?;
                true
            } else {
                false
            }
        };
        if !found {
            let dictionary = self.prepare_byte_dictionary("source", family, packet)?;
            layout.with_encoded_dictionary(state, dictionary.as_ref().map(|d|d.verified()).transpose()?, packet, self.limits.sqlite.max_row_bytes, |stored| {
                self.charge_materialized(1, packet.len() as u64)?;
                self.charge_representation(0, stored.len().saturating_sub(packet.len()) as u64)?;
                self.db().execute(
                    "INSERT INTO knowledge_source_carriers(packet_sha256,packet_len,packet) VALUES (?1,?2,?3)",
                    params![&digest.as_bytes()[..], packet.len() as i64, stored],
                )?;
                Ok(())
            })?;
        }
        self.check(WritePhase::Normalized)?;
        Ok(ExactSourceCarrierRef {
            packet_len: packet.len() as u64,
            packet_sha256: digest,
        })
    }

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

    fn registered_with_owned_state(
        &self,
        source_graph: &str,
        collection: &str,
        state: &crate::d1_public_capture::CreationState<'_>,
    ) -> Result<bool> {
        let equal = |left: &str, right: &str| -> Result<bool> {
            state.active()?;
            state.charge_work(
                left.len()
                    .checked_add(right.len())
                    .ok_or(Error::Budget("owned input registration comparison work"))?,
            )?;
            Ok(left == right)
        };
        state.active()?;
        if matches!(&self.receipt, StageInputReceipt::Candidate(_)) {
            for entry in self.receipt.collections() {
                if equal(&entry.source_graph, source_graph)?
                    && equal(&entry.collection, collection)?
                {
                    return Ok(true);
                }
            }
        } else {
            for (graph, collections) in &self.registrations {
                if equal(graph, source_graph)? {
                    for entry in collections {
                        if equal(entry, collection)? {
                            return Ok(true);
                        }
                    }
                    return Ok(false);
                }
            }
        }
        Ok(false)
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
            StageStorageProfile::Persistent,
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
            StageStorageProfile::Persistent,
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
            StageStorageProfile::Persistent,
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
            StageStorageProfile::Persistent,
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
            StageStorageProfile::NativeProjection,
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

    pub(crate) fn create_captured_native_snapshot_owned(
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
        remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
        heap: &'a Arc<sqlite_budget::DedicatedSessionSqliteHeap>,
        original_sql_limit: u64,
        creation_state: &'a crate::d1_public_capture::CreationState<'a>,
    ) -> Result<Self> {
        if receipt.binding.owner_profile != "tos-native-projection-snapshot-v1"
            || original_sql_limit == 0
        {
            return Err(Error::Invalid("owned native snapshot stage profile/VM"));
        }
        let budget = NativeStageOwnedBudget {
            remaining_after_retained,
            heap,
            creation_state,
            cancelled: Arc::clone(&cancelled),
            // Conservative original phase/session intersection; this absolute
            // shared ceiling does not restart when the Stage begins.
            original_sql_limit: original_sql_limit.min(limits.sqlite.max_sql_vm_steps),
            retained_rust_bytes: 0,
        };
        Self::create_inner_owned(
            candidate,
            limits,
            StageStorageProfile::NativeProjection,
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
            Some(budget),
        )
    }

    /// Disposable public-output staging. Its local inode/lease and SQLite
    /// limits are not a kernel aggregate-spill quota or selected admission.
    /// Only the compiler's full public D1 builder may invoke this entry.
    /// Existing prepare-only companion under its local byte/page profile;
    /// explicitly distinct from the dedicated owned full-public producer.
    pub(crate) fn create_prepared_public_build(
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
            StageStorageProfile::Persistent,
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

    pub(crate) fn create_public_build_owned(
        candidate: &Path,
        limits: StageLimits,
        receipt: ExactInputReceipt,
        owner: &'a dyn StageOwner,
        vm_used: Arc<AtomicU64>,
        work_used: Arc<AtomicU64>,
        cancelled: Arc<AtomicBool>,
        max_work_bytes: u64,
        deadline: Instant,
        remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
        creation_state: &'a crate::d1_public_capture::CreationState<'a>,
    ) -> Result<Self> {
        let budget = NativeStageOwnedBudget {
            remaining_after_retained,
            heap: creation_state.heap(),
            creation_state,
            cancelled: Arc::clone(&cancelled),
            original_sql_limit: creation_state
                .sql_vm_limit()
                .min(limits.sqlite.max_sql_vm_steps),
            retained_rust_bytes: 0,
        };
        Self::create_inner_owned(
            candidate,
            limits,
            StageStorageProfile::Persistent,
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
            Some(budget),
        )
    }

    fn create_inner(
        candidate: &Path,
        limits: StageLimits,
        storage_profile: StageStorageProfile,
        receipt: StageInputReceipt,
        owner: StageInputOwner<'a>,
        isolation: Option<&'a dyn StageIsolation>,
        shared_vm_used: Option<Arc<AtomicU64>>,
        public_work: Option<(PublicWorkLedger, u64)>,
        public_deadline: Option<Instant>,
    ) -> Result<Self> {
        Self::create_inner_owned(
            candidate,
            limits,
            storage_profile,
            receipt,
            owner,
            isolation,
            shared_vm_used,
            public_work,
            public_deadline,
            None,
        )
    }

    fn create_inner_owned(
        candidate: &Path,
        limits: StageLimits,
        storage_profile: StageStorageProfile,
        receipt: StageInputReceipt,
        owner: StageInputOwner<'a>,
        isolation: Option<&'a dyn StageIsolation>,
        shared_vm_used: Option<Arc<AtomicU64>>,
        public_work: Option<(PublicWorkLedger, u64)>,
        public_deadline: Option<Instant>,
        controlled: Option<NativeStageOwnedBudget<'a>>,
    ) -> Result<Self> {
        limits.validate()?;
        // Before receipt.validate's borrowed-key BTreeSet, registrations clones,
        // path/sidecar/fresh/lease copies and Connection construction.
        let mut controlled = controlled;
        if let Some(budget) = controlled.as_mut() {
            let path_bytes = candidate.as_os_str().as_encoded_bytes().len();
            if path_bytes > 8194 || receipt.collections().len() > MAX_COLLECTIONS {
                return Err(Error::Budget("owned native stage path/collections"));
            }
            let mut strings = 0usize;
            for entry in receipt.collections() {
                if entry.source_graph.len() > MAX_NAME_BYTES
                    || entry.collection.len() > MAX_NAME_BYTES
                {
                    return Err(Error::Budget("owned native stage registration field"));
                }
                strings = strings
                    .checked_add(entry.source_graph.len())
                    .and_then(|n| n.checked_add(entry.collection.len()))
                    .ok_or(Error::Budget("owned native stage registration strings"))?;
            }
            // BTree nodes hold 11 key/value slots and 12 child edges. Using one
            // entire node per entry also covers leaf/internal/minimum occupancy.
            let map_node = 11 * std::mem::size_of::<(String, BTreeSet<String>)>()
                + 16 * std::mem::size_of::<usize>();
            let set_node = 11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>();
            let seen_node =
                11 * std::mem::size_of::<(&String, &String)>() + 16 * std::mem::size_of::<usize>();
            let registrations = receipt
                .collections()
                .len()
                .checked_mul(map_node + set_node + seen_node)
                .and_then(|n| n.checked_add(strings));
            budget.retained_rust_bytes = registrations
                .and_then(|n| n.checked_add(16 * (path_bytes + 64)))
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<Self>()
                            + 2 * std::mem::size_of::<Connection>()
                            + 6 * std::mem::size_of::<usize>()
                            + sqlite_budget::SharedVmWindow::callback_state_upper_bound(),
                    )
                })
                .ok_or(Error::Budget("owned native stage Rust forecast"))?;
            budget.creation_state.retain(budget.retained_rust_bytes)?;
            budget.admit(0)?;
        }
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
            controlled,
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
            write_page_generation: 0,
            keep: false,
            selected_full: false,
            payload_layout: KnowledgePayloadLayout::InlineV1,
            storage_profile,
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
        let window = if let Some(budget) = &stage.controlled {
            budget.admit(0)?;
            Some(sqlite_budget::SharedVmWindow::reserve(
                Arc::clone(
                    shared_vm_used
                        .as_ref()
                        .ok_or(Error::Invalid("owned stage VM counter absent"))?,
                ),
                budget.original_sql_limit,
            )?)
        } else {
            None
        };
        let db = Connection::open(candidate)?;
        if let Some(window) = window {
            window.install(
                &db,
                public_deadline.ok_or(Error::Invalid("owned stage deadline absent"))?,
                Arc::clone(
                    &stage
                        .controlled
                        .as_ref()
                        .expect("owned stage budget")
                        .cancelled,
                ),
            );
        }
        stage.db = Some(db);
        // Apply the selected geometry before allocating pages or deriving the
        // page cap from the unchanged main-file byte limit.
        let (geometry_sql, main_page_bytes, temp_page_bytes) = storage_profile.geometry();
        stage.execute_phase_batch(geometry_sql, WritePhase::SqliteOpen)?;
        let applied: i64 = if let Some(state) = stage.owned_creation_state() {
            stage_integer_owned(stage.db(), c"PRAGMA main.page_size", state)?
        } else {
            stage
                .db()
                .query_row("PRAGMA main.page_size", [], |r| r.get(0))?
        };
        if applied != main_page_bytes {
            return Err(Error::Invalid("stage storage profile main page size"));
        }
        stage.vm_used = Some(if let Some(used) = shared_vm_used {
            if stage.controlled.is_some() {
                sqlite_budget::configure_prepaid_limits_with_owned_state(
                    stage.db(),
                    limits.sqlite,
                    stage
                        .owned_creation_state()
                        .ok_or(Error::Invalid("owned stage state absent"))?,
                )?;
            } else {
                sqlite_budget::configure_with_counter_until(
                    stage.db(),
                    limits.sqlite,
                    Arc::clone(&used),
                    public_deadline.ok_or(Error::Invalid("public D1 deadline absent"))?,
                )?;
            }
            used
        } else {
            sqlite_budget::configure(stage.db(), limits.sqlite)?
        });
        // This must precede every TEMP page allocation, including reading its
        // page geometry for the disposable public-build page cap.
        if let Some(state) = stage.owned_creation_state() {
            stage_batch_owned(stage.db(), c"PRAGMA temp.auto_vacuum=INCREMENTAL", state)?;
            if stage_integer_owned(stage.db(), c"PRAGMA temp.auto_vacuum", state)? != 2 {
                return Err(Error::Invalid("stage TEMP reclamation mode"));
            }
        } else {
            configure_stage_temp_reclamation(stage.db())?;
        }
        let applied: i64 = if let Some(state) = stage.owned_creation_state() {
            stage_integer_owned(stage.db(), c"PRAGMA temp.page_size", state)?
        } else {
            stage
                .db()
                .query_row("PRAGMA temp.page_size", [], |r| r.get(0))?
        };
        if applied != temp_page_bytes {
            return Err(Error::Invalid("stage storage profile TEMP page size"));
        }
        let disposable_native_inputs = storage_profile == StageStorageProfile::NativeProjection;
        if stage.public_build || disposable_native_inputs {
            if let Some(state) = stage.owned_creation_state() {
                configure_stage_temp_cap_owned(stage.db(), limits.max_temp_bytes, state)?;
            } else {
                let page_size: u64 = stage
                    .db()
                    .query_row("PRAGMA temp.page_size", [], |row| row.get(0))?;
                if page_size == 0 {
                    return Err(Error::Invalid("stage TEMP page size"));
                }
                let pages = limits.max_temp_bytes / page_size;
                if pages == 0 || pages > i64::MAX as u64 {
                    return Err(Error::Budget("stage TEMP page cap"));
                }
                let applied: i64 = stage.db().query_row(
                    &format!("PRAGMA temp.max_page_count={pages}"),
                    [],
                    |row| row.get(0),
                )?;
                if applied <= 0 || applied as u64 > pages {
                    return Err(Error::Invalid("stage TEMP page cap unavailable"));
                }
            }
        }
        stage.check(WritePhase::Schema)?;
        if let Some(state) = stage.owned_creation_state() {
            stage_batch_owned(
                stage.db(),
                if disposable_native_inputs {
                    NATIVE_SCHEMA_C
                } else {
                    SCHEMA_C
                },
                state,
            )?;
        } else if disposable_native_inputs {
            stage.charge_public_work(NATIVE_SCHEMA.len() as u64)?;
            stage.db().execute_batch(NATIVE_SCHEMA)?;
        } else {
            stage.db().execute_batch(SCHEMA)?;
        }
        stage.check(WritePhase::Schema)?;
        Ok(stage)
    }

    // Controlled SQL must not allocate a native error-message String before
    // its static refusal. The compatibility route retains its phase diagnosis.
    fn execute_phase_batch(&self, sql: &'static std::ffi::CStr, phase: WritePhase) -> Result<()> {
        if let Some(state) = self.owned_creation_state() {
            stage_batch_owned(self.db(), sql, state)
        } else {
            let text = sql
                .to_str()
                .map_err(|_| Error::Invalid("stage static SQL UTF8"))?;
            self.db()
                .execute_batch(text)
                .map_err(|error| Error::SqlitePhase { phase, error })
        }
    }

    fn db(&self) -> &Connection {
        self.db.as_ref().expect("stage database open")
    }
    fn check(&self, phase: WritePhase) -> Result<()> {
        if let Some(budget) = &self.controlled {
            budget.admit(0)?;
        }
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
        Self::check_isolation_with_owned_state(
            self.owned_creation_state(),
            self.isolation,
            &self.candidate,
            self.inode,
            &self.lease_path,
            self.lease_inode,
            self.limits,
            phase,
        )
    }
    fn check_isolation_with_owned_state(
        state: Option<&crate::d1_public_capture::CreationState<'_>>,
        isolation: Option<&dyn StageIsolation>,
        candidate: &Path,
        inode: (u64, u64),
        lease_path: &Path,
        lease_inode: (u64, u64),
        limits: StageLimits,
        phase: WritePhase,
    ) -> Result<()> {
        let _hold = if let Some(state) = state {
            let candidate_bytes = candidate.as_os_str().as_bytes().len();
            let lease_bytes = lease_path.as_os_str().as_bytes().len();
            // Calls are sequential, so a single maximum pathname scratch is
            // reused; both returned metadata records coexist until comparison.
            let scratch = candidate_bytes
                .max(lease_bytes)
                .checked_add(1)
                .and_then(|n| n.checked_add(2 * std::mem::size_of::<fs::Metadata>()))
                .ok_or(Error::Budget("owned stage isolation path workspace"))?;
            let hold = state.hold(scratch)?;
            state.charge_work(
                candidate_bytes
                    .checked_add(lease_bytes)
                    .ok_or(Error::Budget("owned stage isolation path work"))?,
            )?;
            state.active()?;
            Some(hold)
        } else {
            None
        };
        Self::check_isolation(
            isolation,
            candidate,
            inode,
            lease_path,
            lease_inode,
            limits,
            phase,
        )?;
        if let Some(state) = state {
            state.active()?;
        }
        Ok(())
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
    #[track_caller]
    pub(crate) fn with_connection<T>(
        &mut self,
        phase: WritePhase,
        f: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        if self.poisoned {
            return Err(Error::Invalid("stage poisoned by prior failure"));
        }
        let operation_caller = std::panic::Location::caller();
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
        let result = result.map_err(|error| {
            let error = self.annotate_sqlite_full(phase, error);
            if matches!(&error, Error::SqlitePhase { error: rusqlite::Error::SqliteFailure(code, _), .. } if code.code == rusqlite::ErrorCode::DiskFull) {
                eprintln!("Native stage failed operation at {}:{}", operation_caller.file(), operation_caller.line());
            }
            error
        });
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
        let state = self.owned_creation_state();
        // Guard is declared before the copied paths, including early errors.
        let _path_hold = if let Some(state) = state {
            let bytes = self
                .candidate
                .as_os_str()
                .as_bytes()
                .len()
                .checked_add(self.lease_path.as_os_str().as_bytes().len())
                .and_then(|n| n.checked_add(2 * std::mem::size_of::<PathBuf>()))
                .ok_or(Error::Budget("owned stage check callback paths"))?;
            let hold = state.hold(bytes)?;
            state.charge_work(self.candidate.as_os_str().as_bytes().len())?;
            state.charge_work(self.lease_path.as_os_str().as_bytes().len())?;
            state.active()?;
            Some(hold)
        } else {
            None
        };
        let candidate = self.candidate.clone();
        let inode = self.inode;
        let lease_path = self.lease_path.clone();
        let lease_inode = self.lease_inode;
        let limits = self.limits;
        self.with_connection(phase, |db| {
            f(db, &|| {
                Self::check_isolation_with_owned_state(
                    state,
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

    /// One bounded page of data rows. Stage separately reserves and charges
    /// the selected format's framing and dictionary writes; their combined
    /// physical maximum stays inside the existing page ceiling. The caller's
    /// data-row and byte allowance remains independently enforced.
    /// The closure uses
    /// the same Stage methods and connection, so its reads see earlier writes
    /// in this page. Previous committed pages remain independent. A caller
    /// may not commit an ignored row error: every Stage failure poisons it.
    #[track_caller]
    pub(crate) fn with_write_page<T>(
        &mut self,
        phase: WritePhase,
        max_rows: usize,
        max_bytes: u64,
        f: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let caller = std::panic::Location::caller();
        let result = (|| {
            if self.poisoned || self.write_page.is_some() {
                return Err(Error::Invalid("stage write page unavailable"));
            }
            if !matches!(phase, WritePhase::Normalized | WritePhase::Finalize) {
                return Err(Error::Invalid("stage write page phase"));
            }
            let (representation_rows, representation_bytes) = self.payload_write_overhead();
            let max_representation_rows = max_rows
                .checked_mul(representation_rows)
                .ok_or(Error::Budget("stage representation page rows"))?;
            let max_representation_bytes = (max_rows as u64)
                .checked_mul(representation_bytes)
                .ok_or(Error::Budget("stage representation page bytes"))?;
            if max_rows == 0
                || max_rows
                    .checked_add(max_representation_rows)
                    .is_none_or(|n| n > MAX_STAGE_PAGE_ROWS)
                || max_bytes == 0
                || max_bytes
                    .checked_add(max_representation_bytes)
                    .is_none_or(|n| n > MAX_STAGE_PAGE_BYTES)
            {
                return Err(Error::Budget("stage write page bounds"));
            }
            self.check(phase)?;
            if !self.db().is_autocommit() {
                return Err(Error::Invalid("stage write page nested transaction"));
            }
            let generation = self
                .write_page_generation
                .checked_add(1)
                .ok_or(Error::Budget("stage write page generation"))?;
            self.execute_phase_batch(c"BEGIN IMMEDIATE", phase)?;
            self.write_page_generation = generation;
            self.write_page = Some(WritePageCharge {
                rows: 0,
                bytes: 0,
                max_rows: max_rows as u64,
                max_bytes,
                representation_rows: 0,
                representation_bytes: 0,
                max_representation_rows: max_representation_rows as u64,
                max_representation_bytes,
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
                    if matches!(&error, Error::Budget(_)) {
                        eprintln!(
                            "Native stage page refused at {}:{} phase={phase:?} layout={:?} charges={:?}: {error}",
                            caller.file(),
                            caller.line(),
                            self.payload_layout,
                            self.write_page
                        );
                    }
                    let error = self.annotate_sqlite_full(phase, error);
                    // SQLite may already have aborted this transaction. Cleanup
                    // must not replace the failure that poisoned this page.
                    if !self.db().is_autocommit() {
                        let _ = self.execute_phase_batch(c"ROLLBACK", phase);
                    }
                    self.write_page = None;
                    return Err(error);
                }
            };
            if let Err(error) = self.execute_phase_batch(c"COMMIT", phase) {
                let error = self.annotate_sqlite_full(phase, error);
                if !self.db().is_autocommit() {
                    let _ = self.execute_phase_batch(c"ROLLBACK", phase);
                }
                self.write_page = None;
                return Err(error);
            }
            self.write_page = None;
            self.check(phase)?;
            Ok(value)
        })();
        self.poisoned |= result.is_err();
        result
    }

    // Failure-only observation; the original SQLite code/message survive.
    #[track_caller]
    fn annotate_sqlite_full(&self, phase: WritePhase, error: Error) -> Error {
        let (sqlite, original_phase) = match error {
            Error::Sql(error) => (error, None),
            Error::SqlitePhase { phase, error } => (error, Some(phase)),
            other => return other,
        };
        let wrap = |error| match original_phase {
            Some(phase) => Error::SqlitePhase { phase, error },
            None => Error::Sql(error),
        };
        let (failure, message) = match sqlite {
            rusqlite::Error::SqliteFailure(failure, message)
                if failure.extended_code & 0xff == rusqlite::ffi::SQLITE_FULL =>
            {
                (failure, message)
            }
            other => return wrap(other),
        };
        let caller = std::panic::Location::caller();
        eprintln!(
            "Native stage SQLITE_FULL observed at {}:{}",
            caller.file(),
            caller.line()
        );
        // Same shared progress handler, deadline and cancellation owner.
        let active = || self.check_public_work_active().is_ok();
        if !active() {
            return wrap(rusqlite::Error::SqliteFailure(failure, message));
        }
        let pager = |sql| {
            if !active() {
                None
            } else {
                self.db()
                    .query_row(sql, [], |row| row.get::<_, u64>(0))
                    .ok()
            }
        };
        let page_size = pager("PRAGMA main.page_size");
        let page_count = pager("PRAGMA main.page_count");
        let max_page_count = pager("PRAGMA main.max_page_count");
        let temp_page_size = pager("PRAGMA temp.page_size");
        let temp_page_count = pager("PRAGMA temp.page_count");
        let temp_max_page_count = pager("PRAGMA temp.max_page_count");
        // Failure-only physical geometry. The bundled DBSTAT aggregate cursor
        // visits each b-tree once and returns fixed schema metadata, never row
        // contents. It shares the original SQLite heap, VM/deadline and work
        // ledger; an unavailable diagnostic cannot replace SQLITE_FULL.
        if !message
            .as_deref()
            .is_some_and(|text| text.contains("stage_sqlite_full"))
        {
            for (schema, observed_pages, observed_size, byte_limit) in [
                (
                    "main",
                    page_count,
                    page_size,
                    self.limits.sqlite.max_output_bytes,
                ),
                (
                    "temp",
                    temp_page_count,
                    temp_page_size,
                    self.limits.max_temp_bytes,
                ),
            ] {
                if let Some(bytes) = observed_pages
                    .zip(observed_size)
                    .and_then(|(n, size)| n.checked_mul(size))
                {
                    if bytes <= byte_limit && self.charge_public_work(bytes).is_ok() {
                        let state = self.owned_creation_state();
                        let hold = state.map(|state| state.hold(4096)).transpose();
                        if let Ok(_hold) = hold {
                            let observed = (|| -> rusqlite::Result<()> {
                                let mut statement = self.db().prepare(
                                    if schema == "temp" {
                                        "SELECT name,pageno,pgsize,payload,unused FROM dbstat('temp') WHERE aggregate=TRUE LIMIT 65"
                                    } else {
                                        "SELECT name,pageno,pgsize,payload,unused FROM dbstat('main') WHERE aggregate=TRUE LIMIT 65"
                                    },
                                )?;
                                let mut rows = statement.query([])?;
                                for _ in 0..64 {
                                    if !active() {
                                        break;
                                    }
                                    let Some(row) = rows.next()? else {
                                        break;
                                    };
                                    let name = row.get_ref(0)?.as_str()?;
                                    if name.len() > 128
                                        || !name
                                            .bytes()
                                            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                                    {
                                        break;
                                    }
                                    let pages: u64 = row.get(1)?;
                                    let allocated: u64 = row.get(2)?;
                                    let payload: u64 = row.get(3)?;
                                    let unused: u64 = row.get(4)?;
                                    eprintln!(
                                        "Native stage geometry schema={schema} table={name} pages={pages} allocated_bytes={allocated} payload_bytes={payload} unused_bytes={unused}"
                                    );
                                }
                                Ok(())
                            })();
                            if observed.is_err() {
                                eprintln!(
                                    "Native stage geometry unavailable under original owner limits"
                                );
                            }
                        }
                    }
                }
            }
        }
        let mut file_bytes = None;
        let mut fs_total_bytes = None;
        let mut fs_free_bytes = None;
        let mut fs_available_bytes = None;
        let mut fs_free_inodes = None;
        if active() {
            if let Ok(file) =
                safe_open::open_regular(&self.candidate, self.limits.sqlite.max_output_bytes)
            {
                if active() {
                    if let Ok(metadata) = file.metadata() {
                        if metadata.file_type().is_file()
                            && (metadata.dev(), metadata.ino()) == self.inode
                        {
                            file_bytes = Some(metadata.len());
                            let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
                            if active()
                                && unsafe { libc::fstatvfs(file.as_raw_fd(), &mut stats) } == 0
                            {
                                fs_total_bytes = stats.f_blocks.checked_mul(stats.f_frsize);
                                fs_free_bytes = stats.f_bfree.checked_mul(stats.f_frsize);
                                fs_available_bytes = stats.f_bavail.checked_mul(stats.f_frsize);
                                fs_free_inodes = Some(stats.f_ffree);
                            }
                        }
                    }
                }
            }
        }
        if !active() {
            return wrap(rusqlite::Error::SqliteFailure(failure, message));
        }
        let details = format!(
            "{}; stage_sqlite_full phase={phase:?}; extended_code={}; page_size={page_size:?}; page_count={page_count:?}; max_page_count={max_page_count:?}; temp_page_size={temp_page_size:?}; temp_page_count={temp_page_count:?}; temp_max_page_count={temp_max_page_count:?}; file_bytes={file_bytes:?}; selected_output_bytes={}; selected_temp_bytes={}; fs_total_bytes={fs_total_bytes:?}; fs_free_bytes={fs_free_bytes:?}; fs_available_bytes={fs_available_bytes:?}; fs_free_inodes={fs_free_inodes:?}; observation=after_failure_before_owned_cleanup; attempted_allocation=unknown",
            message.as_deref().unwrap_or("database or disk is full"),
            failure.extended_code,
            self.limits.sqlite.max_output_bytes,
            self.limits.max_temp_bytes,
        );
        wrap(rusqlite::Error::SqliteFailure(failure, Some(details)))
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
            let (count, root) =
                input_root_with_state(self.db(), entry, self.owned_creation_state())?;
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
                db.execute_batch(
                    "PRAGMA secure_delete=ON; PRAGMA temp.secure_delete=ON; DROP TABLE raw_records",
                )?;
                Ok(())
            })?;
            if self.payload_layout.dictionary_bytes() {
                self.with_connection(WritePhase::Finalize, |db| {
                    db.execute_batch("DROP TABLE knowledge_byte_dictionary_pending")?;
                    Ok(())
                })?;
            }
            self.closed_input_rows = Some(rows);
            Ok(())
        })();
        self.poisoned |= result.is_err();
        result
    }

    pub(crate) fn core_roots(&mut self) -> Result<CoreRoots> {
        let creation = self.owned_creation_state();
        self.with_connection(WritePhase::Sort, |db| {
            let (nodes, node_sha256) = output_root_with_state(db, "knowledge_nodes", creation)?;
            let (relations, relation_sha256) =
                output_root_with_state(db, "knowledge_relations", creation)?;
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
        self.charge_materialized_kind(rows, bytes, false)
    }
    fn charge_representation(&mut self, rows: u64, bytes: u64) -> Result<()> {
        self.charge_materialized_kind(rows, bytes, true)
    }
    fn charge_materialized_kind(
        &mut self,
        rows: u64,
        bytes: u64,
        representation: bool,
    ) -> Result<()> {
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
            if representation {
                if let Some(page) = self.write_page.as_mut() {
                    page.representation_rows = page
                        .representation_rows
                        .checked_add(rows)
                        .ok_or(Error::Budget("stage representation page rows"))?;
                    page.representation_bytes = page
                        .representation_bytes
                        .checked_add(bytes)
                        .ok_or(Error::Budget("stage representation page bytes"))?;
                    if page.representation_rows > page.max_representation_rows
                        || page.representation_bytes > page.max_representation_bytes
                    {
                        return Err(Error::Budget("stage representation page rows/bytes"));
                    }
                }
            } else {
                self.charge_write_page(rows, bytes)?;
            }
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
        let result = self
            .ingest_input_inner(row)
            .map_err(|error| self.annotate_sqlite_full(WritePhase::Input, error));
        self.poisoned |= result.is_err();
        result
    }
    /// The existing row/byte ceilings for an atomic raw-input chunk.
    pub(crate) fn input_batch_limits(&self) -> (usize, u64) {
        (self.limits.max_seek_rows, self.limits.max_seek_bytes)
    }
    fn payload_write_overhead(&self) -> (usize, u64) {
        let framing = if self.payload_layout.packed_bytes() {
            crate::knowledge_byte_codec::HEADER as u64
        } else {
            0
        };
        if self.payload_layout.dictionary_bytes() {
            (
                crate::knowledge_byte_dictionary::MAX_WRITE_ROWS,
                framing + crate::knowledge_byte_dictionary::write_bytes(self.payload_layout) as u64,
            )
        } else {
            (0, framing)
        }
    }
    /// Data-row allowance after reserving format-owned physical writes. The
    /// combined rows/bytes never exceed the unchanged private Stage ceiling.
    pub(crate) fn write_page_limits(&self) -> (usize, u64) {
        let (extra_rows, extra_bytes) = self.payload_write_overhead();
        let rows = MAX_STAGE_PAGE_ROWS / (1 + extra_rows);
        (rows, MAX_STAGE_PAGE_BYTES - rows as u64 * extra_bytes)
    }
    /// Price a logical row and its optional exact source packet before seeking
    /// the input page. Reused packets consume less than this physical bound.
    pub(crate) fn exact_source_write_page_limits(
        &self,
        requested_rows: usize,
        max_payload_bytes: usize,
        max_source_bytes: usize,
    ) -> Result<(usize, usize, u64)> {
        let carrier = self.payload_layout.uses_carriers();
        let rows_per_input = if carrier { 2 } else { 1 };
        let bytes_per_input = max_payload_bytes
            .checked_add(if carrier { max_source_bytes } else { 0 })
            .filter(|bytes| *bytes > 0)
            .ok_or(Error::Budget("exact source physical row bytes"))?;
        let (write_rows, write_bytes) = self.write_page_limits();
        let byte_cap = usize::try_from(write_bytes)
            .map_err(|_| Error::Budget("exact source page byte conversion"))?;
        let rows = requested_rows
            .min(write_rows / rows_per_input)
            .min(byte_cap / bytes_per_input);
        if rows == 0 {
            return Err(Error::Budget(
                "exact source physical page cannot hold one row",
            ));
        }
        Ok((rows, rows * rows_per_input, (rows * bytes_per_input) as u64))
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
        let result = result.map_err(|error| self.annotate_sqlite_full(WritePhase::Input, error));
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
        if let Some(state) = self.owned_creation_state() {
            stage_insert_owned(
                self.db(),
                c"INSERT INTO raw_records VALUES (?1,?2,?3,?4,?5,?6)",
                &[
                    StageSqlBinding::Text(row.source_graph),
                    StageSqlBinding::Text(row.collection),
                    StageSqlBinding::Text(row.id),
                    StageSqlBinding::Integer(row.payload.len() as i64),
                    StageSqlBinding::Blob(digest.as_bytes()),
                    StageSqlBinding::Blob(row.payload),
                ],
                state,
            )?;
            return self.check(WritePhase::Input);
        }
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
    /// Same-owner physical write from an actual supplied source carrier.
    /// Logical metadata/digest remain unchanged; any error poisons this Stage.
    pub(crate) fn insert_node_with_exact_source(
        &mut self,
        row: NodeRow<'_>,
        source: &[u8],
    ) -> Result<()> {
        if self.payload_layout == KnowledgePayloadLayout::InlineV1 {
            return self.insert_node(row);
        }
        let result = (|| {
            if self.poisoned {
                return Err(Error::Invalid("poisoned carrier Stage"));
            }
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("carrier same owner state absent"))?;
            let cap = usize::try_from(self.limits.sqlite.max_row_bytes)
                .map_err(|_| Error::Budget("carrier row bound conversion"))?;
            let limits = crate::knowledge_normalization::SourceRow::json_limits(cap)?;
            self.charge_preparation_work(row.payload.len() as u64)?;
            state.charge_work(row.payload.len())?;
            let digest = Digest256::of_bytes(row.payload);
            crate::knowledge_payload_codec::with_factored_payload(
                state,
                row.payload,
                source,
                limits,
                limits,
                limits,
                cap,
                |stored, source_digest| {
                    let reference =
                        self.retain_exact_source_carrier_for_family(row.source_graph, source)?;
                    if reference.packet_sha256() != &source_digest {
                        return Err(Error::Invalid("carrier source reference differs"));
                    }
                    self.insert_node_storage_inner(
                        NodeRow {
                            id: row.id,
                            source_graph: row.source_graph,
                            native_id: row.native_id,
                            entity_id: row.entity_id,
                            kind_id: row.kind_id,
                            type_id: row.type_id,
                            source_order: row.source_order,
                            payload: stored,
                        },
                        Some((row.payload.len(), digest, source_digest)),
                    )
                },
            )
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// Point reads expose only verified logical bytes. The original state holds
    /// selected physical bytes, source bytes and hydrated encoding together;
    /// SQL statements close before a consumer borrows this Stage mutably.
    /// A returned owned result must have separate caller admission.
    pub(crate) fn with_node_payload_owned<T>(
        &mut self,
        id: &str,
        max_bytes: usize,
        consume: impl FnOnce(&mut Self, &[u8]) -> Result<T>,
    ) -> Result<Option<T>> {
        self.with_normalized_payload_owned(false, id, max_bytes, |stage, logical, _| {
            consume(stage, logical)
        })
    }
    pub(crate) fn with_relation_payload_owned<T>(
        &mut self,
        id: &str,
        max_bytes: usize,
        consume: impl FnOnce(&mut Self, &[u8]) -> Result<T>,
    ) -> Result<Option<T>> {
        self.with_normalized_payload_owned(true, id, max_bytes, |stage, logical, _| {
            consume(stage, logical)
        })
    }
    /// Keep exact source spelling for late normalized updates. Inline rows
    /// have no external source owner; Carrier rows supply the held raw packet.
    pub(crate) fn with_node_payload_source_owned<T>(
        &mut self,
        id: &str,
        max_bytes: usize,
        consume: impl FnOnce(&mut Self, &[u8], Option<&[u8]>) -> Result<T>,
    ) -> Result<Option<T>> {
        self.with_normalized_payload_owned(false, id, max_bytes, consume)
    }
    pub(crate) fn with_relation_payload_source_owned<T>(
        &mut self,
        id: &str,
        max_bytes: usize,
        consume: impl FnOnce(&mut Self, &[u8], Option<&[u8]>) -> Result<T>,
    ) -> Result<Option<T>> {
        self.with_normalized_payload_owned(true, id, max_bytes, consume)
    }

    fn with_normalized_payload_owned<T>(
        &mut self,
        relation: bool,
        id: &str,
        max_bytes: usize,
        consume: impl FnOnce(&mut Self, &[u8], Option<&[u8]>) -> Result<T>,
    ) -> Result<Option<T>> {
        self.with_normalized_payload_decoded_owned(
            relation,
            id,
            max_bytes,
            false,
            |stage, logical, source| match logical {
                NormalizedLogical::Bytes(raw) => consume(stage, raw, source),
                NormalizedLogical::Value { .. } => {
                    Err(Error::Invalid("byte reader representation differs"))
                }
            },
        )
    }

    pub(crate) fn with_normalized_payload_decoded_owned<T>(
        &mut self,
        relation: bool,
        id: &str,
        max_bytes: usize,
        typed: bool,
        consume: impl FnOnce(&mut Self, NormalizedLogical<'_>, Option<&[u8]>) -> Result<T>,
    ) -> Result<Option<T>> {
        let result = (|| {
            if self.poisoned {
                return Err(Error::Invalid("poisoned normalized payload read"));
            }
            valid_id(id)?;
            let stage_cap = usize::try_from(self.limits.sqlite.max_row_bytes)
                .map_err(|_| Error::Budget("normalized Stage row conversion"))?;
            // Both values are upper bounds, not requested allocations. A
            // consumer may accept larger rows than this selected Stage does;
            // apply the stricter bound to the actual row below.
            let max_bytes = max_bytes.min(stage_cap);
            if max_bytes == 0 {
                return Err(Error::Budget("normalized payload read cap"));
            }
            // This reference is the same retained owner, not a fresh allowance.
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("normalized payload owner absent"))?;
            let layout = self.payload_layout;
            let carrier = layout.uses_carriers();
            let sql = match (relation, carrier) {
                (false, false) => {
                    "SELECT 0,payload_len,payload_sha256,payload,NULL,0 FROM knowledge_nodes WHERE id=?1"
                }
                (true, false) => {
                    "SELECT 0,payload_len,payload_sha256,payload,NULL,0 FROM knowledge_relations WHERE id=?1"
                }
                (false, true) => {
                    "SELECT p.payload_codec,p.payload_len,p.payload_sha256,p.payload,CASE WHEN p.payload_codec=1 AND typeof(c.packet)='blob' AND length(c.packet)<=c.packet_len+17 AND c.packet_len>0 AND c.packet_len<=?2 AND c.packet_sha256=p.source_packet_sha256 THEN c.packet END,c.packet_len FROM knowledge_nodes p LEFT JOIN knowledge_source_carriers c ON c.packet_sha256=p.source_packet_sha256 WHERE p.id=?1"
                }
                (true, true) => {
                    "SELECT p.payload_codec,p.payload_len,p.payload_sha256,p.payload,CASE WHEN p.payload_codec=1 AND typeof(c.packet)='blob' AND length(c.packet)<=c.packet_len+17 AND c.packet_len>0 AND c.packet_len<=?2 AND c.packet_sha256=p.source_packet_sha256 THEN c.packet END,c.packet_len FROM knowledge_relations p LEFT JOIN knowledge_source_carriers c ON c.packet_sha256=p.source_packet_sha256 WHERE p.id=?1"
                }
            };
            let record = self.with_connection(WritePhase::Sort, |db| {
                let mut statement = db.prepare(sql)?;
                let mut rows = if carrier {
                    statement.query(params![id, max_bytes as i64])?
                } else {
                    statement.query(params![id])?
                };
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };
                let codec: i64 = row.get(0)?;
                let logical_len: i64 = row.get(1)?;
                let sha = row
                    .get_ref(2)?
                    .as_blob()
                    .map_err(|_| Error::Invalid("normalized digest type"))?;
                let raw = row
                    .get_ref(3)?
                    .as_blob()
                    .map_err(|_| Error::Invalid("normalized payload type"))?;
                if !matches!(codec, 0 | 1)
                    || logical_len <= 0
                    || logical_len as u64 > max_bytes as u64
                    || sha.len() != 32
                    || raw.is_empty()
                    || raw.len() > layout.physical_bound(max_bytes)?
                {
                    return Err(Error::Budget("normalized payload transfer bound"));
                }
                let source = if codec == 1 {
                    row.get_ref(4)?
                        .as_blob()
                        .map_err(|_| Error::Invalid("normalized source carrier absent"))?
                } else {
                    &[]
                };
                if codec == 0 && !layout.packed_bytes() && raw.len() != logical_len as usize {
                    return Err(Error::Invalid("normalized inline length differs"));
                }
                let source_len = if codec == 1 {
                    usize::try_from(row.get::<_, i64>(5)?)
                        .map_err(|_| Error::Invalid("normalized source logical length"))?
                } else {
                    0
                };
                let owned_bytes = raw
                    .len()
                    .checked_add(source.len())
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<NormalizedPayloadRead<'_, '_>>())
                    })
                    .ok_or(Error::Budget("normalized payload held transfer state"))?;
                let hold = state.hold(owned_bytes)?;
                fn copy(
                    state: &crate::d1_public_capture::CreationState<'_>,
                    raw: &[u8],
                ) -> Result<Vec<u8>> {
                    state.active()?;
                    let mut bytes = Vec::with_capacity(raw.len());
                    for part in raw.chunks(4096) {
                        state.charge_work(part.len())?;
                        bytes.extend_from_slice(part);
                    }
                    state.active()?;
                    Ok(bytes)
                }
                let mut digest = [0u8; 32];
                digest.copy_from_slice(sha);
                let raw = copy(state, raw)?;
                let source = copy(state, source)?;
                Ok(Some(NormalizedPayloadRead {
                    codec,
                    logical_len: logical_len as usize,
                    digest: Digest256::from_bytes(digest),
                    raw,
                    source,
                    source_len,
                    _hold: hold,
                }))
            })?;
            let Some(record) = record else {
                return Ok(None);
            };
            let dictionary = layout.read_dictionary(self.db(), state, &record.raw, max_bytes)?;
            let source_dictionary = if record.codec == 1 {
                layout.read_dictionary(self.db(), state, &record.source, max_bytes)?
            } else {
                None
            };
            let observed = layout.with_decoded_dictionary(
                state,
                &record.raw,
                dictionary.as_ref().map(|d| d.verified()).transpose()?,
                (record.codec == 0).then_some(record.logical_len),
                max_bytes,
                |raw| {
                    if record.codec == 0 {
                        state.charge_work(raw.len())?;
                        if Digest256::of_bytes(raw) != record.digest {
                            return Err(Error::Invalid("normalized inline digest differs"));
                        }
                        state.active()?;
                        if typed {
                            let limits =
                                crate::knowledge_normalization::SourceRow::json_limits(max_bytes)?;
                            state.with_serde_owned_value_with_limits(raw, limits, |value| {
                                consume(
                                    self,
                                    NormalizedLogical::Value {
                                        value,
                                        logical_len: record.logical_len,
                                        digest: record.digest,
                                        source_receipt: None,
                                    },
                                    None,
                                )
                                .map(Some)
                            })
                        } else {
                            consume(self, NormalizedLogical::Bytes(raw), None).map(Some)
                        }
                    } else {
                        let limits =
                            crate::knowledge_normalization::SourceRow::json_limits(max_bytes)?;
                        layout.with_decoded_dictionary(
                            state,
                            &record.source,
                            source_dictionary
                                .as_ref()
                                .map(|d| d.verified())
                                .transpose()?,
                            Some(record.source_len),
                            max_bytes,
                            |source| {
                                if typed {
                                    crate::knowledge_payload_codec::with_hydrated_value_payload(
                                        state,
                                        raw,
                                        source,
                                        limits,
                                        limits,
                                        max_bytes,
                                        record.logical_len,
                                        record.digest,
                                        |value, source_digest| {
                                            // No write has occurred between this
                                            // owned SQL read and authentication.
                                            state.charge_work(std::mem::size_of::<
                                                SourceCarrierReadReceipt,
                                            >(
                                            ))?;
                                            let source_receipt = (self.write_page.is_some()
                                                && !self.db().is_autocommit())
                                            .then(|| SourceCarrierReadReceipt {
                                                stage_inode: self.inode,
                                                sql_changes: self.db().total_changes(),
                                                page_generation: self.write_page_generation,
                                                relation,
                                                previous: record.digest,
                                                source_len: source.len(),
                                                source_digest,
                                            });
                                            consume(
                                                self,
                                                NormalizedLogical::Value {
                                                    value,
                                                    logical_len: record.logical_len,
                                                    digest: record.digest,
                                                    source_receipt,
                                                },
                                                Some(source),
                                            )
                                            .map(Some)
                                        },
                                    )
                                } else {
                                    crate::knowledge_payload_codec::with_hydrated_payload(
                                        state,
                                        raw,
                                        source,
                                        limits,
                                        limits,
                                        max_bytes,
                                        record.logical_len,
                                        record.digest,
                                        |logical| {
                                            consume(
                                                self,
                                                NormalizedLogical::Bytes(logical),
                                                Some(source),
                                            )
                                            .map(Some)
                                        },
                                    )
                                }
                            },
                        )
                    }
                },
            );
            drop(record);
            observed
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// Raw membership belongs only to the open-input phase. Closed late
    /// consumers must never prepare SQL naming the already-dropped raw table.
    fn normalized_raw_membership_owned(
        &mut self,
        relation: bool,
        cursor: &NormalizedCursorRow<'_, '_>,
        state: &crate::d1_public_capture::CreationState<'_>,
    ) -> Result<bool> {
        state.active()?;
        self.check(WritePhase::Sort)?;
        if self.closed_input_rows.is_some() {
            return Ok(false);
        }
        let Some(native) = cursor.native_id.as_deref() else {
            return Ok(false);
        };
        const SQL: &std::ffi::CStr = c"SELECT EXISTS(SELECT 1 FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3)";
        let collection = if relation { "edges" } else { "nodes" };
        let work = SQL
            .to_bytes()
            .len()
            .checked_add(cursor.source_graph.len())
            .and_then(|n| n.checked_add(collection.len()))
            .and_then(|n| n.checked_add(native.len()))
            .ok_or(Error::Budget("raw membership work"))?;
        state.charge_work(work)?;
        type Frame<'s, 'a, 'b> = (
            &'s mut KnowledgeStage<'b>,
            (usize, usize),
            &'s NormalizedCursorRow<'a, 'b>,
            &'s crate::d1_public_capture::CreationState<'b>,
            bool,
            &'static std::ffi::CStr,
            &'static str,
            &'s str,
            i64,
            Result<bool>,
        );
        let geometry =
            tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound()
                .checked_add(std::mem::size_of::<Frame<'_, '_, '_>>())
                .ok_or(Error::Budget("raw membership controller geometry"))?;
        let _hold = state.hold(geometry)?;
        let mut statement =
            tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(self.db(), SQL)
                .map_err(carrier_schema_sql_error)?;
        statement
            .bind_text(1, &cursor.source_graph)
            .map_err(carrier_schema_sql_error)?;
        statement
            .bind_text(2, collection)
            .map_err(carrier_schema_sql_error)?;
        statement
            .bind_text(3, native)
            .map_err(carrier_schema_sql_error)?;
        state.active()?;
        if !statement.step().map_err(carrier_schema_sql_error)? {
            return Err(Error::Invalid("raw membership scalar absent"));
        }
        state.active()?;
        let present = statement.integer(0).map_err(carrier_schema_sql_error)?;
        if !(0..=1).contains(&present) || statement.step().map_err(carrier_schema_sql_error)? {
            return Err(Error::Invalid("raw membership scalar differs"));
        }
        drop(statement);
        state.active()?;
        self.check(WritePhase::Sort)?;
        Ok(present == 1)
    }

    /// Stream a bounded normalized page in the maintained global source order.
    /// Only one scalar cursor row and its exact logical/source payload are live.
    /// SQL closes before consume; no intermediate Vec<Page> escapes custody.
    pub(crate) fn with_normalized_rows_owned(
        &mut self,
        relation: bool,
        after_order: i64,
        max_rows: usize,
        max_bytes: usize,
        mut consume: impl FnMut(
            &mut Self,
            &NormalizedRowMetadata<'_>,
            &[u8],
            Option<&[u8]>,
        ) -> Result<()>,
    ) -> Result<(usize, Option<i64>)> {
        self.with_normalized_rows_decoded_owned(
            relation,
            after_order,
            max_rows,
            max_bytes,
            false,
            |stage, metadata, logical, source| match logical {
                NormalizedLogical::Bytes(raw) => consume(stage, metadata, raw, source),
                NormalizedLogical::Value { .. } => {
                    Err(Error::Invalid("byte cursor representation differs"))
                }
            },
        )
    }

    pub(crate) fn with_normalized_rows_decoded_owned(
        &mut self,
        relation: bool,
        after_order: i64,
        max_rows: usize,
        max_bytes: usize,
        typed: bool,
        mut consume: impl FnMut(
            &mut Self,
            &NormalizedRowMetadata<'_>,
            NormalizedLogical<'_>,
            Option<&[u8]>,
        ) -> Result<()>,
    ) -> Result<(usize, Option<i64>)> {
        let result = (|| {
            if self.poisoned
                || max_rows == 0
                || max_rows > self.limits.max_seek_rows
                || after_order < -1
            {
                return Err(Error::Budget("normalized cursor page admission"));
            }
            let cap = usize::try_from(self.limits.sqlite.max_row_bytes)
                .map_err(|_| Error::Budget("normalized cursor row conversion"))?;
            let max_bytes = max_bytes.min(cap);
            if max_bytes == 0 {
                return Err(Error::Budget("normalized cursor row cap"));
            }
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("normalized cursor same owner absent"))?;
            let sql = if relation {
                "SELECT source_order,CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 4096 THEN id END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB)) BETWEEN 1 AND 4096 THEN source_graph END,CASE WHEN native_id IS NULL OR (typeof(native_id)='text' AND length(CAST(native_id AS BLOB)) BETWEEN 1 AND 4096) THEN native_id END,CASE WHEN typeof(predicate_id)='text' AND length(CAST(predicate_id AS BLOB)) BETWEEN 1 AND 4096 THEN predicate_id END,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 END,native_id IS NULL OR (typeof(native_id)='text' AND length(CAST(native_id AS BLOB)) BETWEEN 1 AND 4096),0 FROM knowledge_relations WHERE source_order>?1 ORDER BY source_order,id LIMIT 1"
            } else {
                "SELECT source_order,CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 4096 THEN id END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB)) BETWEEN 1 AND 4096 THEN source_graph END,CASE WHEN native_id IS NULL OR (typeof(native_id)='text' AND length(CAST(native_id AS BLOB)) BETWEEN 1 AND 4096) THEN native_id END,CASE WHEN typeof(kind_id)='text' AND length(CAST(kind_id AS BLOB)) BETWEEN 1 AND 4096 THEN kind_id END,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 END,native_id IS NULL OR (typeof(native_id)='text' AND length(CAST(native_id AS BLOB)) BETWEEN 1 AND 4096),0 FROM knowledge_nodes WHERE source_order>?1 ORDER BY source_order,id LIMIT 1"
            };
            let mut after = after_order;
            let mut count = 0usize;
            let mut bytes = 0u64;
            while count < max_rows {
                state.active()?;
                let cursor = self.with_connection(WritePhase::Sort, |db| {
                    let mut statement = db.prepare(sql)?;
                    let mut rows = statement.query(params![after])?;
                    let Some(row) = rows.next()? else {
                        return Ok(None);
                    };
                    Ok(Some(read_normalized_cursor_row(state, row, Some(after))?))
                })?;
                let Some(mut cursor) = cursor else {
                    return Ok((count, None));
                };
                cursor.raw_input_present =
                    self.normalized_raw_membership_owned(relation, &cursor, state)?;
                let metadata = NormalizedRowMetadata {
                    id: &cursor.id,
                    source_graph: &cursor.source_graph,
                    native_id: cursor.native_id.as_deref(),
                    semantic_key: &cursor.semantic_key,
                    source_order: cursor.source_order,
                    logical_digest: cursor.logical_digest,
                    raw_input_present: cursor.raw_input_present,
                };
                let delivered = self.with_normalized_payload_decoded_owned(
                    relation,
                    &cursor.id,
                    max_bytes,
                    typed,
                    |stage, logical, source| {
                        bytes = bytes
                            .checked_add(logical.len() as u64)
                            .ok_or(Error::Budget("normalized cursor page bytes"))?;
                        if bytes > stage.limits.max_seek_bytes {
                            return Err(Error::Budget("normalized cursor page bytes"));
                        }
                        // The typed reader already authenticated its exact serialized
                        // bytes before moving the admitted tree. Bind that digest to
                        // this cursor without serializing or reparsing it again.
                        let digest = match &logical {
                            NormalizedLogical::Bytes(raw) => {
                                state.charge_work(raw.len())?;
                                Digest256::of_bytes(raw)
                            }
                            NormalizedLogical::Value { digest, .. } => *digest,
                        };
                        if digest != metadata.logical_digest {
                            return Err(Error::Invalid(
                                "normalized cursor payload revision changed",
                            ));
                        }
                        consume(stage, &metadata, logical, source)
                    },
                )?;
                if delivered.is_none() {
                    return Err(Error::Invalid("normalized cursor row disappeared"));
                }
                after = cursor.source_order;
                count += 1;
                drop(cursor);
            }
            Ok((count, Some(after)))
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// One source-scoped lexical ID step for maintained late Node consumers.
    /// Progress is admitted by the caller inside consume; no String escapes.
    pub(crate) fn with_next_normalized_node_by_id_owned(
        &mut self,
        source_graph: &str,
        after_id: Option<&str>,
        max_bytes: usize,
        consume: impl FnOnce(&mut Self, &NormalizedRowMetadata<'_>, &[u8], Option<&[u8]>) -> Result<()>,
    ) -> Result<bool> {
        let result = (|| {
            if self.poisoned || self.limits.max_seek_rows == 0 {
                return Err(Error::Invalid("normalized ID cursor unavailable"));
            }
            valid_id(source_graph)?;
            if let Some(after) = after_id {
                valid_id(after)?;
            }
            let cap = usize::try_from(self.limits.sqlite.max_row_bytes)
                .map_err(|_| Error::Budget("normalized ID cursor row conversion"))?;
            let max_bytes = max_bytes
                .min(cap)
                .min(self.limits.max_seek_bytes.min(usize::MAX as u64) as usize);
            if max_bytes == 0 {
                return Err(Error::Budget("normalized ID cursor row cap"));
            }
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("normalized ID cursor same owner absent"))?;
            let sql = "SELECT source_order,CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 4096 THEN id END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB)) BETWEEN 1 AND 4096 THEN source_graph END,CASE WHEN native_id IS NULL OR (typeof(native_id)='text' AND length(CAST(native_id AS BLOB)) BETWEEN 1 AND 4096) THEN native_id END,CASE WHEN typeof(kind_id)='text' AND length(CAST(kind_id AS BLOB)) BETWEEN 1 AND 4096 THEN kind_id END,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 END,native_id IS NULL OR (typeof(native_id)='text' AND length(CAST(native_id AS BLOB)) BETWEEN 1 AND 4096),0 FROM knowledge_nodes WHERE source_graph=?1 AND (?2 IS NULL OR id>?2) ORDER BY id LIMIT 1";
            state.charge_work(sql.len())?;
            state.charge_work(
                source_graph
                    .len()
                    .checked_add(after_id.map_or(0, str::len))
                    .ok_or(Error::Budget("normalized ID cursor binding work"))?,
            )?;
            state.active()?;
            let cursor = self.with_connection(WritePhase::Sort, |db| {
                let mut statement = db.prepare(sql)?;
                let mut rows = statement.query(params![source_graph, after_id])?;
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };
                Ok(Some(read_normalized_cursor_row(state, row, None)?))
            })?;
            let Some(mut cursor) = cursor else {
                return Ok(false);
            };
            state.charge_work(
                cursor
                    .source_graph
                    .len()
                    .checked_add(cursor.id.len())
                    .ok_or(Error::Budget("normalized ID cursor identity work"))?,
            )?;
            if cursor.source_graph != source_graph
                || after_id.is_some_and(|after| cursor.id.as_str() <= after)
            {
                return Err(Error::Invalid("normalized ID cursor progress differs"));
            }
            cursor.raw_input_present =
                self.normalized_raw_membership_owned(false, &cursor, state)?;
            let metadata = NormalizedRowMetadata {
                id: &cursor.id,
                source_graph: &cursor.source_graph,
                native_id: cursor.native_id.as_deref(),
                semantic_key: &cursor.semantic_key,
                source_order: cursor.source_order,
                logical_digest: cursor.logical_digest,
                raw_input_present: cursor.raw_input_present,
            };
            let delivered = self.with_normalized_payload_owned(
                false,
                &cursor.id,
                max_bytes,
                |stage, logical, source| {
                    state.charge_work(logical.len())?;
                    if Digest256::of_bytes(logical) != metadata.logical_digest {
                        return Err(Error::Invalid(
                            "normalized ID cursor payload revision changed",
                        ));
                    }
                    consume(stage, &metadata, logical, source)
                },
            )?;
            if delivered.is_none() {
                return Err(Error::Invalid("normalized ID cursor row disappeared"));
            }
            drop(cursor);
            state.active()?;
            Ok(true)
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// Preserve honest Inline rows and source-bound Carrier rows in late CAS.
    pub(crate) fn replace_node_logical_payload_if_current(
        &mut self,
        id: &str,
        logical: &[u8],
        source: Option<&[u8]>,
        previous: Option<Digest256>,
    ) -> Result<()> {
        self.replace_logical_payload_if_current(false, id, logical, source, previous)
    }
    pub(crate) fn replace_relation_logical_payload_if_current(
        &mut self,
        id: &str,
        logical: &[u8],
        source: Option<&[u8]>,
        previous: Option<Digest256>,
    ) -> Result<()> {
        self.replace_logical_payload_if_current(true, id, logical, source, previous)
    }
    fn replace_logical_payload_if_current(
        &mut self,
        relation: bool,
        id: &str,
        logical: &[u8],
        source: Option<&[u8]>,
        previous: Option<Digest256>,
    ) -> Result<()> {
        if self.payload_layout.uses_carriers() {
            if let Some(source) = source {
                return self.replace_normalized_payload_with_exact_source_if_current(
                    relation, id, logical, source, previous,
                );
            }
        }
        let result = (|| {
            if self.poisoned {
                return Err(Error::Invalid("logical Inline update poisoned"));
            }
            valid_id(id)?;
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("logical Inline update same owner absent"))?;
            let cap = usize::try_from(self.limits.sqlite.max_row_bytes)
                .map_err(|_| Error::Budget("logical Inline update row conversion"))?;
            if logical.len() > cap {
                return Err(Error::Budget("logical Inline update row cap"));
            }
            let limits = crate::knowledge_normalization::SourceRow::json_limits(cap)?;
            self.charge_preparation_work(logical.len() as u64)?;
            state.with_serde_owned_with_limits(logical,limits,|value| {
                let actual=value.get("id").and_then(serde_json::Value::as_str)
                    .ok_or(Error::Invalid("logical Inline update ID absent"))?;
                state.charge_work(actual.len())?;
                if actual!=id {return Err(Error::Invalid("logical Inline update ID differs"));}
                state.charge_work(logical.len())?;
                let mut hasher=Digest256Hasher::new();
                for part in logical.chunks(4096) {state.active()?;hasher.update(part);}
                let digest=hasher.finalize();
                self.charge_materialized(1,logical.len() as u64)?;
                self.charge_representation(0, if self.payload_layout.packed_bytes() { crate::knowledge_byte_codec::HEADER as u64 } else { 0 })?;
                let sql=match (self.payload_layout,relation) {
                    (KnowledgePayloadLayout::InlineV1,false)=>"UPDATE knowledge_nodes SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND (?5 IS NULL OR payload_sha256=?5)",
                    (KnowledgePayloadLayout::InlineV1,true)=>"UPDATE knowledge_relations SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND (?5 IS NULL OR payload_sha256=?5)",
                    (KnowledgePayloadLayout::CarrierOnceV1 | KnowledgePayloadLayout::CarrierOnceV2 | KnowledgePayloadLayout::CarrierOnceV3 | KnowledgePayloadLayout::CarrierOnceV4,false)=>"UPDATE knowledge_nodes SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND payload_codec=0 AND source_packet_sha256 IS NULL AND (?5 IS NULL OR payload_sha256=?5)",
                    (KnowledgePayloadLayout::CarrierOnceV1 | KnowledgePayloadLayout::CarrierOnceV2 | KnowledgePayloadLayout::CarrierOnceV3 | KnowledgePayloadLayout::CarrierOnceV4,true)=>"UPDATE knowledge_relations SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND payload_codec=0 AND source_packet_sha256 IS NULL AND (?5 IS NULL OR payload_sha256=?5)",
                };
                let family = value.get("source_graph").and_then(serde_json::Value::as_str).unwrap_or("updated");
                let dictionary = self.prepare_byte_dictionary(if relation {"relation"} else {"node"}, family, logical)?;
                self.payload_layout.with_encoded_dictionary(state, dictionary.as_ref().map(|d|d.verified()).transpose()?, logical, cap, |physical| {
                    self.with_connection(WritePhase::Normalized,|db| {
                        if db.execute(sql,params![logical.len() as i64,digest.as_bytes().as_slice(),physical,id,previous.as_ref().map(|d|d.as_bytes().as_slice())])?!=1 {
                            return Err(Error::Invalid("logical Inline update absent or revision differs"));
                        }
                        Ok(())
                    })
                })
            })
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// Update only the existing row's logical payload. Identity/index columns
    /// and source carrier binding stay unchanged; no source re-admission.
    pub(crate) fn replace_node_payload_with_exact_source(
        &mut self,
        id: &str,
        logical: &[u8],
        source: &[u8],
    ) -> Result<()> {
        self.replace_node_payload_with_exact_source_if_current(id, logical, source, None)
    }
    /// Optional previous logical digest preserves existing producer CAS law.
    pub(crate) fn replace_node_payload_with_exact_source_if_current(
        &mut self,
        id: &str,
        logical: &[u8],
        source: &[u8],
        previous: Option<Digest256>,
    ) -> Result<()> {
        self.replace_normalized_payload_with_exact_source_if_current(
            false, id, logical, source, previous,
        )
    }
    pub(crate) fn replace_relation_payload_with_exact_source_if_current(
        &mut self,
        id: &str,
        logical: &[u8],
        source: &[u8],
        previous: Option<Digest256>,
    ) -> Result<()> {
        self.replace_normalized_payload_with_exact_source_if_current(
            true, id, logical, source, previous,
        )
    }
    fn replace_normalized_payload_with_exact_source_if_current(
        &mut self,
        relation: bool,
        id: &str,
        logical: &[u8],
        source: &[u8],
        previous: Option<Digest256>,
    ) -> Result<()> {
        let result = (|| {
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("carrier update owner absent"))?;
            let limits = crate::knowledge_normalization::SourceRow::json_limits(
                self.limits.sqlite.max_row_bytes,
            )?;
            state.with_serde_owned_with_limits(logical, limits, |value| {
                self.replace_normalized_value_with_exact_source_if_current(
                    relation, id, value, logical, source, previous, None,
                )
            })
        })();
        self.poisoned |= result.is_err();
        result
    }

    /// The finalizer retains the admission of this tree through exact codec
    /// verification and SQL CAS. A caller-supplied tree is not trusted merely
    /// because it was decoded earlier: the codec proves its emitted bytes.
    pub(crate) fn replace_finalized_value_if_current(
        &mut self,
        relation: bool,
        id: &str,
        value: &serde_json::Value,
        logical: &[u8],
        source: Option<&[u8]>,
        previous: Digest256,
        source_receipt: Option<SourceCarrierReadReceipt>,
    ) -> Result<()> {
        if self.payload_layout.uses_carriers() {
            if let Some(source) = source {
                return self.replace_normalized_value_with_exact_source_if_current(
                    relation,
                    id,
                    value,
                    logical,
                    source,
                    Some(previous),
                    source_receipt,
                );
            }
        }
        self.replace_logical_payload_if_current(relation, id, logical, source, Some(previous))
    }

    fn replace_normalized_value_with_exact_source_if_current(
        &mut self,
        relation: bool,
        id: &str,
        value: &serde_json::Value,
        logical: &[u8],
        source: &[u8],
        previous: Option<Digest256>,
        source_receipt: Option<SourceCarrierReadReceipt>,
    ) -> Result<()> {
        let result = (|| {
            if self.poisoned || !self.payload_layout.uses_carriers() {
                return Err(Error::Invalid("carrier normalized update unavailable"));
            }
            valid_id(id)?;
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("carrier update owner absent"))?;
            let cap = usize::try_from(self.limits.sqlite.max_row_bytes)
                .map_err(|_| Error::Budget("carrier update row conversion"))?;
            let limits = crate::knowledge_normalization::SourceRow::json_limits(cap)?;
            self.charge_preparation_work(logical.len() as u64)?;
            let actual = value
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or(Error::Invalid("carrier update logical ID absent"))?;
            state.charge_work(actual.len())?;
            if actual != id {
                return Err(Error::Invalid("carrier update logical ID differs"));
            }
            state.charge_work(logical.len())?;
            let digest = Digest256::of_bytes(logical);
            crate::knowledge_payload_codec::with_factored_value_payload(
                state,
                value,
                logical,
                source,
                limits,
                limits,
                cap,
                |stored, source_digest| {
                    if let Some(receipt) = source_receipt {
                        state.charge_work(std::mem::size_of::<SourceCarrierReadReceipt>())?;
                        if receipt.stage_inode != self.inode
                            || self.write_page.is_none()
                            || self.db().is_autocommit()
                            || receipt.page_generation != self.write_page_generation
                            || receipt.sql_changes != self.db().total_changes()
                            || receipt.relation != relation
                            || Some(receipt.previous) != previous
                            || receipt.source_len != source.len()
                            || receipt.source_digest != source_digest
                        {
                            return Err(Error::Invalid(
                                "carrier read receipt changed before update",
                            ));
                        }
                    } else {
                        let reference = self.retain_exact_source_carrier(source)?;
                        if reference.packet_sha256() != &source_digest {
                            return Err(Error::Invalid("carrier update source differs"));
                        }
                    }
                    self.charge_materialized(1, stored.len() as u64)?;
                    self.charge_representation(
                        0,
                        if self.payload_layout.packed_bytes() {
                            crate::knowledge_byte_codec::HEADER as u64
                        } else {
                            0
                        },
                    )?;
                    let family = value
                        .get("source_graph")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("updated");
                    let dictionary = self.prepare_byte_dictionary(
                        if relation { "relation" } else { "node" },
                        family,
                        stored,
                    )?;
                    self.payload_layout.with_encoded_dictionary(state, dictionary.as_ref().map(|d|d.verified()).transpose()?, stored, cap, |physical| {
                        self.with_connection(WritePhase::Normalized,|db| {
                            // Existing codec1 must retain the exact raw byte key.
                            // Inline rows may be factored from authentic supplied raw
                            // only when the codec's exact logical roundtrip succeeded.
                            let sql = if relation {
                                "UPDATE knowledge_relations SET payload_len=?1,payload_sha256=?2,payload=?3,payload_codec=1,source_packet_sha256=?4 WHERE id=?5 AND (payload_codec=0 OR (payload_codec=1 AND source_packet_sha256=?4)) AND (?6 IS NULL OR payload_sha256=?6)"
                            } else {
                                "UPDATE knowledge_nodes SET payload_len=?1,payload_sha256=?2,payload=?3,payload_codec=1,source_packet_sha256=?4 WHERE id=?5 AND (payload_codec=0 OR (payload_codec=1 AND source_packet_sha256=?4)) AND (?6 IS NULL OR payload_sha256=?6)"
                            };
                            if db.execute(sql,
                                params![logical.len() as i64,digest.as_bytes().as_slice(),physical,source_digest.as_bytes().as_slice(),id,previous.as_ref().map(|v|v.as_bytes().as_slice())])?!=1 {
                                return Err(Error::Invalid("carrier update absent or source binding differs"));
                            }
                            Ok(())
                        })
                        })
                },
            )
        })();
        self.poisoned |= result.is_err();
        result
    }

    #[track_caller]
    pub fn insert_node(&mut self, row: NodeRow<'_>) -> Result<()> {
        let result = self.insert_node_inner(row);
        if result.is_err() {
            let caller = std::panic::Location::caller();
            eprintln!(
                "Native stage inline node caller: {}:{}",
                caller.file(),
                caller.line()
            );
        }
        self.poisoned |= result.is_err();
        result
    }
    fn insert_node_inner(&mut self, row: NodeRow<'_>) -> Result<()> {
        self.insert_node_storage_inner(row, None)
    }
    fn insert_node_storage_inner(
        &mut self,
        row: NodeRow<'_>,
        logical: Option<(usize, Digest256, Digest256)>,
    ) -> Result<()> {
        if self.poisoned || (logical.is_some() && !self.payload_layout.uses_carriers()) {
            return Err(Error::Invalid("normalized carrier write unavailable"));
        }
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
        let digest = logical
            .map(|v| v.1)
            .unwrap_or_else(|| Digest256::of_bytes(row.payload));
        let logical_len = logical.map(|v| v.0).unwrap_or(row.payload.len());
        if let Some(state) = self.owned_creation_state() {
            let layout = self.payload_layout;
            if layout.packed_bytes() {
                self.charge_representation(0, crate::knowledge_byte_codec::HEADER as u64)?;
            }
            let dictionary = self.prepare_byte_dictionary("node", row.source_graph, row.payload)?;
            layout.with_encoded_dictionary(state, dictionary.as_ref().map(|d|d.verified()).transpose()?, row.payload, self.limits.sqlite.max_row_bytes, |physical| {
            if let Some((_, _, source_digest)) = logical {
                stage_insert_owned(self.db(), c"INSERT INTO knowledge_nodes (id,source_graph,native_id,entity_id,kind_id,type_id,source_order,payload_len,payload_sha256,payload,payload_codec,source_packet_sha256) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11)",
                    &[StageSqlBinding::Text(row.id), StageSqlBinding::Text(row.source_graph), StageSqlBinding::OptionalText(row.native_id), StageSqlBinding::OptionalText(row.entity_id), StageSqlBinding::Text(row.kind_id), StageSqlBinding::Text(row.type_id), StageSqlBinding::Integer(row.source_order), StageSqlBinding::Integer(logical_len as i64), StageSqlBinding::Blob(digest.as_bytes()), StageSqlBinding::Blob(physical), StageSqlBinding::Blob(source_digest.as_bytes())], state)?;
            } else {
                stage_insert_owned(self.db(), c"INSERT INTO knowledge_nodes (id,source_graph,native_id,entity_id,kind_id,type_id,source_order,payload_len,payload_sha256,payload) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    &[StageSqlBinding::Text(row.id), StageSqlBinding::Text(row.source_graph), StageSqlBinding::OptionalText(row.native_id), StageSqlBinding::OptionalText(row.entity_id), StageSqlBinding::Text(row.kind_id), StageSqlBinding::Text(row.type_id), StageSqlBinding::Integer(row.source_order), StageSqlBinding::Integer(logical_len as i64), StageSqlBinding::Blob(digest.as_bytes()), StageSqlBinding::Blob(physical)], state)?;
            }
            Ok(())
            })?;
            return self.check(WritePhase::Normalized);
        }
        if self.payload_layout.packed_bytes() {
            return Err(Error::Invalid("packed normalized writer owner absent"));
        }
        self.db().execute(
            "INSERT INTO knowledge_nodes (id,source_graph,native_id,entity_id,kind_id,type_id,source_order,payload_len,payload_sha256,payload) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                row.id,
                row.source_graph,
                row.native_id,
                row.entity_id,
                row.kind_id,
                row.type_id,
                row.source_order,
                logical_len as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        if let Some((_, _, source_digest)) = logical {
            if self.db().execute(
                "UPDATE knowledge_nodes SET payload_codec=1,source_packet_sha256=?1 WHERE id=?2",
                params![source_digest.as_bytes().as_slice(), row.id],
            )? != 1
            {
                return Err(Error::Invalid("carrier row metadata update absent"));
            }
        }
        self.check(WritePhase::Normalized)?;
        Ok(())
    }
    /// Same-owner physical write from an actual supplied source carrier.
    /// Logical metadata/digest remain unchanged; any error poisons this Stage.
    pub(crate) fn insert_relation_with_exact_source(
        &mut self,
        row: RelationRow<'_>,
        source: &[u8],
    ) -> Result<()> {
        if self.payload_layout == KnowledgePayloadLayout::InlineV1 {
            return self.insert_relation(row);
        }
        let result = (|| {
            if self.poisoned {
                return Err(Error::Invalid("poisoned carrier Stage"));
            }
            let state = self
                .owned_creation_state()
                .ok_or(Error::Invalid("carrier same owner state absent"))?;
            let cap = usize::try_from(self.limits.sqlite.max_row_bytes)
                .map_err(|_| Error::Budget("carrier row bound conversion"))?;
            let limits = crate::knowledge_normalization::SourceRow::json_limits(cap)?;
            self.charge_preparation_work(row.payload.len() as u64)?;
            state.charge_work(row.payload.len())?;
            let digest = Digest256::of_bytes(row.payload);
            crate::knowledge_payload_codec::with_factored_payload(
                state,
                row.payload,
                source,
                limits,
                limits,
                limits,
                cap,
                |stored, source_digest| {
                    let reference =
                        self.retain_exact_source_carrier_for_family(row.source_graph, source)?;
                    if reference.packet_sha256() != &source_digest {
                        return Err(Error::Invalid("carrier source reference differs"));
                    }
                    self.insert_relation_storage_inner(
                        RelationRow {
                            id: row.id,
                            source_graph: row.source_graph,
                            native_id: row.native_id,
                            from_id: row.from_id,
                            to_id: row.to_id,
                            predicate_id: row.predicate_id,
                            relation_type_id: row.relation_type_id,
                            source_order: row.source_order,
                            payload: stored,
                        },
                        Some((row.payload.len(), digest, source_digest)),
                    )
                },
            )
        })();
        self.poisoned |= result.is_err();
        result
    }

    #[track_caller]
    pub fn insert_relation(&mut self, row: RelationRow<'_>) -> Result<()> {
        let result = self.insert_relation_inner(row);
        if result.is_err() {
            let caller = std::panic::Location::caller();
            eprintln!(
                "Native stage inline relation caller: {}:{}",
                caller.file(),
                caller.line()
            );
        }
        self.poisoned |= result.is_err();
        result
    }
    fn insert_relation_inner(&mut self, row: RelationRow<'_>) -> Result<()> {
        self.insert_relation_storage_inner(row, None)
    }
    fn insert_relation_storage_inner(
        &mut self,
        row: RelationRow<'_>,
        logical: Option<(usize, Digest256, Digest256)>,
    ) -> Result<()> {
        if self.poisoned || (logical.is_some() && !self.payload_layout.uses_carriers()) {
            return Err(Error::Invalid("normalized carrier write unavailable"));
        }
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
        let digest = logical
            .map(|v| v.1)
            .unwrap_or_else(|| Digest256::of_bytes(row.payload));
        let logical_len = logical.map(|v| v.0).unwrap_or(row.payload.len());
        if let Some(state) = self.owned_creation_state() {
            let layout = self.payload_layout;
            if layout.packed_bytes() {
                self.charge_representation(0, crate::knowledge_byte_codec::HEADER as u64)?;
            }
            let dictionary =
                self.prepare_byte_dictionary("relation", row.source_graph, row.payload)?;
            layout.with_encoded_dictionary(state, dictionary.as_ref().map(|d|d.verified()).transpose()?, row.payload, self.limits.sqlite.max_row_bytes, |physical| {
            if let Some((_, _, source_digest)) = logical {
                stage_insert_owned(self.db(), c"INSERT INTO knowledge_relations (id,source_graph,native_id,from_id,to_id,predicate_id,relation_type_id,source_order,payload_len,payload_sha256,payload,payload_codec,source_packet_sha256) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,1,?12)",
                    &[StageSqlBinding::Text(row.id), StageSqlBinding::Text(row.source_graph), StageSqlBinding::OptionalText(row.native_id), StageSqlBinding::Text(row.from_id), StageSqlBinding::Text(row.to_id), StageSqlBinding::Text(row.predicate_id), StageSqlBinding::Text(row.relation_type_id), StageSqlBinding::Integer(row.source_order), StageSqlBinding::Integer(logical_len as i64), StageSqlBinding::Blob(digest.as_bytes()), StageSqlBinding::Blob(physical), StageSqlBinding::Blob(source_digest.as_bytes())], state)?;
            } else {
                stage_insert_owned(self.db(), c"INSERT INTO knowledge_relations (id,source_graph,native_id,from_id,to_id,predicate_id,relation_type_id,source_order,payload_len,payload_sha256,payload) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    &[StageSqlBinding::Text(row.id), StageSqlBinding::Text(row.source_graph), StageSqlBinding::OptionalText(row.native_id), StageSqlBinding::Text(row.from_id), StageSqlBinding::Text(row.to_id), StageSqlBinding::Text(row.predicate_id), StageSqlBinding::Text(row.relation_type_id), StageSqlBinding::Integer(row.source_order), StageSqlBinding::Integer(logical_len as i64), StageSqlBinding::Blob(digest.as_bytes()), StageSqlBinding::Blob(physical)], state)?;
            }
            Ok(())
            })?;
            return self.check(WritePhase::Normalized);
        }
        if self.payload_layout.packed_bytes() {
            return Err(Error::Invalid("packed normalized writer owner absent"));
        }
        self.db().execute(
            "INSERT INTO knowledge_relations (id,source_graph,native_id,from_id,to_id,predicate_id,relation_type_id,source_order,payload_len,payload_sha256,payload) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                row.id,
                row.source_graph,
                row.native_id,
                row.from_id,
                row.to_id,
                row.predicate_id,
                row.relation_type_id,
                row.source_order,
                logical_len as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        if let Some((_, _, source_digest)) = logical {
            if self.db().execute("UPDATE knowledge_relations SET payload_codec=1,source_packet_sha256=?1 WHERE id=?2",
                params![source_digest.as_bytes().as_slice(),row.id])? != 1 {
                return Err(Error::Invalid("carrier row metadata update absent"));
            }
        }
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
    pub(crate) fn raw_matches_bytes(
        &self,
        source_graph: &str,
        collection: &str,
        id: &str,
        expected: &[u8],
    ) -> Result<bool> {
        let Some(state) = self.owned_creation_state() else {
            return Ok(self
                .raw_by_id(source_graph, collection, id)?
                .is_some_and(|row| row.payload.as_slice() == expected));
        };
        self.require_open_inputs()?;
        if !self.registered(source_graph, collection) {
            return Err(Error::Invalid("unregistered input collection"));
        }
        valid_id(id)?;
        const SQL: &std::ffi::CStr = c"SELECT payload_len,payload,payload_sha256 FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3";
        state.charge_work(SQL.to_bytes().len())?;
        let _hold = state.hold(
            tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
        )?;
        let mut statement =
            tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(self.db(), SQL)
                .map_err(|error| owned_stage_sql_error(error))?;
        for (slot, text) in [(1, source_graph), (2, collection), (3, id)] {
            state.charge_work(text.len())?;
            statement
                .bind_text(slot, text)
                .map_err(|error| owned_stage_sql_error(error))?;
        }
        state.active()?;
        if !statement
            .step()
            .map_err(|error| owned_stage_sql_error(error))?
        {
            return Ok(false);
        }
        let declared = statement
            .unsigned_integer(0)
            .map_err(|error| owned_stage_sql_error(error))?;
        let payload = match statement
            .value_ref(1)
            .map_err(|error| owned_stage_sql_error(error))?
        {
            rusqlite::types::ValueRef::Blob(raw) => raw,
            _ => return Err(Error::Invalid("stage raw comparison payload type")),
        };
        let digest: [u8; 32] = match statement
            .value_ref(2)
            .map_err(|error| owned_stage_sql_error(error))?
        {
            rusqlite::types::ValueRef::Blob(raw) => raw
                .try_into()
                .map_err(|_| Error::Invalid("stage raw comparison digest"))?,
            _ => return Err(Error::Invalid("stage raw comparison digest type")),
        };
        if declared != payload.len() as u64 || payload.len() > self.raw_input_max_bytes {
            return Err(Error::Invalid("stage raw comparison input length"));
        }
        self.charge_raw_observation_read(payload.len() as u64)?;
        state.charge_work(
            payload
                .len()
                .checked_mul(2)
                .ok_or(Error::Budget("stage raw comparison work"))?,
        )?;
        if tos_foundation::Digest256::of_bytes(payload).as_bytes() != &digest {
            return Err(Error::Invalid("stage raw comparison digest differs"));
        }
        let matches = payload == expected;
        state.active()?;
        Ok(matches)
    }
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
            let length = if let Some(state) = self.owned_creation_state() {
                const SQL:&std::ffi::CStr=c"SELECT payload_len,length(payload) FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3";
                state.charge_work(SQL.to_bytes().len())?;
                let _hold=state.hold(tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound())?;
                let mut statement =
                    tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(
                        self.db(),
                        SQL,
                    )
                    .map_err(|error| owned_stage_sql_error(error))?;
                for (slot, text) in [(1, source_graph), (2, collection), (3, id)] {
                    state.charge_work(text.len())?;
                    statement
                        .bind_text(slot, text)
                        .map_err(|error| owned_stage_sql_error(error))?;
                }
                state.active()?;
                if statement
                    .step()
                    .map_err(|error| owned_stage_sql_error(error))?
                {
                    Some((
                        statement
                            .unsigned_integer(0)
                            .map_err(|error| owned_stage_sql_error(error))?,
                        statement
                            .unsigned_integer(1)
                            .map_err(|error| owned_stage_sql_error(error))?,
                    ))
                } else {
                    None
                }
            } else {
                self.db().query_row(
                    "SELECT payload_len,length(payload) FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3",
                    params![source_graph, collection, id],
                    |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?)),
                ).optional()?
            };
            if let Some((declared, actual)) = length {
                if declared != actual || actual > self.raw_input_max_bytes as u64 {
                    return Err(Error::Invalid("stage raw observation input length"));
                }
                self.charge_raw_observation_read(actual)?;
            }
        }
        if let Some(state) = self.owned_creation_state() {
            const SQL: &std::ffi::CStr = c"SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3 AND length(payload)<=?4 AND payload_len=length(payload)";
            state.charge_work(SQL.to_bytes().len())?;
            let _hold=state.hold(tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound())?;
            let mut statement =
                tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(
                    self.db(),
                    SQL,
                )
                .map_err(|error| owned_stage_sql_error(error))?;
            for (slot, text) in [(1, source_graph), (2, collection), (3, id)] {
                state.charge_work(text.len())?;
                statement
                    .bind_text(slot, text)
                    .map_err(|error| owned_stage_sql_error(error))?;
            }
            statement
                .bind_i64(4, self.raw_input_max_bytes as i64)
                .map_err(|error| owned_stage_sql_error(error))?;
            state.active()?;
            let value = if statement
                .step()
                .map_err(|error| owned_stage_sql_error(error))?
            {
                Some(verify_seek_row(
                    read_seek_row_bounded(&statement, state, self.raw_input_max_bytes)?,
                    self.raw_input_max_bytes,
                )?)
            } else {
                None
            };
            state.active()?;
            return Ok(value);
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

    /// Same indexed input page consumed under scoped original state. The
    /// statement is closed before the mutable Stage callback starts; input
    /// rows and their holds stay live through the callback and then drop.
    pub(crate) fn with_scan_input_owned<T>(
        &mut self,
        source_graph: &str,
        collection: &str,
        after_id: Option<&str>,
        max_rows: usize,
        consume: impl FnOnce(&mut Self, &ScanPage) -> Result<T>,
    ) -> Result<T> {
        let Some(state) = self.owned_creation_state() else {
            let page = self.scan_input(source_graph, collection, after_id, max_rows)?;
            return consume(self, &page);
        };
        let result = (|| {
            let page =
                self.scoped_input_page(source_graph, collection, after_id, max_rows, state)?;
            let result = consume(self, &page.page);
            drop(page);
            result.and_then(|value| {
                state.active()?;
                Ok(value)
            })
        })();
        self.poisoned |= result.is_err();
        result
    }

    pub(crate) fn with_raw_by_id_owned<T>(
        &mut self,
        source_graph: &str,
        collection: &str,
        id: &str,
        consume: impl FnOnce(&mut Self, Option<&SeekRow>) -> Result<T>,
    ) -> Result<T> {
        let result = (|| {
            let row = self.scoped_raw_by_id(source_graph, collection, id)?;
            let result = consume(self, row.as_ref());
            drop(row);
            result.and_then(|value| {
                if let Some(state) = self.owned_creation_state() {
                    state.active()?;
                }
                Ok(value)
            })
        })();
        self.poisoned |= result.is_err();
        result
    }

    pub(crate) fn scoped_raw_by_id(
        &self,
        source_graph: &str,
        collection: &str,
        id: &str,
    ) -> Result<ScopedRawRow<'a>> {
        if let Some(state) = self.owned_creation_state() {
            state.charge_work(id.len())?;
            valid_id(id)?;
            Ok(ScopedRawRow {
                owned: Some(self.scoped_input_page_selected(
                    source_graph,
                    collection,
                    None,
                    1,
                    Some(id),
                    state,
                )?),
                legacy: None,
            })
        } else {
            Ok(ScopedRawRow {
                owned: None,
                legacy: self.raw_by_id(source_graph, collection, id)?,
            })
        }
    }

    fn scoped_input_page(
        &self,
        source_graph: &str,
        collection: &str,
        after_id: Option<&str>,
        max_rows: usize,
        state: &'a crate::d1_public_capture::CreationState<'a>,
    ) -> Result<OwnedInputPage<'a>> {
        self.scoped_input_page_selected(source_graph, collection, after_id, max_rows, None, state)
    }

    fn scoped_input_page_selected(
        &self,
        source_graph: &str,
        collection: &str,
        after_id: Option<&str>,
        max_rows: usize,
        exact_id: Option<&str>,
        state: &'a crate::d1_public_capture::CreationState<'a>,
    ) -> Result<OwnedInputPage<'a>> {
        self.require_open_inputs()?;
        if !self.registered_with_owned_state(source_graph, collection, state)? {
            return Err(Error::Invalid("unregistered input collection"));
        }
        if let Some(id) = after_id {
            state.charge_work(id.len())?;
            valid_id(id)?;
        }
        if max_rows == 0 || max_rows > self.limits.max_seek_rows {
            return Err(Error::Budget("stage seek rows"));
        }
        if self.raw_read_budget.is_some() {
            if let Some(id) = exact_id {
                const LENGTH_SQL: &std::ffi::CStr = c"SELECT payload_len,length(payload) FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3";
                state.charge_work(LENGTH_SQL.to_bytes().len())?;
                let _hold = state.hold(tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound())?;
                let mut statement =
                    tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(
                        self.db(),
                        LENGTH_SQL,
                    )
                    .map_err(|error| owned_stage_sql_error(error))?;
                for (slot, text) in [(1, source_graph), (2, collection), (3, id)] {
                    state.charge_work(text.len())?;
                    statement
                        .bind_text(slot, text)
                        .map_err(|error| owned_stage_sql_error(error))?;
                }
                state.active()?;
                if statement
                    .step()
                    .map_err(|error| owned_stage_sql_error(error))?
                {
                    let declared = statement
                        .unsigned_integer(0)
                        .map_err(|error| owned_stage_sql_error(error))?;
                    let actual = statement
                        .unsigned_integer(1)
                        .map_err(|error| owned_stage_sql_error(error))?;
                    if declared != actual || actual > self.raw_input_max_bytes as u64 {
                        return Err(Error::Invalid("stage raw observation input length"));
                    }
                }
            }
        }
        let lookahead = i64::try_from(
            max_rows
                .checked_add(1)
                .ok_or(Error::Budget("stage seek rows"))?,
        )
        .map_err(|_| Error::Budget("stage seek rows"))?;
        let containers = max_rows
            .checked_mul(
                std::mem::size_of::<SeekRow>()
                    + std::mem::size_of::<crate::d1_public_capture::CreationStateHold<'a, 'a>>(),
            )
            .and_then(|n| n.checked_add(MAX_NAME_BYTES))
            .and_then(|n| {
                n.checked_add(
                    std::mem::size_of::<OwnedInputPage<'a>>()
                        + std::mem::size_of::<SeekRow>()
                        + std::mem::size_of::<Digest256Hasher>(),
                )
            })
            .ok_or(Error::Budget("owned input page containers"))?;
        let container_hold = state.hold(containers)?;
        let mut page = OwnedInputPage {
            page: ScanPage {
                rows: Vec::with_capacity(max_rows),
                next_id: None,
            },
            row_holds: Vec::with_capacity(max_rows),
            container_hold,
        };
        let sql = if exact_id.is_some() {
            c"SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id=?3 AND length(payload)<=?4 AND payload_len=length(payload) ORDER BY id LIMIT ?5"
        } else if after_id.is_some() {
            c"SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records WHERE source_graph=?1 AND collection=?2 AND id>?3 AND length(payload)<=?4 AND payload_len=length(payload) ORDER BY id LIMIT ?5"
        } else {
            c"SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records WHERE source_graph=?1 AND collection=?2 AND length(payload)<=?3 AND payload_len=length(payload) ORDER BY id LIMIT ?4"
        };
        state.charge_work(sql.to_bytes().len())?;
        let _statement_hold = state.hold(
            tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
        )?;
        let mut statement =
            tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(self.db(), sql)
                .map_err(|error| owned_stage_sql_error(error))?;
        for (slot, text) in [(1, source_graph), (2, collection)] {
            state.charge_work(text.len())?;
            statement
                .bind_text(slot, text)
                .map_err(|error| owned_stage_sql_error(error))?;
        }
        let cap_slot = if let Some(id) = exact_id.or(after_id) {
            state.charge_work(id.len())?;
            statement
                .bind_text(3, id)
                .map_err(|error| owned_stage_sql_error(error))?;
            4
        } else {
            3
        };
        statement
            .bind_i64(cap_slot, self.raw_input_max_bytes as i64)
            .map_err(|error| owned_stage_sql_error(error))?;
        statement
            .bind_i64(cap_slot + 1, lookahead)
            .map_err(|error| owned_stage_sql_error(error))?;
        let mut bytes = 0u64;
        let mut has_more = false;
        loop {
            state.active()?;
            if !statement
                .step()
                .map_err(|error| owned_stage_sql_error(error))?
            {
                break;
            }
            if page.page.rows.len() == max_rows {
                has_more = true;
                break;
            }
            use rusqlite::types::ValueRef;
            let id_raw = match statement
                .value_ref(0)
                .map_err(|error| owned_stage_sql_error(error))?
            {
                ValueRef::Text(raw) => raw,
                _ => return Err(Error::Invalid("stage seek id")),
            };
            let graph_raw = match statement
                .value_ref(1)
                .map_err(|error| owned_stage_sql_error(error))?
            {
                ValueRef::Text(raw) => raw,
                _ => return Err(Error::Invalid("stage seek graph")),
            };
            if id_raw.len() > MAX_NAME_BYTES || graph_raw.len() > MAX_NAME_BYTES {
                return Err(Error::Budget("owned stage seek text bytes"));
            }
            state.charge_work(
                id_raw
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(graph_raw.len()))
                    .ok_or(Error::Budget("owned input page text work"))?,
            )?;
            let id =
                std::str::from_utf8(id_raw).map_err(|_| Error::Invalid("stage seek id UTF8"))?;
            let graph = std::str::from_utf8(graph_raw)
                .map_err(|_| Error::Invalid("stage seek graph UTF8"))?;
            valid_id(id)?;
            let payload = match statement
                .value_ref(3)
                .map_err(|error| owned_stage_sql_error(error))?
            {
                ValueRef::Blob(raw) => raw,
                _ => return Err(Error::Invalid("stage seek payload")),
            };
            if payload.len() > self.raw_input_max_bytes {
                return Err(Error::Budget("stage seek row bytes"));
            }
            let digest: [u8; 32] = match statement
                .value_ref(4)
                .map_err(|error| owned_stage_sql_error(error))?
            {
                ValueRef::Blob(raw) => raw
                    .try_into()
                    .map_err(|_| Error::Invalid("stage seek digest bytes"))?,
                _ => return Err(Error::Invalid("stage seek digest")),
            };
            self.charge_raw_observation_read(payload.len() as u64)?;
            state.charge_work(payload.len())?;
            let mut hash = Digest256Hasher::new();
            for chunk in payload.chunks(65536) {
                state.active()?;
                hash.update(chunk);
            }
            if hash.finalize().as_bytes() != &digest {
                return Err(Error::Invalid("stage seek payload digest"));
            }
            let next = bytes
                .checked_add(payload.len() as u64)
                .ok_or(Error::Budget("stage seek bytes"))?;
            if exact_id.is_none() && next > self.limits.max_seek_bytes {
                if page.page.rows.is_empty() {
                    return Err(Error::Budget("stage seek bytes"));
                }
                has_more = true;
                break;
            }
            let row_bytes = id
                .len()
                .checked_add(graph.len())
                .and_then(|n| n.checked_add(payload.len()))
                .and_then(|n| n.checked_add(64))
                .ok_or(Error::Budget("owned input page row state"))?;
            let row_hold = state.hold(row_bytes)?;
            state.charge_work(row_bytes)?;
            let row = SeekRow {
                id: id.to_owned(),
                source_graph: graph.to_owned(),
                source_order: None,
                payload: payload.to_owned(),
                payload_sha256: Digest256::from_bytes(digest).to_hex(),
            };
            page.page.rows.push(row);
            page.row_holds.push(row_hold);
            bytes = next;
        }
        if has_more {
            if let Some(row) = page.page.rows.last() {
                state.charge_work(row.id.len())?;
                page.page.next_id = Some(row.id.clone());
            }
        }
        state.active()?;
        Ok(page)
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
            let item = verify_seek_row(
                read_seek_row_with_state(
                    row,
                    self.owned_creation_state(),
                    self.raw_input_max_bytes,
                )?,
                self.raw_input_max_bytes,
            )?;
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
            if let Some(state) = self.owned_creation_state() {
                // Old+new Vec buffers for one geometric growth step. Retained
                // conservatively until the producer phase closes.
                state.retain(4 * std::mem::size_of::<SeekRow>())?;
            }
            page.push(item);
        }
        let next_id = if has_more {
            if let (Some(state), Some(row)) = (self.owned_creation_state(), page.last()) {
                state.retain(row.id.len())?;
            }
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
            match self.payload_layout() {
                KnowledgePayloadLayout::InlineV1 => "SELECT id,source_graph,source_order,payload,payload_sha256 FROM knowledge_relations
                  WHERE from_id=?1 AND source_order>?2 AND length(payload)<=?3
                    AND payload_len=length(payload)
                  ORDER BY source_order,id LIMIT ?4",
                KnowledgePayloadLayout::CarrierOnceV1 | KnowledgePayloadLayout::CarrierOnceV2 | KnowledgePayloadLayout::CarrierOnceV3 | KnowledgePayloadLayout::CarrierOnceV4 => "SELECT id,source_graph,source_order,payload,payload_sha256,payload_len,payload_codec,source_packet_sha256 FROM knowledge_relations
                  WHERE from_id=?1 AND source_order>?2 AND payload_len<=?3 AND length(payload)<=?3+17
                  ORDER BY source_order,id LIMIT ?4",
            },
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
            match self.payload_layout() {
                KnowledgePayloadLayout::InlineV1 => "SELECT id,source_graph,source_order,payload,payload_sha256 FROM knowledge_relations
                  WHERE to_id=?1 AND source_order>?2 AND length(payload)<=?3
                    AND payload_len=length(payload)
                  ORDER BY source_order,id LIMIT ?4",
                KnowledgePayloadLayout::CarrierOnceV1 | KnowledgePayloadLayout::CarrierOnceV2 | KnowledgePayloadLayout::CarrierOnceV3 | KnowledgePayloadLayout::CarrierOnceV4 => "SELECT id,source_graph,source_order,payload,payload_sha256,payload_len,payload_codec,source_packet_sha256 FROM knowledge_relations
                  WHERE to_id=?1 AND source_order>?2 AND payload_len<=?3 AND length(payload)<=?3+17
                  ORDER BY source_order,id LIMIT ?4",
            },
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
            let item = if self.payload_layout().uses_carriers() {
                let state = self
                    .owned_creation_state()
                    .ok_or(Error::Invalid("compact stage seek owner absent"))?;
                let id: &str = row
                    .get_ref(0)?
                    .as_str()
                    .map_err(|_| Error::Invalid("stage seek id type"))?;
                let graph: &str = row
                    .get_ref(1)?
                    .as_str()
                    .map_err(|_| Error::Invalid("stage seek graph type"))?;
                if id.len() > MAX_NAME_BYTES || graph.len() > MAX_NAME_BYTES {
                    return Err(Error::Budget("owned stage seek text bytes"));
                }
                state.charge_work(id.len() + graph.len())?;
                valid_id(id)?;
                let digest = row
                    .get_ref(4)?
                    .as_blob()
                    .map_err(|_| Error::Invalid("stage seek digest type"))?;
                let key = match row.get_ref(7)? {
                    rusqlite::types::ValueRef::Null => None,
                    rusqlite::types::ValueRef::Blob(key) => Some(key),
                    _ => return Err(Error::Invalid("stage seek source key")),
                };
                crate::knowledge_payload_codec::with_sql_logical_payload(
                    self.db(),
                    state,
                    self.payload_layout(),
                    row.get(5)?,
                    digest,
                    row.get_ref(3)?
                        .as_blob()
                        .map_err(|_| Error::Invalid("stage seek payload type"))?,
                    row.get(6)?,
                    key,
                    self.limits.sqlite.max_row_bytes,
                    |logical| {
                        state.retain(
                            std::mem::size_of::<SeekRow>()
                                + id.len()
                                + graph.len()
                                + logical.len()
                                + 64,
                        )?;
                        state.charge_work(logical.len() + id.len() + graph.len())?;
                        let digest: [u8; 32] = digest
                            .try_into()
                            .map_err(|_| Error::Invalid("stage seek digest bytes"))?;
                        Ok(SeekRow {
                            id: id.to_owned(),
                            source_graph: graph.to_owned(),
                            source_order: row.get(2)?,
                            payload: logical.to_owned(),
                            payload_sha256: Digest256::from_bytes(digest).to_hex(),
                        })
                    },
                )?
            } else {
                verify_seek_row(
                    read_seek_row_with_state(
                        row,
                        self.owned_creation_state(),
                        self.limits.sqlite.max_row_bytes,
                    )?,
                    self.limits.sqlite.max_row_bytes,
                )?
            };
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
        if let Some(state) = self.owned_creation_state() {
            // SQLite statement locals. Digest stack is scoped at its actual
            // kernel invocation; names remain held by the Stage owner.
            state.retain(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())?;
        }
        if let Some(state) = self.owned_creation_state() {
            let binding = &self.exact_receipt()?.binding;
            let mut bytes = std::mem::size_of::<StageReceipt>() + 64;
            for string in [
                &binding.owner_profile,
                &binding.source_cut,
                &binding.membership_root,
                &binding.index_generation,
                &binding.route_map_version,
                &binding.reader_abi,
                &binding.projection_root_sha256,
                &binding.source_cut,
                &binding.membership_root,
            ] {
                bytes = bytes
                    .checked_add(string.len())
                    .ok_or(Error::Budget("owned Stage output binding"))?;
            }
            bytes = bytes
                .checked_add(
                    self.receipt
                        .collections()
                        .len()
                        .checked_mul(std::mem::size_of::<InputCollectionReceipt>())
                        .ok_or(Error::Budget("owned Stage output collections"))?,
                )
                .ok_or(Error::Budget("owned Stage output slots"))?;
            for entry in self.receipt.collections() {
                for string in [
                    &entry.source_graph,
                    &entry.collection,
                    &entry.input_role,
                    &entry.adapter_profile,
                    &entry.expected_root_sha256,
                ] {
                    bytes = bytes
                        .checked_add(string.len())
                        .ok_or(Error::Budget("owned Stage output strings"))?;
                }
            }
            state.retain(bytes)?;
        }
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
        let (node_rows, node_root) =
            output_root_with_state(self.db(), "knowledge_nodes", self.owned_creation_state())?;
        self.check(WritePhase::Sort)?;
        let (relation_rows, relation_root) = output_root_with_state(
            self.db(),
            "knowledge_relations",
            self.owned_creation_state(),
        )?;
        self.check(WritePhase::Sort)?;
        const DANGLING_SQL: &std::ffi::CStr = c"SELECT 1 FROM knowledge_relations r
             WHERE NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.from_id)
                OR NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.to_id)
             LIMIT 1";
        let dangling = if let Some(state) = self.owned_creation_state() {
            state.charge_work(DANGLING_SQL.to_bytes().len())?;
            let _hold = state.hold(tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound())?;
            let mut statement =
                tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(
                    self.db(),
                    DANGLING_SQL,
                )
                .map_err(|error| owned_stage_sql_error(error))?;
            state.active()?;
            let found = statement
                .step()
                .map_err(|error| owned_stage_sql_error(error))?;
            state.active()?;
            found
        } else {
            self.db()
                .query_row(
                    DANGLING_SQL
                        .to_str()
                        .map_err(|_| Error::Invalid("stage static SQL UTF8"))?,
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .is_some()
        };
        if dangling {
            return Err(Error::Invalid("stage relation endpoint absent"));
        }
        self.check(WritePhase::Sort)?;
        let mut selected_file = None;
        if self.selected_full {
            crate::knowledge_navigation_original::verify_stage(&mut self, None)?;
            crate::knowledge_philosophy_original::verify_stage(&mut self, None)?;
            crate::knowledge_corpus_original::verify_stage(&mut self, None)?;
            self.check(WritePhase::Finalize)?;
            preflight_selected_vacuum_with_state(
                self.db(),
                &self.candidate,
                self.inode,
                self.limits,
                self.owned_creation_state(),
            )?;
            // Owner input is removed from the private stage only after exact
            // root checks. VACUUM INTO then creates a different SQLite inode
            // containing the allowlisted logical tables; the private stage
            // inode is never the selected artifact.
            if self.closed_input_rows.is_none() {
                if let Some(state) = self.owned_creation_state() {
                    stage_batch_owned(self.db(),c"PRAGMA secure_delete=ON; PRAGMA temp.secure_delete=ON; DROP TABLE raw_records",state)?;
                } else {
                    self.db().execute_batch("PRAGMA secure_delete=ON; PRAGMA temp.secure_delete=ON; DROP TABLE raw_records")?;
                }
            }
            if self.payload_layout.uses_carriers() {
                crate::knowledge_selected::verify_schema_with_layout(
                    self.db(),
                    self.payload_layout,
                    self.owned_creation_state(),
                )?;
            } else {
                selected_table_closure_with_owned_state(self.db(), self.owned_creation_state())?;
            }
            self.check(WritePhase::Finalize)?;
            let fresh = fresh_selected_path(&self.candidate);
            self.isolation
                .ok_or(Error::Invalid("selected stage isolation absent"))?
                .verify(&fresh, self.limits, WritePhase::Finalize)?;
            let fresh_utf8 = fresh
                .to_str()
                .ok_or(Error::Invalid("stage fresh selected path encoding"))?;
            self.fresh_selected = Some(fresh.clone());
            if let Some(state) = self.owned_creation_state() {
                // Same installed progress hook and shared SQLite pool; only
                // this statement's Rust workspace and bound path copy are new.
                stage_insert_owned(
                    self.db(),
                    c"VACUUM INTO ?1",
                    &[StageSqlBinding::Text(fresh_utf8)],
                    state,
                )?;
            } else {
                self.db().execute("VACUUM INTO ?1", [fresh_utf8])?;
            }
            self.check(WritePhase::Finalize)?;
            fs::set_permissions(&fresh, fs::Permissions::from_mode(0o600))?;
            let pinned = safe_open::open_regular(&fresh, self.limits.sqlite.max_output_bytes)?;
            verify_fresh_selected(
                &fresh,
                &pinned,
                self.limits.sqlite,
                Arc::clone(self.vm_used.as_ref().expect("stage VM counter")),
                self.controlled.as_ref(),
                self.public_deadline,
                self.payload_layout,
            )?;
            selected_file = Some(pinned);
        }
        self.check(WritePhase::Finalize)?;
        let integrity_ok = if let Some(state) = self.owned_creation_state() {
            stage_integrity_first_row_owned(self.db(), state)?
        } else {
            self.db().query_row("PRAGMA integrity_check", [], |row| {
                Ok(row.get_ref(0)?.as_str()? == "ok")
            })?
        };
        if !integrity_ok {
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
            if let Some(state) = self.owned_creation_state() {
                stage_stream_digest_owned(
                    &mut digest_file,
                    self.limits.sqlite.max_output_bytes,
                    state,
                )?
            } else {
                stream_digest(&mut digest_file)?
            }
        } else {
            // Preserve the old owned nonselected stack admission while that
            // fallback's source/work/IO owner seam remains explicitly open.
            let _fallback_hold = match self.owned_creation_state() {
                Some(state) => Some(state.hold(65536)?),
                None => None,
            };
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
fn preflight_selected_vacuum_with_state(
    db: &Connection,
    candidate: &Path,
    inode: (u64, u64),
    limits: StageLimits,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<()> {
    let Some(state) = state else {
        return preflight_selected_vacuum(db, candidate, inode, limits);
    };
    let page_count = stage_integer_owned(db, c"PRAGMA page_count", state)?;
    let page_size = stage_integer_owned(db, c"PRAGMA page_size", state)?;
    let page_count =
        u64::try_from(page_count).map_err(|_| Error::Invalid("selected page count"))?;
    let page_size = u64::try_from(page_size).map_err(|_| Error::Invalid("selected page size"))?;
    if page_count == 0 || page_size == 0 {
        return Err(Error::Invalid("selected SQLite page geometry"));
    }
    let database_bytes = page_count
        .checked_mul(page_size)
        .ok_or(Error::Budget("selected SQLite page bytes"))?;
    let rebuild_reserve = database_bytes
        .checked_mul(2)
        .ok_or(Error::Budget("selected VACUUM rebuild reserve"))?;
    // std's pathname syscall conversion is admitted before the filesystem
    // call. This does not replace the actual isolation/storage owner.
    let path_bytes = candidate.as_os_str().as_bytes().len();
    let scratch = path_bytes
        .checked_add(1)
        .and_then(|n| n.checked_add(std::mem::size_of::<fs::Metadata>()))
        .ok_or(Error::Budget("selected VACUUM metadata workspace"))?;
    let _path_hold = state.hold(scratch)?;
    state.charge_work(path_bytes)?;
    state.active()?;
    let metadata = fs::symlink_metadata(candidate)?;
    state.active()?;
    if !metadata.file_type().is_file()
        || (metadata.dev(), metadata.ino()) != inode
        || metadata.len() != database_bytes
    {
        return Err(Error::Invalid("selected SQLite file/page mismatch"));
    }
    if database_bytes > limits.sqlite.max_output_bytes || rebuild_reserve > limits.max_temp_bytes {
        return Err(Error::Budget("selected VACUUM output/temp reserve"));
    }
    state.active()
}

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

fn carrier_schema_sql_error(error: tos_source_store::StoreError) -> Error {
    if error.code == tos_source_store::StoreErrorCode::BudgetExceeded {
        Error::Budget("carrier schema bounded SQL budget")
    } else {
        Error::Invalid("carrier schema bounded SQL refusal")
    }
}

/// Exact physical SQL emitted by SCHEMA plus the two owned ADD COLUMNs.
/// Hashes follow pinned SQLite ADD COLUMN's byte-prefix + ", " + column rule.
/// This checks schema bytes only; logical payload/root validation is separate.
pub(crate) fn verify_selected_payload_ddl(
    db: &Connection,
    layout: KnowledgePayloadLayout,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<()> {
    if layout == KnowledgePayloadLayout::InlineV1 {
        return Ok(());
    }
    let state = state.ok_or(Error::Invalid("carrier schema requires owned state"))?;
    const TABLES: [(&str, &str); 3] = [
        (
            "knowledge_nodes",
            "6c55a558a6fc5b9d780b8cf8772ceb3e8cb408e2411a0a0cc20b3159a6f747f7",
        ),
        (
            "knowledge_relations",
            "6ac6d24809beec061118ddb122ad9f341659f17936fb1949cda2e565bccde6ba",
        ),
        (
            "knowledge_source_carriers",
            "5fce2ae1dd99d857c864de07b99b9a05eeae963d665d1c6a2711f8f8c5a1438e",
        ),
    ];
    // Distinct caller frame remains live beside the bounded statement owner.
    // No stack/controller geometry is borrowed from that owner's allowance.
    type CallerFrame<'s, 'b> = (
        &'s Connection,
        KnowledgePayloadLayout,
        Option<&'s crate::d1_public_capture::CreationState<'b>>,
        std::array::IntoIter<(&'static str, &'static str), 3>,
        (&'static str, &'static str),
        Digest256,
        &'s [u8],
        std::slice::Chunks<'s, u8>,
        Result<()>,
    );
    let _frame_hold = state.hold(std::mem::size_of::<CallerFrame<'_, '_>>())?;
    for (table, expected) in TABLES {
        let expected = if layout.packed_bytes() && table == "knowledge_source_carriers" {
            "49e47dee375918b4ba0b22053bbd37538f495aa57ad683c2d1ecdbb58b61d4ca"
        } else {
            expected
        };
        state.active()?;
        const SQL: &std::ffi::CStr = c"SELECT CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=4096 THEN CAST(sql AS BLOB) ELSE NULL END FROM sqlite_master WHERE type='table' AND name=?1";
        state.charge_work(SQL.to_bytes().len())?;
        let workspace =
            tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound()
                .checked_add(std::mem::size_of::<Digest256Hasher>())
                .ok_or(Error::Budget("carrier schema controller geometry"))?;
        let _hold = state.hold(workspace)?;
        let mut statement =
            tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(db, SQL)
                .map_err(carrier_schema_sql_error)?;
        state.charge_work(table.len())?;
        statement
            .bind_text(1, table)
            .map_err(carrier_schema_sql_error)?;
        state.active()?;
        if !statement.step().map_err(carrier_schema_sql_error)? {
            return Err(Error::Invalid("carrier schema table absent"));
        }
        state.active()?;
        let rusqlite::types::ValueRef::Blob(raw) =
            statement.value_ref(0).map_err(carrier_schema_sql_error)?
        else {
            return Err(Error::Invalid("carrier schema DDL type/bytes"));
        };
        state.charge_work(raw.len())?;
        let mut hasher = Digest256Hasher::new();
        for part in raw.chunks(4096) {
            state.active()?;
            hasher.update(part);
        }
        let expected = Digest256::from_hex(expected)
            .map_err(|_| Error::Invalid("carrier schema expected digest"))?;
        if hasher.finalize() != expected {
            return Err(Error::Invalid("carrier selected physical DDL differs"));
        }
        state.active()?;
        if statement.step().map_err(carrier_schema_sql_error)? {
            return Err(Error::Invalid("carrier selected schema duplicate table"));
        }
        state.active()?;
    }
    state.active()?;
    Ok(())
}

pub(crate) const SELECTED_EXPLICIT_INDEXES: &[(&str, &str)] = &[
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

// source_order is UNIQUE in each table. The final text ID adds no ordering
// information to these V3 rowid indexes; exact identity remains in the table
// and its primary-key index. Older selected layouts retain their original SQL.
const COMPACT_ORDER_INDEXES: &[(&str, &str, &str)] = &[
    (
        "knowledge_nodes_source_order",
        "DROP INDEX knowledge_nodes_source_order",
        "CREATE INDEX knowledge_nodes_source_order ON knowledge_nodes(source_graph,source_order)",
    ),
    (
        "knowledge_nodes_entity",
        "DROP INDEX knowledge_nodes_entity",
        "CREATE INDEX knowledge_nodes_entity ON knowledge_nodes(entity_id,source_order)",
    ),
    (
        "knowledge_nodes_native",
        "DROP INDEX knowledge_nodes_native",
        "CREATE INDEX knowledge_nodes_native ON knowledge_nodes(native_id,source_order)",
    ),
    (
        "knowledge_relations_native",
        "DROP INDEX knowledge_relations_native",
        "CREATE INDEX knowledge_relations_native ON knowledge_relations(native_id,source_order)",
    ),
    (
        "knowledge_relations_source_order",
        "DROP INDEX knowledge_relations_source_order",
        "CREATE INDEX knowledge_relations_source_order ON knowledge_relations(source_graph,source_order)",
    ),
    (
        "knowledge_relations_from",
        "DROP INDEX knowledge_relations_from",
        "CREATE INDEX knowledge_relations_from ON knowledge_relations(from_id,source_order)",
    ),
    (
        "knowledge_relations_to",
        "DROP INDEX knowledge_relations_to",
        "CREATE INDEX knowledge_relations_to ON knowledge_relations(to_id,source_order)",
    ),
];
pub(crate) fn selected_index_sql<'s>(
    layout: KnowledgePayloadLayout,
    name: &str,
    legacy: &'s str,
) -> &'s str {
    if layout.dictionary_bytes() {
        for (compact_name, _, sql) in COMPACT_ORDER_INDEXES {
            if name == *compact_name {
                return sql;
            }
        }
    }
    legacy
}

pub(crate) fn selected_table_closure(db: &Connection) -> Result<()> {
    selected_table_closure_with_owned_state(db, None)
}

fn selected_table_closure_with_owned_state(
    db: &Connection,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<()> {
    selected_table_closure_with_layout_and_state(db, KnowledgePayloadLayout::InlineV1, state)
}

pub(crate) fn selected_table_closure_with_layout(
    db: &Connection,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    selected_table_closure_with_layout_and_state(db, layout, None)
}

fn selected_table_closure_with_layout_and_state(
    db: &Connection,
    layout: KnowledgePayloadLayout,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<()> {
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
    let node = 11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>();
    let key_count = TABLES.len()
        + 8
        + SELECTED_EXPLICIT_INDEXES.len()
        + crate::knowledge_corpus_original::INDEXES.len();
    let _closure_state=state.map(|state|state.hold(key_count.checked_mul(node+128)
        .and_then(|n|n.checked_add(1152+4*tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()))
        .ok_or(Error::Budget("owned selected closure state"))?)).transpose()?;
    if let Some(state) = state {
        state.active()?;
    }
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
        if let Some(state) = state {
            let raw = match row.get_ref(0)? {
                rusqlite::types::ValueRef::Text(raw) => raw,
                _ => return Err(Error::Budget("selected knowledge table name bytes")),
            };
            if raw.len() > 128 {
                return Err(Error::Budget("selected knowledge table name bytes"));
            }
            state.charge_work(
                raw.len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(key_count * 128))
                    .ok_or(Error::Budget("owned selected table comparison work"))?,
            )?;
        }
        let name: Option<String> = row.get(0)?;
        let Some(name) = name else {
            return Err(Error::Budget("selected knowledge table name bytes"));
        };
        if !(TABLES.contains(&name.as_str())
            || layout.uses_carriers() && name == "knowledge_source_carriers"
            || layout.dictionary_bytes() && name == "knowledge_byte_dictionaries"
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
            + if layout.uses_carriers() { 1 } else { 0 }
            + if layout.dictionary_bytes() { 1 } else { 0 }
            + if navigation_original { 3 } else { 0 }
            + if philosophy_original { 2 } else { 0 }
            + if corpus_original { 2 } else { 0 }
    {
        return Err(Error::Invalid("missing selected knowledge table"));
    }
    let mut statement = db.prepare(
        "SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN name ELSE NULL END,
                CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=1024 THEN sql ELSE NULL END
         FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name",
    )?;
    let mut rows = statement.query([])?;
    let mut indexes = BTreeSet::new();
    while let Some(row) = rows.next()? {
        if let Some(state) = state {
            let name = match row.get_ref(0)? {
                rusqlite::types::ValueRef::Text(raw) => raw,
                _ => return Err(Error::Budget("selected knowledge schema text bytes")),
            };
            let sql = match row.get_ref(1)? {
                rusqlite::types::ValueRef::Text(raw) => raw,
                _ => return Err(Error::Budget("selected knowledge schema text bytes")),
            };
            if name.len() > 128 || sql.len() > 1024 {
                return Err(Error::Budget("selected knowledge schema text bytes"));
            }
            let compare_bytes = SELECTED_EXPLICIT_INDEXES
                .iter()
                .chain(crate::knowledge_corpus_original::INDEXES.iter())
                .try_fold(0usize, |sum, (name, sql)| {
                    sum.checked_add(name.len())
                        .and_then(|n| n.checked_add(sql.len()))
                })
                .ok_or(Error::Budget("owned selected index comparisons"))?;
            state.charge_work(
                name.len()
                    .checked_add(sql.len())
                    .and_then(|n| n.checked_mul(2))
                    .and_then(|n| n.checked_add(compare_bytes + key_count * 128))
                    .ok_or(Error::Budget("owned selected index work"))?,
            )?;
        }
        let name: Option<String> = row.get(0)?;
        let sql: Option<String> = row.get(1)?;
        let (Some(name), Some(sql)) = (name, sql) else {
            return Err(Error::Budget("selected knowledge schema text bytes"));
        };
        if !(SELECTED_EXPLICIT_INDEXES
            .iter()
            .any(|(expected_name, expected_sql)| {
                name == *expected_name
                    && sql == selected_index_sql(layout, expected_name, expected_sql)
            })
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
        != SELECTED_EXPLICIT_INDEXES.len()
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
    controlled: Option<&NativeStageOwnedBudget<'_>>,
    deadline: Option<Instant>,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    // std filesystem pathname conversion may own a NUL spelling, separately
    // from the already-retained input Path and the sidecar path owners below.
    let path_bytes = path.as_os_str().as_encoded_bytes().len();
    let _fresh_path_hold = controlled
        .map(|budget| {
            budget.creation_state.charge_work(path_bytes)?;
            budget.creation_state.hold(
                path_bytes
                    .checked_add(1)
                    .and_then(|n| n.checked_add(std::mem::size_of::<fs::Metadata>() * 2))
                    .ok_or(Error::Budget("fresh filesystem path state"))?,
            )
        })
        .transpose()?;
    let metadata = fs::symlink_metadata(path)?;
    let opened = pinned.metadata()?;
    if !metadata.file_type().is_file()
        || (metadata.dev(), metadata.ino()) != (opened.dev(), opened.ino())
        || opened.len() > limits.max_output_bytes
    {
        return Err(Error::Budget("fresh selected SQLite bytes/type"));
    }
    if let Some(budget) = controlled {
        verify_fresh_sidecars_owned(path, budget.creation_state)?;
    } else if sqlite_sidecar_paths(path)
        .iter()
        .any(|sidecar| sidecar.exists() || sidecar.is_symlink())
    {
        return Err(Error::Invalid("fresh selected SQLite sidecar"));
    }
    // The original main hook remains live while this fresh verification hook
    // is installed. Hold its distinct Box before reserve/open/install and keep
    // it until this connection drops; never borrow a fresh SQLite grant.
    let _fresh_callback_hold = controlled
        .map(|budget| {
            budget
                .creation_state
                .hold(sqlite_budget::SharedVmWindow::callback_state_upper_bound())
        })
        .transpose()?;
    // Distinct retained connection Rust must remain admitted while every
    // verifier/settings statement is live. The once-process VFS and native
    // SQLite heap pool are still retained by the original session owner.
    let connection_bytes =
        tos_source_store::PinnedSqliteConnection::immutable_retained_rust_state_upper_bound();
    let _fresh_connection_hold = controlled
        .map(|budget| budget.creation_state.hold(connection_bytes))
        .transpose()?;
    // Caller has already retained the once-process SourceStore Rust VFS owner.
    // This open retains its distinct File/connection owner under the same model
    // remainder; the native SQLite allocator remains in the one shared heap.
    let window = controlled
        .map(|budget| {
            budget.admit(0)?;
            sqlite_budget::SharedVmWindow::reserve(Arc::clone(&used), budget.original_sql_limit)
        })
        .transpose()?;
    let db = if let Some(budget) = controlled {
        let remaining = |extra: usize| {
            // SourceStore's sole opening preflight includes this exact retained
            // connection alias. It is already held above; only its opening
            // workspace is prospective here, never the process pool/VFS/hook.
            let extra = extra.checked_sub(connection_bytes).ok_or_else(|| {
                tos_source_store::StoreError::new(
                    tos_source_store::StoreErrorCode::BudgetExceeded,
                    "owned fresh connection census alias",
                )
            })?;
            budget.admit(extra).map_err(|_| {
                tos_source_store::StoreError::new(
                    tos_source_store::StoreErrorCode::BudgetExceeded,
                    "owned stage state refusal",
                )
            })
        };
        tos_source_store::PinnedSqliteConnection::open_readonly_immutable_with_state(
            pinned, &remaining,
        )
        .map_err(|error| owned_stage_sql_error(error))?
    } else {
        tos_source_store::PinnedSqliteConnection::open_readonly_immutable(pinned)
            .map_err(|error| Error::Source(error.to_string()))?
    };
    if let Some(window) = window {
        window.install(
            &db,
            deadline.ok_or(Error::Invalid("owned selected stage deadline"))?,
            Arc::clone(&controlled.expect("owned stage budget").cancelled),
        );
    } else {
        sqlite_budget::install_progress(&db, limits, used);
    }
    if let Some(budget) = controlled {
        if db
            .retained_rust_state_upper_bound()
            .map_err(|error| owned_stage_sql_error(error))?
            > connection_bytes
        {
            return Err(Error::Budget(
                "fresh connection Rust exceeds original admission",
            ));
        }
        configure_fresh_readonly_owned(
            &db,
            u64::from(limits.sqlite_cache_kib),
            budget.creation_state,
        )?;
    } else {
        db.pragma_update(None, "cache_size", -(limits.sqlite_cache_kib as i64))?;
        db.execute_batch("PRAGMA temp_store=FILE")?;
    }
    if layout.uses_carriers() {
        crate::knowledge_selected::verify_schema_with_layout(
            &db,
            layout,
            controlled.map(|budget| budget.creation_state),
        )?;
    } else {
        crate::knowledge_selected::verify_schema(&db)?;
    }
    if let Some(budget) = controlled {
        verify_fresh_integrity_owned(&db, budget.creation_state)?;
    } else {
        let integrity: String = db.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        let freelist: u64 = db.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        if integrity != "ok" || freelist != 0 {
            return Err(Error::Invalid("fresh selected SQLite integrity/pages"));
        }
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
pub(crate) fn root_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}
#[track_caller]
fn owned_stage_sql_error(error: tos_source_store::StoreError) -> Error {
    // StoreError.detail is a code-owned static literal, never SQLite's native
    // message or a bound value. Preserve the failing operation and source site.
    let site = std::panic::Location::caller();
    eprintln!(
        "Native stage SQL refused at {}:{}: {:?}: {}",
        site.file(),
        site.line(),
        error.code,
        error.detail
    );
    if error.code == tos_source_store::StoreErrorCode::BudgetExceeded {
        Error::Budget(error.detail)
    } else {
        Error::Invalid(error.detail)
    }
}
#[track_caller]
fn owned_stage_connection_sql_error(db: &Connection, error: tos_source_store::StoreError) -> Error {
    // The connection is still borrowed by this operation. Read only SQLite's
    // numeric extended status; private SQL, paths and row values stay private.
    let status = unsafe { rusqlite::ffi::sqlite3_extended_errcode(db.handle()) };
    eprintln!("Native stage SQLite status: {status}");
    owned_stage_sql_error(error)
}

// Fixed SQL spelling workspace is admitted before initialization/formatting.
// The installed prepaid hook and process pool remain the original Stage owners.
fn configure_stage_temp_cap_owned(
    db: &Connection,
    max_temp_bytes: u64,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    struct Sql {
        bytes: [u8; 96],
        len: usize,
    }
    impl std::fmt::Write for Sql {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            let end = self.len.checked_add(value.len()).ok_or(std::fmt::Error)?;
            if end >= self.bytes.len() {
                return Err(std::fmt::Error);
            }
            self.bytes[self.len..end].copy_from_slice(value.as_bytes());
            self.len = end;
            Ok(())
        }
    }
    use std::fmt::Write;
    let _frame_hold = state.hold(
        std::mem::size_of::<Sql>()
            + 3 * std::mem::size_of::<u64>()
            + 2 * std::mem::size_of::<Result<i64>>(),
    )?;
    let page_size = u64::try_from(stage_integer_owned(db, c"PRAGMA temp.page_size", state)?)
        .ok()
        .filter(|n| *n != 0)
        .ok_or(Error::Invalid("owned Stage TEMP page size"))?;
    let pages = max_temp_bytes / page_size;
    if pages == 0 || pages > i64::MAX as u64 {
        return Err(Error::Budget("owned Stage TEMP page cap"));
    }
    state.charge_work(96 + 95)?;
    let mut sql = Sql {
        bytes: [0; 96],
        len: 0,
    };
    write!(&mut sql, "PRAGMA temp.max_page_count={pages}")
        .map_err(|_| Error::Budget("owned Stage TEMP SQL spelling"))?;
    let sql = std::ffi::CStr::from_bytes_with_nul(&sql.bytes[..sql.len + 1])
        .map_err(|_| Error::Invalid("owned Stage TEMP SQL spelling"))?;
    let applied = stage_integer_owned(db, sql, state)?;
    if applied <= 0 || applied as u64 > pages {
        return Err(Error::Invalid("owned Stage TEMP page cap unavailable"));
    }
    state.active()
}

fn verify_fresh_sidecars_owned(
    path: &Path,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    let base = path.as_os_str();
    let cap = base
        .as_encoded_bytes()
        .len()
        .checked_add(8)
        .ok_or(Error::Budget("fresh sidecar path bytes"))?;
    let bytes = cap
        .checked_mul(3)
        .and_then(|n| n.checked_add(cap + 1))
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<[PathBuf; 3]>() + std::mem::size_of::<fs::Metadata>())
        })
        .ok_or(Error::Budget("fresh sidecar state"))?;
    let _hold = state.hold(bytes)?;
    // Exact pre-reserved spelling prevents OsString append growth/reallocation
    // while the previous three path buffers are simultaneously live.
    let make = |suffix: &str| -> Result<PathBuf> {
        state.charge_work(
            base.as_encoded_bytes()
                .len()
                .checked_add(suffix.len())
                .ok_or(Error::Budget("fresh sidecar copy work"))?,
        )?;
        let mut name = std::ffi::OsString::with_capacity(cap);
        name.push(base);
        name.push(suffix);
        Ok(PathBuf::from(name))
    };
    let sidecars = [make("-journal")?, make("-wal")?, make("-shm")?];
    for sidecar in &sidecars {
        state.charge_work(sidecar.as_os_str().as_encoded_bytes().len())?;
        match fs::symlink_metadata(sidecar) {
            Ok(_) => return Err(Error::Invalid("fresh selected SQLite sidecar")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(Error::Io(error)),
        }
    }
    state.active()
}

fn configure_fresh_readonly_owned(
    db: &Connection,
    cache_kib: u64,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    struct Sql {
        bytes: [u8; 96],
        len: usize,
    }
    impl std::fmt::Write for Sql {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let end = self.len.checked_add(text.len()).ok_or(std::fmt::Error)?;
            if end >= self.bytes.len() {
                return Err(std::fmt::Error);
            }
            self.bytes[self.len..end].copy_from_slice(text.as_bytes());
            self.len = end;
            Ok(())
        }
    }
    use std::fmt::Write;
    let _sql_hold = state.hold(std::mem::size_of::<Sql>() + 2 * std::mem::size_of::<i64>())?;
    let cache = i64::try_from(cache_kib).map_err(|_| Error::Budget("fresh cache integer"))?;
    state.charge_work(96 + 95)?;
    let mut sql = Sql {
        bytes: [0; 96],
        len: 0,
    };
    write!(&mut sql, "PRAGMA cache_size={}", -cache)
        .map_err(|_| Error::Budget("fresh cache SQL"))?;
    let text = std::ffi::CStr::from_bytes_with_nul(&sql.bytes[..sql.len + 1])
        .map_err(|_| Error::Invalid("fresh cache SQL"))?;
    // Assignment PRAGMA does not return a row, so use the same bounded batch
    // kernel after its explicit spelling and workspace have been admitted.
    state.charge_work(text.to_bytes().len())?;
    let _statement_hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let mut statement =
        tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(db, text)
            .map_err(|error| owned_stage_sql_error(error))?;
    state.active()?;
    if statement
        .step()
        .map_err(|error| owned_stage_sql_error(error))?
    {
        return Err(Error::Invalid("fresh cache unexpected row"));
    }
    drop(statement);
    stage_batch_owned(db, c"PRAGMA temp_store=FILE", state)?;
    if stage_integer_owned(db, c"PRAGMA cache_size", state)? != -cache
        || stage_integer_owned(db, c"PRAGMA temp_store", state)? != 1
    {
        return Err(Error::Invalid("fresh cache/temp setting differs"));
    }
    state.active()
}

fn verify_fresh_integrity_owned(
    db: &Connection,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    state.charge_work(c"PRAGMA integrity_check".to_bytes().len())?;
    let _hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let mut statement = tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(
        db,
        c"PRAGMA integrity_check",
    )
    .map_err(|error| owned_stage_sql_error(error))?;
    state.active()?;
    if !statement
        .step()
        .map_err(|error| owned_stage_sql_error(error))?
    {
        return Err(Error::Invalid("fresh integrity absent"));
    }
    let text = match statement
        .value_ref(0)
        .map_err(|error| owned_stage_sql_error(error))?
    {
        rusqlite::types::ValueRef::Text(raw) => raw,
        _ => return Err(Error::Invalid("fresh integrity type")),
    };
    // Any other SQLite diagnostic row is rejected while still borrowed. It
    // never becomes an unpriced String or UTF8 scan in the Rust owner.
    if text.len() != 2 {
        return Err(Error::Invalid("fresh integrity differs"));
    }
    state.charge_work(2)?;
    if text != b"ok" {
        return Err(Error::Invalid("fresh integrity differs"));
    }
    state.active()?;
    if statement
        .step()
        .map_err(|error| owned_stage_sql_error(error))?
    {
        return Err(Error::Invalid("fresh integrity extra row"));
    }
    drop(statement);
    if stage_integer_owned(db, c"PRAGMA freelist_count", state)? != 0 {
        return Err(Error::Invalid("fresh selected SQLite integrity/pages"));
    }
    state.active()
}

fn stage_stream_digest_owned(
    file: &mut fs::File,
    max_bytes: u64,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<(String, u64)> {
    // The receipt forecast already retains the one resulting 64-byte hex
    // string. The actual kernel buffer and callback controller live only here.
    let _hold = state.hold(
        65536
            + std::mem::size_of::<Digest256Hasher>()
            + std::mem::size_of::<(bool, u64)>()
            + std::mem::size_of::<Result<(String, u64)>>(),
    )?;
    state.charge_work(65536 + 64)?; // buffer initialization and final hex spelling
    state.active()?;
    let mut before_read = true;
    let mut observed = 0u64;
    let callback = |bytes: usize| {
        state.active()?;
        if before_read {
            // Prepaid request ceiling: a short/failed read does not renew or
            // refund this original byte-work reservation. It is not labeled
            // exact observed IO usage. The maintained kernel requests 64 KiB.
            state.charge_work(65536)?;
            before_read = false;
        } else {
            before_read = true;
            observed = observed
                .checked_add(bytes as u64)
                .filter(|n| *n <= max_bytes)
                .ok_or(Error::Budget("owned stage digest bytes"))?;
            state.charge_work(bytes)?; // actual hash traversal, before update
        }
        state.active()
    };
    // This is the actual closure target moved into the maintained kernel;
    // its captured owner references/cap are distinct from captured scalars.
    let _callback_hold = state.hold(std::mem::size_of_val(&callback))?;
    crate::stream_digest_with_check(file, callback)
}

fn stage_integrity_first_row_owned(
    db: &Connection,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<bool> {
    state.charge_work(c"PRAGMA integrity_check".to_bytes().len())?;
    let _hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let mut statement = tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(
        db,
        c"PRAGMA integrity_check",
    )
    .map_err(|error| owned_stage_sql_error(error))?;
    state.active()?;
    if !statement
        .step()
        .map_err(|error| owned_stage_sql_error(error))?
    {
        return Err(Error::Invalid("stage integrity absent"));
    }
    let text = match statement
        .value_ref(0)
        .map_err(|error| owned_stage_sql_error(error))?
    {
        rusqlite::types::ValueRef::Text(raw) => raw,
        _ => return Err(Error::Invalid("stage integrity type")),
    };
    // Preserve the maintained first-row predicate without materializing any
    // diagnostic text or scanning unbounded UTF8 on a refusal path.
    let valid = if text.len() == 2 {
        state.charge_work(2)?;
        text == b"ok"
    } else {
        false
    };
    state.active()?;
    Ok(valid)
}

fn stage_integer_owned(
    db: &Connection,
    sql: &std::ffi::CStr,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<i64> {
    state.active()?;
    state.charge_work(sql.to_bytes().len())?;
    let _hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let mut statement =
        tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(db, sql)
            .map_err(|error| owned_stage_sql_error(error))?;
    state.active()?;
    if !statement
        .step()
        .map_err(|error| owned_stage_sql_error(error))?
    {
        return Err(Error::Invalid("owned stage scalar row absent"));
    }
    let value = statement
        .integer(0)
        .map_err(|error| owned_stage_sql_error(error))?;
    state.active()?;
    if statement
        .step()
        .map_err(|error| owned_stage_sql_error(error))?
    {
        return Err(Error::Invalid("owned stage scalar multiple rows"));
    }
    state.active()?;
    Ok(value)
}

fn stage_batch_owned(
    db: &Connection,
    sql: &'static std::ffi::CStr,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    state.charge_work(sql.to_bytes().len())?;
    let _hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let check = || {
        state.active().map_err(|_| {
            tos_source_store::StoreError::new(
                tos_source_store::StoreErrorCode::BudgetExceeded,
                "owned stage SQL active refusal",
            )
        })
    };
    tos_source_store::PinnedBoundedStatement::execute_static_batch_on_owned_connection(
        db, sql, &check,
    )
    .map_err(|error| owned_stage_connection_sql_error(db, error))
}

// These are borrowed values on the existing fixed row frame, not a heap or
// a new SQL/domain representation. SQLite copies remain in the shared heap pool.
enum StageSqlBinding<'a> {
    Text(&'a str),
    OptionalText(Option<&'a str>),
    Integer(i64),
    Blob(&'a [u8]),
}
#[track_caller]
fn stage_insert_owned(
    db: &Connection,
    sql: &std::ffi::CStr,
    values: &[StageSqlBinding<'_>],
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    state.charge_work(sql.to_bytes().len())?;
    let _statement_hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let mut statement =
        tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(db, sql)
            .map_err(|error| owned_stage_connection_sql_error(db, error))?;
    for (slot, value) in values.iter().enumerate() {
        state.active()?;
        let index =
            i32::try_from(slot + 1).map_err(|_| Error::Budget("owned stage SQL binding count"))?;
        match value {
            StageSqlBinding::Text(text) | StageSqlBinding::OptionalText(Some(text)) => {
                state.charge_work(text.len())?;
                statement
                    .bind_text(index, text)
                    .map_err(|error| owned_stage_connection_sql_error(db, error))?;
            }
            StageSqlBinding::OptionalText(None) => statement
                .bind_null(index)
                .map_err(|error| owned_stage_connection_sql_error(db, error))?,
            StageSqlBinding::Integer(value) => statement
                .bind_i64(index, *value)
                .map_err(|error| owned_stage_connection_sql_error(db, error))?,
            StageSqlBinding::Blob(bytes) => {
                state.charge_work(bytes.len())?;
                statement
                    .bind_blob(index, bytes)
                    .map_err(|error| owned_stage_connection_sql_error(db, error))?;
            }
        }
    }
    state.active()?;
    let row = match statement.step() {
        Ok(row) => row,
        Err(error) => {
            // Capture the original refusal before any diagnostic SQL changes
            // SQLite's last-error state. No row values or SQL text are emitted.
            let code = unsafe { rusqlite::ffi::sqlite3_extended_errcode(db.handle()) };
            let error = owned_stage_connection_sql_error(db, error);
            drop(statement);
            let site = std::panic::Location::caller();
            eprintln!(
                "Native stage insert refused at {}:{}",
                site.file(),
                site.line()
            );
            if code & 0xff == rusqlite::ffi::SQLITE_FULL {
                for (label, query) in [
                    ("main.page_size", c"PRAGMA main.page_size"),
                    ("main.page_count", c"PRAGMA main.page_count"),
                    ("main.max_page_count", c"PRAGMA main.max_page_count"),
                    ("temp.page_size", c"PRAGMA temp.page_size"),
                    ("temp.page_count", c"PRAGMA temp.page_count"),
                    ("temp.max_page_count", c"PRAGMA temp.max_page_count"),
                ] {
                    // Each observation uses the original work/state/deadline
                    // ledger. A failed observation cannot replace the refusal.
                    if let Ok(value) = stage_integer_owned(db, query, state) {
                        eprintln!("Native stage pager {label}={value}");
                    } else {
                        break;
                    }
                }
            }
            return Err(error);
        }
    };
    if row {
        return Err(Error::Invalid(
            "owned stage insert unexpectedly returned row",
        ));
    }
    state.active()
}

fn stage_root_owned(
    db: &Connection,
    sql: &std::ffi::CStr,
    entry: Option<&InputCollectionReceipt>,
    output: bool,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<(u64, String)> {
    state.charge_work(sql.to_bytes().len())?;
    let _statement_hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let mut statement =
        tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(db, sql)
            .map_err(|error| owned_stage_sql_error(error))?;
    if let Some(entry) = entry {
        statement
            .bind_text(1, &entry.source_graph)
            .map_err(|error| owned_stage_sql_error(error))?;
        statement
            .bind_text(2, &entry.collection)
            .map_err(|error| owned_stage_sql_error(error))?;
    }
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    loop {
        state.active()?;
        if !statement
            .step()
            .map_err(|error| owned_stage_sql_error(error))?
        {
            break;
        }
        let raw_id = match statement
            .value_ref(0)
            .map_err(|error| owned_stage_sql_error(error))?
        {
            rusqlite::types::ValueRef::Text(raw) => raw,
            _ => return Err(Error::Invalid("owned stage root id type")),
        };
        if raw_id.len() > MAX_NAME_BYTES {
            return Err(Error::Budget("owned stage root id bytes"));
        }
        state.charge_work(raw_id.len())?;
        let id =
            std::str::from_utf8(raw_id).map_err(|_| Error::Invalid("owned stage root id UTF8"))?;
        let digest = match statement
            .value_ref(if output { 3 } else { 1 })
            .map_err(|error| owned_stage_sql_error(error))?
        {
            rusqlite::types::ValueRef::Blob(raw) => raw,
            _ => return Err(Error::Invalid("owned stage root digest type")),
        };
        if digest.len() != 32 {
            return Err(Error::Invalid("stage payload digest size"));
        }
        if output {
            let order = statement
                .integer(2)
                .map_err(|error| owned_stage_sql_error(error))?;
            if order < 0 || order as u64 != count {
                return Err(Error::Invalid("stage output source order/digest"));
            }
        }
        state.charge_work(
            id.len()
                .checked_add(digest.len())
                .ok_or(Error::Budget("owned stage root hash work"))?,
        )?;
        root_item(&mut hash, id, digest);
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("stage root rows"))?;
    }
    state.retain(64)?;
    Ok((count, hash.finalize().to_hex()))
}

fn input_root(db: &Connection, entry: &InputCollectionReceipt) -> Result<(u64, String)> {
    input_root_with_state(db, entry, None)
}
fn input_root_with_state(
    db: &Connection,
    entry: &InputCollectionReceipt,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<(u64, String)> {
    if let Some(state) = state {
        return stage_root_owned(db,c"SELECT id,payload_sha256 FROM raw_records WHERE source_graph=?1 AND collection=?2 ORDER BY id",Some(entry),false,state);
    }
    let _stmt_hold=state.map(|s|s.hold(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())).transpose()?;
    let mut statement = db.prepare(
        "SELECT id,payload_sha256 FROM raw_records
      WHERE source_graph=?1 AND collection=?2 ORDER BY id",
    )?;
    let mut rows = statement.query(params![entry.source_graph, entry.collection])?;
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        let id_raw = row.get_ref(0)?;
        if let Some(state) = state {
            let raw = match id_raw {
                rusqlite::types::ValueRef::Text(raw) => raw,
                _ => return Err(Error::Invalid("owned stage root id type")),
            };
            if raw.len() > MAX_NAME_BYTES {
                return Err(Error::Budget("owned stage root id bytes"));
            }
            state.charge_work(raw.len())?;
        }
        let id = id_raw
            .as_str()
            .map_err(|_| Error::Invalid("selected Stage SQL text column"))?;
        let digest = row
            .get_ref(1)?
            .as_blob()
            .map_err(|_| Error::Invalid("selected Stage SQL blob column"))?;
        if digest.len() != 32 {
            return Err(Error::Invalid("stage payload digest size"));
        }
        if let Some(state) = state {
            state.charge_work(
                id.len()
                    .checked_add(digest.len())
                    .ok_or(Error::Budget("owned Stage input hash work"))?,
            )?;
        }
        root_item(&mut hash, id, digest);
        count = count.checked_add(1).ok_or(Error::Budget("input rows"))?;
    }
    if let Some(state) = state {
        state.retain(64)?;
    }
    Ok((count, hash.finalize().to_hex()))
}
fn output_root(db: &Connection, table: &str) -> Result<(u64, String)> {
    output_root_with_state(db, table, None)
}
fn output_root_with_state(
    db: &Connection,
    table: &str,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<(u64, String)> {
    if let Some(state) = state {
        let sql=match table {
            "knowledge_nodes"=>c"SELECT id,source_graph,source_order,payload_sha256 FROM knowledge_nodes ORDER BY source_graph,id",
            "knowledge_relations"=>c"SELECT id,source_graph,source_order,payload_sha256 FROM knowledge_relations ORDER BY source_graph,id",
            _=>return Err(Error::Invalid("unknown stage output table")),
        };
        return stage_root_owned(db, sql, None, true, state);
    }
    let _stmt_hold=state.map(|s|s.hold(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())).transpose()?;
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
        let id_raw = row.get_ref(0)?;
        if let Some(state) = state {
            let raw = match id_raw {
                rusqlite::types::ValueRef::Text(raw) => raw,
                _ => return Err(Error::Invalid("owned stage root id type")),
            };
            if raw.len() > MAX_NAME_BYTES {
                return Err(Error::Budget("owned stage root id bytes"));
            }
            state.charge_work(raw.len())?;
        }
        let id = id_raw
            .as_str()
            .map_err(|_| Error::Invalid("selected Stage SQL text column"))?;
        let order: i64 = row.get(2)?;
        let digest = row
            .get_ref(3)?
            .as_blob()
            .map_err(|_| Error::Invalid("selected Stage SQL blob column"))?;
        if digest.len() != 32 || order < 0 || order as u64 != count {
            return Err(Error::Invalid("stage output source order/digest"));
        }
        if let Some(state) = state {
            state.charge_work(
                id.len()
                    .checked_add(digest.len())
                    .ok_or(Error::Budget("owned Stage output hash work"))?,
            )?;
        }
        root_item(&mut hash, id, digest);
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("stage output rows"))?;
    }
    if let Some(state) = state {
        state.retain(64)?;
    }
    Ok((count, hash.finalize().to_hex()))
}
fn read_seek_row_bounded(
    statement: &tos_source_store::PinnedBoundedStatement<'_>,
    state: &crate::d1_public_capture::CreationState<'_>,
    cap: usize,
) -> Result<SeekRow> {
    use rusqlite::types::ValueRef;
    let text = |index| match statement
        .value_ref(index)
        .map_err(|error| owned_stage_sql_error(error))?
    {
        ValueRef::Text(raw) => Ok(raw),
        _ => Err(Error::Invalid("stage bounded seek text type")),
    };
    let id_raw = text(0)?;
    let graph_raw = text(1)?;
    if id_raw.len() > MAX_NAME_BYTES || graph_raw.len() > MAX_NAME_BYTES {
        return Err(Error::Budget("owned stage seek text bytes"));
    }
    state.charge_work(
        id_raw
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(graph_raw.len()))
            .ok_or(Error::Budget("owned stage seek text traversal"))?,
    )?;
    let id = std::str::from_utf8(id_raw).map_err(|_| Error::Invalid("stage seek id UTF8"))?;
    let graph =
        std::str::from_utf8(graph_raw).map_err(|_| Error::Invalid("stage seek graph UTF8"))?;
    valid_id(id)?;
    let order = match statement
        .value_ref(2)
        .map_err(|error| owned_stage_sql_error(error))?
    {
        ValueRef::Null => None,
        ValueRef::Integer(order) => Some(order),
        _ => return Err(Error::Invalid("stage bounded seek order type")),
    };
    let payload = match statement
        .value_ref(3)
        .map_err(|error| owned_stage_sql_error(error))?
    {
        ValueRef::Blob(raw) => raw,
        _ => return Err(Error::Invalid("stage seek payload")),
    };
    if payload.len() > cap {
        return Err(Error::Budget("stage seek row bytes"));
    }
    let digest: [u8; 32] = match statement
        .value_ref(4)
        .map_err(|error| owned_stage_sql_error(error))?
    {
        ValueRef::Blob(raw) => raw
            .try_into()
            .map_err(|_| Error::Invalid("stage seek digest bytes"))?,
        _ => return Err(Error::Invalid("stage seek digest")),
    };
    state.retain(
        std::mem::size_of::<SeekRow>()
            .checked_add(id.len())
            .and_then(|n| n.checked_add(graph.len()))
            .and_then(|n| n.checked_add(payload.len()))
            .and_then(|n| n.checked_add(128))
            .ok_or(Error::Budget("owned stage seek state"))?,
    )?;
    // Payload copy and the unchanged subsequent digest verification both walk
    // these bytes; admit both before the first copy, including terminal errors.
    state.charge_work(
        payload
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(id.len()))
            .and_then(|n| n.checked_add(graph.len()))
            .ok_or(Error::Budget("owned stage seek work"))?,
    )?;
    Ok(SeekRow {
        id: id.to_owned(),
        source_graph: graph.to_owned(),
        source_order: order,
        payload: payload.to_owned(),
        payload_sha256: Digest256::from_bytes(digest).to_hex(),
    })
}

fn read_seek_row_with_state(
    row: &rusqlite::Row<'_>,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
    cap: usize,
) -> Result<SeekRow> {
    let Some(state) = state else {
        return Ok(read_seek_row(row)?);
    };
    use rusqlite::types::ValueRef;
    let id_raw = match row.get_ref(0)? {
        ValueRef::Text(raw) => raw,
        _ => return Err(Error::Invalid("stage seek id")),
    };
    let graph_raw = match row.get_ref(1)? {
        ValueRef::Text(raw) => raw,
        _ => return Err(Error::Invalid("stage seek graph")),
    };
    if id_raw.len() > MAX_NAME_BYTES || graph_raw.len() > MAX_NAME_BYTES {
        return Err(Error::Budget("owned stage seek text bytes"));
    }
    state.charge_work(
        id_raw
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(graph_raw.len()))
            .ok_or(Error::Budget("owned stage seek text traversal"))?,
    )?;
    let id = std::str::from_utf8(id_raw).map_err(|_| Error::Invalid("stage seek id UTF8"))?;
    let graph =
        std::str::from_utf8(graph_raw).map_err(|_| Error::Invalid("stage seek graph UTF8"))?;
    valid_id(id)?;
    if graph.len() > 4096 {
        return Err(Error::Budget("stage seek graph bytes"));
    }
    let payload = match row.get_ref(3)? {
        ValueRef::Blob(value) => value,
        _ => return Err(Error::Invalid("stage seek payload")),
    };
    if payload.len() > cap {
        return Err(Error::Budget("stage seek row bytes"));
    }
    let digest: [u8; 32] = match row.get_ref(4)? {
        ValueRef::Blob(value) => value
            .try_into()
            .map_err(|_| Error::Invalid("stage seek digest bytes"))?,
        _ => return Err(Error::Invalid("stage seek digest")),
    };
    state.retain(
        std::mem::size_of::<SeekRow>()
            .checked_add(id.len())
            .and_then(|n| n.checked_add(graph.len()))
            .and_then(|n| n.checked_add(payload.len()))
            .and_then(|n| n.checked_add(128))
            .ok_or(Error::Budget("owned stage seek state"))?,
    )?;
    state.charge_work(
        payload
            .len()
            .checked_add(id.len())
            .and_then(|n| n.checked_add(graph.len()))
            .ok_or(Error::Budget("owned stage seek work"))?,
    )?;
    Ok(SeekRow {
        id: id.to_owned(),
        source_graph: graph.to_owned(),
        source_order: row.get(2)?,
        payload: payload.to_owned(),
        payload_sha256: Digest256::from_bytes(digest).to_hex(),
    })
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

// One normalized schema literal preserves exact SQL for both physical layouts.
// Native construction starts with the established Inline ABI. Only the explicit
// pristine CarrierOnce transition replaces these empty tables with rowid storage.
macro_rules! normalized_schema_literal { ($id_null:literal, $normalized_storage:literal) => { concat!(r#"
CREATE TABLE knowledge_nodes(
 id TEXT PRIMARY KEY"#, $id_null, r#",source_graph TEXT NOT NULL,native_id TEXT,entity_id TEXT,
 kind_id TEXT NOT NULL,type_id TEXT NOT NULL,source_order INTEGER NOT NULL UNIQUE,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL)"#, $normalized_storage, r#";
CREATE INDEX knowledge_nodes_source_order ON knowledge_nodes(source_graph,source_order,id);
CREATE INDEX knowledge_nodes_kind ON knowledge_nodes(kind_id,source_order);
CREATE INDEX knowledge_nodes_entity ON knowledge_nodes(entity_id,source_order,id);
CREATE INDEX knowledge_nodes_native ON knowledge_nodes(native_id,source_order,id);
CREATE INDEX knowledge_nodes_entity_id ON knowledge_nodes(entity_id,id);
CREATE TABLE knowledge_relations(
 id TEXT PRIMARY KEY"#, $id_null, r#",source_graph TEXT NOT NULL,native_id TEXT,
 from_id TEXT NOT NULL,to_id TEXT NOT NULL,predicate_id TEXT NOT NULL,
 relation_type_id TEXT NOT NULL,source_order INTEGER NOT NULL UNIQUE,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL)"#, $normalized_storage, r#";
CREATE INDEX knowledge_relations_source_order ON knowledge_relations(source_graph,source_order,id);
CREATE INDEX knowledge_relations_native ON knowledge_relations(native_id,source_order,id);
CREATE INDEX knowledge_relations_from ON knowledge_relations(from_id,source_order,id);
CREATE INDEX knowledge_relations_to ON knowledge_relations(to_id,source_order,id);
CREATE INDEX knowledge_relations_from_id ON knowledge_relations(from_id,id);
CREATE INDEX knowledge_relations_to_id ON knowledge_relations(to_id,id);
CREATE INDEX knowledge_relations_predicate ON knowledge_relations(predicate_id,source_order);
"#) }; }
macro_rules! stage_schema_literal {
    ($raw_location:literal, $raw_storage:literal) => {
        concat!(
            "\nCREATE ",
            $raw_location,
            r#"TABLE raw_records(
 source_graph TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL,
 PRIMARY KEY(source_graph,collection,id))"#,
            $raw_storage,
            ";\n",
            normalized_schema_literal!("", " WITHOUT ROWID")
        )
    };
}
const SCHEMA: &str = stage_schema_literal!("", " WITHOUT ROWID");
const SCHEMA_C: &std::ffi::CStr = match std::ffi::CStr::from_bytes_with_nul(
    concat!(stage_schema_literal!("", " WITHOUT ROWID"), "\0").as_bytes(),
) {
    Ok(value) => value,
    Err(_) => panic!("stage static schema contains interior NUL"),
};

// Native inputs keep their exact bytes in a disposable rowid table. The 16-KiB
// native pager keeps medium source records on leaf pages; WITHOUT ROWID would
// spill these bytes at its much smaller index-cell threshold. All raw readers
// retain the same composite unique key and explicit ordering. This physical
// choice does not alter the normalized payload ABI or either byte ceiling.
const NATIVE_SCHEMA: &str = stage_schema_literal!("TEMP ", "");
const NATIVE_SCHEMA_C: &std::ffi::CStr = match std::ffi::CStr::from_bytes_with_nul(
    concat!(stage_schema_literal!("TEMP ", ""), "\0").as_bytes(),
) {
    Ok(value) => value,
    Err(_) => panic!("native stage static schema contains interior NUL"),
};
const CARRIER_NORMALIZED_SCHEMA_C: &std::ffi::CStr = match std::ffi::CStr::from_bytes_with_nul(
    concat!(
        "DROP TABLE knowledge_relations; DROP TABLE knowledge_nodes;",
        normalized_schema_literal!(" NOT NULL", ""),
        "\0"
    )
    .as_bytes(),
) {
    Ok(value) => value,
    Err(_) => panic!("carrier normalized schema contains interior NUL"),
};

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
    fn native_dictionary_stage_roundtrips_exact_source_and_normalized_bytes() {
        const CHILD: &str = "TOS_DICTIONARY_STAGE_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "knowledge_stage::tests::native_dictionary_stage_roundtrips_exact_source_and_normalized_bytes", "--nocapture"])
                .env(CHILD, "1").output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            return;
        }
        use crate::knowledge_payload_read::RuntimeKnowledgeOwnedBudget;
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        let cancelled = Arc::new(AtomicBool::new(false));
        let remaining = |bytes: usize| {
            (32 * 1024 * 1024usize)
                .checked_sub(bytes)
                .ok_or(Error::Budget("dictionary Stage test retained bytes"))
        };
        let heap = sqlite_budget::DedicatedSessionSqliteHeap::establish(
            8 * 1024 * 1024,
            &remaining,
            deadline,
            &cancelled,
        )
        .unwrap();
        let work = Arc::new(AtomicU64::new(0));
        let vm = Arc::new(AtomicU64::new(0));
        let budget = RuntimeKnowledgeOwnedBudget {
            remaining_after_retained: &remaining,
            original_work: &work,
            original_work_limit: 256 * 1024 * 1024,
            original_sql_vm: &vm,
            original_sql_vm_limit: 10_000_000,
            original_sqlite_heap: &heap,
            remaining_json_visits: 1_000_000,
            owner_deadline: deadline,
            operation_deadline: deadline,
            cancelled: &cancelled,
        };
        let state =
            crate::d1_public_capture::CreationState::from_runtime_owned_budget(&budget).unwrap();
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        // V3 seals on the first large row; V4 gathers up to 32 KiB or 32 samples.
        // Exact source spelling differs from the normalized representation.
        let source = format!(
            "{{ \"text\" : \"{}\", \"number\" : 1.2300, \"claim_ref\" : \"claim.test\" }}\n",
            "source words ".repeat(500)
        )
        .into_bytes();
        let source_value: serde_json::Value = serde_json::from_slice(&source).unwrap();
        {
            let logical = serde_json::json!({"id": "codec.exact", "attributes": {"text": source_value["text"]},
                "source_record": {"payload": source_value, "field_map": {"attributes.text": "/text"}}});
            let raw = serde_json::to_vec(&logical).unwrap();
            let mut mismatched = logical.clone();
            mismatched["id"] = serde_json::json!("codec.other");
            let json_limits =
                crate::knowledge_normalization::SourceRow::json_limits(32768).unwrap();
            let mut delivered = false;
            assert!(
                crate::knowledge_payload_codec::with_factored_value_payload(
                    &state,
                    &mismatched,
                    &raw,
                    &source,
                    json_limits,
                    json_limits,
                    32768,
                    |_, _| {
                        delivered = true;
                        Ok(())
                    },
                )
                .is_err()
            );
            assert!(
                !delivered,
                "mismatched borrowed tree must not reach consumer"
            );
            crate::knowledge_payload_codec::with_factored_value_payload(
                &state,
                &logical,
                &raw,
                &source,
                json_limits,
                json_limits,
                32768,
                |stored, _| {
                    let physical: serde_json::Value = serde_json::from_slice(stored).unwrap();
                    assert!(physical["spine"]["attributes"]["text"].is_null());
                    assert!(physical["spine"]["source_record"]["payload"].is_null());
                    assert_eq!(logical["attributes"]["text"], source_value["text"]);
                    crate::knowledge_payload_codec::with_hydrated_value_payload(
                        &state,
                        stored,
                        &source,
                        json_limits,
                        json_limits,
                        32768,
                        raw.len(),
                        Digest256::of_bytes(&raw),
                        |value, source_digest| {
                            assert_eq!(source_digest, Digest256::of_bytes(&source));
                            assert_eq!(serde_json::to_vec(&value).unwrap(), raw);
                            assert_eq!(value["attributes"]["text"], source_value["text"]);
                            assert_eq!(value["source_record"]["payload"], source_value);
                            Ok(())
                        },
                    )?;
                    let mut delivered = false;
                    assert!(
                        crate::knowledge_payload_codec::with_hydrated_value_payload(
                            &state,
                            stored,
                            &source,
                            json_limits,
                            json_limits,
                            32768,
                            raw.len(),
                            Digest256::of_bytes(b"wrong logical digest"),
                            |_, _| {
                                delivered = true;
                                Ok(())
                            },
                        )
                        .is_err()
                    );
                    assert!(!delivered);
                    Ok(())
                },
            )
            .unwrap();
        }
        for layout in [
            KnowledgePayloadLayout::CarrierOnceV2,
            KnowledgePayloadLayout::CarrierOnceV3,
            KnowledgePayloadLayout::CarrierOnceV4,
        ] {
            let candidate = stage_path("dictionary-roundtrip");
            let mut receipt = exact_receipt(RAW_ROOT);
            receipt.binding.owner_profile = "tos-native-projection-snapshot-v1".into();
            let mut tiny_family = receipt.collections[0].clone();
            tiny_family.source_graph = "tiny".into();
            receipt.collections.push(tiny_family);
            let mut selected = limits();
            selected.sqlite.max_output_bytes = 8 * 1024 * 1024;
            selected.sqlite.max_row_bytes = 32768;
            selected.max_seek_bytes = 32768;
            selected.sqlite.sqlite_cache_kib = 1024;
            selected.sqlite.max_work_bytes = budget.original_work_limit;
            selected.sqlite.max_sql_vm_steps = budget.original_sql_vm_limit;
            let mut stage = KnowledgeStage::create_captured_native_snapshot_owned(
                &candidate,
                selected,
                receipt,
                &owner,
                &quota,
                Arc::clone(&vm),
                Arc::clone(&work),
                Arc::clone(&cancelled),
                budget.original_work_limit,
                deadline,
                &remaining,
                &heap,
                budget.original_sql_vm_limit,
                &state,
            )
            .unwrap();
            stage.enable_carrier_once_layout(layout).unwrap();
            stage
                .ingest_input(InputRow {
                    source_graph: "fixture.graph",
                    collection: "fixture/raw",
                    id: "raw.1",
                    payload: b"raw",
                })
                .unwrap();
            // The finalizer's exact raw witness is temporary. Repeated reads
            // must not reserve every previously dropped source row forever.
            let baseline = state.remaining(0).unwrap();
            for _ in 0..32 {
                let raw = stage
                    .scoped_raw_by_id("fixture.graph", "fixture/raw", "raw.1")
                    .unwrap();
                assert_eq!(raw.as_ref().unwrap().payload, b"raw");
                assert!(state.remaining(0).unwrap() < baseline);
                drop(raw);
                assert_eq!(state.remaining(0).unwrap(), baseline);
            }
            assert!(
                stage
                    .scoped_raw_by_id("fixture.graph", "fixture/raw", "absent")
                    .unwrap()
                    .as_ref()
                    .is_none()
            );
            assert_eq!(state.remaining(0).unwrap(), baseline);
            stage
                .with_raw_by_id_owned("fixture.graph", "fixture/raw", "raw.1", |_, raw| {
                    assert_eq!(raw.unwrap().payload, b"raw");
                    Ok(())
                })
                .unwrap();
            assert_eq!(state.remaining(0).unwrap(), baseline);
            stage
                .db()
                .execute(
                    "UPDATE raw_records SET payload_sha256=zeroblob(32) WHERE id='raw.1'",
                    [],
                )
                .unwrap();
            assert!(
                stage
                    .scoped_raw_by_id("fixture.graph", "fixture/raw", "raw.1")
                    .is_err()
            );
            assert_eq!(state.remaining(0).unwrap(), baseline);
            stage
                .db()
                .execute(
                    "UPDATE raw_records SET payload_sha256=?1 WHERE id='raw.1'",
                    [Digest256::of_bytes(b"raw").as_bytes().as_slice()],
                )
                .unwrap();
            for order in 0..2 {
                let id = format!("node.{order}");
                let logical = serde_json::to_vec(&serde_json::json!({
                    "id": id, "source_graph": "fixture.graph", "kind_id": "kind.fixture", "attributes": {}, "view_ids": [],
                    "display": {"title": {"text": id, "language": "und"}},
                    "metadata": "normalized words ".repeat(400),
                    "source_record": {"payload": source_value, "field_map": {}}
                }))
                .unwrap();
                let (seek, rows, bytes) = stage
                    .exact_source_write_page_limits(1, 32768, 32768)
                    .unwrap();
                assert_eq!(seek, 1);
                assert_eq!(
                    rows, 2,
                    "caller prices data rows; Stage owns format overhead"
                );
                stage
                    .with_write_page(WritePhase::Normalized, rows, bytes, |stage| {
                        stage.insert_node_with_exact_source(
                            NodeRow {
                                id: &id,
                                source_graph: "fixture.graph",
                                native_id: None,
                                entity_id: None,
                                kind_id: "kind.fixture",
                                type_id: "type.fixture",
                                source_order: order,
                                payload: &logical,
                            },
                            &source,
                        )
                    })
                    .unwrap();
                let before_update = work.load(Ordering::Acquire);
                stage.with_write_page(WritePhase::Finalize, 1, 32768, |stage| {
                    // Both entrypoints preserve exact bytes and the same CAS.
                    if order == 0 {
                        stage.replace_node_payload_with_exact_source_if_current(
                            &id, &logical, &source, Some(Digest256::of_bytes(&logical)),
                        )
                    } else {
                        stage.with_normalized_payload_decoded_owned(false, &id, 32768, true, |stage, decoded, source| {
                            let NormalizedLogical::Value { value, digest, source_receipt, .. } = decoded else {
                                return Err(Error::Invalid("typed update fixture requires Value"));
                            };
                            assert!(source_receipt.is_some());
                            let before_typed = work.load(Ordering::Acquire);
                            let result = stage.replace_finalized_value_if_current(
                                false, &id, &value, &logical, source, digest, source_receipt,
                            );
                            eprintln!("dictionary finalized typed update layout={layout:?} work={}", work.load(Ordering::Acquire)-before_typed);
                            result
                        })?.ok_or(Error::Invalid("typed update fixture row absent"))
                    }
                }).unwrap();
                eprintln!(
                    "dictionary finalized complete update layout={layout:?} order={order} work={}",
                    work.load(Ordering::Acquire) - before_update
                );
                stage
                    .with_node_payload_source_owned(&id, 32768, |_, actual, raw| {
                        assert_eq!(actual, logical);
                        assert_eq!(raw, Some(source.as_slice()));
                        Ok(())
                    })
                    .unwrap()
                    .unwrap();
            }
            // The actual SQL consumer interface checks the same packed frames,
            // dictionaries, exact source and normalized digest in both forms.
            // Typed semantic/catalog readers avoid another whole JSON parse.
            {
                let db = stage.db();
                let mut sql = db.prepare("SELECT payload_len,payload_sha256,payload,payload_codec,source_packet_sha256 FROM knowledge_nodes WHERE id='node.0'").unwrap();
                let mut rows = sql.query([]).unwrap();
                let row = rows.next().unwrap().unwrap();
                let len = row.get(0).unwrap();
                let digest = row.get_ref(1).unwrap().as_blob().unwrap();
                let stored = row.get_ref(2).unwrap().as_blob().unwrap();
                let codec = row.get(3).unwrap();
                let key = row.get_ref(4).unwrap().as_blob().unwrap();
                let limits = crate::knowledge_normalization::SourceRow::json_limits(32768).unwrap();
                let before = work.load(Ordering::Acquire);
                let expected = crate::knowledge_payload_codec::with_sql_logical_payload(
                    db,
                    &state,
                    layout,
                    len,
                    digest,
                    stored,
                    codec,
                    Some(key),
                    32768,
                    |raw| {
                        state.with_serde_owned_with_limits(raw, limits, |value| {
                            Ok(serde_json::to_vec(value).unwrap())
                        })
                    },
                )
                .unwrap();
                let byte_work = work.load(Ordering::Acquire) - before;
                let before = work.load(Ordering::Acquire);
                crate::knowledge_payload_codec::with_sql_logical_value(
                    db,
                    &state,
                    layout,
                    len,
                    digest,
                    stored,
                    codec,
                    Some(key),
                    32768,
                    |value| {
                        assert_eq!(serde_json::to_vec(value).unwrap(), expected);
                        Ok(())
                    },
                )
                .unwrap();
                let typed_work = work.load(Ordering::Acquire) - before;
                assert!(
                    typed_work < byte_work,
                    "typed={typed_work} byte={byte_work}"
                );
                eprintln!(
                    "SQL logical reader layout={layout:?} byte_work={byte_work} typed_work={typed_work}"
                );
                for (bad_len, bad_digest, bad_codec, bad_key) in [
                    (len + 1, digest, codec, Some(key)),
                    (len, &[0u8; 32][..], codec, Some(key)),
                    (len, digest, 2, Some(key)),
                    (len, digest, codec, None),
                    (len, digest, codec, Some(&[0u8; 32][..])),
                ] {
                    let mut delivered = false;
                    assert!(
                        crate::knowledge_payload_codec::with_sql_logical_value(
                            db,
                            &state,
                            layout,
                            bad_len,
                            bad_digest,
                            stored,
                            bad_codec,
                            bad_key,
                            32768,
                            |_| {
                                delivered = true;
                                Ok(())
                            },
                        )
                        .is_err()
                    );
                    assert!(!delivered);
                }
                // Joining valid constituents must still retain the logical
                // depth/visit limits formerly enforced by the redundant parse.
                let mut nested = serde_json::Value::Null;
                for _ in 0..97 {
                    nested = serde_json::json!([nested]);
                }
                assert!(state.check_serde_structure(&nested, limits).is_err());
                let tight = tos_foundation::JsonLimits::new(32768, 96, 2, 4096).unwrap();
                assert!(
                    state
                        .check_serde_structure(&serde_json::json!([0, 1]), tight)
                        .is_err()
                );
            }
            let count: u64 = stage
                .db()
                .query_row(
                    "SELECT count(*) FROM knowledge_source_carriers",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "identical exact source retains one carrier");
            if layout.dictionary_bytes() {
                let dictionaries: u64 = stage
                    .db()
                    .query_row(
                        "SELECT count(*) FROM knowledge_byte_dictionaries",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                if layout == KnowledgePayloadLayout::CarrierOnceV4 {
                    assert_eq!(
                        dictionaries, 0,
                        "V4 retains the short initial training group in TEMP"
                    );
                } else {
                    assert_eq!(dictionaries, 2);
                }
                let pending: u64 = stage
                    .db()
                    .query_row(
                        "SELECT count(*) FROM temp.knowledge_byte_dictionary_pending",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(pending, 2);
            }
            let title_roots = stage.core_roots().unwrap();
            let seal = crate::knowledge_global_titles::CompleteBaseNodes {
                source_cut: stage.exact_receipt().unwrap().binding.source_cut.clone(),
                node_count: title_roots.nodes,
                node_root_sha256: title_roots.node_sha256.clone(),
            };
            let before = work.load(Ordering::Acquire);
            let titles = crate::knowledge_global_titles::prepare_global_titles(
                &mut stage,
                &seal,
                crate::knowledge_global_titles::GlobalTitleLimits {
                    max_nodes: 2,
                    max_page_rows: 1,
                    max_page_bytes: 32768,
                    max_node_bytes: 32768,
                    max_title_bytes: 1024,
                    max_work_bytes: 1024 * 1024,
                },
            )
            .unwrap();
            assert_eq!(titles.title_count, 2);
            eprintln!(
                "global title typed scan layout={layout:?} work={}",
                work.load(Ordering::Acquire) - before
            );
            for order in 0..2 {
                let id = format!("node.{order}");
                let title =
                    crate::knowledge_global_titles::endpoint_title(&mut stage, &titles, &id, 1024)
                        .unwrap();
                assert_eq!(title, serde_json::json!({"text": id, "language": "und"}));
            }
            assert_eq!(
                stage.core_roots().unwrap().node_sha256,
                title_roots.node_sha256
            );
            // The actual global finalizer reads a factored, packed row, adds an
            // inherited view, stamps its revision and CAS-writes it. Compare the
            // independent scalar normalization and preserve original source bytes.
            stage
                .create_preparation_tables(crate::knowledge_inherited_views::PREPARATION_SCHEMA)
                .unwrap();
            stage.with_connection(WritePhase::Normalized, |db| {
                db.execute("INSERT INTO knowledge_global_inherited_views(endpoint_id,view_id) VALUES('node.0','view.inherited')", [])?;
                Ok(())
            }).unwrap();
            let entity = include_bytes!(
                "../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json"
            );
            let registry = crate::KnowledgeRegistry::parse(
                entity,
                include_bytes!(
                    "../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json"
                ),
            )
            .unwrap();
            let roots = stage.core_roots().unwrap();
            let inherited = crate::knowledge_inherited_views::InheritedViewReceipt {
                source_cut: stage.exact_receipt().unwrap().binding.source_cut.clone(),
                relation_count: roots.relations,
                relation_root_sha256: roots.relation_sha256,
                endpoint_evidence_rows: 0,
                inherited_view_rows: 1,
                dependency_root_sha256: "0".repeat(64),
                final_graph_rows_written: false,
            };
            let mut expected = None;
            stage
                .with_node_payload_source_owned("node.0", 32768, |_, raw, _| {
                    let mut value: serde_json::Value = serde_json::from_slice(raw).unwrap();
                    value["view_ids"] = serde_json::json!(["view.inherited"]);
                    crate::knowledge_normalization::stamp_content_revision(&mut value, 32768)?;
                    expected = Some(serde_json::to_vec(&value).unwrap());
                    Ok(())
                })
                .unwrap()
                .unwrap();
            let finalize_limits = crate::knowledge_native_finalize::NativeFinalizeLimits {
                max_rows: 2,
                max_page_rows: 1,
                max_page_bytes: 32768,
                max_row_bytes: 32768,
                max_view_ids_per_node: 8,
                max_context_sources: 8,
                max_work_bytes: 16 * 1024 * 1024,
            };
            let before_finalize = work.load(Ordering::Acquire);
            let finalized = crate::knowledge_native_finalize::finalize_native_graph_rows(
                &mut stage,
                &registry,
                entity,
                &inherited,
                finalize_limits,
                |_, _, _| Err(Error::Invalid("fixture has no claim references")),
            )
            .unwrap();
            eprintln!(
                "full finalizer measured work layout={layout:?} work={}",
                work.load(Ordering::Acquire) - before_finalize
            );
            assert_eq!(
                (
                    finalized.nodes,
                    finalized.relations,
                    finalized.readable_rows
                ),
                (2, 0, 0)
            );
            stage
                .with_node_payload_source_owned("node.0", 32768, |_, raw, exact_source| {
                    assert_eq!(raw, expected.as_ref().unwrap());
                    assert_eq!(exact_source, Some(source.as_slice()));
                    Ok(())
                })
                .unwrap()
                .unwrap();
            let replay = crate::knowledge_native_finalize::finalize_native_graph_rows(
                &mut stage,
                &registry,
                entity,
                &inherited,
                finalize_limits,
                |_, _, _| Err(Error::Invalid("fixture has no claim references")),
            )
            .unwrap();
            assert_eq!(replay.node_root_sha256, finalized.node_root_sha256);
            // Inheritance uses the same typed reader for an actual packed
            // relation, including source-derived attributes and both endpoints.
            crate::knowledge_inherited_views::clear_inherited_views(&mut stage).unwrap();
            let relation = serde_json::to_vec(&serde_json::json!({
                "id": "edge.0", "source_graph": "fixture.graph", "predicate_id": "related_to", "from_id": "node.0", "to_id": "node.1",
                "view_ids": ["view.inherited"], "attributes": {"text": source_value["text"]},
                "source_record": {"payload": source_value, "field_map": {"attributes.text": "/text"}}
            })).unwrap();
            let (_, rows, bytes) = stage
                .exact_source_write_page_limits(1, 32768, 32768)
                .unwrap();
            stage
                .with_write_page(WritePhase::Normalized, rows, bytes, |stage| {
                    stage.insert_relation_with_exact_source(
                        RelationRow {
                            id: "edge.0",
                            source_graph: "fixture.graph",
                            native_id: None,
                            from_id: "node.0",
                            to_id: "node.1",
                            predicate_id: "related_to",
                            relation_type_id: "tos.relation.unmapped",
                            source_order: 0,
                            payload: &relation,
                        },
                        &source,
                    )
                })
                .unwrap();
            // Run the actual late Claim join on packed carriers: a changed
            // context is CAS-written once; replay preserves its exact revision.
            let claim_limits = crate::knowledge_source_claims::ClaimNormalizeLimits {
                max_raw_bytes: 32768,
                max_output_bytes: 32768,
                max_page_rows: 1,
                max_contexts: 8,
                max_work_bytes: 1024 * 1024,
            };
            let mut contexts = crate::knowledge_source_claims::prepare_claim_context_groups(
                &mut stage,
                claim_limits,
            )
            .unwrap();
            let context = serde_json::json!({"binding_role":"referenced-claim", "fields":{"review_status":{"value":"source-recorded"}}});
            let context_bytes = serde_json::to_vec(&context).unwrap();
            let context_digest = Digest256::from_hex(
                &crate::knowledge_normalization::stable_digest(&context).unwrap(),
            )
            .unwrap();
            let source_digest = Digest256::of_bytes(&source);
            stage
                .with_connection(WritePhase::Normalized, |db| {
                    db.execute(
                        "INSERT INTO knowledge_claim_context_groups VALUES(?1,?2,0,?3,?4,?5,?6)",
                        params![
                            "fixture.graph",
                            "claim.test",
                            context_digest.as_bytes().as_slice(),
                            &context_bytes,
                            "node.0",
                            source_digest.as_bytes().as_slice()
                        ],
                    )?;
                    Ok(())
                })
                .unwrap();
            let mut root = Digest256Hasher::new();
            root.update(b"tos-claim-context-groups-v1\0");
            for (id, digest) in [
                ("fixture.graph", &[][..]),
                ("claim.test", context_digest.as_bytes().as_slice()),
                ("node.0", source_digest.as_bytes().as_slice()),
            ] {
                root.update(&(id.len() as u64).to_be_bytes());
                root.update(id.as_bytes());
                root.update(digest);
            }
            contexts.contexts = 1;
            contexts.root_sha256 = root.finalize().to_hex();
            crate::knowledge_source_claims::verify_claim_context_groups(
                &mut stage,
                &contexts,
                claim_limits,
            )
            .unwrap();
            let vocabulary = crate::QueryVocabulary::parse(
                include_bytes!("../tests/fixtures/query-vocabulary.v1.json"),
                &[
                    "indexed-node-edge-v1",
                    "candidate-relation-v1",
                    "canon-node-relation-v1",
                    "declared-identity-and-source-ref-joins-v1",
                    "philosophy-node-edge-v1",
                    "reified-bibliographic-claims-v1",
                    "repository-topology-v1",
                    "source-navigation-node-edge-v1",
                ],
            )
            .unwrap();
            let before = work.load(Ordering::Acquire);
            crate::knowledge_native::bind_native_claim_contexts(
                &mut stage,
                &vocabulary,
                &contexts,
                claim_limits,
                finalize_limits,
            )
            .unwrap();
            eprintln!(
                "Claim typed binding layout={layout:?} work={}",
                work.load(Ordering::Acquire) - before
            );
            let mut joined_bytes = Vec::new();
            stage
                .with_relation_payload_source_owned("edge.0", 32768, |_, raw, exact| {
                    let value: serde_json::Value = serde_json::from_slice(raw).unwrap();
                    assert_eq!(
                        value["semantics"]["assertion_contexts"],
                        serde_json::json!([context])
                    );
                    assert!(value["content_revision"].as_str().is_some());
                    assert_eq!(exact, Some(source.as_slice()));
                    joined_bytes = raw.to_vec();
                    Ok(())
                })
                .unwrap()
                .unwrap();
            crate::knowledge_native::bind_native_claim_contexts(
                &mut stage,
                &vocabulary,
                &contexts,
                claim_limits,
                finalize_limits,
            )
            .unwrap();
            stage
                .with_relation_payload_source_owned("edge.0", 32768, |_, raw, exact| {
                    assert_eq!(raw, joined_bytes);
                    assert_eq!(exact, Some(source.as_slice()));
                    Ok(())
                })
                .unwrap()
                .unwrap();
            let roots = stage.core_roots().unwrap();
            let seal = crate::knowledge_inherited_views::CompleteRelationSeal {
                source_cut: inherited.source_cut.clone(),
                relation_count: 1,
                relation_root_sha256: roots.relation_sha256,
            };
            let joined = crate::knowledge_inherited_views::prepare_global_inherited_views(
                &mut stage,
                &seal,
                crate::knowledge_inherited_views::InheritedViewLimits {
                    max_relations: 1,
                    max_endpoint_evidence_rows: 2,
                    max_view_tokens: 1,
                    max_page_rows: 1,
                    max_page_bytes: 32768,
                    max_row_bytes: 32768,
                    max_work_bytes: 16 * 1024 * 1024,
                },
            )
            .unwrap();
            assert_eq!(
                (
                    joined.relation_count,
                    joined.endpoint_evidence_rows,
                    joined.inherited_view_rows
                ),
                (1, 2, 2)
            );
            for endpoint in ["node.0", "node.1"] {
                assert_eq!(
                    crate::knowledge_inherited_views::endpoint_inherited_views(
                        &mut stage, endpoint, 8
                    )
                    .unwrap(),
                    vec!["view.inherited".to_owned()]
                );
            }
            crate::knowledge_inherited_views::clear_inherited_views(&mut stage).unwrap();
            // Exercise the other producer path with a byte-tight inline page.
            // Tiny packets collect over multiple pages and seal at sample 32;
            // both insert and replacement pay framing/dictionary overhead here.
            for order in 0..32 {
                let id = format!("tiny.{order}");
                let logical = serde_json::to_vec(
                    &serde_json::json!({"id": id, "source_graph": "tiny", "kind_id": "kind.fixture", "value": order}),
                )
                .unwrap();
                stage
                    .with_write_page(WritePhase::Normalized, 1, logical.len() as u64, |stage| {
                        stage.insert_node(NodeRow {
                            id: &id,
                            source_graph: "tiny",
                            native_id: None,
                            entity_id: None,
                            kind_id: "kind.fixture",
                            type_id: "type.fixture",
                            source_order: order + 2,
                            payload: &logical,
                        })
                    })
                    .unwrap();
                stage
                    .with_write_page(WritePhase::Finalize, 1, logical.len() as u64, |stage| {
                        stage.replace_node_logical_payload_if_current(
                            &id,
                            &logical,
                            None,
                            Some(Digest256::of_bytes(&logical)),
                        )
                    })
                    .unwrap();
                stage
                    .with_node_payload_source_owned(&id, 32768, |_, actual, raw| {
                        assert_eq!(actual, logical);
                        assert!(raw.is_none());
                        Ok(())
                    })
                    .unwrap()
                    .unwrap();
            }
            if layout.dictionary_bytes() {
                let samples: u64 = stage.db().query_row("SELECT samples FROM temp.knowledge_byte_dictionary_pending WHERE dictionary_kind='node' AND source_graph='tiny'", [], |row| row.get(0)).unwrap();
                assert_eq!(
                    samples,
                    if layout == KnowledgePayloadLayout::CarrierOnceV4 {
                        64
                    } else {
                        32
                    }
                );
                let dictionaries: u64 = stage
                    .db()
                    .query_row(
                        "SELECT count(*) FROM knowledge_byte_dictionaries",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(dictionaries > 0, "tiny family seals after 32 samples");
            }
            let search = crate::knowledge_search::build_search_index(
                &mut stage,
                crate::knowledge_search::SearchBuildLimits {
                    max_payload_bytes: 32768,
                    max_document_chars: 32768,
                    max_document_bytes: 131072,
                    max_rank_field_bytes: 32768,
                    max_postings: 100_000,
                    max_work_bytes: 64 * 1024 * 1024,
                    gram_batch_rows: 8,
                },
            )
            .unwrap();
            assert_eq!((search.node_documents, search.relation_documents), (34, 1));
            assert_eq!(search.search_index_root_sha256.len(), 64);
            stage.with_connection(WritePhase::Search, |db| {
                let mut statement=db.prepare("SELECT identity_values,visible_values FROM search_documents WHERE kind='nodes' AND position=0")?;
                let mut rows=statement.query([])?;let row=rows.next()?.unwrap();
                let abi=layout.carrier_model_abi().unwrap();
                let (a,b)=crate::knowledge_search_rank::decode_pair_owned(abi,row.get_ref(0)?,row.get_ref(1)?,32768,&state)?;
                assert_eq!(a, r#"["node.0","und"]"#); assert_eq!(b,a);
                assert_eq!(matches!(row.get_ref(0)?,rusqlite::types::ValueRef::Blob(_)),layout==KnowledgePayloadLayout::CarrierOnceV4);
                Ok(())
            }).unwrap();
            if layout.dictionary_bytes() {
                // A receipt cannot hide even a byte-preserving intervening
                // source write. Refuse before changing the logical row; its
                // original exact source remains readable after cold reopen.
                let refusal = stage.with_write_page(WritePhase::Finalize, 1, 32768, |stage| {
                    stage
                        .with_normalized_payload_decoded_owned(
                            false,
                            "node.0",
                            32768,
                            true,
                            |stage, decoded, source| {
                                let NormalizedLogical::Value {
                                    value,
                                    digest,
                                    source_receipt,
                                    ..
                                } = decoded
                                else {
                                    return Err(Error::Invalid("receipt control requires Value"));
                                };
                                stage.with_connection(WritePhase::Finalize, |db| {
                                    db.execute(
                                        "UPDATE knowledge_source_carriers SET packet=packet",
                                        [],
                                    )?;
                                    Ok(())
                                })?;
                                state.with_json_encoded(&value, 32768, |raw| {
                                    stage.replace_finalized_value_if_current(
                                        false,
                                        "node.0",
                                        &value,
                                        raw,
                                        source,
                                        digest,
                                        source_receipt,
                                    )
                                })
                            },
                        )?
                        .ok_or(Error::Invalid("receipt control row absent"))
                });
                assert!(matches!(
                    refusal,
                    Err(Error::Invalid("carrier read receipt changed before update"))
                ));
                let digest: Vec<u8> = stage
                    .db()
                    .query_row(
                        "SELECT payload_sha256 FROM knowledge_nodes WHERE id='node.0'",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(
                    digest,
                    Digest256::of_bytes(expected.as_ref().unwrap()).as_bytes()
                );
            } else {
                // Reserved representation rows cannot hide excess data writes.
                let refusal = stage.with_write_page(WritePhase::Normalized, 1, 1, |stage| {
                    stage.charge_materialized(2, 1)
                });
                assert!(matches!(
                    refusal,
                    Err(Error::Budget("stage write page rows/bytes"))
                ));
            }
            assert!(stage.poisoned);
            // Close the actual writer before opening a distinct read connection.
            // This tests physical cold decoding; full selected-model admission
            // remains the separate installed-consumer conformance route.
            stage.db.take().unwrap().close().unwrap();
            let cold =
                Connection::open_with_flags(&candidate, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .unwrap();
            let stored: Vec<u8> = cold
                .query_row("SELECT packet FROM knowledge_source_carriers", [], |row| {
                    row.get(0)
                })
                .unwrap();
            layout
                .with_sql_decoded(
                    &cold,
                    &state,
                    &stored,
                    Some(source.len()),
                    32768,
                    |actual| {
                        assert_eq!(actual, source);
                        Ok(())
                    },
                )
                .unwrap();
            let mut statement=cold.prepare("SELECT first_position,last_position,postings,deltas FROM search_posting_blocks WHERE kind='nodes' AND gram=?1 ORDER BY last_position").unwrap();
            let mut rows = statement.query([b"iny".as_slice()]).unwrap();
            let mut positions = Vec::new();
            while let Some(row) = rows.next().unwrap() {
                let first: i64 = row.get(0).unwrap();
                let last: i64 = row.get(1).unwrap();
                let count: i64 = row.get(2).unwrap();
                let bytes: Vec<u8> = row.get(3).unwrap();
                positions.extend(
                    crate::decode_posting_block_for_abi(
                        layout.carrier_model_abi().unwrap(),
                        first as u64,
                        last as u64,
                        count as u16,
                        &bytes,
                    )
                    .unwrap(),
                );
                if layout == KnowledgePayloadLayout::CarrierOnceV4 {
                    assert_eq!(bytes.first(), Some(&0));
                }
            }
            assert_eq!(positions, (2..34).collect::<Vec<u64>>());
            drop(rows);
            drop(statement);
            drop(cold);
            drop(stage);
            fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
        }
    }

    #[test]
    fn native_raw_input_pager_keeps_exact_bytes_under_temp_ceiling() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let candidate = stage_path("raw-input-geometry");
        let payload = format!(
            "{{ \"text\" : \"{}\", \"number\" : 1.2300 }}\n",
            "z".repeat(1500)
        )
        .into_bytes();
        let mut hash = Digest256Hasher::new();
        for n in 0..96 {
            let id = format!("fixture-{n:04}");
            hash.update(&(id.len() as u64).to_be_bytes());
            hash.update(id.as_bytes());
            hash.update(Digest256::of_bytes(&payload).as_bytes());
        }
        let mut receipt = exact_receipt(&hash.finalize().to_hex());
        receipt.binding.owner_profile = "tos-native-projection-snapshot-v1".into();
        receipt.collections[0].expected_count = 96;
        let mut selected = limits();
        selected.sqlite.max_output_bytes = 1024 * 1024;
        selected.max_temp_bytes = 256 * 1024;
        selected.max_seek_bytes = 4096;
        let mut stage = KnowledgeStage::create_captured_native_snapshot(
            &candidate,
            selected,
            receipt,
            &owner,
            &quota,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(false)),
            selected.sqlite.max_work_bytes,
            Instant::now() + std::time::Duration::from_secs(60),
        )
        .unwrap();
        for n in (0..96).rev() {
            stage
                .ingest_input(InputRow {
                    source_graph: "fixture.graph",
                    collection: "fixture/raw",
                    id: &format!("fixture-{n:04}"),
                    payload: &payload,
                })
                .unwrap();
        }
        let mut after = None;
        for n in 0..96 {
            let rows = stage
                .scan_input("fixture.graph", "fixture/raw", after.as_deref(), 1)
                .unwrap();
            assert_eq!(rows.rows.len(), 1);
            assert_eq!(rows.rows[0].id, format!("fixture-{n:04}"));
            assert_eq!(rows.rows[0].payload, payload);
            assert_eq!(
                rows.rows[0].payload_sha256,
                Digest256::of_bytes(&payload).to_hex()
            );
            after = Some(rows.rows[0].id.clone());
        }
        assert!(
            stage
                .scan_input("fixture.graph", "fixture/raw", after.as_deref(), 1)
                .unwrap()
                .rows
                .is_empty()
        );
        assert_eq!(stage.verified_input_rows().unwrap(), 96);
        let size: u64 = stage
            .db()
            .query_row("PRAGMA temp.page_size", [], |r| r.get(0))
            .unwrap();
        let pages: u64 = stage
            .db()
            .query_row("PRAGMA temp.page_count", [], |r| r.get(0))
            .unwrap();
        assert_eq!(size, 16384);
        assert!(size * pages <= selected.max_temp_bytes);
        // The same source bytes and composite key exceed this ceiling in the
        // previous 4-KiB index-btree layout; no payload codec is involved.
        let old = Connection::open_in_memory().unwrap();
        old.execute_batch("PRAGMA page_size=4096; CREATE TABLE raw_records(source_graph TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL,PRIMARY KEY(source_graph,collection,id)) WITHOUT ROWID").unwrap();
        for n in (0..96).rev() {
            old.execute(
                "INSERT INTO raw_records VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    "fixture.graph",
                    "fixture/raw",
                    format!("fixture-{n:04}"),
                    payload.len(),
                    Digest256::of_bytes(&payload).as_bytes().as_slice(),
                    &payload
                ],
            )
            .unwrap();
        }
        let old_pages: u64 = old
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .unwrap();
        assert!(old_pages * 4096 > selected.max_temp_bytes);
        drop(old);
        stage.close_inputs_for_full_components().unwrap();
        assert_eq!(stage.closed_input_rows, Some(96));
        drop(stage);
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn native_preparation_keeps_main_byte_cap_and_refuses_temp_overflow() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        // Native provenance alone must not select a physical storage profile.
        for (native, native_provenance) in [(false, false), (false, true), (true, true)] {
            let candidate = stage_path("preparation-geometry");
            let mut receipt = exact_receipt(RAW_ROOT);
            if native_provenance {
                receipt.binding.owner_profile = "tos-native-projection-snapshot-v1".into();
            }
            let mut selected = limits();
            selected.sqlite.max_output_bytes = 1024 * 1024;
            selected.max_temp_bytes = 256 * 1024;
            let mut stage = if native {
                KnowledgeStage::create_captured_native_snapshot(
                    &candidate,
                    selected,
                    receipt,
                    &owner,
                    &quota,
                    Arc::new(AtomicU64::new(0)),
                    Arc::new(AtomicU64::new(0)),
                    Arc::new(AtomicBool::new(false)),
                    selected.sqlite.max_work_bytes,
                    Instant::now() + std::time::Duration::from_secs(60),
                )
            } else {
                KnowledgeStage::create(&candidate, selected, receipt, &owner, &quota)
            }
            .unwrap();
            stage
                .create_preparation_tables(crate::knowledge_stage::preparation_schema!(
                    table r#"preparation_fixture(id INTEGER PRIMARY KEY,payload BLOB NOT NULL)"#,
                    index r#"preparation_fixture_payload ON preparation_fixture(payload)"#
                ))
                .unwrap();
            let page_size: u64 = stage
                .db()
                .query_row("PRAGMA main.page_size", [], |r| r.get(0))
                .unwrap();
            let pages: u64 = stage
                .db()
                .query_row("PRAGMA main.max_page_count", [], |r| r.get(0))
                .unwrap();
            assert_eq!(page_size, if native { 16384 } else { 4096 });
            assert_eq!(page_size * pages, selected.sqlite.max_output_bytes);
            for (schema, expected) in [("main", !native), ("temp", native)] {
                let count: u64 = stage.db().query_row(
                    &format!("SELECT count(*) FROM {schema}.sqlite_schema WHERE name IN ('preparation_fixture','preparation_fixture_payload','raw_records')"),
                    [], |r| r.get(0),
                ).unwrap();
                assert_eq!(count, if expected { 3 } else { 0 });
            }
            // Exercise the real late-phase schemas: these formerly bypassed
            // the profile and filled main despite being erased before selection.
            for (schema, tables, indices) in [
                (
                    crate::knowledge_inherited_views::PREPARATION_SCHEMA,
                    &[
                        "knowledge_global_inherited_endpoint_evidence",
                        "knowledge_global_inherited_views",
                    ][..],
                    &["knowledge_global_inherited_relation"][..],
                ),
                (
                    crate::knowledge_source_navigation_relation::PREPARATION_SCHEMA,
                    &["knowledge_navigation_relation_dependencies"][..],
                    &["knowledge_navigation_relation_claim_seek"][..],
                ),
                (
                    crate::knowledge_corpus_source::PREPARATION_SCHEMA,
                    &["corpus_capture_pending"][..],
                    &[][..],
                ),
                (
                    crate::knowledge_ordered::PREPARATION_SCHEMA,
                    &["knowledge_node_candidates", "knowledge_relation_candidates"][..],
                    &[
                        "knowledge_node_candidates_order",
                        "knowledge_relation_candidates_order",
                    ][..],
                ),
            ] {
                stage.create_preparation_tables(schema).unwrap();
                for (location, present) in [("main", !native), ("temp", native)] {
                    for name in tables.iter().chain(indices) {
                        let count: u64 = stage
                            .db()
                            .query_row(
                                &format!(
                                    "SELECT count(*) FROM {location}.sqlite_schema WHERE name=?1"
                                ),
                                [name],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert_eq!(count, u64::from(present), "{location}.{name}");
                    }
                }
                for table in tables {
                    stage
                        .db()
                        .execute_batch(&format!("DROP TABLE {table}"))
                        .unwrap();
                }
            }
            if native {
                let error = stage
                    .with_connection(WritePhase::Normalized, |db| {
                        db.execute(
                            "INSERT INTO preparation_fixture VALUES(1,zeroblob(1048576))",
                            [],
                        )?;
                        Ok(())
                    })
                    .unwrap_err();
                assert!(matches!(error, Error::SqlitePhase {
                    error: rusqlite::Error::SqliteFailure(code, _), ..
                } if code.code == rusqlite::ErrorCode::DiskFull));
                assert!(stage.poisoned);
            }
            drop(stage);
            assert!(!candidate.exists());
            fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
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
