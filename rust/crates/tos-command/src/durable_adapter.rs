//! Private real-byte PostgreSQL/STO laboratory path. No ToS source admission.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[path = "cold_membership_spool.rs"]
mod cold_membership_spool;
pub use cold_membership_spool::{ColdWorkspaceLimits, PrivateGenerationWorkspace};
use cold_membership_spool::{RunCollector, ScratchReader, ScratchRows};

use postgres::fallible_iterator::FallibleIterator;
use postgres::{Client, IsolationLevel, NoTls, Transaction};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_segment_store::{
    AttemptRecovery, AuditedStoreRoot, ByteDurabilityReceipt, GenerationCatalogV1,
    GenerationCoverageV1, GenerationCutV1, GenerationDescriptorV1, GenerationNamespaceV1,
    GenerationReadLimits, GenerationRowStreamV1, GenerationShapeLimits, InstalledGenerationV1,
    KeyComparatorV1, PackedPartitionRefV1, PackedPlacementLeafV1, PartitionBoundsV1,
    PlacementGenerationRowV1, SegmentError, SegmentStore, VerificationBudget,
    describe_placement_partition,
};

const PROFILE_ID: &[u8] = b"cmd2.lab.embedded-revision";
const PROFILE_VERSION: &[u8] = b"1";
const LAB_MAGIC: &[u8; 8] = b"CMD2LAB1";
const COLD_AUDIT_PROFILE: &[u8] = b"cmd2-private-complete-state-v3:pg16-row-to-json:fenced-intent";
const RECEIPT_PROFILE: &[u8] = b"cmd2-receipt-v2:attempt-fence";
const HISTORY_KEY_TAG: &[u8] = b"cmd2-history-key-v1";
const CURRENT_KEY_TAG: &[u8] = b"cmd2-current-key-v1";
const MAX_MEMBERS: usize = 64;
const MAX_CUT: u64 = 100_000;
const MAX_MEMBERSHIP_KEY_BYTES: usize = 4096;
const MAX_TOTAL_MEMBERSHIP_KEY_BYTES: usize = 16 * 1024 * 1024;
const HISTORY_NAMESPACE: &[u8] = b"cmd2.history.v1";
const CURRENT_NAMESPACE: &[u8] = b"cmd2.current.v1";
const HISTORY_KEY_CODEC: &[u8] =
    b"cmd2-history-key-v1:tag,u32be-domain-len,domain,u32be-subject-len,subject,u64be-revision";
const CURRENT_KEY_CODEC: &[u8] =
    b"cmd2-current-key-v1:tag,u32be-domain-len,domain,u32be-subject-len,subject";
const MAX_COLD_AUDIT_ELAPSED: Duration = Duration::from_secs(300);

/// An explicit offline profile for the opt-in scratch-backed complete cut.
/// Limits are resource ceilings, never proof that a larger corpus fits them.
#[derive(Clone, Copy, Debug)]
pub struct StreamedGenerationProfile {
    pub max_commit_seq: u64,
    pub max_members: u64,
    pub max_pins: usize,
    pub max_segment_bytes: u64,
    pub max_membership_key_bytes: u64,
    pub max_metadata_rows: usize,
    pub max_metadata_bytes: usize,
    pub max_elapsed: Duration,
    pub max_pg_temp_bytes: u64,
    pub max_sql_statement_ms: u64,
    pub generation: GenerationReadLimits,
    pub rows_per_leaf: usize,
}

impl StreamedGenerationProfile {
    pub(crate) fn validate(self, workspace: &PrivateGenerationWorkspace) -> DurableResult<Self> {
        if self.max_commit_seq == 0
            || self.max_commit_seq > i64::MAX as u64
            || self.max_members == 0
            || self.max_members > workspace.limits().max_rows
            || self.max_pins == 0
            || self.max_segment_bytes == 0
            || self.max_segment_bytes == u64::MAX
            || self.max_membership_key_bytes == 0
            || self.max_membership_key_bytes == u64::MAX
            || self.max_metadata_rows == 0
            || self.max_metadata_bytes == 0
            || self.max_elapsed.is_zero()
            || self.max_pg_temp_bytes < 1024
            || self.max_pg_temp_bytes > i64::MAX as u64
            || self.max_sql_statement_ms == 0
            || self.max_sql_statement_ms > i32::MAX as u64
            || self.rows_per_leaf == 0
            || self.rows_per_leaf as u64 > self.generation.shape.max_rows_per_partition
            || self.generation.max_descriptor_bytes < 256
            || self.generation.max_descriptor_bytes == usize::MAX
            || self.generation.max_stream_rows == 0
            || self.generation.max_stream_rows == u64::MAX
            || self.generation.max_stream_key_bytes == 0
            || self.generation.max_stream_key_bytes == u64::MAX
            || self.generation.shape.max_partitions == 0
            || self.generation.shape.max_partitions > u32::MAX as usize
            || self.generation.shape.max_rows_per_partition == u64::MAX
            || self.generation.shape.max_key_bytes == 0
            || self.generation.shape.max_key_bytes > u32::MAX as usize
            || self.generation.shape.max_leaf_bytes == 0
            || self.generation.shape.max_leaf_bytes == u64::MAX
            || self.generation.shape.max_key_bytes > workspace.limits().max_key_bytes
            || self.max_members > self.generation.max_stream_rows
            || self.max_membership_key_bytes > self.generation.max_stream_key_bytes
            || (self.rows_per_leaf as u128) * (self.generation.shape.max_key_bytes as u128 + 400)
                > self.generation.shape.max_leaf_bytes as u128
        {
            return Err(DurableError::Refused("invalid streamed generation profile"));
        }
        Ok(self)
    }
}

#[path = "source_cohort.rs"]
pub mod source_cohort;

fn check_cold_deadline(
    started: Instant,
    requested: Option<(Instant, &AtomicBool)>,
) -> DurableResult<()> {
    if started.elapsed() > MAX_COLD_AUDIT_ELAPSED
        || requested.is_some_and(|(deadline, cancelled)| {
            cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline
        })
    {
        Err(DurableError::Refused("cold audit deadline exceeded"))
    } else {
        Ok(())
    }
}

fn check_cold_profile_deadline(
    started: Instant,
    requested: Option<(Instant, &AtomicBool)>,
    profile: Option<StreamedGenerationProfile>,
) -> DurableResult<()> {
    if let Some(profile) = profile {
        if started.elapsed() > profile.max_elapsed
            || requested.is_some_and(|(deadline, cancelled)| {
                cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline
            })
        {
            return Err(DurableError::Refused("cold audit deadline exceeded"));
        }
        Ok(())
    } else {
        check_cold_deadline(started, requested)
    }
}

/// The framed keys are sorted as raw unsigned bytes by STO. The exact
/// PostgreSQL traversal therefore orders by UTF-8 byte length, UTF-8 bytes,
/// then revision, rather than by the database's text collation.
fn membership_key(
    tag: &[u8],
    domain: &str,
    subject: &str,
    revision: Option<u64>,
) -> DurableResult<Vec<u8>> {
    if domain.is_empty() || subject.is_empty() || revision == Some(0) {
        return Err(DurableError::Invalid("membership key identity is empty"));
    }
    if tag.len() + 8 + domain.len() + subject.len() + usize::from(revision.is_some()) * 8
        > MAX_MEMBERSHIP_KEY_BYTES
    {
        return Err(DurableError::Refused("membership key exceeds byte budget"));
    }
    let domain_len = u32::try_from(domain.len())
        .map_err(|_| DurableError::Refused("membership domain key too long"))?;
    let subject_len = u32::try_from(subject.len())
        .map_err(|_| DurableError::Refused("membership subject key too long"))?;
    let mut key = Vec::with_capacity(
        tag.len() + 8 + domain.len() + subject.len() + usize::from(revision.is_some()) * 8,
    );
    key.extend_from_slice(tag);
    key.extend_from_slice(&domain_len.to_be_bytes());
    key.extend_from_slice(domain.as_bytes());
    key.extend_from_slice(&subject_len.to_be_bytes());
    key.extend_from_slice(subject.as_bytes());
    if let Some(revision) = revision {
        key.extend_from_slice(&revision.to_be_bytes());
    }
    Ok(key)
}

fn sort_complete_membership(rows: &mut [PlacementGenerationRowV1]) -> DurableResult<()> {
    rows.sort_unstable_by(|a, b| a.key.cmp(&b.key));
    if rows.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(DurableError::Corrupt("duplicate membership key"));
    }
    Ok(())
}

fn logical_membership_root(tag: &[u8], rows: &[PlacementGenerationRowV1]) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-logical-membership-root-v1");
    part(&mut hasher, tag);
    part(&mut hasher, &(rows.len() as u64).to_be_bytes());
    for row in rows {
        part(&mut hasher, &row.key);
        part(&mut hasher, row.logical_digest.as_bytes());
        part(&mut hasher, &row.logical_length.to_be_bytes());
    }
    hasher.finalize()
}

fn membership_root_start(tag: &[u8], count: u64) -> Digest256Hasher {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-logical-membership-root-v1");
    part(&mut hasher, tag);
    part(&mut hasher, &count.to_be_bytes());
    hasher
}

fn membership_root_row(hasher: &mut Digest256Hasher, row: &PlacementGenerationRowV1) {
    part(hasher, &row.key);
    part(hasher, row.logical_digest.as_bytes());
    part(hasher, &row.logical_length.to_be_bytes());
}

fn current_key_from_history(key: &[u8], domain: &str) -> DurableResult<Vec<u8>> {
    let prefix = HISTORY_KEY_TAG.len();
    if !key.starts_with(HISTORY_KEY_TAG) || key.len() < prefix + 4 + domain.len() + 4 + 1 + 8 {
        return Err(DurableError::Corrupt("history membership key malformed"));
    }
    let domain_len =
        u32::from_be_bytes(key[prefix..prefix + 4].try_into().expect("fixed")) as usize;
    let subject_len_at = prefix + 4 + domain_len;
    if domain_len != domain.len()
        || key.get(prefix + 4..subject_len_at) != Some(domain.as_bytes())
        || key.len() < subject_len_at + 4 + 1 + 8
    {
        return Err(DurableError::Corrupt("history membership domain differs"));
    }
    let subject_len = u32::from_be_bytes(
        key[subject_len_at..subject_len_at + 4]
            .try_into()
            .expect("fixed"),
    ) as usize;
    if subject_len == 0
        || key.len() != subject_len_at + 4 + subject_len + 8
        || key[key.len() - 8..] == [0; 8]
    {
        return Err(DurableError::Corrupt(
            "history membership subject/revision differs",
        ));
    }
    let mut current = Vec::with_capacity(CURRENT_KEY_TAG.len() + key.len() - prefix - 8);
    current.extend_from_slice(CURRENT_KEY_TAG);
    current.extend_from_slice(&key[prefix..key.len() - 8]);
    Ok(current)
}

/// Exact sorted spool comparison under the same audited snapshot. In addition
/// to both EOF roots, every subject's final historical placement must be the
/// one emitted by current; the SQL audit separately checks all metadata fields.
fn streamed_membership_roots(
    domain: &str,
    history: &ScratchRows,
    workspace: &PrivateGenerationWorkspace,
    expected_current_count: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(ScratchRows, Digest256, Digest256)> {
    let mut history_reader = history.reader();
    let mut history_root = membership_root_start(HISTORY_KEY_TAG, history.count());
    let mut current_root = membership_root_start(CURRENT_KEY_TAG, expected_current_count);
    let mut current_rows = workspace.ordered_rows()?;
    let mut previous_key = Vec::new();
    let mut pending: Option<PlacementGenerationRowV1> = None;
    let mut subjects = 0u64;
    while let Some(row) = history_reader.next()? {
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(DurableError::Refused("cold workspace deadline exceeded"));
        }
        if !previous_key.is_empty() && row.key <= previous_key {
            return Err(DurableError::Corrupt("history workspace order differs"));
        }
        previous_key.clone_from(&row.key);
        membership_root_row(&mut history_root, &row);
        let expected_current_key = current_key_from_history(&row.key, domain)?;
        if pending
            .as_ref()
            .is_some_and(|previous| previous.key != expected_current_key)
        {
            let expected = pending.take().expect("checked pending");
            membership_root_row(&mut current_root, &expected);
            current_rows.push(expected)?;
            subjects += 1;
        }
        pending = Some(PlacementGenerationRowV1 {
            key: expected_current_key,
            logical_digest: row.logical_digest,
            logical_length: row.logical_length,
            placement: row.placement,
        });
    }
    history_reader.finish()?;
    if let Some(expected) = pending {
        membership_root_row(&mut current_root, &expected);
        current_rows.push(expected)?;
        subjects += 1;
    }
    if subjects != expected_current_count {
        return Err(DurableError::Corrupt(
            "current membership subject count differs",
        ));
    }
    let current = current_rows.finish()?;
    if current.count() != subjects {
        return Err(DurableError::Corrupt("current workspace count differs"));
    }
    Ok((current, history_root.finalize(), current_root.finalize()))
}

#[cfg(test)]
mod membership_key_tests {
    use super::{CURRENT_KEY_TAG, HISTORY_KEY_TAG, membership_key};

    #[test]
    fn framed_history_and_current_keys_are_injective_and_byte_ordered() {
        let current_a = membership_key(CURRENT_KEY_TAG, "d", "a", None).unwrap();
        let current_aa = membership_key(CURRENT_KEY_TAG, "d", "aa", None).unwrap();
        let other_domain = membership_key(CURRENT_KEY_TAG, "dd", "a", None).unwrap();
        let history_a1 = membership_key(HISTORY_KEY_TAG, "d", "a", Some(1)).unwrap();
        let history_a2 = membership_key(HISTORY_KEY_TAG, "d", "a", Some(2)).unwrap();
        let history_aa1 = membership_key(HISTORY_KEY_TAG, "d", "aa", Some(1)).unwrap();
        assert!(current_a < current_aa);
        assert_ne!(current_a, other_domain);
        assert!(history_a1 < history_a2);
        assert!(history_a2 < history_aa1);
        assert_ne!(history_a1, current_a);
        assert!(membership_key(HISTORY_KEY_TAG, "d", "a", Some(0)).is_err());
    }
}

#[derive(Debug)]
pub enum DurableError {
    Database(postgres::Error),
    Storage(SegmentError),
    Source(crate::source_command::SourceCommandError),
    Invalid(&'static str),
    Conflict(&'static str),
    Refused(&'static str),
    Corrupt(&'static str),
    Indeterminate(&'static str),
}

impl fmt::Display for DurableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database: {error}"),
            Self::Storage(error) => write!(f, "storage: {error}"),
            Self::Source(error) => write!(f, "source owner: {error:?}"),
            Self::Invalid(reason) => write!(f, "invalid input: {reason}"),
            Self::Conflict(reason) => write!(f, "conflict: {reason}"),
            Self::Refused(reason) => write!(f, "refused: {reason}"),
            Self::Corrupt(reason) => write!(f, "corrupt metadata: {reason}"),
            Self::Indeterminate(reason) => write!(f, "indeterminate: {reason}"),
        }
    }
}

impl std::error::Error for DurableError {}

impl From<postgres::Error> for DurableError {
    fn from(error: postgres::Error) -> Self {
        Self::Database(error)
    }
}

impl From<SegmentError> for DurableError {
    fn from(error: SegmentError) -> Self {
        Self::Storage(error)
    }
}

pub type DurableResult<T> = std::result::Result<T, DurableError>;

#[derive(Clone, Debug)]
pub struct DurableShadowMember {
    pub member_slot: u32,
    pub subject: String,
    pub expected_predecessor: Option<(u64, Digest256)>,
    pub proposed_revision: u64,
    pub exact_bytes: Vec<u8>,
    pub receipt: ByteDurabilityReceipt,
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowWriteIdentity<'a> {
    pub member_slot: u32,
    pub subject: &'a str,
    pub expected_predecessor: Option<(u64, Digest256)>,
    pub proposed_revision: u64,
    pub exact_bytes: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct RegisterShadowAttempt<'a> {
    pub domain: &'a str,
    pub prepare_id: &'a [u8],
    pub command_id: &'a str,
    pub raw_request_digest: Digest256,
    pub delta_digest: Digest256,
}

#[derive(Clone, Debug)]
pub struct CommitShadowAttempt<'a> {
    pub domain: &'a str,
    pub prepare_id: &'a [u8],
    pub attempt_fence: u64,
    pub receipts: &'a [ByteDurabilityReceipt],
    pub expected_contract_digest: Digest256,
    pub expected_rule_version: u64,
    pub expected_rights_version: u64,
    pub job_id: &'a str,
    pub job_fence: u64,
    pub full_base_seq: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableCommitReceipt {
    pub domain: String,
    pub prepare_id: Vec<u8>,
    pub command_id: String,
    pub commit_seq: u64,
    pub raw_request_digest: Digest256,
    pub delta_digest: Digest256,
    pub member_root: Digest256,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttemptResolution {
    Registered,
    Ready,
    Aborted,
    Committed(DurableCommitReceipt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CancelOutcome {
    Cancelled { pin_fenced: bool },
    AlreadyAborted,
    AlreadyCommitted(DurableCommitReceipt),
}

#[derive(Clone, Debug)]
pub struct DurableTiming {
    pub verification: Duration,
    pub lock_wait: Duration,
    pub lock_held: Duration,
    pub transaction: Duration,
}

/// Opaque cold-verified placement. It does not grant disclosure: each warm
/// selected read checks current rights under the coordinator's read lock.
#[derive(Clone, Debug)]
pub struct ColdRecoveredMember {
    domain: String,
    subject: String,
    revision: u64,
    receipt: ByteDurabilityReceipt,
}

#[derive(Clone, Debug)]
pub struct ColdCut {
    // Private anchored STO capability, retained through candidate selection.
    // Numeric store_id and domain are only descriptive and cannot substitute
    // for this exact opened root and its held pin custody.
    audited_root: AuditedStoreRoot,
    domain: String,
    through_commit_seq: u64,
    log_digest: Digest256,
    state_digest: Digest256,
    schema_profile_digest: Digest256,
    database_oid: u64,
    audit_generation: u64,
    historical_members: u64,
    current_members: u64,
    history_membership_root: Digest256,
    current_membership_root: Digest256,
    history_rows: Vec<PlacementGenerationRowV1>,
    current_rows: Vec<PlacementGenerationRowV1>,
    history_spool: Option<ScratchRows>,
    current_spool: Option<ScratchRows>,
    streamed_profile: Option<StreamedGenerationProfile>,
}

// Two real proofs share installation facts, never proof construction.
#[derive(Clone, Debug)]
pub(crate) struct WarmSuccessorCut {
    audited_root: AuditedStoreRoot,
    domain: String,
    descriptor_cut: GenerationCutV1,
    history_rows: Vec<PlacementGenerationRowV1>,
    current_rows: Vec<PlacementGenerationRowV1>,
}
#[derive(Clone, Debug)]
pub(crate) struct VerifiedWarmGeneration {
    cut: WarmSuccessorCut,
    installed: InstalledGenerationV1,
    selected_audit_generation: u64,
}
#[derive(Clone, Debug)]
pub(crate) enum SelectedSourceGeneration {
    Cold(VerifiedSelectedGeneration),
    Warm(VerifiedWarmGeneration),
}
impl SelectedSourceGeneration {
    pub(crate) fn digest(&self) -> Digest256 {
        match self {
            Self::Cold(value) => value.digest(),
            Self::Warm(value) => value.installed.digest(),
        }
    }
    pub(crate) fn through_seq(&self) -> u64 {
        self.view().descriptor_cut.through_seq
    }
    pub(crate) fn audit_generation(&self) -> u64 {
        match self {
            Self::Cold(value) => value.cut.audit_generation,
            Self::Warm(value) => value.selected_audit_generation,
        }
    }
    fn installed(&self) -> &InstalledGenerationV1 {
        match self {
            Self::Cold(value) => &value.installed,
            Self::Warm(value) => &value.installed,
        }
    }
    fn read_limits(&self) -> GenerationReadLimits {
        match self {
            Self::Cold(value) => value
                .cut
                .streamed_profile
                .map_or_else(generation_limits, |profile| profile.generation),
            Self::Warm(_) => generation_limits(),
        }
    }
    pub(crate) fn current_count(&self) -> u64 {
        self.installed().descriptor().cut.current_members
    }
    pub(crate) fn lookup_current(
        &self,
        domain: &str,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<PlacementGenerationRowV1>> {
        let key = membership_key(CURRENT_KEY_TAG, domain, path.as_str(), None)?;
        Ok(self.installed().lookup(
            GenerationNamespaceV1::Current,
            &key,
            self.read_limits(),
            deadline,
            cancelled,
        )?)
    }
    fn view(&self) -> MembershipInstallation<'_> {
        match self {
            Self::Cold(value) => MembershipInstallation {
                audited_root: &value.cut.audited_root,
                domain: &value.cut.domain,
                descriptor_cut: value.installed.descriptor().cut.clone(),
                history_rows: &value.cut.history_rows,
                current_rows: &value.cut.current_rows,
                history_spool: value.cut.history_spool.as_ref(),
                current_spool: value.cut.current_spool.as_ref(),
                profile: value.cut.streamed_profile,
            },
            Self::Warm(value) => value.cut.installation(),
        }
    }
}
#[derive(Clone)]
struct MembershipInstallation<'a> {
    audited_root: &'a AuditedStoreRoot,
    domain: &'a str,
    descriptor_cut: GenerationCutV1,
    history_rows: &'a [PlacementGenerationRowV1],
    current_rows: &'a [PlacementGenerationRowV1],
    history_spool: Option<&'a ScratchRows>,
    current_spool: Option<&'a ScratchRows>,
    profile: Option<StreamedGenerationProfile>,
}
enum MembershipCursor<'a> {
    Memory(std::slice::Iter<'a, PlacementGenerationRowV1>),
    Scratch(ScratchReader),
}
impl MembershipCursor<'_> {
    fn next_row(&mut self) -> DurableResult<Option<PlacementGenerationRowV1>> {
        match self {
            Self::Memory(rows) => Ok(rows.next().cloned()),
            Self::Scratch(rows) => rows.next(),
        }
    }
    fn finish(&mut self) -> DurableResult<()> {
        match self {
            Self::Memory(rows) if rows.len() != 0 => {
                Err(DurableError::Corrupt("membership iterator lacks EOF"))
            }
            Self::Memory(_) => Ok(()),
            Self::Scratch(rows) => rows.finish(),
        }
    }
}
impl<'a> MembershipInstallation<'a> {
    fn cursor(&self, namespace: GenerationNamespaceV1) -> DurableResult<MembershipCursor<'a>> {
        let (memory, scratch) = match namespace {
            GenerationNamespaceV1::History => (self.history_rows, self.history_spool),
            GenerationNamespaceV1::Current => (self.current_rows, self.current_spool),
        };
        match (scratch, self.profile) {
            (Some(rows), Some(_)) if memory.is_empty() => {
                Ok(MembershipCursor::Scratch(rows.reader()))
            }
            (None, None) => Ok(MembershipCursor::Memory(memory.iter())),
            _ => Err(DurableError::Corrupt("mixed membership carrier")),
        }
    }
    fn count(&self, namespace: GenerationNamespaceV1) -> u64 {
        match namespace {
            GenerationNamespaceV1::History => self
                .history_spool
                .map_or(self.history_rows.len() as u64, ScratchRows::count),
            GenerationNamespaceV1::Current => self
                .current_spool
                .map_or(self.current_rows.len() as u64, ScratchRows::count),
        }
    }
}

fn root_from_cursor(
    tag: &[u8],
    count: u64,
    mut cursor: MembershipCursor<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Digest256> {
    let mut root = membership_root_start(tag, count);
    let mut previous = Vec::new();
    let mut observed = 0u64;
    while let Some(row) = cursor.next_row()? {
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(DurableError::Refused(
                "membership verification deadline exceeded",
            ));
        }
        if !previous.is_empty() && row.key <= previous {
            return Err(DurableError::Corrupt("membership row order differs"));
        }
        previous = row.key.clone();
        membership_root_row(&mut root, &row);
        observed += 1;
        if observed > count {
            return Err(DurableError::Corrupt("membership row count exceeded"));
        }
    }
    cursor.finish()?;
    if observed != count {
        return Err(DurableError::Corrupt("membership EOF count differs"));
    }
    Ok(root.finalize())
}
impl ColdCut {
    fn installation(&self, store: &SegmentStore) -> MembershipInstallation<'_> {
        MembershipInstallation {
            audited_root: &self.audited_root,
            domain: &self.domain,
            descriptor_cut: GenerationCutV1 {
                store_id: store.store_id(),
                domain_digest: store.domain_digest(),
                through_seq: self.through_commit_seq,
                audit_generation: self.audit_generation,
                database_oid: self.database_oid,
                schema_profile_digest: self.schema_profile_digest,
                state_digest: self.state_digest,
                log_digest: self.log_digest,
                historical_members: self.historical_members,
                current_members: self.current_members,
                history_membership_root: self.history_membership_root,
                current_membership_root: self.current_membership_root,
            },
            history_rows: &self.history_rows,
            current_rows: &self.current_rows,
            history_spool: self.history_spool.as_ref(),
            current_spool: self.current_spool.as_ref(),
            profile: self.streamed_profile,
        }
    }
}
impl WarmSuccessorCut {
    fn installation(&self) -> MembershipInstallation<'_> {
        MembershipInstallation {
            audited_root: &self.audited_root,
            domain: &self.domain,
            descriptor_cut: self.descriptor_cut.clone(),
            history_rows: &self.history_rows,
            current_rows: &self.current_rows,
            history_spool: None,
            current_spool: None,
            profile: None,
        }
    }
}

/// Physically installed and independently compared private CMD membership.
/// This is still a synthetic laboratory cut, never a source admission seal.
#[derive(Clone, Debug)]
pub struct CompleteGeneration {
    cut: ColdCut,
    installed: InstalledGenerationV1,
    history_coverage: GenerationCoverageV1,
    current_coverage: GenerationCoverageV1,
}

/// Cold reopened selected descriptor, tied to a freshly re-audited private
/// DB cut. It is historical evidence, not a current-rights disclosure lease.
#[derive(Clone, Debug)]
pub struct VerifiedSelectedGeneration {
    cut: ColdCut,
    installed: InstalledGenerationV1,
    history_coverage: GenerationCoverageV1,
    current_coverage: GenerationCoverageV1,
}

impl VerifiedSelectedGeneration {
    pub fn digest(&self) -> Digest256 {
        self.installed.digest()
    }
    pub fn cut(&self) -> &ColdCut {
        &self.cut
    }
    pub fn history_coverage(&self) -> GenerationCoverageV1 {
        self.history_coverage
    }
    pub fn current_coverage(&self) -> GenerationCoverageV1 {
        self.current_coverage
    }

    /// Pinned, bounded row traversal of the exact selected descriptor. The
    /// caller must consume through EOF and inspect coverage; this is neither
    /// a current-rights lease nor a source validation attestation.
    pub fn stream(&self, namespace: GenerationNamespaceV1) -> DurableResult<GenerationRowStreamV1> {
        let limits = self
            .cut
            .streamed_profile
            .map_or_else(generation_limits, |p| p.generation);
        Ok(self.installed.stream(namespace, limits)?)
    }
}

impl CompleteGeneration {
    pub fn digest(&self) -> Digest256 {
        self.installed.digest()
    }
    pub fn cut(&self) -> &ColdCut {
        &self.cut
    }
    pub fn descriptor(&self) -> &GenerationDescriptorV1 {
        self.installed.descriptor()
    }
    pub fn history_coverage(&self) -> GenerationCoverageV1 {
        self.history_coverage
    }
    pub fn current_coverage(&self) -> GenerationCoverageV1 {
        self.current_coverage
    }
}

impl ColdCut {
    pub fn through_commit_seq(&self) -> u64 {
        self.through_commit_seq
    }
    pub fn log_digest(&self) -> Digest256 {
        self.log_digest
    }
    pub fn state_digest(&self) -> Digest256 {
        self.state_digest
    }
    pub fn audit_generation(&self) -> u64 {
        self.audit_generation
    }
    pub fn historical_members(&self) -> u64 {
        self.historical_members
    }
    pub fn current_members(&self) -> u64 {
        self.current_members
    }
    pub fn history_membership_root(&self) -> Digest256 {
        self.history_membership_root
    }
    pub fn current_membership_root(&self) -> Digest256 {
        self.current_membership_root
    }
}

impl ColdRecoveredMember {
    pub fn digest(&self) -> Digest256 {
        self.receipt.coordinate().sha256
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// A deliberately non-source test profile. The exact bytes embed the proposed
/// owner revision; no post-seal metadata assignment may silently renumber it.
pub fn lab_record_bytes(subject: &str, revision: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(18 + subject.len() + payload.len());
    bytes.extend_from_slice(LAB_MAGIC);
    bytes.extend_from_slice(&revision.to_be_bytes());
    bytes.extend_from_slice(&(subject.len() as u16).to_be_bytes());
    bytes.extend_from_slice(subject.as_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn check_lab_record(bytes: &[u8], subject: &str, revision: u64) -> DurableResult<()> {
    if bytes.len() < 18 || &bytes[..8] != LAB_MAGIC {
        return Err(DurableError::Invalid("lab record envelope differs"));
    }
    let embedded_revision = u64::from_be_bytes(
        bytes[8..16]
            .try_into()
            .map_err(|_| DurableError::Invalid("lab revision absent"))?,
    );
    let subject_len = u16::from_be_bytes(
        bytes[16..18]
            .try_into()
            .map_err(|_| DurableError::Invalid("lab subject length absent"))?,
    ) as usize;
    let end = 18usize
        .checked_add(subject_len)
        .ok_or(DurableError::Invalid("lab subject length overflow"))?;
    if end > bytes.len() || &bytes[18..end] != subject.as_bytes() || embedded_revision != revision {
        return Err(DurableError::Invalid(
            "embedded subject or revision differs from proposal",
        ));
    }
    Ok(())
}

/// Commitment over the exact proposed, owner-checked members. This is a
/// synthetic lab delta, not a legacy source command input profile.
pub fn durable_shadow_delta(members: &[DurableShadowMember]) -> Digest256 {
    let identities: Vec<_> = members
        .iter()
        .map(|member| ShadowWriteIdentity {
            member_slot: member.member_slot,
            subject: &member.subject,
            expected_predecessor: member.expected_predecessor,
            proposed_revision: member.proposed_revision,
            exact_bytes: &member.exact_bytes,
        })
        .collect();
    durable_shadow_delta_prepared(&identities)
}

pub fn durable_shadow_delta_prepared(members: &[ShadowWriteIdentity<'_>]) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-lab-delta-v1");
    part(&mut hasher, &(members.len() as u64).to_be_bytes());
    for member in members {
        part(&mut hasher, &member.member_slot.to_be_bytes());
        part(&mut hasher, member.subject.as_bytes());
        part(&mut hasher, &member.proposed_revision.to_be_bytes());
        match member.expected_predecessor {
            Some((version, digest)) => {
                part(&mut hasher, b"present");
                part(&mut hasher, &version.to_be_bytes());
                part(&mut hasher, digest.as_bytes());
            }
            None => part(&mut hasher, b"absent"),
        }
        part(
            &mut hasher,
            Digest256::of_bytes(member.exact_bytes).as_bytes(),
        );
        part(
            &mut hasher,
            &(member.exact_bytes.len() as u64).to_be_bytes(),
        );
    }
    hasher.finalize()
}

fn part(hasher: &mut Digest256Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn update_member_root(hasher: &mut Digest256Hasher, row: &postgres::Row) {
    let slot: i32 = row.get("member_slot");
    let subject: String = row.get("subject");
    let revision: i64 = row.get("proposed_revision");
    let receipt_id: String = row.get("sto_receipt_id");
    let content_digest: String = row.get("content_digest");
    let content_length: i64 = row.get("content_length");
    part(hasher, &slot.to_be_bytes());
    part(hasher, subject.as_bytes());
    part(hasher, &revision.to_be_bytes());
    part(hasher, receipt_id.as_bytes());
    part(hasher, content_digest.as_bytes());
    part(hasher, &content_length.to_be_bytes());
    source_cohort::hash_source_metadata(hasher, row);
}

fn metadata_locator_digest(row: &postgres::Row) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-metadata-locator-v1");
    for column in [
        "domain",
        "subject",
        "profile_id",
        "profile_version",
        "content_digest",
        "custody_domain_digest",
        "segment_digest",
        "frame_digest",
        "sto_receipt_id",
        "durability_class",
    ] {
        part(&mut hasher, row.get::<_, String>(column).as_bytes());
    }
    for column in ["prepare_id", "store_id", "custody_domain", "pin_id"] {
        part(&mut hasher, &row.get::<_, Vec<u8>>(column));
    }
    for column in [
        "revision",
        "commit_seq",
        "content_length",
        "pin_fence",
        "segment_size",
        "frame_header_offset",
        "frame_length",
    ] {
        part(&mut hasher, &row.get::<_, i64>(column).to_be_bytes());
    }
    for column in ["member_slot", "frame_index"] {
        part(&mut hasher, &row.get::<_, i32>(column).to_be_bytes());
    }
    source_cohort::hash_source_metadata(&mut hasher, row);
    hasher.finalize()
}

fn as_i64(value: u64) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Invalid("number exceeds PostgreSQL bigint"))
}

fn as_u64(value: i64) -> DurableResult<u64> {
    u64::try_from(value).map_err(|_| DurableError::Corrupt("negative metadata number"))
}

fn parse_hex(value: String) -> DurableResult<Digest256> {
    Digest256::from_hex(&value).map_err(|_| DurableError::Corrupt("invalid metadata digest"))
}

fn schema_profile_digest() -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-coordinator-schema-profile-v3");
    part(&mut hasher, include_bytes!("durable_schema.sql"));
    part(&mut hasher, COLD_AUDIT_PROFILE);
    part(&mut hasher, RECEIPT_PROFILE);
    hasher.finalize()
}

const METADATA_TABLES: [(&str, &str); 10] = [
    ("cmd2_job", "job_id"),
    ("cmd2_predicate", "kind,owner,scope,token"),
    ("cmd2_attempt", "prepare_id"),
    ("cmd2_member", "prepare_id,member_slot"),
    ("cmd2_current", "subject"),
    ("cmd2_history", "subject,revision"),
    ("cmd2_receipt", "command_id"),
    ("cmd2_log", "commit_seq"),
    ("cmd2_outbox", "commit_seq"),
    ("cmd2_source_index", "kind,token,path"),
];
fn admit_private_metadata(
    tx: &mut Transaction<'_>,
    domain: &str,
    started: Instant,
    requested: Option<(Instant, &AtomicBool)>,
    profile: Option<StreamedGenerationProfile>,
) -> DurableResult<()> {
    let mut admitted_rows = 0u64;
    let mut admitted_bytes = 0u64;
    for (table, _) in METADATA_TABLES {
        check_cold_profile_deadline(started, requested, profile)?;
        let query = format!(
            "SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0),
                        coalesce(sum(octet_length(row_to_json(t)::text)),0)
                 FROM {table} t WHERE domain=$1"
        );
        let row = tx.query_one(&query, &[&domain])?;
        admitted_rows = admitted_rows
            .checked_add(as_u64(row.get::<_, i64>(0))?)
            .ok_or(DurableError::Refused("cold metadata row count overflow"))?;
        admitted_bytes = admitted_bytes
            .checked_add(as_u64(row.get::<_, i64>(2))?)
            .ok_or(DurableError::Refused("cold metadata byte count overflow"))?;
        if admitted_rows > profile.map_or(100_000, |p| p.max_metadata_rows) as u64
            || row.get::<_, i32>(1) > 1_048_576
            || admitted_bytes > profile.map_or(64 * 1024 * 1024, |p| p.max_metadata_bytes) as u64
        {
            return Err(DurableError::Refused(
                "cold metadata preadmission budget exceeded",
            ));
        }
    }
    Ok(())
}
fn append_private_metadata(
    tx: &mut Transaction<'_>,
    domain: &str,
    state_hasher: &mut Digest256Hasher,
    started: Instant,
    requested: Option<(Instant, &AtomicBool)>,
    profile: Option<StreamedGenerationProfile>,
) -> DurableResult<()> {
    let mut audited_rows = 0usize;
    let mut audited_metadata_bytes = 0usize;
    for (table, order) in METADATA_TABLES {
        check_cold_profile_deadline(started, requested, profile)?;
        part(state_hasher, table.as_bytes());
        let query =
            format!("SELECT row_to_json(t)::text FROM {table} t WHERE domain=$1 ORDER BY {order}");
        let mut rows = tx.query_raw(&query, &[&domain])?;
        let mut table_rows = 0u64;
        while let Some(row) = rows.next()? {
            check_cold_profile_deadline(started, requested, profile)?;
            audited_rows = audited_rows
                .checked_add(1)
                .ok_or(DurableError::Refused("cold metadata row count overflow"))?;
            if audited_rows > profile.map_or(100_000, |p| p.max_metadata_rows) {
                return Err(DurableError::Refused("cold metadata row budget exceeded"));
            }
            table_rows += 1;
            let encoded: String = row.get(0);
            if encoded.len() > 1_048_576 {
                return Err(DurableError::Refused(
                    "cold metadata row exceeds byte budget",
                ));
            }
            audited_metadata_bytes = audited_metadata_bytes
                .checked_add(encoded.len())
                .ok_or(DurableError::Refused("cold metadata byte count overflow"))?;
            if audited_metadata_bytes > profile.map_or(64 * 1024 * 1024, |p| p.max_metadata_bytes) {
                return Err(DurableError::Refused("cold metadata byte budget exceeded"));
            }
            part(state_hasher, encoded.as_bytes());
        }
        part(state_hasher, &table_rows.to_be_bytes());
    }
    let domain_state: String = tx
        .query_one(
            "SELECT row_to_json(d)::text FROM
             (SELECT domain,head_seq,rights_version,rights_allowed,rule_version,
                     contract_digest,schema_profile_digest,source_revision,source_membership_digest,
                     source_membership_count,source_epoch,source_generation,source_complete,
                     source_definition_digest
              FROM cmd2_domain WHERE domain=$1) d",
            &[&domain],
        )?
        .get(0);
    part(state_hasher, domain_state.as_bytes());
    Ok(())
}

fn lock_audit_fence(tx: &mut Transaction<'_>, domain: &str) -> DurableResult<u64> {
    let row = tx
        .query_opt(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?
        .ok_or(DurableError::Corrupt("domain audit fence absent"))?;
    if row.get::<_, String>(1) != "normal" {
        return Err(DurableError::Refused("physical maintenance in progress"));
    }
    as_u64(row.get(0))
}

fn database_oid(tx: &mut Transaction<'_>) -> DurableResult<u64> {
    let oid: i64 = tx
        .query_one(
            "SELECT oid::bigint FROM pg_database WHERE datname=current_database()",
            &[],
        )?
        .get(0);
    as_u64(oid)
}

pub struct DurablePgCoordinator {
    client: Client,
}

impl DurablePgCoordinator {
    pub fn connect(url: &str) -> DurableResult<Self> {
        let mut client = Client::connect(url, NoTls)?;
        let version: i32 = client
            .query_one("SELECT current_setting('server_version_num')::integer", &[])?
            .get(0);
        if version / 10_000 != 16 {
            return Err(DurableError::Refused(
                "CMD2 audit row profile requires PostgreSQL 16",
            ));
        }
        Ok(Self { client })
    }

    pub fn backend_pid(&mut self) -> DurableResult<i32> {
        Ok(self
            .client
            .query_one("SELECT pg_backend_pid()", &[])?
            .get(0))
    }

    pub fn init_lab_schema(&mut self) -> DurableResult<()> {
        self.client
            .batch_execute(include_str!("durable_schema.sql"))?;
        Ok(())
    }

    pub fn create_domain(&mut self, domain: &str, contract_digest: Digest256) -> DurableResult<()> {
        if domain.is_empty() {
            return Err(DurableError::Invalid("empty domain"));
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        tx.execute(
            "INSERT INTO cmd2_domain(domain,contract_digest,schema_profile_digest)
             VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
            &[
                &domain,
                &contract_digest.to_hex(),
                &schema_profile_digest().to_hex(),
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_job_epoch(&mut self, domain: &str, job_id: &str, epoch: u64) -> DurableResult<()> {
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, domain)?;
        let changed = tx.execute(
            "INSERT INTO cmd2_job(domain,job_id,fence_epoch) VALUES($1,$2,$3)
             ON CONFLICT(domain,job_id) DO UPDATE SET fence_epoch=EXCLUDED.fence_epoch
             WHERE EXCLUDED.fence_epoch >= cmd2_job.fence_epoch",
            &[&domain, &job_id, &as_i64(epoch)?],
        )?;
        if changed != 1 {
            return Err(DurableError::Refused("job fence cannot decrease"));
        }
        tx.commit()?;
        Ok(())
    }

    /// Return the durable registration fence that must be bound into the
    /// synced STO intent. A repeated registration cannot restart an attempt
    /// that already advanced beyond `registered`.
    pub fn register_attempt(&mut self, request: &RegisterShadowAttempt<'_>) -> DurableResult<u64> {
        if request.domain.is_empty()
            || request.prepare_id.is_empty()
            || request.command_id.is_empty()
        {
            return Err(DurableError::Invalid("empty attempt identity"));
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, request.domain)?;
        let changed = tx.execute(
            "INSERT INTO cmd2_attempt(domain,prepare_id,command_id,raw_request_digest,delta_digest,
             state,attempt_fence) VALUES($1,$2,$3,$4,$5,'registered',1)
             ON CONFLICT DO NOTHING",
            &[
                &request.domain,
                &request.prepare_id,
                &request.command_id,
                &request.raw_request_digest.to_hex(),
                &request.delta_digest.to_hex(),
            ],
        )?;
        let attempt_fence = if changed == 0 {
            let row = tx.query_opt(
                "SELECT command_id,raw_request_digest,delta_digest,state,attempt_fence
                 FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
                &[&request.domain, &request.prepare_id],
            )?;
            let Some(row) = row else {
                return Err(DurableError::Conflict(
                    "command ID already belongs to a different prepare ID",
                ));
            };
            if row.get::<_, String>(0) != request.command_id
                || row.get::<_, String>(1) != request.raw_request_digest.to_hex()
                || row.get::<_, String>(2) != request.delta_digest.to_hex()
            {
                return Err(DurableError::Conflict("attempt identity collision"));
            }
            if row.get::<_, String>(3) != "registered" {
                return Err(DurableError::Refused("registered attempt already advanced"));
            }
            as_u64(row.get::<_, i64>(4))?
        } else {
            1
        };
        if attempt_fence == 0 {
            return Err(DurableError::Corrupt("registered attempt fence is zero"));
        }
        tx.commit()?;
        Ok(attempt_fence)
    }

    /// Attach exact STO handles to a registered private attempt. The shared
    /// pin guard protects the pre-transaction verification through this row
    /// transition; no domain sequencer lock is held during byte I/O.
    pub fn attach_ready(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_attempt_fence: u64,
        members: &[DurableShadowMember],
    ) -> DurableResult<()> {
        self.attach_ready_profile(
            store,
            domain,
            prepare_id,
            expected_attempt_fence,
            members,
            PROFILE_ID,
            None,
            None,
        )
    }

    fn attach_ready_profile(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_attempt_fence: u64,
        members: &[DurableShadowMember],
        profile: &[u8],
        registered_delta: Option<Digest256>,
        source_metadata: Option<&source_cohort::SourceMetadata>,
    ) -> DurableResult<()> {
        self.attach_ready_profile_bound(
            store,
            domain,
            prepare_id,
            expected_attempt_fence,
            members,
            profile,
            registered_delta,
            source_metadata,
            None,
        )
        .map(|_| ())
    }
    fn attach_ready_profile_bound(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_attempt_fence: u64,
        members: &[DurableShadowMember],
        profile: &[u8],
        registered_delta: Option<Digest256>,
        source_metadata: Option<&source_cohort::SourceMetadata>,
        expected_audit: Option<u64>,
    ) -> DurableResult<u64> {
        if expected_attempt_fence == 0 || members.is_empty() || members.len() > MAX_MEMBERS {
            return Err(DurableError::Invalid("invalid member count"));
        }
        let receipts: Vec<_> = members
            .iter()
            .map(|member| member.receipt.clone())
            .collect();
        let _guard = store.verify_and_hold_fenced(
            prepare_id,
            expected_attempt_fence,
            0,
            &receipts,
            verification_budget(receipts.len()),
        )?;
        let first = &members[0].receipt;
        let mut slots = std::collections::HashSet::new();
        let mut subjects = std::collections::HashSet::new();
        for member in members {
            check_durable_member(domain, prepare_id, member, profile)?;
            if !slots.insert(member.member_slot) || !subjects.insert(member.subject.as_str()) {
                return Err(DurableError::Invalid("duplicate member slot or subject"));
            }
            if member.receipt.pin_id() != first.pin_id()
                || member.receipt.fence_epoch() != first.fence_epoch()
                || member.receipt.store_id() != first.store_id()
            {
                return Err(DurableError::Invalid("compound members use different pins"));
            }
        }
        let delta_digest = registered_delta.unwrap_or_else(|| durable_shadow_delta(members));
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let audit = lock_audit_fence(&mut tx, domain)?;
        if expected_audit.is_some_and(|expected| expected != audit) {
            return Err(DurableError::Conflict(
                "warm attach continuation lost; cold reopen required",
            ));
        }
        source_cohort::check_writer_profile(&mut tx, domain, profile)?;
        let row = tx.query_one(
            "SELECT state,delta_digest,attempt_fence FROM cmd2_attempt
             WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        let state: String = row.get(0);
        if row.get::<_, String>(1) != delta_digest.to_hex() {
            return Err(DurableError::Conflict(
                "attached delta differs from registration",
            ));
        }
        if as_u64(row.get::<_, i64>(2))? != expected_attempt_fence {
            return Err(DurableError::Conflict("registered attempt fence changed"));
        }
        if state == "ready" {
            let existing = tx.query(
                "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?;
            if existing.len() != members.len() {
                return Err(DurableError::Conflict("ready member count differs"));
            }
            for member in members {
                let row = existing
                    .iter()
                    .find(|row| row.get::<_, i32>("member_slot") == member.member_slot as i32)
                    .ok_or(DurableError::Conflict("ready member slot differs"))?;
                source_cohort::check_source_metadata(row, source_metadata, &member.subject)?;
                check_member_row(
                    row,
                    &member.receipt,
                    &member.subject,
                    member.proposed_revision,
                )?;
            }
            tx.commit()?;
            return Ok(audit);
        }
        if state != "registered" {
            return Err(DurableError::Refused("attempt is not attachable"));
        }
        for member in members {
            let receipt = &member.receipt;
            let coordinate = receipt.coordinate();
            let (expected_revision, expected_digest) = match member.expected_predecessor {
                Some((version, digest)) => (Some(as_i64(version)?), Some(digest.to_hex())),
                None => (None, None),
            };
            tx.execute(
                "INSERT INTO cmd2_member (
                  domain,prepare_id,member_slot,profile_id,profile_version,subject,
                  expected_revision,expected_digest,proposed_revision,content_digest,content_length,
                  store_id,custody_domain_digest,custody_domain,pin_id,pin_fence,
                  segment_digest,segment_size,frame_index,frame_header_offset,
                  frame_digest,frame_length,sto_receipt_id,durability_class)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,
                         $17,$18,$19,$20,$21,$22,$23,$24)",
                &[
                    &domain,
                    &prepare_id,
                    &(member.member_slot as i32),
                    &std::str::from_utf8(profile)
                        .map_err(|_| DurableError::Invalid("profile UTF-8"))?,
                    &"1",
                    &member.subject,
                    &expected_revision,
                    &expected_digest,
                    &as_i64(member.proposed_revision)?,
                    &coordinate.sha256.to_hex(),
                    &as_i64(coordinate.size_bytes)?,
                    &receipt.store_id().as_slice(),
                    &receipt.domain_digest().to_hex(),
                    &receipt.custody_domain(),
                    &receipt.pin_id().as_slice(),
                    &as_i64(receipt.fence_epoch())?,
                    &receipt.segment_digest().to_hex(),
                    &as_i64(receipt.segment_size())?,
                    &(receipt.frame_index() as i32),
                    &as_i64(coordinate.header_offset)?,
                    &coordinate.sha256.to_hex(),
                    &as_i64(coordinate.size_bytes)?,
                    &receipt.receipt_id().to_hex(),
                    &"LinuxFileAndDirectorySyncReopenSha256V1",
                ],
            )?;
            if let Some(metadata) = source_metadata {
                let value = metadata
                    .get(&member.subject)
                    .ok_or(DurableError::Corrupt("source member metadata absent"))?;
                tx.execute("UPDATE cmd2_member SET source_mode=$4,source_dependencies=$5 WHERE domain=$1 AND prepare_id=$2 AND member_slot=$3", &[&domain,&prepare_id,&(member.member_slot as i32),&(value.mode as i32),&value.dependencies])?;
            }
        }
        tx.execute(
            "UPDATE cmd2_attempt SET state='ready' WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &prepare_id],
        )?;
        let audit = lock_audit_fence(&mut tx, domain)?;
        tx.commit()?;
        Ok(audit)
    }

    /// One private shadow commit. STO verifies whole pinned segments before
    /// PostgreSQL begins; the guard prevents an exclusive pin abort through
    /// the short attempt-row and sequencer transaction.
    pub fn commit_shadow(
        &mut self,
        store: &SegmentStore,
        request: &CommitShadowAttempt<'_>,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        self.commit_durable(store, request, source_cohort::CommitMode::Shadow)
    }

    fn commit_durable(
        &mut self,
        store: &SegmentStore,
        request: &CommitShadowAttempt<'_>,
        mode: source_cohort::CommitMode<'_>,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        if request.attempt_fence == 0
            || request.receipts.is_empty()
            || request.receipts.len() > MAX_MEMBERS
        {
            return Err(DurableError::Invalid("invalid receipt count"));
        }
        let verification_start = Instant::now();
        let guard = store.verify_and_hold_fenced(
            request.prepare_id,
            request.attempt_fence,
            0,
            request.receipts,
            verification_budget(request.receipts.len()),
        )?;
        if guard.prepare_id() != request.prepare_id {
            return Err(DurableError::Conflict("guard prepare ID differs"));
        }
        let verification = verification_start.elapsed();
        let tx_start = Instant::now();
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let lock_start = Instant::now();
        let observed_audit = lock_audit_fence(&mut tx, request.domain)?;
        let fence_acquired = Instant::now();
        let attempt = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&request.domain, &request.prepare_id],
        )?;
        let state: String = attempt.get("state");
        let replayed = state == "committed";
        source_cohort::check_warm_continuation(&mode, observed_audit, replayed)?;
        if as_u64(attempt.get::<_, i64>("attempt_fence"))? != request.attempt_fence {
            return Err(DurableError::Conflict("attempt fence changed after seal"));
        }
        if !replayed && state != "ready" {
            return Err(DurableError::Refused(
                "attempt is not ready at registered fence",
            ));
        }
        let member_rows = tx.query(
            "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot",
            &[&request.domain, &request.prepare_id],
        )?;
        if member_rows.len() != request.receipts.len() {
            return Err(DurableError::Corrupt("ready member count differs"));
        }
        for row in &member_rows {
            let slot: i32 = row.get("member_slot");
            let receipt = request
                .receipts
                .iter()
                .find(|receipt| receipt.binding().member_slot == slot as u32)
                .ok_or(DurableError::Conflict("receipt member slot missing"))?;
            let subject: String = row.get("subject");
            let revision = as_u64(row.get("proposed_revision"))?;
            check_member_row(row, receipt, &subject, revision)?;
        }
        tx.query_one(
            "SELECT 1 FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&request.domain],
        )?;
        let lock_wait = lock_start.elapsed();
        // The fence is the first serialization lock in this lab profile.
        // Report its full hold duration, including later attempt/domain waits.
        // A fresh READ COMMITTED statement after the row-lock wait observes
        // any rights/rule/sequence decision made by the preceding owner.
        let domain = tx.query_one(
            "SELECT head_seq,rights_version,rights_allowed,rule_version,contract_digest,
                    schema_profile_digest
             FROM cmd2_domain WHERE domain=$1",
            &[&request.domain],
        )?;
        let head = as_u64(domain.get(0))?;
        if head >= MAX_CUT {
            return Err(DurableError::Refused("shadow cut budget exceeded"));
        }
        let rights_version = as_u64(domain.get(1))?;
        let rights_allowed: bool = domain.get(2);
        if !rights_allowed || rights_version != request.expected_rights_version {
            return Err(DurableError::Refused("current local rights changed"));
        }
        source_cohort::check_commit_owner(
            &mut tx,
            request,
            &attempt,
            &member_rows,
            &mode,
            replayed,
        )?;
        if replayed {
            let receipt = receipt_from_committed_attempt(&mut tx, &attempt)?;
            source_cohort::verify_owner_before_outcome(&mode)?;
            let lock_held = fence_acquired.elapsed();
            tx.commit()
                .map_err(|_| DurableError::Indeterminate("replay transaction outcome unknown"))?;
            drop(guard);
            return Ok((
                DurableCommitReceipt {
                    replayed: true,
                    ..receipt
                },
                DurableTiming {
                    verification,
                    lock_wait,
                    lock_held,
                    transaction: tx_start.elapsed(),
                },
            ));
        }
        if !matches!(mode, source_cohort::CommitMode::Creation { .. })
            && head != request.full_base_seq
        {
            return Err(DurableError::Conflict(
                "FullOnly base drift; re-audit outside lock",
            ));
        }
        if as_u64(domain.get(3))? != request.expected_rule_version
            || domain.get::<_, String>(4) != request.expected_contract_digest.to_hex()
            || domain.get::<_, Option<String>>(5) != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Conflict(
                "rule/schema/registry contract changed",
            ));
        }
        let lease = tx.query_opt(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR UPDATE",
            &[&request.domain, &request.job_id],
        )?;
        if lease.map(|row| row.get::<_, i64>(0)) != Some(as_i64(request.job_fence)?) {
            return Err(DurableError::Refused("job fence changed"));
        }
        let seq = head
            .checked_add(1)
            .ok_or(DurableError::Corrupt("commit sequence overflow"))?;
        let seq_db = as_i64(seq)?;
        let mut member_hasher = Digest256Hasher::new();
        part(&mut member_hasher, b"cmd2-member-root-v1");
        part(
            &mut member_hasher,
            &(member_rows.len() as u64).to_be_bytes(),
        );
        for row in &member_rows {
            let subject: String = row.get("subject");
            let revision: i64 = row.get("proposed_revision");
            let expected_revision: Option<i64> = row.get("expected_revision");
            let expected_digest: Option<String> = row.get("expected_digest");
            let current = tx.query_opt(
                "SELECT revision,content_digest FROM cmd2_current
                 WHERE domain=$1 AND subject=$2",
                &[&request.domain, &subject],
            )?;
            match (current, expected_revision, expected_digest.as_deref()) {
                (None, None, None) if revision == 1 => {}
                (Some(current), Some(expected), Some(digest))
                    if current.get::<_, i64>(0) == expected
                        && current.get::<_, String>(1) == digest
                        && expected.checked_add(1) == Some(revision) => {}
                _ => return Err(DurableError::Conflict("exact predecessor/revision changed")),
            }
            let slot: i32 = row.get("member_slot");
            update_member_root(&mut member_hasher, row);
            // The source-visible projection and retained exact locator are
            // inserted from the same attached row inside this transaction.
            tx.execute(
                "INSERT INTO cmd2_history (
                  domain,subject,revision,commit_seq,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class,source_mode,source_dependencies)
                 SELECT domain,subject,proposed_revision,$3,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class,source_mode,source_dependencies
                 FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 AND member_slot=$4",
                &[&request.domain, &request.prepare_id, &seq_db, &slot],
            )?;
            tx.execute(
                "DELETE FROM cmd2_current WHERE domain=$1 AND subject=$2",
                &[&request.domain, &subject],
            )?;
            tx.execute(
                "INSERT INTO cmd2_current (
                  domain,subject,revision,commit_seq,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class,source_mode,source_dependencies)
                 SELECT domain,subject,proposed_revision,$3,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class,source_mode,source_dependencies
                 FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 AND member_slot=$4",
                &[&request.domain, &request.prepare_id, &seq_db, &slot],
            )?;
        }
        source_cohort::apply_source_change(&mut tx, request, &mode, seq)?;
        let members_root = member_hasher.finalize();
        let command_id: String = attempt.get("command_id");
        let raw_request_digest: String = attempt.get("raw_request_digest");
        let delta_digest: String = attempt.get("delta_digest");
        let mut receipt_hasher = Digest256Hasher::new();
        part(&mut receipt_hasher, RECEIPT_PROFILE);
        for value in [
            request.domain.as_bytes(),
            request.prepare_id,
            &request.attempt_fence.to_be_bytes(),
            command_id.as_bytes(),
            &seq.to_be_bytes(),
            raw_request_digest.as_bytes(),
            delta_digest.as_bytes(),
            members_root.as_bytes(),
        ] {
            part(&mut receipt_hasher, value);
        }
        let receipt_digest = receipt_hasher.finalize();
        tx.execute(
            "UPDATE cmd2_domain SET head_seq=$2 WHERE domain=$1",
            &[&request.domain, &seq_db],
        )?;
        tx.execute(
            "INSERT INTO cmd2_receipt(domain,command_id,prepare_id,commit_seq,raw_request_digest,
             delta_digest,receipt_digest,members_root) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &request.domain,
                &command_id,
                &request.prepare_id,
                &seq_db,
                &raw_request_digest,
                &delta_digest,
                &receipt_digest.to_hex(),
                &members_root.to_hex(),
            ],
        )?;
        tx.execute(
            "INSERT INTO cmd2_log(domain,commit_seq,event_kind,command_id,delta_digest,members_root)
             VALUES($1,$2,'command',$3,$4,$5)",
            &[
                &request.domain,
                &seq_db,
                &command_id,
                &delta_digest,
                &members_root.to_hex(),
            ],
        )?;
        let event_id = format!("{}:{seq}", request.domain);
        tx.execute(
            "INSERT INTO cmd2_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&request.domain, &seq_db, &event_id],
        )?;
        tx.execute(
            "UPDATE cmd2_attempt SET state='committed',commit_seq=$3,receipt_digest=$4
             WHERE domain=$1 AND prepare_id=$2",
            &[
                &request.domain,
                &request.prepare_id,
                &seq_db,
                &receipt_digest.to_hex(),
            ],
        )?;
        source_cohort::verify_owner_before_outcome(&mode)?;
        let committed_audit = lock_audit_fence(&mut tx, request.domain)?;
        let lock_held = fence_acquired.elapsed();
        tx.commit()
            .map_err(|_| DurableError::Indeterminate("commit outcome unknown; retain pin"))?;
        source_cohort::record_warm_commit(&mode, committed_audit, seq, request.receipts);
        drop(guard);
        Ok((
            DurableCommitReceipt {
                domain: request.domain.to_owned(),
                prepare_id: request.prepare_id.to_vec(),
                command_id,
                commit_seq: seq,
                raw_request_digest: parse_hex(raw_request_digest)?,
                delta_digest: parse_hex(delta_digest)?,
                member_root: members_root,
                replayed: false,
            },
            DurableTiming {
                verification,
                lock_wait,
                lock_held,
                transaction: tx_start.elapsed(),
            },
        ))
    }

    pub fn resolve_attempt(
        &mut self,
        domain: &str,
        prepare_id: &[u8],
    ) -> DurableResult<AttemptResolution> {
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        let resolution = match row.get::<_, String>("state").as_str() {
            "registered" => AttemptResolution::Registered,
            "ready" => AttemptResolution::Ready,
            "aborted" => AttemptResolution::Aborted,
            "committed" => {
                AttemptResolution::Committed(receipt_from_committed_attempt(&mut tx, &row)?)
            }
            _ => return Err(DurableError::Corrupt("unknown attempt state")),
        };
        tx.commit()?;
        Ok(resolution)
    }

    /// Fence the attempt in PostgreSQL first. The STO exclusive abort happens
    /// only after the transaction has committed and released both DB locks.
    /// A failed STO abort leaves retained forensic bytes and a durable DB
    /// refusal; the caller can retry/reconcile, never infer a safe delete.
    pub fn cancel_attempt(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
    ) -> DurableResult<CancelOutcome> {
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let mut tx = self.client.transaction()?;
        lock_audit_fence(&mut tx, domain)?;
        let attempt = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        let state: String = attempt.get("state");
        if state == "committed" {
            let receipt = receipt_from_committed_attempt(&mut tx, &attempt)?;
            tx.commit()?;
            return Ok(CancelOutcome::AlreadyCommitted(receipt));
        }
        let stored_attempt_fence = as_u64(attempt.get::<_, i64>("attempt_fence"))?;
        let pre_abort_attempt_fence = if state == "aborted" {
            stored_attempt_fence
                .checked_sub(1)
                .filter(|fence| *fence > 0)
                .ok_or(DurableError::Corrupt("aborted attempt fence invalid"))?
        } else {
            if stored_attempt_fence == 0 {
                return Err(DurableError::Corrupt("registered attempt fence is zero"));
            }
            stored_attempt_fence
        };
        let member_rows = tx.query(
            "SELECT pin_id,pin_fence FROM cmd2_member
             WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &prepare_id],
        )?;
        let pin = if member_rows.is_empty() {
            None
        } else {
            let pin: Vec<u8> = member_rows[0].get(0);
            let fence: i64 = member_rows[0].get(1);
            if pin.len() != 16
                || member_rows
                    .iter()
                    .any(|row| row.get::<_, Vec<u8>>(0) != pin || row.get::<_, i64>(1) != fence)
            {
                return Err(DurableError::Corrupt(
                    "attempt has inconsistent member pins",
                ));
            }
            let mut pin_id = [0u8; 16];
            pin_id.copy_from_slice(&pin);
            Some((pin_id, as_u64(fence)?))
        };
        if state != "aborted" {
            if state != "registered" && state != "ready" {
                return Err(DurableError::Corrupt("unknown attempt state"));
            }
            tx.execute(
                "UPDATE cmd2_attempt SET state='aborted',attempt_fence=attempt_fence+1
                 WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?;
        }
        tx.commit()
            .map_err(|_| DurableError::Indeterminate("cancel fence outcome unknown; retain pin"))?;
        // A seal can survive SIGKILL before attach_ready wrote member rows.
        // The durable STO intent discovers that pin by exact prepare ID; the
        // already-committed PostgreSQL attempt fence is the abort authority.
        // No physical lookup or absent receipt alone authorizes abort.
        let recovered = store.recover_attempt_fenced(prepare_id, pre_abort_attempt_fence, 0)?;
        let pin_fenced =
            match recovered {
                Some(AttemptRecovery::Sealed { receipts }) => {
                    let first = receipts
                        .first()
                        .ok_or(DurableError::Corrupt("sealed attempt has no frames"))?;
                    if receipts.iter().any(|r| {
                        r.pin_id() != first.pin_id() || r.fence_epoch() != first.fence_epoch()
                    }) || pin.is_some_and(|(id, fence)| {
                        id != first.pin_id() || fence != first.fence_epoch()
                    }) {
                        return Err(DurableError::Corrupt(
                            "attempt pin differs from member rows",
                        ));
                    }
                    store
                        .abort_uncommitted_fenced(
                            first.pin_id(),
                            prepare_id,
                            pre_abort_attempt_fence,
                            0,
                            first.fence_epoch(),
                        )
                        .is_ok()
                }
                Some(AttemptRecovery::Aborted { pin_id, .. }) => {
                    if pin.is_some_and(|(id, _)| id != pin_id) {
                        return Err(DurableError::Corrupt(
                            "aborted pin differs from member rows",
                        ));
                    }
                    true
                }
                Some(AttemptRecovery::IntentOnly { pin_id })
                | Some(AttemptRecovery::Preparing { pin_id, .. }) => {
                    if pin.is_some_and(|(id, _)| id != pin_id) {
                        return Err(DurableError::Corrupt(
                            "preparing pin differs from member rows",
                        ));
                    }
                    false
                }
                None => {
                    if pin.is_some() {
                        return Err(DurableError::Indeterminate(
                            "member pin has no durable intent",
                        ));
                    }
                    false
                }
            };
        Ok(CancelOutcome::Cancelled { pin_fenced })
    }

    pub fn revoke_local(&mut self, domain: &str) -> DurableResult<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        lock_audit_fence(&mut tx, domain)?;
        tx.query_one(
            "SELECT 1 FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let row = tx.query_one(
            "SELECT head_seq,rights_version FROM cmd2_domain WHERE domain=$1",
            &[&domain],
        )?;
        let seq = as_u64(row.get::<_, i64>(0))?
            .checked_add(1)
            .ok_or(DurableError::Corrupt("sequence overflow"))?;
        if seq > MAX_CUT {
            return Err(DurableError::Refused("shadow cut budget exceeded"));
        }
        let rights_version = as_u64(row.get::<_, i64>(1))?
            .checked_add(1)
            .ok_or(DurableError::Corrupt("rights version overflow"))?;
        tx.execute(
            "UPDATE cmd2_domain SET head_seq=$2,rights_version=$3,rights_allowed=false
             WHERE domain=$1",
            &[&domain, &as_i64(seq)?, &as_i64(rights_version)?],
        )?;
        let event_id = format!("rights.revoke:{seq}");
        let empty = Digest256::of_bytes(b"").to_hex();
        tx.execute(
            "INSERT INTO cmd2_log(domain,commit_seq,event_kind,command_id,delta_digest,members_root)
             VALUES($1,$2,'rights',$3,$4,$4)",
            &[&domain, &as_i64(seq)?, &event_id, &empty],
        )?;
        tx.execute(
            "INSERT INTO cmd2_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&domain, &as_i64(seq)?, &event_id],
        )?;
        tx.commit()?;
        Ok(seq)
    }

    pub fn head_seq(&mut self, domain: &str) -> DurableResult<u64> {
        let value: i64 = self
            .client
            .query_one(
                "SELECT head_seq FROM cmd2_domain WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        as_u64(value)
    }

    pub fn receipt_count(&mut self, domain: &str) -> DurableResult<i64> {
        Ok(self
            .client
            .query_one(
                "SELECT count(*) FROM cmd2_receipt WHERE domain=$1",
                &[&domain],
            )?
            .get(0))
    }

    pub fn history_count(&mut self, domain: &str) -> DurableResult<i64> {
        Ok(self
            .client
            .query_one(
                "SELECT count(*) FROM cmd2_history WHERE domain=$1",
                &[&domain],
            )?
            .get(0))
    }

    /// Full pin and segment verification is an explicit cold job, never an
    /// implicit per-query scan. The returned handle remains private here.
    pub fn cold_recover_exact(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        subject: &str,
        revision: u64,
    ) -> DurableResult<ColdRecoveredMember> {
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let mut metadata_tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        metadata_tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '60s'; SET LOCAL work_mem = '4MB'")?;
        let row = metadata_tx
            .query_opt(
                "SELECT * FROM cmd2_history
                 WHERE domain=$1 AND subject=$2 AND revision=$3",
                &[&domain, &subject, &as_i64(revision)?],
            )?
            .ok_or(DurableError::Corrupt("committed historical locator absent"))?;
        let pin: Vec<u8> = row.get("pin_id");
        if pin.len() != 16 {
            return Err(DurableError::Corrupt("historical pin ID malformed"));
        }
        let mut pin_id = [0u8; 16];
        pin_id.copy_from_slice(&pin);
        let prepare_id: Vec<u8> = row.get("prepare_id");
        let attempt = metadata_tx
            .query_opt(
                "SELECT state,attempt_fence,commit_seq FROM cmd2_attempt
                 WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?
            .ok_or(DurableError::Corrupt("historical attempt absent"))?;
        if attempt.get::<_, String>(0) != "committed"
            || attempt.get::<_, Option<i64>>(2) != Some(row.get("commit_seq"))
        {
            return Err(DurableError::Corrupt("historical attempt not committed"));
        }
        let attempt_fence = as_u64(attempt.get::<_, i64>(1))?;
        metadata_tx.commit()?;
        let receipts = match store.recover_attempt_fenced(&prepare_id, attempt_fence, 0)? {
            Some(AttemptRecovery::Sealed { receipts }) => receipts,
            _ => return Err(DurableError::Corrupt("historical fenced intent not sealed")),
        };
        if receipts.iter().any(|receipt| receipt.pin_id() != pin_id) {
            return Err(DurableError::Corrupt("historical fenced pin differs"));
        }
        let expected_id: String = row.get("sto_receipt_id");
        let receipt = receipts
            .into_iter()
            .find(|receipt| receipt.receipt_id().to_hex() == expected_id)
            .ok_or(DurableError::Corrupt("historical frame receipt absent"))?;
        check_history_locator(&row, &receipt, domain, subject, revision)?;
        Ok(ColdRecoveredMember {
            domain: domain.to_owned(),
            subject: subject.to_owned(),
            revision,
            receipt,
        })
    }

    /// Current local rights stay locked through selected frame verification
    /// and staging. A real external owner needs its own equivalent read fence.
    pub fn warm_read_selected(
        &mut self,
        store: &SegmentStore,
        recovered: &ColdRecoveredMember,
        max_bytes: u64,
    ) -> DurableResult<Vec<u8>> {
        if store.custody_domain() != recovered.domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let allowed: bool = tx
            .query_one(
                "SELECT rights_allowed FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&recovered.domain],
            )?
            .get(0);
        if !allowed {
            return Err(DurableError::Refused("current rights revoked"));
        }
        let row = tx
            .query_opt(
                "SELECT * FROM cmd2_history
                 WHERE domain=$1 AND subject=$2 AND revision=$3",
                &[
                    &recovered.domain,
                    &recovered.subject,
                    &as_i64(recovered.revision)?,
                ],
            )?
            .ok_or(DurableError::Corrupt("selected historical locator absent"))?;
        check_history_locator(
            &row,
            &recovered.receipt,
            &recovered.domain,
            &recovered.subject,
            recovered.revision,
        )?;
        let mut bytes = Vec::new();
        store.read_selected(&recovered.receipt, max_bytes, &mut bytes)?;
        tx.commit()?;
        Ok(bytes)
    }

    /// Offline/full cold job: one MVCC metadata cut, contiguous log and
    /// receipt audit, plus every referenced sealed pin and exact segment.
    /// This lab digest is not a source membership/index completeness seal.
    pub fn cold_verify_cut(
        &mut self,
        store: &SegmentStore,
        domain: &str,
    ) -> DurableResult<ColdCut> {
        self.cold_verify_cut_with_budget(store, domain, None)
    }

    fn cold_verify_cut_with_budget(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        requested: Option<(Instant, &AtomicBool)>,
    ) -> DurableResult<ColdCut> {
        self.cold_verify_cut_inner(store, domain, requested, None)
    }

    /// Explicit disk-backed complete-cut profile. The caller owns workspace
    /// admission and supplies its resource ceilings before any scratch write.
    /// The former finite in-memory API stays unchanged for compatibility.
    pub fn cold_verify_cut_streamed(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        workspace: &PrivateGenerationWorkspace,
        profile: StreamedGenerationProfile,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ColdCut> {
        let profile = profile.validate(workspace)?;
        self.cold_verify_cut_inner(
            store,
            domain,
            Some((deadline, cancelled)),
            Some((workspace, profile)),
        )
    }

    fn cold_verify_cut_inner(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        requested: Option<(Instant, &AtomicBool)>,
        streamed: Option<(&PrivateGenerationWorkspace, StreamedGenerationProfile)>,
    ) -> DurableResult<ColdCut> {
        let profile = streamed.map(|(_, profile)| profile);
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let audited_root = store.hold_audit_root()?;
        let started = Instant::now();
        check_cold_profile_deadline(started, requested, profile)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        // PostgreSQL remains a bounded participant: its per-statement work
        // memory and temp files are distinct from the explicit Rust workspace.
        tx.batch_execute("SET LOCAL statement_timeout = '60s'; SET LOCAL work_mem = '4MB'")?;
        if let Some(profile) = profile {
            if tx
                .query_one("SHOW server_encoding", &[])?
                .get::<_, String>(0)
                != "UTF8"
            {
                return Err(DurableError::Refused(
                    "streamed membership requires PostgreSQL UTF8",
                ));
            }
            let temp_kb = profile.max_pg_temp_bytes / 1024;
            tx.query_one(
                "SELECT set_config('temp_file_limit',$1,true)",
                &[&format!("{temp_kb}kB")],
            )?;
            tx.query_one(
                "SELECT set_config('statement_timeout',$1,true)",
                &[&format!("{}ms", profile.max_sql_statement_ms)],
            )?;
        }
        let domain_row = tx.query_one(
            "SELECT d.head_seq,d.schema_profile_digest,f.generation,f.maintenance_state
                 FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain)
                 WHERE d.domain=$1",
            &[&domain],
        )?;
        if domain_row.get::<_, String>(3) != "normal" {
            return Err(DurableError::Refused("physical maintenance is active"));
        }
        let profile_digest = schema_profile_digest();
        if domain_row.get::<_, Option<String>>(1) != Some(profile_digest.to_hex()) {
            return Err(DurableError::Conflict("coordinator schema profile changed"));
        }
        let head = as_u64(domain_row.get::<_, i64>(0))?;
        let audit_generation = as_u64(domain_row.get::<_, i64>(2))?;
        let database_oid = database_oid(&mut tx)?;
        if head > profile.map_or(MAX_CUT, |p| p.max_commit_seq) {
            return Err(DurableError::Refused("cold cut exceeds laboratory budget"));
        }
        admit_private_metadata(&mut tx, domain, started, requested, profile)?;
        let mut recovered_pins = 0usize;
        let mut segment_bytes = 0u64;
        let mut historical_members = 0u64;
        let mut latest: HashMap<String, (u64, Digest256, tos_segment_store::PlacementV1)> =
            HashMap::new();
        let mut latest_subject_bytes = 0usize;
        let mut membership_key_bytes = 0usize;
        let mut history_rows = Vec::new();
        let mut history_run = streamed.map(|(workspace, _)| workspace.collector());
        let mut log_hasher = Digest256Hasher::new();
        part(&mut log_hasher, b"cmd2-cold-cut-v1");
        let mut state_hasher = Digest256Hasher::new();
        part(&mut state_hasher, COLD_AUDIT_PROFILE);
        part(&mut state_hasher, domain.as_bytes());
        part(&mut state_hasher, &head.to_be_bytes());
        part(&mut state_hasher, &database_oid.to_be_bytes());
        part(&mut state_hasher, profile_digest.to_hex().as_bytes());
        let mut command_events = 0u64;
        let mut next_log_seq = 1u64;
        loop {
            check_cold_profile_deadline(started, requested, profile)?;
            // Keyset pagination keeps the ordered log bounded in process
            // memory while the same REPEATABLE READ snapshot holds throughout
            // all linked history/receipt/outbox checks.
            let log_rows = tx.query(
                "SELECT commit_seq,event_kind,command_id,delta_digest,members_root
                 FROM cmd2_log WHERE domain=$1 AND commit_seq >= $2 AND commit_seq <= $3
                 ORDER BY commit_seq LIMIT 128",
                &[&domain, &as_i64(next_log_seq)?, &as_i64(head)?],
            )?;
            if log_rows.is_empty() {
                break;
            }
            for log in &log_rows {
                check_cold_profile_deadline(started, requested, profile)?;
                let seq: i64 = log.get(0);
                if seq != as_i64(next_log_seq)? {
                    return Err(DurableError::Corrupt("cold cut sequence gap"));
                }
                next_log_seq = next_log_seq
                    .checked_add(1)
                    .ok_or(DurableError::Corrupt("cold sequence overflow"))?;
                let kind: String = log.get(1);
                let command_id: String = log.get(2);
                let delta_digest: String = log.get(3);
                let members_root: String = log.get(4);
                for bytes in [
                    &seq.to_be_bytes()[..],
                    kind.as_bytes(),
                    command_id.as_bytes(),
                    delta_digest.as_bytes(),
                    members_root.as_bytes(),
                ] {
                    part(&mut log_hasher, bytes);
                }
                let history = tx.query(
                    "SELECT * FROM cmd2_history WHERE domain=$1 AND commit_seq=$2
                 ORDER BY member_slot LIMIT 65",
                    &[&domain, &seq],
                )?;
                let outbox = tx.query_opt(
                    "SELECT event_id FROM cmd2_outbox WHERE domain=$1 AND commit_seq=$2",
                    &[&domain, &seq],
                )?;
                let expected_event_id = if kind == "command" {
                    format!("{domain}:{seq}")
                } else {
                    command_id.clone()
                };
                if outbox.map(|row| row.get::<_, String>(0)) != Some(expected_event_id) {
                    return Err(DurableError::Corrupt("cold cut outbox event differs"));
                }
                if kind != "command" {
                    if !history.is_empty() {
                        return Err(DurableError::Corrupt("authority event has source history"));
                    }
                    continue;
                }
                command_events += 1;
                let attempt = tx
                    .query_opt(
                        "SELECT * FROM cmd2_attempt WHERE domain=$1 AND command_id=$2",
                        &[&domain, &command_id],
                    )?
                    .ok_or(DurableError::Corrupt("command event has no attempt"))?;
                if attempt.get::<_, String>("state") != "committed"
                    || attempt.get::<_, Option<i64>>("commit_seq") != Some(seq)
                {
                    return Err(DurableError::Corrupt("command event attempt not committed"));
                }
                let receipt = receipt_from_committed_attempt(&mut tx, &attempt)?;
                if receipt.delta_digest.to_hex() != delta_digest
                    || receipt.member_root.to_hex() != members_root
                {
                    return Err(DurableError::Corrupt("command event receipt differs"));
                }
                let members = tx.query(
                    "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2
                 ORDER BY member_slot LIMIT 65",
                    &[&domain, &receipt.prepare_id],
                )?;
                if history.len() != members.len()
                    || members.is_empty()
                    || members.len() > MAX_MEMBERS
                {
                    return Err(DurableError::Corrupt("cold cut member count differs"));
                }
                if recovered_pins >= profile.map_or(10_000, |p| p.max_pins) {
                    return Err(DurableError::Refused("cold pin budget exceeded"));
                }
                let attempt_fence = as_u64(attempt.get::<_, i64>("attempt_fence"))?;
                let sealed =
                    match store.recover_attempt_fenced(&receipt.prepare_id, attempt_fence, 0)? {
                        Some(AttemptRecovery::Sealed { receipts }) => receipts,
                        _ => return Err(DurableError::Corrupt("cold fenced intent not sealed")),
                    };
                let first = sealed
                    .first()
                    .ok_or(DurableError::Corrupt("sealed attempt has no frames"))?;
                if sealed
                    .iter()
                    .any(|candidate| candidate.pin_id() != first.pin_id())
                {
                    return Err(DurableError::Corrupt("compound fenced pin differs"));
                }
                recovered_pins += 1;
                segment_bytes = segment_bytes
                    .checked_add(first.segment_size())
                    .ok_or(DurableError::Refused("cold byte budget overflow"))?;
                if segment_bytes > profile.map_or(256 * 1024 * 1024, |p| p.max_segment_bytes) {
                    return Err(DurableError::Refused("cold byte budget exceeded"));
                }
                let mut member_hasher = Digest256Hasher::new();
                part(&mut member_hasher, b"cmd2-member-root-v1");
                part(&mut member_hasher, &(members.len() as u64).to_be_bytes());
                for (member, historical) in members.iter().zip(history.iter()) {
                    check_cold_profile_deadline(started, requested, profile)?;
                    let slot: i32 = member.get("member_slot");
                    if historical.get::<_, i32>("member_slot") != slot
                        || historical.get::<_, Vec<u8>>("prepare_id") != receipt.prepare_id
                        || historical.get::<_, String>("subject")
                            != member.get::<_, String>("subject")
                        || historical.get::<_, i64>("revision")
                            != member.get::<_, i64>("proposed_revision")
                    {
                        return Err(DurableError::Corrupt(
                            "cold history/member identity differs",
                        ));
                    }
                    if member.get::<_, Option<i32>>("source_mode")
                        != historical.get::<_, Option<i32>>("source_mode")
                        || member.get::<_, Option<Vec<String>>>("source_dependencies")
                            != historical.get::<_, Option<Vec<String>>>("source_dependencies")
                    {
                        return Err(DurableError::Corrupt(
                            "cold source carrier metadata differs from attached member",
                        ));
                    }
                    update_member_root(&mut member_hasher, member);
                    let pin: Vec<u8> = historical.get("pin_id");
                    if pin.len() != 16 {
                        return Err(DurableError::Corrupt("cold history pin malformed"));
                    }
                    let mut pin_id = [0u8; 16];
                    pin_id.copy_from_slice(&pin);
                    if pin_id != first.pin_id() {
                        return Err(DurableError::Corrupt("cold history fenced pin differs"));
                    }
                    let selected = sealed
                        .iter()
                        .find(|candidate| {
                            candidate.receipt_id().to_hex()
                                == historical.get::<_, String>("sto_receipt_id")
                        })
                        .ok_or(DurableError::Corrupt("cold committed frame absent"))?;
                    check_history_locator(
                        historical,
                        selected,
                        domain,
                        &historical.get::<_, String>("subject"),
                        as_u64(historical.get("revision"))?,
                    )?;
                    check_member_row(
                        member,
                        selected,
                        &member.get::<_, String>("subject"),
                        as_u64(member.get("proposed_revision"))?,
                    )?;
                    // The private STO handle was recovered from the actual sealed
                    // pin and full segment. Bind its physical placement as well
                    // as the corresponding coordinator row into this cut.
                    part(&mut state_hasher, &selected.placement().encode());
                    let subject: String = historical.get("subject");
                    let revision = as_u64(historical.get("revision"))?;
                    let commitment = metadata_locator_digest(historical);
                    let coordinate = selected.coordinate();
                    let key = membership_key(HISTORY_KEY_TAG, domain, &subject, Some(revision))?;
                    membership_key_bytes = membership_key_bytes
                        .checked_add(key.len())
                        .ok_or(DurableError::Refused("membership key byte count overflow"))?;
                    if membership_key_bytes as u64
                        > profile.map_or(MAX_TOTAL_MEMBERSHIP_KEY_BYTES as u64, |p| {
                            p.max_membership_key_bytes
                        })
                        || historical_members >= profile.map_or(MAX_CUT, |p| p.max_members)
                    {
                        return Err(DurableError::Refused("history membership budget exceeded"));
                    }
                    let generation_row = PlacementGenerationRowV1 {
                        key,
                        logical_digest: coordinate.sha256,
                        logical_length: coordinate.size_bytes,
                        placement: selected.placement(),
                    };
                    if let Some(run) = &mut history_run {
                        run.push(generation_row)?;
                    } else {
                        history_rows.push(generation_row);
                        if latest
                            .get(&subject)
                            .is_some_and(|(previous, _, _)| *previous >= revision)
                        {
                            return Err(DurableError::Corrupt(
                                "historical revision is not monotonic",
                            ));
                        }
                        if !latest.contains_key(&subject) {
                            latest_subject_bytes = latest_subject_bytes
                                .checked_add(subject.len())
                                .ok_or(DurableError::Refused("subject byte count overflow"))?;
                            if latest_subject_bytes > 16 * 1024 * 1024
                                || latest.len() >= MAX_CUT as usize
                            {
                                return Err(DurableError::Refused(
                                    "current identity budget exceeded",
                                ));
                            }
                        }
                        latest.insert(subject, (revision, commitment, selected.placement()));
                    }
                    historical_members += 1;
                }
                if member_hasher.finalize() != receipt.member_root {
                    return Err(DurableError::Corrupt("cold command member root differs"));
                }
            }
        }
        if next_log_seq != head + 1 {
            return Err(DurableError::Corrupt("cold cut log has a gap"));
        }
        let future_history: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_history WHERE domain=$1 AND commit_seq > $2",
                &[&domain, &as_i64(head)?],
            )?
            .get(0);
        if future_history != 0 {
            return Err(DurableError::Corrupt(
                "history extends beyond committed head",
            ));
        }
        let mut current_members = 0u64;
        let mut current_rows = Vec::new();
        let (history_spool, current_spool, history_membership_root, current_membership_root) =
            if let Some((workspace, profile)) = streamed {
                // Without the finite path's latest-subject map, re-read the
                // complete history through its existing (domain, subject,
                // revision) primary key. Commit order must advance for every
                // subject before the largest revision can be called latest.
                let mut ordered = tx.query_raw(
                    "SELECT subject,revision,commit_seq FROM cmd2_history
                     WHERE domain=$1 ORDER BY subject COLLATE \"C\",revision",
                    &[&domain],
                )?;
                let mut previous_subject: Option<String> = None;
                let mut previous_revision = 0u64;
                let mut previous_seq = 0u64;
                let mut ordered_count = 0u64;
                while let Some(row) = ordered.next()? {
                    check_cold_profile_deadline(started, requested, Some(profile))?;
                    let subject: String = row.get(0);
                    let revision = as_u64(row.get::<_, i64>(1))?;
                    let seq = as_u64(row.get::<_, i64>(2))?;
                    if previous_subject.as_deref() == Some(subject.as_str()) {
                        if revision <= previous_revision || seq <= previous_seq {
                            return Err(DurableError::Corrupt(
                                "historical revision is not monotonic",
                            ));
                        }
                    } else {
                        previous_subject = Some(subject);
                    }
                    previous_revision = revision;
                    previous_seq = seq;
                    ordered_count += 1;
                    if ordered_count > historical_members {
                        return Err(DurableError::Corrupt("ordered history has extra member"));
                    }
                }
                drop(ordered);
                if ordered_count != historical_members {
                    return Err(DurableError::Corrupt("ordered history membership differs"));
                }
                // The primary-key predecessor seek uses one index lookup per
                // current subject. Composite JSON equality compares every
                // current/history column, including source-owner metadata.
                // The sorted physical stream below independently proves that
                // every historical subject occurs exactly once in current.
                if tx
                    .query_opt(
                        "SELECT 1 FROM cmd2_current c LEFT JOIN LATERAL
                       (SELECT * FROM cmd2_history h
                        WHERE h.domain=c.domain AND h.subject=c.subject
                        ORDER BY h.revision DESC LIMIT 1) h ON true
                     WHERE c.domain=$1 AND
                       (h.subject IS NULL OR to_jsonb(c) IS DISTINCT FROM to_jsonb(h)) LIMIT 1",
                        &[&domain],
                    )?
                    .is_some()
                {
                    return Err(DurableError::Corrupt(
                        "current locator differs from latest history",
                    ));
                }
                current_members = as_u64(
                    tx.query_one(
                        "SELECT count(*) FROM cmd2_current WHERE domain=$1",
                        &[&domain],
                    )?
                    .get::<_, i64>(0),
                )?;
                if current_members > profile.max_members {
                    return Err(DurableError::Refused("current member budget exceeded"));
                }
                let (deadline, cancelled) =
                    requested.ok_or(DurableError::Refused("streamed cut requires deadline"))?;
                let deadline = started
                    .checked_add(profile.max_elapsed)
                    .map_or(deadline, |limit| deadline.min(limit));
                let history = history_run
                    .take()
                    .expect("streamed collector")
                    .finish(deadline, cancelled)?;
                if history.count() != historical_members {
                    return Err(DurableError::Corrupt("history workspace count differs"));
                }
                let (current, history_root, current_root) = streamed_membership_roots(
                    domain,
                    &history,
                    workspace,
                    current_members,
                    deadline,
                    cancelled,
                )?;
                (Some(history), Some(current), history_root, current_root)
            } else {
                let mut current =
                    tx.query_raw("SELECT * FROM cmd2_current WHERE domain=$1", &[&domain])?;
                while let Some(row) = current.next()? {
                    check_cold_profile_deadline(started, requested, profile)?;
                    current_members = current_members
                        .checked_add(1)
                        .ok_or(DurableError::Refused("current member count overflow"))?;
                    if current_members > MAX_CUT {
                        return Err(DurableError::Refused("current member budget exceeded"));
                    }
                    let subject: String = row.get("subject");
                    let revision = as_u64(row.get("revision"))?;
                    let Some((latest_revision, latest_digest, placement)) = latest.get(&subject)
                    else {
                        return Err(DurableError::Corrupt("current member has no history"));
                    };
                    if *latest_revision != revision
                        || *latest_digest != metadata_locator_digest(&row)
                    {
                        return Err(DurableError::Corrupt(
                            "current locator differs from latest history",
                        ));
                    }
                    let key = membership_key(CURRENT_KEY_TAG, domain, &subject, None)?;
                    membership_key_bytes = membership_key_bytes
                        .checked_add(key.len())
                        .ok_or(DurableError::Refused("membership key byte count overflow"))?;
                    if membership_key_bytes > MAX_TOTAL_MEMBERSHIP_KEY_BYTES {
                        return Err(DurableError::Refused("current membership budget exceeded"));
                    }
                    current_rows.push(PlacementGenerationRowV1 {
                        key,
                        logical_digest: placement.coordinate().sha256,
                        logical_length: placement.coordinate().size_bytes,
                        placement: *placement,
                    });
                }
                drop(current);
                if current_members != latest.len() as u64
                    || history_rows.len() as u64 != historical_members
                    || current_rows.len() as u64 != current_members
                {
                    return Err(DurableError::Corrupt(
                        "membership stream cardinality differs",
                    ));
                }
                sort_complete_membership(&mut history_rows)?;
                sort_complete_membership(&mut current_rows)?;
                let history_root = logical_membership_root(HISTORY_KEY_TAG, &history_rows);
                let current_root = logical_membership_root(CURRENT_KEY_TAG, &current_rows);
                (None, None, history_root, current_root)
            };
        let receipt_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_receipt WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        let outbox_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_outbox WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        let history_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_history WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        if as_u64(receipt_count)? != command_events
            || as_u64(outbox_count)? != head
            || as_u64(history_count)? != historical_members
        {
            return Err(DurableError::Corrupt("cold cut table membership differs"));
        }
        // This is deliberately an offline metadata scan. The publication
        // transaction later compares only its trigger-maintained generation;
        // no full scan or segment hashing occurs under the sequencer lock.
        // The row encoding is a PostgreSQL-16 laboratory profile, bound by
        // schema_profile_digest and database_oid, not a portable source codec.
        append_private_metadata(
            &mut tx,
            domain,
            &mut state_hasher,
            started,
            requested,
            profile,
        )?;
        let cut = ColdCut {
            audited_root,
            domain: domain.to_owned(),
            through_commit_seq: head,
            log_digest: log_hasher.finalize(),
            state_digest: state_hasher.finalize(),
            schema_profile_digest: profile_digest,
            database_oid,
            audit_generation,
            historical_members,
            current_members,
            history_membership_root,
            current_membership_root,
            history_rows,
            current_rows,
            history_spool,
            current_spool,
            streamed_profile: profile,
        };
        tx.commit()?;
        Ok(cut)
    }

    /// Build the first bounded complete private history/current generation.
    /// The `ColdCut` is an opaque result of our exhaustive PG/STO cold audit;
    /// STO installation verifies physical shape, and both exact row streams
    /// are compared to that independently audited result through EOF.
    pub fn build_complete_generation(
        &mut self,
        store: &SegmentStore,
        cut: &ColdCut,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<CompleteGeneration> {
        let (installed, history_coverage, current_coverage) =
            install_verified_membership(store, cut.installation(store), deadline, cancelled)?;
        Ok(CompleteGeneration {
            cut: cut.clone(),
            installed,
            history_coverage,
            current_coverage,
        })
    }

    /// Compare physical complete streams against the independently verified cold facts.
    pub fn verify_generation_candidate(
        &mut self,
        store: &SegmentStore,
        cut: &ColdCut,
        installed: InstalledGenerationV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<CompleteGeneration> {
        let (history_coverage, current_coverage) = verify_installed_membership(
            store,
            cut.installation(store),
            &installed,
            deadline,
            cancelled,
        )?;
        Ok(CompleteGeneration {
            cut: cut.clone(),
            installed,
            history_coverage,
            current_coverage,
        })
    }

    pub fn seal_shadow_cut(&mut self, cut: &ColdCut) -> DurableResult<()> {
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        // The audit fence is the first metadata lock for every laboratory
        // writer. Its generation substitutes for a full scan under this
        // short publication transaction.
        let generation = lock_audit_fence(&mut tx, &cut.domain)?;
        let row = tx.query_one(
            "SELECT head_seq,published_seq,complete_cut_digest,complete_cut_generation,
                    schema_profile_digest FROM cmd2_domain
             WHERE domain=$1 FOR UPDATE",
            &[&cut.domain],
        )?;
        let head = as_u64(row.get::<_, i64>(0))?;
        let published = as_u64(row.get::<_, i64>(1))?;
        let published_digest: Option<String> = row.get(2);
        let published_generation: Option<i64> = row.get(3);
        if row.get::<_, Option<String>>(4) != Some(cut.schema_profile_digest.to_hex())
            || cut.schema_profile_digest != schema_profile_digest()
            || database_oid(&mut tx)? != cut.database_oid
        {
            return Err(DurableError::Conflict(
                "database or coordinator schema changed",
            ));
        }
        if head != cut.through_commit_seq {
            return Err(DurableError::Conflict("audited head advanced; re-audit"));
        }
        if published == cut.through_commit_seq && published_digest.is_some() {
            let sealed_generation = published_generation
                .ok_or(DurableError::Corrupt("published fence generation absent"))?;
            if published_digest != Some(cut.state_digest.to_hex())
                || generation != as_u64(sealed_generation)?
                || !(cut.audit_generation == generation
                    || cut.audit_generation.checked_add(1) == Some(generation))
            {
                return Err(DurableError::Conflict(
                    "published cut identity or fence differs",
                ));
            }
            tx.commit()?;
            return Ok(());
        }
        if published > cut.through_commit_seq || generation != cut.audit_generation {
            return Err(DurableError::Conflict(
                "audited metadata generation changed",
            ));
        }
        let sealed_generation = generation
            .checked_add(1)
            .ok_or(DurableError::Corrupt("audit generation overflow"))?;
        tx.execute(
            "UPDATE cmd2_domain SET published_seq=$2,complete_cut_digest=$3,
                    complete_cut_generation=$4,selected_generation_digest=NULL WHERE domain=$1",
            &[
                &cut.domain,
                &as_i64(cut.through_commit_seq)?,
                &cut.state_digest.to_hex(),
                &as_i64(sealed_generation)?,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Select the exact installed descriptor in the same short publication
    /// update as the complete private cut. No history, leaf, segment or
    /// metadata scan occurs while the audit fence is locked.
    pub fn select_complete_generation(
        &mut self,
        candidate: &CompleteGeneration,
    ) -> DurableResult<()> {
        let cut = &candidate.cut;
        cut.audited_root.require_installed(&candidate.installed)?;
        let digest = candidate.installed.digest();
        if candidate.history_coverage.descriptor_digest != digest
            || candidate.current_coverage.descriptor_digest != digest
            || candidate.history_coverage.rows != cut.historical_members
            || candidate.current_coverage.rows != cut.current_members
            || candidate.installed.descriptor().cut.state_digest != cut.state_digest
            || candidate.installed.descriptor().cut.history_membership_root
                != cut.history_membership_root
            || candidate.installed.descriptor().cut.current_membership_root
                != cut.current_membership_root
        {
            return Err(DurableError::Corrupt("complete generation binding differs"));
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let generation = lock_audit_fence(&mut tx, &cut.domain)?;
        let row = tx.query_one(
            "SELECT head_seq,published_seq,complete_cut_digest,complete_cut_generation,
                    selected_generation_digest,schema_profile_digest
             FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&cut.domain],
        )?;
        let head = as_u64(row.get::<_, i64>(0))?;
        let published = as_u64(row.get::<_, i64>(1))?;
        let published_digest: Option<String> = row.get(2);
        let published_generation: Option<i64> = row.get(3);
        let selected_digest: Option<String> = row.get(4);
        if row.get::<_, Option<String>>(5) != Some(cut.schema_profile_digest.to_hex())
            || cut.schema_profile_digest != schema_profile_digest()
            || database_oid(&mut tx)? != cut.database_oid
            || head != cut.through_commit_seq
        {
            return Err(DurableError::Conflict("audited database or head changed"));
        }
        if published == cut.through_commit_seq && selected_digest.is_some() {
            let sealed_generation = published_generation
                .ok_or(DurableError::Corrupt("selected fence generation absent"))?;
            if selected_digest != Some(digest.to_hex())
                || published_digest != Some(cut.state_digest.to_hex())
                || generation != as_u64(sealed_generation)?
                || !(cut.audit_generation == generation
                    || cut.audit_generation.checked_add(1) == Some(generation))
            {
                return Err(DurableError::Conflict(
                    "selected generation identity differs",
                ));
            }
            tx.commit()?;
            return Ok(());
        }
        if published > cut.through_commit_seq || generation != cut.audit_generation {
            return Err(DurableError::Conflict(
                "audited metadata generation changed",
            ));
        }
        if published == cut.through_commit_seq
            && published_digest.is_some()
            && published_digest != Some(cut.state_digest.to_hex())
        {
            return Err(DurableError::Conflict("prior bare cut digest differs"));
        }
        let sealed_generation = generation
            .checked_add(1)
            .ok_or(DurableError::Corrupt("audit generation overflow"))?;
        tx.execute(
            "UPDATE cmd2_domain SET published_seq=$2,complete_cut_digest=$3,
                    complete_cut_generation=$4,selected_generation_digest=$5 WHERE domain=$1",
            &[
                &cut.domain,
                &as_i64(cut.through_commit_seq)?,
                &cut.state_digest.to_hex(),
                &as_i64(sealed_generation)?,
                &digest.to_hex(),
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Cold restore/open of the *selected* private descriptor. A standalone
    /// STO digest never authorizes this read: the exact pointer and fence are
    /// read from PostgreSQL and matched to a fresh complete cold audit.
    pub fn cold_open_selected_generation(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<VerifiedSelectedGeneration> {
        self.cold_open_selected_generation_inner(store, domain, None, deadline, cancelled)
    }

    pub fn cold_open_selected_generation_streamed(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        workspace: &PrivateGenerationWorkspace,
        profile: StreamedGenerationProfile,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<VerifiedSelectedGeneration> {
        let profile = profile.validate(workspace)?;
        self.cold_open_selected_generation_inner(
            store,
            domain,
            Some((workspace, profile)),
            deadline,
            cancelled,
        )
    }

    fn cold_open_selected_generation_inner(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        streamed: Option<(&PrivateGenerationWorkspace, StreamedGenerationProfile)>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<VerifiedSelectedGeneration> {
        let cut = match streamed {
            Some((workspace, profile)) => self
                .cold_verify_cut_streamed(store, domain, workspace, profile, deadline, cancelled)?,
            None => self.cold_verify_cut_with_budget(store, domain, Some((deadline, cancelled)))?,
        };
        let profile = cut.streamed_profile;
        let limits = profile.map_or_else(generation_limits, |p| p.generation);
        check_cold_profile_deadline(Instant::now(), Some((deadline, cancelled)), profile)?;
        let row = self.client.query_one(
            "SELECT d.head_seq,d.published_seq,d.complete_cut_digest,
                    d.complete_cut_generation,d.selected_generation_digest,
                    f.generation,f.maintenance_state
             FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain)
             WHERE d.domain=$1",
            &[&domain],
        )?;
        let selected_digest: Option<String> = row.get(4);
        let selected_digest =
            selected_digest.ok_or(DurableError::Refused("complete generation not selected"))?;
        let sealed_generation = as_u64(
            row.get::<_, Option<i64>>(3)
                .ok_or(DurableError::Corrupt("selected fence generation absent"))?,
        )?;
        if row.get::<_, String>(6) != "normal"
            || as_u64(row.get::<_, i64>(0))? != cut.through_commit_seq
            || as_u64(row.get::<_, i64>(1))? != cut.through_commit_seq
            || row.get::<_, Option<String>>(2) != Some(cut.state_digest.to_hex())
            || as_u64(row.get::<_, i64>(5))? != cut.audit_generation
            || sealed_generation != cut.audit_generation
        {
            return Err(DurableError::Conflict(
                "selected cut changed after cold audit",
            ));
        }
        let mut expected_cut = generation_cut(store, &cut);
        expected_cut.audit_generation = sealed_generation
            .checked_sub(1)
            .ok_or(DurableError::Corrupt("selected generation fence invalid"))?;
        check_cold_profile_deadline(Instant::now(), Some((deadline, cancelled)), profile)?;
        let installed =
            store.open_generation_candidate(parse_hex(selected_digest)?, &expected_cut, limits)?;
        cut.audited_root.require_installed(&installed)?;
        check_cold_profile_deadline(Instant::now(), Some((deadline, cancelled)), profile)?;
        if installed.descriptor().history.key_codec_digest != Digest256::of_bytes(HISTORY_KEY_CODEC)
            || installed.descriptor().current.key_codec_digest
                != Digest256::of_bytes(CURRENT_KEY_CODEC)
        {
            return Err(DurableError::Conflict("selected key codec differs"));
        }
        let facts = cut.installation(store);
        let history_coverage = compare_installed_membership(
            &installed,
            GenerationNamespaceV1::History,
            facts.cursor(GenerationNamespaceV1::History)?,
            cut.historical_members,
            limits,
            deadline,
            cancelled,
        )?;
        let current_coverage = compare_installed_membership(
            &installed,
            GenerationNamespaceV1::Current,
            facts.cursor(GenerationNamespaceV1::Current)?,
            cut.current_members,
            limits,
            deadline,
            cancelled,
        )?;
        Ok(VerifiedSelectedGeneration {
            cut,
            installed,
            history_coverage,
            current_coverage,
        })
    }

    pub fn published_seq(&mut self, domain: &str) -> DurableResult<u64> {
        let value: i64 = self
            .client
            .query_one(
                "SELECT published_seq FROM cmd2_domain WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        as_u64(value)
    }
}

fn verification_budget(member_count: usize) -> VerificationBudget {
    VerificationBudget {
        max_receipts: member_count,
        max_segments: 1,
        max_total_segment_bytes: 64 * 1024 * 1024,
    }
}

fn generation_limits() -> GenerationReadLimits {
    GenerationReadLimits {
        // Both namespaces repeat their first/last keys in the descriptor.
        // Four 4096-byte endpoints require more than 16 KiB.
        max_descriptor_bytes: 32 * 1024,
        shape: GenerationShapeLimits {
            max_partitions: 2,
            max_rows_per_partition: MAX_CUT,
            max_key_bytes: MAX_MEMBERSHIP_KEY_BYTES,
            max_leaf_bytes: 64 * 1024 * 1024,
        },
        max_stream_rows: MAX_CUT,
        max_stream_key_bytes: MAX_TOTAL_MEMBERSHIP_KEY_BYTES as u64,
    }
}

fn generation_cut(store: &SegmentStore, cut: &ColdCut) -> GenerationCutV1 {
    cut.installation(store).descriptor_cut
}

fn install_verified_membership(
    store: &SegmentStore,
    facts: MembershipInstallation<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(
    InstalledGenerationV1,
    GenerationCoverageV1,
    GenerationCoverageV1,
)> {
    facts.audited_root.require_store(store)?;
    if store.custody_domain() != facts.domain.as_bytes() {
        return Err(DurableError::Conflict("STO custody domain differs"));
    }
    check_cold_deadline(Instant::now(), Some((deadline, cancelled)))?;
    let cut = &facts.descriptor_cut;
    if cut.schema_profile_digest != schema_profile_digest()
        || cut.historical_members != facts.count(GenerationNamespaceV1::History)
        || cut.current_members != facts.count(GenerationNamespaceV1::Current)
        || cut.history_membership_root
            != root_from_cursor(
                HISTORY_KEY_TAG,
                cut.historical_members,
                facts.cursor(GenerationNamespaceV1::History)?,
                deadline,
                cancelled,
            )?
        || cut.current_membership_root
            != root_from_cursor(
                CURRENT_KEY_TAG,
                cut.current_members,
                facts.cursor(GenerationNamespaceV1::Current)?,
                deadline,
                cancelled,
            )?
    {
        return Err(DurableError::Corrupt(
            "verified membership certificate differs",
        ));
    }
    let limits = facts
        .profile
        .map_or_else(generation_limits, |p| p.generation);
    let rows_per_leaf = facts.profile.map_or(MAX_CUT as usize, |p| p.rows_per_leaf);
    let history = install_membership_catalog(
        store,
        HISTORY_NAMESPACE,
        HISTORY_KEY_CODEC,
        facts.cursor(GenerationNamespaceV1::History)?,
        cut.historical_members,
        rows_per_leaf,
        limits,
        deadline,
        cancelled,
    )?;
    check_cold_deadline(Instant::now(), Some((deadline, cancelled)))?;
    let current = install_membership_catalog(
        store,
        CURRENT_NAMESPACE,
        CURRENT_KEY_CODEC,
        facts.cursor(GenerationNamespaceV1::Current)?,
        cut.current_members,
        rows_per_leaf,
        limits,
        deadline,
        cancelled,
    )?;
    let descriptor = GenerationDescriptorV1 {
        cut: facts.descriptor_cut.clone(),
        history,
        current,
    };
    let installed = store.install_generation_candidate(descriptor, limits)?;
    let (history_coverage, current_coverage) =
        verify_installed_membership(store, facts, &installed, deadline, cancelled)?;
    Ok((installed, history_coverage, current_coverage))
}
fn verify_installed_membership(
    store: &SegmentStore,
    facts: MembershipInstallation<'_>,
    installed: &InstalledGenerationV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(GenerationCoverageV1, GenerationCoverageV1)> {
    facts.audited_root.require_store(store)?;
    facts.audited_root.require_installed(installed)?;
    if store.custody_domain() != facts.domain.as_bytes()
        || installed.descriptor().cut != facts.descriptor_cut
        || installed.descriptor().history.key_codec_digest != Digest256::of_bytes(HISTORY_KEY_CODEC)
        || installed.descriptor().current.key_codec_digest != Digest256::of_bytes(CURRENT_KEY_CODEC)
    {
        return Err(DurableError::Conflict("generation candidate cut differs"));
    }
    let limits = facts
        .profile
        .map_or_else(generation_limits, |p| p.generation);
    let history = compare_installed_membership(
        installed,
        GenerationNamespaceV1::History,
        facts.cursor(GenerationNamespaceV1::History)?,
        facts.count(GenerationNamespaceV1::History),
        limits,
        deadline,
        cancelled,
    )?;
    let current = compare_installed_membership(
        installed,
        GenerationNamespaceV1::Current,
        facts.cursor(GenerationNamespaceV1::Current)?,
        facts.count(GenerationNamespaceV1::Current),
        limits,
        deadline,
        cancelled,
    )?;
    Ok((history, current))
}

fn install_membership_catalog(
    store: &SegmentStore,
    namespace: &[u8],
    codec: &[u8],
    mut rows: MembershipCursor<'_>,
    expected_count: u64,
    rows_per_leaf: usize,
    limits: GenerationReadLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<GenerationCatalogV1> {
    if rows_per_leaf == 0 || rows_per_leaf as u64 > limits.shape.max_rows_per_partition {
        return Err(DurableError::Refused("invalid generation leaf row budget"));
    }
    let needed = expected_count.saturating_add(rows_per_leaf as u64 - 1) / rows_per_leaf as u64;
    if needed.max(1) > limits.shape.max_partitions as u64 {
        return Err(DurableError::Refused(
            "generation partition budget exceeded",
        ));
    }
    let mut partitions = Vec::with_capacity(needed.max(1) as usize);
    let mut lower = None;
    let mut next = rows.next_row()?;
    let mut observed = 0u64;
    loop {
        let mut chunk = Vec::new();
        while chunk.len() < rows_per_leaf {
            let Some(row) = next.take() else {
                break;
            };
            if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return Err(DurableError::Refused("generation build deadline exceeded"));
            }
            observed = observed
                .checked_add(1)
                .ok_or(DurableError::Refused("generation row count overflow"))?;
            if observed > expected_count {
                return Err(DurableError::Corrupt("generation has extra source row"));
            }
            chunk.push(row);
            next = rows.next_row()?;
        }
        let upper = next.as_ref().map(|row| row.key.clone());
        let bounds = PartitionBoundsV1 {
            lower_inclusive: lower.clone(),
            upper_exclusive: upper.clone(),
        };
        let leaf = PackedPlacementLeafV1 {
            domain_digest: store.domain_digest(),
            bounds: bounds.clone(),
            rows: chunk,
        };
        let content_digest = store.install_packed_leaf(&leaf, limits.shape)?;
        let semantic = describe_placement_partition(
            store.domain_digest(),
            bounds,
            leaf.rows.iter().cloned().map(Ok),
            limits.shape,
        )?;
        partitions.push(PackedPartitionRefV1 {
            semantic,
            content_digest,
        });
        lower = upper;
        if next.is_none() {
            break;
        }
    }
    rows.finish()?;
    if observed != expected_count {
        return Err(DurableError::Corrupt("generation source EOF count differs"));
    }
    let key_codec_digest = Digest256::of_bytes(codec);
    let catalog_root = store.verify_packed_catalog_shape(
        store.custody_domain(),
        namespace,
        b"all",
        key_codec_digest,
        KeyComparatorV1::RawUnsignedBytes,
        &partitions,
        limits.shape,
    )?;
    Ok(GenerationCatalogV1 {
        key_codec_digest,
        catalog_root,
        partitions,
    })
}

fn compare_installed_membership(
    installed: &InstalledGenerationV1,
    namespace: GenerationNamespaceV1,
    mut expected: MembershipCursor<'_>,
    expected_count: u64,
    limits: GenerationReadLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<GenerationCoverageV1> {
    let mut stream = installed.stream(namespace, limits)?;
    let mut observed = 0u64;
    while let Some(row) = expected.next_row()? {
        if stream.next_row(deadline, cancelled)? != Some(row) {
            return Err(DurableError::Corrupt("installed membership row differs"));
        }
        observed += 1;
        if observed > expected_count {
            return Err(DurableError::Corrupt(
                "installed membership row count exceeded",
            ));
        }
    }
    expected.finish()?;
    if stream.next_row(deadline, cancelled)?.is_some() {
        return Err(DurableError::Corrupt("installed membership has extra row"));
    }
    let coverage = stream.coverage().ok_or(DurableError::Corrupt(
        "installed membership lacks EOF coverage",
    ))?;
    if coverage.rows != expected_count
        || observed != expected_count
        || coverage.descriptor_digest != installed.digest()
    {
        return Err(DurableError::Corrupt(
            "installed membership coverage differs",
        ));
    }
    Ok(coverage)
}

fn check_durable_member(
    domain: &str,
    prepare_id: &[u8],
    member: &DurableShadowMember,
    profile: &[u8],
) -> DurableResult<()> {
    let receipt = &member.receipt;
    let binding = receipt.binding();
    let coordinate = receipt.coordinate();
    if binding.profile_id != profile
        || binding.profile_version != PROFILE_VERSION
        || binding.subject_key != member.subject.as_bytes()
        || binding.member_slot != member.member_slot
        || receipt.prepare_id() != prepare_id
        || receipt.custody_domain() != domain.as_bytes()
        || coordinate.sha256 != Digest256::of_bytes(&member.exact_bytes)
        || coordinate.size_bytes != member.exact_bytes.len() as u64
    {
        return Err(DurableError::Invalid(
            "STO receipt and member binding differ",
        ));
    }
    if member.proposed_revision == 0
        || match member.expected_predecessor {
            None => member.proposed_revision != 1,
            Some((version, _)) => version.checked_add(1) != Some(member.proposed_revision),
        }
    {
        return Err(DurableError::Invalid(
            "owner revision is not exact successor",
        ));
    }
    if profile == PROFILE_ID {
        check_lab_record(
            &member.exact_bytes,
            &member.subject,
            member.proposed_revision,
        )
    } else {
        source_cohort::check_source_member(profile, member)
    }
}

fn check_member_row(
    row: &postgres::Row,
    receipt: &ByteDurabilityReceipt,
    subject: &str,
    revision: u64,
) -> DurableResult<()> {
    let coordinate = receipt.coordinate();
    if row.get::<_, String>("subject") != subject
        || !source_cohort::known_profile(&receipt.binding().profile_id)
        || receipt.binding().profile_version != PROFILE_VERSION
        || receipt.binding().subject_key != subject.as_bytes()
        || row.get::<_, i32>("member_slot") != receipt.binding().member_slot as i32
        || row.get::<_, Vec<u8>>("prepare_id") != receipt.prepare_id()
        || row.get::<_, i64>("proposed_revision") != as_i64(revision)?
        || row.get::<_, String>("profile_id").as_bytes() != receipt.binding().profile_id
        || row.get::<_, String>("profile_version") != "1"
        || row.get::<_, Vec<u8>>("store_id") != receipt.store_id()
        || row.get::<_, String>("custody_domain_digest") != receipt.domain_digest().to_hex()
        || row.get::<_, Vec<u8>>("custody_domain") != receipt.custody_domain()
        || row.get::<_, Vec<u8>>("pin_id") != receipt.pin_id()
        || row.get::<_, i64>("pin_fence") != as_i64(receipt.fence_epoch())?
        || row.get::<_, String>("segment_digest") != receipt.segment_digest().to_hex()
        || row.get::<_, i64>("segment_size") != as_i64(receipt.segment_size())?
        || row.get::<_, i32>("frame_index") != receipt.frame_index() as i32
        || row.get::<_, i64>("frame_header_offset") != as_i64(coordinate.header_offset)?
        || row.get::<_, String>("frame_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("frame_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("content_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("content_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("sto_receipt_id") != receipt.receipt_id().to_hex()
        || row.get::<_, String>("durability_class") != "LinuxFileAndDirectorySyncReopenSha256V1"
    {
        return Err(DurableError::Conflict(
            "durable member metadata differs from receipt",
        ));
    }
    Ok(())
}

fn receipt_from_committed_attempt(
    tx: &mut Transaction<'_>,
    attempt: &postgres::Row,
) -> DurableResult<DurableCommitReceipt> {
    let domain: String = attempt.get("domain");
    let prepare_id: Vec<u8> = attempt.get("prepare_id");
    let command_id: String = attempt.get("command_id");
    let commit_seq: i64 = attempt.get("commit_seq");
    let attempt_fence = as_u64(attempt.get::<_, i64>("attempt_fence"))?;
    if attempt_fence == 0 {
        return Err(DurableError::Corrupt("committed attempt fence is zero"));
    }
    let receipt_digest: String = attempt.get("receipt_digest");
    let row = tx
        .query_opt(
            "SELECT commit_seq,raw_request_digest,delta_digest,receipt_digest,members_root
             FROM cmd2_receipt WHERE domain=$1 AND command_id=$2 AND prepare_id=$3",
            &[&domain, &command_id, &prepare_id],
        )?
        .ok_or(DurableError::Corrupt("committed attempt has no receipt"))?;
    let raw_request_digest: String = row.get(1);
    let delta_digest: String = row.get(2);
    let members_root: String = row.get(4);
    if row.get::<_, i64>(0) != commit_seq
        || row.get::<_, String>(3) != receipt_digest
        || raw_request_digest != attempt.get::<_, String>("raw_request_digest")
        || delta_digest != attempt.get::<_, String>("delta_digest")
    {
        return Err(DurableError::Corrupt("attempt and receipt differ"));
    }
    let log = tx
        .query_opt(
            "SELECT event_kind,command_id,delta_digest,members_root FROM cmd2_log
             WHERE domain=$1 AND commit_seq=$2",
            &[&domain, &commit_seq],
        )?
        .ok_or(DurableError::Corrupt("committed receipt has no log event"))?;
    if log.get::<_, String>(0) != "command"
        || log.get::<_, String>(1) != command_id
        || log.get::<_, String>(2) != delta_digest
        || log.get::<_, String>(3) != members_root
    {
        return Err(DurableError::Corrupt("receipt and log differ"));
    }
    let outbox = tx.query_opt(
        "SELECT event_id FROM cmd2_outbox WHERE domain=$1 AND commit_seq=$2",
        &[&domain, &commit_seq],
    )?;
    if outbox.map(|row| row.get::<_, String>(0)) != Some(format!("{domain}:{commit_seq}")) {
        return Err(DurableError::Corrupt("committed outbox event differs"));
    }
    let members = tx.query(
        "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot",
        &[&domain, &prepare_id],
    )?;
    if members.is_empty() || members.len() > MAX_MEMBERS {
        return Err(DurableError::Corrupt("committed member count invalid"));
    }
    let mut members_hasher = Digest256Hasher::new();
    part(&mut members_hasher, b"cmd2-member-root-v1");
    part(&mut members_hasher, &(members.len() as u64).to_be_bytes());
    for member in &members {
        update_member_root(&mut members_hasher, member);
    }
    if members_hasher.finalize().to_hex() != members_root {
        return Err(DurableError::Corrupt("committed member root differs"));
    }
    let commit_seq_u64 = as_u64(commit_seq)?;
    let members_root_digest = parse_hex(members_root.clone())?;
    let mut receipt_hasher = Digest256Hasher::new();
    part(&mut receipt_hasher, RECEIPT_PROFILE);
    for value in [
        domain.as_bytes(),
        &prepare_id,
        &attempt_fence.to_be_bytes(),
        command_id.as_bytes(),
        &commit_seq_u64.to_be_bytes(),
        raw_request_digest.as_bytes(),
        delta_digest.as_bytes(),
        members_root_digest.as_bytes(),
    ] {
        part(&mut receipt_hasher, value);
    }
    if receipt_hasher.finalize().to_hex() != receipt_digest {
        return Err(DurableError::Corrupt("committed receipt digest differs"));
    }
    Ok(DurableCommitReceipt {
        domain,
        prepare_id,
        command_id,
        commit_seq: commit_seq_u64,
        raw_request_digest: parse_hex(raw_request_digest)?,
        delta_digest: parse_hex(delta_digest)?,
        member_root: members_root_digest,
        replayed: false,
    })
}

fn check_history_locator(
    row: &postgres::Row,
    receipt: &ByteDurabilityReceipt,
    domain: &str,
    subject: &str,
    revision: u64,
) -> DurableResult<()> {
    let coordinate = receipt.coordinate();
    if row.get::<_, String>("domain") != domain
        || row.get::<_, String>("subject") != subject
        || !source_cohort::known_profile(&receipt.binding().profile_id)
        || receipt.binding().profile_version != PROFILE_VERSION
        || receipt.binding().subject_key != subject.as_bytes()
        || row.get::<_, i64>("revision") != as_i64(revision)?
        || row.get::<_, i32>("member_slot") != receipt.binding().member_slot as i32
        || row.get::<_, Vec<u8>>("prepare_id") != receipt.prepare_id()
        || row.get::<_, String>("profile_id").as_bytes() != receipt.binding().profile_id
        || row.get::<_, String>("profile_version") != "1"
        || row.get::<_, Vec<u8>>("store_id") != receipt.store_id()
        || row.get::<_, String>("custody_domain_digest") != receipt.domain_digest().to_hex()
        || row.get::<_, Vec<u8>>("custody_domain") != receipt.custody_domain()
        || row.get::<_, Vec<u8>>("pin_id") != receipt.pin_id()
        || row.get::<_, i64>("pin_fence") != as_i64(receipt.fence_epoch())?
        || row.get::<_, String>("segment_digest") != receipt.segment_digest().to_hex()
        || row.get::<_, i64>("segment_size") != as_i64(receipt.segment_size())?
        || row.get::<_, i32>("frame_index") != receipt.frame_index() as i32
        || row.get::<_, i64>("frame_header_offset") != as_i64(coordinate.header_offset)?
        || row.get::<_, String>("frame_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("frame_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("content_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("content_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("sto_receipt_id") != receipt.receipt_id().to_hex()
        || row.get::<_, String>("durability_class") != "LinuxFileAndDirectorySyncReopenSha256V1"
    {
        return Err(DurableError::Corrupt(
            "committed locator and STO receipt differ",
        ));
    }
    Ok(())
}
