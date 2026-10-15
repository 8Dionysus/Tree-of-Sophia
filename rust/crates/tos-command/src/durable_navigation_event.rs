//! Event-local PostgreSQL persistence for complete source-navigation cohorts.
//!
//! These rows remain a private derived projection. The publisher rechecks the
//! committed CMD event and its exact STO-backed input closure before replacing
//! one record contribution and advancing the projection cursor atomically.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use postgres::{GenericClient, IsolationLevel};
use serde_json::{Value, json};
use tos_compiler::VerifiedKnowledgeModel;
use tos_compiler::source_navigation_claim_event::{
    NavigationClaimDependencyRefreshOwnerManifestSetV1,
    NavigationClaimEventAffectedOwnerManifestV1, NavigationClaimEventBasisV1,
    NavigationClaimEventCohortV1, NavigationClaimEventContributionIntegrityV1,
    NavigationClaimEventContributionV1, NavigationClaimRevisionProofV1,
};
use tos_compiler::source_navigation_event::{
    NavigationDependencyRoleV1, NavigationDependencyStateV1, NavigationDependencyV1,
    NavigationRecordDependencyRefreshCauseV1, NavigationRecordEventBasisV1,
    NavigationRecordEventCohortV1,
};
use tos_compiler::{NavigationHeaderClaim, NavigationOriginalReceipt};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath,
    canonical_bytes_v1, parse_json,
};
use tos_segment_store::SegmentStore;

use crate::source_creation::ManagedSerializedCreation;
use crate::source_creation_store::CreationFilesystem;
use crate::source_managed_selection::{
    ManagedAgentSelectedParent, ManagedCurrentModelLease, ManagedSelectedProof,
    ManagedSelectionError, ManagedSelectionResult,
};

#[path = "durable_navigation_event/durable_navigation_range.rs"]
mod durable_navigation_range;
pub use durable_navigation_range::{
    DurableNavigationEventRangeApplyV1, DurableNavigationEventRangeClaimMutationV1,
    DurableNavigationEventRangeCoverageV1, DurableNavigationEventRangeDependencyCauseV1,
    DurableNavigationEventRangeEventV1, DurableNavigationEventRangeMemberV1,
    DurableNavigationEventRangePendingReasonV1, DurableNavigationEventRangePriorClaimOwnerV1,
    DurableNavigationEventRangePriorOwnerBasisV1, DurableNavigationEventRangePriorRecordOwnerV1,
    DurableNavigationEventRangeSeedV1,
};

use super::{
    DomainSnapshot, DurableError, DurablePgCoordinator, DurableProjectionLimits,
    DurableProjectionProfile, DurableResult, EventMember, ProjectionEvent, as_i64, as_u64,
    lock_domain_snapshot, read_domain_snapshot, set_local_timeouts, validate_identifier,
    validate_relative_path,
};

const MAX_NAVIGATION_DEPENDENCIES: usize = 4096;
const MAX_NAVIGATION_ROWS: usize = 16_384;
const MAX_NAVIGATION_ROW_BYTES: usize = 4 * 1024 * 1024;
const MAX_NAVIGATION_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_NAVIGATION_CURRENT_COHORTS: usize = 16_384;
const MAX_NAVIGATION_CURRENT_ROWS: usize = 65_536;
const MAX_NAVIGATION_ADJACENCY_ROWS: usize = 65_536;
const MAX_NAVIGATION_ADJACENCY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_NAVIGATION_EVENT_ID_BYTES: usize = 256;
const MAX_NAVIGATION_RECORD_ID_BYTES: usize = 256;
const MAX_NAVIGATION_EVENT_LOG_ROWS: usize = 100_000;
const MAX_NAVIGATION_EVENT_STEPS_PER_CALL: usize = 256;
pub(super) const VERIFIED_PREFIX_FORMAT_VERSION: i16 = 2;

/// Independent resource ceilings for one event-local cohort or selected cut.
#[derive(Clone, Copy, Debug)]
pub struct DurableNavigationEventLimits {
    pub max_dependencies: usize,
    pub max_cohort_rows: usize,
    pub max_current_cohorts: usize,
    pub max_current_rows: usize,
    /// Total visible edge carriers inspected while building affected-node
    /// adjacency certificates in one bootstrap or event transaction.
    pub max_adjacency_rows: usize,
    /// Total exact edge-carrier bytes inspected while building adjacency
    /// certificates in one bootstrap or event transaction.
    pub max_adjacency_bytes: u64,
    pub max_row_bytes: usize,
    pub max_total_bytes: u64,
    pub statement_timeout: Duration,
}

impl Default for DurableNavigationEventLimits {
    fn default() -> Self {
        Self {
            max_dependencies: 4096,
            max_cohort_rows: 16_384,
            max_current_cohorts: 4096,
            max_current_rows: 65_536,
            max_adjacency_rows: 32_768,
            max_adjacency_bytes: 16 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_total_bytes: 16 * 1024 * 1024,
            statement_timeout: Duration::from_secs(30),
        }
    }
}

impl DurableNavigationEventLimits {
    fn validate(self) -> DurableResult<Self> {
        if self.max_dependencies == 0
            || self.max_dependencies > MAX_NAVIGATION_DEPENDENCIES
            || self.max_cohort_rows == 0
            || self.max_cohort_rows > MAX_NAVIGATION_ROWS
            || self.max_current_cohorts == 0
            || self.max_current_cohorts > MAX_NAVIGATION_CURRENT_COHORTS
            || self.max_current_rows == 0
            || self.max_current_rows > MAX_NAVIGATION_CURRENT_ROWS
            || self.max_adjacency_rows == 0
            || self.max_adjacency_rows > MAX_NAVIGATION_ADJACENCY_ROWS
            || self.max_adjacency_bytes == 0
            || self.max_adjacency_bytes > MAX_NAVIGATION_ADJACENCY_BYTES
            || self.max_adjacency_bytes > self.max_total_bytes
            || self.max_row_bytes == 0
            || self.max_row_bytes > MAX_NAVIGATION_ROW_BYTES
            || self.max_total_bytes == 0
            || self.max_total_bytes > MAX_NAVIGATION_TOTAL_BYTES
            || self.max_row_bytes as u64 > self.max_total_bytes
            || self.statement_timeout.is_zero()
            || self.statement_timeout > MAX_NAVIGATION_TIMEOUT
        {
            return Err(DurableError::Invalid(
                "invalid navigation event projection limits",
            ));
        }
        Ok(self)
    }
}

/// One exact source version selected from the CMD history at an event cut.
/// `exact_source_bytes` is present only for source members; an absent or
/// non-source state never fabricates source bytes.
#[derive(Clone, Debug)]
pub struct DurableNavigationEventSourceV1 {
    pub source_path: String,
    pub revision: Option<u64>,
    pub commit_sequence: Option<u64>,
    pub content_digest: Option<Digest256>,
    pub content_length: Option<u64>,
    pub is_source: bool,
    pub exact_source_bytes: Option<Vec<u8>>,
}

/// Trusted event envelope resolved from `cmd2_log`, `cmd2_outbox`, the
/// committed attempt receipt, member rows, and STO. Callers may use this to
/// construct a compiler cohort, while publication always resolves it again.
#[derive(Clone, Debug)]
pub struct DurableNavigationEventContextV1 {
    event_id: String,
    event_sequence: u64,
    event_kind: String,
    /// CMD source head observed in the same repeatable-read cut as the exact
    /// selected source versions below.
    source_head_sequence: u64,
    consumer_cursor: DurableNavigationEventCursorV1,
    members: Vec<DurableNavigationEventSourceV1>,
    selected_sources: Vec<DurableNavigationEventSourceV1>,
}

/// One owner whose persisted navigation contribution is affected by the
/// supplied changed source members.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableNavigationEventAffectedOwnerKindV1 {
    Record,
    Claim,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventAffectedOwnerV1 {
    source_path: String,
    record_id: String,
    kind: DurableNavigationEventAffectedOwnerKindV1,
}

impl DurableNavigationEventAffectedOwnerV1 {
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn kind(&self) -> DurableNavigationEventAffectedOwnerKindV1 {
        self.kind
    }
}

impl DurableNavigationEventContextV1 {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    pub fn event_kind(&self) -> &str {
        &self.event_kind
    }

    pub fn source_head_sequence(&self) -> u64 {
        self.source_head_sequence
    }

    pub fn consumer_cursor(&self) -> &DurableNavigationEventCursorV1 {
        &self.consumer_cursor
    }

    pub fn members(&self) -> &[DurableNavigationEventSourceV1] {
        &self.members
    }

    pub fn selected_sources(&self) -> &[DurableNavigationEventSourceV1] {
        &self.selected_sources
    }
}

/// Last committed navigation-consumer position used to prepare the next
/// producer invocation. Publication still rechecks it under a row lock.
#[derive(Clone, Debug)]
pub struct DurableNavigationEventCursorV1 {
    pub sequence: u64,
    pub source_revision: Option<Digest256>,
    pub last_event_id: Option<String>,
    pub last_input_root: Option<Digest256>,
    pub last_output_root: Option<Digest256>,
}

#[derive(Clone, Debug)]
pub struct DurableNavigationEventApply {
    pub event_sequence: u64,
    pub event_id: String,
    pub output_root: Digest256,
    pub replayed: bool,
}

/// Complete bounded compiler result for one CMD command. The private fields
/// prevent callers from supplying an unrelated event/member root; only the
/// exact owner-selected context can seal this set. Publication re-reads the
/// event members and verifies the same roots before changing any stored rows.
pub struct DurableNavigationEventCohortSetV2 {
    event_id: String,
    event_sequence: u64,
    source_head_sequence: u64,
    member_root: Digest256,
    set_root: Digest256,
    manifest_raw: Vec<u8>,
    record_cohorts: Vec<NavigationRecordEventCohortV1>,
    claim_cohorts: Vec<NavigationClaimEventCohortV1>,
    claim_refresh_cohorts:
        Vec<tos_compiler::source_navigation_claim_event::NavigationClaimDependencyRefreshCohortV1>,
    claim_binding: Option<DurableNavigationClaimSetBindingV1>,
    claim_refresh_binding: Option<DurableNavigationClaimRefreshSetBindingV1>,
    claim_empty_closure_binding: Option<DurableNavigationClaimEmptyClosureBindingV1>,
    claim_navigation_closure_root: Option<Digest256>,
}

#[derive(Clone, Debug)]
struct DurableNavigationClaimStreamBindingV1 {
    source_path: String,
    stream_sha256: Digest256,
    owner_claim_ids: Vec<String>,
    rows_scanned: u64,
}

#[derive(Clone, Debug)]
struct DurableNavigationClaimSetBindingV1 {
    mutation_claim_ids: Vec<String>,
    owner_claim_ids: Vec<String>,
    mutation_receipt_path: String,
    mutation_receipt_sha256: Digest256,
    claim_stream_path: String,
    claim_stream_sha256: Digest256,
    event_claim_set_root: Digest256,
    revision_claim_id: Option<String>,
    revision_history_path: Option<String>,
    revision_history_sha256: Option<Digest256>,
    revision_proof_root: Option<Digest256>,
    owner_inventory_root: Digest256,
    claim_stream_rows_scanned: u64,
    mutation_source_path: String,
    affected_owner_manifest_root: Option<Digest256>,
    closure_root: Option<Digest256>,
    reverse_claim_ids: Vec<String>,
    streams: Vec<DurableNavigationClaimStreamBindingV1>,
}

#[derive(Clone, Debug)]
struct DurableNavigationClaimRefreshCauseBindingV1 {
    changed_dependency_path: String,
    changed_dependency_sha256: Digest256,
    reverse_claim_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct DurableNavigationClaimRefreshSetBindingV1 {
    owner_inventory_root: Digest256,
    closure_root: Digest256,
    affected_owner_manifest_root: Digest256,
    reverse_claim_ids: Vec<String>,
    causes: Vec<DurableNavigationClaimRefreshCauseBindingV1>,
    streams: Vec<DurableNavigationClaimStreamBindingV1>,
}

#[derive(Clone, Debug)]
struct DurableNavigationClaimEmptyClosureBindingV1 {
    closure_root: Digest256,
    covered_source_paths: Vec<String>,
}

fn durable_claim_stream_bindings(
    streams: &[tos_compiler::source_navigation_claim_event::NavigationClaimEventAffectedOwnerStreamV1],
) -> Vec<DurableNavigationClaimStreamBindingV1> {
    streams
        .iter()
        .map(|stream| DurableNavigationClaimStreamBindingV1 {
            source_path: stream.source_path().to_owned(),
            stream_sha256: stream.stream_sha256(),
            owner_claim_ids: stream.owner_ids().to_vec(),
            rows_scanned: stream.rows_scanned(),
        })
        .collect()
}

fn verify_selected_claim_stream_bindings(
    context: &DurableNavigationEventContextV1,
    streams: &[DurableNavigationClaimStreamBindingV1],
    event_sequence: u64,
    mutation_source_path: Option<&str>,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    if streams
        .windows(2)
        .any(|pair| pair[0].source_path >= pair[1].source_path)
    {
        return Err(DurableError::Conflict(
            "navigation Claim streams are not uniquely ordered",
        ));
    }
    for stream in streams {
        validate_relative_path(&stream.source_path)?;
        let member = context
            .members
            .iter()
            .find(|member| member.source_path == stream.source_path)
            .ok_or(DurableError::Conflict(
                "navigation Claim stream is absent from committed source members",
            ))?;
        let selected = context
            .selected_sources
            .iter()
            .find(|source| source.source_path == stream.source_path)
            .ok_or(DurableError::Conflict(
                "navigation Claim stream bytes are absent from selected source cut",
            ))?;
        let raw = selected
            .exact_source_bytes
            .as_deref()
            .ok_or(DurableError::Conflict(
                "navigation Claim stream bytes are absent",
            ))?;
        let sequence = selected.commit_sequence.ok_or(DurableError::Conflict(
            "navigation Claim stream commit sequence is absent",
        ))?;
        let is_mutation_stream = mutation_source_path == Some(stream.source_path.as_str());
        if !member.is_source
            || !selected.is_source
            || (is_mutation_stream && sequence != event_sequence)
            || (!is_mutation_stream && sequence > event_sequence)
            || selected.revision != member.revision
            || selected.content_digest != member.content_digest
            || selected.content_length != member.content_length
            || selected.content_digest != Some(stream.stream_sha256)
            || selected.content_length != Some(raw.len() as u64)
            || Digest256::of_bytes(raw) != stream.stream_sha256
            || navigation_claim_owner_ids_from_stream(
                raw,
                limits.max_row_bytes,
                limits.max_current_rows,
                limits.max_total_bytes,
            )? != stream.owner_claim_ids
            || stream.rows_scanned != stream.owner_claim_ids.len() as u64
        {
            return Err(DurableError::Conflict(
                "navigation Claim stream differs from its selected complete owner inventory",
            ));
        }
    }
    Ok(())
}

fn claim_navigation_closure_ids(
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
) -> BTreeSet<String> {
    closure
        .lookups()
        .values()
        .flat_map(|observation| observation.rows.iter())
        .map(|row| row.claim_id.clone())
        .collect()
}

fn validate_claim_stream_bindings(
    context: &DurableNavigationEventContextV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    streams: &[DurableNavigationClaimStreamBindingV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<BTreeSet<String>> {
    if streams.len() > limits.max_current_cohorts
        || streams
            .windows(2)
            .any(|pair| pair[0].source_path >= pair[1].source_path)
    {
        return Err(DurableError::Refused(
            "navigation Claim stream manifest count or ordering",
        ));
    }
    let selected = context
        .selected_sources
        .iter()
        .map(|source| (source.source_path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut total_rows = 0usize;
    let mut total_bytes = 0u64;
    let mut all_ids = BTreeSet::new();
    for stream in streams {
        validate_relative_path(&stream.source_path)?;
        if stream.owner_claim_ids.is_empty()
            || stream.owner_claim_ids.len() > limits.max_current_rows
            || stream
                .owner_claim_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || stream.rows_scanned != stream.owner_claim_ids.len() as u64
        {
            return Err(DurableError::Conflict(
                "navigation Claim stream owner manifest is incomplete",
            ));
        }
        let selected_stream =
            selected
                .get(stream.source_path.as_str())
                .copied()
                .ok_or(DurableError::Conflict(
                    "navigation Claim owner stream is absent from selected sources",
                ))?;
        let raw = selected_stream
            .exact_source_bytes
            .as_deref()
            .ok_or(DurableError::Conflict(
                "navigation Claim owner stream bytes are absent",
            ))?;
        let fact =
            closure
                .member_facts()
                .get(&stream.source_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim owner stream is absent from sealed closure",
                ))?;
        let selected_size = selected_stream
            .content_length
            .ok_or(DurableError::Conflict(
                "navigation Claim owner stream length is absent",
            ))?;
        let selected_sequence = selected_stream
            .commit_sequence
            .ok_or(DurableError::Conflict(
                "navigation Claim owner stream sequence is absent",
            ))?;
        if !selected_stream.is_source
            || selected_stream.content_digest != Some(stream.stream_sha256)
            || selected_size != raw.len() as u64
            || Digest256::of_bytes(raw) != stream.stream_sha256
            || selected_sequence > context.event_sequence
            || fact.path != stream.source_path
            || fact.digest != stream.stream_sha256
            || fact.size_bytes != selected_size
            || fact.commit_seq != selected_sequence
        {
            return Err(DurableError::Conflict(
                "navigation Claim stream differs from exact selected source closure",
            ));
        }
        let owner_ids = navigation_claim_owner_ids_from_stream(
            raw,
            limits.max_row_bytes,
            limits.max_current_rows,
            limits.max_total_bytes,
        )?;
        if owner_ids != stream.owner_claim_ids {
            return Err(DurableError::Conflict(
                "navigation Claim owner IDs differ from exact selected stream",
            ));
        }
        total_rows = total_rows
            .checked_add(owner_ids.len())
            .filter(|count| *count <= limits.max_current_rows)
            .ok_or(DurableError::Refused(
                "navigation Claim stream owner count budget",
            ))?;
        total_bytes = total_bytes
            .checked_add(selected_size)
            .filter(|bytes| *bytes <= limits.max_total_bytes)
            .ok_or(DurableError::Refused("navigation Claim stream byte budget"))?;
        for owner_id in owner_ids {
            if !all_ids.insert(owner_id) {
                return Err(DurableError::Conflict(
                    "navigation Claim owner ID appears in multiple current streams",
                ));
            }
        }
    }
    Ok(all_ids)
}

impl DurableNavigationEventCohortSetV2 {
    pub fn from_context(
        context: &DurableNavigationEventContextV1,
        mut cohorts: Vec<NavigationRecordEventCohortV1>,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Self> {
        let limits = limits.validate()?;
        if context.event_kind != "command"
            || context.event_id.is_empty()
            || context.event_sequence == 0
            || context.event_sequence > context.source_head_sequence
            || cohorts.is_empty()
            || cohorts.len() > limits.max_current_cohorts
            || context.members.is_empty()
        {
            return Err(DurableError::Invalid(
                "navigation event cohort set context or count",
            ));
        }
        if cohorts.iter().any(|cohort| {
            matches!(
                cohort.basis(),
                NavigationRecordEventBasisV1::ClaimRevisionRefresh { .. }
                    | NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
            )
        }) {
            return Err(DurableError::Conflict(
                "compound refresh requires its complete owner-closure cohort set",
            ));
        }
        let member_root = navigation_event_selected_member_root(&context.members)?;
        let members = context
            .members
            .iter()
            .filter(|member| member.is_source)
            .map(|member| member.source_path.as_str())
            .collect::<BTreeSet<_>>();
        let mut cohort_keys = BTreeSet::new();
        let mut dependency_paths = BTreeSet::new();
        for cohort in &cohorts {
            let owner_member = match cohort.basis() {
                NavigationRecordEventBasisV1::Mutation => {
                    members.contains(cohort.source_path.as_str())
                }
                NavigationRecordEventBasisV1::DependencyRefresh {
                    changed_dependency_path,
                    ..
                } => members.contains(changed_dependency_path.as_str()),
                NavigationRecordEventBasisV1::DependencyRefreshSet {
                    changed_dependencies,
                } => {
                    !changed_dependencies.is_empty()
                        && changed_dependencies
                            .iter()
                            .all(|cause| members.contains(cause.path.as_str()))
                }
                NavigationRecordEventBasisV1::ClaimRevisionRefresh {
                    changed_dependency_path,
                    history_path,
                    ..
                } => {
                    members.contains(changed_dependency_path.as_str())
                        && members.contains(history_path.as_str())
                }
                NavigationRecordEventBasisV1::RangeReconciliation { .. } => false,
            };
            if cohort.event_id != context.event_id
                || cohort.event_sequence != context.event_sequence
                || cohort.source_revision != cohorts[0].source_revision
                || !owner_member
                || !cohort_keys.insert((cohort.source_path.as_str(), cohort.record_id.as_str()))
            {
                return Err(DurableError::Conflict(
                    "navigation cohort set differs from its exact command context",
                ));
            }
            let rows = validate_cohort(cohort, limits)?;
            verify_dependency_cut(cohort, &context.selected_sources, limits)?;
            verify_event_manifest(cohort, &context.selected_sources, limits)?;
            let (dependency_root, output_root, input_root) = cohort_roots(cohort, limits)?;
            if dependency_root != cohort.dependency_root
                || output_root != cohort.output_root
                || input_root != cohort.input_root
                || rows.cohort_bytes > limits.max_total_bytes
            {
                return Err(DurableError::Conflict(
                    "navigation cohort set contains an invalid sealed cohort",
                ));
            }
            for dependency in &cohort.dependencies {
                dependency_paths.insert(dependency.path.as_str());
            }
        }
        if context
            .members
            .iter()
            .any(|member| !dependency_paths.contains(member.source_path.as_str()))
        {
            return Err(DurableError::Conflict(
                "navigation cohort set omits a committed event member dependency",
            ));
        }
        cohorts.sort_by(|left, right| {
            (left.source_path.as_str(), left.record_id.as_str())
                .cmp(&(right.source_path.as_str(), right.record_id.as_str()))
        });
        let has_refresh = cohorts.iter().any(|cohort| {
            matches!(
                cohort.basis(),
                NavigationRecordEventBasisV1::DependencyRefresh { .. }
                    | NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
                    | NavigationRecordEventBasisV1::ClaimRevisionRefresh { .. }
            )
        });
        let manifest_raw = if has_refresh {
            navigation_mixed_event_cohort_manifest(
                &context.event_id,
                context.event_sequence,
                member_root,
                &cohorts,
                &[],
                None,
                limits.max_row_bytes as usize,
            )?
        } else {
            navigation_event_cohort_manifest(
                &context.event_id,
                context.event_sequence,
                member_root,
                &cohorts,
                limits.max_row_bytes as usize,
            )?
        };
        let set_root = Digest256::of_bytes(&manifest_raw);
        Ok(Self {
            event_id: context.event_id.clone(),
            event_sequence: context.event_sequence,
            source_head_sequence: context.source_head_sequence,
            member_root,
            set_root,
            manifest_raw,
            record_cohorts: cohorts,
            claim_cohorts: Vec::new(),
            claim_refresh_cohorts: Vec::new(),
            claim_binding: None,
            claim_refresh_binding: None,
            claim_empty_closure_binding: None,
            claim_navigation_closure_root: None,
        })
    }

    /// Seal a complete Claim mutation set. The exact changed Claim IDs and
    /// mutation receipt bytes are derived from the committed event context;
    /// callers cannot submit a convenient subset and advance the consumer.
    pub fn from_claim_context(
        context: &DurableNavigationEventContextV1,
        cohorts: Vec<NavigationClaimEventCohortV1>,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Self> {
        Self::from_mixed_context(context, Vec::new(), cohorts, limits)
    }

    /// Seal all projected owners affected by one committed source event.
    /// Claim owners are complete for their current source stream; unchanged
    /// Records enter only through the compiler's dependency-refresh basis.
    pub fn from_mixed_context(
        context: &DurableNavigationEventContextV1,
        mut record_cohorts: Vec<NavigationRecordEventCohortV1>,
        mut claim_cohorts: Vec<NavigationClaimEventCohortV1>,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Self> {
        let limits = limits.validate()?;
        if record_cohorts.iter().any(|cohort| {
            matches!(
                cohort.basis(),
                NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
            )
        }) {
            return Err(DurableError::Conflict(
                "Record dependency-refresh set requires the complete owner closure",
            ));
        }
        let cohort_count = record_cohorts
            .len()
            .checked_add(claim_cohorts.len())
            .ok_or(DurableError::Refused("navigation cohort-set owner count"))?;
        if context.event_kind != "command"
            || context.event_id.is_empty()
            || context.event_sequence == 0
            || context.event_sequence > context.source_head_sequence
            || cohort_count == 0
            || cohort_count > limits.max_current_cohorts
            || context.members.is_empty()
        {
            return Err(DurableError::Invalid(
                "navigation mixed event cohort set context or count",
            ));
        }
        let member_root = navigation_event_selected_member_root(&context.members)?;
        let members = context
            .members
            .iter()
            .filter(|member| member.is_source)
            .map(|member| member.source_path.as_str())
            .collect::<BTreeSet<_>>();
        let mut cohort_keys = BTreeSet::new();
        let mut dependency_paths = BTreeSet::new();
        let first_record = record_cohorts.first();
        let first_claim = claim_cohorts.first();
        let source_revision = first_record
            .map(|cohort| cohort.source_revision)
            .or_else(|| first_claim.map(|cohort| cohort.source_revision()))
            .ok_or(DurableError::Invalid("navigation cohort set has no owner"))?;
        let predecessor_source_revision = first_record
            .map(|cohort| cohort.predecessor_source_revision)
            .or_else(|| first_claim.map(|cohort| cohort.predecessor_source_revision()))
            .ok_or(DurableError::Invalid("navigation cohort set has no owner"))?;

        let mut claim_revision_binding = None;
        for cohort in &claim_cohorts {
            let Some(proof) = cohort.revision_proof() else {
                continue;
            };
            if claim_revision_binding.is_some()
                || proof.event_id() != context.event_id
                || proof.event_sequence() != context.event_sequence
                || proof.claim_id() != cohort.record_id()
                || proof.mutation_claim_ids().len() != 1
                || proof.mutation_claim_ids()[0] != proof.claim_id()
            {
                return Err(DurableError::Conflict(
                    "navigation Claim revision proof is not the unique changed owner",
                ));
            }
            claim_revision_binding = Some((
                proof.claim_id().to_owned(),
                proof.claim_stream_path().as_str().to_owned(),
                proof.claim_stream_sha256(),
                proof.history_path().as_str().to_owned(),
                proof.history_sha256(),
                proof.revision_receipt_sha256(),
                proof.binding_root(),
            ));
        }

        for cohort in &record_cohorts {
            let owner_member = match cohort.basis() {
                NavigationRecordEventBasisV1::Mutation => {
                    members.contains(cohort.source_path.as_str())
                }
                NavigationRecordEventBasisV1::DependencyRefresh {
                    changed_dependency_path,
                    ..
                } => members.contains(changed_dependency_path.as_str()),
                NavigationRecordEventBasisV1::DependencyRefreshSet {
                    changed_dependencies,
                } => {
                    !changed_dependencies.is_empty()
                        && changed_dependencies
                            .iter()
                            .all(|cause| members.contains(cause.path.as_str()))
                }
                NavigationRecordEventBasisV1::ClaimRevisionRefresh {
                    changed_dependency_path,
                    history_path,
                    claim_id,
                    revision_proof_root,
                    changed_dependency_sha256,
                    history_sha256,
                } => {
                    members.contains(changed_dependency_path.as_str())
                        && members.contains(history_path.as_str())
                        && claim_revision_binding.as_ref().is_some_and(
                            |(
                                expected_id,
                                expected_stream,
                                expected_stream_sha,
                                expected_history,
                                expected_sha,
                                _expected_revision_receipt_sha,
                                expected_root,
                            )| {
                                expected_id == claim_id
                                    && expected_stream == changed_dependency_path.as_str()
                                    && expected_stream_sha == changed_dependency_sha256
                                    && expected_history == history_path.as_str()
                                    && expected_sha == history_sha256
                                    && expected_root == revision_proof_root
                            },
                        )
                }
                NavigationRecordEventBasisV1::RangeReconciliation { .. } => false,
            };
            if cohort.event_id != context.event_id
                || cohort.event_sequence != context.event_sequence
                || cohort.source_revision != source_revision
                || cohort.predecessor_source_revision != predecessor_source_revision
                || !owner_member
                || !cohort_keys.insert((
                    cohort.source_path.as_str().to_owned(),
                    cohort.record_id.clone(),
                ))
            {
                return Err(DurableError::Conflict(
                    "navigation Record cohort differs from its exact command context",
                ));
            }
            let rows = validate_cohort(cohort, limits)?;
            verify_dependency_cut(cohort, &context.selected_sources, limits)?;
            verify_event_manifest(cohort, &context.selected_sources, limits)?;
            let (dependency_root, output_root, input_root) = cohort_roots(cohort, limits)?;
            if dependency_root != cohort.dependency_root
                || output_root != cohort.output_root
                || input_root != cohort.input_root
                || rows.cohort_bytes > limits.max_total_bytes
            {
                return Err(DurableError::Conflict(
                    "navigation cohort set contains an invalid sealed Record cohort",
                ));
            }
            dependency_paths.extend(
                cohort
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.path.as_str()),
            );
        }

        let claim_binding = if let Some(first) = first_claim {
            if claim_cohorts.is_empty() {
                return Err(DurableError::Invalid(
                    "navigation Claim cohort set is empty",
                ));
            }
            let mutation_receipt_path = first.mutation_receipt_path().as_str();
            let exact_receipt_member = context
                .members
                .iter()
                .find(|member| member.source_path == mutation_receipt_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim mutation receipt is not a committed event member",
                ))?;
            if !exact_receipt_member.is_source
                || exact_receipt_member.commit_sequence != Some(context.event_sequence)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim mutation receipt is outside the committed event",
                ));
            }
            let selected_receipt = context
                .selected_sources
                .iter()
                .find(|source| source.source_path == mutation_receipt_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim mutation receipt bytes are not selected",
                ))?;
            let receipt_raw =
                selected_receipt
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Claim mutation receipt bytes are absent",
                    ))?;
            let receipt_value = strict_json_value(receipt_raw, limits.max_row_bytes)?;
            let is_revision_receipt = receipt_value.get("schema_version").and_then(Value::as_str)
                == Some("tos_claim_revision_history_v1");
            let receipt_digest = selected_receipt
                .content_digest
                .ok_or(DurableError::Corrupt(
                    "navigation Claim mutation receipt digest absent",
                ))?;
            if is_revision_receipt != claim_revision_binding.is_some()
                || claim_revision_binding.as_ref().is_some_and(
                    |(_, _, _, history_path, history_sha256, _, _)| {
                        history_path != mutation_receipt_path || *history_sha256 != receipt_digest
                    },
                )
            {
                return Err(DurableError::Conflict(
                    "navigation Claim revision proof differs from its mutation receipt",
                ));
            }
            let receipt_length = selected_receipt
                .content_length
                .ok_or(DurableError::Corrupt(
                    "navigation Claim mutation receipt length absent",
                ))?;
            if selected_receipt.commit_sequence != Some(context.event_sequence)
                || selected_receipt.revision != exact_receipt_member.revision
                || Some(receipt_digest) != exact_receipt_member.content_digest
                || Some(receipt_length) != exact_receipt_member.content_length
                || Digest256::of_bytes(receipt_raw) != receipt_digest
                || receipt_raw.len() as u64 != receipt_length
            {
                return Err(DurableError::Conflict(
                    "navigation Claim mutation receipt differs from exact committed member",
                ));
            }
            let mutation_receipt = navigation_claim_ids_from_mutation_receipt(
                receipt_raw,
                first.source_path().as_str(),
                mutation_receipt_path,
                limits.max_row_bytes,
            )?;
            if mutation_receipt.revision_receipt_sha256
                != claim_revision_binding.as_ref().map(|binding| binding.5)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim changed receipt differs from its revision proof",
                ));
            }
            let mutation_claim_ids = mutation_receipt.claim_ids;

            let claim_stream_path = first.source_path().as_str();
            let exact_claim_member = context
                .members
                .iter()
                .find(|member| member.source_path == claim_stream_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim stream is not a committed event member",
                ))?;
            if !exact_claim_member.is_source
                || exact_claim_member.commit_sequence != Some(context.event_sequence)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim stream is outside the committed event",
                ));
            }
            let selected_claim_stream = context
                .selected_sources
                .iter()
                .find(|source| source.source_path == claim_stream_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim stream bytes are not selected",
                ))?;
            let claim_stream_raw = selected_claim_stream.exact_source_bytes.as_deref().ok_or(
                DurableError::Conflict("navigation Claim stream bytes are absent"),
            )?;
            if selected_claim_stream.commit_sequence != Some(context.event_sequence)
                || selected_claim_stream.revision != exact_claim_member.revision
                || selected_claim_stream.content_digest != exact_claim_member.content_digest
                || selected_claim_stream.content_length != exact_claim_member.content_length
            {
                return Err(DurableError::Conflict(
                    "navigation Claim stream differs from its committed member",
                ));
            }
            let claim_stream_sha256 = Digest256::of_bytes(claim_stream_raw);
            if selected_claim_stream.content_digest != Some(claim_stream_sha256) {
                return Err(DurableError::Conflict(
                    "navigation Claim stream bytes differ from the exact event cut",
                ));
            }
            if claim_revision_binding.as_ref().is_some_and(
                |(_, stream_path, stream_sha256, _, _, _, _)| {
                    stream_path != claim_stream_path || *stream_sha256 != claim_stream_sha256
                },
            ) {
                return Err(DurableError::Conflict(
                    "navigation Claim revision proof differs from its current stream",
                ));
            }
            let owner_claim_ids = navigation_claim_owner_ids_from_stream(
                claim_stream_raw,
                limits.max_row_bytes,
                limits.max_current_rows,
                limits.max_total_bytes,
            )?;
            let first_integrity = first
                .verify_integrity()
                .map_err(|_| DurableError::Conflict("navigation Claim integrity differs"))?;
            let event_claim_set_root = first_integrity.event_claim_set_root;
            let owner_inventory_root = first_integrity.owner_inventory_root;
            let claim_stream_rows_scanned = first_integrity.claim_stream_rows_scanned;
            let mut claim_keys = BTreeSet::new();
            for cohort in &claim_cohorts {
                let integrity = cohort
                    .verify_integrity()
                    .map_err(|_| DurableError::Conflict("navigation Claim integrity differs"))?;
                if cohort.event_id() != context.event_id
                    || cohort.event_sequence() != context.event_sequence
                    || cohort.source_revision() != source_revision
                    || cohort.predecessor_source_revision() != predecessor_source_revision
                    || cohort.source_path().as_str() != claim_stream_path
                    || cohort.mutation_receipt_path().as_str() != mutation_receipt_path
                    || integrity.event_claim_set_binding != receipt_digest
                    || integrity.event_claim_set_root != event_claim_set_root
                    || integrity.event_mutation_claim_ids != mutation_claim_ids.as_slice()
                    || integrity.event_claim_owner_ids != owner_claim_ids.as_slice()
                    || integrity.claim_stream_sha256 != claim_stream_sha256
                    || integrity.owner_inventory_root != owner_inventory_root
                    || integrity.claim_stream_rows_scanned != claim_stream_rows_scanned
                    || integrity.revision_proof_root
                        != cohort
                            .revision_proof()
                            .map(NavigationClaimRevisionProofV1::binding_root)
                    || !claim_keys.insert(cohort.record_id().to_owned())
                    || !cohort_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim cohort differs from its complete event stream",
                    ));
                }
                let rows = validate_navigation_claim_cohort(cohort, limits)?;
                verify_navigation_claim_dependency_cut(cohort, &context.selected_sources, limits)?;
                if integrity.dependency_root != cohort.dependency_root()
                    || integrity.output_root != cohort.output_root()
                    || integrity.input_root != cohort.input_root()
                    || rows.cohort_bytes > limits.max_total_bytes
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim set contains an invalid sealed cohort",
                    ));
                }
                dependency_paths.extend(
                    cohort
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.path.as_str()),
                );
            }
            if owner_claim_ids.iter().cloned().collect::<BTreeSet<_>>() != claim_keys
                || mutation_claim_ids
                    .iter()
                    .any(|id| owner_claim_ids.binary_search(id).is_err())
            {
                return Err(DurableError::Conflict(
                    "navigation Claim cohorts do not cover the complete selected stream",
                ));
            }
            Some(DurableNavigationClaimSetBindingV1 {
                mutation_claim_ids,
                owner_claim_ids: owner_claim_ids.clone(),
                mutation_receipt_path: mutation_receipt_path.to_owned(),
                mutation_receipt_sha256: receipt_digest,
                claim_stream_path: claim_stream_path.to_owned(),
                claim_stream_sha256,
                event_claim_set_root,
                revision_claim_id: claim_revision_binding
                    .as_ref()
                    .map(|binding| binding.0.clone()),
                revision_history_path: claim_revision_binding
                    .as_ref()
                    .map(|binding| binding.3.clone()),
                revision_history_sha256: claim_revision_binding.as_ref().map(|binding| binding.4),
                revision_proof_root: claim_revision_binding.as_ref().map(|binding| binding.6),
                owner_inventory_root,
                claim_stream_rows_scanned,
                mutation_source_path: first.mutation_source_path().as_str().to_owned(),
                affected_owner_manifest_root: None,
                closure_root: None,
                reverse_claim_ids: Vec::new(),
                streams: vec![DurableNavigationClaimStreamBindingV1 {
                    source_path: claim_stream_path.to_owned(),
                    stream_sha256: claim_stream_sha256,
                    owner_claim_ids,
                    rows_scanned: claim_stream_rows_scanned,
                }],
            })
        } else {
            None
        };

        if context
            .members
            .iter()
            .any(|member| !dependency_paths.contains(member.source_path.as_str()))
        {
            return Err(DurableError::Conflict(
                "navigation cohort set omits a committed event member dependency",
            ));
        }
        record_cohorts.sort_by(|left, right| {
            (left.source_path.as_str(), left.record_id.as_str())
                .cmp(&(right.source_path.as_str(), right.record_id.as_str()))
        });
        claim_cohorts.sort_by(|left, right| {
            (left.source_path().as_str(), left.record_id())
                .cmp(&(right.source_path().as_str(), right.record_id()))
        });
        let has_refresh = record_cohorts.iter().any(|cohort| {
            matches!(
                cohort.basis(),
                NavigationRecordEventBasisV1::DependencyRefresh { .. }
                    | NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
                    | NavigationRecordEventBasisV1::ClaimRevisionRefresh { .. }
            )
        });
        let manifest_raw = if claim_binding.is_some() || has_refresh {
            navigation_mixed_event_cohort_manifest(
                &context.event_id,
                context.event_sequence,
                member_root,
                &record_cohorts,
                &claim_cohorts,
                claim_binding.as_ref(),
                limits.max_row_bytes as usize,
            )?
        } else {
            navigation_event_cohort_manifest(
                &context.event_id,
                context.event_sequence,
                member_root,
                &record_cohorts,
                limits.max_row_bytes as usize,
            )?
        };
        let set_root = Digest256::of_bytes(&manifest_raw);
        Ok(Self {
            event_id: context.event_id.clone(),
            event_sequence: context.event_sequence,
            source_head_sequence: context.source_head_sequence,
            member_root,
            set_root,
            manifest_raw,
            record_cohorts,
            claim_cohorts,
            claim_refresh_cohorts: Vec::new(),
            claim_binding,
            claim_refresh_binding: None,
            claim_empty_closure_binding: None,
            claim_navigation_closure_root: None,
        })
    }

    /// Seal an event set against the owner-maintained Claim reverse-reference
    /// index. Both Claim mutations and non-Claim dependency refreshes require
    /// a complete manifest; an empty refresh manifest proves a bounded empty
    /// reverse closure and still binds the changed source members.
    pub(crate) fn from_mixed_context_with_claim_navigation_closure(
        domain: &str,
        context: &DurableNavigationEventContextV1,
        mut record_cohorts: Vec<NavigationRecordEventCohortV1>,
        claim_contributions: Vec<NavigationClaimEventContributionV1>,
        claim_mutation_manifest: Option<&NavigationClaimEventAffectedOwnerManifestV1>,
        claim_refresh_manifest_set: Option<&NavigationClaimDependencyRefreshOwnerManifestSetV1>,
        claim_closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Self> {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation closure domain")?;
        let selection = claim_closure.selection_identity();
        if claim_closure.domain() != domain
            || selection.domain_digest != Digest256::of_bytes(domain.as_bytes())
            || selection.head_seq != context.source_head_sequence
            || context.event_kind != "command"
            || context.event_id.is_empty()
            || context.event_sequence == 0
            || context.event_sequence > context.source_head_sequence
            || context.members.is_empty()
        {
            return Err(DurableError::Conflict(
                "navigation Claim closure differs from the selected domain/event cut",
            ));
        }
        let mutation_mode = claim_contributions.first().is_some_and(|contribution| {
            contribution.basis() == NavigationClaimEventBasisV1::Mutation
        });
        let empty_claim_closure = claim_contributions.is_empty()
            && claim_mutation_manifest.is_none()
            && claim_refresh_manifest_set.is_none();
        if !empty_claim_closure
            && (claim_contributions.iter().any(|contribution| {
                (contribution.basis() == NavigationClaimEventBasisV1::Mutation) != mutation_mode
            }) || (mutation_mode
                && (claim_mutation_manifest.is_none() || claim_refresh_manifest_set.is_some()))
                || (!mutation_mode
                    && (claim_mutation_manifest.is_some() || claim_refresh_manifest_set.is_none())))
        {
            return Err(DurableError::Conflict(
                "navigation Claim event has mixed or incomplete cause manifests",
            ));
        }
        let claim_closure_root = claim_closure.closure_root();
        let closure_ids = claim_navigation_closure_ids(claim_closure);
        let member_root = navigation_event_selected_member_root(&context.members)?;
        let members = context
            .members
            .iter()
            .filter(|member| member.is_source)
            .map(|member| member.source_path.as_str())
            .collect::<BTreeSet<_>>();
        let cohort_count = record_cohorts
            .len()
            .checked_add(claim_contributions.len())
            .ok_or(DurableError::Refused("navigation cohort-set owner count"))?;
        if cohort_count == 0
            || cohort_count > limits.max_current_cohorts
            || claim_closure.row_count() > limits.max_current_rows
            || claim_closure.lookup_count()
                > crate::source_claim_dependency_index::MAX_NAVIGATION_REFERENCE_LOOKUPS
        {
            return Err(DurableError::Refused(
                "navigation Claim closure or owner count budget",
            ));
        }
        let first_record = record_cohorts.first();
        let first_claim = claim_contributions.first();
        let source_revision = first_record
            .map(|cohort| cohort.source_revision)
            .or_else(|| first_claim.map(NavigationClaimEventContributionV1::source_revision))
            .ok_or(DurableError::Invalid("navigation cohort set has no owner"))?;
        let predecessor_source_revision = first_record
            .map(|cohort| cohort.predecessor_source_revision)
            .or_else(|| {
                first_claim.map(NavigationClaimEventContributionV1::predecessor_source_revision)
            })
            .ok_or(DurableError::Invalid("navigation cohort set has no owner"))?;
        let mut cohort_keys = BTreeSet::new();
        let mut dependency_paths = BTreeSet::<String>::new();
        let mut claim_revision_binding = None;
        for contribution in &claim_contributions {
            if let NavigationClaimEventContributionV1::Mutation(cohort) = contribution {
                let Some(proof) = cohort.revision_proof() else {
                    continue;
                };
                if claim_revision_binding.is_some()
                    || proof.event_id() != context.event_id
                    || proof.event_sequence() != context.event_sequence
                    || proof.claim_id() != cohort.record_id()
                    || proof.mutation_claim_ids().len() != 1
                    || proof.mutation_claim_ids()[0] != proof.claim_id()
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim revision proof is not the unique changed owner",
                    ));
                }
                claim_revision_binding = Some((
                    proof.claim_id().to_owned(),
                    proof.claim_stream_path().as_str().to_owned(),
                    proof.claim_stream_sha256(),
                    proof.history_path().as_str().to_owned(),
                    proof.history_sha256(),
                    proof.revision_receipt_sha256(),
                    proof.binding_root(),
                ));
            }
        }
        for cohort in &record_cohorts {
            let owner_member = match cohort.basis() {
                NavigationRecordEventBasisV1::Mutation => {
                    members.contains(cohort.source_path.as_str())
                }
                NavigationRecordEventBasisV1::DependencyRefresh {
                    changed_dependency_path,
                    ..
                } => members.contains(changed_dependency_path.as_str()),
                NavigationRecordEventBasisV1::DependencyRefreshSet {
                    changed_dependencies,
                } => {
                    !changed_dependencies.is_empty()
                        && changed_dependencies
                            .iter()
                            .all(|cause| members.contains(cause.path.as_str()))
                }
                NavigationRecordEventBasisV1::ClaimRevisionRefresh {
                    changed_dependency_path,
                    history_path,
                    claim_id,
                    revision_proof_root,
                    changed_dependency_sha256,
                    history_sha256,
                } => {
                    members.contains(changed_dependency_path.as_str())
                        && members.contains(history_path.as_str())
                        && claim_revision_binding.as_ref().is_some_and(
                            |(
                                expected_id,
                                expected_stream,
                                expected_stream_sha,
                                expected_history,
                                expected_sha,
                                _expected_revision_receipt_sha,
                                expected_root,
                            )| {
                                expected_id == claim_id
                                    && expected_stream == changed_dependency_path.as_str()
                                    && expected_stream_sha == changed_dependency_sha256
                                    && expected_history == history_path.as_str()
                                    && expected_sha == history_sha256
                                    && expected_root == revision_proof_root
                            },
                        )
                }
                NavigationRecordEventBasisV1::RangeReconciliation { .. } => false,
            };
            if cohort.event_id != context.event_id
                || cohort.event_sequence != context.event_sequence
                || cohort.source_revision != source_revision
                || cohort.predecessor_source_revision != predecessor_source_revision
                || !owner_member
                || !cohort_keys.insert((
                    cohort.source_path.as_str().to_owned(),
                    cohort.record_id.clone(),
                ))
            {
                return Err(DurableError::Conflict(
                    "navigation Record cohort differs from its exact command context",
                ));
            }
            let rows = validate_cohort(cohort, limits)?;
            verify_dependency_cut(cohort, &context.selected_sources, limits)?;
            verify_event_manifest(cohort, &context.selected_sources, limits)?;
            let (dependency_root, output_root, input_root) = cohort_roots(cohort, limits)?;
            if dependency_root != cohort.dependency_root
                || output_root != cohort.output_root
                || input_root != cohort.input_root
                || rows.cohort_bytes > limits.max_total_bytes
            {
                return Err(DurableError::Conflict(
                    "navigation cohort set contains an invalid sealed Record cohort",
                ));
            }
            dependency_paths.extend(
                cohort
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.path.as_str().to_owned()),
            );
        }

        let mut claim_cohorts = Vec::new();
        let mut claim_refresh_cohorts = Vec::new();
        let mut claim_binding = None;
        let mut claim_refresh_binding = None;
        let mut claim_empty_closure_binding = None;
        if empty_claim_closure {
            let changed_source_paths = context
                .members
                .iter()
                .filter(|member| member.is_source && member.source_path.starts_with("ToS/"))
                .map(|member| member.source_path.clone())
                .collect::<BTreeSet<_>>();
            let looked_up_source_paths = claim_closure
                .lookups()
                .keys()
                .filter_map(|(kind, path)| {
                    (*kind
                        == crate::source_claim_dependency_index::ClaimNavigationReferenceKind::SourcePath)
                        .then_some(path.clone())
                })
                .collect::<BTreeSet<_>>();
            if changed_source_paths.is_empty()
                || changed_source_paths
                    .iter()
                    .any(|path| path.ends_with("/source-claims.jsonl"))
                || !changed_source_paths.is_subset(&looked_up_source_paths)
                || !closure_ids.is_empty()
                || !claim_closure.member_facts().is_empty()
                || !claim_closure.members().is_empty()
                || claim_closure
                    .lookups()
                    .values()
                    .any(|observation| !observation.rows.is_empty())
            {
                return Err(DurableError::Conflict(
                    "navigation empty Claim closure does not cover exact changed source paths",
                ));
            }
            let covered_source_paths = changed_source_paths.into_iter().collect::<Vec<_>>();
            claim_empty_closure_binding = Some(DurableNavigationClaimEmptyClosureBindingV1 {
                closure_root: claim_closure_root,
                covered_source_paths,
            });
        } else if mutation_mode {
            let manifest = claim_mutation_manifest.expect("cause shape checked");
            manifest
                .verify_integrity()
                .map_err(|_| DurableError::Conflict("navigation Claim owner manifest differs"))?;
            if manifest
                .reverse_claim_ids()
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                != closure_ids
            {
                return Err(DurableError::Conflict(
                    "navigation Claim owner manifest differs from reverse-reference closure",
                ));
            }
            let first = match claim_contributions.first() {
                Some(NavigationClaimEventContributionV1::Mutation(cohort)) => cohort,
                _ => return Err(DurableError::Invalid("navigation Claim mutation absent")),
            };
            let mutation_source_path = manifest.mutation_source_path();
            if first.mutation_source_path().as_str() != mutation_source_path
                || !claim_closure.lookups().contains_key(&(
                    crate::source_claim_dependency_index::ClaimNavigationReferenceKind::SourcePath,
                    mutation_source_path.to_owned(),
                ))
                || manifest.mutation_claim_ids().iter().any(|claim_id| {
                    !claim_closure.lookups().contains_key(&(
                        crate::source_claim_dependency_index::ClaimNavigationReferenceKind::TosId,
                        claim_id.clone(),
                    ))
                })
            {
                return Err(DurableError::Conflict(
                    "navigation Claim mutation cause is absent from reverse-reference closure",
                ));
            }
            let receipt_path = first.mutation_receipt_path().as_str().to_owned();
            let receipt_member = context
                .members
                .iter()
                .find(|member| member.source_path == receipt_path.as_str())
                .ok_or(DurableError::Conflict(
                    "navigation Claim mutation receipt is not a committed event member",
                ))?;
            let selected_receipt = context
                .selected_sources
                .iter()
                .find(|source| source.source_path == receipt_path.as_str())
                .ok_or(DurableError::Conflict(
                    "navigation Claim mutation receipt bytes are not selected",
                ))?;
            let receipt_raw =
                selected_receipt
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Claim mutation receipt bytes are absent",
                    ))?;
            let receipt_sha = selected_receipt
                .content_digest
                .ok_or(DurableError::Corrupt(
                    "navigation Claim receipt digest absent",
                ))?;
            let receipt_len = selected_receipt
                .content_length
                .ok_or(DurableError::Corrupt(
                    "navigation Claim receipt length absent",
                ))?;
            let mutation_receipt = navigation_claim_ids_from_mutation_receipt(
                receipt_raw,
                mutation_source_path,
                receipt_path.as_str(),
                limits.max_row_bytes,
            )?;
            if mutation_receipt.revision_receipt_sha256
                != claim_revision_binding.as_ref().map(|binding| binding.5)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim changed receipt differs from its revision proof",
                ));
            }
            let mutation_claim_ids = mutation_receipt.claim_ids;
            if !receipt_member.is_source
                || receipt_member.commit_sequence != Some(context.event_sequence)
                || selected_receipt.commit_sequence != Some(context.event_sequence)
                || selected_receipt.revision != receipt_member.revision
                || selected_receipt.content_digest != receipt_member.content_digest
                || selected_receipt.content_length != receipt_member.content_length
                || selected_receipt.content_digest != Some(receipt_sha)
                || selected_receipt.content_length != Some(receipt_len)
                || Digest256::of_bytes(receipt_raw) != receipt_sha
                || receipt_raw.len() as u64 != receipt_len
                || manifest.mutation_claim_ids() != mutation_claim_ids.as_slice()
            {
                return Err(DurableError::Conflict(
                    "navigation Claim mutation receipt differs from its owner manifest",
                ));
            }
            let streams = durable_claim_stream_bindings(manifest.streams());
            let owner_ids =
                validate_claim_stream_bindings(context, claim_closure, &streams, limits)?;
            let mutation_stream = streams
                .iter()
                .find(|stream| stream.source_path == mutation_source_path)
                .ok_or(DurableError::Conflict(
                    "navigation mutation stream is absent from owner manifest",
                ))?;
            if mutation_stream.stream_sha256
                != first
                    .dependencies()
                    .iter()
                    .find(|dependency| dependency.path.as_str() == mutation_source_path)
                    .and_then(|dependency| match &dependency.state {
                        NavigationDependencyStateV1::Present { sha256, .. } => Some(*sha256),
                        NavigationDependencyStateV1::Absent { .. } => None,
                    })
                    .ok_or(DurableError::Conflict(
                        "navigation Claim mutation stream dependency is absent",
                    ))?
            {
                return Err(DurableError::Conflict(
                    "navigation mutation stream digest differs from owner manifest",
                ));
            }
            let first_integrity = first
                .verify_integrity()
                .map_err(|_| DurableError::Conflict("navigation Claim integrity differs"))?;
            if first_integrity.owner_inventory_root != manifest.inventory_root() {
                return Err(DurableError::Conflict(
                    "navigation Claim owner inventory differs from manifest",
                ));
            }
            let event_claim_set_root = first_integrity.event_claim_set_root;
            let mut all_claim_keys = BTreeSet::new();
            for contribution in claim_contributions {
                let NavigationClaimEventContributionV1::Mutation(cohort) = contribution else {
                    return Err(DurableError::Conflict(
                        "navigation Claim mutation set contains another basis",
                    ));
                };
                let integrity = cohort
                    .verify_integrity()
                    .map_err(|_| DurableError::Conflict("navigation Claim integrity differs"))?;
                let stream = streams
                    .iter()
                    .find(|stream| stream.source_path == cohort.source_path().as_str())
                    .ok_or(DurableError::Conflict(
                        "navigation Claim owner stream absent from full manifest",
                    ))?;
                if cohort.event_id() != context.event_id
                    || cohort.event_sequence() != context.event_sequence
                    || cohort.source_revision() != source_revision
                    || cohort.predecessor_source_revision() != predecessor_source_revision
                    || cohort.mutation_source_path().as_str() != mutation_source_path
                    || cohort.mutation_receipt_path().as_str() != receipt_path.as_str()
                    || integrity.event_claim_set_binding != receipt_sha
                    || integrity.event_claim_set_root != event_claim_set_root
                    || integrity.event_mutation_claim_ids != mutation_claim_ids.as_slice()
                    || integrity.event_claim_owner_ids != stream.owner_claim_ids.as_slice()
                    || integrity.claim_stream_sha256 != stream.stream_sha256
                    || integrity.claim_stream_rows_scanned != stream.rows_scanned
                    || integrity.owner_inventory_root != manifest.inventory_root()
                    || integrity.revision_proof_root
                        != cohort
                            .revision_proof()
                            .map(NavigationClaimRevisionProofV1::binding_root)
                    || !cohort_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                    || !all_claim_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim cohort differs from its full affected-stream manifest",
                    ));
                }
                let rows = validate_navigation_claim_cohort(&cohort, limits)?;
                verify_navigation_claim_dependency_cut(&cohort, &context.selected_sources, limits)?;
                if integrity.dependency_root != cohort.dependency_root()
                    || integrity.output_root != cohort.output_root()
                    || integrity.input_root != cohort.input_root()
                    || rows.cohort_bytes > limits.max_total_bytes
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim set contains an invalid sealed cohort",
                    ));
                }
                dependency_paths.extend(
                    cohort
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.path.as_str().to_owned()),
                );
                claim_cohorts.push(cohort);
            }
            let expected_keys = streams
                .iter()
                .flat_map(|stream| {
                    stream
                        .owner_claim_ids
                        .iter()
                        .map(move |id| (stream.source_path.clone(), id.clone()))
                })
                .collect::<BTreeSet<_>>();
            if all_claim_keys != expected_keys {
                return Err(DurableError::Conflict(
                    "navigation Claim contributions do not cover every affected stream owner",
                ));
            }
            let mutation_stream_sha = mutation_stream.stream_sha256;
            claim_binding = Some(DurableNavigationClaimSetBindingV1 {
                mutation_claim_ids,
                owner_claim_ids: owner_ids.into_iter().collect(),
                mutation_receipt_path: receipt_path,
                mutation_receipt_sha256: receipt_sha,
                claim_stream_path: mutation_source_path.to_owned(),
                claim_stream_sha256: mutation_stream_sha,
                event_claim_set_root,
                revision_claim_id: claim_revision_binding
                    .as_ref()
                    .map(|binding| binding.0.clone()),
                revision_history_path: claim_revision_binding
                    .as_ref()
                    .map(|binding| binding.3.clone()),
                revision_history_sha256: claim_revision_binding.as_ref().map(|binding| binding.4),
                revision_proof_root: claim_revision_binding.as_ref().map(|binding| binding.6),
                owner_inventory_root: manifest.inventory_root(),
                claim_stream_rows_scanned: mutation_stream.rows_scanned,
                mutation_source_path: mutation_source_path.to_owned(),
                affected_owner_manifest_root: Some(manifest.root()),
                closure_root: Some(claim_closure_root),
                reverse_claim_ids: manifest.reverse_claim_ids().to_vec(),
                streams,
            });
        } else {
            let manifest = claim_refresh_manifest_set.expect("cause shape checked");
            manifest.verify_integrity().map_err(|_| {
                DurableError::Conflict("navigation Claim dependency-refresh manifest differs")
            })?;
            if manifest.closure_root() != claim_closure_root
                || manifest
                    .reverse_claim_ids()
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    != closure_ids
            {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh manifest differs from reverse-reference closure",
                ));
            }
            let mut expected_changed_paths = context
                .members
                .iter()
                .filter(|member| member.is_source && member.source_path.starts_with("ToS/"))
                .filter(|member| {
                    !member
                        .source_path
                        .starts_with("ToS/source-witnesses/relations/")
                        || !member.source_path.ends_with("/source-claims.jsonl")
                })
                .map(|member| member.source_path.as_str())
                .collect::<BTreeSet<_>>();
            let manifest_paths = manifest
                .causes()
                .iter()
                .map(|cause| cause.changed_dependency_path().as_str())
                .collect::<BTreeSet<_>>();
            if expected_changed_paths != manifest_paths {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh causes do not cover every changed ToS source member",
                ));
            }
            for cause in manifest.causes() {
                let path = cause.changed_dependency_path().as_str();
                if !claim_closure.lookups().contains_key(&(
                    crate::source_claim_dependency_index::ClaimNavigationReferenceKind::SourcePath,
                    path.to_owned(),
                )) {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh cause is absent from reverse-reference closure",
                    ));
                }
                let member = context
                    .members
                    .iter()
                    .find(|member| member.source_path == path)
                    .ok_or(DurableError::Conflict(
                        "navigation Claim refresh cause is not a committed event member",
                    ))?;
                let selected = context
                    .selected_sources
                    .iter()
                    .find(|source| source.source_path == path)
                    .ok_or(DurableError::Conflict(
                        "navigation Claim refresh cause bytes are absent",
                    ))?;
                if !member.is_source
                    || member.commit_sequence != Some(context.event_sequence)
                    || !selected.is_source
                    || selected.commit_sequence != Some(context.event_sequence)
                    || selected.content_digest != Some(cause.changed_dependency_sha256())
                    || selected.exact_source_bytes.as_deref().is_none_or(|raw| {
                        Digest256::of_bytes(raw) != cause.changed_dependency_sha256()
                    })
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh cause differs from exact event member",
                    ));
                }
            }
            let streams = durable_claim_stream_bindings(manifest.streams());
            let _stream_owner_ids =
                validate_claim_stream_bindings(context, claim_closure, &streams, limits)?;
            if claim_contributions.is_empty() != streams.is_empty() {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh cohorts differ from the complete reverse owner set",
                ));
            }
            let expected_keys = streams
                .iter()
                .flat_map(|stream| {
                    stream
                        .owner_claim_ids
                        .iter()
                        .map(move |id| (stream.source_path.clone(), id.clone()))
                })
                .collect::<BTreeSet<_>>();
            let mut actual_keys = BTreeSet::new();
            for contribution in claim_contributions {
                let NavigationClaimEventContributionV1::DependencyRefresh(cohort) = contribution
                else {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh set contains a mutation contribution",
                    ));
                };
                let integrity = cohort.verify_integrity().map_err(|_| {
                    DurableError::Conflict("navigation Claim refresh integrity differs")
                })?;
                let stream = streams
                    .iter()
                    .find(|stream| stream.source_path == cohort.source_path().as_str())
                    .ok_or(DurableError::Conflict(
                        "navigation Claim refresh owner stream absent from manifest",
                    ))?;
                if cohort.event_id() != context.event_id
                    || cohort.event_sequence() != context.event_sequence
                    || cohort.source_revision() != source_revision
                    || cohort.predecessor_source_revision() != predecessor_source_revision
                    || integrity.owner_inventory_root != manifest.inventory_root()
                    || integrity.proof.closure_root() != claim_closure_root
                    || integrity.proof.affected_owner_manifest_root() != manifest.root()
                    || integrity.proof.reverse_claim_ids() != manifest.reverse_claim_ids()
                    || cohort.event_claim_owner_ids() != stream.owner_claim_ids.as_slice()
                    || cohort.claim_stream_sha256() != stream.stream_sha256
                    || cohort.claim_stream_rows_scanned() != stream.rows_scanned
                    || !actual_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                    || !cohort_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh cohort differs from its full owner closure",
                    ));
                }
                let proof_causes = integrity.proof.changed_dependencies();
                if proof_causes.len() != manifest.causes().len()
                    || proof_causes
                        .iter()
                        .zip(manifest.causes())
                        .any(|(proof, cause)| {
                            proof.changed_dependency_path() != cause.changed_dependency_path()
                                || proof.changed_dependency_sha256()
                                    != cause.changed_dependency_sha256()
                                || proof.reverse_claim_ids() != cause.reverse_claim_ids()
                        })
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh proof causes differ from owner manifest",
                    ));
                }
                let rows = validate_navigation_claim_refresh_cohort(&cohort, limits)?;
                verify_navigation_claim_refresh_dependency_cut(
                    &cohort,
                    &context.selected_sources,
                    limits,
                )?;
                if integrity.dependency_root != cohort.dependency_root()
                    || integrity.output_root != cohort.output_root()
                    || integrity.input_root != cohort.input_root()
                    || rows.cohort_bytes > limits.max_total_bytes
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh set contains an invalid sealed cohort",
                    ));
                }
                dependency_paths.extend(
                    cohort
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.path.as_str().to_owned()),
                );
                claim_refresh_cohorts.push(cohort);
            }
            if actual_keys != expected_keys {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh set omits a current stream owner",
                ));
            }
            let causes = manifest
                .causes()
                .iter()
                .map(|cause| DurableNavigationClaimRefreshCauseBindingV1 {
                    changed_dependency_path: cause.changed_dependency_path().as_str().to_owned(),
                    changed_dependency_sha256: cause.changed_dependency_sha256(),
                    reverse_claim_ids: cause.reverse_claim_ids().to_vec(),
                })
                .collect();
            claim_refresh_binding = Some(DurableNavigationClaimRefreshSetBindingV1 {
                owner_inventory_root: manifest.inventory_root(),
                closure_root: claim_closure_root,
                affected_owner_manifest_root: manifest.root(),
                reverse_claim_ids: manifest.reverse_claim_ids().to_vec(),
                causes,
                streams,
            });
        }

        if context
            .members
            .iter()
            .any(|member| !dependency_paths.contains(member.source_path.as_str()))
        {
            return Err(DurableError::Conflict(
                "navigation cohort set omits a committed event member dependency",
            ));
        }
        record_cohorts.sort_by(|left, right| {
            (left.source_path.as_str(), left.record_id.as_str())
                .cmp(&(right.source_path.as_str(), right.record_id.as_str()))
        });
        claim_cohorts.sort_by(|left, right| {
            (left.source_path().as_str(), left.record_id())
                .cmp(&(right.source_path().as_str(), right.record_id()))
        });
        claim_refresh_cohorts.sort_by(|left, right| {
            (left.source_path().as_str(), left.record_id())
                .cmp(&(right.source_path().as_str(), right.record_id()))
        });
        let manifest_raw = navigation_mixed_event_cohort_manifest_v5(
            &context.event_id,
            context.event_sequence,
            member_root,
            &record_cohorts,
            &claim_cohorts,
            &claim_refresh_cohorts,
            claim_binding.as_ref(),
            claim_refresh_binding.as_ref(),
            claim_empty_closure_binding.as_ref(),
            claim_closure_root,
            limits.max_row_bytes as usize,
        )?;
        let set_root = Digest256::of_bytes(&manifest_raw);
        Ok(Self {
            event_id: context.event_id.clone(),
            event_sequence: context.event_sequence,
            source_head_sequence: context.source_head_sequence,
            member_root,
            set_root,
            manifest_raw,
            record_cohorts,
            claim_cohorts,
            claim_refresh_cohorts,
            claim_binding,
            claim_refresh_binding,
            claim_empty_closure_binding,
            claim_navigation_closure_root: Some(claim_closure_root),
        })
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    pub fn source_head_sequence(&self) -> u64 {
        self.source_head_sequence
    }

    pub fn member_root(&self) -> Digest256 {
        self.member_root
    }

    pub fn set_root(&self) -> Digest256 {
        self.set_root
    }

    pub fn manifest_raw(&self) -> &[u8] {
        &self.manifest_raw
    }

    pub fn cohorts(&self) -> &[NavigationRecordEventCohortV1] {
        &self.record_cohorts
    }

    pub fn claim_cohorts(&self) -> &[NavigationClaimEventCohortV1] {
        &self.claim_cohorts
    }

    pub fn claim_dependency_refresh_cohorts(
        &self,
    ) -> &[tos_compiler::source_navigation_claim_event::NavigationClaimDependencyRefreshCohortV1]
    {
        &self.claim_refresh_cohorts
    }

    fn is_claim_set(&self) -> bool {
        !self.claim_cohorts.is_empty()
            || !self.claim_refresh_cohorts.is_empty()
            || self.claim_binding.is_some()
            || self.claim_refresh_binding.is_some()
            || self.claim_empty_closure_binding.is_some()
    }
}

/// One bounded step through the durable event cursor. The cursor persists
/// after each successful step, so callers may resume a backlog after process
/// restart without replacing the last committed state on a failed step.
#[derive(Clone, Debug)]
pub enum DurableNavigationEventAdvanceV2 {
    UpToDate {
        cursor_sequence: u64,
        source_head_sequence: u64,
    },
    Published {
        cursor_before: u64,
        source_head_sequence: u64,
        apply: DurableNavigationEventApply,
    },
    /// The next committed event is exact, but this consumer has no owner
    /// contract proving that it cannot affect navigation. The cursor is left
    /// unchanged so a later producer or explicit rebuild can resume safely.
    Pending {
        cursor_sequence: u64,
        source_head_sequence: u64,
        event_sequence: u64,
        reason: DurableNavigationEventPendingReasonV2,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableNavigationEventPendingReasonV2 {
    NoNavigationEffectProof,
    UnsupportedEventKind,
    EventNotCurrentSourceHead,
}

/// Exact first cut for an incremental producer. The seed includes the event
/// path and every committed source member, but deliberately omits unrelated
/// source dependencies so the caller can derive a bounded request list.
#[derive(Clone, Debug)]
pub enum DurableNavigationEventSeedV1 {
    UpToDate {
        cursor_sequence: u64,
        source_head_sequence: u64,
    },
    Pending {
        cursor_sequence: u64,
        source_head_sequence: u64,
        event_sequence: u64,
        reason: DurableNavigationEventPendingReasonV2,
    },
    Ready(DurableNavigationEventContextV1),
}

/// Result of a bounded backlog drain. Every entry in `published` has already
/// committed its own cursor compare-and-set, so a later failure or process
/// restart resumes from `cursor_sequence` without replaying earlier work.
#[derive(Clone, Debug)]
pub struct DurableNavigationEventBacklogV2 {
    pub cursor_before: u64,
    pub cursor_sequence: u64,
    pub source_head_sequence: u64,
    pub published: Vec<DurableNavigationEventApply>,
    pub stop: DurableNavigationEventBacklogStopV2,
}

#[derive(Clone, Debug)]
pub enum DurableNavigationEventBacklogStopV2 {
    UpToDate,
    StepBudgetReached,
    Pending {
        event_sequence: u64,
        reason: DurableNavigationEventPendingReasonV2,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DurableNavigationAdjacencyCertificateReadV1 {
    edge_count: u64,
    edge_digest: Digest256,
}

impl DurableNavigationAdjacencyCertificateReadV1 {
    pub(crate) fn edge_count(&self) -> u64 {
        self.edge_count
    }

    pub(crate) fn edge_digest(&self) -> Digest256 {
        self.edge_digest
    }
}

#[derive(Clone, Debug)]
pub struct DurableNavigationBootstrapApplyV1 {
    pub through_sequence: u64,
    pub bootstrap_root: Digest256,
    pub replayed: bool,
}

#[derive(Clone, Debug)]
pub struct DurableNavigationEventDependencyReadV1 {
    pub path: String,
    pub role: String,
    pub sha256: Option<Digest256>,
    pub size_bytes: Option<u64>,
    pub absent_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationDependencyRefreshCauseReadV1 {
    pub path: String,
    pub sha256: Digest256,
}

/// One stored record-local cohort. This read does not claim whole-graph
/// completeness; consumers must bind it to their own cursor/currentness gate.
#[derive(Clone, Debug)]
pub struct DurableNavigationEventCohortReadV1 {
    /// CMD outbox event identity (`{domain}:{commit_seq}`).
    pub event_id: String,
    pub event_sequence: u64,
    /// Authored ToS provenance identity read from the exact v2 event bytes.
    pub source_event_id: String,
    pub source_event_version: u64,
    pub predecessor_source_revision: Digest256,
    pub source_revision: Digest256,
    pub basis_kind: String,
    pub basis_dependency_path: Option<String>,
    pub basis_dependency_sha256: Option<Digest256>,
    pub basis_dependency_causes: Vec<DurableNavigationDependencyRefreshCauseReadV1>,
    pub basis_dependency_causes_root: Option<Digest256>,
    pub basis_history_path: Option<String>,
    pub basis_history_sha256: Option<Digest256>,
    pub basis_claim_id: Option<String>,
    pub basis_revision_proof_root: Option<Digest256>,
    claim_source_line: Option<u64>,
    claim_trace_sha256: Option<Digest256>,
    claim_event_set_root: Option<Digest256>,
    claim_mutation_receipt_path: Option<String>,
    claim_mutation_receipt_sha256: Option<Digest256>,
    claim_stream_sha256: Option<Digest256>,
    claim_owner_inventory_root: Option<Digest256>,
    claim_stream_rows_scanned: Option<u64>,
    claim_event_basis_root: Option<Digest256>,
    claim_navigation_manifest_root: Option<Digest256>,
    pub source_path: String,
    pub record_id: String,
    pub predecessor_cohort_root: Option<Digest256>,
    pub dependency_root: Digest256,
    pub input_root: Digest256,
    pub output_root: Digest256,
    pub dependencies: Vec<DurableNavigationEventDependencyReadV1>,
    pub nodes: Vec<Value>,
    pub node_visibility: Vec<bool>,
    pub edges: Vec<Value>,
    pub edge_visibility: Vec<bool>,
    pub diagnostics: Vec<Value>,
}

/// A global navigation read is usable only after the event cursor, full
/// bootstrap receipt, selected source generation, and all stored cohorts
/// agree under one repeatable-read cut.
#[derive(Clone, Debug)]
pub struct DurableNavigationCurrentIndexV1 {
    status: DurableNavigationCurrentIndexStatusV1,
}

impl DurableNavigationCurrentIndexV1 {
    pub fn status(&self) -> &DurableNavigationCurrentIndexStatusV1 {
        &self.status
    }
}

#[derive(Clone, Debug)]
pub enum DurableNavigationCurrentIndexStatusV1 {
    Ready(DurableNavigationCurrentIndexReadyV1),
    NotReady(DurableNavigationCurrentIndexNotReadyV1),
}

#[derive(Clone, Debug)]
pub struct DurableNavigationCurrentIndexReadyV1 {
    domain_head_sequence: u64,
    coverage_sequence: u64,
    coverage_root: Digest256,
    adjacency_root: Digest256,
    source_revision: Digest256,
    index_root: Digest256,
    owner_binding: DurableNavigationCurrentIndexOwnerBindingV1,
    /// Exact bootstrap carriers remain separate from event-local cohorts so
    /// consumers can build node and adjacency lookups without reserializing.
    base_rows: Vec<DurableNavigationBaseRowReadV1>,
    header_raw: Vec<u8>,
    header_sha256: Digest256,
    cohorts: Vec<DurableNavigationEventCohortReadV1>,
}

impl DurableNavigationCurrentIndexReadyV1 {
    pub(crate) fn domain_head_sequence(&self) -> u64 {
        self.domain_head_sequence
    }

    pub(crate) fn coverage_sequence(&self) -> u64 {
        self.coverage_sequence
    }

    pub(crate) fn coverage_root(&self) -> Digest256 {
        self.coverage_root
    }

    pub(crate) fn adjacency_root(&self) -> Digest256 {
        self.adjacency_root
    }

    pub(crate) fn source_revision(&self) -> Digest256 {
        self.source_revision
    }

    pub(crate) fn index_root(&self) -> Digest256 {
        self.index_root
    }

    pub(crate) fn owner_binding(&self) -> &DurableNavigationCurrentIndexOwnerBindingV1 {
        &self.owner_binding
    }

    pub(crate) fn base_rows(&self) -> &[DurableNavigationBaseRowReadV1] {
        &self.base_rows
    }

    pub(crate) fn header_raw(&self) -> &[u8] {
        &self.header_raw
    }

    pub(crate) fn header_sha256(&self) -> Digest256 {
        self.header_sha256
    }

    pub(crate) fn cohorts(&self) -> &[DurableNavigationEventCohortReadV1] {
        &self.cohorts
    }
}

/// Metadata-only result used by bounded read-model adapters. A Ready value is
/// sealed by the owner binding and current CMD/coverage facts; it contains no
/// carrier or dependency rows.
#[derive(Clone, Debug)]
pub(crate) enum DurableNavigationCurrentnessV1 {
    Ready(DurableNavigationCurrentnessGuardV1),
    NotReady(DurableNavigationCurrentIndexNotReadyV1),
}

#[derive(Clone, Debug)]
pub(crate) struct DurableNavigationCurrentnessGuardV1 {
    domain_head_sequence: u64,
    coverage_sequence: u64,
    coverage_root: Digest256,
    adjacency_root: Digest256,
    cohort_root: Digest256,
    source_revision: Digest256,
    index_root: Digest256,
    audit_generation: u64,
    rights_version: u64,
    rights_allowed: bool,
    profile_id: String,
    profile_version: u32,
    source_contract_digest: Digest256,
    owner_binding: DurableNavigationCurrentIndexOwnerBindingV1,
    bootstrap_root: Digest256,
    header_raw: Vec<u8>,
    header_sha256: Digest256,
}

impl DurableNavigationCurrentnessGuardV1 {
    pub(crate) fn domain_head_sequence(&self) -> u64 {
        self.domain_head_sequence
    }
    pub(crate) fn coverage_sequence(&self) -> u64 {
        self.coverage_sequence
    }
    pub(crate) fn coverage_root(&self) -> Digest256 {
        self.coverage_root
    }
    pub(crate) fn adjacency_root(&self) -> Digest256 {
        self.adjacency_root
    }
    pub(crate) fn source_revision(&self) -> Digest256 {
        self.source_revision
    }
    pub(crate) fn index_root(&self) -> Digest256 {
        self.index_root
    }
    pub(crate) fn audit_generation(&self) -> u64 {
        self.audit_generation
    }
    pub(crate) fn rights_version(&self) -> u64 {
        self.rights_version
    }
    pub(crate) fn rights_allowed(&self) -> bool {
        self.rights_allowed
    }
    pub(crate) fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub(crate) fn profile_version(&self) -> u32 {
        self.profile_version
    }
    pub(crate) fn source_contract_digest(&self) -> Digest256 {
        self.source_contract_digest
    }
    pub(crate) fn owner_binding(&self) -> &DurableNavigationCurrentIndexOwnerBindingV1 {
        &self.owner_binding
    }
    pub(crate) fn bootstrap_root(&self) -> Digest256 {
        self.bootstrap_root
    }
    pub(crate) fn header_raw(&self) -> &[u8] {
        &self.header_raw
    }
    pub(crate) fn header_sha256(&self) -> Digest256 {
        self.header_sha256
    }
}

/// Versioned proof that a complete navigation generation was published at a
/// cursor, even when CMD has since committed a successor event. This proves
/// the stored publication's own coverage and current access fence; it does
/// not claim that the published carriers include source events after its
/// cursor.
#[derive(Clone, Debug)]
pub(crate) enum DurableNavigationPublishedGenerationV2 {
    Ready(DurableNavigationPublishedGenerationGuardV2),
    NotReady(DurableNavigationCurrentIndexNotReadyV1),
}

#[derive(Clone, Debug)]
pub(crate) struct DurableNavigationPublishedGenerationGuardV2 {
    inner: DurableNavigationCurrentnessGuardV1,
}

impl DurableNavigationPublishedGenerationGuardV2 {
    pub(crate) fn domain_head_sequence(&self) -> u64 {
        self.inner.domain_head_sequence()
    }
    pub(crate) fn published_sequence(&self) -> u64 {
        self.inner.coverage_sequence()
    }
    pub(crate) fn published_root(&self) -> Digest256 {
        self.inner.index_root()
    }
    pub(crate) fn coverage_root(&self) -> Digest256 {
        self.inner.coverage_root()
    }
    pub(crate) fn adjacency_root(&self) -> Digest256 {
        self.inner.adjacency_root()
    }
    pub(crate) fn source_revision(&self) -> Digest256 {
        self.inner.source_revision()
    }
    pub(crate) fn rights_allowed(&self) -> bool {
        self.inner.rights_allowed()
    }
    pub(crate) fn rights_version(&self) -> u64 {
        self.inner.rights_version()
    }
    pub(crate) fn audit_generation(&self) -> u64 {
        self.inner.audit_generation()
    }
    pub(crate) fn profile_id(&self) -> &str {
        self.inner.profile_id()
    }
    pub(crate) fn profile_version(&self) -> u32 {
        self.inner.profile_version()
    }
    pub(crate) fn source_contract_digest(&self) -> Digest256 {
        self.inner.source_contract_digest()
    }
    pub(crate) fn owner_binding(&self) -> &DurableNavigationCurrentIndexOwnerBindingV1 {
        self.inner.owner_binding()
    }
    pub(crate) fn bootstrap_root(&self) -> Digest256 {
        self.inner.bootstrap_root()
    }
    pub(crate) fn header_raw(&self) -> &[u8] {
        self.inner.header_raw()
    }
    pub(crate) fn header_sha256(&self) -> Digest256 {
        self.inner.header_sha256()
    }
}

/// A full-source row captured from the selected source-navigation projection.
/// `raw` is the exact UTF-8 carrier supplied by its producer; its digest is
/// always computed over those bytes, never over a serde reserialization.
#[derive(Clone, Debug)]
pub struct DurableNavigationBootstrapRowV1 {
    pub kind: DurableNavigationBootstrapRowKindV1,
    pub id: String,
    pub owner_source_path: String,
    pub owner_record_id: String,
    /// Shared compiler visibility rule for the exact node and edge carriers.
    /// Rights and diagnostics have no visibility bit and must use `None`.
    pub visible: Option<bool>,
    pub raw: Vec<u8>,
}

/// Exact selected source member that owns one or more bootstrap carriers.
#[derive(Clone, Debug)]
pub struct DurableNavigationBootstrapOwnerSourceV1 {
    pub source_path: String,
    pub sha256: Digest256,
    pub size_bytes: u64,
}

/// Exact original rights row retained with its source-order ordinal. This is
/// a seal witness only; query-facing rights carriers remain their own rows.
#[derive(Clone, Debug)]
pub struct DurableNavigationOriginalRightWitnessV1 {
    pub ordinal: u64,
    pub raw: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DurableNavigationBootstrapRowKindV1 {
    Node,
    Edge,
    Right,
    Diagnostic,
}

impl DurableNavigationBootstrapRowKindV1 {
    fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Edge => "edge",
            Self::Right => "right",
            Self::Diagnostic => "diagnostic",
        }
    }

    fn id_field(self) -> Option<&'static str> {
        match self {
            Self::Node => Some("node_id"),
            Self::Edge => Some("edge_id"),
            Self::Right => Some("rights_id"),
            Self::Diagnostic => None,
        }
    }
}

/// One bounded exact carrier returned by the global current-index read.
/// Endpoints and source references are parsed from `raw` in the same read cut.
#[derive(Clone, Debug)]
pub struct DurableNavigationBaseRowReadV1 {
    pub kind: DurableNavigationBootstrapRowKindV1,
    pub id: String,
    pub owner_source_path: String,
    pub owner_record_id: String,
    pub active: bool,
    pub visible: Option<bool>,
    pub raw: Vec<u8>,
    pub sha256: Digest256,
    pub from_id: Option<String>,
    pub to_id: Option<String>,
    pub source_refs: Vec<String>,
}

/// Sealed input to the full-source navigation bootstrap. It can only be made
/// from the actual managed selected parent and its exact original receipt.
#[derive(Clone, Debug)]
pub struct DurableNavigationBootstrapPackageV1 {
    domain: String,
    owner_binding: DurableNavigationCurrentIndexOwnerBindingV1,
    through_sequence: u64,
    source_revision: Digest256,
    header_raw: Vec<u8>,
    header_sha256: Digest256,
    rows: Vec<DurableNavigationBaseRowReadV1>,
    owner_sources: BTreeMap<String, DurableNavigationBootstrapOwnerSourceV1>,
    dependencies: BTreeMap<(String, String), BTreeSet<String>>,
    bootstrap_root: Digest256,
}

impl DurableNavigationBootstrapPackageV1 {
    pub(crate) fn checked_from_selected<P: ManagedSelectedProof>(
        domain: &str,
        selected: &ManagedAgentSelectedParent<P>,
        profile: &DurableProjectionProfile,
        header: &NavigationHeaderClaim,
        rows: Vec<DurableNavigationBootstrapRowV1>,
        original_rights: Vec<DurableNavigationOriginalRightWitnessV1>,
        owner_sources: Vec<DurableNavigationBootstrapOwnerSourceV1>,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Self> {
        validate_identifier(
            domain,
            super::MAX_DOMAIN_BYTES,
            "navigation bootstrap domain",
        )?;
        profile.validate()?;
        let limits = limits.validate()?;
        let expectation = selected.selection_expectation();
        let receipt: &NavigationOriginalReceipt = selected.navigation_original_receipt();
        if !expectation.complete
            || expectation.owner_receipt_id.is_empty()
            || expectation.navigation_original_root_sha256.is_none()
            || expectation.through_commit_seq != selected.generation().commit_seq()
            || expectation.membership_root != receipt.membership_root
            || expectation.source_cut != receipt.source_cut
            || expectation.descriptor_sha256 != receipt.descriptor_sha256
            || expectation
                .source_scopes
                .iter()
                .all(|scope| scope.source_graph != receipt.source_graph)
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap selected source binding differs",
            ));
        }
        let header_digest = Digest256::of_bytes(&header.raw_json);
        let receipt_header_digest = Digest256::from_hex(&receipt.header_sha256)
            .map_err(|_| DurableError::Corrupt("navigation original header digest"))?;
        if header_digest != receipt_header_digest
            || header.expected_sha256.trim_start_matches("sha256:") != receipt.header_sha256
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap detached header differs from selected receipt",
            ));
        }
        let header_value = strict_json_value(&header.raw_json, limits.max_row_bytes)?;
        let counts = header_value
            .get("counts")
            .ok_or(DurableError::Invalid("navigation bootstrap header counts"))?;
        if counts.get("nodes").and_then(Value::as_u64) != Some(receipt.nodes)
            || counts.get("edges").and_then(Value::as_u64) != Some(receipt.edges)
            || counts.get("rights").and_then(Value::as_u64) != Some(receipt.rights)
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap header counts differ",
            ));
        }
        if original_rights.len() as u64 != receipt.rights {
            return Err(DurableError::Conflict(
                "navigation bootstrap original rights count differs",
            ));
        }
        let mut rights_bytes = 0u64;
        for (expected_ordinal, witness) in original_rights.iter().enumerate() {
            if witness.ordinal != expected_ordinal as u64
                || witness.raw.len() > limits.max_row_bytes
            {
                return Err(DurableError::Invalid(
                    "navigation bootstrap original rights ordinal or size",
                ));
            }
            let value = strict_json_value(&witness.raw, limits.max_row_bytes)?;
            if value
                .get("rights_id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(DurableError::Invalid(
                    "navigation bootstrap original rights carrier",
                ));
            }
            rights_bytes =
                rights_bytes
                    .checked_add(witness.raw.len() as u64)
                    .ok_or(DurableError::Refused(
                        "navigation bootstrap rights byte budget",
                    ))?;
        }
        let rights_refs = original_rights
            .iter()
            .map(|witness| witness.raw.as_slice())
            .collect::<Vec<_>>();
        if tos_compiler::navigation_original_rights_root(&rights_refs) != receipt.rights_root_sha256
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap original rights root differs",
            ));
        }
        if rows.len() > limits.max_current_rows {
            return Err(DurableError::Refused("navigation bootstrap row budget"));
        }
        let mut charged_bytes = (header.raw_json.len() as u64)
            .checked_add(rights_bytes)
            .ok_or(DurableError::Refused("navigation bootstrap byte budget"))?;
        let mut identities = BTreeSet::new();
        let mut carriers =
            BTreeMap::<(DurableNavigationBootstrapRowKindV1, String), (Digest256, Vec<u8>)>::new();
        let mut base_rows = Vec::with_capacity(rows.len());
        let mut dependencies = BTreeMap::<(String, String), BTreeSet<String>>::new();
        for row in rows {
            validate_identifier(
                &row.id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation bootstrap row ID",
            )?;
            validate_relative_path(&row.owner_source_path)?;
            validate_identifier(
                &row.owner_record_id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation bootstrap owner record ID",
            )?;
            if row.raw.len() > limits.max_row_bytes
                || !identities.insert((
                    row.kind,
                    row.id.clone(),
                    row.owner_source_path.clone(),
                    row.owner_record_id.clone(),
                ))
            {
                return Err(DurableError::Invalid(
                    "navigation bootstrap row identity or size",
                ));
            }
            if matches!(
                row.kind,
                DurableNavigationBootstrapRowKindV1::Node
                    | DurableNavigationBootstrapRowKindV1::Edge
            ) != row.visible.is_some()
            {
                return Err(DurableError::Invalid(
                    "navigation bootstrap visibility witness",
                ));
            }
            charged_bytes = charged_bytes
                .checked_add(row.raw.len() as u64)
                .ok_or(DurableError::Refused("navigation bootstrap byte budget"))?;
            if charged_bytes > limits.max_total_bytes {
                return Err(DurableError::Refused("navigation bootstrap byte budget"));
            }
            let value = strict_json_value(&row.raw, limits.max_row_bytes)?;
            if let Some(field) = row.kind.id_field()
                && value.get(field).and_then(Value::as_str) != Some(row.id.as_str())
            {
                return Err(DurableError::Invalid(
                    "navigation bootstrap carrier ID differs",
                ));
            }
            let (mut source_refs, mut dependencies_for_owner) =
                bootstrap_ref_parts(&value, "navigation bootstrap source ref")?;
            source_refs.insert(row.owner_source_path.clone());
            dependencies_for_owner.insert(row.owner_source_path.clone());
            let sha256 = Digest256::of_bytes(&row.raw);
            if let Some((prior_digest, prior_raw)) = carriers.get(&(row.kind, row.id.clone())) {
                if *prior_digest != sha256 || prior_raw != &row.raw {
                    return Err(DurableError::Conflict(
                        "navigation bootstrap shared carrier bytes differ",
                    ));
                }
            } else {
                carriers.insert((row.kind, row.id.clone()), (sha256, row.raw.clone()));
            }
            let owner = (row.owner_source_path.clone(), row.owner_record_id.clone());
            dependencies
                .entry(owner)
                .or_default()
                .extend(dependencies_for_owner);
            let source_refs = source_refs.into_iter().collect::<Vec<_>>();
            base_rows.push(DurableNavigationBaseRowReadV1 {
                kind: row.kind,
                id: row.id,
                owner_source_path: row.owner_source_path,
                owner_record_id: row.owner_record_id,
                active: true,
                visible: row.visible,
                sha256,
                from_id: value
                    .get("from_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                to_id: value
                    .get("to_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                source_refs,
                raw: row.raw,
            });
        }
        for (kind, expected) in [
            (DurableNavigationBootstrapRowKindV1::Node, receipt.nodes),
            (DurableNavigationBootstrapRowKindV1::Edge, receipt.edges),
        ] {
            let actual = carriers
                .keys()
                .filter(|(row_kind, _)| *row_kind == kind)
                .count();
            if actual as u64 != expected {
                return Err(DurableError::Conflict(
                    "navigation bootstrap carrier count differs from selected receipt",
                ));
            }
        }
        let projected_right_count = carriers
            .keys()
            .filter(|(row_kind, _)| *row_kind == DurableNavigationBootstrapRowKindV1::Right)
            .count() as u64;
        for (kind, expected_root, label) in [
            (
                DurableNavigationBootstrapRowKindV1::Node,
                &receipt.node_input_root_sha256,
                "navigation bootstrap node input root",
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                &receipt.edge_input_root_sha256,
                "navigation bootstrap edge input root",
            ),
        ] {
            let mut root = Digest256Hasher::new();
            for ((row_kind, id), (sha256, _)) in &carriers {
                if *row_kind == kind {
                    navigation_hash_part(&mut root, id.as_bytes());
                    root.update(sha256.as_bytes());
                }
            }
            if root.finalize().to_hex() != *expected_root {
                return Err(DurableError::Conflict(label));
            }
        }
        let mut owner_source_by_path = BTreeMap::new();
        for owner_source in owner_sources {
            validate_relative_path(&owner_source.source_path)?;
            if owner_source.size_bytes == 0
                || owner_source.size_bytes > limits.max_total_bytes
                || owner_source_by_path
                    .insert(owner_source.source_path.clone(), owner_source)
                    .is_some()
            {
                return Err(DurableError::Invalid("navigation bootstrap owner source"));
            }
        }
        let required_owner_paths = base_rows
            .iter()
            .map(|row| row.owner_source_path.as_str())
            .collect::<BTreeSet<_>>();
        if required_owner_paths
            .iter()
            .any(|path| !owner_source_by_path.contains_key(*path))
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap owner source metadata is incomplete",
            ));
        }
        if dependencies
            .values()
            .flat_map(BTreeSet::iter)
            .any(|path| !owner_source_by_path.contains_key(path))
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap dependency owner metadata is incomplete",
            ));
        }
        base_rows.sort_by(|left, right| {
            (
                left.kind,
                left.id.as_str(),
                left.owner_source_path.as_str(),
                left.owner_record_id.as_str(),
            )
                .cmp(&(
                    right.kind,
                    right.id.as_str(),
                    right.owner_source_path.as_str(),
                    right.owner_record_id.as_str(),
                ))
        });
        let mut owner_binding = navigation_owner_binding(domain, selected, profile)?;
        owner_binding.navigation_header = header_value;
        owner_binding.navigation_projected_rights = projected_right_count;
        let source_revision = Digest256::from_hex(&expectation.membership_root)
            .map_err(|_| DurableError::Corrupt("navigation bootstrap membership root"))?;
        let bootstrap_root = navigation_bootstrap_root(
            &owner_binding,
            &header.raw_json,
            &base_rows,
            &owner_source_by_path,
            limits.max_total_bytes as usize,
        )?;
        Ok(Self {
            domain: domain.to_owned(),
            owner_binding,
            through_sequence: expectation.through_commit_seq,
            source_revision,
            header_raw: header.raw_json.clone(),
            header_sha256: header_digest,
            rows: base_rows,
            owner_sources: owner_source_by_path,
            dependencies,
            bootstrap_root,
        })
    }
}

/// Current-owner facts carried into a global read model. The navigation
/// receipt and detached header come from the selected immutable model; the
/// managed generation and membership facts are rechecked against CMD under
/// the same database cut as the index rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationCurrentIndexOwnerBindingV1 {
    pub domain: String,
    pub owner_receipt_id: String,
    pub managed_generation_digest: Digest256,
    pub managed_generation_commit_sequence: u64,
    pub managed_generation_member_count: u64,
    pub managed_agent_definition_digest: Digest256,
    /// Immutable cohort bootstrap retained by CMD's source_membership columns.
    /// The selected model and current generation have independent commitments.
    pub source_membership_root: Digest256,
    pub source_membership_count: u64,
    pub source_projection_digest: Digest256,
    pub selected_software_profile_id: String,
    pub selected_software_profile_version: u32,
    pub selected_software_contract_digest: Digest256,
    pub selected_model_digest: Digest256,
    /// Exact complete expectation admitted by the bootstrap cold verifier.
    pub selected_expectation_digest: Option<Digest256>,
    pub selected_model_abi: String,
    pub selected_selection_profile: String,
    pub selected_descriptor_digest: Digest256,
    pub selected_source_cut: String,
    pub selected_membership_root: Digest256,
    pub selected_index_generation: String,
    pub selected_route_map_version: String,
    pub selected_reader_abi: String,
    pub selected_authority_boundary: String,
    pub source_graph: String,
    pub navigation_original_root: Digest256,
    /// Complete root-authenticated admission metadata, retained at bootstrap.
    /// Older publications may omit it; bounded native reads require a rebuild
    /// into a new consumer before selection, never a silent model rescan.
    pub navigation_original_receipt: Option<NavigationOriginalReceipt>,
    pub navigation_header_sha256: Digest256,
    pub navigation_header: Value,
    pub navigation_node_input_root: Digest256,
    pub navigation_edge_input_root: Digest256,
    pub navigation_nodes: u64,
    pub navigation_edges: u64,
    /// Rights count retained in the original navigation receipt and header.
    pub navigation_rights: u64,
    /// Unique rights IDs in the normalized query-facing carrier table.
    pub navigation_projected_rights: u64,
}

#[derive(Clone, Debug)]
pub struct DurableNavigationCurrentIndexNotReadyV1 {
    pub domain_head_sequence: u64,
    pub cursor_sequence: Option<u64>,
    pub reason: DurableNavigationCurrentIndexNotReadyReasonV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableNavigationCurrentIndexNotReadyReasonV1 {
    NoConsumer,
    ProfileMismatch,
    CursorBehindHead,
    CoverageAbsent,
    SourceIncomplete,
    ManagedGenerationMismatch,
    MembershipMismatch,
    HeaderMismatch,
    BootstrapCoverageMismatch,
    AdjacencyCoverageMismatch,
    CoverageChainMismatch,
    IndexRootMismatch,
    BudgetExceeded,
}

pub(crate) fn navigation_owner_binding<P: ManagedSelectedProof>(
    domain: &str,
    selected: &ManagedAgentSelectedParent<P>,
    profile: &DurableProjectionProfile,
) -> DurableResult<DurableNavigationCurrentIndexOwnerBindingV1> {
    selected
        .source_proof()
        .validate()
        .map_err(|_| DurableError::Conflict("navigation managed source proof is invalid"))?;
    let expectation = selected.selection_expectation();
    if selected.selection_profile() != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
        || !tos_foundation::is_managed_knowledge_model_abi(&expectation.model_abi)
    {
        return Err(DurableError::Conflict(
            "navigation selected model profile is unsupported",
        ));
    }
    let receipt = selected.navigation_original_receipt();
    let bootstrap_membership = selected.generation().cohort().initial_membership();
    let digest =
        |value: &str, label| Digest256::from_hex(value).map_err(|_| DurableError::Corrupt(label));
    Ok(DurableNavigationCurrentIndexOwnerBindingV1 {
        domain: domain.to_owned(),
        owner_receipt_id: expectation.owner_receipt_id.clone(),
        managed_generation_digest: selected.generation().digest(),
        managed_generation_commit_sequence: selected.generation().commit_seq(),
        managed_generation_member_count: selected.generation().member_count(),
        managed_agent_definition_digest: selected.generation().agent_definition_digest(),
        source_membership_root: bootstrap_membership.digest,
        source_membership_count: bootstrap_membership.count,
        source_projection_digest: digest(
            selected.source_proof().native_inventory_root(),
            "navigation source projection digest",
        )?,
        selected_software_profile_id: profile.id.clone(),
        selected_software_profile_version: profile.version,
        selected_software_contract_digest: profile.source_contract,
        selected_model_digest: digest(
            &expectation.model_sha256,
            "navigation selected model digest",
        )?,
        selected_expectation_digest: Some(navigation_selected_expectation_digest(expectation)?),
        selected_model_abi: expectation.model_abi.clone(),
        selected_selection_profile: selected.selection_profile().to_owned(),
        selected_descriptor_digest: digest(
            &expectation.descriptor_sha256,
            "navigation selected descriptor digest",
        )?,
        selected_source_cut: expectation.source_cut.clone(),
        selected_membership_root: digest(
            &expectation.membership_root,
            "navigation selected membership root",
        )?,
        selected_index_generation: expectation.index_generation.clone(),
        selected_route_map_version: expectation.route_map_version.clone(),
        selected_reader_abi: expectation.reader_abi.clone(),
        selected_authority_boundary: expectation.authority_boundary.clone(),
        source_graph: receipt.source_graph.clone(),
        navigation_original_receipt: Some(receipt.clone()),
        navigation_original_root: digest(
            expectation
                .navigation_original_root_sha256
                .as_deref()
                .ok_or(DurableError::Conflict(
                    "navigation selected original root is absent",
                ))?,
            "navigation original root",
        )?,
        navigation_header_sha256: digest(&receipt.header_sha256, "navigation header digest")?,
        navigation_header: Value::Null,
        navigation_node_input_root: digest(
            &receipt.node_input_root_sha256,
            "navigation node input root",
        )?,
        navigation_edge_input_root: digest(
            &receipt.edge_input_root_sha256,
            "navigation edge input root",
        )?,
        navigation_nodes: receipt.nodes,
        navigation_edges: receipt.edges,
        navigation_rights: receipt.rights,
        navigation_projected_rights: 0,
    })
}

pub(crate) fn navigation_selected_expectation_digest(
    expected: &tos_compiler::KnowledgeSelectedExpectation,
) -> DurableResult<Digest256> {
    let value = serde_json::to_value(expected)
        .map_err(|_| DurableError::Invalid("navigation selected expectation JSON"))?;
    let raw = canonical_value(&value, 1_048_576)?;
    let mut root = Digest256Hasher::new();
    root.update(b"tos-pg-navigation-admitted-expectation-v1");
    navigation_hash_part(&mut root, &raw);
    Ok(root.finalize())
}

fn navigation_owner_binding_json(binding: &DurableNavigationCurrentIndexOwnerBindingV1) -> Value {
    let mut value = json!({
        "domain":binding.domain,
        "owner_receipt_id":binding.owner_receipt_id,
        "managed_generation_digest":binding.managed_generation_digest.to_hex(),
        "managed_generation_commit_sequence":binding.managed_generation_commit_sequence,
        "managed_generation_member_count":binding.managed_generation_member_count,
        "managed_agent_definition_digest":binding.managed_agent_definition_digest.to_hex(),
        "source_membership_root":binding.source_membership_root.to_hex(),
        "source_membership_count":binding.source_membership_count,
        "source_projection_digest":binding.source_projection_digest.to_hex(),
        "selected_software_profile_id":binding.selected_software_profile_id,
        "selected_software_profile_version":binding.selected_software_profile_version,
        "selected_software_contract_digest":binding.selected_software_contract_digest.to_hex(),
        "selected_model_digest":binding.selected_model_digest.to_hex(),
        "selected_model_abi":binding.selected_model_abi,
        "selected_selection_profile":binding.selected_selection_profile,
        "selected_descriptor_digest":binding.selected_descriptor_digest.to_hex(),
        "selected_source_cut":binding.selected_source_cut,
        "selected_membership_root":binding.selected_membership_root.to_hex(),
        "selected_index_generation":binding.selected_index_generation,
        "selected_route_map_version":binding.selected_route_map_version,
        "selected_reader_abi":binding.selected_reader_abi,
        "selected_authority_boundary":binding.selected_authority_boundary,
        "source_graph":binding.source_graph,
        "navigation_original_root":binding.navigation_original_root.to_hex(),
        "navigation_header_sha256":binding.navigation_header_sha256.to_hex(),
        "navigation_header":binding.navigation_header,
        "navigation_node_input_root":binding.navigation_node_input_root.to_hex(),
        "navigation_edge_input_root":binding.navigation_edge_input_root.to_hex(),
        "navigation_nodes":binding.navigation_nodes,
        "navigation_edges":binding.navigation_edges,
        "navigation_rights":binding.navigation_rights,
        "navigation_projected_rights":binding.navigation_projected_rights
    });
    if let Some(digest) = binding.selected_expectation_digest {
        value["selected_expectation_sha256"] = json!(digest.to_hex());
    }
    if let Some(receipt) = &binding.navigation_original_receipt {
        value["navigation_original_receipt"] = json!(receipt);
    }
    value
}

fn navigation_bootstrap_root(
    binding: &DurableNavigationCurrentIndexOwnerBindingV1,
    header_raw: &[u8],
    rows: &[DurableNavigationBaseRowReadV1],
    owner_sources: &BTreeMap<String, DurableNavigationBootstrapOwnerSourceV1>,
    max_bytes: usize,
) -> DurableResult<Digest256> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-bootstrap-v1");
    let binding = canonical_value(&navigation_owner_binding_json(binding), max_bytes)?;
    navigation_hash_part(&mut hash, &binding);
    navigation_hash_part(&mut hash, header_raw);
    for source in owner_sources.values() {
        navigation_hash_part(&mut hash, source.source_path.as_bytes());
        navigation_hash_part(&mut hash, source.sha256.to_hex().as_bytes());
        navigation_hash_part(&mut hash, &source.size_bytes.to_be_bytes());
    }
    for row in rows {
        navigation_hash_part(&mut hash, row.kind.as_str().as_bytes());
        navigation_hash_part(&mut hash, row.id.as_bytes());
        navigation_hash_part(&mut hash, row.owner_source_path.as_bytes());
        navigation_hash_part(&mut hash, row.owner_record_id.as_bytes());
        navigation_hash_part(
            &mut hash,
            &[match row.visible {
                Some(false) => 0,
                Some(true) => 1,
                None => 2,
            }],
        );
        navigation_hash_part(&mut hash, row.sha256.to_hex().as_bytes());
        navigation_hash_part(&mut hash, &row.raw);
    }
    Ok(hash.finalize())
}

fn navigation_hash_part(hash: &mut Digest256Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}

fn navigation_json_text(value: &Value, field: &'static str) -> DurableResult<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(DurableError::Corrupt(
            "navigation owner binding field absent",
        ))
}

fn navigation_json_u64(value: &Value, field: &'static str) -> DurableResult<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(DurableError::Corrupt(
            "navigation owner binding number absent",
        ))
}

fn navigation_json_digest(value: &Value, field: &'static str) -> DurableResult<Digest256> {
    Digest256::from_hex(&navigation_json_text(value, field)?)
        .map_err(|_| DurableError::Corrupt("navigation owner binding digest"))
}

fn navigation_owner_binding_from_json(
    value: &Value,
) -> DurableResult<DurableNavigationCurrentIndexOwnerBindingV1> {
    Ok(DurableNavigationCurrentIndexOwnerBindingV1 {
        domain: navigation_json_text(value, "domain")?,
        owner_receipt_id: navigation_json_text(value, "owner_receipt_id")?,
        managed_generation_digest: navigation_json_digest(value, "managed_generation_digest")?,
        managed_generation_commit_sequence: navigation_json_u64(
            value,
            "managed_generation_commit_sequence",
        )?,
        managed_generation_member_count: navigation_json_u64(
            value,
            "managed_generation_member_count",
        )?,
        managed_agent_definition_digest: navigation_json_digest(
            value,
            "managed_agent_definition_digest",
        )?,
        source_membership_root: navigation_json_digest(value, "source_membership_root")?,
        source_membership_count: navigation_json_u64(value, "source_membership_count")?,
        source_projection_digest: navigation_json_digest(value, "source_projection_digest")?,
        selected_software_profile_id: navigation_json_text(value, "selected_software_profile_id")?,
        selected_software_profile_version: u32::try_from(navigation_json_u64(
            value,
            "selected_software_profile_version",
        )?)
        .map_err(|_| DurableError::Corrupt("navigation software profile version"))?,
        selected_software_contract_digest: navigation_json_digest(
            value,
            "selected_software_contract_digest",
        )?,
        selected_model_digest: navigation_json_digest(value, "selected_model_digest")?,
        selected_expectation_digest: value
            .get("selected_expectation_sha256")
            .map(|_| navigation_json_digest(value, "selected_expectation_sha256"))
            .transpose()?,
        selected_model_abi: navigation_json_text(value, "selected_model_abi")?,
        selected_selection_profile: navigation_json_text(value, "selected_selection_profile")?,
        selected_descriptor_digest: navigation_json_digest(value, "selected_descriptor_digest")?,
        selected_source_cut: navigation_json_text(value, "selected_source_cut")?,
        selected_membership_root: navigation_json_digest(value, "selected_membership_root")?,
        selected_index_generation: navigation_json_text(value, "selected_index_generation")?,
        selected_route_map_version: navigation_json_text(value, "selected_route_map_version")?,
        selected_reader_abi: navigation_json_text(value, "selected_reader_abi")?,
        selected_authority_boundary: navigation_json_text(value, "selected_authority_boundary")?,
        source_graph: navigation_json_text(value, "source_graph")?,
        navigation_original_root: navigation_json_digest(value, "navigation_original_root")?,
        navigation_original_receipt: value
            .get("navigation_original_receipt")
            .filter(|receipt| !receipt.is_null())
            .map(|receipt| {
                serde_json::from_value(receipt.clone())
                    .map_err(|_| DurableError::Corrupt("navigation original admission receipt"))
            })
            .transpose()?,
        navigation_header_sha256: navigation_json_digest(value, "navigation_header_sha256")?,
        navigation_header: value
            .get("navigation_header")
            .cloned()
            .ok_or(DurableError::Corrupt("navigation selected header absent"))?,
        navigation_node_input_root: navigation_json_digest(value, "navigation_node_input_root")?,
        navigation_edge_input_root: navigation_json_digest(value, "navigation_edge_input_root")?,
        navigation_nodes: navigation_json_u64(value, "navigation_nodes")?,
        navigation_edges: navigation_json_u64(value, "navigation_edges")?,
        navigation_rights: navigation_json_u64(value, "navigation_rights")?,
        navigation_projected_rights: navigation_json_u64(value, "navigation_projected_rights")?,
    })
}

fn parse_bootstrap_kind(value: String) -> DurableResult<DurableNavigationBootstrapRowKindV1> {
    match value.as_str() {
        "node" => Ok(DurableNavigationBootstrapRowKindV1::Node),
        "edge" => Ok(DurableNavigationBootstrapRowKindV1::Edge),
        "right" => Ok(DurableNavigationBootstrapRowKindV1::Right),
        "diagnostic" => Ok(DurableNavigationBootstrapRowKindV1::Diagnostic),
        _ => Err(DurableError::Corrupt("navigation bootstrap row kind")),
    }
}

fn has_uri_scheme(reference: &str) -> bool {
    let Some((scheme, _)) = reference.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-'))
}

/// Source refs are evidence, not necessarily ToS paths: generated carriers
/// may retain external URLs or opaque identifiers. Only strict `source_ref`
/// / `path` owner fields and URI-free normalized relative members participate
/// in the reverse-dependency closure.
fn bootstrap_ref_parts(
    value: &Value,
    invalid_reason: &'static str,
) -> DurableResult<(BTreeSet<String>, BTreeSet<String>)> {
    let mut refs = BTreeSet::new();
    let mut dependencies = BTreeSet::new();
    for field in ["source_ref", "path"] {
        if let Some(raw) = value.get(field) {
            let path = raw.as_str().ok_or(DurableError::Invalid(invalid_reason))?;
            let source_path = path.split_once('#').map_or(path, |(path, _)| path);
            validate_relative_path(source_path)?;
            refs.insert(path.to_owned());
            dependencies.insert(source_path.to_owned());
        }
    }
    let bindings = value
        .get("source_ref_bindings")
        .map(|raw| raw.as_object().ok_or(DurableError::Invalid(invalid_reason)))
        .transpose()?;
    let bindings = crate::source_navigation_bootstrap::checked_source_reference_bindings(
        bindings
            .into_iter()
            .flat_map(|map| map.iter())
            .map(|(key, value)| (key.as_str(), value.as_str())),
        |reference| {
            value
                .get("source_refs")
                .and_then(Value::as_array)
                .is_some_and(|refs| refs.iter().any(|v| v.as_str() == Some(reference)))
        },
    )?;
    if let Some(raw) = value.get("source_refs") {
        let paths = raw
            .as_array()
            .ok_or(DurableError::Invalid(invalid_reason))?;
        for raw in paths {
            let reference = raw.as_str().ok_or(DurableError::Invalid(invalid_reason))?;
            if reference.is_empty() {
                return Err(DurableError::Invalid(invalid_reason));
            }
            refs.insert(reference.to_owned());
            if !has_uri_scheme(reference) {
                let bound = bindings
                    .get(reference)
                    .map(String::as_str)
                    .unwrap_or(reference);
                let source_path = bound.split_once('#').map_or(bound, |(path, _)| path);
                validate_relative_path(source_path)?;
                dependencies.insert(source_path.to_owned());
            }
        }
    }
    Ok((refs, dependencies))
}

fn bootstrap_source_refs(value: &Value, owner_source_path: &str) -> DurableResult<Vec<String>> {
    let (mut refs, _) = bootstrap_ref_parts(value, "navigation stored source ref")
        .map_err(|_| DurableError::Corrupt("navigation stored source ref"))?;
    refs.insert(owner_source_path.to_owned());
    Ok(refs.into_iter().collect())
}

#[cfg(test)]
mod bootstrap_reference_binding_tests {
    use super::*;

    #[test]
    fn navigation_reference_binding_seal_keeps_stable_identity() {
        let value = json!({
            "source_ref":"ToS/claims.jsonl",
            "source_refs":["tos.anchor.example", "ToS/evidence.md#part", "https://example.org/evidence"],
            "source_ref_bindings":{"tos.anchor.example":"ToS/anchors.jsonl"}
        });
        let (refs, paths) = bootstrap_ref_parts(&value, "test source refs").unwrap();
        assert!(refs.contains("tos.anchor.example"));
        assert!(refs.contains("https://example.org/evidence"));
        assert_eq!(
            paths,
            BTreeSet::from([
                "ToS/claims.jsonl".into(),
                "ToS/anchors.jsonl".into(),
                "ToS/evidence.md".into(),
            ])
        );
        let mut invalid = value;
        invalid["source_ref_bindings"] = json!({"tos.anchor.absent":"ToS/anchors.jsonl"});
        assert!(bootstrap_ref_parts(&invalid, "test source refs").is_err());
    }
}

fn navigation_not_ready(
    domain_head_sequence: u64,
    cursor_sequence: Option<u64>,
    reason: DurableNavigationCurrentIndexNotReadyReasonV1,
) -> DurableNavigationCurrentIndexV1 {
    DurableNavigationCurrentIndexV1 {
        status: DurableNavigationCurrentIndexStatusV1::NotReady(
            DurableNavigationCurrentIndexNotReadyV1 {
                domain_head_sequence,
                cursor_sequence,
                reason,
            },
        ),
    }
}

fn navigation_currentness_not_ready(
    domain_head_sequence: u64,
    cursor_sequence: Option<u64>,
    reason: DurableNavigationCurrentIndexNotReadyReasonV1,
) -> DurableNavigationCurrentnessV1 {
    DurableNavigationCurrentnessV1::NotReady(DurableNavigationCurrentIndexNotReadyV1 {
        domain_head_sequence,
        cursor_sequence,
        reason,
    })
}

fn navigation_current_index_root(
    bootstrap_root: Digest256,
    coverage_root: Digest256,
    adjacency_root: Digest256,
    cohort_root: Digest256,
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-current-index-v2");
    navigation_hash_part(&mut hash, bootstrap_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, coverage_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, adjacency_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, cohort_root.to_hex().as_bytes());
    hash.finalize()
}

/// Commit the bounded current owner-cohort table, not its unbounded event history.
/// The owner publishers advance this root in the same transaction as the cursor.
pub(super) fn navigation_current_cohort_root_in_tx(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Digest256> {
    let row_cap = i64::try_from(limits.max_current_cohorts.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation current cohort count"))?;
    let rows = tx.query(
        "SELECT row_to_json(c)::text AS row_json
           FROM cmd2_projection_navigation_cohort c
          WHERE c.domain=$1 AND c.consumer_id=$2
          ORDER BY c.source_path COLLATE \"C\",c.record_id COLLATE \"C\" LIMIT $3",
        &[&domain, &consumer_id, &row_cap],
    )?;
    if rows.len() > limits.max_current_cohorts {
        return Err(DurableError::Refused(
            "navigation current cohort count budget",
        ));
    }
    let row_json = rows
        .into_iter()
        .map(|row| row.get::<_, String>("row_json"))
        .collect::<Vec<_>>();
    navigation_current_cohort_root_from_row_json(&row_json, limits.max_total_bytes)
}

fn navigation_current_cohort_root_from_row_json(
    rows: &[String],
    max_total_bytes: u64,
) -> DurableResult<Digest256> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-current-cohorts-v1");
    let row_count = u64::try_from(rows.len())
        .map_err(|_| DurableError::Refused("navigation current cohort count"))?;
    navigation_hash_part(&mut hash, &row_count.to_be_bytes());
    let mut bytes = 0u64;
    let mut source_bytes = 0u64;
    let row_limit = usize::try_from(max_total_bytes)
        .map_err(|_| DurableError::Refused("navigation current cohort byte budget"))?;
    for raw in rows {
        source_bytes = source_bytes
            .checked_add(raw.len() as u64)
            .ok_or(DurableError::Refused("navigation current cohort bytes"))?;
        if source_bytes > max_total_bytes {
            return Err(DurableError::Refused(
                "navigation current cohort byte budget",
            ));
        }
        let value: Value = serde_json::from_str(raw)
            .map_err(|_| DurableError::Corrupt("navigation current cohort row JSON"))?;
        let canonical = canonical_value(&value, row_limit)?;
        bytes = bytes
            .checked_add(canonical.len() as u64)
            .ok_or(DurableError::Refused("navigation current cohort bytes"))?;
        if bytes > max_total_bytes {
            return Err(DurableError::Refused(
                "navigation current cohort byte budget",
            ));
        }
        navigation_hash_part(&mut hash, &canonical);
    }
    Ok(hash.finalize())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn navigation_verified_prefix_digest(
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    bootstrap_sequence: u64,
    bootstrap_root: Digest256,
    owner_binding_json: &str,
    sequence: u64,
    coverage_root: Digest256,
    cohort_root: Digest256,
    adjacency_root: Digest256,
    event_id: &str,
    source_revision: Digest256,
    input_root: Digest256,
    output_root: Digest256,
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-verified-prefix-v2");
    for part in [
        domain.as_bytes(),
        consumer_id.as_bytes(),
        profile.id.as_bytes(),
    ] {
        navigation_hash_part(&mut hash, part);
    }
    navigation_hash_part(&mut hash, profile.version.to_string().as_bytes());
    navigation_hash_part(&mut hash, profile.source_contract.to_hex().as_bytes());
    navigation_hash_part(&mut hash, &bootstrap_sequence.to_be_bytes());
    navigation_hash_part(&mut hash, bootstrap_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, owner_binding_json.as_bytes());
    navigation_hash_part(&mut hash, &sequence.to_be_bytes());
    navigation_hash_part(&mut hash, coverage_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, cohort_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, adjacency_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, event_id.as_bytes());
    navigation_hash_part(&mut hash, source_revision.to_hex().as_bytes());
    navigation_hash_part(&mut hash, input_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, output_root.to_hex().as_bytes());
    hash.finalize()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_navigation_verified_prefix_in_tx(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    bootstrap_sequence: u64,
    bootstrap_root: Digest256,
    owner_binding_json: &str,
    consumer: &postgres::Row,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    if consumer.get::<_, Option<i16>>("verified_prefix_version")
        != Some(VERIFIED_PREFIX_FORMAT_VERSION)
    {
        return Err(DurableError::Conflict(
            "navigation verified prefix format requires a full bootstrap",
        ));
    }
    let cursor = as_u64(consumer.get("cursor_seq"))?;
    let prefix_sequence = consumer
        .get::<_, Option<i64>>("verified_prefix_seq")
        .map(as_u64)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "navigation verified prefix is absent",
        ))?;
    let prefix_coverage_root = consumer
        .get::<_, Option<String>>("verified_prefix_coverage_root")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "navigation verified prefix root is absent",
        ))?;
    let prefix_cohort_root = consumer
        .get::<_, Option<String>>("verified_prefix_cohort_root")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "navigation verified cohort root is absent",
        ))?;
    let prefix_digest = consumer
        .get::<_, Option<String>>("verified_prefix_digest")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "navigation verified prefix digest is absent",
        ))?;
    let coverage_root = consumer
        .get::<_, Option<String>>("coverage_root")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Conflict("navigation coverage root is absent"))?;
    let adjacency_root = consumer
        .get::<_, Option<String>>("adjacency_root")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "navigation adjacency root is absent",
        ))?;
    let cohort_root = navigation_current_cohort_root_in_tx(tx, domain, consumer_id, limits)?;
    let event_id: String = consumer
        .get::<_, Option<String>>("last_event_id")
        .ok_or(DurableError::Conflict("navigation cursor event is absent"))?;
    let source_revision =
        parse_digest(consumer.get::<_, Option<String>>("source_revision").ok_or(
            DurableError::Conflict("navigation cursor revision is absent"),
        )?)?;
    let input_root = parse_digest(consumer.get::<_, Option<String>>("last_input_root").ok_or(
        DurableError::Conflict("navigation cursor input root is absent"),
    )?)?;
    let output_root = parse_digest(
        consumer
            .get::<_, Option<String>>("last_output_root")
            .ok_or(DurableError::Conflict(
                "navigation cursor output root is absent",
            ))?,
    )?;
    let expected = navigation_verified_prefix_digest(
        domain,
        consumer_id,
        profile,
        bootstrap_sequence,
        bootstrap_root,
        owner_binding_json,
        cursor,
        coverage_root,
        cohort_root,
        adjacency_root,
        &event_id,
        source_revision,
        input_root,
        output_root,
    );
    if prefix_sequence != cursor
        || prefix_coverage_root != coverage_root
        || prefix_cohort_root != cohort_root
        || prefix_digest != expected
        || as_u64(consumer.get("coverage_seq"))? != cursor
        || as_u64(consumer.get("adjacency_seq"))? != cursor
        || !consumer.get::<_, bool>("adjacency_ready")
    {
        return Err(DurableError::Conflict(
            "navigation verified prefix is stale or corrupt",
        ));
    }
    Ok(())
}

fn navigation_event_selected_member_root(
    members: &[DurableNavigationEventSourceV1],
) -> DurableResult<Digest256> {
    if members.is_empty() || members.len() > MAX_NAVIGATION_DEPENDENCIES {
        return Err(DurableError::Refused(
            "navigation event member root count budget",
        ));
    }
    let mut ordered = members.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.source_path.cmp(&right.source_path));
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-event-members-v2");
    navigation_hash_part(&mut hash, &(ordered.len() as u64).to_be_bytes());
    let mut previous_path: Option<&str> = None;
    for member in ordered {
        validate_relative_path(&member.source_path)?;
        if previous_path.is_some_and(|previous| previous >= member.source_path.as_str()) {
            return Err(DurableError::Conflict(
                "navigation event member paths are not unique",
            ));
        }
        previous_path = Some(&member.source_path);
        let revision = member.revision.ok_or(DurableError::Corrupt(
            "navigation event member revision absent",
        ))?;
        let commit_sequence = member.commit_sequence.ok_or(DurableError::Corrupt(
            "navigation event member sequence absent",
        ))?;
        let content_digest = member.content_digest.ok_or(DurableError::Corrupt(
            "navigation event member digest absent",
        ))?;
        let content_length = member.content_length.ok_or(DurableError::Corrupt(
            "navigation event member length absent",
        ))?;
        navigation_hash_part(&mut hash, member.source_path.as_bytes());
        navigation_hash_part(&mut hash, &revision.to_be_bytes());
        navigation_hash_part(&mut hash, &commit_sequence.to_be_bytes());
        navigation_hash_part(&mut hash, content_digest.as_bytes());
        navigation_hash_part(&mut hash, &content_length.to_be_bytes());
        navigation_hash_part(&mut hash, &[u8::from(member.is_source)]);
    }
    Ok(hash.finalize())
}

fn navigation_event_member_root_from_witness(
    witness: &EventWitness,
    selected: &[DurableNavigationEventSourceV1],
) -> DurableResult<Digest256> {
    if witness.members.is_empty() {
        return Err(DurableError::Conflict(
            "navigation command has no committed source history members",
        ));
    }
    let selected_by_path = selected
        .iter()
        .map(|source| (source.source_path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut members = Vec::with_capacity(witness.members.len());
    for member in &witness.members {
        let source =
            selected_by_path
                .get(member.source_path.as_str())
                .ok_or(DurableError::Conflict(
                    "navigation event cut omits a committed history member",
                ))?;
        if source.revision != Some(member.revision)
            || source.content_length != Some(member.content_length)
            || source.is_source != member.is_source
            || source.commit_sequence != Some(witness.event.commit_seq)
        {
            return Err(DurableError::Conflict(
                "navigation selected member differs from committed history",
            ));
        }
        members.push((**source).clone());
    }
    navigation_event_selected_member_root(&members)
}

fn navigation_event_cohort_manifest(
    event_id: &str,
    event_sequence: u64,
    member_root: Digest256,
    cohorts: &[NavigationRecordEventCohortV1],
    byte_limit: usize,
) -> DurableResult<Vec<u8>> {
    let cohort_rows = cohorts
        .iter()
        .map(|cohort| {
            json!({
                "source_path": cohort.source_path.as_str(),
                "record_id": cohort.record_id,
                "source_revision": cohort.source_revision.to_hex(),
                "predecessor_cohort_root": cohort.predecessor_cohort_root.map(Digest256::to_hex),
                "input_root": cohort.input_root.to_hex(),
                "dependency_root": cohort.dependency_root.to_hex(),
                "output_root": cohort.output_root.to_hex(),
            })
        })
        .collect::<Vec<_>>();
    canonical_value(
        &json!({
            "schema": "tos_source_navigation_event_cohort_set_v2",
            "event_id": event_id,
            "event_sequence": event_sequence,
            "member_root": member_root.to_hex(),
            "cohorts": cohort_rows,
        }),
        byte_limit,
    )
}

fn navigation_mixed_event_cohort_manifest(
    event_id: &str,
    event_sequence: u64,
    member_root: Digest256,
    record_cohorts: &[NavigationRecordEventCohortV1],
    claim_cohorts: &[NavigationClaimEventCohortV1],
    claim_binding: Option<&DurableNavigationClaimSetBindingV1>,
    byte_limit: usize,
) -> DurableResult<Vec<u8>> {
    let mut cohort_rows = Vec::<(String, String, Value)>::new();
    for cohort in record_cohorts {
        let (
            basis_kind,
            basis_dependency_path,
            basis_dependency_sha256,
            basis_history_path,
            basis_history_sha256,
            basis_claim_id,
            basis_revision_proof_root,
        ) = match cohort.basis() {
            NavigationRecordEventBasisV1::Mutation => (
                "record-mutation",
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ),
            NavigationRecordEventBasisV1::DependencyRefresh {
                changed_dependency_path,
                changed_dependency_sha256,
            } => (
                "dependency-refresh",
                json!(changed_dependency_path.as_str()),
                json!(changed_dependency_sha256.to_hex()),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ),
            NavigationRecordEventBasisV1::DependencyRefreshSet { .. } => {
                return Err(DurableError::Conflict(
                    "Record dependency-refresh set requires the v5 owner-closure manifest",
                ));
            }
            NavigationRecordEventBasisV1::RangeReconciliation { .. } => {
                return Err(DurableError::Conflict(
                    "range reconciliation requires the exact range manifest",
                ));
            }
            NavigationRecordEventBasisV1::ClaimRevisionRefresh {
                changed_dependency_path,
                changed_dependency_sha256,
                history_path,
                history_sha256,
                claim_id,
                revision_proof_root,
            } => (
                "claim-revision-refresh",
                json!(changed_dependency_path.as_str()),
                json!(changed_dependency_sha256.to_hex()),
                json!(history_path.as_str()),
                json!(history_sha256.to_hex()),
                json!(claim_id),
                json!(revision_proof_root.to_hex()),
            ),
        };
        cohort_rows.push((
            cohort.source_path.as_str().to_owned(),
            cohort.record_id.clone(),
            json!({
                "source_path":cohort.source_path.as_str(),
                "record_id":cohort.record_id,
                "owner_kind":"record",
                "basis_kind":basis_kind,
                "basis_dependency_path":basis_dependency_path,
                "basis_dependency_sha256":basis_dependency_sha256,
                "basis_history_path":basis_history_path,
                "basis_history_sha256":basis_history_sha256,
                "basis_claim_id":basis_claim_id,
                "basis_revision_proof_root":basis_revision_proof_root,
                "claim_owner_inventory_root":null,
                "claim_stream_rows_scanned":null,
                "claim_source_line":null,
                "claim_trace_sha256":null,
                "source_revision":cohort.source_revision.to_hex(),
                "predecessor_cohort_root":cohort.predecessor_cohort_root.map(Digest256::to_hex),
                "input_root":cohort.input_root.to_hex(),
                "dependency_root":cohort.dependency_root.to_hex(),
                "output_root":cohort.output_root.to_hex(),
            }),
        ));
    }
    for cohort in claim_cohorts {
        let integrity = cohort
            .verify_integrity()
            .map_err(|_| DurableError::Conflict("navigation Claim integrity differs"))?;
        cohort_rows.push((
            cohort.source_path().as_str().to_owned(),
            cohort.record_id().to_owned(),
            json!({
                "source_path":cohort.source_path().as_str(),
                "record_id":cohort.record_id(),
                "owner_kind":"claim",
                "basis_kind":"claim-mutation",
                "basis_dependency_path":null,
                "basis_dependency_sha256":null,
                "basis_dependency_causes":null,
                "basis_dependency_causes_root":null,
                "basis_history_path":null,
                "basis_history_sha256":null,
                "basis_claim_id":null,
                "basis_revision_proof_root":cohort
                    .revision_proof()
                    .map(|proof| proof.binding_root().to_hex()),
                "claim_owner_inventory_root":integrity.owner_inventory_root.to_hex(),
                "claim_stream_rows_scanned":integrity.claim_stream_rows_scanned,
                "claim_source_line":cohort.source_line(),
                "claim_trace_sha256":cohort.trace_sha256().to_hex(),
                "source_revision":cohort.source_revision().to_hex(),
                "predecessor_cohort_root":cohort.predecessor_cohort_root().map(Digest256::to_hex),
                "input_root":cohort.input_root().to_hex(),
                "dependency_root":cohort.dependency_root().to_hex(),
                "output_root":cohort.output_root().to_hex(),
            }),
        ));
    }
    cohort_rows.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    let claim_set = claim_binding.map(|binding| {
        json!({
            "mutation_claim_ids":binding.mutation_claim_ids,
            "owner_claim_ids":binding.owner_claim_ids,
            "mutation_receipt_path":binding.mutation_receipt_path,
            "mutation_receipt_sha256":binding.mutation_receipt_sha256.to_hex(),
            "claim_stream_path":binding.claim_stream_path,
            "claim_stream_sha256":binding.claim_stream_sha256.to_hex(),
            "event_claim_set_root":binding.event_claim_set_root.to_hex(),
            "revision_claim_id":binding.revision_claim_id,
            "revision_history_path":binding.revision_history_path,
            "revision_history_sha256":binding.revision_history_sha256.map(Digest256::to_hex),
            "revision_proof_root":binding.revision_proof_root.map(Digest256::to_hex),
            "owner_inventory_root":binding.owner_inventory_root.to_hex(),
            "claim_stream_rows_scanned":binding.claim_stream_rows_scanned,
        })
    });
    canonical_value(
        &json!({
            "schema":"tos_source_navigation_event_cohort_set_v4",
            "event_id":event_id,
            "event_sequence":event_sequence,
            "member_root":member_root.to_hex(),
            "cohorts":cohort_rows.into_iter().map(|(_,_,row)| row).collect::<Vec<_>>(),
            "claim_set":claim_set,
        }),
        byte_limit,
    )
}

fn navigation_mixed_event_cohort_manifest_v5(
    event_id: &str,
    event_sequence: u64,
    member_root: Digest256,
    record_cohorts: &[NavigationRecordEventCohortV1],
    claim_cohorts: &[NavigationClaimEventCohortV1],
    claim_refresh_cohorts: &[tos_compiler::source_navigation_claim_event::NavigationClaimDependencyRefreshCohortV1],
    claim_binding: Option<&DurableNavigationClaimSetBindingV1>,
    claim_refresh_binding: Option<&DurableNavigationClaimRefreshSetBindingV1>,
    claim_empty_closure_binding: Option<&DurableNavigationClaimEmptyClosureBindingV1>,
    closure_root: Digest256,
    byte_limit: usize,
) -> DurableResult<Vec<u8>> {
    if [
        claim_binding.is_some(),
        claim_refresh_binding.is_some(),
        claim_empty_closure_binding.is_some(),
    ]
    .into_iter()
    .filter(|bound| *bound)
    .count()
        != 1
    {
        return Err(DurableError::Conflict(
            "navigation v5 manifest requires exactly one Claim closure binding",
        ));
    }
    let mut cohort_rows = Vec::<(String, String, Value)>::new();
    for cohort in record_cohorts {
        let (basis_dependency_causes, basis_dependency_causes_root) =
            if let NavigationRecordEventBasisV1::DependencyRefreshSet {
                changed_dependencies,
            } = cohort.basis()
            {
                let (causes, root) = navigation_record_dependency_refresh_causes_value(
                    changed_dependencies,
                    cohort.source_path.as_str(),
                    byte_limit,
                )?;
                (causes, Some(root))
            } else {
                (Value::Null, None)
            };
        let (
            basis_kind,
            basis_dependency_path,
            basis_dependency_sha256,
            basis_history_path,
            basis_history_sha256,
            basis_claim_id,
            basis_revision_proof_root,
        ) = match cohort.basis() {
            NavigationRecordEventBasisV1::Mutation => (
                "record-mutation",
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ),
            NavigationRecordEventBasisV1::DependencyRefresh {
                changed_dependency_path,
                changed_dependency_sha256,
            } => (
                "dependency-refresh",
                json!(changed_dependency_path.as_str()),
                json!(changed_dependency_sha256.to_hex()),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ),
            NavigationRecordEventBasisV1::DependencyRefreshSet { .. } => (
                "dependency-refresh-set",
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ),
            NavigationRecordEventBasisV1::RangeReconciliation { .. } => {
                return Err(DurableError::Conflict(
                    "range reconciliation requires the exact range manifest",
                ));
            }
            NavigationRecordEventBasisV1::ClaimRevisionRefresh {
                changed_dependency_path,
                changed_dependency_sha256,
                history_path,
                history_sha256,
                claim_id,
                revision_proof_root,
            } => (
                "claim-revision-refresh",
                json!(changed_dependency_path.as_str()),
                json!(changed_dependency_sha256.to_hex()),
                json!(history_path.as_str()),
                json!(history_sha256.to_hex()),
                json!(claim_id),
                json!(revision_proof_root.to_hex()),
            ),
        };
        cohort_rows.push((
            cohort.source_path.as_str().to_owned(),
            cohort.record_id.clone(),
            json!({
                "source_path":cohort.source_path.as_str(),
                "record_id":cohort.record_id,
                "owner_kind":"record",
                "basis_kind":basis_kind,
                "basis_dependency_path":basis_dependency_path,
                "basis_dependency_sha256":basis_dependency_sha256,
                "basis_dependency_causes":basis_dependency_causes,
                "basis_dependency_causes_root":basis_dependency_causes_root.map(Digest256::to_hex),
                "basis_history_path":basis_history_path,
                "basis_history_sha256":basis_history_sha256,
                "basis_claim_id":basis_claim_id,
                "basis_revision_proof_root":basis_revision_proof_root,
                "claim_owner_inventory_root":null,
                "claim_stream_rows_scanned":null,
                "claim_event_basis_root":null,
                "claim_navigation_manifest_root":null,
                "claim_source_line":null,
                "claim_trace_sha256":null,
                "source_revision":cohort.source_revision.to_hex(),
                "predecessor_cohort_root":cohort.predecessor_cohort_root.map(Digest256::to_hex),
                "input_root":cohort.input_root.to_hex(),
                "dependency_root":cohort.dependency_root.to_hex(),
                "output_root":cohort.output_root.to_hex(),
            }),
        ));
    }
    for cohort in claim_cohorts {
        let integrity = cohort
            .verify_integrity()
            .map_err(|_| DurableError::Conflict("navigation Claim integrity differs"))?;
        let manifest_root = claim_binding
            .and_then(|binding| binding.affected_owner_manifest_root)
            .ok_or(DurableError::Conflict(
                "navigation Claim mutation manifest root absent",
            ))?;
        cohort_rows.push((
            cohort.source_path().as_str().to_owned(),
            cohort.record_id().to_owned(),
            json!({
                "source_path":cohort.source_path().as_str(),
                "record_id":cohort.record_id(),
                "owner_kind":"claim",
                "basis_kind":"claim-mutation",
                "basis_dependency_path":null,
                "basis_dependency_sha256":null,
                "basis_dependency_causes":null,
                "basis_dependency_causes_root":null,
                "basis_history_path":null,
                "basis_history_sha256":null,
                "basis_claim_id":null,
                "basis_revision_proof_root":cohort
                    .revision_proof()
                    .map(|proof| proof.binding_root().to_hex()),
                "claim_owner_inventory_root":integrity.owner_inventory_root.to_hex(),
                "claim_stream_rows_scanned":integrity.claim_stream_rows_scanned,
                "claim_event_basis_root":integrity.event_claim_set_root.to_hex(),
                "claim_navigation_manifest_root":manifest_root.to_hex(),
                "claim_source_line":cohort.source_line(),
                "claim_trace_sha256":cohort.trace_sha256().to_hex(),
                "source_revision":cohort.source_revision().to_hex(),
                "predecessor_cohort_root":cohort.predecessor_cohort_root().map(Digest256::to_hex),
                "input_root":cohort.input_root().to_hex(),
                "dependency_root":cohort.dependency_root().to_hex(),
                "output_root":cohort.output_root().to_hex(),
            }),
        ));
    }
    for cohort in claim_refresh_cohorts {
        let integrity = cohort
            .verify_integrity()
            .map_err(|_| DurableError::Conflict("navigation Claim refresh integrity differs"))?;
        let manifest_root = claim_refresh_binding
            .map(|binding| binding.affected_owner_manifest_root)
            .ok_or(DurableError::Conflict(
                "navigation Claim refresh manifest root absent",
            ))?;
        cohort_rows.push((
            cohort.source_path().as_str().to_owned(),
            cohort.record_id().to_owned(),
            json!({
                "source_path":cohort.source_path().as_str(),
                "record_id":cohort.record_id(),
                "owner_kind":"claim",
                "basis_kind":"claim-dependency-refresh",
                "basis_dependency_path":null,
                "basis_dependency_sha256":null,
                "basis_dependency_causes":null,
                "basis_dependency_causes_root":null,
                "basis_history_path":null,
                "basis_history_sha256":null,
                "basis_claim_id":null,
                "basis_revision_proof_root":null,
                "claim_owner_inventory_root":integrity.owner_inventory_root.to_hex(),
                "claim_stream_rows_scanned":cohort.claim_stream_rows_scanned(),
                "claim_event_basis_root":integrity.proof.binding_root().to_hex(),
                "claim_navigation_manifest_root":manifest_root.to_hex(),
                "claim_source_line":cohort.source_line(),
                "claim_trace_sha256":cohort.trace_sha256().to_hex(),
                "source_revision":cohort.source_revision().to_hex(),
                "predecessor_cohort_root":cohort.predecessor_cohort_root().map(Digest256::to_hex),
                "input_root":cohort.input_root().to_hex(),
                "dependency_root":cohort.dependency_root().to_hex(),
                "output_root":cohort.output_root().to_hex(),
            }),
        ));
    }
    cohort_rows.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    let stream_rows = |streams: &[DurableNavigationClaimStreamBindingV1]| {
        streams
            .iter()
            .map(|stream| {
                json!({
                    "source_path":stream.source_path,
                    "stream_sha256":stream.stream_sha256.to_hex(),
                    "owner_claim_ids":stream.owner_claim_ids,
                    "rows_scanned":stream.rows_scanned,
                })
            })
            .collect::<Vec<_>>()
    };
    let claim_set = claim_binding.map(|binding| {
        json!({
            "basis_kind":"claim-mutation",
            "mutation_source_path":binding.mutation_source_path,
            "mutation_claim_ids":binding.mutation_claim_ids,
            "owner_claim_ids":binding.owner_claim_ids,
            "mutation_receipt_path":binding.mutation_receipt_path,
            "mutation_receipt_sha256":binding.mutation_receipt_sha256.to_hex(),
            "event_claim_set_root":binding.event_claim_set_root.to_hex(),
            "revision_claim_id":binding.revision_claim_id,
            "revision_history_path":binding.revision_history_path,
            "revision_history_sha256":binding.revision_history_sha256.map(Digest256::to_hex),
            "revision_proof_root":binding.revision_proof_root.map(Digest256::to_hex),
            "owner_inventory_root":binding.owner_inventory_root.to_hex(),
            "claim_stream_rows_scanned":binding.claim_stream_rows_scanned,
            "affected_owner_manifest_root":binding.affected_owner_manifest_root.map(Digest256::to_hex),
            "closure_root":binding.closure_root.map(Digest256::to_hex),
            "reverse_claim_ids":binding.reverse_claim_ids,
            "streams":stream_rows(&binding.streams),
        })
    });
    let claim_refresh_set = claim_refresh_binding.map(|binding| {
        json!({
            "basis_kind":"claim-dependency-refresh",
            "owner_inventory_root":binding.owner_inventory_root.to_hex(),
            "affected_owner_manifest_root":binding.affected_owner_manifest_root.to_hex(),
            "closure_root":binding.closure_root.to_hex(),
            "reverse_claim_ids":binding.reverse_claim_ids,
            "causes":binding.causes.iter().map(|cause| json!({
                "changed_dependency_path":cause.changed_dependency_path,
                "changed_dependency_sha256":cause.changed_dependency_sha256.to_hex(),
                "reverse_claim_ids":cause.reverse_claim_ids,
            })).collect::<Vec<_>>(),
            "streams":stream_rows(&binding.streams),
        })
    });
    let claim_empty_closure = claim_empty_closure_binding.map(|binding| {
        json!({
            "basis_kind":"claim-empty-reverse-closure",
            "closure_root":binding.closure_root.to_hex(),
            "covered_source_paths":binding.covered_source_paths,
        })
    });
    canonical_value(
        &json!({
            "schema":"tos_source_navigation_event_cohort_set_v5",
            "event_id":event_id,
            "event_sequence":event_sequence,
            "member_root":member_root.to_hex(),
            "claim_navigation_closure_root":closure_root.to_hex(),
            "cohorts":cohort_rows.into_iter().map(|(_,_,row)| row).collect::<Vec<_>>(),
            "claim_set":claim_set,
            "claim_refresh_set":claim_refresh_set,
            "claim_empty_closure":claim_empty_closure,
        }),
        byte_limit,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NavigationClaimMutationReceiptV1 {
    claim_ids: Vec<String>,
    revision_receipt_sha256: Option<Digest256>,
}

fn navigation_claim_ids_from_mutation_receipt(
    raw: &[u8],
    mutation_source_path: &str,
    mutation_receipt_path: &str,
    byte_limit: usize,
) -> DurableResult<NavigationClaimMutationReceiptV1> {
    let value = strict_json_value(raw, byte_limit)?;
    let schema = value.get("schema_version").and_then(Value::as_str);
    let parent = mutation_source_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(DurableError::Invalid(
            "navigation Claim mutation source path has no parent",
        ))?;
    let (receipt_name, mut ids, revision_receipt_sha256) = match schema {
        Some("tos_local_claim_create_receipt_v1") => {
            if !manifest_v5_has_exact_fields(
                value.as_object().ok_or(DurableError::Corrupt(
                    "navigation Claim creation receipt is not an object",
                ))?,
                &[
                    "schema_version",
                    "command_id",
                    "request_digest",
                    "principal_id",
                    "authority_ref",
                    "owner_configuration",
                    "recorded_at",
                    "source_path",
                    "dependencies",
                    "source_bindings",
                    "claims",
                    "files",
                    "grants_admission",
                ],
            ) || value.get("source_path").and_then(Value::as_str) != Some(mutation_source_path)
                || value.get("grants_admission") != Some(&Value::Bool(false))
            {
                return Err(DurableError::Conflict(
                    "navigation Claim creation receipt owner differs",
                ));
            }
            let ids = value
                .get("claims")
                .and_then(Value::as_array)
                .ok_or(DurableError::Invalid(
                    "navigation Claim create receipt IDs absent",
                ))?
                .iter()
                .map(|claim| {
                    claim
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.is_empty())
                        .map(str::to_owned)
                        .ok_or(DurableError::Invalid(
                            "navigation Claim create receipt ID invalid",
                        ))
                })
                .collect::<DurableResult<Vec<_>>>()?;
            ("source-create-receipt.json", ids, None)
        }
        Some("tos_claim_revision_history_v1") => {
            let history = value.as_object().ok_or(DurableError::Corrupt(
                "navigation Claim revision history is not an object",
            ))?;
            if !manifest_v5_has_exact_fields(
                history,
                &["schema_version", "source_path", "receipts"],
            ) || value.get("source_path").and_then(Value::as_str) != Some(mutation_source_path)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim revision history owner differs",
                ));
            }
            let receipts = value
                .get("receipts")
                .and_then(Value::as_array)
                .filter(|receipts| !receipts.is_empty() && receipts.len() <= 128)
                .ok_or(DurableError::Invalid(
                    "navigation Claim revision history receipts absent or over budget",
                ))?;
            let receipt = receipts.last().ok_or(DurableError::Invalid(
                "navigation Claim changed receipt absent",
            ))?;
            let receipt_object = receipt.as_object().ok_or(DurableError::Corrupt(
                "navigation Claim changed receipt is not an object",
            ))?;
            if !manifest_v5_has_exact_fields(
                receipt_object,
                &[
                    "command_id",
                    "request_digest",
                    "principal_id",
                    "authority_ref",
                    "owner_configuration",
                    "recorded_at",
                    "reason",
                    "previous_source",
                    "source",
                    "previous_revision",
                    "archive_path",
                    "dependencies",
                    "source_bindings",
                    "changed_fields",
                    "forms",
                    "grants_admission",
                    "request",
                ],
            ) {
                return Err(DurableError::Corrupt(
                    "navigation Claim changed receipt fields differ",
                ));
            }
            let source = receipt.get("source").ok_or(DurableError::Corrupt(
                "navigation Claim changed receipt source absent",
            ))?;
            let previous_source = receipt.get("previous_source").ok_or(DurableError::Corrupt(
                "navigation Claim changed receipt predecessor absent",
            ))?;
            let source_id = source
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or(DurableError::Invalid(
                    "navigation Claim changed receipt source ID absent",
                ))?;
            let previous_id = previous_source
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or(DurableError::Invalid(
                    "navigation Claim changed receipt predecessor ID absent",
                ))?;
            let request = receipt.get("request").ok_or(DurableError::Corrupt(
                "navigation Claim changed receipt request absent",
            ))?;
            if source_id != previous_id
                || receipt
                    .get("grants_admission")
                    .is_none_or(|value| value != &Value::Bool(false))
                || request.get("operation").and_then(Value::as_str) != Some("claim.revise")
            {
                return Err(DurableError::Conflict(
                    "navigation Claim changed receipt identity differs",
                ));
            }
            let previous_version = previous_source
                .get("version")
                .and_then(Value::as_u64)
                .ok_or(DurableError::Invalid(
                    "navigation Claim changed receipt predecessor version absent",
                ))?;
            if source.get("version").and_then(Value::as_u64) != previous_version.checked_add(1) {
                return Err(DurableError::Conflict(
                    "navigation Claim changed receipt version transition differs",
                ));
            }
            let receipt_digest = digest_json(receipt, byte_limit)?;
            (
                "claim-revision-history.json",
                vec![source_id.to_owned()],
                Some(receipt_digest),
            )
        }
        _ => {
            return Err(DurableError::Conflict(
                "navigation Claim mutation receipt schema is unsupported",
            ));
        }
    };
    let expected_receipt_path = format!("{parent}/{receipt_name}");
    if mutation_receipt_path != expected_receipt_path {
        return Err(DurableError::Conflict(
            "navigation Claim mutation receipt path differs from its owner schema",
        ));
    }
    ids.sort();
    if ids.is_empty() || ids.len() > 32 || ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(DurableError::Conflict(
            "navigation Claim mutation receipt has duplicate or excessive IDs",
        ));
    }
    for id in &ids {
        validate_identifier(id, MAX_NAVIGATION_RECORD_ID_BYTES, "navigation Claim ID")?;
    }
    Ok(NavigationClaimMutationReceiptV1 {
        claim_ids: ids,
        revision_receipt_sha256,
    })
}

fn navigation_claim_owner_ids_from_stream(
    raw: &[u8],
    row_byte_limit: usize,
    owner_limit: usize,
    total_byte_limit: u64,
) -> DurableResult<Vec<String>> {
    if raw.is_empty() || raw.len() as u64 > total_byte_limit {
        return Err(DurableError::Refused(
            "navigation Claim stream exceeds the selected byte budget",
        ));
    }
    let lines = raw.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    let mut ids = BTreeSet::new();
    for (index, raw_line) in lines.iter().enumerate() {
        if raw_line.is_empty() && index + 1 == lines.len() {
            continue;
        }
        if raw_line.is_empty() {
            return Err(DurableError::Conflict(
                "navigation Claim stream contains a blank row",
            ));
        }
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        let value = strict_json_value(line, row_byte_limit)?;
        let id = value
            .get("claim_id")
            .and_then(Value::as_str)
            .filter(|id| id.starts_with("tos.claim."))
            .ok_or(DurableError::Conflict(
                "navigation Claim stream member ID is invalid",
            ))?;
        validate_identifier(
            id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation Claim stream member ID",
        )?;
        if value
            .get("claim_version")
            .and_then(Value::as_u64)
            .is_none_or(|version| version == 0)
        {
            return Err(DurableError::Conflict(
                "navigation Claim stream member version is invalid",
            ));
        }
        if !ids.insert(id.to_owned()) {
            return Err(DurableError::Conflict(
                "navigation Claim stream repeats an owner ID",
            ));
        }
        if ids.len() > owner_limit {
            return Err(DurableError::Refused(
                "navigation Claim stream exceeds the owner budget",
            ));
        }
    }
    if ids.is_empty() {
        return Err(DurableError::Conflict(
            "navigation Claim stream has no current owners",
        ));
    }
    Ok(ids.into_iter().collect())
}

fn navigation_claim_current_row_from_stream(
    raw: &[u8],
    claim_id: &str,
    row_byte_limit: usize,
    total_byte_limit: u64,
) -> DurableResult<Value> {
    if raw.is_empty() || raw.len() as u64 > total_byte_limit {
        return Err(DurableError::Refused(
            "navigation Claim stream exceeds the selected byte budget",
        ));
    }
    let lines = raw.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    let mut found = None;
    for (index, raw_line) in lines.iter().enumerate() {
        if raw_line.is_empty() && index + 1 == lines.len() {
            continue;
        }
        if raw_line.is_empty() {
            return Err(DurableError::Corrupt(
                "navigation Claim stream contains a blank row",
            ));
        }
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        let value = strict_json_value(line, row_byte_limit)?;
        let id = value
            .get("claim_id")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt(
                "navigation Claim stream member ID is absent",
            ))?;
        if id == claim_id && found.replace(value).is_some() {
            return Err(DurableError::Conflict(
                "navigation Claim stream repeats its revised owner",
            ));
        }
    }
    found.ok_or(DurableError::Conflict(
        "navigation revised Claim is absent from the selected stream",
    ))
}

fn navigation_claim_event_set_root(
    event_id: &str,
    event_sequence: u64,
    mutation_receipt_path: &str,
    mutation_receipt_sha256: Digest256,
    claim_stream_path: &str,
    claim_stream_sha256: Digest256,
    mutation_claim_ids: &[String],
    owner_claim_ids: &[String],
    byte_limit: usize,
) -> DurableResult<Digest256> {
    if event_id.is_empty()
        || event_sequence == 0
        || mutation_claim_ids.is_empty()
        || owner_claim_ids.is_empty()
        || mutation_claim_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || owner_claim_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || mutation_claim_ids
            .iter()
            .any(|id| owner_claim_ids.binary_search(id).is_err())
    {
        return Err(DurableError::Conflict(
            "navigation Claim event set identity is incomplete",
        ));
    }
    validate_relative_path(mutation_receipt_path)?;
    validate_relative_path(claim_stream_path)?;
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_event_members_v2",
            "event_id":event_id,
            "event_sequence":event_sequence,
            "mutation_receipt_path":mutation_receipt_path,
            "mutation_receipt_sha256":mutation_receipt_sha256.to_hex(),
            "claim_stream_path":claim_stream_path,
            "claim_stream_sha256":claim_stream_sha256.to_hex(),
            "mutation_claim_ids":mutation_claim_ids,
            "owner_claim_ids":owner_claim_ids,
        }),
        byte_limit,
    )
}

#[derive(Clone, Debug)]
struct NavigationEventManifestCohortV2 {
    source_path: String,
    record_id: String,
    owner_kind: String,
    basis_kind: String,
    basis_dependency_path: Option<String>,
    basis_dependency_sha256: Option<Digest256>,
    basis_dependency_causes: Vec<NavigationRecordDependencyRefreshCauseV1>,
    basis_dependency_causes_root: Option<Digest256>,
    basis_history_path: Option<String>,
    basis_history_sha256: Option<Digest256>,
    basis_claim_id: Option<String>,
    basis_revision_proof_root: Option<Digest256>,
    claim_owner_inventory_root: Option<Digest256>,
    claim_stream_rows_scanned: Option<u64>,
    claim_event_basis_root: Option<Digest256>,
    claim_navigation_manifest_root: Option<Digest256>,
    claim_source_line: Option<u64>,
    claim_trace_sha256: Option<Digest256>,
    source_event_id: Option<String>,
    source_event_version: Option<u64>,
    predecessor_source_revision: Option<Digest256>,
    predecessor_cohort_root: Option<Digest256>,
    source_revision: Digest256,
    input_root: Digest256,
    dependency_root: Digest256,
    output_root: Digest256,
}

fn navigation_record_dependency_refresh_causes_value(
    causes: &[NavigationRecordDependencyRefreshCauseV1],
    owner_path: &str,
    byte_limit: usize,
) -> DurableResult<(Value, Digest256)> {
    if causes.is_empty()
        || causes.windows(2).any(|pair| pair[0].path >= pair[1].path)
        || causes.iter().any(|cause| {
            cause.path.as_str() == owner_path || !cause.path.as_str().starts_with("ToS/")
        })
    {
        return Err(DurableError::Conflict(
            "navigation Record dependency-refresh cause set is not canonical",
        ));
    }
    let rows = causes
        .iter()
        .map(|cause| {
            json!({
                "path":cause.path.as_str(),
                "sha256":cause.sha256.to_hex(),
            })
        })
        .collect::<Vec<_>>();
    let root = digest_json(
        &json!({
            "schema":"tos_source_navigation_record_dependency_refresh_causes_v1",
            "owner_path":owner_path,
            "causes":rows,
        }),
        byte_limit,
    )?;
    Ok((Value::Array(rows), root))
}

fn parse_navigation_record_dependency_refresh_causes(
    raw: &str,
    root: Digest256,
    owner_path: &str,
    byte_limit: usize,
) -> DurableResult<Vec<NavigationRecordDependencyRefreshCauseV1>> {
    let value = strict_json_value(raw.as_bytes(), byte_limit)?;
    let rows = value.as_array().ok_or(DurableError::Corrupt(
        "stored navigation dependency-refresh causes are not an array",
    ))?;
    let mut causes = Vec::with_capacity(rows.len());
    for row in rows {
        let object = row.as_object().ok_or(DurableError::Corrupt(
            "stored navigation dependency-refresh cause is not an object",
        ))?;
        if object.len() != 2 {
            return Err(DurableError::Corrupt(
                "stored navigation dependency-refresh cause fields differ",
            ));
        }
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .ok_or(DurableError::Corrupt(
                "stored navigation dependency-refresh cause path is absent",
            ))?;
        validate_relative_path(path)
            .map_err(|_| DurableError::Corrupt("stored dependency-refresh path is invalid"))?;
        let sha256 = object
            .get("sha256")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| {
                DurableError::Corrupt("stored dependency-refresh cause digest is invalid")
            })?
            .ok_or(DurableError::Corrupt(
                "stored dependency-refresh cause digest is absent",
            ))?;
        causes.push(NavigationRecordDependencyRefreshCauseV1 {
            path: RelativePath::parse(path)
                .map_err(|_| DurableError::Corrupt("stored dependency-refresh path is invalid"))?,
            sha256,
        });
    }
    let (canonical, actual_root) =
        navigation_record_dependency_refresh_causes_value(&causes, owner_path, byte_limit)?;
    let canonical_raw = serde_json::to_string(&canonical)
        .map_err(|_| DurableError::Corrupt("stored dependency-refresh JSON encoding"))?;
    if canonical_raw != raw || actual_root != root {
        return Err(DurableError::Corrupt(
            "stored navigation dependency-refresh cause root differs",
        ));
    }
    Ok(causes)
}

fn parse_navigation_record_dependency_refresh_cause_columns(
    basis_kind: &str,
    raw: Option<&str>,
    root: Option<Digest256>,
    owner_path: &str,
    byte_limit: usize,
) -> DurableResult<Vec<NavigationRecordDependencyRefreshCauseV1>> {
    match (basis_kind, raw, root) {
        ("dependency-refresh-set", Some(raw), Some(root)) => {
            parse_navigation_record_dependency_refresh_causes(raw, root, owner_path, byte_limit)
        }
        ("dependency-refresh-set", _, _) => Err(DurableError::Corrupt(
            "stored Record dependency-refresh set fields are absent",
        )),
        (_, None, None) => Ok(Vec::new()),
        _ => Err(DurableError::Corrupt(
            "stored navigation basis has unexpected dependency-refresh set fields",
        )),
    }
}

#[derive(Clone, Debug)]
struct NavigationEventManifestV2 {
    cohorts: BTreeMap<(String, String), NavigationEventManifestCohortV2>,
    claim_set: Option<NavigationEventManifestClaimSetV2>,
    claim_refresh_set: Option<NavigationEventManifestClaimRefreshSetV1>,
    claim_empty_closure: Option<NavigationEventManifestClaimEmptyClosureV1>,
    claim_range_set: Option<NavigationEventManifestClaimRangeSetV1>,
}

#[derive(Clone, Debug)]
struct NavigationEventManifestClaimStreamV1 {
    source_path: String,
    stream_sha256: Digest256,
    owner_claim_ids: Vec<String>,
    rows_scanned: u64,
}

#[derive(Clone, Debug)]
struct NavigationEventManifestClaimSetV2 {
    owner_claim_ids: Vec<String>,
    mutation_receipt_path: String,
    mutation_receipt_sha256: Digest256,
    claim_stream_path: Option<String>,
    claim_stream_sha256: Option<Digest256>,
    event_claim_set_root: Digest256,
    revision_claim_id: Option<String>,
    revision_history_path: Option<String>,
    revision_history_sha256: Option<Digest256>,
    revision_proof_root: Option<Digest256>,
    owner_inventory_root: Option<Digest256>,
    claim_stream_rows_scanned: Option<u64>,
    mutation_source_path: Option<String>,
    affected_owner_manifest_root: Option<Digest256>,
    closure_root: Option<Digest256>,
    reverse_claim_ids: Vec<String>,
    streams: Vec<NavigationEventManifestClaimStreamV1>,
}

#[derive(Clone, Debug)]
struct NavigationEventManifestClaimRefreshSetV1 {
    owner_inventory_root: Digest256,
    affected_owner_manifest_root: Digest256,
    closure_root: Digest256,
    reverse_claim_ids: Vec<String>,
    causes: Vec<DurableNavigationClaimRefreshCauseBindingV1>,
    streams: Vec<NavigationEventManifestClaimStreamV1>,
}

#[derive(Clone, Debug)]
struct NavigationEventManifestClaimEmptyClosureV1 {
    closure_root: Digest256,
    covered_source_paths: Vec<String>,
}

#[derive(Clone, Debug)]
struct NavigationEventManifestClaimRangeSetV1 {
    range_manifest_root: Digest256,
    owner_inventory_root: Digest256,
    terminal_owner_ids: Vec<String>,
    terminal_streams: Vec<NavigationEventManifestClaimStreamV1>,
}

fn verify_navigation_event_cohort_manifest(
    raw: &[u8],
    stored_root: Digest256,
    stored_count: u32,
    event_id: &str,
    event_sequence: u64,
    byte_limit: usize,
) -> DurableResult<NavigationEventManifestV2> {
    if raw.is_empty()
        || raw.len() > byte_limit
        || stored_count == 0
        || Digest256::of_bytes(raw) != stored_root
    {
        return Err(DurableError::Conflict(
            "navigation event cohort manifest root or count differs",
        ));
    }
    let value = strict_json_value(raw, byte_limit)?;
    let object = value.as_object().ok_or(DurableError::Corrupt(
        "navigation event manifest object absent",
    ))?;
    if object.get("schema").and_then(Value::as_str)
        == Some("tos_source_navigation_event_cohort_set_v5")
    {
        return verify_navigation_event_cohort_manifest_v5(
            raw,
            stored_root,
            stored_count,
            event_id,
            event_sequence,
            byte_limit,
        );
    }
    let schema = object.get("schema").and_then(Value::as_str);
    let is_v2 = schema == Some("tos_source_navigation_event_cohort_set_v2");
    let is_v3 = schema == Some("tos_source_navigation_event_cohort_set_v3");
    let is_v4 = schema == Some("tos_source_navigation_event_cohort_set_v4");
    if (!is_v2 && !is_v3 && !is_v4)
        || (is_v2 && !matches!(object.len(), 5 | 9))
        || ((is_v3 || is_v4) && object.len() != 6)
        || object.get("event_id").and_then(Value::as_str) != Some(event_id)
        || object.get("event_sequence").and_then(Value::as_u64) != Some(event_sequence)
    {
        return Err(DurableError::Conflict(
            "navigation event cohort manifest identity differs",
        ));
    }
    let cohorts = object
        .get("cohorts")
        .and_then(Value::as_array)
        .ok_or(DurableError::Corrupt(
            "navigation event manifest cohorts absent",
        ))?;
    if cohorts.len() != stored_count as usize {
        return Err(DurableError::Conflict(
            "navigation event cohort manifest count differs",
        ));
    }
    let _member_root = object
        .get("member_root")
        .and_then(Value::as_str)
        .and_then(|value| Digest256::from_hex(value).ok())
        .ok_or(DurableError::Corrupt(
            "navigation event manifest member root invalid",
        ))?;

    let claim_set =
        if is_v2 && object.len() == 9 {
            let event_claim_ids = object
                .get("event_claim_ids")
                .and_then(Value::as_array)
                .ok_or(DurableError::Corrupt(
                    "navigation Claim manifest IDs absent",
                ))?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .map(str::to_owned)
                        .ok_or(DurableError::Corrupt(
                            "navigation Claim manifest ID invalid",
                        ))
                })
                .collect::<DurableResult<Vec<_>>>()?;
            if event_claim_ids.is_empty()
                || event_claim_ids.len() != stored_count as usize
                || event_claim_ids.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(DurableError::Conflict(
                    "navigation Claim manifest IDs are not complete and sorted",
                ));
            }
            let mutation_receipt_path = object
                .get("mutation_receipt_path")
                .and_then(Value::as_str)
                .filter(|path| !path.is_empty())
                .ok_or(DurableError::Corrupt(
                    "navigation Claim manifest receipt path absent",
                ))?
                .to_owned();
            validate_relative_path(&mutation_receipt_path)?;
            let mutation_receipt_sha256 = object
                .get("event_claim_set_binding")
                .and_then(Value::as_str)
                .and_then(|value| Digest256::from_hex(value).ok())
                .ok_or(DurableError::Corrupt(
                    "navigation Claim manifest receipt binding invalid",
                ))?;
            let event_claim_set_root = object
                .get("event_claim_set_root")
                .and_then(Value::as_str)
                .and_then(|value| Digest256::from_hex(value).ok())
                .ok_or(DurableError::Corrupt(
                    "navigation Claim manifest set root invalid",
                ))?;
            Some(NavigationEventManifestClaimSetV2 {
                owner_claim_ids: event_claim_ids,
                mutation_receipt_path,
                mutation_receipt_sha256,
                claim_stream_path: None,
                claim_stream_sha256: None,
                event_claim_set_root,
                revision_claim_id: None,
                revision_history_path: None,
                revision_history_sha256: None,
                revision_proof_root: None,
                owner_inventory_root: None,
                claim_stream_rows_scanned: None,
                mutation_source_path: None,
                affected_owner_manifest_root: None,
                closure_root: None,
                reverse_claim_ids: Vec::new(),
                streams: Vec::new(),
            })
        } else if is_v3 || is_v4 {
            let claim_value = object.get("claim_set").ok_or(DurableError::Corrupt(
                "navigation mixed manifest Claim-set field absent",
            ))?;
            if claim_value.is_null() {
                None
            } else {
                let claim = claim_value.as_object().ok_or(DurableError::Corrupt(
                    "navigation mixed manifest Claim set invalid",
                ))?;
                if claim.len() != if is_v4 { 13 } else { 7 } {
                    return Err(DurableError::Corrupt(
                        "navigation mixed manifest Claim-set fields differ",
                    ));
                }
                let parse_ids =
                    |name: &str| -> DurableResult<Vec<String>> {
                        let values = claim.get(name).and_then(Value::as_array).ok_or(
                            DurableError::Corrupt("navigation mixed manifest Claim IDs absent"),
                        )?;
                        let mut ids = Vec::with_capacity(values.len());
                        for value in values {
                            let id = value
                                .as_str()
                                .filter(|id| id.starts_with("tos.claim."))
                                .ok_or(DurableError::Corrupt(
                                    "navigation mixed manifest Claim ID invalid",
                                ))?;
                            validate_identifier(
                                id,
                                MAX_NAVIGATION_RECORD_ID_BYTES,
                                "navigation manifest Claim ID",
                            )?;
                            ids.push(id.to_owned());
                        }
                        if ids.is_empty() || ids.windows(2).any(|pair| pair[0] >= pair[1]) {
                            return Err(DurableError::Conflict(
                                "navigation mixed manifest Claim IDs are not sorted and unique",
                            ));
                        }
                        Ok(ids)
                    };
                let mutation_claim_ids = parse_ids("mutation_claim_ids")?;
                let owner_claim_ids = parse_ids("owner_claim_ids")?;
                if mutation_claim_ids
                    .iter()
                    .any(|id| owner_claim_ids.binary_search(id).is_err())
                {
                    return Err(DurableError::Conflict(
                        "navigation mixed manifest mutation IDs exceed owner inventory",
                    ));
                }
                let mutation_receipt_path = claim
                    .get("mutation_receipt_path")
                    .and_then(Value::as_str)
                    .filter(|path| !path.is_empty())
                    .ok_or(DurableError::Corrupt(
                        "navigation mixed manifest receipt path absent",
                    ))?
                    .to_owned();
                validate_relative_path(&mutation_receipt_path)?;
                let mutation_receipt_sha256 = claim
                    .get("mutation_receipt_sha256")
                    .and_then(Value::as_str)
                    .and_then(|value| Digest256::from_hex(value).ok())
                    .ok_or(DurableError::Corrupt(
                        "navigation mixed manifest receipt digest invalid",
                    ))?;
                let claim_stream_path = claim
                    .get("claim_stream_path")
                    .and_then(Value::as_str)
                    .filter(|path| !path.is_empty())
                    .ok_or(DurableError::Corrupt(
                        "navigation mixed manifest Claim stream path absent",
                    ))?
                    .to_owned();
                validate_relative_path(&claim_stream_path)?;
                let claim_stream_sha256 = claim
                    .get("claim_stream_sha256")
                    .and_then(Value::as_str)
                    .and_then(|value| Digest256::from_hex(value).ok())
                    .ok_or(DurableError::Corrupt(
                        "navigation mixed manifest Claim stream digest invalid",
                    ))?;
                let event_claim_set_root = claim
                    .get("event_claim_set_root")
                    .and_then(Value::as_str)
                    .and_then(|value| Digest256::from_hex(value).ok())
                    .ok_or(DurableError::Corrupt(
                        "navigation mixed manifest Claim-set root invalid",
                    ))?;
                let (
                    revision_claim_id,
                    revision_history_path,
                    revision_history_sha256,
                    revision_proof_root,
                ) = if is_v4 {
                    let revision_claim_id = match claim.get("revision_claim_id") {
                        Some(Value::Null) => None,
                        Some(Value::String(id)) if !id.is_empty() => {
                            validate_identifier(
                                id,
                                MAX_NAVIGATION_RECORD_ID_BYTES,
                                "navigation revision Claim ID",
                            )?;
                            Some(id.clone())
                        }
                        _ => {
                            return Err(DurableError::Corrupt(
                                "navigation mixed manifest revision Claim ID invalid",
                            ));
                        }
                    };
                    let revision_history_path = match claim.get("revision_history_path") {
                        Some(Value::Null) => None,
                        Some(Value::String(path)) if !path.is_empty() => {
                            validate_relative_path(path)?;
                            Some(path.clone())
                        }
                        _ => {
                            return Err(DurableError::Corrupt(
                                "navigation mixed manifest revision history path invalid",
                            ));
                        }
                    };
                    let revision_history_sha256 = claim
                        .get("revision_history_sha256")
                        .and_then(Value::as_str)
                        .map(Digest256::from_hex)
                        .transpose()
                        .map_err(|_| {
                            DurableError::Corrupt(
                                "navigation mixed manifest revision history digest invalid",
                            )
                        })?;
                    let revision_proof_root = claim
                        .get("revision_proof_root")
                        .and_then(Value::as_str)
                        .map(Digest256::from_hex)
                        .transpose()
                        .map_err(|_| {
                            DurableError::Corrupt(
                                "navigation mixed manifest revision proof root invalid",
                            )
                        })?;
                    let proof_fields = [
                        revision_claim_id.is_some(),
                        revision_history_path.is_some(),
                        revision_history_sha256.is_some(),
                        revision_proof_root.is_some(),
                    ];
                    if proof_fields.iter().any(|value| *value)
                        && !proof_fields.iter().all(|value| *value)
                    {
                        return Err(DurableError::Conflict(
                            "navigation mixed manifest revision proof binding is partial",
                        ));
                    }
                    if revision_claim_id.as_ref().is_some_and(|id| {
                        owner_claim_ids.binary_search(id).is_err()
                            || mutation_claim_ids.binary_search(id).is_err()
                    }) || revision_history_path
                        .as_deref()
                        .is_some_and(|path| path != mutation_receipt_path)
                        || revision_history_sha256.is_some_and(|sha| sha != mutation_receipt_sha256)
                    {
                        return Err(DurableError::Conflict(
                            "navigation mixed manifest revision proof differs from mutation",
                        ));
                    }
                    (
                        revision_claim_id,
                        revision_history_path,
                        revision_history_sha256,
                        revision_proof_root,
                    )
                } else {
                    (None, None, None, None)
                };
                let (owner_inventory_root, claim_stream_rows_scanned) = if is_v4 {
                    let inventory_root = claim
                        .get("owner_inventory_root")
                        .and_then(Value::as_str)
                        .map(Digest256::from_hex)
                        .transpose()
                        .map_err(|_| {
                            DurableError::Corrupt(
                                "navigation mixed manifest Claim inventory root invalid",
                            )
                        })?;
                    let rows_scanned = claim
                        .get("claim_stream_rows_scanned")
                        .and_then(Value::as_u64)
                        .filter(|rows| *rows > 0);
                    if inventory_root.is_none() || rows_scanned.is_none() {
                        return Err(DurableError::Conflict(
                            "navigation mixed manifest Claim inventory evidence is absent",
                        ));
                    }
                    (inventory_root, rows_scanned)
                } else {
                    (None, None)
                };
                let calculated_claim_set_root = navigation_claim_event_set_root(
                    event_id,
                    event_sequence,
                    &mutation_receipt_path,
                    mutation_receipt_sha256,
                    &claim_stream_path,
                    claim_stream_sha256,
                    &mutation_claim_ids,
                    &owner_claim_ids,
                    byte_limit,
                )?;
                if calculated_claim_set_root != event_claim_set_root {
                    return Err(DurableError::Conflict(
                        "navigation mixed manifest Claim-set root differs",
                    ));
                }
                Some(NavigationEventManifestClaimSetV2 {
                    owner_claim_ids,
                    mutation_receipt_path,
                    mutation_receipt_sha256,
                    claim_stream_path: Some(claim_stream_path),
                    claim_stream_sha256: Some(claim_stream_sha256),
                    event_claim_set_root,
                    revision_claim_id,
                    revision_history_path,
                    revision_history_sha256,
                    revision_proof_root,
                    owner_inventory_root,
                    claim_stream_rows_scanned,
                    mutation_source_path: None,
                    affected_owner_manifest_root: None,
                    closure_root: None,
                    reverse_claim_ids: Vec::new(),
                    streams: Vec::new(),
                })
            }
        } else {
            None
        };

    let mut parsed = BTreeMap::new();
    let mut previous_key: Option<(String, String)> = None;
    for cohort in cohorts {
        let cohort = cohort.as_object().ok_or(DurableError::Corrupt(
            "navigation event manifest cohort invalid",
        ))?;
        let (
            owner_kind,
            basis_kind,
            basis_dependency_path,
            basis_dependency_sha256,
            basis_history_path,
            basis_history_sha256,
            basis_claim_id,
            basis_revision_proof_root,
            claim_owner_inventory_root,
            claim_stream_rows_scanned,
            claim_source_line,
            claim_trace_sha256,
        ) = if is_v2 {
            if cohort.len() != 7 {
                return Err(DurableError::Corrupt(
                    "navigation event manifest cohort fields differ",
                ));
            }
            (
                if claim_set.is_some() {
                    "claim"
                } else {
                    "record"
                }
                .to_owned(),
                if claim_set.is_some() {
                    "claim-mutation"
                } else {
                    "record-mutation"
                }
                .to_owned(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        } else {
            if cohort.len() != if is_v4 { 19 } else { 13 } {
                return Err(DurableError::Corrupt(
                    "navigation mixed manifest cohort fields differ",
                ));
            }
            let owner_kind = cohort
                .get("owner_kind")
                .and_then(Value::as_str)
                .filter(|kind| matches!(*kind, "record" | "claim"))
                .ok_or(DurableError::Corrupt(
                    "navigation mixed manifest owner kind invalid",
                ))?
                .to_owned();
            let basis_kind = cohort
                .get("basis_kind")
                .and_then(Value::as_str)
                .ok_or(DurableError::Corrupt(
                    "navigation mixed manifest basis kind absent",
                ))?
                .to_owned();
            let path = cohort
                .get("basis_dependency_path")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let digest = cohort
                .get("basis_dependency_sha256")
                .and_then(Value::as_str)
                .map(Digest256::from_hex)
                .transpose()
                .map_err(|_| {
                    DurableError::Corrupt(
                        "navigation mixed manifest basis dependency digest invalid",
                    )
                })?;
            let (history_path, history_sha256, claim_id, revision_proof_root) = if is_v4 {
                let path = match cohort.get("basis_history_path") {
                    Some(Value::Null) => None,
                    Some(Value::String(path)) if !path.is_empty() => {
                        validate_relative_path(path)?;
                        Some(path.clone())
                    }
                    _ => {
                        return Err(DurableError::Corrupt(
                            "navigation mixed manifest basis history path invalid",
                        ));
                    }
                };
                let sha = cohort
                    .get("basis_history_sha256")
                    .and_then(Value::as_str)
                    .map(Digest256::from_hex)
                    .transpose()
                    .map_err(|_| {
                        DurableError::Corrupt(
                            "navigation mixed manifest basis history digest invalid",
                        )
                    })?;
                let id = match cohort.get("basis_claim_id") {
                    Some(Value::Null) => None,
                    Some(Value::String(id)) if !id.is_empty() => Some(id.clone()),
                    _ => {
                        return Err(DurableError::Corrupt(
                            "navigation mixed manifest basis Claim ID invalid",
                        ));
                    }
                };
                let root = cohort
                    .get("basis_revision_proof_root")
                    .and_then(Value::as_str)
                    .map(Digest256::from_hex)
                    .transpose()
                    .map_err(|_| {
                        DurableError::Corrupt("navigation mixed manifest basis proof root invalid")
                    })?;
                (path, sha, id, root)
            } else {
                (None, None, None, None)
            };
            let (claim_owner_inventory_root, claim_stream_rows_scanned) = if is_v4 {
                let inventory_root = cohort
                    .get("claim_owner_inventory_root")
                    .and_then(Value::as_str)
                    .map(Digest256::from_hex)
                    .transpose()
                    .map_err(|_| {
                        DurableError::Corrupt(
                            "navigation mixed manifest Claim inventory root invalid",
                        )
                    })?;
                let rows_scanned = cohort
                    .get("claim_stream_rows_scanned")
                    .and_then(Value::as_u64)
                    .filter(|rows| *rows > 0);
                (inventory_root, rows_scanned)
            } else {
                (None, None)
            };
            let claim_source_line = cohort
                .get("claim_source_line")
                .and_then(Value::as_u64)
                .filter(|line| *line > 0);
            let claim_trace_sha256 = cohort
                .get("claim_trace_sha256")
                .and_then(Value::as_str)
                .map(Digest256::from_hex)
                .transpose()
                .map_err(|_| {
                    DurableError::Corrupt("navigation mixed manifest Claim trace digest invalid")
                })?;
            let valid_basis = match (
                owner_kind.as_str(),
                basis_kind.as_str(),
                path.as_deref(),
                digest,
                history_path.as_deref(),
                history_sha256,
                claim_id.as_deref(),
                revision_proof_root,
            ) {
                ("record", "record-mutation", None, None, None, None, None, None)
                | ("claim", "claim-mutation", None, None, None, None, None, _) => true,
                ("record", "dependency-refresh", Some(path), Some(_), None, None, None, None) => {
                    validate_relative_path(path).is_ok()
                }
                (
                    "record",
                    "claim-revision-refresh",
                    Some(path),
                    Some(_),
                    Some(history),
                    Some(_),
                    Some(id),
                    Some(_),
                ) => {
                    validate_relative_path(path).is_ok()
                        && validate_relative_path(history).is_ok()
                        && !id.is_empty()
                }
                _ => false,
            };
            if !valid_basis {
                return Err(DurableError::Conflict(
                    "navigation mixed manifest owner basis is inconsistent",
                ));
            }
            if (owner_kind == "claim"
                && (claim_source_line.is_none() || claim_trace_sha256.is_none()))
                || (owner_kind == "record"
                    && (cohort.get("claim_source_line") != Some(&Value::Null)
                        || cohort.get("claim_trace_sha256") != Some(&Value::Null)))
            {
                return Err(DurableError::Conflict(
                    "navigation mixed manifest Claim projection evidence is inconsistent",
                ));
            }
            if (owner_kind == "claim"
                && is_v4
                && (claim_owner_inventory_root.is_none() || claim_stream_rows_scanned.is_none()))
                || (owner_kind == "record"
                    && (claim_owner_inventory_root.is_some()
                        || claim_stream_rows_scanned.is_some()))
            {
                return Err(DurableError::Conflict(
                    "navigation mixed manifest Claim inventory fields are inconsistent",
                ));
            }
            (
                owner_kind,
                basis_kind,
                path,
                digest,
                history_path,
                history_sha256,
                claim_id,
                revision_proof_root,
                claim_owner_inventory_root,
                claim_stream_rows_scanned,
                claim_source_line,
                claim_trace_sha256,
            )
        };

        let source_path = cohort
            .get("source_path")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(DurableError::Corrupt(
                "navigation event manifest source path absent",
            ))?
            .to_owned();
        validate_relative_path(&source_path)?;
        let record_id = cohort
            .get("record_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(DurableError::Corrupt(
                "navigation event manifest record ID absent",
            ))?
            .to_owned();
        validate_identifier(
            &record_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation manifest owner ID",
        )?;
        let key = (source_path.clone(), record_id.clone());
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &key)
        {
            return Err(DurableError::Conflict(
                "navigation event manifest owners are not uniquely C-sorted",
            ));
        }
        previous_key = Some(key.clone());
        let source_revision = cohort
            .get("source_revision")
            .and_then(Value::as_str)
            .and_then(|value| Digest256::from_hex(value).ok())
            .ok_or(DurableError::Corrupt(
                "navigation event manifest source revision invalid",
            ))?;
        let input_root = cohort
            .get("input_root")
            .and_then(Value::as_str)
            .and_then(|value| Digest256::from_hex(value).ok())
            .ok_or(DurableError::Corrupt(
                "navigation event manifest input root invalid",
            ))?;
        let dependency_root = cohort
            .get("dependency_root")
            .and_then(Value::as_str)
            .and_then(|value| Digest256::from_hex(value).ok())
            .ok_or(DurableError::Corrupt(
                "navigation event manifest dependency root invalid",
            ))?;
        let output_root = cohort
            .get("output_root")
            .and_then(Value::as_str)
            .and_then(|value| Digest256::from_hex(value).ok())
            .ok_or(DurableError::Corrupt(
                "navigation event manifest output root invalid",
            ))?;
        if let Some(predecessor) = cohort.get("predecessor_cohort_root") {
            if !predecessor.is_null()
                && predecessor
                    .as_str()
                    .and_then(|value| Digest256::from_hex(value).ok())
                    .is_none()
            {
                return Err(DurableError::Corrupt(
                    "navigation event manifest predecessor root invalid",
                ));
            }
        } else {
            return Err(DurableError::Corrupt(
                "navigation event manifest predecessor field absent",
            ));
        }
        if parsed
            .insert(
                key,
                NavigationEventManifestCohortV2 {
                    source_path,
                    record_id,
                    owner_kind,
                    basis_kind,
                    basis_dependency_path,
                    basis_dependency_sha256,
                    basis_dependency_causes: Vec::new(),
                    basis_dependency_causes_root: None,
                    basis_history_path,
                    basis_history_sha256,
                    basis_claim_id,
                    basis_revision_proof_root,
                    claim_owner_inventory_root,
                    claim_stream_rows_scanned,
                    claim_event_basis_root: None,
                    claim_navigation_manifest_root: None,
                    claim_source_line,
                    claim_trace_sha256,
                    source_event_id: None,
                    source_event_version: None,
                    predecessor_source_revision: None,
                    predecessor_cohort_root: None,
                    source_revision,
                    input_root,
                    dependency_root,
                    output_root,
                },
            )
            .is_some()
        {
            return Err(DurableError::Conflict(
                "navigation event manifest repeats an owner",
            ));
        }
    }

    let claim_rows = parsed
        .values()
        .filter(|cohort| cohort.owner_kind == "claim")
        .collect::<Vec<_>>();
    if let Some(claim_set) = &claim_set {
        let manifest_ids = claim_rows
            .iter()
            .map(|cohort| cohort.record_id.clone())
            .collect::<Vec<_>>();
        let paths = claim_rows
            .iter()
            .map(|cohort| cohort.source_path.as_str())
            .collect::<BTreeSet<_>>();
        let only_path = paths.first().copied();
        if manifest_ids != claim_set.owner_claim_ids
            || only_path.is_none()
            || paths.len() != 1
            || claim_set
                .claim_stream_path
                .as_deref()
                .is_some_and(|path| Some(path) != only_path)
        {
            return Err(DurableError::Conflict(
                "navigation Claim manifest differs from its complete owner set",
            ));
        }
        let has_revision_binding = claim_set.revision_claim_id.is_some();
        let revision_claim_rows = claim_rows
            .iter()
            .filter(|cohort| cohort.basis_revision_proof_root.is_some())
            .collect::<Vec<_>>();
        if has_revision_binding {
            let revision_id = claim_set
                .revision_claim_id
                .as_deref()
                .ok_or(DurableError::Corrupt("navigation revision Claim ID absent"))?;
            if revision_claim_rows.len() != 1
                || revision_claim_rows[0].record_id != revision_id
                || revision_claim_rows[0].basis_revision_proof_root != claim_set.revision_proof_root
            {
                return Err(DurableError::Conflict(
                    "navigation Claim revision proof owner differs from its set binding",
                ));
            }
            let (
                Some(claim_stream_path),
                Some(claim_stream_sha256),
                Some(history_path),
                Some(history_sha256),
                Some(proof_root),
            ) = (
                claim_set.claim_stream_path.as_deref(),
                claim_set.claim_stream_sha256,
                claim_set.revision_history_path.as_deref(),
                claim_set.revision_history_sha256,
                claim_set.revision_proof_root,
            )
            else {
                return Err(DurableError::Conflict(
                    "navigation Claim revision proof binding is incomplete",
                ));
            };
            for record in parsed
                .values()
                .filter(|cohort| cohort.basis_kind == "claim-revision-refresh")
            {
                if record.owner_kind != "record"
                    || record.basis_dependency_path.as_deref() != Some(claim_stream_path)
                    || record.basis_dependency_sha256 != Some(claim_stream_sha256)
                    || record.basis_history_path.as_deref() != Some(history_path)
                    || record.basis_history_sha256 != Some(history_sha256)
                    || record.basis_claim_id.as_deref() != Some(revision_id)
                    || record.basis_revision_proof_root != Some(proof_root)
                {
                    return Err(DurableError::Conflict(
                        "navigation refreshed Record basis differs from Claim revision proof",
                    ));
                }
            }
        } else if !revision_claim_rows.is_empty()
            || parsed
                .values()
                .any(|cohort| cohort.basis_kind == "claim-revision-refresh")
        {
            return Err(DurableError::Conflict(
                "navigation revision proof rows lack a Claim-set proof binding",
            ));
        }
    } else if !claim_rows.is_empty() {
        return Err(DurableError::Conflict(
            "navigation Claim owners have no complete Claim-set binding",
        ));
    }
    if claim_set.is_none()
        && parsed
            .values()
            .any(|cohort| cohort.basis_kind == "claim-revision-refresh")
    {
        return Err(DurableError::Conflict(
            "navigation Record revision refresh lacks a Claim-set proof",
        ));
    }

    let canonical = canonical_value(&value, byte_limit)?;
    if canonical != raw {
        return Err(DurableError::Corrupt(
            "navigation event cohort manifest is not canonical",
        ));
    }
    Ok(NavigationEventManifestV2 {
        cohorts: parsed,
        claim_set,
        claim_refresh_set: None,
        claim_empty_closure: None,
        claim_range_set: None,
    })
}

fn manifest_v5_ids(value: &Value, allow_empty: bool) -> DurableResult<Vec<String>> {
    let values = value.as_array().ok_or(DurableError::Corrupt(
        "navigation v5 Claim ID set is not an array",
    ))?;
    let ids = values
        .iter()
        .map(|value| {
            let id = value
                .as_str()
                .filter(|id| id.starts_with("tos.claim."))
                .ok_or(DurableError::Corrupt("navigation v5 Claim ID is invalid"))?;
            validate_identifier(id, MAX_NAVIGATION_RECORD_ID_BYTES, "navigation v5 Claim ID")?;
            Ok(id.to_owned())
        })
        .collect::<DurableResult<Vec<_>>>()?;
    if (!allow_empty && ids.is_empty()) || ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DurableError::Conflict(
            "navigation v5 Claim IDs are not sorted and unique",
        ));
    }
    Ok(ids)
}

fn manifest_v5_source_paths(value: &Value) -> DurableResult<Vec<String>> {
    let values = value.as_array().ok_or(DurableError::Corrupt(
        "navigation v5 closure paths are not an array",
    ))?;
    if values.is_empty() {
        return Err(DurableError::Corrupt(
            "navigation v5 empty-closure paths are empty",
        ));
    }
    let mut paths = Vec::with_capacity(values.len());
    for value in values {
        let path = value
            .as_str()
            .filter(|path| path.starts_with("ToS/"))
            .ok_or(DurableError::Corrupt(
                "navigation v5 empty-closure path invalid",
            ))?
            .to_owned();
        validate_relative_path(&path)?;
        if path.ends_with("/source-claims.jsonl")
            || path == "ToS/source-witnesses/owner-local"
            || path.starts_with("ToS/source-witnesses/owner-local/")
        {
            return Err(DurableError::Corrupt(
                "navigation v5 empty-closure path is not public source",
            ));
        }
        if paths.last().is_some_and(|previous| previous >= &path) {
            return Err(DurableError::Conflict(
                "navigation v5 empty-closure paths are not C-sorted",
            ));
        }
        paths.push(path);
    }
    Ok(paths)
}

fn manifest_v5_streams(
    value: &Value,
    allow_empty: bool,
) -> DurableResult<Vec<NavigationEventManifestClaimStreamV1>> {
    let values = value.as_array().ok_or(DurableError::Corrupt(
        "navigation v5 Claim streams are not an array",
    ))?;
    if (!allow_empty && values.is_empty()) || values.len() > MAX_NAVIGATION_DEPENDENCIES {
        return Err(DurableError::Refused(
            "navigation v5 Claim stream count budget",
        ));
    }
    let mut streams = Vec::with_capacity(values.len());
    let mut all_ids = BTreeSet::new();
    for value in values {
        let stream = value.as_object().ok_or(DurableError::Corrupt(
            "navigation v5 Claim stream is not an object",
        ))?;
        if stream.len() != 4 {
            return Err(DurableError::Corrupt(
                "navigation v5 Claim stream fields differ",
            ));
        }
        let source_path = stream
            .get("source_path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .ok_or(DurableError::Corrupt(
                "navigation v5 Claim stream path absent",
            ))?
            .to_owned();
        validate_relative_path(&source_path)?;
        let stream_sha256 = stream
            .get("stream_sha256")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 Claim stream digest invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 Claim stream digest absent",
            ))?;
        let owner_claim_ids = manifest_v5_ids(
            stream.get("owner_claim_ids").ok_or(DurableError::Corrupt(
                "navigation v5 Claim stream owners absent",
            ))?,
            false,
        )?;
        let rows_scanned = stream
            .get("rows_scanned")
            .and_then(Value::as_u64)
            .filter(|rows| *rows > 0)
            .ok_or(DurableError::Corrupt(
                "navigation v5 Claim stream row count invalid",
            ))?;
        if rows_scanned != owner_claim_ids.len() as u64
            || streams
                .last()
                .is_some_and(|previous: &NavigationEventManifestClaimStreamV1| {
                    previous.source_path >= source_path
                })
            || owner_claim_ids.iter().any(|id| !all_ids.insert(id.clone()))
        {
            return Err(DurableError::Conflict(
                "navigation v5 Claim stream inventory is inconsistent",
            ));
        }
        streams.push(NavigationEventManifestClaimStreamV1 {
            source_path,
            stream_sha256,
            owner_claim_ids,
            rows_scanned,
        });
    }
    Ok(streams)
}

fn navigation_claim_affected_owner_manifest_root_v1(
    inventory_root: Digest256,
    mutation_source_path: &str,
    mutation_claim_ids: &[String],
    reverse_claim_ids: &[String],
    streams: &[NavigationEventManifestClaimStreamV1],
    byte_limit: usize,
) -> DurableResult<Digest256> {
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_affected_owner_manifest_v1",
            "inventory_root":inventory_root.to_hex(),
            "mutation_source_path":mutation_source_path,
            "mutation_claim_ids":mutation_claim_ids,
            "reverse_claim_ids":reverse_claim_ids,
            "streams":streams.iter().map(|stream| json!({
                "source_path":stream.source_path,
                "stream_sha256":stream.stream_sha256.to_hex(),
                "rows_scanned":stream.rows_scanned,
                "owner_ids":stream.owner_claim_ids,
            })).collect::<Vec<_>>(),
        }),
        byte_limit,
    )
}

fn navigation_claim_refresh_manifest_set_root_v1(
    inventory_root: Digest256,
    closure_root: Digest256,
    causes: &[DurableNavigationClaimRefreshCauseBindingV1],
    reverse_claim_ids: &[String],
    streams: &[NavigationEventManifestClaimStreamV1],
    byte_limit: usize,
) -> DurableResult<Digest256> {
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_dependency_refresh_owner_manifest_set_v1",
            "inventory_root":inventory_root.to_hex(),
            "closure_root":closure_root.to_hex(),
            "causes":causes.iter().map(|cause| json!({
                "changed_dependency_path":cause.changed_dependency_path,
                "changed_dependency_sha256":cause.changed_dependency_sha256.to_hex(),
                "reverse_claim_ids":cause.reverse_claim_ids,
            })).collect::<Vec<_>>(),
            "reverse_claim_ids":reverse_claim_ids,
            "streams":streams.iter().map(|stream| json!({
                "source_path":stream.source_path,
                "stream_sha256":stream.stream_sha256.to_hex(),
                "rows_scanned":stream.rows_scanned,
                "owner_ids":stream.owner_claim_ids,
            })).collect::<Vec<_>>(),
        }),
        byte_limit,
    )
}

fn manifest_v5_optional_digest(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> DurableResult<Option<Digest256>> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Digest256::from_hex(value)
            .map(Some)
            .map_err(|_| DurableError::Corrupt("navigation v5 optional digest invalid")),
        _ => Err(DurableError::Corrupt(
            "navigation v5 optional digest field absent",
        )),
    }
}

fn manifest_v5_has_exact_fields(object: &serde_json::Map<String, Value>, fields: &[&str]) -> bool {
    object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
}

fn manifest_v5_optional_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> DurableResult<Option<String>> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => {
            validate_relative_path(value)?;
            Ok(Some(value.clone()))
        }
        _ => Err(DurableError::Corrupt(
            "navigation v5 optional path field invalid",
        )),
    }
}

fn verify_navigation_event_cohort_manifest_v5(
    raw: &[u8],
    stored_root: Digest256,
    stored_count: u32,
    event_id: &str,
    event_sequence: u64,
    byte_limit: usize,
) -> DurableResult<NavigationEventManifestV2> {
    if raw.is_empty()
        || raw.len() > byte_limit
        || stored_count == 0
        || Digest256::of_bytes(raw) != stored_root
    {
        return Err(DurableError::Conflict(
            "navigation v5 manifest root or count differs",
        ));
    }
    let value = strict_json_value(raw, byte_limit)?;
    let object = value.as_object().ok_or(DurableError::Corrupt(
        "navigation v5 manifest object absent",
    ))?;
    if !manifest_v5_has_exact_fields(
        object,
        &[
            "schema",
            "event_id",
            "event_sequence",
            "member_root",
            "claim_navigation_closure_root",
            "cohorts",
            "claim_set",
            "claim_refresh_set",
            "claim_empty_closure",
        ],
    ) || object.get("schema").and_then(Value::as_str)
        != Some("tos_source_navigation_event_cohort_set_v5")
        || object.get("event_id").and_then(Value::as_str) != Some(event_id)
        || object.get("event_sequence").and_then(Value::as_u64) != Some(event_sequence)
    {
        return Err(DurableError::Conflict(
            "navigation v5 manifest identity or fields differ",
        ));
    }
    let _member_root = object
        .get("member_root")
        .and_then(Value::as_str)
        .map(Digest256::from_hex)
        .transpose()
        .map_err(|_| DurableError::Corrupt("navigation v5 member root invalid"))?
        .ok_or(DurableError::Corrupt("navigation v5 member root absent"))?;
    let closure_root = object
        .get("claim_navigation_closure_root")
        .and_then(Value::as_str)
        .map(Digest256::from_hex)
        .transpose()
        .map_err(|_| DurableError::Corrupt("navigation v5 closure root invalid"))?
        .ok_or(DurableError::Corrupt("navigation v5 closure root absent"))?;
    let cohorts = object
        .get("cohorts")
        .and_then(Value::as_array)
        .ok_or(DurableError::Corrupt("navigation v5 cohorts absent"))?;
    if cohorts.len() != stored_count as usize {
        return Err(DurableError::Conflict("navigation v5 cohort count differs"));
    }
    let claim_set_value = object.get("claim_set").ok_or(DurableError::Corrupt(
        "navigation v5 Claim-set field absent",
    ))?;
    let refresh_value = object
        .get("claim_refresh_set")
        .ok_or(DurableError::Corrupt(
            "navigation v5 Claim-refresh field absent",
        ))?;
    let empty_closure_null = Value::Null;
    let empty_closure_value = object
        .get("claim_empty_closure")
        .unwrap_or(&empty_closure_null);
    if [
        !claim_set_value.is_null(),
        !refresh_value.is_null(),
        !empty_closure_value.is_null(),
    ]
    .into_iter()
    .filter(|bound| *bound)
    .count()
        != 1
    {
        return Err(DurableError::Conflict(
            "navigation v5 manifest must bind exactly one Claim closure cause",
        ));
    }
    let claim_set = if let Some(claim) = claim_set_value.as_object() {
        if !manifest_v5_has_exact_fields(
            claim,
            &[
                "basis_kind",
                "mutation_source_path",
                "mutation_claim_ids",
                "owner_claim_ids",
                "mutation_receipt_path",
                "mutation_receipt_sha256",
                "event_claim_set_root",
                "revision_claim_id",
                "revision_history_path",
                "revision_history_sha256",
                "revision_proof_root",
                "owner_inventory_root",
                "claim_stream_rows_scanned",
                "affected_owner_manifest_root",
                "closure_root",
                "reverse_claim_ids",
                "streams",
            ],
        ) || claim.get("basis_kind").and_then(Value::as_str) != Some("claim-mutation")
        {
            return Err(DurableError::Corrupt(
                "navigation v5 Claim-set fields differ",
            ));
        }
        let mutation_source_path = claim
            .get("mutation_source_path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .ok_or(DurableError::Corrupt(
                "navigation v5 mutation source path absent",
            ))?
            .to_owned();
        validate_relative_path(&mutation_source_path)?;
        let mutation_claim_ids = manifest_v5_ids(
            claim
                .get("mutation_claim_ids")
                .ok_or(DurableError::Corrupt(
                    "navigation v5 mutation Claim IDs absent",
                ))?,
            false,
        )?;
        let owner_claim_ids = manifest_v5_ids(
            claim.get("owner_claim_ids").ok_or(DurableError::Corrupt(
                "navigation v5 owner Claim IDs absent",
            ))?,
            false,
        )?;
        if mutation_claim_ids
            .iter()
            .any(|id| owner_claim_ids.binary_search(id).is_err())
        {
            return Err(DurableError::Conflict(
                "navigation v5 mutation IDs exceed full owner set",
            ));
        }
        let mutation_receipt_path = claim
            .get("mutation_receipt_path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .ok_or(DurableError::Corrupt(
                "navigation v5 mutation receipt path absent",
            ))?
            .to_owned();
        validate_relative_path(&mutation_receipt_path)?;
        let mutation_receipt_sha256 = claim
            .get("mutation_receipt_sha256")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 receipt digest invalid"))?
            .ok_or(DurableError::Corrupt("navigation v5 receipt digest absent"))?;
        let event_claim_set_root = claim
            .get("event_claim_set_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 Claim event root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 Claim event root absent",
            ))?;
        let revision_claim_id = match claim.get("revision_claim_id") {
            Some(Value::Null) => None,
            Some(Value::String(id)) if id.starts_with("tos.claim.") => Some(id.clone()),
            _ => {
                return Err(DurableError::Corrupt(
                    "navigation v5 revision Claim ID invalid",
                ));
            }
        };
        let revision_history_path = manifest_v5_optional_string(claim, "revision_history_path")?;
        let revision_history_sha256 =
            manifest_v5_optional_digest(claim, "revision_history_sha256")?;
        let revision_proof_root = manifest_v5_optional_digest(claim, "revision_proof_root")?;
        let revision_fields = [
            revision_claim_id.is_some(),
            revision_history_path.is_some(),
            revision_history_sha256.is_some(),
            revision_proof_root.is_some(),
        ];
        if revision_fields.iter().any(|field| *field) && !revision_fields.iter().all(|field| *field)
        {
            return Err(DurableError::Conflict(
                "navigation v5 revision proof fields are partial",
            ));
        }
        if revision_claim_id.as_ref().is_some_and(|id| {
            owner_claim_ids.binary_search(id).is_err()
                || mutation_claim_ids.binary_search(id).is_err()
        }) || revision_history_path
            .as_deref()
            .is_some_and(|path| path != mutation_receipt_path)
            || revision_history_sha256.is_some_and(|sha| sha != mutation_receipt_sha256)
        {
            return Err(DurableError::Conflict(
                "navigation v5 revision proof differs from the mutation receipt",
            ));
        }
        let owner_inventory_root = claim
            .get("owner_inventory_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 owner inventory root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 owner inventory root absent",
            ))?;
        let claim_stream_rows_scanned = claim
            .get("claim_stream_rows_scanned")
            .and_then(Value::as_u64)
            .filter(|rows| *rows > 0)
            .ok_or(DurableError::Corrupt(
                "navigation v5 Claim scanned-row count invalid",
            ))?;
        let affected_owner_manifest_root = claim
            .get("affected_owner_manifest_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 affected-owner root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 affected-owner root absent",
            ))?;
        let claim_closure_root = claim
            .get("closure_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 Claim closure root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 Claim closure root absent",
            ))?;
        let reverse_claim_ids = manifest_v5_ids(
            claim.get("reverse_claim_ids").ok_or(DurableError::Corrupt(
                "navigation v5 reverse Claim IDs absent",
            ))?,
            true,
        )?;
        let streams = manifest_v5_streams(
            claim
                .get("streams")
                .ok_or(DurableError::Corrupt("navigation v5 streams absent"))?,
            false,
        )?;
        if navigation_claim_affected_owner_manifest_root_v1(
            owner_inventory_root,
            &mutation_source_path,
            &mutation_claim_ids,
            &reverse_claim_ids,
            &streams,
            byte_limit,
        )? != affected_owner_manifest_root
        {
            return Err(DurableError::Conflict(
                "navigation v5 mutation owner-manifest root differs",
            ));
        }
        let stream_ids = streams
            .iter()
            .flat_map(|stream| stream.owner_claim_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        if stream_ids != owner_claim_ids.iter().cloned().collect::<BTreeSet<_>>() {
            return Err(DurableError::Conflict(
                "navigation v5 Claim stream owner union differs",
            ));
        }
        if reverse_claim_ids
            .iter()
            .any(|id| owner_claim_ids.binary_search(id).is_err())
        {
            return Err(DurableError::Conflict(
                "navigation v5 reverse Claim IDs exceed the full owner inventory",
            ));
        }
        let mutation_stream = streams
            .iter()
            .find(|stream| stream.source_path == mutation_source_path)
            .ok_or(DurableError::Conflict(
                "navigation v5 mutation stream is absent",
            ))?;
        if mutation_stream.rows_scanned != claim_stream_rows_scanned
            || claim_closure_root != closure_root
            || navigation_claim_event_set_root_v4(
                event_id,
                event_sequence,
                &mutation_receipt_path,
                mutation_receipt_sha256,
                affected_owner_manifest_root,
                byte_limit,
            )? != event_claim_set_root
        {
            return Err(DurableError::Conflict(
                "navigation v5 Claim cause roots differ",
            ));
        }
        Some(NavigationEventManifestClaimSetV2 {
            owner_claim_ids,
            mutation_receipt_path,
            mutation_receipt_sha256,
            claim_stream_path: Some(mutation_source_path.clone()),
            claim_stream_sha256: Some(mutation_stream.stream_sha256),
            event_claim_set_root,
            revision_claim_id,
            revision_history_path,
            revision_history_sha256,
            revision_proof_root,
            owner_inventory_root: Some(owner_inventory_root),
            claim_stream_rows_scanned: Some(claim_stream_rows_scanned),
            mutation_source_path: Some(mutation_source_path),
            affected_owner_manifest_root: Some(affected_owner_manifest_root),
            closure_root: Some(claim_closure_root),
            reverse_claim_ids,
            streams,
        })
    } else {
        None
    };
    let claim_refresh_set = if let Some(refresh) = refresh_value.as_object() {
        if !manifest_v5_has_exact_fields(
            refresh,
            &[
                "basis_kind",
                "owner_inventory_root",
                "affected_owner_manifest_root",
                "closure_root",
                "reverse_claim_ids",
                "causes",
                "streams",
            ],
        ) || refresh.get("basis_kind").and_then(Value::as_str)
            != Some("claim-dependency-refresh")
        {
            return Err(DurableError::Corrupt(
                "navigation v5 Claim-refresh fields differ",
            ));
        }
        let owner_inventory_root = refresh
            .get("owner_inventory_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 refresh inventory root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 refresh inventory root absent",
            ))?;
        let affected_owner_manifest_root = refresh
            .get("affected_owner_manifest_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 refresh manifest root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 refresh manifest root absent",
            ))?;
        let refresh_closure_root = refresh
            .get("closure_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 refresh closure root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 refresh closure root absent",
            ))?;
        let reverse_claim_ids = manifest_v5_ids(
            refresh
                .get("reverse_claim_ids")
                .ok_or(DurableError::Corrupt(
                    "navigation v5 refresh reverse IDs absent",
                ))?,
            true,
        )?;
        let causes_value = refresh
            .get("causes")
            .and_then(Value::as_array)
            .filter(|causes| !causes.is_empty())
            .ok_or(DurableError::Corrupt("navigation v5 refresh causes absent"))?;
        let mut causes = Vec::with_capacity(causes_value.len());
        for value in causes_value {
            let cause = value
                .as_object()
                .ok_or(DurableError::Corrupt("navigation v5 refresh cause invalid"))?;
            if cause.len() != 3 {
                return Err(DurableError::Corrupt(
                    "navigation v5 refresh cause fields differ",
                ));
            }
            let changed_dependency_path = cause
                .get("changed_dependency_path")
                .and_then(Value::as_str)
                .filter(|path| !path.is_empty())
                .ok_or(DurableError::Corrupt(
                    "navigation v5 refresh cause path absent",
                ))?
                .to_owned();
            validate_relative_path(&changed_dependency_path)?;
            let changed_dependency_sha256 = cause
                .get("changed_dependency_sha256")
                .and_then(Value::as_str)
                .map(Digest256::from_hex)
                .transpose()
                .map_err(|_| DurableError::Corrupt("navigation v5 refresh cause digest invalid"))?
                .ok_or(DurableError::Corrupt(
                    "navigation v5 refresh cause digest absent",
                ))?;
            let cause_reverse_ids = manifest_v5_ids(
                cause.get("reverse_claim_ids").ok_or(DurableError::Corrupt(
                    "navigation v5 cause reverse IDs absent",
                ))?,
                true,
            )?;
            if causes.last().is_some_and(
                |previous: &DurableNavigationClaimRefreshCauseBindingV1| {
                    previous.changed_dependency_path >= changed_dependency_path
                },
            ) {
                return Err(DurableError::Conflict(
                    "navigation v5 refresh causes are not sorted",
                ));
            }
            causes.push(DurableNavigationClaimRefreshCauseBindingV1 {
                changed_dependency_path,
                changed_dependency_sha256,
                reverse_claim_ids: cause_reverse_ids,
            });
        }
        let streams = manifest_v5_streams(
            refresh.get("streams").ok_or(DurableError::Corrupt(
                "navigation v5 refresh streams absent",
            ))?,
            true,
        )?;
        let union_ids = causes
            .iter()
            .flat_map(|cause| cause.reverse_claim_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let stream_ids = streams
            .iter()
            .flat_map(|stream| stream.owner_claim_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        if union_ids != reverse_claim_ids.iter().cloned().collect::<BTreeSet<_>>()
            || reverse_claim_ids.is_empty() != streams.is_empty()
            || reverse_claim_ids.iter().any(|id| !stream_ids.contains(id))
            || refresh_closure_root != closure_root
            || navigation_claim_refresh_manifest_set_root_v1(
                owner_inventory_root,
                refresh_closure_root,
                &causes,
                &reverse_claim_ids,
                &streams,
                byte_limit,
            )? != affected_owner_manifest_root
        {
            return Err(DurableError::Conflict(
                "navigation v5 refresh closure or stream union differs",
            ));
        }
        Some(NavigationEventManifestClaimRefreshSetV1 {
            owner_inventory_root,
            affected_owner_manifest_root,
            closure_root: refresh_closure_root,
            reverse_claim_ids,
            causes,
            streams,
        })
    } else {
        None
    };
    let claim_empty_closure = if let Some(empty) = empty_closure_value.as_object() {
        if !manifest_v5_has_exact_fields(
            empty,
            &["basis_kind", "closure_root", "covered_source_paths"],
        ) || empty.get("basis_kind").and_then(Value::as_str)
            != Some("claim-empty-reverse-closure")
        {
            return Err(DurableError::Corrupt(
                "navigation v5 empty-closure fields differ",
            ));
        }
        let empty_closure_root = empty
            .get("closure_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 empty-closure root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 empty-closure root absent",
            ))?;
        let covered_source_paths =
            manifest_v5_source_paths(empty.get("covered_source_paths").ok_or(
                DurableError::Corrupt("navigation v5 empty-closure paths absent"),
            )?)?;
        if empty_closure_root != closure_root {
            return Err(DurableError::Conflict(
                "navigation v5 empty-closure root differs from event closure",
            ));
        }
        Some(NavigationEventManifestClaimEmptyClosureV1 {
            closure_root: empty_closure_root,
            covered_source_paths,
        })
    } else if empty_closure_value.is_null() {
        None
    } else {
        return Err(DurableError::Corrupt(
            "navigation v5 empty-closure field is invalid",
        ));
    };
    let mut parsed = BTreeMap::new();
    let mut previous_key: Option<(String, String)> = None;
    for cohort in cohorts {
        let cohort = cohort.as_object().ok_or(DurableError::Corrupt(
            "navigation v5 cohort row is not an object",
        ))?;
        if !manifest_v5_has_exact_fields(
            cohort,
            &[
                "source_path",
                "record_id",
                "owner_kind",
                "basis_kind",
                "basis_dependency_path",
                "basis_dependency_sha256",
                "basis_dependency_causes",
                "basis_dependency_causes_root",
                "basis_history_path",
                "basis_history_sha256",
                "basis_claim_id",
                "basis_revision_proof_root",
                "claim_owner_inventory_root",
                "claim_stream_rows_scanned",
                "claim_event_basis_root",
                "claim_navigation_manifest_root",
                "claim_source_line",
                "claim_trace_sha256",
                "source_revision",
                "predecessor_cohort_root",
                "input_root",
                "dependency_root",
                "output_root",
            ],
        ) {
            return Err(DurableError::Corrupt("navigation v5 cohort fields differ"));
        }
        let source_path = cohort
            .get("source_path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .ok_or(DurableError::Corrupt("navigation v5 owner path absent"))?
            .to_owned();
        validate_relative_path(&source_path)?;
        let record_id = cohort
            .get("record_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt("navigation v5 owner ID absent"))?
            .to_owned();
        validate_identifier(
            &record_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation v5 owner ID",
        )?;
        let key = (source_path.clone(), record_id.clone());
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &key)
        {
            return Err(DurableError::Conflict(
                "navigation v5 owners are not C-sorted",
            ));
        }
        previous_key = Some(key.clone());
        let owner_kind = cohort
            .get("owner_kind")
            .and_then(Value::as_str)
            .filter(|kind| matches!(*kind, "record" | "claim"))
            .ok_or(DurableError::Corrupt("navigation v5 owner kind invalid"))?
            .to_owned();
        let basis_kind = cohort
            .get("basis_kind")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("navigation v5 basis absent"))?
            .to_owned();
        let basis_dependency_path = manifest_v5_optional_string(cohort, "basis_dependency_path")?;
        let basis_dependency_sha256 =
            manifest_v5_optional_digest(cohort, "basis_dependency_sha256")?;
        let basis_dependency_causes_root =
            manifest_v5_optional_digest(cohort, "basis_dependency_causes_root")?;
        let causes_value = cohort
            .get("basis_dependency_causes")
            .ok_or(DurableError::Corrupt(
                "navigation v5 dependency-refresh causes absent",
            ))?;
        let basis_dependency_causes = match (causes_value, basis_dependency_causes_root) {
            (Value::Null, None) => Vec::new(),
            (Value::Array(rows), Some(root)) => {
                let raw = serde_json::to_string(&Value::Array(rows.clone()))
                    .map_err(|_| DurableError::Corrupt("navigation v5 causes JSON"))?;
                parse_navigation_record_dependency_refresh_causes(
                    &raw,
                    root,
                    &source_path,
                    byte_limit,
                )?
            }
            _ => {
                return Err(DurableError::Corrupt(
                    "navigation v5 dependency-refresh cause root differs",
                ));
            }
        };
        let basis_history_path = manifest_v5_optional_string(cohort, "basis_history_path")?;
        let basis_history_sha256 = manifest_v5_optional_digest(cohort, "basis_history_sha256")?;
        let basis_claim_id = match cohort.get("basis_claim_id") {
            Some(Value::Null) => None,
            Some(Value::String(id)) if id.starts_with("tos.claim.") => {
                validate_identifier(
                    id,
                    MAX_NAVIGATION_RECORD_ID_BYTES,
                    "navigation v5 basis Claim ID",
                )?;
                Some(id.clone())
            }
            _ => {
                return Err(DurableError::Corrupt(
                    "navigation v5 basis Claim ID invalid",
                ));
            }
        };
        let basis_revision_proof_root =
            manifest_v5_optional_digest(cohort, "basis_revision_proof_root")?;
        let claim_owner_inventory_root =
            manifest_v5_optional_digest(cohort, "claim_owner_inventory_root")?;
        let claim_stream_rows_scanned = match cohort.get("claim_stream_rows_scanned") {
            Some(Value::Null) => None,
            Some(value) => Some(value.as_u64().filter(|rows| *rows > 0).ok_or(
                DurableError::Corrupt("navigation v5 Claim stream row count invalid"),
            )?),
            None => {
                return Err(DurableError::Corrupt(
                    "navigation v5 Claim stream row count absent",
                ));
            }
        };
        let claim_event_basis_root = manifest_v5_optional_digest(cohort, "claim_event_basis_root")?;
        let claim_navigation_manifest_root =
            manifest_v5_optional_digest(cohort, "claim_navigation_manifest_root")?;
        let claim_source_line = match cohort.get("claim_source_line") {
            Some(Value::Null) => None,
            Some(value) => Some(value.as_u64().filter(|line| *line > 0).ok_or(
                DurableError::Corrupt("navigation v5 Claim source line invalid"),
            )?),
            None => {
                return Err(DurableError::Corrupt(
                    "navigation v5 Claim source line absent",
                ));
            }
        };
        let claim_trace_sha256 = manifest_v5_optional_digest(cohort, "claim_trace_sha256")?;
        let source_revision = cohort
            .get("source_revision")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 source revision invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 source revision absent",
            ))?;
        let input_root = cohort
            .get("input_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 input root invalid"))?
            .ok_or(DurableError::Corrupt("navigation v5 input root absent"))?;
        let dependency_root = cohort
            .get("dependency_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 dependency root invalid"))?
            .ok_or(DurableError::Corrupt(
                "navigation v5 dependency root absent",
            ))?;
        let output_root = cohort
            .get("output_root")
            .and_then(Value::as_str)
            .map(Digest256::from_hex)
            .transpose()
            .map_err(|_| DurableError::Corrupt("navigation v5 output root invalid"))?
            .ok_or(DurableError::Corrupt("navigation v5 output root absent"))?;
        let predecessor = cohort
            .get("predecessor_cohort_root")
            .ok_or(DurableError::Corrupt(
                "navigation v5 predecessor field absent",
            ))?;
        if !predecessor.is_null()
            && predecessor
                .as_str()
                .and_then(|value| Digest256::from_hex(value).ok())
                .is_none()
        {
            return Err(DurableError::Corrupt(
                "navigation v5 predecessor root invalid",
            ));
        }
        if owner_kind == "claim" {
            let (set_streams, expected_event_root, expected_manifest_root, expected_inventory_root) =
                if let Some(set) = &claim_set {
                    (
                        &set.streams,
                        Some(set.event_claim_set_root),
                        set.affected_owner_manifest_root,
                        set.owner_inventory_root,
                    )
                } else if let Some(set) = &claim_refresh_set {
                    (
                        &set.streams,
                        None,
                        Some(set.affected_owner_manifest_root),
                        Some(set.owner_inventory_root),
                    )
                } else {
                    return Err(DurableError::Conflict(
                        "navigation v5 Claim owner lacks cause set",
                    ));
                };
            let stream = set_streams
                .iter()
                .find(|stream| stream.source_path == source_path)
                .ok_or(DurableError::Conflict(
                    "navigation v5 Claim owner stream absent",
                ))?;
            if stream.owner_claim_ids.binary_search(&record_id).is_err()
                || claim_owner_inventory_root != expected_inventory_root
                || claim_stream_rows_scanned != Some(stream.rows_scanned)
                || claim_navigation_manifest_root != expected_manifest_root
                || claim_source_line.is_none()
                || claim_trace_sha256.is_none()
            {
                return Err(DurableError::Conflict(
                    "navigation v5 Claim owner evidence differs",
                ));
            }
            if let Some(set) = &claim_set {
                if basis_kind != "claim-mutation"
                    || claim_event_basis_root != expected_event_root
                    || basis_dependency_path.is_some()
                    || basis_dependency_sha256.is_some()
                    || !basis_dependency_causes.is_empty()
                    || basis_dependency_causes_root.is_some()
                    || basis_history_path.is_some()
                    || basis_history_sha256.is_some()
                    || basis_claim_id.is_some()
                    || set.revision_claim_id.as_deref() == Some(&record_id)
                        && set.revision_proof_root != basis_revision_proof_root
                    || set.revision_claim_id.as_deref() != Some(&record_id)
                        && basis_revision_proof_root.is_some()
                {
                    return Err(DurableError::Conflict(
                        "navigation v5 Claim mutation owner basis differs",
                    ));
                }
            } else if basis_kind != "claim-dependency-refresh"
                || claim_event_basis_root.is_none()
                || basis_revision_proof_root.is_some()
                || basis_dependency_path.is_some()
                || basis_dependency_sha256.is_some()
                || !basis_dependency_causes.is_empty()
                || basis_dependency_causes_root.is_some()
                || basis_history_path.is_some()
                || basis_history_sha256.is_some()
                || basis_claim_id.is_some()
            {
                return Err(DurableError::Conflict(
                    "navigation v5 Claim refresh owner basis differs",
                ));
            }
        } else {
            if claim_owner_inventory_root.is_some()
                || claim_stream_rows_scanned.is_some()
                || claim_event_basis_root.is_some()
                || claim_navigation_manifest_root.is_some()
                || claim_source_line.is_some()
                || claim_trace_sha256.is_some()
            {
                return Err(DurableError::Conflict(
                    "navigation v5 Record row contains Claim metadata",
                ));
            }
            let basis_is_valid = match basis_kind.as_str() {
                "record-mutation" => {
                    basis_dependency_path.is_none()
                        && basis_dependency_sha256.is_none()
                        && basis_dependency_causes.is_empty()
                        && basis_dependency_causes_root.is_none()
                        && basis_history_path.is_none()
                        && basis_history_sha256.is_none()
                        && basis_claim_id.is_none()
                        && basis_revision_proof_root.is_none()
                }
                "dependency-refresh" => {
                    basis_dependency_path.is_some()
                        && basis_dependency_sha256.is_some()
                        && basis_dependency_causes.is_empty()
                        && basis_dependency_causes_root.is_none()
                        && basis_history_path.is_none()
                        && basis_history_sha256.is_none()
                        && basis_claim_id.is_none()
                        && basis_revision_proof_root.is_none()
                }
                "dependency-refresh-set" => {
                    basis_dependency_path.is_none()
                        && basis_dependency_sha256.is_none()
                        && !basis_dependency_causes.is_empty()
                        && basis_dependency_causes_root.is_some()
                        && basis_history_path.is_none()
                        && basis_history_sha256.is_none()
                        && basis_claim_id.is_none()
                        && basis_revision_proof_root.is_none()
                }
                "claim-revision-refresh" => {
                    basis_dependency_path.is_some()
                        && basis_dependency_sha256.is_some()
                        && basis_dependency_causes.is_empty()
                        && basis_dependency_causes_root.is_none()
                        && basis_history_path.is_some()
                        && basis_history_sha256.is_some()
                        && basis_claim_id.as_ref().is_some_and(|id| {
                            claim_set.as_ref().is_some_and(|set| {
                                set.revision_claim_id.as_deref() == Some(id)
                                    && set.revision_proof_root == basis_revision_proof_root
                            })
                        })
                }
                _ => false,
            };
            if !basis_is_valid {
                return Err(DurableError::Conflict(
                    "navigation v5 Record owner basis is inconsistent",
                ));
            }
        }
        parsed.insert(
            key,
            NavigationEventManifestCohortV2 {
                source_path,
                record_id,
                owner_kind,
                basis_kind,
                basis_dependency_path,
                basis_dependency_sha256,
                basis_dependency_causes,
                basis_dependency_causes_root,
                basis_history_path,
                basis_history_sha256,
                basis_claim_id,
                basis_revision_proof_root,
                claim_owner_inventory_root,
                claim_stream_rows_scanned,
                claim_event_basis_root,
                claim_navigation_manifest_root,
                claim_source_line,
                claim_trace_sha256,
                source_event_id: None,
                source_event_version: None,
                predecessor_source_revision: None,
                predecessor_cohort_root: None,
                source_revision,
                input_root,
                dependency_root,
                output_root,
            },
        );
    }
    if parsed.len() != stored_count as usize {
        return Err(DurableError::Conflict("navigation v5 owner count differs"));
    }
    if let Some(set) = &claim_set {
        let claim_rows = parsed
            .values()
            .filter(|row| row.owner_kind == "claim")
            .collect::<Vec<_>>();
        let ids = claim_rows
            .iter()
            .map(|row| row.record_id.clone())
            .collect::<BTreeSet<_>>();
        if ids != set.owner_claim_ids.iter().cloned().collect::<BTreeSet<_>>()
            || claim_rows.len() != set.owner_claim_ids.len()
        {
            return Err(DurableError::Conflict(
                "navigation v5 Claim rows differ from full owner IDs",
            ));
        }
        for row in &claim_rows {
            if row.basis_kind != "claim-mutation"
                || row.claim_event_basis_root != Some(set.event_claim_set_root)
                || row.claim_navigation_manifest_root != set.affected_owner_manifest_root
            {
                return Err(DurableError::Conflict(
                    "navigation v5 Claim mutation row roots differ",
                ));
            }
        }
        if let Some(claim_id) = &set.revision_claim_id {
            let revision_rows = claim_rows
                .iter()
                .filter(|row| row.basis_revision_proof_root.is_some())
                .collect::<Vec<_>>();
            if revision_rows.len() != 1
                || revision_rows[0].record_id != *claim_id
                || revision_rows[0].basis_revision_proof_root != set.revision_proof_root
            {
                return Err(DurableError::Conflict(
                    "navigation v5 Claim revision row differs",
                ));
            }
        }
    }
    if let Some(set) = &claim_refresh_set {
        let claim_rows = parsed
            .values()
            .filter(|row| row.owner_kind == "claim")
            .collect::<Vec<_>>();
        let ids = claim_rows
            .iter()
            .map(|row| row.record_id.clone())
            .collect::<BTreeSet<_>>();
        let expected_ids = set
            .streams
            .iter()
            .flat_map(|stream| stream.owner_claim_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        if ids != expected_ids || claim_rows.len() != expected_ids.len() {
            return Err(DurableError::Conflict(
                "navigation v5 Claim refresh rows differ from closure",
            ));
        }
        for row in &claim_rows {
            if row.basis_kind != "claim-dependency-refresh"
                || row.claim_navigation_manifest_root != Some(set.affected_owner_manifest_root)
            {
                return Err(DurableError::Conflict(
                    "navigation v5 Claim refresh row root differs",
                ));
            }
        }
    }
    if claim_empty_closure.is_some()
        && (claim_set.is_some()
            || claim_refresh_set.is_some()
            || parsed.values().any(|row| {
                row.owner_kind == "claim"
                    || row.owner_kind == "record"
                        && row.basis_kind == "record-mutation"
                        && claim_empty_closure.as_ref().is_some_and(|empty| {
                            !empty
                                .covered_source_paths
                                .iter()
                                .any(|path| path == &row.source_path)
                        })
            }))
    {
        return Err(DurableError::Conflict(
            "navigation v5 empty Claim closure contains Claim owners",
        ));
    }
    for row in parsed.values().filter(|row| row.owner_kind == "record") {
        match row.basis_kind.as_str() {
            "dependency-refresh" => {
                let Some(path) = row.basis_dependency_path.as_deref() else {
                    return Err(DurableError::Conflict(
                        "navigation v5 Record refresh path is absent",
                    ));
                };
                let bound_by_refresh = claim_refresh_set.as_ref().is_some_and(|refresh_set| {
                    refresh_set.causes.iter().any(|cause| {
                        path == cause.changed_dependency_path
                            && row.basis_dependency_sha256 == Some(cause.changed_dependency_sha256)
                    })
                });
                let bound_by_empty = claim_empty_closure.as_ref().is_some_and(|empty| {
                    empty
                        .covered_source_paths
                        .iter()
                        .any(|covered| covered == path)
                });
                if !bound_by_refresh && !bound_by_empty {
                    return Err(DurableError::Conflict(
                        "navigation v5 Record refresh differs from its changed-path manifest",
                    ));
                }
            }
            "dependency-refresh-set" => {
                if row.basis_dependency_causes.is_empty() {
                    return Err(DurableError::Conflict(
                        "navigation v5 Record refresh cause set is empty",
                    ));
                }
                let causes_match_refresh = claim_refresh_set.as_ref().is_some_and(|refresh_set| {
                    row.basis_dependency_causes.iter().all(|cause| {
                        refresh_set.causes.iter().any(|event_cause| {
                            event_cause.changed_dependency_path == cause.path.as_str()
                                && event_cause.changed_dependency_sha256 == cause.sha256
                        })
                    })
                });
                let causes_match_empty = claim_empty_closure.as_ref().is_some_and(|empty| {
                    row.basis_dependency_causes.iter().all(|cause| {
                        empty
                            .covered_source_paths
                            .iter()
                            .any(|covered| covered == cause.path.as_str())
                    })
                });
                if !causes_match_refresh && !causes_match_empty {
                    return Err(DurableError::Conflict(
                        "navigation v5 Record cause set differs from changed-path manifest",
                    ));
                }
            }
            "claim-revision-refresh" => {
                let Some(claim_set) = &claim_set else {
                    return Err(DurableError::Conflict(
                        "navigation v5 Record Claim refresh lacks mutation proof",
                    ));
                };
                let (
                    Some(stream_path),
                    Some(stream_sha),
                    Some(history_path),
                    Some(history_sha),
                    Some(claim_id),
                    Some(proof_root),
                ) = (
                    claim_set.claim_stream_path.as_deref(),
                    claim_set.claim_stream_sha256,
                    claim_set.revision_history_path.as_deref(),
                    claim_set.revision_history_sha256,
                    claim_set.revision_claim_id.as_deref(),
                    claim_set.revision_proof_root,
                )
                else {
                    return Err(DurableError::Conflict(
                        "navigation v5 Record Claim refresh proof is incomplete",
                    ));
                };
                if row.basis_dependency_path.as_deref() != Some(stream_path)
                    || row.basis_dependency_sha256 != Some(stream_sha)
                    || row.basis_history_path.as_deref() != Some(history_path)
                    || row.basis_history_sha256 != Some(history_sha)
                    || row.basis_claim_id.as_deref() != Some(claim_id)
                    || row.basis_revision_proof_root != Some(proof_root)
                {
                    return Err(DurableError::Conflict(
                        "navigation v5 Record Claim refresh differs from mutation proof",
                    ));
                }
            }
            _ => {}
        }
    }
    if canonical_value(&value, byte_limit)? != raw {
        return Err(DurableError::Corrupt(
            "navigation v5 manifest is not canonical",
        ));
    }
    Ok(NavigationEventManifestV2 {
        cohorts: parsed,
        claim_set,
        claim_refresh_set,
        claim_empty_closure,
        claim_range_set: None,
    })
}

fn navigation_claim_event_set_root_v4(
    event_id: &str,
    event_sequence: u64,
    receipt_path: &str,
    receipt_sha256: Digest256,
    affected_owner_manifest_root: Digest256,
    byte_limit: usize,
) -> DurableResult<Digest256> {
    if event_id.is_empty() || event_sequence == 0 {
        return Err(DurableError::Conflict(
            "navigation Claim event identity absent",
        ));
    }
    validate_relative_path(receipt_path)?;
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_event_members_v4",
            "event_id":event_id,
            "event_sequence":event_sequence,
            "mutation_receipt_path":receipt_path,
            "mutation_receipt_sha256":receipt_sha256.to_hex(),
            "affected_owner_manifest_root":affected_owner_manifest_root.to_hex(),
        }),
        byte_limit,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_navigation_event_cohort_manifest_single(
    raw: &[u8],
    stored_root: Digest256,
    stored_count: u32,
    event_id: &str,
    event_sequence: u64,
    source_path: &str,
    record_id: &str,
    source_revision: Digest256,
    input_root: Digest256,
    dependency_root: Digest256,
    output_root: Digest256,
    byte_limit: usize,
) -> DurableResult<()> {
    let manifest = verify_navigation_event_cohort_manifest(
        raw,
        stored_root,
        stored_count,
        event_id,
        event_sequence,
        byte_limit,
    )?;
    let owner = manifest
        .cohorts
        .get(&(source_path.to_owned(), record_id.to_owned()))
        .ok_or(DurableError::Conflict(
            "navigation coverage owner is absent from its cohort manifest",
        ))?;
    if owner.source_revision != source_revision
        || owner.input_root != input_root
        || owner.dependency_root != dependency_root
        || owner.output_root != output_root
    {
        return Err(DurableError::Conflict(
            "navigation coverage owner roots differ from its cohort manifest",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct NavigationAdjacencyCertificate {
    node_id: String,
    edge_count: u64,
    edge_digest: Digest256,
    certificate_root: Digest256,
}

fn navigation_adjacency_certificate_root_v1(
    bootstrap_root: Digest256,
    node_id: &str,
    edge_count: u64,
    edge_digest: Digest256,
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-adjacency-certificate-v1");
    navigation_hash_part(&mut hash, bootstrap_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, node_id.as_bytes());
    navigation_hash_part(&mut hash, &edge_count.to_be_bytes());
    navigation_hash_part(&mut hash, edge_digest.as_bytes());
    hash.finalize()
}

pub(crate) fn read_navigation_adjacency_certificate_v1(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    node_id: &str,
    expected_bootstrap_root: Digest256,
) -> DurableResult<Option<DurableNavigationAdjacencyCertificateReadV1>> {
    validate_identifier(
        domain,
        super::MAX_DOMAIN_BYTES,
        "navigation certificate domain",
    )?;
    validate_identifier(
        consumer_id,
        super::MAX_CONSUMER_ID_BYTES,
        "navigation certificate consumer ID",
    )?;
    validate_identifier(
        node_id,
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "navigation certificate node ID",
    )?;
    let row = tx.query_opt(
        "SELECT edge_count,edge_digest,certificate_root
         FROM cmd2_projection_navigation_adjacency_certificate
         WHERE domain=$1 AND consumer_id=$2 AND node_id=$3",
        &[&domain, &consumer_id, &node_id],
    )?;
    let Some(row) = row else {
        return Ok(None);
    };
    let edge_count = as_u64(row.get("edge_count"))?;
    let edge_digest = parse_digest(row.get("edge_digest"))?;
    let certificate_root = parse_digest(row.get("certificate_root"))?;
    if navigation_adjacency_certificate_root_v1(
        expected_bootstrap_root,
        node_id,
        edge_count,
        edge_digest,
    ) != certificate_root
    {
        return Err(DurableError::Corrupt(
            "navigation adjacency certificate root differs",
        ));
    }
    Ok(Some(DurableNavigationAdjacencyCertificateReadV1 {
        edge_count,
        edge_digest,
    }))
}

fn navigation_adjacency_root_initial(
    bootstrap_root: Digest256,
    certificates: &[NavigationAdjacencyCertificate],
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-adjacency-root-v1");
    navigation_hash_part(&mut hash, bootstrap_root.to_hex().as_bytes());
    for certificate in certificates {
        navigation_hash_part(&mut hash, certificate.node_id.as_bytes());
        navigation_hash_part(&mut hash, &certificate.edge_count.to_be_bytes());
        navigation_hash_part(&mut hash, certificate.edge_digest.as_bytes());
        navigation_hash_part(&mut hash, certificate.certificate_root.as_bytes());
    }
    hash.finalize()
}

fn navigation_adjacency_root_next(
    previous_root: Digest256,
    event_sequence: u64,
    updates: &BTreeMap<String, Option<NavigationAdjacencyCertificate>>,
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-adjacency-update-v1");
    navigation_hash_part(&mut hash, previous_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, &event_sequence.to_be_bytes());
    for (node_id, certificate) in updates {
        navigation_hash_part(&mut hash, node_id.as_bytes());
        if let Some(certificate) = certificate {
            navigation_hash_part(&mut hash, &[1]);
            navigation_hash_part(&mut hash, &certificate.edge_count.to_be_bytes());
            navigation_hash_part(&mut hash, certificate.edge_digest.as_bytes());
            navigation_hash_part(&mut hash, certificate.certificate_root.as_bytes());
        } else {
            navigation_hash_part(&mut hash, &[0]);
        }
    }
    hash.finalize()
}

fn navigation_edge_root_item(root: &mut Digest256Hasher, edge_id: &str, sha256: Digest256) {
    root.update(&(edge_id.len() as u64).to_be_bytes());
    root.update(edge_id.as_bytes());
    root.update(sha256.as_bytes());
}

fn make_navigation_adjacency_certificate(
    bootstrap_root: Digest256,
    node_id: String,
    outgoing: &BTreeMap<String, Digest256>,
) -> NavigationAdjacencyCertificate {
    let mut edge_digest = Digest256Hasher::new();
    for (edge_id, sha256) in outgoing {
        navigation_edge_root_item(&mut edge_digest, edge_id, *sha256);
    }
    let edge_digest = edge_digest.finalize();
    let edge_count = outgoing.len() as u64;
    let certificate_root =
        navigation_adjacency_certificate_root_v1(bootstrap_root, &node_id, edge_count, edge_digest);
    NavigationAdjacencyCertificate {
        node_id,
        edge_count,
        edge_digest,
        certificate_root,
    }
}

fn bootstrap_adjacency_certificates(
    bootstrap_root: Digest256,
    rows: &[DurableNavigationBaseRowReadV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<Vec<NavigationAdjacencyCertificate>> {
    let mut visible_nodes = BTreeMap::<String, bool>::new();
    let mut all_node_ids = BTreeMap::<&str, &DurableNavigationBaseRowReadV1>::new();
    let mut all_edge_ids = BTreeMap::<&str, &DurableNavigationBaseRowReadV1>::new();
    let mut outgoing = BTreeMap::<String, BTreeMap<String, Digest256>>::new();
    let mut adjacency_rows = 0usize;
    let mut adjacency_bytes = 0u64;
    for row in rows {
        match row.kind {
            DurableNavigationBootstrapRowKindV1::Node => {
                if let Some(prior) = all_node_ids.insert(&row.id, row) {
                    if prior.sha256 != row.sha256
                        || prior.raw != row.raw
                        || prior.visible != row.visible
                    {
                        return Err(DurableError::Conflict(
                            "navigation bootstrap shared node contributions conflict",
                        ));
                    }
                    continue;
                }
                let visible = row.visible.ok_or(DurableError::Corrupt(
                    "navigation bootstrap node visibility absent",
                ))?;
                visible_nodes.insert(row.id.clone(), visible);
                if visible {
                    outgoing.entry(row.id.clone()).or_default();
                }
            }
            DurableNavigationBootstrapRowKindV1::Edge => {
                if let Some(prior) = all_edge_ids.insert(&row.id, row) {
                    if prior.sha256 != row.sha256
                        || prior.raw != row.raw
                        || prior.visible != row.visible
                        || prior.from_id != row.from_id
                        || prior.to_id != row.to_id
                    {
                        return Err(DurableError::Conflict(
                            "navigation bootstrap shared edge contributions conflict",
                        ));
                    }
                    continue;
                }
                let visible = row.visible.ok_or(DurableError::Corrupt(
                    "navigation bootstrap edge visibility absent",
                ))?;
                let from_id = row.from_id.as_deref().ok_or(DurableError::Corrupt(
                    "navigation bootstrap edge source absent",
                ))?;
                let to_id = row.to_id.as_deref().ok_or(DurableError::Corrupt(
                    "navigation bootstrap edge target absent",
                ))?;
                let from_visible =
                    visible_nodes
                        .get(from_id)
                        .copied()
                        .ok_or(DurableError::Conflict(
                            "navigation bootstrap edge source receipt absent",
                        ))?;
                let to_visible =
                    visible_nodes
                        .get(to_id)
                        .copied()
                        .ok_or(DurableError::Conflict(
                            "navigation bootstrap edge target receipt absent",
                        ))?;
                if visible != (from_visible && to_visible) {
                    return Err(DurableError::Conflict(
                        "navigation bootstrap edge visibility differs from endpoints",
                    ));
                }
                if visible {
                    adjacency_rows = adjacency_rows
                        .checked_add(1)
                        .ok_or(DurableError::Refused("navigation adjacency row count"))?;
                    adjacency_bytes = adjacency_bytes
                        .checked_add(row.raw.len() as u64)
                        .ok_or(DurableError::Refused("navigation adjacency byte count"))?;
                    if adjacency_rows > limits.max_adjacency_rows
                        || adjacency_bytes > limits.max_adjacency_bytes
                    {
                        return Err(DurableError::Refused(
                            "navigation adjacency certificate budget",
                        ));
                    }
                    if outgoing
                        .entry(from_id.to_owned())
                        .or_default()
                        .insert(row.id.clone(), row.sha256)
                        .is_some()
                    {
                        return Err(DurableError::Conflict(
                            "navigation bootstrap has duplicate effective edge IDs",
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    let mut certificates = Vec::with_capacity(outgoing.len());
    for (node_id, outgoing) in outgoing {
        certificates.push(make_navigation_adjacency_certificate(
            bootstrap_root,
            node_id,
            &outgoing,
        ));
    }
    Ok(certificates)
}

#[cfg(test)]
mod bootstrap_adjacency_tests {
    use super::*;

    #[test]
    fn shared_bootstrap_rows_have_one_adjacency_and_conflicts_refuse() {
        let row = |kind, id: &str, from: Option<&str>, to: Option<&str>| {
            let raw = serde_json::to_vec(&json!({"id":id,"from":from,"to":to})).unwrap();
            DurableNavigationBaseRowReadV1 {
                kind,
                id: id.into(),
                owner_source_path: "ToS/source-witnesses/claims/a/source-claims.jsonl".into(),
                owner_record_id: "claim:a".into(),
                active: true,
                visible: Some(true),
                sha256: Digest256::of_bytes(&raw),
                raw,
                from_id: from.map(str::to_owned),
                to_id: to.map(str::to_owned),
                source_refs: Vec::new(),
            }
        };
        let node = row(
            DurableNavigationBootstrapRowKindV1::Node,
            "shared",
            None,
            None,
        );
        let target = row(
            DurableNavigationBootstrapRowKindV1::Node,
            "target",
            None,
            None,
        );
        let edge = row(
            DurableNavigationBootstrapRowKindV1::Edge,
            "edge",
            Some("shared"),
            Some("target"),
        );
        let mut limits = DurableNavigationEventLimits::default();
        limits.max_adjacency_rows = 1;
        let root = Digest256::of_bytes(b"bootstrap");
        let baseline = bootstrap_adjacency_certificates(
            root,
            &[node.clone(), target.clone(), edge.clone()],
            limits,
        )
        .unwrap();
        let mut shared_node = node.clone();
        shared_node.owner_record_id = "claim:b".into();
        let mut shared_edge = edge.clone();
        shared_edge.owner_record_id = "claim:b".into();
        let rows = vec![node, shared_node, target, edge, shared_edge];
        let actual = bootstrap_adjacency_certificates(root, &rows, limits).unwrap();
        assert_eq!(actual.len(), 2);
        for (left, right) in baseline.iter().zip(&actual) {
            assert_eq!(left.node_id, right.node_id);
            assert_eq!(left.edge_count, right.edge_count);
            assert_eq!(left.certificate_root, right.certificate_root);
        }
        assert_eq!(actual[0].edge_count, 1);
        for case in 0..4 {
            let mut conflicting = rows.clone();
            match case {
                0 => conflicting[1].visible = Some(false),
                1 => {
                    conflicting[1].raw.push(b' ');
                    conflicting[1].sha256 = Digest256::of_bytes(&conflicting[1].raw);
                }
                2 => conflicting[4].to_id = Some("shared".into()),
                _ => conflicting[4].id = "another-edge".into(),
            }
            assert!(bootstrap_adjacency_certificates(root, &conflicting, limits).is_err());
        }
    }
}

fn navigation_coverage_root_next(
    previous_root: Digest256,
    event_sequence: u64,
    event_id: &str,
    source_path: &str,
    record_id: &str,
    source_revision: Digest256,
    input_root: Digest256,
    dependency_root: Digest256,
    output_root: Digest256,
    cohort_set_root: Digest256,
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-coverage-event-v2");
    navigation_hash_part(&mut hash, previous_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, &event_sequence.to_be_bytes());
    navigation_hash_part(&mut hash, event_id.as_bytes());
    navigation_hash_part(&mut hash, source_path.as_bytes());
    navigation_hash_part(&mut hash, record_id.as_bytes());
    navigation_hash_part(&mut hash, source_revision.to_hex().as_bytes());
    navigation_hash_part(&mut hash, input_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, dependency_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, output_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, cohort_set_root.to_hex().as_bytes());
    hash.finalize()
}

#[derive(Clone)]
struct SelectedVersion {
    revision: u64,
    commit_sequence: u64,
    content_length: u64,
    content_digest: Digest256,
    is_source: bool,
}

#[derive(Clone)]
struct PersistedRows {
    dependencies: Vec<NavigationDependencyV1>,
    nodes: Vec<PersistedNavigationNode>,
    edges: Vec<PersistedNavigationEdge>,
    diagnostics: Vec<Vec<u8>>,
    cohort_bytes: u64,
}

#[derive(Clone)]
struct NavigationEventOwnerV2 {
    event_id: String,
    event_sequence: u64,
    source_event_id: String,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    basis_kind: String,
    basis_dependency_path: Option<String>,
    basis_dependency_sha256: Option<Digest256>,
    basis_dependency_causes_json: Option<String>,
    basis_dependency_causes_root: Option<Digest256>,
    basis_history_path: Option<String>,
    basis_history_sha256: Option<Digest256>,
    basis_claim_id: Option<String>,
    basis_revision_proof_root: Option<Digest256>,
    claim_owner_inventory_root: Option<Digest256>,
    claim_stream_rows_scanned: Option<u64>,
    claim_event_basis_root: Option<Digest256>,
    claim_navigation_manifest_root: Option<Digest256>,
    source_path: RelativePath,
    record_id: String,
    predecessor_cohort_root: Option<Digest256>,
    dependency_root: Digest256,
    input_root: Digest256,
    output_root: Digest256,
    dependencies: Vec<NavigationDependencyV1>,
}

impl NavigationEventOwnerV2 {
    fn from_record(
        cohort: &NavigationRecordEventCohortV1,
        byte_limit: usize,
    ) -> DurableResult<Self> {
        let (
            basis_kind,
            basis_dependency_path,
            basis_dependency_sha256,
            basis_history_path,
            basis_history_sha256,
            basis_claim_id,
            basis_revision_proof_root,
        ) = match cohort.basis() {
            NavigationRecordEventBasisV1::Mutation => {
                ("record-mutation", None, None, None, None, None, None)
            }
            NavigationRecordEventBasisV1::DependencyRefresh {
                changed_dependency_path,
                changed_dependency_sha256,
            } => (
                "dependency-refresh",
                Some(changed_dependency_path.as_str().to_owned()),
                Some(*changed_dependency_sha256),
                None,
                None,
                None,
                None,
            ),
            NavigationRecordEventBasisV1::DependencyRefreshSet { .. } => {
                ("dependency-refresh-set", None, None, None, None, None, None)
            }
            NavigationRecordEventBasisV1::RangeReconciliation { .. } => {
                ("range-reconciliation", None, None, None, None, None, None)
            }
            NavigationRecordEventBasisV1::ClaimRevisionRefresh {
                changed_dependency_path,
                changed_dependency_sha256,
                history_path,
                history_sha256,
                claim_id,
                revision_proof_root,
            } => (
                "claim-revision-refresh",
                Some(changed_dependency_path.as_str().to_owned()),
                Some(*changed_dependency_sha256),
                Some(history_path.as_str().to_owned()),
                Some(*history_sha256),
                Some(claim_id.clone()),
                Some(*revision_proof_root),
            ),
        };
        let (basis_dependency_causes_json, basis_dependency_causes_root) =
            if let NavigationRecordEventBasisV1::DependencyRefreshSet {
                changed_dependencies,
            } = cohort.basis()
            {
                let (causes, root) = navigation_record_dependency_refresh_causes_value(
                    changed_dependencies,
                    cohort.source_path.as_str(),
                    byte_limit,
                )?;
                (
                    Some(serde_json::to_string(&causes).map_err(|_| {
                        DurableError::Invalid("navigation dependency-refresh causes JSON")
                    })?),
                    Some(root),
                )
            } else {
                (None, None)
            };
        Ok(Self {
            event_id: cohort.event_id.clone(),
            event_sequence: cohort.event_sequence,
            source_event_id: cohort.source_event_id.clone(),
            source_event_version: cohort.source_event_version,
            predecessor_source_revision: cohort.predecessor_source_revision,
            source_revision: cohort.source_revision,
            basis_kind: basis_kind.to_owned(),
            basis_dependency_path,
            basis_dependency_sha256,
            basis_dependency_causes_json,
            basis_dependency_causes_root,
            basis_history_path,
            basis_history_sha256,
            basis_claim_id,
            basis_revision_proof_root,
            claim_owner_inventory_root: None,
            claim_stream_rows_scanned: None,
            claim_event_basis_root: None,
            claim_navigation_manifest_root: None,
            source_path: cohort.source_path.clone(),
            record_id: cohort.record_id.clone(),
            predecessor_cohort_root: cohort.predecessor_cohort_root,
            dependency_root: cohort.dependency_root,
            input_root: cohort.input_root,
            output_root: cohort.output_root,
            dependencies: cohort.dependencies.clone(),
        })
    }

    fn from_claim(
        cohort: &NavigationClaimEventCohortV1,
        integrity: &tos_compiler::source_navigation_claim_event::NavigationClaimEventIntegrityV1<
            '_,
        >,
        manifest_root: Digest256,
    ) -> Self {
        Self {
            event_id: cohort.event_id().to_owned(),
            event_sequence: cohort.event_sequence(),
            source_event_id: cohort.source_event_id().to_owned(),
            source_event_version: cohort.source_event_version(),
            predecessor_source_revision: cohort.predecessor_source_revision(),
            source_revision: cohort.source_revision(),
            basis_kind: "claim-mutation".to_owned(),
            basis_dependency_path: None,
            basis_dependency_sha256: None,
            basis_dependency_causes_json: None,
            basis_dependency_causes_root: None,
            basis_history_path: None,
            basis_history_sha256: None,
            basis_claim_id: None,
            basis_revision_proof_root: integrity.revision_proof_root,
            claim_owner_inventory_root: Some(integrity.owner_inventory_root),
            claim_stream_rows_scanned: Some(integrity.claim_stream_rows_scanned),
            claim_event_basis_root: Some(integrity.event_claim_set_root),
            claim_navigation_manifest_root: Some(manifest_root),
            source_path: cohort.source_path().clone(),
            record_id: cohort.record_id().to_owned(),
            predecessor_cohort_root: cohort.predecessor_cohort_root(),
            dependency_root: cohort.dependency_root(),
            input_root: cohort.input_root(),
            output_root: cohort.output_root(),
            dependencies: cohort.dependencies().to_vec(),
        }
    }

    fn from_claim_refresh(
        cohort: &tos_compiler::source_navigation_claim_event::NavigationClaimDependencyRefreshCohortV1,
        integrity: &tos_compiler::source_navigation_claim_event::NavigationClaimDependencyRefreshIntegrityV1<'_>,
        manifest_root: Digest256,
    ) -> Self {
        Self {
            event_id: cohort.event_id().to_owned(),
            event_sequence: cohort.event_sequence(),
            source_event_id: cohort.source_event_id().to_owned(),
            source_event_version: cohort.source_event_version(),
            predecessor_source_revision: cohort.predecessor_source_revision(),
            source_revision: cohort.source_revision(),
            basis_kind: "claim-dependency-refresh".to_owned(),
            basis_dependency_path: None,
            basis_dependency_sha256: None,
            basis_dependency_causes_json: None,
            basis_dependency_causes_root: None,
            basis_history_path: None,
            basis_history_sha256: None,
            basis_claim_id: None,
            basis_revision_proof_root: None,
            claim_owner_inventory_root: Some(integrity.owner_inventory_root),
            claim_stream_rows_scanned: Some(cohort.claim_stream_rows_scanned()),
            claim_event_basis_root: Some(integrity.proof.binding_root()),
            claim_navigation_manifest_root: Some(manifest_root),
            source_path: cohort.source_path().clone(),
            record_id: cohort.record_id().to_owned(),
            predecessor_cohort_root: cohort.predecessor_cohort_root(),
            dependency_root: integrity.dependency_root,
            input_root: integrity.input_root,
            output_root: integrity.output_root,
            dependencies: cohort.dependencies().to_vec(),
        }
    }
}

#[derive(Clone)]
struct PersistedNavigationNode {
    node_id: String,
    visible: bool,
    row_json: Vec<u8>,
    sha256: Digest256,
}

#[derive(Clone)]
struct PersistedNavigationEdge {
    edge_id: String,
    from_id: String,
    to_id: String,
    visible: bool,
    row_json: Vec<u8>,
    sha256: Digest256,
}

fn replace_navigation_event_cohort_rows(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    owner: &NavigationEventOwnerV2,
    rows: &PersistedRows,
    sequence: u64,
    coverage_manifest_sha256: Digest256,
) -> DurableResult<()> {
    tx.execute(
        "UPDATE cmd2_projection_navigation_bootstrap_row
            SET active=false
          WHERE domain=$1 AND consumer_id=$2 AND owner_source_path=$3
            AND owner_record_id=$4",
        &[
            &domain,
            &consumer_id,
            &owner.source_path.as_str(),
            &owner.record_id,
        ],
    )?;
    tx.execute(
        "DELETE FROM cmd2_projection_navigation_cohort
          WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4",
        &[
            &domain,
            &consumer_id,
            &owner.source_path.as_str(),
            &owner.record_id,
        ],
    )?;
    tx.execute(
        "INSERT INTO cmd2_projection_navigation_cohort(
            domain,consumer_id,source_path,record_id,event_id,event_sequence,
            source_event_id,source_event_version,predecessor_source_revision,
            source_revision,basis_kind,basis_dependency_path,basis_dependency_sha256,
            basis_dependency_causes_json,basis_dependency_causes_root,
            basis_history_path,basis_history_sha256,basis_claim_id,basis_revision_proof_root,
            claim_owner_inventory_root,claim_stream_rows_scanned,claim_event_basis_root,
            claim_navigation_manifest_root,
            predecessor_cohort_root,dependency_root,input_root,output_root,
            dependency_count,node_count,edge_count,diagnostic_count,cohort_bytes,
            coverage_manifest_sha256)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31,$32,$33)",
        &[
            &domain,
            &consumer_id,
            &owner.source_path.as_str(),
            &owner.record_id,
            &owner.event_id,
            &as_i64(sequence)?,
            &owner.source_event_id,
            &as_i64(owner.source_event_version)?,
            &owner.predecessor_source_revision.to_hex(),
            &owner.source_revision.to_hex(),
            &owner.basis_kind,
            &owner.basis_dependency_path,
            &owner.basis_dependency_sha256.map(Digest256::to_hex),
            &owner.basis_dependency_causes_json,
            &owner.basis_dependency_causes_root.map(Digest256::to_hex),
            &owner.basis_history_path,
            &owner.basis_history_sha256.map(Digest256::to_hex),
            &owner.basis_claim_id,
            &owner.basis_revision_proof_root.map(Digest256::to_hex),
            &owner.claim_owner_inventory_root.map(Digest256::to_hex),
            &owner.claim_stream_rows_scanned.map(as_i64).transpose()?,
            &owner.claim_event_basis_root.map(Digest256::to_hex),
            &owner.claim_navigation_manifest_root.map(Digest256::to_hex),
            &owner.predecessor_cohort_root.map(Digest256::to_hex),
            &owner.dependency_root.to_hex(),
            &owner.input_root.to_hex(),
            &owner.output_root.to_hex(),
            &i32::try_from(rows.dependencies.len())
                .map_err(|_| DurableError::Refused("navigation dependency count"))?,
            &i32::try_from(rows.nodes.len())
                .map_err(|_| DurableError::Refused("navigation node count"))?,
            &i32::try_from(rows.edges.len())
                .map_err(|_| DurableError::Refused("navigation edge count"))?,
            &i32::try_from(rows.diagnostics.len())
                .map_err(|_| DurableError::Refused("navigation diagnostic count"))?,
            &as_i64(rows.cohort_bytes)?,
            &coverage_manifest_sha256.to_hex(),
        ],
    )?;
    for dependency in &rows.dependencies {
        let (state, sha256, size_bytes, absent_reason) = match &dependency.state {
            NavigationDependencyStateV1::Present { sha256, size_bytes } => (
                "present",
                Some(sha256.to_hex()),
                Some(as_i64(*size_bytes)?),
                None,
            ),
            NavigationDependencyStateV1::Absent { reason } => {
                ("absent", None, None, Some(reason.as_str()))
            }
        };
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_dependency(
                domain,consumer_id,source_path,record_id,dependency_path,
                dependency_role,observation_state,sha256,size_bytes,absent_reason)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &domain,
                &consumer_id,
                &owner.source_path.as_str(),
                &owner.record_id,
                &dependency.path.as_str(),
                &dependency_role(dependency.role),
                &state,
                &sha256,
                &size_bytes,
                &absent_reason,
            ],
        )?;
    }
    for node in &rows.nodes {
        let row_json = std::str::from_utf8(&node.row_json)
            .map_err(|_| DurableError::Invalid("navigation event node carrier UTF-8"))?;
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_node(
                domain,consumer_id,source_path,record_id,node_id,visible,row_sha256,row_json)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &domain,
                &consumer_id,
                &owner.source_path.as_str(),
                &owner.record_id,
                &node.node_id,
                &node.visible,
                &node.sha256.to_hex(),
                &row_json,
            ],
        )?;
    }
    for edge in &rows.edges {
        let row_json = std::str::from_utf8(&edge.row_json)
            .map_err(|_| DurableError::Invalid("navigation event edge carrier UTF-8"))?;
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_edge(
                domain,consumer_id,source_path,record_id,edge_id,from_id,to_id,
                visible,row_sha256,row_json)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &domain,
                &consumer_id,
                &owner.source_path.as_str(),
                &owner.record_id,
                &edge.edge_id,
                &edge.from_id,
                &edge.to_id,
                &edge.visible,
                &edge.sha256.to_hex(),
                &row_json,
            ],
        )?;
    }
    for (ordinal, row_json) in rows.diagnostics.iter().enumerate() {
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_diagnostic(
                domain,consumer_id,source_path,record_id,diagnostic_ordinal,row_json)
             VALUES($1,$2,$3,$4,$5,$6)",
            &[
                &domain,
                &consumer_id,
                &owner.source_path.as_str(),
                &owner.record_id,
                &i32::try_from(ordinal)
                    .map_err(|_| DurableError::Refused("navigation diagnostic ordinal"))?,
                &String::from_utf8_lossy(row_json).as_ref(),
            ],
        )?;
    }
    Ok(())
}

#[derive(Clone)]
struct EventWitness {
    event_id: String,
    event: ProjectionEvent,
    source_head_sequence: u64,
    members: Vec<EventMember>,
}

fn verify_navigation_reverse_dependency_closure<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_source_path: &str,
    target_record_id: &str,
    changed_paths: &[String],
) -> DurableResult<()> {
    if changed_paths.is_empty() || !changed_paths.iter().any(|path| path == target_source_path) {
        return Err(DurableError::Conflict(
            "navigation update does not cover its changed source owner",
        ));
    }
    let other_bootstrap_owner: bool = tx
        .query_one(
            "SELECT EXISTS(
            SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2
              AND owner_source_path=ANY($3)
              AND (owner_source_path<>$4 OR owner_record_id<>$5)
         )",
            &[
                &domain,
                &consumer_id,
                &changed_paths,
                &target_source_path,
                &target_record_id,
            ],
        )?
        .get(0);
    let other_bootstrap_dependency: bool = tx
        .query_one(
            "SELECT EXISTS(
            SELECT 1 FROM cmd2_projection_navigation_bootstrap_dependency
            WHERE domain=$1 AND consumer_id=$2
              AND dependency_path=ANY($3)
              AND (owner_source_path<>$4 OR owner_record_id<>$5)
         )",
            &[
                &domain,
                &consumer_id,
                &changed_paths,
                &target_source_path,
                &target_record_id,
            ],
        )?
        .get(0);
    let other_event_dependency: bool = tx
        .query_one(
            "SELECT EXISTS(
            SELECT 1 FROM cmd2_projection_navigation_dependency
            WHERE domain=$1 AND consumer_id=$2
              AND dependency_path=ANY($3)
              AND (source_path<>$4 OR record_id<>$5)
         )",
            &[
                &domain,
                &consumer_id,
                &changed_paths,
                &target_source_path,
                &target_record_id,
            ],
        )?
        .get(0);
    let unsupported_rights: bool = tx
        .query_one(
            "SELECT EXISTS(
            SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND owner_source_path=$3
              AND owner_record_id=$4 AND row_kind='right'
         )",
            &[
                &domain,
                &consumer_id,
                &target_source_path,
                &target_record_id,
            ],
        )?
        .get(0);
    if other_bootstrap_owner
        || other_bootstrap_dependency
        || other_event_dependency
        || unsupported_rights
    {
        return Err(DurableError::Conflict(
            "navigation event does not cover the complete reverse-dependency closure",
        ));
    }
    Ok(())
}

fn verify_navigation_reverse_dependency_closure_set<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_owners: &BTreeSet<(String, String)>,
    changed_paths: &[String],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    if target_owners.is_empty() || changed_paths.is_empty() {
        return Err(DurableError::Conflict(
            "navigation batch has no affected owner or changed source path",
        ));
    }
    let owner_limit = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation reverse-owner count"))?;
    let owners = tx.query(
        "SELECT owner_source_path,owner_record_id FROM (
           SELECT DISTINCT owner_source_path,owner_record_id
             FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND active
              AND owner_source_path=ANY($3)
           UNION
           SELECT DISTINCT b.owner_source_path,b.owner_record_id
             FROM cmd2_projection_navigation_bootstrap_dependency b
            WHERE b.domain=$1 AND b.consumer_id=$2 AND b.dependency_path=ANY($3)
              AND EXISTS(
                SELECT 1 FROM cmd2_projection_navigation_bootstrap_row r
                 WHERE r.domain=b.domain AND r.consumer_id=b.consumer_id AND r.active
                   AND r.owner_source_path=b.owner_source_path
                   AND r.owner_record_id=b.owner_record_id
              )
           UNION
           SELECT DISTINCT source_path,record_id
             FROM cmd2_projection_navigation_cohort
            WHERE domain=$1 AND consumer_id=$2 AND source_path=ANY($3)
           UNION
           SELECT DISTINCT source_path,record_id
             FROM cmd2_projection_navigation_dependency
            WHERE domain=$1 AND consumer_id=$2 AND dependency_path=ANY($3)
         ) affected_owners
         ORDER BY owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"
         LIMIT $4",
        &[&domain, &consumer_id, &changed_paths, &owner_limit],
    )?;
    if owners.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation reverse-owner count budget",
        ));
    }
    for owner in owners {
        let key = (
            owner.get::<_, String>("owner_source_path"),
            owner.get::<_, String>("owner_record_id"),
        );
        if !target_owners.contains(&key) {
            return Err(DurableError::Conflict(
                "navigation event does not cover the complete reverse-dependency closure",
            ));
        }
    }
    for (owner_path, owner_id) in target_owners {
        let unsupported_rights: bool = tx
            .query_one(
                "SELECT EXISTS(
                   SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
                    WHERE domain=$1 AND consumer_id=$2 AND active
                      AND owner_source_path=$3 AND owner_record_id=$4 AND row_kind='right'
                 )",
                &[&domain, &consumer_id, owner_path, owner_id],
            )?
            .get(0);
        if unsupported_rights {
            return Err(DurableError::Conflict(
                "navigation batch cannot replace an owner with unsupported rights rows",
            ));
        }
    }
    Ok(())
}

fn verify_navigation_batch_final_budget<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_owners: &BTreeSet<(String, String)>,
    replacements: &[PersistedRows],
    header_bytes: u64,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let (target_paths, target_ids): (Vec<_>, Vec<_>) = target_owners.iter().cloned().unzip();
    let base_usage = tx.query_one(
        "SELECT count(*) AS row_count,COALESCE(sum(octet_length(row_json)),0) AS row_bytes
           FROM cmd2_projection_navigation_bootstrap_row
          WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer_id],
    )?;
    let owner_usage = tx.query_one(
        "SELECT count(*) AS row_count,
                COALESCE(sum(octet_length(owner_source_path)+96),0) AS row_bytes
           FROM cmd2_projection_navigation_bootstrap_owner
          WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer_id],
    )?;
    let event_usage = tx.query_one(
        "SELECT count(*) AS row_count,COALESCE(sum(octet_length(row_json)),0) AS row_bytes
           FROM (
             SELECT row_json FROM cmd2_projection_navigation_node
              WHERE domain=$1 AND consumer_id=$2
             UNION ALL
             SELECT row_json FROM cmd2_projection_navigation_edge
              WHERE domain=$1 AND consumer_id=$2
             UNION ALL
             SELECT row_json FROM cmd2_projection_navigation_diagnostic
              WHERE domain=$1 AND consumer_id=$2
           ) event_rows",
        &[&domain, &consumer_id],
    )?;
    let target_event_usage = tx.query_one(
        "SELECT count(*) AS row_count,COALESCE(sum(octet_length(row_json)),0) AS row_bytes
           FROM (
             SELECT n.row_json FROM cmd2_projection_navigation_node n
              JOIN unnest($3::text[],$4::text[]) AS target(source_path,record_id)
                ON n.source_path=target.source_path AND n.record_id=target.record_id
              WHERE n.domain=$1 AND n.consumer_id=$2
             UNION ALL
             SELECT e.row_json FROM cmd2_projection_navigation_edge e
              JOIN unnest($3::text[],$4::text[]) AS target(source_path,record_id)
                ON e.source_path=target.source_path AND e.record_id=target.record_id
              WHERE e.domain=$1 AND e.consumer_id=$2
             UNION ALL
             SELECT d.row_json FROM cmd2_projection_navigation_diagnostic d
              JOIN unnest($3::text[],$4::text[]) AS target(source_path,record_id)
                ON d.source_path=target.source_path AND d.record_id=target.record_id
              WHERE d.domain=$1 AND d.consumer_id=$2
           ) target_rows",
        &[&domain, &consumer_id, &target_paths, &target_ids],
    )?;
    let dependency_usage = tx.query_one(
        "SELECT count(*) AS row_count,
                COALESCE(sum(octet_length(dependency_path)+octet_length(dependency_role)
                  +COALESCE(octet_length(sha256),0)+COALESCE(octet_length(absent_reason),0)
                  +64),0) AS row_bytes
           FROM cmd2_projection_navigation_dependency
          WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer_id],
    )?;
    let target_dependency_usage = tx.query_one(
        "SELECT count(*) AS row_count,
                COALESCE(sum(octet_length(d.dependency_path)+octet_length(d.dependency_role)
                  +COALESCE(octet_length(d.sha256),0)+COALESCE(octet_length(d.absent_reason),0)
                  +64),0) AS row_bytes
           FROM cmd2_projection_navigation_dependency d
           JOIN unnest($3::text[],$4::text[]) AS target(source_path,record_id)
             ON d.source_path=target.source_path AND d.record_id=target.record_id
          WHERE d.domain=$1 AND d.consumer_id=$2",
        &[&domain, &consumer_id, &target_paths, &target_ids],
    )?;
    let current_cohort_count: u64 = as_u64(
        tx.query_one(
            "SELECT count(*) AS row_count FROM cmd2_projection_navigation_cohort
              WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?
        .get("row_count"),
    )?;
    let target_cohort_count: u64 = as_u64(
        tx.query_one(
            "SELECT count(*) AS row_count FROM cmd2_projection_navigation_cohort c
             JOIN unnest($3::text[],$4::text[]) AS target(source_path,record_id)
               ON c.source_path=target.source_path AND c.record_id=target.record_id
             WHERE c.domain=$1 AND c.consumer_id=$2",
            &[&domain, &consumer_id, &target_paths, &target_ids],
        )?
        .get("row_count"),
    )?;

    let new_carrier_count = replacements.iter().try_fold(0u64, |total, rows| {
        total
            .checked_add(rows.nodes.len() as u64)
            .and_then(|value| value.checked_add(rows.edges.len() as u64))
            .and_then(|value| value.checked_add(rows.diagnostics.len() as u64))
            .ok_or(DurableError::Refused("navigation batch carrier count"))
    })?;
    let new_carrier_bytes = replacements.iter().try_fold(0u64, |total, rows| {
        let row_bytes = rows
            .nodes
            .iter()
            .map(|row| row.row_json.len() as u64)
            .chain(rows.edges.iter().map(|row| row.row_json.len() as u64))
            .chain(rows.diagnostics.iter().map(|row| row.len() as u64))
            .try_fold(0u64, |subtotal, bytes| {
                subtotal
                    .checked_add(bytes)
                    .ok_or(DurableError::Refused("navigation batch carrier bytes"))
            })?;
        total
            .checked_add(row_bytes)
            .ok_or(DurableError::Refused("navigation batch carrier bytes"))
    })?;
    let new_dependency_count = replacements.iter().try_fold(0u64, |total, rows| {
        total
            .checked_add(rows.dependencies.len() as u64)
            .ok_or(DurableError::Refused("navigation batch dependency count"))
    })?;
    let new_dependency_bytes = replacements.iter().try_fold(0u64, |total, rows| {
        let row_bytes = rows
            .dependencies
            .iter()
            .try_fold(0u64, |subtotal, dependency| {
                let state_bytes = match &dependency.state {
                    NavigationDependencyStateV1::Present { sha256, .. } => {
                        sha256.to_hex().len() as u64
                    }
                    NavigationDependencyStateV1::Absent { reason } => reason.len() as u64,
                };
                subtotal
                    .checked_add(dependency.path.as_str().len() as u64)
                    .and_then(|value| {
                        value.checked_add(dependency_role(dependency.role).len() as u64)
                    })
                    .and_then(|value| value.checked_add(state_bytes))
                    .and_then(|value| value.checked_add(64))
                    .ok_or(DurableError::Refused("navigation batch dependency bytes"))
            })?;
        total
            .checked_add(row_bytes)
            .ok_or(DurableError::Refused("navigation batch dependency bytes"))
    })?;
    let final_carrier_count = as_u64(event_usage.get("row_count"))?
        .checked_sub(as_u64(target_event_usage.get("row_count"))?)
        .and_then(|value| value.checked_add(new_carrier_count))
        .ok_or(DurableError::Corrupt(
            "navigation batch carrier count underflow",
        ))?;
    let final_dependency_count = as_u64(dependency_usage.get("row_count"))?
        .checked_sub(as_u64(target_dependency_usage.get("row_count"))?)
        .and_then(|value| value.checked_add(new_dependency_count))
        .ok_or(DurableError::Corrupt(
            "navigation batch dependency count underflow",
        ))?;
    let final_cohort_count = current_cohort_count
        .checked_sub(target_cohort_count)
        .and_then(|value| value.checked_add(replacements.len() as u64))
        .ok_or(DurableError::Corrupt(
            "navigation batch cohort count underflow",
        ))?;
    let final_rows = as_u64(base_usage.get("row_count"))?
        .checked_add(final_carrier_count)
        .and_then(|value| value.checked_add(final_dependency_count))
        .ok_or(DurableError::Refused("navigation batch total row count"))?;
    let final_bytes = header_bytes
        .checked_add(as_u64(owner_usage.get("row_bytes"))?)
        .and_then(|value| value.checked_add(as_u64(base_usage.get("row_bytes")).ok()?))
        .and_then(|value| {
            value.checked_add(
                as_u64(event_usage.get("row_bytes"))
                    .ok()?
                    .checked_sub(as_u64(target_event_usage.get("row_bytes")).ok()?)?
                    .checked_add(new_carrier_bytes)?,
            )
        })
        .and_then(|value| {
            value.checked_add(
                as_u64(dependency_usage.get("row_bytes"))
                    .ok()?
                    .checked_sub(as_u64(target_dependency_usage.get("row_bytes")).ok()?)?
                    .checked_add(new_dependency_bytes)?,
            )
        })
        .ok_or(DurableError::Refused("navigation batch total byte count"))?;
    if final_rows > limits.max_current_rows as u64
        || final_dependency_count > limits.max_dependencies as u64
        || final_cohort_count > limits.max_current_cohorts as u64
        || final_bytes > limits.max_total_bytes
    {
        return Err(DurableError::Refused(
            "navigation batch final current-index budget",
        ));
    }
    Ok(())
}

fn verify_navigation_endpoint_closure<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_source_path: &str,
    target_record_id: &str,
    replacement_nodes: &[PersistedNavigationNode],
) -> DurableResult<()> {
    let old_rows = tx.query(
        "SELECT row_id FROM cmd2_projection_navigation_bootstrap_row
         WHERE domain=$1 AND consumer_id=$2 AND row_kind='node' AND active
           AND owner_source_path=$3 AND owner_record_id=$4",
        &[
            &domain,
            &consumer_id,
            &target_source_path,
            &target_record_id,
        ],
    )?;
    let old_event_rows = tx.query(
        "SELECT node_id FROM cmd2_projection_navigation_node
         WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4",
        &[
            &domain,
            &consumer_id,
            &target_source_path,
            &target_record_id,
        ],
    )?;
    let mut old_node_ids = old_rows
        .into_iter()
        .map(|row| row.get::<_, String>("row_id"))
        .chain(
            old_event_rows
                .into_iter()
                .map(|row| row.get::<_, String>("node_id")),
        )
        .collect::<BTreeSet<_>>();
    let replacement_node_ids = replacement_nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    old_node_ids.retain(|node_id| !replacement_node_ids.contains(node_id));
    if old_node_ids.is_empty() {
        return Ok(());
    }
    let removed_node_ids = old_node_ids.into_iter().collect::<Vec<_>>();
    let bootstrap_edge_uses_removed_node: bool = tx
        .query_one(
            "SELECT EXISTS(
            SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND row_kind='edge' AND active
              AND (from_id=ANY($3) OR to_id=ANY($3))
              AND (owner_source_path<>$4 OR owner_record_id<>$5)
         )",
            &[
                &domain,
                &consumer_id,
                &removed_node_ids,
                &target_source_path,
                &target_record_id,
            ],
        )?
        .get(0);
    let event_edge_uses_removed_node: bool = tx
        .query_one(
            "SELECT EXISTS(
            SELECT 1 FROM cmd2_projection_navigation_edge
            WHERE domain=$1 AND consumer_id=$2
              AND (from_id=ANY($3) OR to_id=ANY($3))
              AND (source_path<>$4 OR record_id<>$5)
         )",
            &[
                &domain,
                &consumer_id,
                &removed_node_ids,
                &target_source_path,
                &target_record_id,
            ],
        )?
        .get(0);
    if bootstrap_edge_uses_removed_node || event_edge_uses_removed_node {
        return Err(DurableError::Conflict(
            "navigation update removes a node with an uncovered external edge",
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct NavigationEffectiveEdge {
    edge_id: String,
    from_id: String,
    to_id: String,
    visible: bool,
    sha256: Digest256,
    raw: Vec<u8>,
}

#[derive(Clone)]
struct NavigationEffectiveNode {
    node_id: String,
    visible: bool,
    sha256: Digest256,
    raw: Vec<u8>,
}

fn stored_navigation_edge(
    row: &postgres::Row,
    limits: DurableNavigationEventLimits,
) -> DurableResult<NavigationEffectiveEdge> {
    let edge_id = row
        .get::<_, Option<String>>("edge_id")
        .ok_or(DurableError::Corrupt("stored navigation edge ID absent"))?;
    let from_id = row
        .get::<_, Option<String>>("from_id")
        .ok_or(DurableError::Corrupt(
            "stored navigation edge source absent",
        ))?;
    let to_id = row
        .get::<_, Option<String>>("to_id")
        .ok_or(DurableError::Corrupt(
            "stored navigation edge target absent",
        ))?;
    let visible = row
        .get::<_, Option<bool>>("visible")
        .ok_or(DurableError::Corrupt(
            "stored navigation edge visibility absent",
        ))?;
    let sha256 = row
        .get::<_, Option<String>>("row_sha256")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Corrupt(
            "stored navigation edge digest absent",
        ))?;
    let raw = row
        .get::<_, Option<String>>("row_json")
        .ok_or(DurableError::Corrupt(
            "stored navigation edge carrier absent",
        ))?
        .into_bytes();
    if raw.len() > limits.max_row_bytes || Digest256::of_bytes(&raw) != sha256 {
        return Err(DurableError::Corrupt(
            "stored navigation edge digest differs",
        ));
    }
    let value = strict_json_value(&raw, limits.max_row_bytes)?;
    if value.get("edge_id").and_then(Value::as_str) != Some(edge_id.as_str())
        || value.get("from_id").and_then(Value::as_str) != Some(from_id.as_str())
        || value.get("to_id").and_then(Value::as_str) != Some(to_id.as_str())
    {
        return Err(DurableError::Corrupt(
            "stored navigation edge carrier metadata differs",
        ));
    }
    Ok(NavigationEffectiveEdge {
        edge_id,
        from_id,
        to_id,
        visible,
        sha256,
        raw,
    })
}

fn stored_navigation_node(
    row: &postgres::Row,
    limits: DurableNavigationEventLimits,
) -> DurableResult<NavigationEffectiveNode> {
    let node_id = row
        .get::<_, Option<String>>("node_id")
        .ok_or(DurableError::Corrupt("stored navigation node ID absent"))?;
    let visible = row
        .get::<_, Option<bool>>("visible")
        .ok_or(DurableError::Corrupt(
            "stored navigation node visibility absent",
        ))?;
    let sha256 = row
        .get::<_, Option<String>>("row_sha256")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Corrupt(
            "stored navigation node digest absent",
        ))?;
    let raw = row
        .get::<_, Option<String>>("row_json")
        .ok_or(DurableError::Corrupt(
            "stored navigation node carrier absent",
        ))?
        .into_bytes();
    if raw.len() > limits.max_row_bytes || Digest256::of_bytes(&raw) != sha256 {
        return Err(DurableError::Corrupt(
            "stored navigation node digest differs",
        ));
    }
    let value = strict_json_value(&raw, limits.max_row_bytes)?;
    if value.get("node_id").and_then(Value::as_str) != Some(node_id.as_str()) {
        return Err(DurableError::Corrupt(
            "stored navigation node carrier metadata differs",
        ));
    }
    Ok(NavigationEffectiveNode {
        node_id,
        visible,
        sha256,
        raw,
    })
}

fn same_effective_node(left: &NavigationEffectiveNode, right: &NavigationEffectiveNode) -> bool {
    left.visible == right.visible && left.sha256 == right.sha256 && left.raw == right.raw
}

fn same_effective_edge(left: &NavigationEffectiveEdge, right: &NavigationEffectiveEdge) -> bool {
    left.from_id == right.from_id
        && left.to_id == right.to_id
        && left.visible == right.visible
        && left.sha256 == right.sha256
        && left.raw == right.raw
}

fn prepare_navigation_adjacency_update_batch<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_owners: &BTreeSet<(String, String)>,
    bootstrap_root: Digest256,
    previous_adjacency_root: Digest256,
    event_sequence: u64,
    replacements: &[&PersistedRows],
    limits: DurableNavigationEventLimits,
) -> DurableResult<(
    BTreeMap<String, Option<NavigationAdjacencyCertificate>>,
    Digest256,
)> {
    let row_cap = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation batch graph row count"))?;
    let bootstrap_rows = tx.query(
        "SELECT row_kind,row_id AS node_id,row_id AS edge_id,visible,row_sha256,row_json,
                from_id,to_id,owner_source_path,owner_record_id
           FROM cmd2_projection_navigation_bootstrap_row
          WHERE domain=$1 AND consumer_id=$2 AND active AND row_kind IN ('node','edge')
          ORDER BY row_kind COLLATE \"C\",row_id COLLATE \"C\"
          LIMIT $3",
        &[&domain, &consumer_id, &row_cap],
    )?;
    let event_nodes = tx.query(
        "SELECT source_path AS owner_source_path,record_id AS owner_record_id,
                node_id,visible,row_sha256,row_json
           FROM cmd2_projection_navigation_node
          WHERE domain=$1 AND consumer_id=$2
          ORDER BY node_id COLLATE \"C\",source_path COLLATE \"C\",record_id COLLATE \"C\"
          LIMIT $3",
        &[&domain, &consumer_id, &row_cap],
    )?;
    let event_edges = tx.query(
        "SELECT source_path AS owner_source_path,record_id AS owner_record_id,
                edge_id,from_id,to_id,visible,row_sha256,row_json
           FROM cmd2_projection_navigation_edge
          WHERE domain=$1 AND consumer_id=$2
          ORDER BY edge_id COLLATE \"C\",source_path COLLATE \"C\",record_id COLLATE \"C\"
          LIMIT $3",
        &[&domain, &consumer_id, &row_cap],
    )?;
    if bootstrap_rows.len() > limits.max_current_rows
        || event_nodes.len() > limits.max_current_rows
        || event_edges.len() > limits.max_current_rows
    {
        return Err(DurableError::Refused("navigation batch graph row budget"));
    }
    let mut graph_rows = 0usize;
    let mut graph_bytes = 0u64;
    let mut charge = |bytes: usize| -> DurableResult<()> {
        graph_rows = graph_rows
            .checked_add(1)
            .ok_or(DurableError::Refused("navigation batch graph row count"))?;
        graph_bytes = graph_bytes
            .checked_add(bytes as u64)
            .ok_or(DurableError::Refused("navigation batch graph byte count"))?;
        if graph_rows > limits.max_current_rows || graph_bytes > limits.max_total_bytes {
            return Err(DurableError::Refused(
                "navigation batch graph row or byte budget",
            ));
        }
        Ok(())
    };
    let is_target =
        |path: &str, id: &str| target_owners.contains(&(path.to_owned(), id.to_owned()));
    let mut nodes = BTreeMap::<String, NavigationEffectiveNode>::new();
    let mut edges = BTreeMap::<String, NavigationEffectiveEdge>::new();
    let mut insert_node = |node: NavigationEffectiveNode| -> DurableResult<()> {
        charge(node.raw.len())?;
        if let Some(existing) = nodes.get(&node.node_id) {
            if !same_effective_node(existing, &node) {
                // Emit only fingerprints and fixed field names, never source
                // values, so a refused merge can be diagnosed from owner logs.
                let left = strict_json_value(&existing.raw, limits.max_row_bytes)?;
                let right = strict_json_value(&node.raw, limits.max_row_bytes)?;
                let fields = [
                    "/node_kind",
                    "/source_ref",
                    "/source_line",
                    "/source_sha256",
                    "/label",
                    "/identity_status",
                    "/display",
                    "/properties",
                    "/properties/source_record",
                    "/properties/source_claim",
                    "/properties/human_forms",
                    "/properties/record_version_view",
                ];
                let changed: Vec<_> = fields
                    .into_iter()
                    .filter(|field| left.pointer(field) != right.pointer(field))
                    .collect();
                eprintln!(
                    "navigation node conflict id_sha256={} left_sha256={} right_sha256={} visible_changed={} fields={changed:?}",
                    Digest256::of_bytes(node.node_id.as_bytes()).to_hex(),
                    existing.sha256.to_hex(),
                    node.sha256.to_hex(),
                    existing.visible != node.visible
                );
                return Err(DurableError::Conflict(
                    "navigation shared node contributions conflict",
                ));
            }
            return Ok(());
        }
        nodes.insert(node.node_id.clone(), node);
        Ok(())
    };
    for row in &bootstrap_rows {
        let kind: String = row.get("row_kind");
        let owner_path: String = row.get("owner_source_path");
        let owner_id: String = row.get("owner_record_id");
        if is_target(&owner_path, &owner_id) {
            continue;
        }
        if kind == "node" {
            insert_node(stored_navigation_node(row, limits)?)?;
        }
    }
    for row in &event_nodes {
        let owner_path: String = row.get("owner_source_path");
        let owner_id: String = row.get("owner_record_id");
        if is_target(&owner_path, &owner_id) {
            continue;
        }
        insert_node(stored_navigation_node(row, limits)?)?;
    }
    for rows in replacements {
        for node in &rows.nodes {
            insert_node(NavigationEffectiveNode {
                node_id: node.node_id.clone(),
                visible: node.visible,
                sha256: node.sha256,
                raw: node.row_json.clone(),
            })?;
        }
    }
    drop(insert_node);

    let mut insert_edge = |edge: NavigationEffectiveEdge| -> DurableResult<()> {
        charge(edge.raw.len())?;
        if let Some(existing) = edges.get(&edge.edge_id) {
            if !same_effective_edge(existing, &edge) {
                return Err(DurableError::Conflict(
                    "navigation shared edge contributions conflict",
                ));
            }
            return Ok(());
        }
        edges.insert(edge.edge_id.clone(), edge);
        Ok(())
    };
    for row in &bootstrap_rows {
        let kind: String = row.get("row_kind");
        if kind != "edge" {
            continue;
        }
        let owner_path: String = row.get("owner_source_path");
        let owner_id: String = row.get("owner_record_id");
        if is_target(&owner_path, &owner_id) {
            continue;
        }
        insert_edge(stored_navigation_edge(row, limits)?)?;
    }
    for row in &event_edges {
        let owner_path: String = row.get("owner_source_path");
        let owner_id: String = row.get("owner_record_id");
        if is_target(&owner_path, &owner_id) {
            continue;
        }
        insert_edge(stored_navigation_edge(row, limits)?)?;
    }
    for rows in replacements {
        for edge in &rows.edges {
            insert_edge(NavigationEffectiveEdge {
                edge_id: edge.edge_id.clone(),
                from_id: edge.from_id.clone(),
                to_id: edge.to_id.clone(),
                visible: edge.visible,
                sha256: edge.sha256,
                raw: edge.row_json.clone(),
            })?;
        }
    }
    drop(insert_edge);

    let mut outgoing = BTreeMap::<String, BTreeMap<String, Digest256>>::new();
    for (node_id, node) in &nodes {
        if node.visible {
            outgoing.entry(node_id.clone()).or_default();
        }
    }
    let mut adjacency_rows = 0usize;
    let mut adjacency_bytes = 0u64;
    for edge in edges.values() {
        let from_visible = nodes.get(&edge.from_id).map(|node| node.visible);
        let to_visible = nodes.get(&edge.to_id).map(|node| node.visible);
        if from_visible.is_none()
            || to_visible.is_none()
            || edge.visible != (from_visible == Some(true) && to_visible == Some(true))
        {
            return Err(DurableError::Conflict(
                "navigation effective edge endpoint visibility differs",
            ));
        }
        if !edge.visible {
            continue;
        }
        adjacency_rows = adjacency_rows.checked_add(1).ok_or(DurableError::Refused(
            "navigation batch adjacency row count",
        ))?;
        adjacency_bytes =
            adjacency_bytes
                .checked_add(edge.raw.len() as u64)
                .ok_or(DurableError::Refused(
                    "navigation batch adjacency byte count",
                ))?;
        if adjacency_rows > limits.max_adjacency_rows
            || adjacency_bytes > limits.max_adjacency_bytes
        {
            return Err(DurableError::Refused("navigation batch adjacency budget"));
        }
        let node_edges = outgoing
            .get_mut(&edge.from_id)
            .ok_or(DurableError::Conflict(
                "navigation visible edge source is not visible",
            ))?;
        node_edges.insert(edge.edge_id.clone(), edge.sha256);
    }
    if outgoing.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation batch adjacency node budget",
        ));
    }
    let certificate_cap = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation certificate count"))?;
    let stored_certificates = tx.query(
        "SELECT node_id FROM cmd2_projection_navigation_adjacency_certificate
          WHERE domain=$1 AND consumer_id=$2 ORDER BY node_id COLLATE \"C\" LIMIT $3",
        &[&domain, &consumer_id, &certificate_cap],
    )?;
    if stored_certificates.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation stored certificate count budget",
        ));
    }
    let mut affected = outgoing.keys().cloned().collect::<BTreeSet<_>>();
    affected.extend(
        stored_certificates
            .into_iter()
            .map(|row| row.get::<_, String>("node_id")),
    );
    if affected.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation batch certificate update count budget",
        ));
    }
    let mut updates = BTreeMap::new();
    for node_id in affected {
        let certificate = outgoing.get(&node_id).map(|edges| {
            make_navigation_adjacency_certificate(bootstrap_root, node_id.clone(), edges)
        });
        updates.insert(node_id, certificate);
    }
    let root = navigation_adjacency_root_next(previous_adjacency_root, event_sequence, &updates);
    Ok((updates, root))
}

fn navigation_external_ids_exist<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_source_path: &str,
    target_record_id: &str,
    ids: &[String],
    row_kind: DurableNavigationBootstrapRowKindV1,
) -> DurableResult<bool> {
    if ids.is_empty() {
        return Ok(false);
    }
    let ids = ids.to_vec();
    let exists = match row_kind {
        DurableNavigationBootstrapRowKindV1::Node => tx.query_one(
            "SELECT EXISTS(
               SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
                WHERE domain=$1 AND consumer_id=$2 AND row_kind='node' AND active
                  AND row_id=ANY($5)
                  AND (owner_source_path<>$3 OR owner_record_id<>$4)
               UNION ALL
               SELECT 1 FROM cmd2_projection_navigation_node
                WHERE domain=$1 AND consumer_id=$2 AND node_id=ANY($5)
                  AND (source_path<>$3 OR record_id<>$4)
             )",
            &[
                &domain,
                &consumer_id,
                &target_source_path,
                &target_record_id,
                &ids,
            ],
        )?,
        DurableNavigationBootstrapRowKindV1::Edge => tx.query_one(
            "SELECT EXISTS(
               SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
                WHERE domain=$1 AND consumer_id=$2 AND row_kind='edge' AND active
                  AND row_id=ANY($5)
                  AND (owner_source_path<>$3 OR owner_record_id<>$4)
               UNION ALL
               SELECT 1 FROM cmd2_projection_navigation_edge
                WHERE domain=$1 AND consumer_id=$2 AND edge_id=ANY($5)
                  AND (source_path<>$3 OR record_id<>$4)
             )",
            &[
                &domain,
                &consumer_id,
                &target_source_path,
                &target_record_id,
                &ids,
            ],
        )?,
        _ => return Err(DurableError::Invalid("navigation effective ID kind")),
    };
    Ok(exists.get(0))
}

fn navigation_external_incident_edges_exist<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_source_path: &str,
    target_record_id: &str,
    node_ids: &[String],
) -> DurableResult<bool> {
    if node_ids.is_empty() {
        return Ok(false);
    }
    let node_ids = node_ids.to_vec();
    let exists = tx.query_one(
        "SELECT EXISTS(
           SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND row_kind='edge' AND active
              AND (from_id=ANY($5) OR to_id=ANY($5))
              AND (owner_source_path<>$3 OR owner_record_id<>$4)
           UNION ALL
           SELECT 1 FROM cmd2_projection_navigation_edge
            WHERE domain=$1 AND consumer_id=$2
              AND (from_id=ANY($5) OR to_id=ANY($5))
              AND (source_path<>$3 OR record_id<>$4)
         )",
        &[
            &domain,
            &consumer_id,
            &target_source_path,
            &target_record_id,
            &node_ids,
        ],
    )?;
    Ok(exists.get(0))
}

fn prepare_navigation_adjacency_update<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_source_path: &str,
    target_record_id: &str,
    bootstrap_root: Digest256,
    previous_adjacency_root: Digest256,
    event_sequence: u64,
    replacement: &PersistedRows,
    limits: DurableNavigationEventLimits,
) -> DurableResult<(
    BTreeMap<String, Option<NavigationAdjacencyCertificate>>,
    Digest256,
)> {
    let old_row_cap = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation affected owner row count"))?;
    let old_nodes = tx.query(
        "SELECT node_id,visible FROM (
           SELECT row_id AS node_id,visible
             FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND row_kind='node' AND active
              AND owner_source_path=$3 AND owner_record_id=$4
           UNION ALL
           SELECT node_id,visible FROM cmd2_projection_navigation_node
            WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4
         ) owner_nodes ORDER BY node_id COLLATE \"C\" LIMIT $5",
        &[
            &domain,
            &consumer_id,
            &target_source_path,
            &target_record_id,
            &old_row_cap,
        ],
    )?;
    if old_nodes.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation affected owner node count budget",
        ));
    }
    let mut old_node_visibility = BTreeMap::<String, bool>::new();
    for row in old_nodes {
        let node_id: String = row.get("node_id");
        let visible = row
            .get::<_, Option<bool>>("visible")
            .ok_or(DurableError::Corrupt(
                "stored navigation affected-node visibility absent",
            ))?;
        if old_node_visibility.insert(node_id, visible).is_some() {
            return Err(DurableError::Conflict(
                "navigation target has duplicate effective node IDs",
            ));
        }
    }
    let mut replacement_node_visibility = BTreeMap::<String, bool>::new();
    for node in &replacement.nodes {
        if replacement_node_visibility
            .insert(node.node_id.clone(), node.visible)
            .is_some()
        {
            return Err(DurableError::Conflict(
                "navigation replacement has duplicate effective node IDs",
            ));
        }
    }

    let old_edges = tx.query(
        "SELECT edge_id,from_id,to_id,visible,row_sha256,row_json FROM (
           SELECT row_id AS edge_id,from_id,to_id,visible,row_sha256,row_json
             FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND row_kind='edge' AND active
              AND owner_source_path=$3 AND owner_record_id=$4
           UNION ALL
           SELECT edge_id,from_id,to_id,visible,row_sha256,row_json
             FROM cmd2_projection_navigation_edge
            WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4
         ) owner_edges ORDER BY edge_id COLLATE \"C\" LIMIT $5",
        &[
            &domain,
            &consumer_id,
            &target_source_path,
            &target_record_id,
            &old_row_cap,
        ],
    )?;
    if old_edges.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation affected owner edge count budget",
        ));
    }
    let mut adjacency_rows = 0usize;
    let mut adjacency_bytes = 0u64;
    let mut charge = |row_bytes: usize| -> DurableResult<()> {
        adjacency_rows = adjacency_rows
            .checked_add(1)
            .ok_or(DurableError::Refused("navigation adjacency row count"))?;
        adjacency_bytes = adjacency_bytes
            .checked_add(row_bytes as u64)
            .ok_or(DurableError::Refused("navigation adjacency byte count"))?;
        if adjacency_rows > limits.max_adjacency_rows
            || adjacency_bytes > limits.max_adjacency_bytes
        {
            return Err(DurableError::Refused(
                "navigation adjacency certificate budget",
            ));
        }
        Ok(())
    };
    let mut affected_nodes = BTreeSet::<String>::new();
    affected_nodes.extend(old_node_visibility.keys().cloned());
    affected_nodes.extend(replacement_node_visibility.keys().cloned());
    let mut old_edge_ids = BTreeSet::new();
    for row in &old_edges {
        let edge = stored_navigation_edge(row, limits)?;
        if !old_edge_ids.insert(edge.edge_id.clone()) {
            return Err(DurableError::Conflict(
                "navigation target has duplicate effective edge IDs",
            ));
        }
        affected_nodes.insert(edge.from_id.clone());
        charge(edge.raw.len())?;
    }
    let mut replacement_edge_ids = BTreeSet::new();
    for edge in &replacement.edges {
        if !replacement_edge_ids.insert(edge.edge_id.clone()) {
            return Err(DurableError::Conflict(
                "navigation replacement has duplicate effective edge IDs",
            ));
        }
        affected_nodes.insert(edge.from_id.clone());
        charge(edge.row_json.len())?;
    }

    let target_node_ids = old_node_visibility
        .keys()
        .chain(replacement_node_visibility.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if navigation_external_ids_exist(
        tx,
        domain,
        consumer_id,
        target_source_path,
        target_record_id,
        &target_node_ids,
        DurableNavigationBootstrapRowKindV1::Node,
    )? {
        return Err(DurableError::Conflict(
            "navigation update collides with an effective node ID",
        ));
    }
    let changed_visibility = old_node_visibility
        .keys()
        .chain(replacement_node_visibility.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|node_id| {
            old_node_visibility.get(node_id) != replacement_node_visibility.get(node_id)
        })
        .collect::<Vec<_>>();
    if navigation_external_incident_edges_exist(
        tx,
        domain,
        consumer_id,
        target_source_path,
        target_record_id,
        &changed_visibility,
    )? {
        return Err(DurableError::Conflict(
            "navigation visibility change has an uncovered external edge",
        ));
    }
    if navigation_external_ids_exist(
        tx,
        domain,
        consumer_id,
        target_source_path,
        target_record_id,
        &old_edge_ids
            .iter()
            .chain(replacement_edge_ids.iter())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        DurableNavigationBootstrapRowKindV1::Edge,
    )? {
        return Err(DurableError::Conflict(
            "navigation update collides with an effective edge ID",
        ));
    }

    if affected_nodes.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation affected-node count budget",
        ));
    }
    let affected_ids = affected_nodes.iter().cloned().collect::<Vec<_>>();
    let affected_id_bytes = affected_ids.iter().try_fold(0u64, |total, node_id| {
        total
            .checked_add(node_id.len() as u64)
            .ok_or(DurableError::Refused(
                "navigation affected-node key byte budget",
            ))
    })?;
    if affected_id_bytes > limits.max_total_bytes {
        return Err(DurableError::Refused(
            "navigation affected-node key byte budget",
        ));
    }
    let edge_row_cap = i64::try_from(limits.max_adjacency_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation adjacency edge count"))?;
    let external_edges = if affected_ids.is_empty() {
        Vec::new()
    } else {
        tx.query(
            "SELECT edge_id,from_id,to_id,visible,row_sha256,row_json FROM (
               SELECT row_id AS edge_id,from_id,to_id,visible,row_sha256,row_json
                 FROM cmd2_projection_navigation_bootstrap_row
                WHERE domain=$1 AND consumer_id=$2 AND row_kind='edge' AND active AND visible
                  AND from_id=ANY($5)
                  AND (owner_source_path<>$3 OR owner_record_id<>$4)
               UNION ALL
               SELECT edge_id,from_id,to_id,visible,row_sha256,row_json
                 FROM cmd2_projection_navigation_edge
                WHERE domain=$1 AND consumer_id=$2 AND visible AND from_id=ANY($5)
                  AND (source_path<>$3 OR record_id<>$4)
             ) outgoing ORDER BY edge_id COLLATE \"C\" LIMIT $6",
            &[
                &domain,
                &consumer_id,
                &target_source_path,
                &target_record_id,
                &affected_ids,
                &edge_row_cap,
            ],
        )?
    };
    if external_edges.len() > limits.max_adjacency_rows {
        return Err(DurableError::Refused(
            "navigation outgoing-edge certificate row budget",
        ));
    }
    let mut external_edge_rows = BTreeMap::<String, NavigationEffectiveEdge>::new();
    for row in &external_edges {
        let edge = stored_navigation_edge(row, limits)?;
        if !edge.visible {
            return Err(DurableError::Corrupt(
                "stored navigation visible-edge seek returned a hidden edge",
            ));
        }
        charge(edge.raw.len())?;
        if !affected_nodes.contains(&edge.from_id)
            || external_edge_rows
                .insert(edge.edge_id.clone(), edge)
                .is_some()
        {
            return Err(DurableError::Conflict(
                "navigation effective outgoing edge identity is ambiguous",
            ));
        }
    }
    let mut endpoint_ids = affected_nodes.clone();
    endpoint_ids.extend(external_edge_rows.values().map(|edge| edge.to_id.clone()));
    endpoint_ids.extend(replacement.edges.iter().map(|edge| edge.to_id.clone()));
    if endpoint_ids.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation affected endpoint count budget",
        ));
    }
    let endpoint_ids = endpoint_ids.into_iter().collect::<Vec<_>>();
    let node_row_cap = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation endpoint node count"))?;
    let external_nodes = if endpoint_ids.is_empty() {
        Vec::new()
    } else {
        tx.query(
            "SELECT node_id,visible FROM (
               SELECT row_id AS node_id,visible
                 FROM cmd2_projection_navigation_bootstrap_row
                WHERE domain=$1 AND consumer_id=$2 AND row_kind='node' AND active
                  AND row_id=ANY($5)
                  AND (owner_source_path<>$3 OR owner_record_id<>$4)
               UNION ALL
               SELECT node_id,visible FROM cmd2_projection_navigation_node
                WHERE domain=$1 AND consumer_id=$2 AND node_id=ANY($5)
                  AND (source_path<>$3 OR record_id<>$4)
             ) endpoint_nodes ORDER BY node_id COLLATE \"C\" LIMIT $6",
            &[
                &domain,
                &consumer_id,
                &target_source_path,
                &target_record_id,
                &endpoint_ids,
                &node_row_cap,
            ],
        )?
    };
    if external_nodes.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation endpoint node count budget",
        ));
    }
    let mut current_nodes = BTreeMap::<String, bool>::new();
    for row in external_nodes {
        let node_id: String = row.get("node_id");
        let visible = row
            .get::<_, Option<bool>>("visible")
            .ok_or(DurableError::Corrupt(
                "stored navigation endpoint visibility absent",
            ))?;
        if current_nodes.insert(node_id, visible).is_some() {
            return Err(DurableError::Conflict(
                "navigation effective node ID is ambiguous",
            ));
        }
    }
    for (node_id, visible) in &replacement_node_visibility {
        if current_nodes.insert(node_id.clone(), *visible).is_some() {
            return Err(DurableError::Conflict(
                "navigation replacement node ID is duplicated",
            ));
        }
    }
    let mut outgoing = BTreeMap::<String, BTreeMap<String, Digest256>>::new();
    for node_id in &affected_nodes {
        if current_nodes.get(node_id) == Some(&true) {
            outgoing.entry(node_id.clone()).or_default();
        }
    }
    for edge in external_edge_rows.values() {
        if current_nodes.get(&edge.from_id) != Some(&true)
            || current_nodes.get(&edge.to_id) != Some(&true)
            || outgoing
                .entry(edge.from_id.clone())
                .or_default()
                .insert(edge.edge_id.clone(), edge.sha256)
                .is_some()
        {
            return Err(DurableError::Conflict(
                "navigation visible outgoing edge endpoint or ID differs",
            ));
        }
    }
    for edge in &replacement.edges {
        if !edge.visible {
            continue;
        }
        if current_nodes.get(&edge.from_id) != Some(&true)
            || current_nodes.get(&edge.to_id) != Some(&true)
            || outgoing
                .entry(edge.from_id.clone())
                .or_default()
                .insert(edge.edge_id.clone(), edge.sha256)
                .is_some()
        {
            return Err(DurableError::Conflict(
                "navigation replacement outgoing edge endpoint or ID differs",
            ));
        }
    }
    let mut updates = BTreeMap::new();
    for node_id in affected_nodes {
        let certificate = if current_nodes.get(&node_id) == Some(&true) {
            let outgoing_edges = outgoing.get(&node_id).cloned().unwrap_or_default();
            Some(make_navigation_adjacency_certificate(
                bootstrap_root,
                node_id.clone(),
                &outgoing_edges,
            ))
        } else {
            None
        };
        updates.insert(node_id, certificate);
    }
    let root = navigation_adjacency_root_next(previous_adjacency_root, event_sequence, &updates);
    Ok((updates, root))
}

fn read_navigation_event_range_owner_manifest_in_tx(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    event_sequence: u64,
    event_id: &str,
    source_path: &str,
    record_id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<NavigationEventManifestV2> {
    let row = tx
        .query_opt(
            "SELECT cursor_before,through_seq,held_source_head_seq,terminal_event_id,range_root,range_manifest,
                    source_revision,input_root,dependency_root,output_root,
                    terminal_cohort_count,terminal_cohort_root,terminal_cohort_manifest
               FROM cmd2_projection_navigation_coverage_range
              WHERE domain=$1 AND consumer_id=$2 AND through_seq=$3 AND terminal_event_id=$4",
            &[&domain, &consumer_id, &as_i64(event_sequence)?, &event_id],
        )?
        .ok_or(DurableError::Conflict(
            "stored navigation range owner has no terminal range manifest",
        ))?;
    let cursor_before = as_u64(row.get("cursor_before"))?;
    let through_sequence = as_u64(row.get("through_seq"))?;
    let held_source_head_sequence = as_u64(row.get("held_source_head_seq"))?;
    let range_root = parse_digest(row.get("range_root"))?;
    let range_manifest = row.get::<_, Vec<u8>>("range_manifest");
    let terminal_cohort_count = u32::try_from(row.get::<_, i32>("terminal_cohort_count"))
        .map_err(|_| DurableError::Corrupt("navigation range cohort count is negative"))?;
    let terminal_cohort_root = parse_digest(row.get("terminal_cohort_root"))?;
    let terminal_cohort_manifest = row.get::<_, Vec<u8>>("terminal_cohort_manifest");
    let manifest = durable_navigation_range::verify_navigation_event_range_terminal_manifest(
        &terminal_cohort_manifest,
        terminal_cohort_root,
        terminal_cohort_count,
        cursor_before,
        through_sequence,
        held_source_head_sequence,
        event_id,
        range_root,
        &range_manifest,
        parse_digest(row.get("input_root"))?,
        parse_digest(row.get("dependency_root"))?,
        parse_digest(row.get("output_root"))?,
        limits.max_total_bytes as usize,
    )?;
    if through_sequence != event_sequence {
        return Err(DurableError::Conflict(
            "stored navigation range owner is not at the terminal sequence",
        ));
    }
    if !manifest
        .cohorts
        .contains_key(&(source_path.to_owned(), record_id.to_owned()))
    {
        return Err(DurableError::Conflict(
            "stored navigation range owner is absent from terminal manifest",
        ));
    }
    Ok(manifest)
}

fn read_navigation_currentness_guard_in_tx(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationCurrentnessV1> {
    read_navigation_currentness_guard_in_tx_mode(tx, domain, consumer_id, profile, limits, false)
}

/// Recheck the retained proof at the current coverage head independently of
/// the verified prefix. A prefix commits its root, but a zero-owner terminal
/// range has no current cohort row to force the current-index reader to load
/// the range manifest.
fn navigation_terminal_coverage_is_current_in_tx(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    bootstrap_sequence: u64,
    coverage_sequence: u64,
    head: u64,
    expected_coverage_root: Option<Digest256>,
    expected_terminal_event_id: Option<&str>,
    expected_source_revision: Option<Digest256>,
    expected_input_root: Option<Digest256>,
    expected_output_root: Option<Digest256>,
    limits: DurableNavigationEventLimits,
) -> DurableResult<bool> {
    if coverage_sequence == bootstrap_sequence {
        let event = tx.query_opt(
            "SELECT 1 FROM cmd2_projection_navigation_coverage_event
              WHERE domain=$1 AND consumer_id=$2 AND event_sequence=$3",
            &[&domain, &consumer_id, &as_i64(coverage_sequence)?],
        )?;
        let range = tx.query_opt(
            "SELECT 1 FROM cmd2_projection_navigation_coverage_range
              WHERE domain=$1 AND consumer_id=$2 AND through_seq=$3",
            &[&domain, &consumer_id, &as_i64(coverage_sequence)?],
        )?;
        return Ok(event.is_none() && range.is_none());
    }
    let (
        Some(expected_coverage_root),
        Some(expected_terminal_event_id),
        Some(expected_source_revision),
        Some(expected_input_root),
        Some(expected_output_root),
    ) = (
        expected_coverage_root,
        expected_terminal_event_id,
        expected_source_revision,
        expected_input_root,
        expected_output_root,
    )
    else {
        return Ok(false);
    };
    let event = tx.query_opt(
        "SELECT event_id,source_path,record_id,source_revision,input_root,
                dependency_root,output_root,previous_coverage_root,coverage_root,
                cohort_count,cohort_set_root,cohort_manifest
           FROM cmd2_projection_navigation_coverage_event
          WHERE domain=$1 AND consumer_id=$2 AND event_sequence=$3",
        &[&domain, &consumer_id, &as_i64(coverage_sequence)?],
    )?;
    let range = tx.query_opt(
        "SELECT cursor_before,through_seq,held_source_head_seq,terminal_event_id,
                range_root,range_manifest,source_revision,input_root,dependency_root,
                output_root,previous_coverage_root,coverage_root,terminal_cohort_count,
                terminal_cohort_root,terminal_cohort_manifest
           FROM cmd2_projection_navigation_coverage_range
          WHERE domain=$1 AND consumer_id=$2 AND through_seq=$3",
        &[&domain, &consumer_id, &as_i64(coverage_sequence)?],
    )?;
    if event.is_some() == range.is_some() {
        return Ok(false);
    }
    if let Some(row) = event {
        let event_id: String = row.get("event_id");
        let source_path: String = row.get("source_path");
        let record_id: String = row.get("record_id");
        let source_revision = parse_digest(row.get("source_revision"))?;
        let input_root = parse_digest(row.get("input_root"))?;
        let dependency_root = parse_digest(row.get("dependency_root"))?;
        let output_root = parse_digest(row.get("output_root"))?;
        let previous_root = parse_digest(row.get("previous_coverage_root"))?;
        let stored_root = parse_digest(row.get("coverage_root"))?;
        let cohort_count = u32::try_from(row.get::<_, i32>("cohort_count"))
            .map_err(|_| DurableError::Corrupt("navigation event cohort count is negative"))?;
        let cohort_set_root = row
            .get::<_, Option<String>>("cohort_set_root")
            .map(parse_digest)
            .transpose()?;
        let cohort_manifest = row.get::<_, Option<Vec<u8>>>("cohort_manifest");
        let (Some(cohort_set_root), Some(cohort_manifest)) = (cohort_set_root, cohort_manifest)
        else {
            return Ok(false);
        };
        if cohort_manifest.len() as u64 > limits.max_total_bytes
            || verify_navigation_event_cohort_manifest_single(
                &cohort_manifest,
                cohort_set_root,
                cohort_count,
                &event_id,
                coverage_sequence,
                &source_path,
                &record_id,
                source_revision,
                input_root,
                dependency_root,
                output_root,
                limits.max_row_bytes,
            )
            .is_err()
        {
            return Ok(false);
        }
        let calculated = navigation_coverage_root_next(
            previous_root,
            coverage_sequence,
            &event_id,
            &source_path,
            &record_id,
            source_revision,
            input_root,
            dependency_root,
            output_root,
            cohort_set_root,
        );
        return Ok(event_id == expected_terminal_event_id
            && source_revision == expected_source_revision
            && input_root == expected_input_root
            && output_root == expected_output_root
            && stored_root == expected_coverage_root
            && calculated == expected_coverage_root);
    }
    let row = range.ok_or(DurableError::Corrupt(
        "navigation terminal coverage range disappeared",
    ))?;
    let cursor_before = as_u64(row.get("cursor_before"))?;
    let through_sequence = as_u64(row.get("through_seq"))?;
    let held_source_head_sequence = as_u64(row.get("held_source_head_seq"))?;
    let event_id: String = row.get("terminal_event_id");
    let range_root = parse_digest(row.get("range_root"))?;
    let range_manifest = row.get::<_, Vec<u8>>("range_manifest");
    let source_revision = parse_digest(row.get("source_revision"))?;
    let input_root = parse_digest(row.get("input_root"))?;
    let dependency_root = parse_digest(row.get("dependency_root"))?;
    let output_root = parse_digest(row.get("output_root"))?;
    let previous_root = parse_digest(row.get("previous_coverage_root"))?;
    let stored_root = parse_digest(row.get("coverage_root"))?;
    let cohort_count = u32::try_from(row.get::<_, i32>("terminal_cohort_count"))
        .map_err(|_| DurableError::Corrupt("navigation range cohort count is negative"))?;
    let terminal_cohort_root = parse_digest(row.get("terminal_cohort_root"))?;
    let terminal_manifest = row.get::<_, Vec<u8>>("terminal_cohort_manifest");
    let manifest_bytes = (range_manifest.len() as u64)
        .checked_add(terminal_manifest.len() as u64)
        .ok_or(DurableError::Refused(
            "navigation terminal manifest byte budget",
        ))?;
    if manifest_bytes > limits.max_total_bytes
        || cursor_before >= through_sequence
        || through_sequence != coverage_sequence
        || held_source_head_sequence < through_sequence
        || held_source_head_sequence > head
        || durable_navigation_range::verify_navigation_event_range_terminal_manifest(
            &terminal_manifest,
            terminal_cohort_root,
            cohort_count,
            cursor_before,
            through_sequence,
            held_source_head_sequence,
            &event_id,
            range_root,
            &range_manifest,
            input_root,
            dependency_root,
            output_root,
            limits.max_total_bytes as usize,
        )
        .is_err()
    {
        return Ok(false);
    }
    let calculated = durable_navigation_range::navigation_range_coverage_root_next(
        previous_root,
        cursor_before,
        through_sequence,
        held_source_head_sequence,
        &event_id,
        range_root,
        terminal_cohort_root,
        source_revision,
        dependency_root,
        output_root,
    );
    Ok(event_id == expected_terminal_event_id
        && source_revision == expected_source_revision
        && input_root == expected_input_root
        && output_root == expected_output_root
        && stored_root == expected_coverage_root
        && calculated == expected_coverage_root)
}

fn read_navigation_currentness_guard_in_tx_mode(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
    published_generation: bool,
) -> DurableResult<DurableNavigationCurrentnessV1> {
    let snapshot = read_domain_snapshot(tx, domain)?;
    snapshot.require_live_projection(profile)?;
    let head = snapshot.head_seq()?;
    if !published_generation && !snapshot.source_complete {
        return Ok(navigation_currentness_not_ready(
            head,
            None,
            DurableNavigationCurrentIndexNotReadyReasonV1::SourceIncomplete,
        ));
    }
    let current = tx.query_one(
        "SELECT source_membership_digest,source_membership_count,source_generation,
                source_complete,selected_generation_digest,source_projection_digest
         FROM cmd2_domain WHERE domain=$1",
        &[&domain],
    )?;
    if !published_generation && !current.get::<_, bool>("source_complete") {
        return Ok(navigation_currentness_not_ready(
            head,
            None,
            DurableNavigationCurrentIndexNotReadyReasonV1::SourceIncomplete,
        ));
    }
    let consumer = tx.query_opt(
        "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,
                source_revision,last_event_id,last_input_root,last_output_root,
                coverage_seq,coverage_root,adjacency_seq,adjacency_root,adjacency_ready,
                verified_prefix_version,verified_prefix_seq,verified_prefix_coverage_root,
                verified_prefix_cohort_root,verified_prefix_digest
         FROM cmd2_projection_navigation_consumer WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer_id],
    )?;
    let Some(consumer) = consumer else {
        return Ok(navigation_currentness_not_ready(
            head,
            None,
            DurableNavigationCurrentIndexNotReadyReasonV1::NoConsumer,
        ));
    };
    let cursor = as_u64(consumer.get("cursor_seq"))?;
    let profile_version = profile.version.to_string();
    if consumer.get::<_, String>("profile_id") != profile.id
        || consumer.get::<_, String>("profile_version") != profile_version
        || consumer.get::<_, String>("source_contract_digest") != profile.source_contract.to_hex()
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::ProfileMismatch,
        ));
    }
    if (!published_generation && cursor != head) || (published_generation && cursor > head) {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            if published_generation {
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch
            } else {
                DurableNavigationCurrentIndexNotReadyReasonV1::CursorBehindHead
            },
        ));
    }
    let bootstrap = tx.query_opt(
        "SELECT through_seq,source_revision,bootstrap_root,owner_binding_json,
                header_raw,header_sha256,node_count,edge_count,right_count,
                projected_right_count,diagnostic_count
         FROM cmd2_projection_navigation_bootstrap
         WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer_id],
    )?;
    let Some(bootstrap) = bootstrap else {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageAbsent,
        ));
    };
    let bootstrap_sequence = as_u64(bootstrap.get("through_seq"))?;
    let bootstrap_root = parse_digest(bootstrap.get("bootstrap_root"))?;
    let bootstrap_source_revision = parse_digest(bootstrap.get("source_revision"))?;
    let coverage_sequence = as_u64(consumer.get("coverage_seq"))?;
    let coverage_root = consumer
        .get::<_, Option<String>>("coverage_root")
        .map(parse_digest)
        .transpose()?;
    let verified_prefix_sequence = consumer
        .get::<_, Option<i64>>("verified_prefix_seq")
        .map(as_u64)
        .transpose()?;
    let verified_prefix_coverage_root = consumer
        .get::<_, Option<String>>("verified_prefix_coverage_root")
        .map(parse_digest)
        .transpose()?;
    let adjacency_sequence = as_u64(consumer.get("adjacency_seq"))?;
    let adjacency_root = consumer
        .get::<_, Option<String>>("adjacency_root")
        .map(parse_digest)
        .transpose()?;
    if coverage_sequence != cursor
        || coverage_root.is_none()
        || bootstrap_sequence > coverage_sequence
        || consumer.get::<_, Option<i16>>("verified_prefix_version")
            != Some(VERIFIED_PREFIX_FORMAT_VERSION)
        || verified_prefix_sequence != Some(cursor)
        || verified_prefix_coverage_root != coverage_root
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageAbsent,
        ));
    }
    let coverage_span = coverage_sequence - cursor;
    if coverage_span > limits.max_current_cohorts as u64 {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded,
        ));
    }
    let coverage_row_cap = i64::try_from(coverage_span.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation coverage event count"))?;
    let range_rows = tx.query(
        "SELECT cursor_before,through_seq,held_source_head_seq,terminal_event_id,range_root,range_manifest,
                source_revision,input_root,dependency_root,output_root,
                previous_coverage_root,coverage_root,terminal_cohort_count,
                terminal_cohort_root,terminal_cohort_manifest
           FROM cmd2_projection_navigation_coverage_range
          WHERE domain=$1 AND consumer_id=$2 AND through_seq>$3 AND through_seq<=$4
          ORDER BY through_seq LIMIT $5",
        &[
            &domain,
            &consumer_id,
            &as_i64(cursor)?,
            &as_i64(coverage_sequence)?,
            &coverage_row_cap,
        ],
    )?;
    let first_range_start = range_rows
        .first()
        .map(|row| as_u64(row.get::<_, i64>("cursor_before")))
        .transpose()?
        .unwrap_or(coverage_sequence);
    if first_range_start < cursor || first_range_start > coverage_sequence {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    let legacy_span = first_range_start - cursor;
    let coverage_rows = tx.query(
        "SELECT event_sequence,event_id,source_path,record_id,source_revision,
                input_root,dependency_root,output_root,previous_coverage_root,coverage_root,
                cohort_count,cohort_set_root,cohort_manifest
         FROM cmd2_projection_navigation_coverage_event
         WHERE domain=$1 AND consumer_id=$2 AND event_sequence>$3 AND event_sequence<=$4
         ORDER BY event_sequence LIMIT $5",
        &[
            &domain,
            &consumer_id,
            &as_i64(cursor)?,
            &as_i64(first_range_start)?,
            &coverage_row_cap,
        ],
    )?;
    if coverage_rows.len() as u64 != legacy_span {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    if !range_rows.is_empty()
        && !tx
            .query(
                "SELECT event_sequence
                   FROM cmd2_projection_navigation_coverage_event
                  WHERE domain=$1 AND consumer_id=$2 AND event_sequence>$3 AND event_sequence<=$4
                  LIMIT 1",
                &[
                    &domain,
                    &consumer_id,
                    &as_i64(first_range_start)?,
                    &as_i64(coverage_sequence)?,
                ],
            )?
            .is_empty()
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    let mut expected_coverage_root = verified_prefix_coverage_root.ok_or(DurableError::Corrupt(
        "navigation checkpoint coverage root absent",
    ))?;
    let mut terminal: Option<(String, Digest256, Digest256, Digest256)> = Some((
        consumer.get::<_, String>("last_event_id"),
        parse_digest(consumer.get::<_, String>("source_revision"))?,
        parse_digest(consumer.get::<_, String>("last_input_root"))?,
        parse_digest(consumer.get::<_, String>("last_output_root"))?,
    ));
    let mut manifest_bytes = 0u64;
    for (offset, row) in coverage_rows.iter().enumerate() {
        let sequence = as_u64(row.get("event_sequence"))?;
        let expected_sequence =
            cursor
                .checked_add(offset as u64 + 1)
                .ok_or(DurableError::Corrupt(
                    "navigation coverage sequence overflow",
                ))?;
        let previous_root = parse_digest(row.get("previous_coverage_root"))?;
        let source_revision = parse_digest(row.get("source_revision"))?;
        let input_root = parse_digest(row.get("input_root"))?;
        let dependency_root = parse_digest(row.get("dependency_root"))?;
        let output_root = parse_digest(row.get("output_root"))?;
        let event_id: String = row.get("event_id");
        let source_path: String = row.get("source_path");
        let record_id: String = row.get("record_id");
        let cohort_count: i32 = row.get("cohort_count");
        let cohort_set_root = row
            .get::<_, Option<String>>("cohort_set_root")
            .map(parse_digest)
            .transpose()?;
        let cohort_manifest = row.get::<_, Option<Vec<u8>>>("cohort_manifest");
        let (Some(cohort_set_root), Some(cohort_manifest)) = (cohort_set_root, cohort_manifest)
        else {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
            ));
        };
        manifest_bytes = manifest_bytes
            .checked_add(cohort_manifest.len() as u64)
            .ok_or(DurableError::Refused("navigation manifest byte budget"))?;
        if manifest_bytes > limits.max_total_bytes {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded,
            ));
        }
        if verify_navigation_event_cohort_manifest_single(
            &cohort_manifest,
            cohort_set_root,
            u32::try_from(cohort_count).unwrap_or(0),
            &event_id,
            expected_sequence,
            &source_path,
            &record_id,
            source_revision,
            input_root,
            dependency_root,
            output_root,
            limits.max_row_bytes as usize,
        )
        .is_err()
        {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
            ));
        }
        let calculated = navigation_coverage_root_next(
            expected_coverage_root,
            expected_sequence,
            &event_id,
            &source_path,
            &record_id,
            source_revision,
            input_root,
            dependency_root,
            output_root,
            cohort_set_root,
        );
        if sequence != expected_sequence
            || previous_root != expected_coverage_root
            || parse_digest(row.get("coverage_root"))? != calculated
        {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
            ));
        }
        terminal = Some((event_id, source_revision, input_root, output_root));
        expected_coverage_root = calculated;
    }
    let mut expected_range_cursor = first_range_start;
    for row in &range_rows {
        let cursor_before = as_u64(row.get("cursor_before"))?;
        let through_sequence = as_u64(row.get("through_seq"))?;
        let held_source_head_sequence = as_u64(row.get("held_source_head_seq"))?;
        let terminal_event_id: String = row.get("terminal_event_id");
        let range_root = parse_digest(row.get("range_root"))?;
        let range_manifest = row.get::<_, Vec<u8>>("range_manifest");
        let source_revision = parse_digest(row.get("source_revision"))?;
        let input_root = parse_digest(row.get("input_root"))?;
        let dependency_root = parse_digest(row.get("dependency_root"))?;
        let output_root = parse_digest(row.get("output_root"))?;
        let previous_root = parse_digest(row.get("previous_coverage_root"))?;
        let stored_root = parse_digest(row.get("coverage_root"))?;
        let cohort_count = u32::try_from(row.get::<_, i32>("terminal_cohort_count"))
            .map_err(|_| DurableError::Corrupt("navigation range cohort count is negative"))?;
        let terminal_cohort_root = parse_digest(row.get("terminal_cohort_root"))?;
        let terminal_manifest = row.get::<_, Vec<u8>>("terminal_cohort_manifest");
        manifest_bytes = manifest_bytes
            .checked_add(range_manifest.len() as u64)
            .and_then(|value| value.checked_add(terminal_manifest.len() as u64))
            .ok_or(DurableError::Refused(
                "navigation range manifest byte budget",
            ))?;
        if manifest_bytes > limits.max_total_bytes
            || cursor_before != expected_range_cursor
            || through_sequence <= cursor_before
            || through_sequence > coverage_sequence
            || held_source_head_sequence < through_sequence
            || held_source_head_sequence > head
        {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                if manifest_bytes > limits.max_total_bytes {
                    DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded
                } else {
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch
                },
            ));
        }
        if durable_navigation_range::verify_navigation_event_range_terminal_manifest(
            &terminal_manifest,
            terminal_cohort_root,
            cohort_count,
            cursor_before,
            through_sequence,
            held_source_head_sequence,
            &terminal_event_id,
            range_root,
            &range_manifest,
            input_root,
            dependency_root,
            output_root,
            limits.max_total_bytes as usize,
        )
        .is_err()
        {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
            ));
        }
        let calculated = durable_navigation_range::navigation_range_coverage_root_next(
            expected_coverage_root,
            cursor_before,
            through_sequence,
            held_source_head_sequence,
            &terminal_event_id,
            range_root,
            terminal_cohort_root,
            source_revision,
            dependency_root,
            output_root,
        );
        if previous_root != expected_coverage_root || stored_root != calculated {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
            ));
        }
        expected_coverage_root = calculated;
        expected_range_cursor = through_sequence;
        terminal = Some((terminal_event_id, source_revision, input_root, output_root));
    }
    let terminal_range_head_matches = range_rows
        .last()
        .map(|row| as_u64(row.get::<_, i64>("held_source_head_seq")))
        .transpose()?
        .is_none_or(|held_head| held_head == head);
    if expected_range_cursor != coverage_sequence
        || (coverage_sequence == head && !terminal_range_head_matches)
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    if Some(expected_coverage_root) != coverage_root {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    let owner_binding_raw: String = bootstrap.get("owner_binding_json");
    let owner_binding_value =
        strict_json_value(owner_binding_raw.as_bytes(), limits.max_row_bytes)?;
    let owner_binding = navigation_owner_binding_from_json(&owner_binding_value)?;
    let profile_digest = profile.source_contract;
    if owner_binding.domain != domain
        || owner_binding.selected_software_profile_id != profile.id
        || owner_binding.selected_software_profile_version != profile.version
        || owner_binding.selected_software_contract_digest != profile_digest
        || !tos_foundation::is_managed_knowledge_model_abi(&owner_binding.selected_model_abi)
        || owner_binding.selected_selection_profile
            != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
        || bootstrap_source_revision != owner_binding.selected_membership_root
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::MembershipMismatch,
        ));
    }
    if !published_generation {
        let membership = current
            .get::<_, Option<String>>("source_membership_digest")
            .map(parse_digest)
            .transpose()?;
        let membership_count = current
            .get::<_, Option<i64>>("source_membership_count")
            .map(as_u64)
            .transpose()?;
        let current_generation = current
            .get::<_, Option<i64>>("source_generation")
            .map(as_u64)
            .transpose()?;
        let selected_generation: Option<String> = current.get("selected_generation_digest");
        let source_projection: Option<String> = current.get("source_projection_digest");
        if membership != Some(owner_binding.source_membership_root)
            || membership_count != Some(owner_binding.source_membership_count)
            || current_generation != Some(owner_binding.managed_generation_commit_sequence)
            || selected_generation.as_deref()
                != Some(owner_binding.managed_generation_digest.to_hex().as_str())
            || source_projection.as_deref()
                != Some(owner_binding.source_projection_digest.to_hex().as_str())
        {
            return Ok(navigation_currentness_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::ManagedGenerationMismatch,
            ));
        }
    }
    let header_raw: String = bootstrap.get("header_raw");
    let header_raw = header_raw.into_bytes();
    let header_sha256 = parse_digest(bootstrap.get::<_, String>("header_sha256"))?;
    if Digest256::of_bytes(&header_raw) != header_sha256
        || header_sha256 != owner_binding.navigation_header_sha256
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::HeaderMismatch,
        ));
    }
    let header_value = strict_json_value(&header_raw, limits.max_row_bytes)?;
    if owner_binding.navigation_header != header_value {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::HeaderMismatch,
        ));
    }
    let counts = header_value
        .get("counts")
        .ok_or(DurableError::Corrupt("navigation stored header counts"))?;
    if counts.get("nodes").and_then(Value::as_u64) != Some(owner_binding.navigation_nodes)
        || counts.get("edges").and_then(Value::as_u64) != Some(owner_binding.navigation_edges)
        || counts.get("rights").and_then(Value::as_u64) != Some(owner_binding.navigation_rights)
        || as_u64(bootstrap.get("node_count"))? != owner_binding.navigation_nodes
        || as_u64(bootstrap.get("edge_count"))? != owner_binding.navigation_edges
        || as_u64(bootstrap.get("right_count"))? != owner_binding.navigation_rights
        || as_u64(bootstrap.get("projected_right_count"))?
            != owner_binding.navigation_projected_rights
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::BootstrapCoverageMismatch,
        ));
    }
    let cursor_source_revision: Option<String> = consumer.get("source_revision");
    let cursor_event_id: Option<String> = consumer.get("last_event_id");
    let cursor_input_root: Option<String> = consumer.get("last_input_root");
    let cursor_output_root: Option<String> = consumer.get("last_output_root");
    let cursor_source_revision = cursor_source_revision.map(parse_digest).transpose()?;
    let cursor_input_root = cursor_input_root.map(parse_digest).transpose()?;
    let cursor_output_root = cursor_output_root.map(parse_digest).transpose()?;
    if !navigation_terminal_coverage_is_current_in_tx(
        tx,
        domain,
        consumer_id,
        bootstrap_sequence,
        coverage_sequence,
        head,
        coverage_root,
        cursor_event_id.as_deref(),
        cursor_source_revision,
        cursor_input_root,
        cursor_output_root,
        limits,
    )? {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    let terminal_matches = match terminal {
        Some((event_id, source_revision, input_root, output_root)) => {
            cursor_event_id.as_deref() == Some(event_id.as_str())
                && cursor_source_revision == Some(source_revision)
                && cursor_input_root == Some(input_root)
                && cursor_output_root == Some(output_root)
        }
        None => {
            let bootstrap_event_id = format!("bootstrap:{}", bootstrap_root.to_hex());
            cursor_event_id.as_deref() == Some(bootstrap_event_id.as_str())
                && cursor_source_revision == Some(bootstrap_source_revision)
                && cursor_input_root == Some(bootstrap_root)
                && cursor_output_root == Some(bootstrap_root)
                && coverage_root == Some(bootstrap_root)
        }
    };
    if !terminal_matches {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    let checkpoint_sequence = consumer
        .get::<_, Option<i64>>("verified_prefix_seq")
        .map(as_u64)
        .transpose()?;
    let checkpoint_coverage_root = consumer
        .get::<_, Option<String>>("verified_prefix_coverage_root")
        .map(parse_digest)
        .transpose()?;
    let checkpoint_cohort_root = consumer
        .get::<_, Option<String>>("verified_prefix_cohort_root")
        .map(parse_digest)
        .transpose()?;
    let checkpoint_digest = consumer
        .get::<_, Option<String>>("verified_prefix_digest")
        .map(parse_digest)
        .transpose()?;
    let cohort_root = navigation_current_cohort_root_in_tx(tx, domain, consumer_id, limits)?;
    let (
        Some(checkpoint_sequence),
        Some(checkpoint_coverage_root),
        Some(checkpoint_cohort_root),
        Some(checkpoint_digest),
    ) = (
        checkpoint_sequence,
        checkpoint_coverage_root,
        checkpoint_cohort_root,
        checkpoint_digest,
    )
    else {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    };
    let terminal_event_id = cursor_event_id.as_deref().ok_or(DurableError::Corrupt(
        "navigation checkpoint terminal event absent",
    ))?;
    let terminal_source_revision = cursor_source_revision.ok_or(DurableError::Corrupt(
        "navigation checkpoint source revision absent",
    ))?;
    let terminal_input_root = cursor_input_root.ok_or(DurableError::Corrupt(
        "navigation checkpoint input root absent",
    ))?;
    let terminal_output_root = cursor_output_root.ok_or(DurableError::Corrupt(
        "navigation checkpoint output root absent",
    ))?;
    let expected_checkpoint_digest = navigation_verified_prefix_digest(
        domain,
        consumer_id,
        profile,
        bootstrap_sequence,
        bootstrap_root,
        &owner_binding_raw,
        cursor,
        expected_coverage_root,
        cohort_root,
        adjacency_root.unwrap_or(bootstrap_root),
        terminal_event_id,
        terminal_source_revision,
        terminal_input_root,
        terminal_output_root,
    );
    if checkpoint_sequence != cursor
        || checkpoint_coverage_root != expected_coverage_root
        || checkpoint_cohort_root != cohort_root
        || checkpoint_digest != expected_checkpoint_digest
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
        ));
    }
    if !consumer.get::<_, bool>("adjacency_ready")
        || adjacency_sequence != cursor
        || adjacency_root.is_none()
    {
        return Ok(navigation_currentness_not_ready(
            head,
            Some(cursor),
            DurableNavigationCurrentIndexNotReadyReasonV1::AdjacencyCoverageMismatch,
        ));
    }
    let adjacency_root = adjacency_root.ok_or(DurableError::Corrupt(
        "navigation adjacency root absent after readiness check",
    ))?;
    let index_root = navigation_current_index_root(
        bootstrap_root,
        expected_coverage_root,
        adjacency_root,
        cohort_root,
    );
    Ok(DurableNavigationCurrentnessV1::Ready(
        DurableNavigationCurrentnessGuardV1 {
            domain_head_sequence: head,
            coverage_sequence,
            coverage_root: expected_coverage_root,
            adjacency_root,
            cohort_root,
            source_revision: cursor_source_revision.ok_or(DurableError::Corrupt(
                "navigation cursor source revision absent",
            ))?,
            index_root,
            audit_generation: as_u64(snapshot.audit_generation)?,
            rights_version: as_u64(snapshot.rights_version)?,
            rights_allowed: snapshot.rights_allowed,
            profile_id: profile.id.clone(),
            profile_version: profile.version,
            source_contract_digest: profile.source_contract,
            owner_binding,
            bootstrap_root,
            header_raw,
            header_sha256,
        },
    ))
}

/// Read currentness from a separate PostgreSQL client. The verified-prefix
/// commitment touches only the bounded current-cohort rows; it does not load
/// projected carrier payloads or event history.
pub(crate) fn read_navigation_currentness_guard_with_client_v1(
    client: &mut postgres::Client,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationCurrentnessV1> {
    let limits = limits.validate()?;
    validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation current domain")?;
    validate_identifier(
        consumer_id,
        super::MAX_CONSUMER_ID_BYTES,
        "navigation current consumer ID",
    )?;
    profile.validate()?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    let result = read_navigation_currentness_guard_with_tx_v1(
        &mut tx,
        domain,
        consumer_id,
        profile,
        limits,
    )?;
    tx.commit()?;
    Ok(result)
}

/// Read currentness inside a caller-owned repeatable-read transaction. The
/// caller controls the transaction boundary and receives no carrier rows.
pub(crate) fn read_navigation_currentness_guard_with_tx_v1(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationCurrentnessV1> {
    let limits = limits.validate()?;
    validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation current domain")?;
    validate_identifier(
        consumer_id,
        super::MAX_CONSUMER_ID_BYTES,
        "navigation current consumer ID",
    )?;
    profile.validate()?;
    set_local_timeouts(tx, limits.statement_timeout)?;
    read_navigation_currentness_guard_in_tx(tx, domain, consumer_id, profile, limits)
}

/// Validate the last atomically published navigation generation under the
/// caller's repeatable-read transaction. Unlike the V1 currentness guard,
/// this proof remains Ready while CMD has a later source head; it certifies
/// the published prefix and the current access/profile fence, not freshness
/// against source events beyond that prefix.
pub(crate) fn read_navigation_published_guard_with_tx_v2(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationPublishedGenerationV2> {
    let limits = limits.validate()?;
    validate_identifier(
        domain,
        super::MAX_DOMAIN_BYTES,
        "navigation published domain",
    )?;
    validate_identifier(
        consumer_id,
        super::MAX_CONSUMER_ID_BYTES,
        "navigation published consumer ID",
    )?;
    profile.validate()?;
    set_local_timeouts(tx, limits.statement_timeout)?;
    match read_navigation_currentness_guard_in_tx_mode(
        tx,
        domain,
        consumer_id,
        profile,
        limits,
        true,
    )? {
        DurableNavigationCurrentnessV1::Ready(inner) => {
            Ok(DurableNavigationPublishedGenerationV2::Ready(
                DurableNavigationPublishedGenerationGuardV2 { inner },
            ))
        }
        DurableNavigationCurrentnessV1::NotReady(not_ready) => {
            Ok(DurableNavigationPublishedGenerationV2::NotReady(not_ready))
        }
    }
}

/// The callback result is intentionally distinct from failures. `NotReady`
/// keeps the exact published-generation reason and never invokes the reader.
pub(crate) enum DurableNavigationHeldReadV2<T> {
    Ready(T),
    NotReady(DurableNavigationCurrentIndexNotReadyV1),
}

/// Run a bounded source-navigation read under the established managed-owner
/// fence order and one repeatable-read transaction. The selected parent opens
/// the immutable source model and acquires filesystem owner, audit, domain,
/// then job fences; this wrapper rechecks the V2 published proof and its full
/// static binding before allowing the callback to use it. V2 may certify a
/// published cursor below head; the strict V1 currentness path is never used
/// here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_published_navigation_generation_v2<P, T>(
    selected: &ManagedAgentSelectedParent<P>,
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    filesystem: &CreationFilesystem,
    package: &ManagedSerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'model, 'lease, 'guard, 'tx> FnOnce(
        &'model VerifiedKnowledgeModel<'static>,
        &'lease ManagedCurrentModelLease<'lease, P>,
        &'guard DurableNavigationPublishedGenerationGuardV2,
        &mut postgres::Transaction<'tx>,
    ) -> ManagedSelectionResult<T>,
) -> ManagedSelectionResult<DurableNavigationHeldReadV2<T>>
where
    P: ManagedSelectedProof,
{
    let limits = limits.validate().map_err(ManagedSelectionError::Durable)?;
    validate_identifier(
        domain,
        super::MAX_DOMAIN_BYTES,
        "navigation published domain",
    )
    .map_err(ManagedSelectionError::Durable)?;
    validate_identifier(
        consumer_id,
        super::MAX_CONSUMER_ID_BYTES,
        "navigation published consumer ID",
    )
    .map_err(ManagedSelectionError::Durable)?;
    validate_identifier(job_id, 256, "navigation published job ID")
        .map_err(ManagedSelectionError::Durable)?;
    profile.validate().map_err(ManagedSelectionError::Durable)?;
    if store.custody_domain() != domain.as_bytes() {
        return Err(ManagedSelectionError::Durable(DurableError::Conflict(
            "navigation published source store differs",
        )));
    }
    let expected_binding = navigation_owner_binding(domain, selected, profile)
        .map_err(ManagedSelectionError::Durable)?;
    if expected_binding.selected_software_contract_digest != contract {
        return Err(ManagedSelectionError::Durable(DurableError::Conflict(
            "navigation published selected contract differs",
        )));
    }

    selected.with_current_model_and_tx(
        coordinator,
        store,
        filesystem,
        package,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        deadline,
        cancelled,
        |model, source, tx| {
            let isolation_row = tx
                .query_one("SHOW transaction_isolation", &[])
                .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
            let isolation: String = isolation_row.get(0);
            if isolation != "repeatable read" {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published read requires repeatable read",
                )));
            }
            set_local_timeouts(tx, limits.statement_timeout)
                .map_err(ManagedSelectionError::Durable)?;

            // The selected owner wrapper has already acquired audit, domain,
            // and job locks in order. Read their state from the same
            // repeatable-read cut without taking those locks out of order.
            let snapshot =
                read_domain_snapshot(tx, domain).map_err(ManagedSelectionError::Durable)?;
            snapshot
                .require_live_projection(profile)
                .map_err(ManagedSelectionError::Durable)?;
            if !snapshot.source_complete
                || as_u64(snapshot.audit_generation).map_err(ManagedSelectionError::Durable)?
                    != selected.generation().audit_generation()
                || as_u64(snapshot.head_seq).map_err(ManagedSelectionError::Durable)?
                    != selected.generation().commit_seq()
                || as_u64(snapshot.rule_version).map_err(ManagedSelectionError::Durable)?
                    != rule_version
                || as_u64(snapshot.rights_version).map_err(ManagedSelectionError::Durable)?
                    != rights_version
                || snapshot.contract_digest != contract.to_hex()
            {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published live owner fence changed",
                )));
            }

            let current = tx
                .query_one(
                    "SELECT source_membership_digest,source_membership_count,source_generation,
                            selected_generation_digest,source_projection_digest
                     FROM cmd2_domain WHERE domain=$1",
                    &[&domain],
                )
                .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
            let membership = expected_binding.source_membership_root.to_hex();
            let generation = expected_binding.managed_generation_digest.to_hex();
            let projection = expected_binding.source_projection_digest.to_hex();
            if current
                .get::<_, Option<String>>("source_membership_digest")
                .as_deref()
                != Some(membership.as_str())
                || current.get::<_, Option<i64>>("source_membership_count")
                    != Some(
                        as_i64(expected_binding.source_membership_count)
                            .map_err(ManagedSelectionError::Durable)?,
                    )
                || current.get::<_, Option<i64>>("source_generation")
                    != Some(
                        as_i64(expected_binding.managed_generation_commit_sequence)
                            .map_err(ManagedSelectionError::Durable)?,
                    )
                || current
                    .get::<_, Option<String>>("selected_generation_digest")
                    .as_deref()
                    != Some(generation.as_str())
                || current
                    .get::<_, Option<String>>("source_projection_digest")
                    .as_deref()
                    != Some(projection.as_str())
            {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published selected source binding changed",
                )));
            }

            let job = tx
                .query_opt(
                    "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
                    &[&domain, &job_id],
                )
                .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
            if job.map(|row| row.get::<_, i64>(0))
                != Some(as_i64(job_fence).map_err(ManagedSelectionError::Durable)?)
            {
                return Err(ManagedSelectionError::Durable(DurableError::Refused(
                    "navigation published current job changed",
                )));
            }

            // The event/bootstrap writers update this row in the same
            // transaction as carrier replacement. Holding it shared keeps
            // the published carrier generation stable through the callback.
            tx.query_opt(
                "SELECT cursor_seq FROM cmd2_projection_navigation_consumer
                 WHERE domain=$1 AND consumer_id=$2 FOR SHARE",
                &[&domain, &consumer_id],
            )
            .map_err(|error| ManagedSelectionError::Durable(error.into()))?;

            let published = read_navigation_published_guard_with_tx_v2(
                tx,
                domain,
                consumer_id,
                profile,
                limits,
            )
            .map_err(ManagedSelectionError::Durable)?;
            let guard = match published {
                DurableNavigationPublishedGenerationV2::Ready(guard) => guard,
                DurableNavigationPublishedGenerationV2::NotReady(reason) => {
                    return Ok(DurableNavigationHeldReadV2::NotReady(reason));
                }
            };
            if guard.owner_binding() != &expected_binding
                || guard.domain_head_sequence()
                    != as_u64(snapshot.head_seq).map_err(ManagedSelectionError::Durable)?
                || guard.audit_generation()
                    != as_u64(snapshot.audit_generation).map_err(ManagedSelectionError::Durable)?
                || guard.rights_version()
                    != as_u64(snapshot.rights_version).map_err(ManagedSelectionError::Durable)?
                || !guard.rights_allowed()
                || guard.profile_id() != profile.id
                || guard.profile_version() != profile.version
                || guard.source_contract_digest() != profile.source_contract
                || guard.published_sequence() > guard.domain_head_sequence()
            {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published proof differs from the held owner fence",
                )));
            }
            source
                .require_managed_basis(selected.source_proof())
                .map_err(ManagedSelectionError::Source)?;
            consume(model, source, &guard, tx).map(DurableNavigationHeldReadV2::Ready)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn with_published_navigation_generation_v2_selected_owner<P, T>(
    selected: &ManagedAgentSelectedParent<P>,
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    filesystem: &CreationFilesystem,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'model, 'lease, 'guard, 'tx> FnOnce(
        &'model VerifiedKnowledgeModel<'static>,
        &'lease ManagedCurrentModelLease<'lease, P>,
        &'guard DurableNavigationPublishedGenerationGuardV2,
        &mut postgres::Transaction<'tx>,
    ) -> ManagedSelectionResult<T>,
) -> ManagedSelectionResult<DurableNavigationHeldReadV2<T>>
where
    P: ManagedSelectedProof,
{
    let limits = limits.validate().map_err(ManagedSelectionError::Durable)?;
    validate_identifier(
        domain,
        super::MAX_DOMAIN_BYTES,
        "navigation published domain",
    )
    .map_err(ManagedSelectionError::Durable)?;
    validate_identifier(
        consumer_id,
        super::MAX_CONSUMER_ID_BYTES,
        "navigation published consumer ID",
    )
    .map_err(ManagedSelectionError::Durable)?;
    validate_identifier(job_id, 256, "navigation published job ID")
        .map_err(ManagedSelectionError::Durable)?;
    profile.validate().map_err(ManagedSelectionError::Durable)?;
    if store.custody_domain() != domain.as_bytes() {
        return Err(ManagedSelectionError::Durable(DurableError::Conflict(
            "navigation published source store differs",
        )));
    }
    let expected_binding = navigation_owner_binding(domain, selected, profile)
        .map_err(ManagedSelectionError::Durable)?;
    if expected_binding.selected_software_contract_digest != contract {
        return Err(ManagedSelectionError::Durable(DurableError::Conflict(
            "navigation published selected contract differs",
        )));
    }

    selected.with_current_model_and_tx_source_navigation(
        coordinator,
        store,
        filesystem,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        deadline,
        cancelled,
        |model, source, tx| {
            let isolation_row = tx
                .query_one("SHOW transaction_isolation", &[])
                .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
            let isolation: String = isolation_row.get(0);
            if isolation != "repeatable read" {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published read requires repeatable read",
                )));
            }
            set_local_timeouts(tx, limits.statement_timeout)
                .map_err(ManagedSelectionError::Durable)?;

            // The selected owner wrapper has already acquired audit, domain,
            // and job locks in order. Read their state from the same
            // repeatable-read cut without taking those locks out of order.
            let snapshot =
                read_domain_snapshot(tx, domain).map_err(ManagedSelectionError::Durable)?;
            snapshot
                .require_live_projection(profile)
                .map_err(ManagedSelectionError::Durable)?;
            if !snapshot.source_complete
                || as_u64(snapshot.audit_generation).map_err(ManagedSelectionError::Durable)?
                    != selected.generation().audit_generation()
                || as_u64(snapshot.head_seq).map_err(ManagedSelectionError::Durable)?
                    != selected.generation().commit_seq()
                || as_u64(snapshot.rule_version).map_err(ManagedSelectionError::Durable)?
                    != rule_version
                || as_u64(snapshot.rights_version).map_err(ManagedSelectionError::Durable)?
                    != rights_version
                || snapshot.contract_digest != contract.to_hex()
            {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published live owner fence changed",
                )));
            }

            let current = tx
                .query_one(
                    "SELECT source_membership_digest,source_membership_count,source_generation,
                            selected_generation_digest,source_projection_digest
                     FROM cmd2_domain WHERE domain=$1",
                    &[&domain],
                )
                .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
            let membership = expected_binding.source_membership_root.to_hex();
            let generation = expected_binding.managed_generation_digest.to_hex();
            let projection = expected_binding.source_projection_digest.to_hex();
            if current
                .get::<_, Option<String>>("source_membership_digest")
                .as_deref()
                != Some(membership.as_str())
                || current.get::<_, Option<i64>>("source_membership_count")
                    != Some(
                        as_i64(expected_binding.source_membership_count)
                            .map_err(ManagedSelectionError::Durable)?,
                    )
                || current.get::<_, Option<i64>>("source_generation")
                    != Some(
                        as_i64(expected_binding.managed_generation_commit_sequence)
                            .map_err(ManagedSelectionError::Durable)?,
                    )
                || current
                    .get::<_, Option<String>>("selected_generation_digest")
                    .as_deref()
                    != Some(generation.as_str())
                || current
                    .get::<_, Option<String>>("source_projection_digest")
                    .as_deref()
                    != Some(projection.as_str())
            {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published selected source binding changed",
                )));
            }

            let job = tx
                .query_opt(
                    "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
                    &[&domain, &job_id],
                )
                .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
            if job.map(|row| row.get::<_, i64>(0))
                != Some(as_i64(job_fence).map_err(ManagedSelectionError::Durable)?)
            {
                return Err(ManagedSelectionError::Durable(DurableError::Refused(
                    "navigation published current job changed",
                )));
            }

            // The event/bootstrap writers update this row in the same
            // transaction as carrier replacement. Holding it shared keeps
            // the published carrier generation stable through the callback.
            tx.query_opt(
                "SELECT cursor_seq FROM cmd2_projection_navigation_consumer
                 WHERE domain=$1 AND consumer_id=$2 FOR SHARE",
                &[&domain, &consumer_id],
            )
            .map_err(|error| ManagedSelectionError::Durable(error.into()))?;

            let published = read_navigation_published_guard_with_tx_v2(
                tx,
                domain,
                consumer_id,
                profile,
                limits,
            )
            .map_err(ManagedSelectionError::Durable)?;
            let guard = match published {
                DurableNavigationPublishedGenerationV2::Ready(guard) => guard,
                DurableNavigationPublishedGenerationV2::NotReady(reason) => {
                    return Ok(DurableNavigationHeldReadV2::NotReady(reason));
                }
            };
            if guard.owner_binding() != &expected_binding
                || guard.domain_head_sequence()
                    != as_u64(snapshot.head_seq).map_err(ManagedSelectionError::Durable)?
                || guard.audit_generation()
                    != as_u64(snapshot.audit_generation).map_err(ManagedSelectionError::Durable)?
                || guard.rights_version()
                    != as_u64(snapshot.rights_version).map_err(ManagedSelectionError::Durable)?
                || !guard.rights_allowed()
                || guard.profile_id() != profile.id
                || guard.profile_version() != profile.version
                || guard.source_contract_digest() != profile.source_contract
                || guard.published_sequence() > guard.domain_head_sequence()
            {
                return Err(ManagedSelectionError::Durable(DurableError::Conflict(
                    "navigation published proof differs from the held owner fence",
                )));
            }
            source
                .require_managed_basis(selected.source_proof())
                .map_err(ManagedSelectionError::Source)?;
            consume(model, source, &guard, tx).map(DurableNavigationHeldReadV2::Ready)
        },
    )
}

#[derive(Clone, Copy)]
struct DurableNavigationBootstrapHeldFencesV1<'a> {
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &'a str,
    job_fence: u64,
}

impl DurablePgCoordinator {
    /// Publish the exact full-source baseline selected from a managed current
    /// generation. This establishes the only bootstrap point from which a
    /// global current-index read can become Ready.
    pub fn publish_navigation_bootstrap_v1<P: ManagedSelectedProof>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        selected: &ManagedAgentSelectedParent<P>,
        package: &DurableNavigationBootstrapPackageV1,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationBootstrapApplyV1> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let result = Self::publish_navigation_bootstrap_v1_inner(
            &mut tx,
            store,
            domain,
            consumer_id,
            profile,
            selected,
            package,
            limits,
            None,
        )?;
        tx.commit()?;
        Ok(result)
    }

    /// Publish inside the already-held managed source-navigation owner
    /// transaction. The caller owns the RR transaction and its audit, domain,
    /// rights, job, and source-store fences; this writer rechecks their exact
    /// values in that same snapshot and never opens a nested transaction.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish_navigation_bootstrap_v1_in_tx<P: ManagedSelectedProof>(
        tx: &mut postgres::Transaction<'_>,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        selected: &ManagedAgentSelectedParent<P>,
        package: &DurableNavigationBootstrapPackageV1,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationBootstrapApplyV1> {
        validate_identifier(job_id, 256, "navigation bootstrap job ID")?;
        let fences = DurableNavigationBootstrapHeldFencesV1 {
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
        };
        Self::publish_navigation_bootstrap_v1_inner(
            tx,
            store,
            domain,
            consumer_id,
            profile,
            selected,
            package,
            limits,
            Some(&fences),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn publish_navigation_bootstrap_v1_inner<P: ManagedSelectedProof>(
        tx: &mut postgres::Transaction<'_>,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        selected: &ManagedAgentSelectedParent<P>,
        package: &DurableNavigationBootstrapPackageV1,
        limits: DurableNavigationEventLimits,
        held_fences: Option<&DurableNavigationBootstrapHeldFencesV1<'_>>,
    ) -> DurableResult<DurableNavigationBootstrapApplyV1> {
        let limits = limits.validate()?;
        validate_identifier(
            domain,
            super::MAX_DOMAIN_BYTES,
            "navigation bootstrap domain",
        )?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation bootstrap consumer ID",
        )?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() || package.domain != domain {
            return Err(DurableError::Conflict(
                "navigation bootstrap domain differs",
            ));
        }
        if package.rows.len() > limits.max_current_rows
            || package.through_sequence == 0
            || selected.generation().commit_seq() != package.through_sequence
        {
            return Err(DurableError::Refused(
                "navigation bootstrap coverage or row budget",
            ));
        }
        let mut selected_binding = navigation_owner_binding(domain, selected, profile)?;
        selected_binding.navigation_header =
            strict_json_value(&package.header_raw, limits.max_row_bytes)?;
        // The sealed package has authenticated the original rights and their
        // projection. Re-derive its row count instead of comparing with the
        // unpopulated binding's default zero.
        selected_binding.navigation_projected_rights = package
            .rows
            .iter()
            .filter(|row| row.kind == DurableNavigationBootstrapRowKindV1::Right)
            .map(|row| row.id.as_str())
            .collect::<BTreeSet<_>>()
            .len() as u64;
        if navigation_owner_binding_json(&selected_binding)
            != navigation_owner_binding_json(&package.owner_binding)
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap selected owner binding changed",
            ));
        }
        let header_sha256 = Digest256::of_bytes(&package.header_raw);
        if header_sha256 != package.header_sha256
            || package.bootstrap_root
                != navigation_bootstrap_root(
                    &package.owner_binding,
                    &package.header_raw,
                    &package.rows,
                    &package.owner_sources,
                    limits.max_total_bytes as usize,
                )?
        {
            return Err(DurableError::Corrupt("navigation bootstrap package root"));
        }
        let adjacency_certificates =
            bootstrap_adjacency_certificates(package.bootstrap_root, &package.rows, limits)?;
        let adjacency_root =
            navigation_adjacency_root_initial(package.bootstrap_root, &adjacency_certificates);
        let deadline = Instant::now() + limits.statement_timeout;
        let profile_version = profile.version.to_string();
        let source_contract = profile.source_contract.to_hex();
        let owner_binding_json =
            serde_json::to_string(&navigation_owner_binding_json(&package.owner_binding))
                .map_err(|_| DurableError::Invalid("navigation bootstrap owner binding JSON"))?;
        let header_raw = String::from_utf8(package.header_raw.clone())
            .map_err(|_| DurableError::Invalid("navigation bootstrap header UTF-8"))?;
        set_local_timeouts(tx, limits.statement_timeout)?;
        let snapshot = if held_fences.is_some() {
            let isolation_row = tx.query_one("SHOW transaction_isolation", &[])?;
            let isolation: String = isolation_row.get(0);
            if isolation != "repeatable read" {
                return Err(DurableError::Conflict(
                    "navigation bootstrap requires the held repeatable-read owner transaction",
                ));
            }
            read_domain_snapshot(tx, domain)?
        } else {
            lock_domain_snapshot(tx, domain)?
        };
        snapshot.require_live_projection(profile)?;
        if let Some(fences) = held_fences {
            if snapshot.audit_generation != as_i64(selected.generation().audit_generation())?
                || snapshot.rights_version != as_i64(fences.rights_version)?
                || snapshot.rule_version != as_i64(fences.rule_version)?
                || snapshot.contract_digest != fences.contract.to_hex()
                || !snapshot.rights_allowed
            {
                return Err(DurableError::Conflict(
                    "navigation bootstrap held source/rights fence differs",
                ));
            }
            let job = tx.query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
                &[&domain, &fences.job_id],
            )?;
            if job.map(|row| row.get::<_, i64>(0)) != Some(as_i64(fences.job_fence)?) {
                return Err(DurableError::Refused(
                    "navigation bootstrap held job fence differs",
                ));
            }
        }
        if snapshot.head_seq()? != package.through_sequence {
            return Err(DurableError::Conflict(
                "navigation bootstrap is not the current CMD head",
            ));
        }
        let current_sql = if held_fences.is_some() {
            "SELECT source_membership_digest,source_membership_count,source_generation,
                    source_complete,selected_generation_digest,source_projection_digest
             FROM cmd2_domain WHERE domain=$1"
        } else {
            "SELECT source_membership_digest,source_membership_count,source_generation,
                    source_complete,selected_generation_digest,source_projection_digest
             FROM cmd2_domain WHERE domain=$1 FOR UPDATE"
        };
        let current = tx.query_one(current_sql, &[&domain])?;
        let current_membership: Option<String> = current.get("source_membership_digest");
        let current_members = current
            .get::<_, Option<i64>>("source_membership_count")
            .map(as_u64)
            .transpose()?;
        let current_generation = current
            .get::<_, Option<i64>>("source_generation")
            .map(as_u64)
            .transpose()?;
        let selected_generation: Option<String> = current.get("selected_generation_digest");
        let source_projection: Option<String> = current.get("source_projection_digest");
        if !current.get::<_, bool>("source_complete")
            || current_membership.as_deref()
                != Some(
                    package
                        .owner_binding
                        .source_membership_root
                        .to_hex()
                        .as_str(),
                )
            || current_members != Some(package.owner_binding.source_membership_count)
            || current_generation != Some(package.through_sequence)
            || selected_generation.as_deref()
                != Some(
                    package
                        .owner_binding
                        .managed_generation_digest
                        .to_hex()
                        .as_str(),
                )
            || source_projection.as_deref()
                != Some(
                    package
                        .owner_binding
                        .source_projection_digest
                        .to_hex()
                        .as_str(),
                )
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap managed currentness differs from CMD",
            ));
        }
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_consumer(
                domain,consumer_id,profile_id,profile_version,source_contract_digest)
             VALUES($1,$2,$3,$4,$5) ON CONFLICT(domain,consumer_id) DO NOTHING",
            &[
                &domain,
                &consumer_id,
                &profile.id,
                &profile_version,
                &source_contract,
            ],
        )?;
        let consumer = tx.query_one(
            "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,
                    source_revision,last_event_id,last_input_root,last_output_root,coverage_seq,coverage_root,
                    adjacency_seq,adjacency_root,adjacency_ready,verified_prefix_version,verified_prefix_seq,
                    verified_prefix_coverage_root,verified_prefix_cohort_root,verified_prefix_digest
             FROM cmd2_projection_navigation_consumer
             WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?;
        if consumer.get::<_, String>("profile_id") != profile.id
            || consumer.get::<_, String>("profile_version") != profile_version
            || consumer.get::<_, String>("source_contract_digest") != source_contract
        {
            return Err(DurableError::Conflict(
                "navigation bootstrap consumer profile differs",
            ));
        }
        let cursor = as_u64(consumer.get("cursor_seq"))?;
        let existing = tx.query_opt(
            "SELECT through_seq,bootstrap_root,owner_binding_json FROM cmd2_projection_navigation_bootstrap
             WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?;
        if cursor == package.through_sequence {
            if let Some(existing) = existing {
                if consumer.get::<_, Option<i16>>("verified_prefix_version")
                    == Some(VERIFIED_PREFIX_FORMAT_VERSION)
                {
                    let same_baseline = as_u64(existing.get("through_seq"))?
                        == package.through_sequence
                        && existing.get::<_, String>("bootstrap_root").trim_end()
                            == package.bootstrap_root.to_hex()
                        && as_u64(consumer.get("coverage_seq"))? == package.through_sequence
                        && consumer
                            .get::<_, Option<String>>("coverage_root")
                            .as_deref()
                            == Some(package.bootstrap_root.to_hex().as_str())
                        && as_u64(consumer.get("adjacency_seq"))? == package.through_sequence
                        && consumer
                            .get::<_, Option<String>>("adjacency_root")
                            .as_deref()
                            == Some(adjacency_root.to_hex().as_str())
                        && consumer.get::<_, bool>("adjacency_ready");
                    if same_baseline {
                        let existing_sequence = as_u64(existing.get("through_seq"))?;
                        let existing_root = parse_digest(existing.get("bootstrap_root"))?;
                        let owner_binding_json: String = existing.get("owner_binding_json");
                        match verify_navigation_verified_prefix_in_tx(
                            tx,
                            domain,
                            consumer_id,
                            profile,
                            existing_sequence,
                            existing_root,
                            &owner_binding_json,
                            &consumer,
                            limits,
                        ) {
                            Ok(()) => {
                                return Ok(DurableNavigationBootstrapApplyV1 {
                                    through_sequence: package.through_sequence,
                                    bootstrap_root: package.bootstrap_root,
                                    replayed: true,
                                });
                            }
                            Err(error @ DurableError::Database(_))
                            | Err(error @ DurableError::Storage(_))
                            | Err(error @ DurableError::Source(_))
                            | Err(error @ DurableError::Indeterminate(_)) => return Err(error),
                            Err(
                                DurableError::Invalid(_)
                                | DurableError::Conflict(_)
                                | DurableError::Refused(_)
                                | DurableError::Corrupt(_),
                            ) => {}
                        }
                    }
                }
                // An explicit, current source-backed bootstrap replaces a
                // stale, corrupt, v1, or unversioned derived navigation chain.
                // Never manufacture v2 trust from its stored cursor or rows.
            }
        }
        if cursor > package.through_sequence {
            return Err(DurableError::Conflict(
                "navigation bootstrap would move coverage backwards",
            ));
        }
        tx.execute(
            "DELETE FROM cmd2_projection_navigation_cohort
             WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        tx.execute(
            "DELETE FROM cmd2_projection_navigation_coverage_event
             WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        tx.execute(
            "DELETE FROM cmd2_projection_navigation_coverage_range
             WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        tx.execute(
            "DELETE FROM cmd2_projection_navigation_adjacency_certificate
             WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        tx.execute(
            "DELETE FROM cmd2_projection_navigation_bootstrap
             WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_bootstrap(
                domain,consumer_id,through_seq,source_revision,bootstrap_root,
                owner_binding_json,header_raw,header_sha256,node_count,edge_count,
                right_count,projected_right_count,diagnostic_count)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
            &[
                &domain,
                &consumer_id,
                &as_i64(package.through_sequence)?,
                &package.source_revision.to_hex(),
                &package.bootstrap_root.to_hex(),
                &owner_binding_json,
                &header_raw,
                &header_sha256.to_hex(),
                &as_i64(package.owner_binding.navigation_nodes)?,
                &as_i64(package.owner_binding.navigation_edges)?,
                &as_i64(package.owner_binding.navigation_rights)?,
                &as_i64(package.owner_binding.navigation_projected_rights)?,
                &i64::try_from(
                    package
                        .rows
                        .iter()
                        .filter(|row| row.kind == DurableNavigationBootstrapRowKindV1::Diagnostic)
                        .count(),
                )
                .map_err(|_| DurableError::Refused("navigation diagnostic count"))?,
            ],
        )?;
        for owner in package.owner_sources.values() {
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_bootstrap_owner(
                    domain,consumer_id,owner_source_path,source_sha256,size_bytes)
                 VALUES($1,$2,$3,$4,$5)",
                &[
                    &domain,
                    &consumer_id,
                    &owner.source_path,
                    &owner.sha256.to_hex(),
                    &as_i64(owner.size_bytes)?,
                ],
            )?;
        }
        for row in &package.rows {
            let row_json = std::str::from_utf8(&row.raw)
                .map_err(|_| DurableError::Invalid("navigation bootstrap carrier UTF-8"))?;
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_bootstrap_row(
                    domain,consumer_id,row_kind,row_id,owner_source_path,owner_record_id,
                    visible,from_id,to_id,row_json,row_sha256)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
                &[
                    &domain,
                    &consumer_id,
                    &row.kind.as_str(),
                    &row.id,
                    &row.owner_source_path,
                    &row.owner_record_id,
                    &row.visible,
                    &row.from_id,
                    &row.to_id,
                    &row_json,
                    &row.sha256.to_hex(),
                ],
            )?;
        }
        for ((owner_path, owner_id), dependency_paths) in &package.dependencies {
            for dependency_path in dependency_paths {
                tx.execute(
                    "INSERT INTO cmd2_projection_navigation_bootstrap_dependency(
                        domain,consumer_id,owner_source_path,owner_record_id,dependency_path)
                     VALUES($1,$2,$3,$4,$5)",
                    &[&domain, &consumer_id, owner_path, owner_id, dependency_path],
                )?;
            }
        }
        for certificate in &adjacency_certificates {
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_adjacency_certificate(
                    domain,consumer_id,node_id,edge_count,edge_digest,certificate_root)
                 VALUES($1,$2,$3,$4,$5,$6)",
                &[
                    &domain,
                    &consumer_id,
                    &certificate.node_id,
                    &as_i64(certificate.edge_count)?,
                    &certificate.edge_digest.to_hex(),
                    &certificate.certificate_root.to_hex(),
                ],
            )?;
        }
        let synthetic_event_id = format!("bootstrap:{}", package.bootstrap_root.to_hex());
        let cohort_root = navigation_current_cohort_root_in_tx(tx, domain, consumer_id, limits)?;
        let checkpoint_digest = navigation_verified_prefix_digest(
            domain,
            consumer_id,
            profile,
            package.through_sequence,
            package.bootstrap_root,
            &owner_binding_json,
            package.through_sequence,
            package.bootstrap_root,
            cohort_root,
            adjacency_root,
            &synthetic_event_id,
            package.source_revision,
            package.bootstrap_root,
            package.bootstrap_root,
        );
        let updated = tx.execute(
            "UPDATE cmd2_projection_navigation_consumer
             SET cursor_seq=$3,source_revision=$4,last_event_id=$5,last_input_root=$6,
                 last_output_root=$7,coverage_seq=$3,coverage_root=$8,
                 adjacency_seq=$3,adjacency_root=$9,adjacency_ready=true,
                 verified_prefix_version=$13,
                 verified_prefix_seq=$3,verified_prefix_coverage_root=$8,
                 verified_prefix_cohort_root=$10,verified_prefix_digest=$11
             WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=$12",
            &[
                &domain,
                &consumer_id,
                &as_i64(package.through_sequence)?,
                &package.source_revision.to_hex(),
                &synthetic_event_id,
                &package.bootstrap_root.to_hex(),
                &package.bootstrap_root.to_hex(),
                &package.bootstrap_root.to_hex(),
                &adjacency_root.to_hex(),
                &cohort_root.to_hex(),
                &checkpoint_digest.to_hex(),
                &as_i64(cursor)?,
                &VERIFIED_PREFIX_FORMAT_VERSION,
            ],
        )?;
        if updated != 1 {
            return Err(DurableError::Conflict(
                "navigation bootstrap cursor compare-and-set failed",
            ));
        }
        active_navigation(deadline)?;
        Ok(DurableNavigationBootstrapApplyV1 {
            through_sequence: package.through_sequence,
            bootstrap_root: package.bootstrap_root,
            replayed: false,
        })
    }

    /// Resolve an event only when it is the current CMD source head in the
    /// same repeatable-read cut as its selected source bytes. This is the
    /// preferred entry point for a compiler that binds its SourceRevision to
    /// current source state. Historical event-local inspection remains
    /// available through `read_navigation_event_sources_v1`.
    pub fn read_navigation_next_event_seed_v1(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        event_path: &str,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventSeedV1> {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation event domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation event consumer ID",
        )?;
        validate_relative_path(event_path)?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let cursor =
            read_navigation_event_cursor(&mut self.client, domain, consumer_id, profile, limits)?;
        let source_head_sequence =
            read_navigation_event_head(&mut self.client, domain, profile, limits)?;
        if cursor.sequence > source_head_sequence {
            return Err(DurableError::Corrupt(
                "navigation event cursor exceeds source head",
            ));
        }
        if cursor.sequence == source_head_sequence {
            return Ok(DurableNavigationEventSeedV1::UpToDate {
                cursor_sequence: cursor.sequence,
                source_head_sequence,
            });
        }
        let event_sequence = cursor
            .sequence
            .checked_add(1)
            .ok_or(DurableError::Corrupt("navigation event cursor overflow"))?;
        let witness =
            resolve_event_witness(&mut self.client, domain, event_sequence, profile, limits)?;
        let pending = |reason| DurableNavigationEventSeedV1::Pending {
            cursor_sequence: cursor.sequence,
            source_head_sequence: witness.source_head_sequence,
            event_sequence,
            reason,
        };
        if witness.event.event_kind != "command" {
            return Ok(pending(
                DurableNavigationEventPendingReasonV2::UnsupportedEventKind,
            ));
        }
        if witness.members.is_empty() || witness.members.iter().all(|member| !member.is_source) {
            return Ok(pending(
                DurableNavigationEventPendingReasonV2::NoNavigationEffectProof,
            ));
        }
        if event_sequence != witness.source_head_sequence {
            return Ok(pending(
                DurableNavigationEventPendingReasonV2::EventNotCurrentSourceHead,
            ));
        }
        let requested_paths = [event_path.to_owned()];
        let context = self.read_navigation_event_sources_v1(
            store,
            domain,
            consumer_id,
            event_sequence,
            event_path,
            &requested_paths,
            profile,
            limits,
        )?;
        if context.consumer_cursor.sequence != cursor.sequence
            || context.event_id != witness.event_id
            || context.event_sequence != event_sequence
            || context.source_head_sequence != witness.source_head_sequence
        {
            return Err(DurableError::Conflict(
                "navigation event seed differs from its exact current cut",
            ));
        }
        let event_source = context
            .selected_sources
            .iter()
            .find(|source| source.source_path == event_path)
            .ok_or(DurableError::Conflict(
                "navigation event seed bytes are not selected",
            ))?;
        if event_source.exact_source_bytes.is_none() {
            return Err(DurableError::Conflict(
                "navigation event seed source bytes are absent",
            ));
        }
        Ok(DurableNavigationEventSeedV1::Ready(context))
    }

    pub fn read_navigation_current_event_sources_v1(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        event_sequence: u64,
        event_path: &str,
        requested_paths: &[String],
        profile: &DurableProjectionProfile,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventContextV1> {
        let context = self.read_navigation_event_sources_v1(
            store,
            domain,
            consumer_id,
            event_sequence,
            event_path,
            requested_paths,
            profile,
            limits,
        )?;
        if context.event_sequence != context.source_head_sequence {
            return Err(DurableError::Conflict(
                "navigation event is not the current source head",
            ));
        }
        Ok(context)
    }

    /// Resolve an exact committed event and a bounded set of source versions
    /// at its CMD history cut. An empty event path is allowed for memberless
    /// rights/rule events whose dependency cut still needs to be selected.
    /// This is producer context, not a source receipt.
    pub fn read_navigation_event_sources_v1(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        event_sequence: u64,
        event_path: &str,
        requested_paths: &[String],
        profile: &DurableProjectionProfile,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventContextV1> {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation event domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation event consumer ID",
        )?;
        validate_identifier(
            &profile.id,
            super::MAX_PROFILE_FIELD_BYTES,
            "projection profile ID",
        )?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        if event_sequence == 0 {
            return Err(DurableError::Invalid("navigation event sequence"));
        }
        if !event_path.is_empty() {
            validate_relative_path(event_path)?;
        }
        let deadline = Instant::now() + limits.statement_timeout;
        let witness =
            resolve_event_witness(&mut self.client, domain, event_sequence, profile, limits)?;
        let consumer_cursor =
            read_navigation_event_cursor(&mut self.client, domain, consumer_id, profile, limits)?;
        let mut paths = BTreeSet::new();
        for path in requested_paths {
            validate_relative_path(path)?;
            if !paths.insert(path.clone()) {
                return Err(DurableError::Invalid(
                    "duplicate navigation event source path",
                ));
            }
        }
        if paths.len() > limits.max_dependencies {
            return Err(DurableError::Refused(
                "navigation event selected source path budget",
            ));
        }
        if !event_path.is_empty() && !paths.contains(event_path) {
            return Err(DurableError::Invalid(
                "navigation selected sources omit the event JSONL path",
            ));
        }
        let (mut selected_sources, source_head_sequence) = read_selected_sources_at_cut(
            self,
            store,
            domain,
            &witness,
            paths.into_iter().collect(),
            profile,
            limits,
            event_path,
            deadline,
        )?;
        let member_paths = witness
            .members
            .iter()
            .map(|member| member.source_path.clone())
            .collect::<Vec<_>>();
        let member_by_path = selected_sources
            .iter()
            .cloned()
            .map(|source| (source.source_path.clone(), source))
            .collect::<BTreeMap<_, _>>();
        let mut members = Vec::with_capacity(witness.members.len());
        for path in member_paths {
            let source = member_by_path.get(&path).ok_or(DurableError::Corrupt(
                "navigation event member missing from selected cut",
            ))?;
            members.push(source.clone());
        }
        active_navigation(deadline)?;
        selected_sources.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        members.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        Ok(DurableNavigationEventContextV1 {
            event_id: witness.event_id,
            event_sequence,
            event_kind: witness.event.event_kind,
            source_head_sequence,
            consumer_cursor,
            members,
            selected_sources,
        })
    }

    /// Read the persisted owners that currently depend on any exact source
    /// member in this event. The bounded query mirrors the final publisher's
    /// closure check; publication rechecks it inside the atomic write.
    pub fn read_navigation_event_affected_owners_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        context: &DurableNavigationEventContextV1,
        changed_source_paths: &[String],
        profile: &DurableProjectionProfile,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Vec<DurableNavigationEventAffectedOwnerV1>> {
        let limits = limits.validate()?;
        validate_identifier(
            domain,
            super::MAX_DOMAIN_BYTES,
            "navigation affected domain",
        )?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation affected consumer ID",
        )?;
        profile.validate()?;
        if context.event_kind != "command"
            || context.event_id.is_empty()
            || context.event_sequence == 0
            || context.event_sequence != context.source_head_sequence
            || changed_source_paths.is_empty()
        {
            return Err(DurableError::Invalid(
                "navigation affected-owner event context",
            ));
        }
        if changed_source_paths.len() > limits.max_dependencies {
            return Err(DurableError::Refused(
                "navigation changed source path budget",
            ));
        }
        let mut changed = BTreeSet::new();
        for path in changed_source_paths {
            validate_relative_path(path)?;
            if !changed.insert(path.as_str()) {
                return Err(DurableError::Invalid(
                    "duplicate navigation changed source path",
                ));
            }
            let member = context
                .members
                .iter()
                .find(|member| member.source_path == *path)
                .ok_or(DurableError::Conflict(
                    "navigation changed path is absent from exact event members",
                ))?;
            let selected = context
                .selected_sources
                .iter()
                .find(|source| source.source_path == *path)
                .ok_or(DurableError::Conflict(
                    "navigation changed path is absent from exact source cut",
                ))?;
            let exact_bytes =
                selected
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation changed member bytes are absent",
                    ))?;
            if !member.is_source
                || member.commit_sequence != Some(context.event_sequence)
                || selected.commit_sequence != member.commit_sequence
                || selected.revision != member.revision
                || selected.content_digest != member.content_digest
                || selected.content_length != member.content_length
                || selected.content_digest != Some(Digest256::of_bytes(exact_bytes))
                || selected.content_length != Some(exact_bytes.len() as u64)
            {
                return Err(DurableError::Conflict(
                    "navigation changed bytes differ from committed event member",
                ));
            }
        }
        let changed_paths = changed.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let max_owners = limits.max_current_cohorts.min(limits.max_current_rows);
        let owner_limit = i64::try_from(max_owners.saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation reverse-owner count"))?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        let head = snapshot.head_seq()?;
        if head != context.source_head_sequence {
            tx.commit()?;
            return Err(DurableError::Conflict(
                "navigation affected-owner cut is no longer current",
            ));
        }
        verify_navigation_consumer(&mut tx, domain, consumer_id, profile)?;
        let cursor_sequence = tx
            .query_opt(
                "SELECT cursor_seq FROM cmd2_projection_navigation_consumer
                  WHERE domain=$1 AND consumer_id=$2",
                &[&domain, &consumer_id],
            )?
            .map(|row| as_u64(row.get("cursor_seq")))
            .transpose()?
            .ok_or(DurableError::Invalid(
                "navigation affected-owner consumer is absent",
            ))?;
        if cursor_sequence != context.consumer_cursor.sequence {
            tx.commit()?;
            return Err(DurableError::Conflict(
                "navigation affected-owner consumer cursor changed",
            ));
        }
        let rows = tx.query(
            "SELECT owner_source_path,owner_record_id,
                    CASE WHEN owner_record_id LIKE 'tos.claim.%' OR EXISTS(
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=$1 AND c.consumer_id=$2
                         AND c.source_path=affected.owner_source_path
                         AND c.record_id=affected.owner_record_id
                         AND c.basis_kind='claim-mutation'
                    ) THEN 'claim' ELSE 'record' END AS owner_kind
             FROM (
               SELECT DISTINCT owner_source_path,owner_record_id
                 FROM cmd2_projection_navigation_bootstrap_row
                WHERE domain=$1 AND consumer_id=$2 AND active
                  AND owner_source_path=ANY($3)
               UNION
               SELECT DISTINCT b.owner_source_path,b.owner_record_id
                 FROM cmd2_projection_navigation_bootstrap_dependency b
                WHERE b.domain=$1 AND b.consumer_id=$2 AND b.dependency_path=ANY($3)
                  AND EXISTS(
                    SELECT 1 FROM cmd2_projection_navigation_bootstrap_row r
                     WHERE r.domain=b.domain AND r.consumer_id=b.consumer_id AND r.active
                       AND r.owner_source_path=b.owner_source_path
                       AND r.owner_record_id=b.owner_record_id
                  )
               UNION
               SELECT DISTINCT source_path,record_id
                 FROM cmd2_projection_navigation_cohort
                WHERE domain=$1 AND consumer_id=$2 AND source_path=ANY($3)
               UNION
               SELECT DISTINCT source_path,record_id
                 FROM cmd2_projection_navigation_dependency
                WHERE domain=$1 AND consumer_id=$2 AND dependency_path=ANY($3)
             ) affected
             ORDER BY owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"
             LIMIT $4",
            &[&domain, &consumer_id, &changed_paths, &owner_limit],
        )?;
        if rows.len() > max_owners {
            tx.commit()?;
            return Err(DurableError::Refused(
                "navigation reverse-owner count budget",
            ));
        }
        let mut owners = Vec::with_capacity(rows.len());
        let mut owner_paths = Vec::with_capacity(rows.len());
        let mut owner_ids = Vec::with_capacity(rows.len());
        let mut previous_key = None;
        for row in rows {
            let source_path: String = row.get("owner_source_path");
            let record_id: String = row.get("owner_record_id");
            validate_relative_path(&source_path)?;
            validate_identifier(
                &record_id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation affected owner ID",
            )?;
            let key = (source_path.clone(), record_id.clone());
            if previous_key
                .as_ref()
                .is_some_and(|previous| previous >= &key)
            {
                tx.commit()?;
                return Err(DurableError::Corrupt(
                    "navigation affected owners are not uniquely ordered",
                ));
            }
            previous_key = Some(key);
            let owner_kind: String = row.get("owner_kind");
            let kind = match owner_kind.as_str() {
                "record" => DurableNavigationEventAffectedOwnerKindV1::Record,
                "claim" => DurableNavigationEventAffectedOwnerKindV1::Claim,
                _ => {
                    tx.commit()?;
                    return Err(DurableError::Corrupt("navigation affected owner kind"));
                }
            };
            owner_paths.push(source_path.clone());
            owner_ids.push(record_id.clone());
            owners.push(DurableNavigationEventAffectedOwnerV1 {
                source_path,
                record_id,
                kind,
            });
        }
        let unsupported_rights: bool = tx
            .query_one(
                "SELECT EXISTS(
                   SELECT 1 FROM cmd2_projection_navigation_bootstrap_row b
                    WHERE b.domain=$1 AND b.consumer_id=$2 AND b.row_kind='right'
                      AND EXISTS(
                        SELECT 1 FROM unnest($3::text[],$4::text[]) affected(path,id)
                         WHERE affected.path=b.owner_source_path
                           AND affected.id=b.owner_record_id
                      )
                 )",
                &[&domain, &consumer_id, &owner_paths, &owner_ids],
            )?
            .get(0);
        if unsupported_rights {
            tx.commit()?;
            return Err(DurableError::Conflict(
                "navigation affected-owner set contains unsupported rights rows",
            ));
        }
        tx.commit()?;
        Ok(owners)
    }

    /// Advance the durable navigation cursor by one bounded outbox entry.
    /// Command entries are compiled from exact retained event-cut bytes and
    /// committed with the existing CAS. Events without an owner-proven
    /// navigation outcome remain pending and do not move the cursor. Each
    /// successful step persists its cursor so a later call or process restart
    /// resumes at the next sequence.
    pub fn advance_next_navigation_event_v2<F>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        event_path: &str,
        requested_paths: &[String],
        limits: DurableNavigationEventLimits,
        compile: F,
    ) -> DurableResult<DurableNavigationEventAdvanceV2>
    where
        F: FnOnce(
            &DurableNavigationEventContextV1,
        ) -> DurableResult<DurableNavigationEventCohortSetV2>,
    {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation event domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation event consumer ID",
        )?;
        validate_relative_path(event_path)?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let cursor =
            read_navigation_event_cursor(&mut self.client, domain, consumer_id, profile, limits)?;
        let source_head_sequence =
            read_navigation_event_head(&mut self.client, domain, profile, limits)?;
        if cursor.sequence > source_head_sequence {
            return Err(DurableError::Corrupt(
                "navigation event cursor exceeds source head",
            ));
        }
        if cursor.sequence == source_head_sequence {
            return Ok(DurableNavigationEventAdvanceV2::UpToDate {
                cursor_sequence: cursor.sequence,
                source_head_sequence,
            });
        }
        let event_sequence = cursor.sequence.checked_add(1).ok_or(DurableError::Corrupt(
            "navigation event cursor sequence overflow",
        ))?;
        let witness =
            resolve_event_witness(&mut self.client, domain, event_sequence, profile, limits)?;
        if witness.event.event_kind != "command" {
            return Ok(DurableNavigationEventAdvanceV2::Pending {
                cursor_sequence: cursor.sequence,
                source_head_sequence: witness.source_head_sequence,
                event_sequence,
                reason: DurableNavigationEventPendingReasonV2::UnsupportedEventKind,
            });
        }
        if witness.members.is_empty() || witness.members.iter().all(|member| !member.is_source) {
            return Ok(DurableNavigationEventAdvanceV2::Pending {
                cursor_sequence: cursor.sequence,
                source_head_sequence: witness.source_head_sequence,
                event_sequence,
                reason: DurableNavigationEventPendingReasonV2::NoNavigationEffectProof,
            });
        }
        {
            let context = self.read_navigation_event_sources_v1(
                store,
                domain,
                consumer_id,
                event_sequence,
                event_path,
                requested_paths,
                profile,
                limits,
            )?;
            if context.consumer_cursor.sequence != cursor.sequence
                || context.event_id != witness.event_id
                || context.event_sequence != event_sequence
            {
                return Err(DurableError::Conflict(
                    "navigation event cursor changed during source selection",
                ));
            }
            let cohort_set = compile(&context)?;
            if cohort_set.event_sequence != event_sequence
                || cohort_set.event_id != context.event_id
                || cohort_set.member_root
                    != navigation_event_selected_member_root(&context.members)?
            {
                return Err(DurableError::Conflict(
                    "navigation compiler output differs from selected event cut",
                ));
            }
            let apply = self.publish_navigation_event_cohort_set_v2(
                store,
                domain,
                consumer_id,
                profile,
                &cohort_set,
                limits,
            )?;
            return Ok(DurableNavigationEventAdvanceV2::Published {
                cursor_before: cursor.sequence,
                source_head_sequence: context.source_head_sequence,
                apply,
            });
        }
    }

    /// Drain at most `max_events` committed CMD entries in sequence. Each
    /// entry uses the existing single-event publisher and commits its cursor
    /// separately; the next iteration reloads that durable cursor. A pending
    /// or unsupported event stops the drain without advancing past it.
    pub fn advance_navigation_event_backlog_v2<F>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        event_path: &str,
        requested_paths: &[String],
        limits: DurableNavigationEventLimits,
        max_events: usize,
        mut compile: F,
    ) -> DurableResult<DurableNavigationEventBacklogV2>
    where
        F: FnMut(
            &DurableNavigationEventContextV1,
        ) -> DurableResult<DurableNavigationEventCohortSetV2>,
    {
        let limits = limits.validate()?;
        if max_events == 0 || max_events > MAX_NAVIGATION_EVENT_STEPS_PER_CALL {
            return Err(DurableError::Invalid(
                "navigation backlog event-step budget",
            ));
        }
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation event domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation event consumer ID",
        )?;
        profile.validate()?;
        let cursor_before =
            read_navigation_event_cursor(&mut self.client, domain, consumer_id, profile, limits)?
                .sequence;
        let mut cursor_sequence = cursor_before;
        let mut published = Vec::new();
        for _ in 0..max_events {
            match self.advance_next_navigation_event_v2(
                store,
                domain,
                consumer_id,
                profile,
                event_path,
                requested_paths,
                limits,
                |context| compile(context),
            )? {
                DurableNavigationEventAdvanceV2::UpToDate {
                    cursor_sequence: current_cursor,
                    source_head_sequence: current_head,
                } => {
                    cursor_sequence = current_cursor;
                    return Ok(DurableNavigationEventBacklogV2 {
                        cursor_before,
                        cursor_sequence,
                        source_head_sequence: current_head,
                        published,
                        stop: DurableNavigationEventBacklogStopV2::UpToDate,
                    });
                }
                DurableNavigationEventAdvanceV2::Published { apply, .. } => {
                    cursor_sequence = apply.event_sequence;
                    published.push(apply);
                }
                DurableNavigationEventAdvanceV2::Pending {
                    cursor_sequence: current_cursor,
                    source_head_sequence: current_head,
                    event_sequence,
                    reason,
                } => {
                    return Ok(DurableNavigationEventBacklogV2 {
                        cursor_before,
                        cursor_sequence: current_cursor,
                        source_head_sequence: current_head,
                        published,
                        stop: DurableNavigationEventBacklogStopV2::Pending {
                            event_sequence,
                            reason,
                        },
                    });
                }
            }
        }
        let source_head_sequence =
            read_navigation_event_head(&mut self.client, domain, profile, limits)?;
        Ok(DurableNavigationEventBacklogV2 {
            cursor_before,
            cursor_sequence,
            source_head_sequence,
            published,
            stop: DurableNavigationEventBacklogStopV2::StepBudgetReached,
        })
    }

    /// Persist a complete record-local event contribution. The exact CMD
    /// event, changed record path, dependency closure, all roots, predecessor
    /// contribution, and cursor are checked before the transaction commits.
    pub fn publish_navigation_event_cohort_v1(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        cohort: &NavigationRecordEventCohortV1,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventApply> {
        self.publish_navigation_event_cohort_with_semantics(
            store,
            domain,
            consumer_id,
            profile,
            cohort,
            limits,
            false,
            None,
        )
    }

    /// Publish a cohort prepared from exact retained bytes at its event cut.
    /// Unlike V1, this accepts a historical event when it is still the next
    /// cursor entry, while rechecking current rights/profile and all event-cut
    /// dependencies before the atomic row/cursor commit.
    pub fn publish_navigation_event_cohort_v2(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        cohort: &NavigationRecordEventCohortV1,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventApply> {
        self.publish_navigation_event_cohort_with_semantics(
            store,
            domain,
            consumer_id,
            profile,
            cohort,
            limits,
            true,
            None,
        )
    }

    /// Publish one complete compiler-sealed event cohort set. Multi-owner
    /// replacements share one bounded transaction and one final adjacency
    /// rebuild; exact shared rows are deduplicated while conflicting rows
    /// refuse the entire set.
    pub fn publish_navigation_event_cohort_set_v2(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        cohort_set: &DurableNavigationEventCohortSetV2,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventApply> {
        if cohort_set.is_claim_set()
            || cohort_set.record_cohorts.len() > 1
            || cohort_set.record_cohorts.iter().any(|cohort| {
                matches!(
                    cohort.basis(),
                    NavigationRecordEventBasisV1::DependencyRefresh { .. }
                        | NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
                )
            })
        {
            return self.publish_navigation_event_cohort_set_batch_v2(
                store,
                domain,
                consumer_id,
                profile,
                cohort_set,
                limits,
            );
        }
        let Some(cohort) = cohort_set.record_cohorts.first() else {
            return Err(DurableError::Invalid("navigation cohort set is empty"));
        };
        self.publish_navigation_event_cohort_with_semantics(
            store,
            domain,
            consumer_id,
            profile,
            cohort,
            limits,
            true,
            Some(cohort_set),
        )
    }

    fn publish_navigation_event_cohort_set_batch_v2(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        cohort_set: &DurableNavigationEventCohortSetV2,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventApply> {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation event domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation event consumer ID",
        )?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let owner_count = cohort_set
            .record_cohorts
            .len()
            .checked_add(cohort_set.claim_cohorts.len())
            .and_then(|count| count.checked_add(cohort_set.claim_refresh_cohorts.len()))
            .ok_or(DurableError::Refused("navigation atomic owner count"))?;
        if owner_count == 0
            || owner_count > limits.max_current_cohorts
            || cohort_set.manifest_raw.len() > limits.max_row_bytes as usize
        {
            return Err(DurableError::Refused(
                "navigation atomic cohort-set size budget",
            ));
        }
        if cohort_set.is_claim_set() && cohort_set.claim_navigation_closure_root.is_none() {
            return Err(DurableError::Conflict(
                "navigation Claim publication lacks an authenticated reverse-reference closure",
            ));
        }

        let mut dependencies = BTreeMap::<String, ()>::new();
        let mut event_paths = BTreeSet::new();
        for dependency in cohort_set
            .record_cohorts
            .iter()
            .flat_map(|cohort| cohort.dependencies.iter())
            .chain(
                cohort_set
                    .claim_cohorts
                    .iter()
                    .flat_map(|cohort| cohort.dependencies().iter()),
            )
            .chain(
                cohort_set
                    .claim_refresh_cohorts
                    .iter()
                    .flat_map(|cohort| cohort.dependencies().iter()),
            )
        {
            dependencies.insert(dependency.path.as_str().to_owned(), ());
            if dependency.role == NavigationDependencyRoleV1::Event {
                event_paths.insert(dependency.path.as_str().to_owned());
            }
        }
        if event_paths.len() != 1 {
            return Err(DurableError::Conflict(
                "navigation cohort set has ambiguous event source path",
            ));
        }
        let event_path = event_paths.into_iter().next().ok_or(DurableError::Invalid(
            "navigation cohort set event path absent",
        ))?;
        let requested_paths = dependencies.into_keys().collect::<Vec<_>>();
        let context = self.read_navigation_event_sources_v1(
            store,
            domain,
            consumer_id,
            cohort_set.event_sequence,
            &event_path,
            &requested_paths,
            profile,
            limits,
        )?;
        if context.event_id != cohort_set.event_id
            || context.event_sequence != cohort_set.event_sequence
            || navigation_event_selected_member_root(&context.members)? != cohort_set.member_root
        {
            return Err(DurableError::Conflict(
                "navigation cohort set differs from the selected event cut",
            ));
        }

        let has_refresh = cohort_set.record_cohorts.iter().any(|cohort| {
            matches!(
                cohort.basis(),
                NavigationRecordEventBasisV1::DependencyRefresh { .. }
                    | NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
            )
        });
        let canonical_manifest =
            if let Some(closure_root) = cohort_set.claim_navigation_closure_root {
                navigation_mixed_event_cohort_manifest_v5(
                    &context.event_id,
                    context.event_sequence,
                    cohort_set.member_root,
                    &cohort_set.record_cohorts,
                    &cohort_set.claim_cohorts,
                    &cohort_set.claim_refresh_cohorts,
                    cohort_set.claim_binding.as_ref(),
                    cohort_set.claim_refresh_binding.as_ref(),
                    cohort_set.claim_empty_closure_binding.as_ref(),
                    closure_root,
                    limits.max_row_bytes as usize,
                )?
            } else if cohort_set.claim_binding.is_some() || has_refresh {
                navigation_mixed_event_cohort_manifest(
                    &context.event_id,
                    context.event_sequence,
                    cohort_set.member_root,
                    &cohort_set.record_cohorts,
                    &cohort_set.claim_cohorts,
                    cohort_set.claim_binding.as_ref(),
                    limits.max_row_bytes as usize,
                )?
            } else {
                if !cohort_set.claim_cohorts.is_empty() {
                    return Err(DurableError::Conflict(
                        "navigation Claim set lacks its typed current-stream binding",
                    ));
                }
                navigation_event_cohort_manifest(
                    &context.event_id,
                    context.event_sequence,
                    cohort_set.member_root,
                    &cohort_set.record_cohorts,
                    limits.max_row_bytes as usize,
                )?
            };
        if canonical_manifest != cohort_set.manifest_raw
            || Digest256::of_bytes(&canonical_manifest) != cohort_set.set_root
        {
            return Err(DurableError::Conflict(
                "navigation cohort set manifest differs from its sealed members",
            ));
        }

        let member_paths = context
            .members
            .iter()
            .map(|member| member.source_path.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(empty) = &cohort_set.claim_empty_closure_binding {
            let expected_paths = context
                .members
                .iter()
                .filter(|member| member.is_source && member.source_path.starts_with("ToS/"))
                .map(|member| member.source_path.as_str().to_owned())
                .collect::<BTreeSet<_>>();
            let covered_paths = empty
                .covered_source_paths
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if cohort_set.claim_navigation_closure_root != Some(empty.closure_root)
                || cohort_set.claim_binding.is_some()
                || cohort_set.claim_refresh_binding.is_some()
                || !cohort_set.claim_cohorts.is_empty()
                || !cohort_set.claim_refresh_cohorts.is_empty()
                || expected_paths.is_empty()
                || expected_paths
                    .iter()
                    .any(|path| path.ends_with("/source-claims.jsonl"))
                || covered_paths != expected_paths
            {
                return Err(DurableError::Conflict(
                    "navigation empty Claim closure differs from exact event paths",
                ));
            }
            for path in &empty.covered_source_paths {
                let member = context
                    .members
                    .iter()
                    .find(|member| member.source_path == *path)
                    .ok_or(DurableError::Conflict(
                        "navigation empty Claim path is absent from event members",
                    ))?;
                let selected = context
                    .selected_sources
                    .iter()
                    .find(|source| source.source_path == *path)
                    .ok_or(DurableError::Conflict(
                        "navigation empty Claim path bytes are absent from selected cut",
                    ))?;
                let raw = selected
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation empty Claim source bytes are absent",
                    ))?;
                if !member.is_source
                    || member.commit_sequence != Some(context.event_sequence)
                    || !selected.is_source
                    || selected.commit_sequence != Some(context.event_sequence)
                    || member.content_digest != Some(Digest256::of_bytes(raw))
                    || member.content_length != Some(raw.len() as u64)
                    || selected.content_digest != member.content_digest
                    || selected.content_length != member.content_length
                {
                    return Err(DurableError::Conflict(
                        "navigation empty Claim path differs from exact event bytes",
                    ));
                }
            }
        }
        let mut dependency_paths = BTreeSet::new();
        let mut owner_keys = BTreeSet::new();
        let mut owners_and_rows = Vec::<(NavigationEventOwnerV2, PersistedRows)>::new();
        let mut total_rows = 0usize;
        let mut total_bytes = 0u64;
        let mut common_source_revision = None;
        let mut common_predecessor_revision = None;

        let mut account_rows = |rows: &PersistedRows| -> DurableResult<()> {
            total_rows = total_rows
                .checked_add(rows.nodes.len())
                .and_then(|value| value.checked_add(rows.edges.len()))
                .and_then(|value| value.checked_add(rows.diagnostics.len()))
                .and_then(|value| value.checked_add(rows.dependencies.len()))
                .ok_or(DurableError::Refused("navigation cohort-set row count"))?;
            total_bytes = total_bytes
                .checked_add(rows.cohort_bytes)
                .ok_or(DurableError::Refused("navigation cohort-set byte count"))?;
            if total_rows > limits.max_current_rows || total_bytes > limits.max_total_bytes {
                return Err(DurableError::Refused(
                    "navigation cohort-set aggregate row or byte budget",
                ));
            }
            Ok(())
        };

        for cohort in &cohort_set.record_cohorts {
            if cohort.event_id != context.event_id
                || cohort.event_sequence != context.event_sequence
            {
                return Err(DurableError::Conflict(
                    "navigation Record cohort event differs from selected cut",
                ));
            }
            common_source_revision.get_or_insert(cohort.source_revision);
            common_predecessor_revision.get_or_insert(cohort.predecessor_source_revision);
            if common_source_revision != Some(cohort.source_revision)
                || common_predecessor_revision != Some(cohort.predecessor_source_revision)
                || !owner_keys.insert((
                    cohort.source_path.as_str().to_owned(),
                    cohort.record_id.clone(),
                ))
            {
                return Err(DurableError::Conflict(
                    "navigation mixed set repeats an owner or source revision",
                ));
            }
            dependency_paths.extend(
                cohort
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.path.as_str()),
            );
            let rows = validate_cohort(cohort, limits)?;
            verify_dependency_cut(cohort, &context.selected_sources, limits)?;
            verify_event_manifest(cohort, &context.selected_sources, limits)?;
            let (dependency_root, output_root, input_root) = cohort_roots(cohort, limits)?;
            if dependency_root != cohort.dependency_root
                || output_root != cohort.output_root
                || input_root != cohort.input_root
            {
                return Err(DurableError::Conflict(
                    "navigation Record cohort root differs from selected inputs",
                ));
            }
            account_rows(&rows)?;
            owners_and_rows.push((
                NavigationEventOwnerV2::from_record(cohort, limits.max_row_bytes as usize)?,
                rows,
            ));
        }

        if let Some(binding) = &cohort_set.claim_binding {
            let receipt_member = context
                .members
                .iter()
                .find(|member| member.source_path == binding.mutation_receipt_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim receipt is absent from committed members",
                ))?;
            let stream_member = context
                .members
                .iter()
                .find(|member| member.source_path == binding.claim_stream_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim stream is absent from committed members",
                ))?;
            if !receipt_member.is_source
                || receipt_member.commit_sequence != Some(context.event_sequence)
                || !stream_member.is_source
                || stream_member.commit_sequence != Some(context.event_sequence)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim receipt or stream is outside the event",
                ));
            }
            let receipt = context
                .selected_sources
                .iter()
                .find(|source| source.source_path == binding.mutation_receipt_path)
                .ok_or(DurableError::Conflict(
                    "navigation Claim receipt bytes are absent from selected cut",
                ))?;
            let receipt_raw =
                receipt
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Claim receipt bytes are absent",
                    ))?;
            let receipt_digest = receipt.content_digest.ok_or(DurableError::Corrupt(
                "navigation Claim receipt digest absent",
            ))?;
            let receipt_length = receipt.content_length.ok_or(DurableError::Corrupt(
                "navigation Claim receipt length absent",
            ))?;
            let mutation_receipt = navigation_claim_ids_from_mutation_receipt(
                receipt_raw,
                &binding.mutation_source_path,
                &binding.mutation_receipt_path,
                limits.max_row_bytes,
            )?;
            if receipt.commit_sequence != Some(context.event_sequence)
                || Some(receipt_digest) != receipt_member.content_digest
                || Some(receipt_length) != receipt_member.content_length
                || receipt_digest != binding.mutation_receipt_sha256
                || Digest256::of_bytes(receipt_raw) != receipt_digest
                || mutation_receipt.claim_ids != binding.mutation_claim_ids
                || mutation_receipt.revision_receipt_sha256.is_some()
                    != binding.revision_proof_root.is_some()
                || binding.revision_claim_id.as_ref().is_some_and(|claim_id| {
                    mutation_receipt.claim_ids.len() != 1
                        || mutation_receipt.claim_ids.first().map(String::as_str)
                            != Some(claim_id.as_str())
                })
            {
                return Err(DurableError::Conflict(
                    "navigation Claim receipt differs from its sealed mutation set",
                ));
            }
            if binding.claim_stream_path != binding.mutation_source_path {
                return Err(DurableError::Conflict(
                    "navigation Claim mutation stream path differs from its event binding",
                ));
            }
            verify_selected_claim_stream_bindings(
                &context,
                &binding.streams,
                context.event_sequence,
                Some(&binding.mutation_source_path),
                limits,
            )?;
            let stream_owner_ids = binding
                .streams
                .iter()
                .flat_map(|stream| stream.owner_claim_ids.iter().cloned())
                .collect::<BTreeSet<_>>();
            if stream_owner_ids.len() != binding.owner_claim_ids.len()
                || stream_owner_ids
                    != binding
                        .owner_claim_ids
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>()
                || binding
                    .streams
                    .iter()
                    .find(|stream| stream.source_path == binding.mutation_source_path)
                    .map(|stream| stream.stream_sha256)
                    != Some(binding.claim_stream_sha256)
                || stream_member.content_digest != Some(binding.claim_stream_sha256)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim owner streams differ from their complete set binding",
                ));
            }
            if cohort_set.claim_cohorts.is_empty()
                || cohort_set.claim_cohorts.len() != binding.owner_claim_ids.len()
            {
                return Err(DurableError::Conflict(
                    "navigation Claim set does not contain every current owner",
                ));
            }
            for cohort in &cohort_set.claim_cohorts {
                let integrity = cohort
                    .verify_integrity()
                    .map_err(|_| DurableError::Conflict("navigation Claim integrity differs"))?;
                let stream = binding
                    .streams
                    .iter()
                    .find(|stream| stream.source_path == cohort.source_path().as_str())
                    .ok_or(DurableError::Conflict(
                        "navigation Claim cohort stream is absent from its owner manifest",
                    ))?;
                common_source_revision.get_or_insert(cohort.source_revision());
                common_predecessor_revision.get_or_insert(cohort.predecessor_source_revision());
                if cohort.event_id() != context.event_id
                    || cohort.event_sequence() != context.event_sequence
                    || Some(cohort.source_revision()) != common_source_revision
                    || Some(cohort.predecessor_source_revision()) != common_predecessor_revision
                    || cohort.mutation_receipt_path().as_str() != binding.mutation_receipt_path
                    || cohort.mutation_source_path().as_str() != binding.mutation_source_path
                    || integrity.event_mutation_claim_ids != binding.mutation_claim_ids.as_slice()
                    || integrity.event_claim_owner_ids != stream.owner_claim_ids.as_slice()
                    || integrity.claim_stream_sha256 != stream.stream_sha256
                    || integrity.claim_stream_rows_scanned != stream.rows_scanned
                    || integrity.event_claim_set_binding != binding.mutation_receipt_sha256
                    || integrity.event_claim_set_root != binding.event_claim_set_root
                    || integrity.affected_owner_manifest_root
                        != binding
                            .affected_owner_manifest_root
                            .ok_or(DurableError::Conflict(
                                "navigation Claim affected-owner root is absent",
                            ))?
                    || integrity.owner_inventory_root != binding.owner_inventory_root
                    || !owner_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim cohort differs from the complete event set",
                    ));
                }
                dependency_paths.extend(
                    cohort
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.path.as_str()),
                );
                let rows = validate_navigation_claim_cohort(cohort, limits)?;
                verify_navigation_claim_dependency_cut(cohort, &context.selected_sources, limits)?;
                account_rows(&rows)?;
                owners_and_rows.push((
                    NavigationEventOwnerV2::from_claim(
                        cohort,
                        &integrity,
                        binding
                            .affected_owner_manifest_root
                            .ok_or(DurableError::Conflict(
                                "navigation Claim affected-owner root is absent",
                            ))?,
                    ),
                    rows,
                ));
            }
            let claim_owner_keys = cohort_set
                .claim_cohorts
                .iter()
                .map(|cohort| {
                    (
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    )
                })
                .collect::<BTreeSet<_>>();
            let expected_owner_keys = binding
                .streams
                .iter()
                .flat_map(|stream| {
                    stream
                        .owner_claim_ids
                        .iter()
                        .map(move |id| (stream.source_path.clone(), id.clone()))
                })
                .collect::<BTreeSet<_>>();
            if claim_owner_keys != expected_owner_keys
                || binding
                    .mutation_claim_ids
                    .iter()
                    .any(|id| !binding.owner_claim_ids.contains(id))
            {
                return Err(DurableError::Conflict(
                    "navigation Claim owner inventory is incomplete",
                ));
            }
        } else if let Some(binding) = &cohort_set.claim_refresh_binding {
            if cohort_set.claim_refresh_cohorts.is_empty()
                && (!binding.streams.is_empty() || !binding.reverse_claim_ids.is_empty())
                || binding.closure_root
                    != cohort_set
                        .claim_navigation_closure_root
                        .ok_or(DurableError::Conflict(
                            "navigation Claim refresh closure root is absent",
                        ))?
            {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh binding differs from its sealed closure",
                ));
            }
            verify_selected_claim_stream_bindings(
                &context,
                &binding.streams,
                context.event_sequence,
                None,
                limits,
            )?;
            let stream_owner_ids = binding
                .streams
                .iter()
                .flat_map(|stream| stream.owner_claim_ids.iter().cloned())
                .collect::<BTreeSet<_>>();
            if binding.reverse_claim_ids.is_empty() != binding.streams.is_empty()
                || binding
                    .reverse_claim_ids
                    .iter()
                    .any(|id| !stream_owner_ids.contains(id))
            {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh streams differ from reverse owner IDs",
                ));
            }
            for cause in &binding.causes {
                let member = context
                    .members
                    .iter()
                    .find(|member| member.source_path == cause.changed_dependency_path)
                    .ok_or(DurableError::Conflict(
                        "navigation Claim refresh cause is absent from committed members",
                    ))?;
                let selected = context
                    .selected_sources
                    .iter()
                    .find(|source| source.source_path == cause.changed_dependency_path)
                    .ok_or(DurableError::Conflict(
                        "navigation Claim refresh cause bytes are absent",
                    ))?;
                if !member.is_source
                    || member.commit_sequence != Some(context.event_sequence)
                    || !selected.is_source
                    || selected.commit_sequence != Some(context.event_sequence)
                    || selected.content_digest != Some(cause.changed_dependency_sha256)
                    || selected.exact_source_bytes.as_deref().is_none_or(|raw| {
                        Digest256::of_bytes(raw) != cause.changed_dependency_sha256
                    })
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh cause differs from exact event bytes",
                    ));
                }
            }
            let expected_owner_keys = binding
                .streams
                .iter()
                .flat_map(|stream| {
                    stream
                        .owner_claim_ids
                        .iter()
                        .map(move |id| (stream.source_path.clone(), id.clone()))
                })
                .collect::<BTreeSet<_>>();
            let mut actual_owner_keys = BTreeSet::new();
            for cohort in &cohort_set.claim_refresh_cohorts {
                let integrity = cohort.verify_integrity().map_err(|_| {
                    DurableError::Conflict("navigation Claim refresh integrity differs")
                })?;
                common_source_revision.get_or_insert(cohort.source_revision());
                common_predecessor_revision.get_or_insert(cohort.predecessor_source_revision());
                let stream = binding
                    .streams
                    .iter()
                    .find(|stream| stream.source_path == cohort.source_path().as_str())
                    .ok_or(DurableError::Conflict(
                        "navigation Claim refresh cohort stream is absent",
                    ))?;
                if cohort.event_id() != context.event_id
                    || cohort.event_sequence() != context.event_sequence
                    || Some(cohort.source_revision()) != common_source_revision
                    || Some(cohort.predecessor_source_revision()) != common_predecessor_revision
                    || integrity.owner_inventory_root != binding.owner_inventory_root
                    || integrity.proof.closure_root() != binding.closure_root
                    || integrity.proof.affected_owner_manifest_root()
                        != binding.affected_owner_manifest_root
                    || integrity.proof.reverse_claim_ids() != binding.reverse_claim_ids
                    || cohort.event_claim_owner_ids() != stream.owner_claim_ids.as_slice()
                    || cohort.claim_stream_sha256() != stream.stream_sha256
                    || cohort.claim_stream_rows_scanned() != stream.rows_scanned
                    || !actual_owner_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                    || !owner_keys.insert((
                        cohort.source_path().as_str().to_owned(),
                        cohort.record_id().to_owned(),
                    ))
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh cohort differs from its sealed closure",
                    ));
                }
                let proof_causes = integrity.proof.changed_dependencies();
                if proof_causes.len() != binding.causes.len()
                    || proof_causes
                        .iter()
                        .zip(&binding.causes)
                        .any(|(proof, cause)| {
                            proof.changed_dependency_path().as_str()
                                != cause.changed_dependency_path
                                || proof.changed_dependency_sha256()
                                    != cause.changed_dependency_sha256
                                || proof.reverse_claim_ids() != cause.reverse_claim_ids
                        })
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh proof differs from bound causes",
                    ));
                }
                let rows = validate_navigation_claim_refresh_cohort(cohort, limits)?;
                verify_navigation_claim_refresh_dependency_cut(
                    cohort,
                    &context.selected_sources,
                    limits,
                )?;
                if integrity.dependency_root != cohort.dependency_root()
                    || integrity.output_root != cohort.output_root()
                    || integrity.input_root != cohort.input_root()
                    || rows.cohort_bytes > limits.max_total_bytes
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh row roots differ",
                    ));
                }
                dependency_paths.extend(
                    cohort
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.path.as_str()),
                );
                account_rows(&rows)?;
                owners_and_rows.push((
                    NavigationEventOwnerV2::from_claim_refresh(
                        cohort,
                        &integrity,
                        binding.affected_owner_manifest_root,
                    ),
                    rows,
                ));
            }
            if actual_owner_keys != expected_owner_keys
                || binding.causes.is_empty()
                || binding
                    .causes
                    .windows(2)
                    .any(|pair| pair[0].changed_dependency_path >= pair[1].changed_dependency_path)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh set is incomplete or noncanonical",
                ));
            }
        } else if !cohort_set.claim_cohorts.is_empty()
            || !cohort_set.claim_refresh_cohorts.is_empty()
        {
            return Err(DurableError::Conflict(
                "navigation Claim cohorts lack an authenticated owner-set binding",
            ));
        }

        if member_paths
            .iter()
            .any(|member_path| !dependency_paths.contains(member_path))
        {
            return Err(DurableError::Conflict(
                "navigation cohort set omits a committed event member dependency",
            ));
        }
        owners_and_rows.sort_by(|left, right| {
            (left.0.source_path.as_str(), left.0.record_id.as_str())
                .cmp(&(right.0.source_path.as_str(), right.0.record_id.as_str()))
        });
        let witness = resolve_event_witness(
            &mut self.client,
            domain,
            cohort_set.event_sequence,
            profile,
            limits,
        )?;
        if witness.event_id != cohort_set.event_id {
            return Err(DurableError::Conflict(
                "navigation cohort set event differs from committed outbox",
            ));
        }
        let deadline = Instant::now() + limits.statement_timeout;
        active_navigation(deadline)?;
        let (owners, rows): (Vec<_>, Vec<_>) = owners_and_rows.into_iter().unzip();
        self.publish_validated_navigation_cohort_set_batch(
            domain,
            consumer_id,
            profile,
            &owners,
            rows,
            &witness,
            cohort_set,
            limits,
            deadline,
        )
    }

    fn publish_validated_navigation_cohort_set_batch(
        &mut self,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        owners: &[NavigationEventOwnerV2],
        rows: Vec<PersistedRows>,
        witness: &EventWitness,
        cohort_set: &DurableNavigationEventCohortSetV2,
        limits: DurableNavigationEventLimits,
        deadline: Instant,
    ) -> DurableResult<DurableNavigationEventApply> {
        if owners.is_empty() || owners.len() != rows.len() {
            return Err(DurableError::Invalid(
                "navigation atomic batch requires a complete owner set",
            ));
        }
        let sequence = cohort_set.event_sequence;
        let first = owners
            .first()
            .ok_or(DurableError::Invalid("navigation cohort set is empty"))?;
        if owners.iter().any(|owner| {
            owner.event_id != cohort_set.event_id
                || owner.event_sequence != sequence
                || owner.source_revision != first.source_revision
                || owner.predecessor_source_revision != first.predecessor_source_revision
        }) {
            return Err(DurableError::Conflict(
                "navigation atomic batch members disagree on event or source revision",
            ));
        }
        let target_owners = owners
            .iter()
            .map(|owner| {
                (
                    owner.source_path.as_str().to_owned(),
                    owner.record_id.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        if target_owners.len() != owners.len() {
            return Err(DurableError::Conflict(
                "navigation atomic batch repeats an owner",
            ));
        }
        let source_paths = witness
            .members
            .iter()
            .filter(|member| member.is_source)
            .map(|member| member.source_path.clone())
            .collect::<Vec<_>>();
        if source_paths.is_empty() {
            return Err(DurableError::Conflict(
                "navigation atomic batch has no committed source members",
            ));
        }
        active_navigation(deadline)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = lock_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        if witness.event.commit_seq > snapshot.head_seq()? {
            return Err(DurableError::Conflict(
                "navigation event is outside the accepted CMD head",
            ));
        }
        let profile_version = profile.version.to_string();
        let source_contract = profile.source_contract.to_hex();
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_consumer(
                domain,consumer_id,profile_id,profile_version,source_contract_digest)
             VALUES($1,$2,$3,$4,$5) ON CONFLICT(domain,consumer_id) DO NOTHING",
            &[
                &domain,
                &consumer_id,
                &profile.id,
                &profile_version,
                &source_contract,
            ],
        )?;
        let cursor_row = tx.query_one(
            "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,
                    source_revision,last_event_id,last_input_root,last_output_root,
                    coverage_seq,coverage_root,adjacency_seq,adjacency_root,adjacency_ready,
                    verified_prefix_version,verified_prefix_seq,verified_prefix_coverage_root,
                    verified_prefix_cohort_root,verified_prefix_digest
             FROM cmd2_projection_navigation_consumer
             WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?;
        if cursor_row.get::<_, String>("profile_id") != profile.id
            || cursor_row.get::<_, String>("profile_version") != profile_version
            || cursor_row.get::<_, String>("source_contract_digest") != source_contract
        {
            return Err(DurableError::Conflict(
                "navigation event consumer profile differs",
            ));
        }
        let cursor = as_u64(cursor_row.get("cursor_seq"))?;
        if cursor == sequence {
            let stored = tx.query_opt(
                "SELECT cohort_set_root,cohort_manifest FROM
                    cmd2_projection_navigation_coverage_event
                  WHERE domain=$1 AND consumer_id=$2 AND event_sequence=$3",
                &[&domain, &consumer_id, &as_i64(sequence)?],
            )?;
            let already_applied = stored.is_some_and(|row| {
                row.get::<_, Option<String>>("cohort_set_root").as_deref()
                    == Some(cohort_set.set_root.to_hex().as_str())
                    && row.get::<_, Option<Vec<u8>>>("cohort_manifest").as_deref()
                        == Some(cohort_set.manifest_raw.as_slice())
            });
            let last_event_id: Option<String> = cursor_row.get("last_event_id");
            let last_input_root: Option<String> = cursor_row.get("last_input_root");
            let last_output_root: Option<String> = cursor_row.get("last_output_root");
            if already_applied
                && last_event_id.as_deref() == Some(first.event_id.as_str())
                && last_input_root.as_deref() == Some(first.input_root.to_hex().as_str())
                && last_output_root.as_deref() == Some(first.output_root.to_hex().as_str())
                && as_u64(cursor_row.get("coverage_seq"))? == sequence
                && as_u64(cursor_row.get("adjacency_seq"))? == sequence
                && cursor_row.get::<_, bool>("adjacency_ready")
            {
                let bootstrap = tx.query_opt(
                    "SELECT through_seq,bootstrap_root,owner_binding_json
                       FROM cmd2_projection_navigation_bootstrap
                      WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
                    &[&domain, &consumer_id],
                )?;
                let bootstrap = bootstrap.ok_or(DurableError::Conflict(
                    "atomic navigation event batch replay requires a full bootstrap",
                ))?;
                verify_navigation_verified_prefix_in_tx(
                    &mut tx,
                    domain,
                    consumer_id,
                    profile,
                    as_u64(bootstrap.get("through_seq"))?,
                    parse_digest(bootstrap.get("bootstrap_root"))?,
                    &bootstrap.get::<_, String>("owner_binding_json"),
                    &cursor_row,
                    limits,
                )?;
                tx.commit()?;
                return Ok(DurableNavigationEventApply {
                    event_sequence: sequence,
                    event_id: first.event_id.clone(),
                    output_root: first.output_root,
                    replayed: true,
                });
            }
            return Err(DurableError::Conflict(
                "navigation event cursor already owns a different cohort set",
            ));
        }
        if cursor.checked_add(1) != Some(sequence) {
            return Err(DurableError::Conflict(
                "navigation event cursor is not the predecessor",
            ));
        }
        if cursor > 0 {
            let predecessor_revision = cursor_row
                .get::<_, Option<String>>("source_revision")
                .ok_or(DurableError::Corrupt(
                    "navigation event cursor source revision absent",
                ))?;
            if predecessor_revision.trim_end() != first.predecessor_source_revision.to_hex() {
                return Err(DurableError::Conflict(
                    "navigation predecessor source revision differs",
                ));
            }
        } else if sequence != 1
            || cursor_row
                .get::<_, Option<String>>("source_revision")
                .is_some()
        {
            return Err(DurableError::Conflict(
                "navigation event cursor has no trusted predecessor baseline",
            ));
        }
        if owners
            .iter()
            .any(|owner| owner.predecessor_source_revision != first.predecessor_source_revision)
        {
            return Err(DurableError::Conflict(
                "navigation batch predecessor source revisions disagree",
            ));
        }
        let bootstrap = tx.query_opt(
            "SELECT through_seq,bootstrap_root,owner_binding_json,header_raw FROM cmd2_projection_navigation_bootstrap
              WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?;
        let bootstrap = bootstrap.ok_or(DurableError::Conflict(
            "atomic navigation event batch requires a full bootstrap",
        ))?;
        let bootstrap_sequence = as_u64(bootstrap.get("through_seq"))?;
        let bootstrap_root = parse_digest(bootstrap.get("bootstrap_root"))?;
        let coverage_sequence = as_u64(cursor_row.get("coverage_seq"))?;
        let previous_coverage_root = cursor_row
            .get::<_, Option<String>>("coverage_root")
            .ok_or(DurableError::Conflict(
                "navigation coverage root is absent before batch update",
            ))
            .and_then(parse_digest)?;
        let adjacency_sequence = as_u64(cursor_row.get("adjacency_seq"))?;
        let previous_adjacency_root = cursor_row
            .get::<_, Option<String>>("adjacency_root")
            .ok_or(DurableError::Conflict(
                "navigation adjacency root is absent before batch update",
            ))
            .and_then(parse_digest)?;
        if bootstrap_sequence > cursor
            || coverage_sequence != cursor
            || adjacency_sequence != cursor
            || !cursor_row.get::<_, bool>("adjacency_ready")
        {
            return Err(DurableError::Conflict(
                "navigation event cursor lacks complete predecessor coverage",
            ));
        }
        let owner_binding_json: String = bootstrap.get("owner_binding_json");
        verify_navigation_verified_prefix_in_tx(
            &mut tx,
            domain,
            consumer_id,
            profile,
            bootstrap_sequence,
            bootstrap_root,
            &owner_binding_json,
            &cursor_row,
            limits,
        )?;
        let header_raw: String = bootstrap.get("header_raw");
        verify_navigation_batch_final_budget(
            &mut tx,
            domain,
            consumer_id,
            &target_owners,
            &rows,
            header_raw.len() as u64,
            limits,
        )?;
        verify_navigation_reverse_dependency_closure_set(
            &mut tx,
            domain,
            consumer_id,
            &target_owners,
            &source_paths,
            limits,
        )?;
        let replacement_rows = rows.iter().collect::<Vec<_>>();
        let (adjacency_updates, adjacency_root) = prepare_navigation_adjacency_update_batch(
            &mut tx,
            domain,
            consumer_id,
            &target_owners,
            bootstrap_root,
            previous_adjacency_root,
            sequence,
            &replacement_rows,
            limits,
        )?;
        for owner in owners {
            let existing_root = tx.query_opt(
                "SELECT output_root FROM cmd2_projection_navigation_cohort
                  WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4 FOR UPDATE",
                &[
                    &domain,
                    &consumer_id,
                    &owner.source_path.as_str(),
                    &owner.record_id,
                ],
            )?;
            match existing_root {
                Some(row) => {
                    if owner.predecessor_cohort_root != Some(parse_digest(row.get("output_root"))?)
                    {
                        return Err(DurableError::Conflict(
                            "navigation batch predecessor cohort root differs",
                        ));
                    }
                }
                None if owner.predecessor_cohort_root.is_some() => {
                    return Err(DurableError::Conflict(
                        "navigation batch predecessor cohort is absent",
                    ));
                }
                None => {}
            }
        }
        active_navigation(deadline)?;

        for (owner, rows) in owners.iter().zip(&rows) {
            replace_navigation_event_cohort_rows(
                &mut tx,
                domain,
                consumer_id,
                owner,
                rows,
                sequence,
                cohort_set.set_root,
            )?;
        }
        let first = owners
            .first()
            .ok_or(DurableError::Invalid("navigation cohort set is empty"))?;
        let cohort_set_root = Digest256::of_bytes(&cohort_set.manifest_raw);
        let coverage_root = navigation_coverage_root_next(
            previous_coverage_root,
            sequence,
            &first.event_id,
            first.source_path.as_str(),
            &first.record_id,
            first.source_revision,
            first.input_root,
            first.dependency_root,
            first.output_root,
            cohort_set_root,
        );
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_coverage_event(
                domain,consumer_id,event_sequence,event_id,source_path,record_id,
                source_revision,input_root,dependency_root,output_root,
                previous_coverage_root,coverage_root,cohort_count,cohort_set_root,cohort_manifest)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
            &[
                &domain,
                &consumer_id,
                &as_i64(sequence)?,
                &first.event_id,
                &first.source_path.as_str(),
                &first.record_id,
                &first.source_revision.to_hex(),
                &first.input_root.to_hex(),
                &first.dependency_root.to_hex(),
                &first.output_root.to_hex(),
                &previous_coverage_root.to_hex(),
                &coverage_root.to_hex(),
                &i32::try_from(owners.len())
                    .map_err(|_| DurableError::Refused("navigation cohort-set count"))?,
                &cohort_set_root.to_hex(),
                &cohort_set.manifest_raw,
            ],
        )?;
        let affected_ids = adjacency_updates.keys().cloned().collect::<Vec<_>>();
        if !affected_ids.is_empty() {
            tx.execute(
                "DELETE FROM cmd2_projection_navigation_adjacency_certificate
                  WHERE domain=$1 AND consumer_id=$2 AND node_id=ANY($3)",
                &[&domain, &consumer_id, &affected_ids],
            )?;
        }
        for certificate in adjacency_updates.values().flatten() {
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_adjacency_certificate(
                    domain,consumer_id,node_id,edge_count,edge_digest,certificate_root)
                 VALUES($1,$2,$3,$4,$5,$6)",
                &[
                    &domain,
                    &consumer_id,
                    &certificate.node_id,
                    &as_i64(certificate.edge_count)?,
                    &certificate.edge_digest.to_hex(),
                    &certificate.certificate_root.to_hex(),
                ],
            )?;
        }
        active_navigation(deadline)?;
        let cohort_root =
            navigation_current_cohort_root_in_tx(&mut tx, domain, consumer_id, limits)?;
        let owner_binding_json: String = bootstrap.get("owner_binding_json");
        let checkpoint_digest = navigation_verified_prefix_digest(
            domain,
            consumer_id,
            profile,
            bootstrap_sequence,
            bootstrap_root,
            &owner_binding_json,
            sequence,
            coverage_root,
            cohort_root,
            adjacency_root,
            &first.event_id,
            first.source_revision,
            first.input_root,
            first.output_root,
        );
        let changed = tx.execute(
            "UPDATE cmd2_projection_navigation_consumer
                SET cursor_seq=$3,source_revision=$4,last_event_id=$5,
                    last_input_root=$6,last_output_root=$7,coverage_seq=$3,
                    coverage_root=$8,adjacency_seq=$3,adjacency_root=$9,adjacency_ready=true,
                    verified_prefix_version=$13,
                    verified_prefix_seq=$3,verified_prefix_coverage_root=$8,
                    verified_prefix_cohort_root=$10,verified_prefix_digest=$11
              WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=$12",
            &[
                &domain,
                &consumer_id,
                &as_i64(sequence)?,
                &first.source_revision.to_hex(),
                &first.event_id,
                &first.input_root.to_hex(),
                &first.output_root.to_hex(),
                &coverage_root.to_hex(),
                &adjacency_root.to_hex(),
                &cohort_root.to_hex(),
                &checkpoint_digest.to_hex(),
                &as_i64(cursor)?,
                &VERIFIED_PREFIX_FORMAT_VERSION,
            ],
        )?;
        if changed != 1 {
            return Err(DurableError::Conflict(
                "navigation event cohort-set cursor compare-and-set failed",
            ));
        }
        tx.commit()?;
        Ok(DurableNavigationEventApply {
            event_sequence: sequence,
            event_id: first.event_id.clone(),
            output_root: first.output_root,
            replayed: false,
        })
    }

    fn publish_navigation_event_cohort_with_semantics(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        cohort: &NavigationRecordEventCohortV1,
        limits: DurableNavigationEventLimits,
        allow_historical_cut: bool,
        expected_cohort_set: Option<&DurableNavigationEventCohortSetV2>,
    ) -> DurableResult<DurableNavigationEventApply> {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation event domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation event consumer ID",
        )?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let deadline = Instant::now() + limits.statement_timeout;
        let witness = resolve_event_witness(
            &mut self.client,
            domain,
            cohort.event_sequence,
            profile,
            limits,
        )?;
        if cohort.event_id != witness.event_id {
            return Err(DurableError::Conflict(
                "navigation event ID differs from committed outbox",
            ));
        }
        let event_path = cohort
            .dependencies
            .iter()
            .find(|dependency| dependency.role == NavigationDependencyRoleV1::Event)
            .map(|dependency| dependency.path.as_str())
            .ok_or(DurableError::Invalid("navigation event witness absent"))?;
        let changed_paths = witness
            .members
            .iter()
            .filter(|member| member.is_source)
            .map(|member| member.source_path.as_str())
            .collect::<BTreeSet<_>>();
        let owner_changed = match cohort.basis() {
            NavigationRecordEventBasisV1::Mutation => {
                changed_paths.contains(cohort.source_path.as_str())
            }
            NavigationRecordEventBasisV1::DependencyRefresh {
                changed_dependency_path,
                ..
            } => changed_paths.contains(changed_dependency_path.as_str()),
            NavigationRecordEventBasisV1::DependencyRefreshSet {
                changed_dependencies,
            } => {
                !changed_dependencies.is_empty()
                    && changed_dependencies
                        .iter()
                        .all(|cause| changed_paths.contains(cause.path.as_str()))
            }
            NavigationRecordEventBasisV1::ClaimRevisionRefresh {
                changed_dependency_path,
                history_path,
                ..
            } => {
                changed_paths.contains(changed_dependency_path.as_str())
                    && changed_paths.contains(history_path.as_str())
            }
            NavigationRecordEventBasisV1::RangeReconciliation { .. } => false,
        };
        if matches!(
            cohort.basis(),
            NavigationRecordEventBasisV1::DependencyRefresh { .. }
                | NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
        ) || !changed_paths.contains(event_path)
            || !owner_changed
        {
            return Err(DurableError::Conflict(
                "navigation event cohort requires an atomic set or lacks its committed member",
            ));
        }
        let source_paths = cohort
            .dependencies
            .iter()
            .map(|dependency| dependency.path.as_str().to_owned())
            .collect::<Vec<_>>();
        let (selected, _) = read_selected_sources_at_cut(
            self,
            store,
            domain,
            &witness,
            source_paths,
            profile,
            limits,
            event_path,
            deadline,
        )?;
        let event_member_root = navigation_event_member_root_from_witness(&witness, &selected)?;
        let member_paths = witness
            .members
            .iter()
            .map(|member| member.source_path.as_str())
            .collect::<BTreeSet<_>>();
        let dependency_paths = cohort
            .dependencies
            .iter()
            .map(|dependency| dependency.path.as_str())
            .collect::<BTreeSet<_>>();
        if member_paths
            .iter()
            .any(|member_path| !dependency_paths.contains(member_path))
        {
            return Err(DurableError::Conflict(
                "navigation cohort omits a committed command member",
            ));
        }
        let manifest_raw = navigation_event_cohort_manifest(
            &witness.event_id,
            witness.event.commit_seq,
            event_member_root,
            std::slice::from_ref(cohort),
            limits.max_row_bytes as usize,
        )?;
        if let Some(expected) = expected_cohort_set {
            if expected.event_id != witness.event_id
                || expected.event_sequence != witness.event.commit_seq
                || expected.member_root != event_member_root
                || expected.set_root != Digest256::of_bytes(&manifest_raw)
                || expected.manifest_raw != manifest_raw
                || expected.cohorts().len() != 1
                || expected.cohorts()[0].source_path != cohort.source_path
                || expected.cohorts()[0].record_id != cohort.record_id
                || expected.cohorts()[0].output_root != cohort.output_root
            {
                return Err(DurableError::Conflict(
                    "navigation cohort set differs from committed event witness",
                ));
            }
        }
        let persisted = validate_cohort(cohort, limits)?;
        verify_dependency_cut(cohort, &selected, limits)?;
        verify_event_manifest(cohort, &selected, limits)?;
        let (dependency_root, output_root, input_root) = cohort_roots(cohort, limits)?;
        if dependency_root != cohort.dependency_root
            || output_root != cohort.output_root
            || input_root != cohort.input_root
        {
            return Err(DurableError::Invalid(
                "navigation event cohort root differs",
            ));
        }
        active_navigation(deadline)?;
        self.publish_validated_navigation_cohort(
            domain,
            consumer_id,
            profile,
            cohort,
            persisted,
            &witness,
            limits,
            deadline,
            allow_historical_cut,
            manifest_raw,
        )
    }

    /// Return the bounded global navigation index only when its complete
    /// bootstrap and every required committed event are current in one
    /// repeatable-read cut. Missing or stale coverage is a typed `NotReady`.
    pub fn read_navigation_current_index_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationCurrentIndexV1> {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation current domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation current consumer ID",
        )?;
        profile.validate()?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let currentness =
            read_navigation_currentness_guard_in_tx(&mut tx, domain, consumer_id, profile, limits)?;
        let currentness_guard = match currentness {
            DurableNavigationCurrentnessV1::Ready(guard) => guard,
            DurableNavigationCurrentnessV1::NotReady(not_ready) => {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    not_ready.domain_head_sequence,
                    not_ready.cursor_sequence,
                    not_ready.reason,
                ));
            }
        };
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        let head = snapshot.head_seq()?;
        let current = tx.query_one(
            "SELECT source_membership_digest,source_membership_count,source_generation,
                    source_complete,selected_generation_digest,source_projection_digest
             FROM cmd2_domain WHERE domain=$1",
            &[&domain],
        )?;
        if !current.get::<_, bool>("source_complete") {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                None,
                DurableNavigationCurrentIndexNotReadyReasonV1::SourceIncomplete,
            ));
        }
        let consumer = tx.query_opt(
            "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,
                    source_revision,last_event_id,last_input_root,last_output_root,
                    coverage_seq,coverage_root
             FROM cmd2_projection_navigation_consumer WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        let Some(consumer) = consumer else {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                None,
                DurableNavigationCurrentIndexNotReadyReasonV1::NoConsumer,
            ));
        };
        let cursor = as_u64(consumer.get("cursor_seq"))?;
        let profile_version = profile.version.to_string();
        if consumer.get::<_, String>("profile_id") != profile.id
            || consumer.get::<_, String>("profile_version") != profile_version
            || consumer.get::<_, String>("source_contract_digest")
                != profile.source_contract.to_hex()
        {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::ProfileMismatch,
            ));
        }
        let bootstrap = tx.query_opt(
            "SELECT through_seq,source_revision,bootstrap_root,owner_binding_json,
                    header_raw,header_sha256,node_count,edge_count,right_count,
                    projected_right_count,diagnostic_count
             FROM cmd2_projection_navigation_bootstrap
             WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        let Some(bootstrap) = bootstrap else {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageAbsent,
            ));
        };
        let coverage_seq = as_u64(consumer.get("coverage_seq"))?;
        let coverage_root = consumer
            .get::<_, Option<String>>("coverage_root")
            .map(parse_digest)
            .transpose()?;
        if cursor != head {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CursorBehindHead,
            ));
        }
        if coverage_seq != cursor
            || coverage_root.is_none()
            || as_u64(bootstrap.get("through_seq"))? > coverage_seq
        {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageAbsent,
            ));
        }
        let stored_bootstrap_root = parse_digest(bootstrap.get("bootstrap_root"))?;
        let active_sequence_rows = tx.query(
            "SELECT event_sequence,
                    COUNT(*) AS cohort_row_count,
                    COUNT(coverage_manifest_sha256) AS manifest_row_count,
                    COUNT(DISTINCT coverage_manifest_sha256) AS manifest_digest_count,
                    MIN(coverage_manifest_sha256) AS coverage_manifest_sha256
               FROM cmd2_projection_navigation_cohort
              WHERE domain=$1 AND consumer_id=$2
              GROUP BY event_sequence ORDER BY event_sequence LIMIT $3",
            &[
                &domain,
                &consumer_id,
                &i64::try_from(limits.max_current_cohorts.saturating_add(1))
                    .map_err(|_| DurableError::Refused("navigation current cohort count"))?,
            ],
        )?;
        if active_sequence_rows.len() > limits.max_current_cohorts {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded,
            ));
        }
        if active_sequence_rows.iter().any(|row| {
            row.get::<_, i64>("cohort_row_count") != row.get::<_, i64>("manifest_row_count")
                || row.get::<_, i64>("manifest_digest_count") != 1
                || row
                    .get::<_, Option<String>>("coverage_manifest_sha256")
                    .is_none()
        }) {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
            ));
        }
        let mut active_manifest_roots = BTreeMap::new();
        let mut active_sequences = Vec::with_capacity(active_sequence_rows.len());
        for row in &active_sequence_rows {
            let sequence = as_u64(row.get("event_sequence"))?;
            let manifest_root = parse_digest(
                row.get::<_, Option<String>>("coverage_manifest_sha256")
                    .ok_or(DurableError::Corrupt(
                        "navigation cohort manifest digest absent after validation",
                    ))?,
            )?;
            active_sequences.push(as_i64(sequence)?);
            active_manifest_roots.insert(sequence, manifest_root);
        }
        let bootstrap_sequence = as_u64(bootstrap.get("through_seq"))?;
        if coverage_seq > bootstrap_sequence && !active_manifest_roots.contains_key(&coverage_seq) {
            active_sequences.push(as_i64(coverage_seq)?);
            active_sequences.sort_unstable();
        }
        let coverage_row_cap = i64::try_from(active_sequences.len().saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation coverage event count"))?;
        let range_rows = tx.query(
        "SELECT cursor_before,through_seq,held_source_head_seq,terminal_event_id,range_root,range_manifest,
                    source_revision,input_root,dependency_root,output_root,
                    previous_coverage_root,coverage_root,terminal_cohort_count,
                    terminal_cohort_root,terminal_cohort_manifest
               FROM cmd2_projection_navigation_coverage_range
              WHERE domain=$1 AND consumer_id=$2 AND through_seq=ANY($3)
              ORDER BY through_seq LIMIT $4",
            &[
                &domain,
                &consumer_id,
                &active_sequences,
                &coverage_row_cap,
            ],
        )?;
        let coverage_rows = tx.query(
            "SELECT event_sequence,event_id,source_path,record_id,source_revision,
                    input_root,dependency_root,output_root,previous_coverage_root,coverage_root,
                    cohort_count,cohort_set_root,cohort_manifest
             FROM cmd2_projection_navigation_coverage_event
             WHERE domain=$1 AND consumer_id=$2 AND event_sequence=ANY($3)
             ORDER BY event_sequence LIMIT $4",
            &[&domain, &consumer_id, &active_sequences, &coverage_row_cap],
        )?;
        if coverage_rows.len() + range_rows.len() != active_sequences.len() {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
            ));
        }
        let mut coverage_events_by_sequence = BTreeMap::new();
        let mut manifest_bytes = 0u64;
        for row in coverage_rows.iter() {
            let sequence = as_u64(row.get("event_sequence"))?;
            let expected_sequence = sequence;
            let previous_root = parse_digest(row.get("previous_coverage_root"))?;
            let source_revision = parse_digest(row.get("source_revision"))?;
            let input_root = parse_digest(row.get("input_root"))?;
            let dependency_root = parse_digest(row.get("dependency_root"))?;
            let output_root = parse_digest(row.get("output_root"))?;
            let event_id: String = row.get("event_id");
            let source_path: String = row.get("source_path");
            let record_id: String = row.get("record_id");
            let cohort_count: i32 = row.get("cohort_count");
            let cohort_set_root = row
                .get::<_, Option<String>>("cohort_set_root")
                .map(parse_digest)
                .transpose()?;
            let cohort_manifest = row.get::<_, Option<Vec<u8>>>("cohort_manifest");
            let (Some(cohort_set_root), Some(cohort_manifest)) = (cohort_set_root, cohort_manifest)
            else {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            };
            if active_manifest_roots.get(&sequence).copied() != Some(cohort_set_root) {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            }
            manifest_bytes = manifest_bytes
                .checked_add(cohort_manifest.len() as u64)
                .ok_or(DurableError::Refused("navigation manifest byte budget"))?;
            let manifest = if manifest_bytes <= limits.max_total_bytes {
                verify_navigation_event_cohort_manifest(
                    &cohort_manifest,
                    cohort_set_root,
                    u32::try_from(cohort_count).unwrap_or(0),
                    &event_id,
                    expected_sequence,
                    limits.max_row_bytes as usize,
                )
                .ok()
            } else {
                None
            };
            let Some(manifest) = manifest else {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            };
            let Some(first_manifest_cohort) = manifest.cohorts.values().next() else {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            };
            if first_manifest_cohort.source_path != source_path
                || first_manifest_cohort.record_id != record_id
                || first_manifest_cohort.source_revision != source_revision
                || first_manifest_cohort.input_root != input_root
                || first_manifest_cohort.dependency_root != dependency_root
                || first_manifest_cohort.output_root != output_root
            {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            }
            let calculated = navigation_coverage_root_next(
                previous_root,
                expected_sequence,
                &event_id,
                &source_path,
                &record_id,
                source_revision,
                input_root,
                dependency_root,
                output_root,
                cohort_set_root,
            );
            if sequence != expected_sequence
                || parse_digest(row.get("coverage_root"))? != calculated
            {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            }
            coverage_events_by_sequence.insert(
                sequence,
                (
                    event_id,
                    manifest.cohorts,
                    manifest.claim_set,
                    manifest.claim_refresh_set,
                    manifest.claim_empty_closure,
                    manifest.claim_range_set,
                ),
            );
        }
        for row in &range_rows {
            let cursor_before = as_u64(row.get("cursor_before"))?;
            let through_sequence = as_u64(row.get("through_seq"))?;
            let held_source_head_sequence = as_u64(row.get("held_source_head_seq"))?;
            let terminal_event_id: String = row.get("terminal_event_id");
            let range_root = parse_digest(row.get("range_root"))?;
            let range_manifest = row.get::<_, Vec<u8>>("range_manifest");
            let source_revision = parse_digest(row.get("source_revision"))?;
            let input_root = parse_digest(row.get("input_root"))?;
            let dependency_root = parse_digest(row.get("dependency_root"))?;
            let output_root = parse_digest(row.get("output_root"))?;
            let previous_root = parse_digest(row.get("previous_coverage_root"))?;
            let stored_root = parse_digest(row.get("coverage_root"))?;
            let cohort_count = u32::try_from(row.get::<_, i32>("terminal_cohort_count"))
                .map_err(|_| DurableError::Corrupt("navigation range cohort count is negative"))?;
            let terminal_cohort_root = parse_digest(row.get("terminal_cohort_root"))?;
            let current_manifest_root = active_manifest_roots.get(&through_sequence).copied();
            let manifest_matches = if cohort_count == 0 {
                through_sequence == coverage_seq && current_manifest_root.is_none()
            } else {
                current_manifest_root == Some(terminal_cohort_root)
            };
            if !manifest_matches {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            }
            let terminal_manifest = row.get::<_, Vec<u8>>("terminal_cohort_manifest");
            manifest_bytes = manifest_bytes
                .checked_add(range_manifest.len() as u64)
                .and_then(|value| value.checked_add(terminal_manifest.len() as u64))
                .ok_or(DurableError::Refused(
                    "navigation range manifest byte budget",
                ))?;
            if manifest_bytes > limits.max_total_bytes
                || through_sequence <= cursor_before
                || through_sequence > coverage_seq
                || held_source_head_sequence < through_sequence
                || held_source_head_sequence > head
            {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    if manifest_bytes > limits.max_total_bytes {
                        DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded
                    } else {
                        DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch
                    },
                ));
            }
            let Some(manifest) =
                durable_navigation_range::verify_navigation_event_range_terminal_manifest(
                    &terminal_manifest,
                    terminal_cohort_root,
                    cohort_count,
                    cursor_before,
                    through_sequence,
                    held_source_head_sequence,
                    &terminal_event_id,
                    range_root,
                    &range_manifest,
                    input_root,
                    dependency_root,
                    output_root,
                    limits.max_total_bytes as usize,
                )
                .ok()
            else {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            };
            let calculated = durable_navigation_range::navigation_range_coverage_root_next(
                previous_root,
                cursor_before,
                through_sequence,
                held_source_head_sequence,
                &terminal_event_id,
                range_root,
                terminal_cohort_root,
                source_revision,
                dependency_root,
                output_root,
            );
            if stored_root != calculated {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::CoverageChainMismatch,
                ));
            }
            coverage_events_by_sequence.insert(
                through_sequence,
                (
                    terminal_event_id,
                    manifest.cohorts,
                    manifest.claim_set,
                    manifest.claim_refresh_set,
                    manifest.claim_empty_closure,
                    manifest.claim_range_set,
                ),
            );
        }
        let owner_binding_raw: String = bootstrap.get("owner_binding_json");
        let owner_binding_value =
            strict_json_value(owner_binding_raw.as_bytes(), limits.max_row_bytes)?;
        let owner_binding = navigation_owner_binding_from_json(&owner_binding_value)?;
        let membership = current
            .get::<_, Option<String>>("source_membership_digest")
            .map(parse_digest)
            .transpose()?;
        let membership_count = current
            .get::<_, Option<i64>>("source_membership_count")
            .map(as_u64)
            .transpose()?;
        let current_generation = current
            .get::<_, Option<i64>>("source_generation")
            .map(as_u64)
            .transpose()?;
        let selected_generation: Option<String> = current.get("selected_generation_digest");
        let source_projection: Option<String> = current.get("source_projection_digest");
        if membership != Some(owner_binding.source_membership_root)
            || membership_count != Some(owner_binding.source_membership_count)
            || current_generation != Some(owner_binding.managed_generation_commit_sequence)
            || selected_generation.as_deref()
                != Some(owner_binding.managed_generation_digest.to_hex().as_str())
            || source_projection.as_deref()
                != Some(owner_binding.source_projection_digest.to_hex().as_str())
        {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::ManagedGenerationMismatch,
            ));
        }
        let header_raw: String = bootstrap.get("header_raw");
        let header_raw = header_raw.into_bytes();
        let header_sha256 = parse_digest(bootstrap.get::<_, String>("header_sha256"))?;
        if Digest256::of_bytes(&header_raw) != header_sha256
            || header_sha256 != owner_binding.navigation_header_sha256
        {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::HeaderMismatch,
            ));
        }
        let header_value = strict_json_value(&header_raw, limits.max_row_bytes)?;
        if owner_binding.navigation_header != header_value {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::HeaderMismatch,
            ));
        }
        let row_cap = i64::try_from(limits.max_current_rows.saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation current row count"))?;
        let stored_rows = tx.query(
            "SELECT row_kind,row_id,owner_source_path,owner_record_id,active,visible,
                    row_json,row_sha256
             FROM cmd2_projection_navigation_bootstrap_row
             WHERE domain=$1 AND consumer_id=$2
             ORDER BY row_kind COLLATE \"C\",row_id COLLATE \"C\",
                      owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"
             LIMIT $3",
            &[&domain, &consumer_id, &row_cap],
        )?;
        if stored_rows.len() > limits.max_current_rows {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded,
            ));
        }
        let owner_cap = i64::try_from(limits.max_current_rows.saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation owner source count"))?;
        let owner_rows = tx.query(
            "SELECT owner_source_path,source_sha256,size_bytes
             FROM cmd2_projection_navigation_bootstrap_owner
             WHERE domain=$1 AND consumer_id=$2
             ORDER BY owner_source_path COLLATE \"C\" LIMIT $3",
            &[&domain, &consumer_id, &owner_cap],
        )?;
        if owner_rows.len() > limits.max_current_rows {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded,
            ));
        }
        let mut owner_sources = BTreeMap::new();
        for row in owner_rows {
            let source_path: String = row.get("owner_source_path");
            owner_sources.insert(
                source_path.clone(),
                DurableNavigationBootstrapOwnerSourceV1 {
                    source_path,
                    sha256: parse_digest(row.get("source_sha256"))?,
                    size_bytes: as_u64(row.get("size_bytes"))?,
                },
            );
        }
        let mut all_base_rows = Vec::with_capacity(stored_rows.len());
        let bootstrap_row_count = stored_rows.len();
        let mut charged_bytes = header_raw.len() as u64;
        for owner in owner_sources.values() {
            charged_bytes = charged_bytes
                .checked_add(owner.source_path.len() as u64 + 96)
                .ok_or(DurableError::Refused("navigation current byte count"))?;
        }
        for row in stored_rows {
            let kind = parse_bootstrap_kind(row.get("row_kind"))?;
            let id: String = row.get("row_id");
            let owner_source_path: String = row.get("owner_source_path");
            let owner_record_id: String = row.get("owner_record_id");
            let active: bool = row.get("active");
            let visible: Option<bool> = row.get("visible");
            let raw: String = row.get("row_json");
            let raw = raw.into_bytes();
            let sha256 = parse_digest(row.get("row_sha256"))?;
            charged_bytes = charged_bytes
                .checked_add(raw.len() as u64)
                .ok_or(DurableError::Refused("navigation current byte count"))?;
            if charged_bytes > limits.max_total_bytes || Digest256::of_bytes(&raw) != sha256 {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                ));
            }
            let value = strict_json_value(&raw, limits.max_row_bytes)?;
            if kind
                .id_field()
                .is_some_and(|field| value.get(field).and_then(Value::as_str) != Some(id.as_str()))
                || (matches!(
                    kind,
                    DurableNavigationBootstrapRowKindV1::Node
                        | DurableNavigationBootstrapRowKindV1::Edge
                ) != visible.is_some())
            {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                ));
            }
            let source_refs = bootstrap_source_refs(&value, &owner_source_path)?;
            all_base_rows.push(DurableNavigationBaseRowReadV1 {
                kind,
                id,
                owner_source_path,
                owner_record_id,
                active,
                visible,
                sha256,
                from_id: value
                    .get("from_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                to_id: value
                    .get("to_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                source_refs,
                raw,
            });
        }
        let mut node_ids = BTreeSet::new();
        let mut edge_ids = BTreeSet::new();
        let mut right_ids = BTreeSet::new();
        let mut diagnostic_count = 0u64;
        for row in &all_base_rows {
            if !owner_sources.contains_key(&row.owner_source_path) {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::BootstrapCoverageMismatch,
                ));
            }
            match row.kind {
                DurableNavigationBootstrapRowKindV1::Node => {
                    node_ids.insert(row.id.as_str());
                }
                DurableNavigationBootstrapRowKindV1::Edge => {
                    edge_ids.insert(row.id.as_str());
                }
                DurableNavigationBootstrapRowKindV1::Right => {
                    right_ids.insert(row.id.as_str());
                }
                DurableNavigationBootstrapRowKindV1::Diagnostic => {
                    diagnostic_count = diagnostic_count.saturating_add(1);
                }
            }
        }
        let header_counts = header_value
            .get("counts")
            .ok_or(DurableError::Corrupt("navigation stored header counts"))?;
        if node_ids.len() as u64 != owner_binding.navigation_nodes
            || edge_ids.len() as u64 != owner_binding.navigation_edges
            || right_ids.len() as u64 != owner_binding.navigation_projected_rights
            || header_counts.get("nodes").and_then(Value::as_u64)
                != Some(owner_binding.navigation_nodes)
            || header_counts.get("edges").and_then(Value::as_u64)
                != Some(owner_binding.navigation_edges)
            || header_counts.get("rights").and_then(Value::as_u64)
                != Some(owner_binding.navigation_rights)
            || as_u64(bootstrap.get("node_count"))? != owner_binding.navigation_nodes
            || as_u64(bootstrap.get("edge_count"))? != owner_binding.navigation_edges
            || as_u64(bootstrap.get("right_count"))? != owner_binding.navigation_rights
            || as_u64(bootstrap.get("projected_right_count"))?
                != owner_binding.navigation_projected_rights
            || as_u64(bootstrap.get("diagnostic_count"))? != diagnostic_count
        {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BootstrapCoverageMismatch,
            ));
        }
        if navigation_bootstrap_root(
            &owner_binding,
            &header_raw,
            &all_base_rows,
            &owner_sources,
            limits.max_total_bytes as usize,
        )? != stored_bootstrap_root
        {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BootstrapCoverageMismatch,
            ));
        }
        let base_rows = all_base_rows
            .into_iter()
            .filter(|row| row.active)
            .collect::<Vec<_>>();
        let event_carrier_usage = tx.query_one(
            "SELECT count(*) AS row_count,
                    COALESCE(sum(octet_length(row_json)),0) AS row_bytes
             FROM (
               SELECT row_json FROM cmd2_projection_navigation_node
                WHERE domain=$1 AND consumer_id=$2
               UNION ALL
               SELECT row_json FROM cmd2_projection_navigation_edge
                WHERE domain=$1 AND consumer_id=$2
               UNION ALL
               SELECT row_json FROM cmd2_projection_navigation_diagnostic
                WHERE domain=$1 AND consumer_id=$2
             ) carriers",
            &[&domain, &consumer_id],
        )?;
        let dependency_usage = tx.query_one(
            "SELECT count(*) AS row_count,
                    COALESCE(sum(octet_length(dependency_path)+octet_length(dependency_role)
                      +COALESCE(octet_length(sha256),0)+COALESCE(octet_length(absent_reason),0)
                      +64),0) AS row_bytes
             FROM cmd2_projection_navigation_dependency
             WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        let event_row_count = as_u64(event_carrier_usage.get("row_count"))?;
        let event_row_bytes = as_u64(event_carrier_usage.get("row_bytes"))?;
        let dependency_count = as_u64(dependency_usage.get("row_count"))?;
        let dependency_bytes = as_u64(dependency_usage.get("row_bytes"))?;
        let aggregate_row_count = (bootstrap_row_count as u64)
            .checked_add(event_row_count)
            .ok_or(DurableError::Refused("navigation current row count"))?;
        charged_bytes = charged_bytes
            .checked_add(event_row_bytes)
            .and_then(|value| value.checked_add(dependency_bytes))
            .ok_or(DurableError::Refused("navigation current byte count"))?;
        if aggregate_row_count > limits.max_current_rows as u64
            || dependency_count > limits.max_dependencies as u64
            || charged_bytes > limits.max_total_bytes
        {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded,
            ));
        }
        let cohort_cap = i64::try_from(limits.max_current_cohorts.saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation current cohort count"))?;
        let cohort_rows = tx.query(
            "SELECT source_path,record_id,event_id,event_sequence,source_event_id,
                    source_event_version,predecessor_source_revision,source_revision,
                    basis_kind,basis_dependency_path,basis_dependency_sha256,
                    basis_dependency_causes_json,basis_dependency_causes_root,
                    basis_history_path,basis_history_sha256,basis_claim_id,
                    basis_revision_proof_root,claim_owner_inventory_root,
                    claim_stream_rows_scanned,claim_event_basis_root,
                    claim_navigation_manifest_root,
                    predecessor_cohort_root,dependency_root,input_root,output_root
             FROM cmd2_projection_navigation_cohort
             WHERE domain=$1 AND consumer_id=$2
             ORDER BY source_path COLLATE \"C\",record_id COLLATE \"C\" LIMIT $3",
            &[&domain, &consumer_id, &cohort_cap],
        )?;
        if cohort_rows.len() > limits.max_current_cohorts {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::BudgetExceeded,
            ));
        }
        let mut cohorts = Vec::with_capacity(cohort_rows.len());
        for row in cohort_rows {
            let source_path: String = row.get("source_path");
            let record_id: String = row.get("record_id");
            let event_sequence = as_u64(row.get("event_sequence"))?;
            if event_sequence > cursor {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                ));
            }
            let event_id: String = row.get("event_id");
            let source_event_id: String = row.get("source_event_id");
            let source_event_version = as_u64(row.get("source_event_version"))?;
            let predecessor_source_revision = parse_digest(row.get("predecessor_source_revision"))?;
            let source_revision = parse_digest(row.get("source_revision"))?;
            let basis_kind: String = row.get("basis_kind");
            let basis_dependency_path: Option<String> = row.get("basis_dependency_path");
            let basis_dependency_sha256 = row
                .get::<_, Option<String>>("basis_dependency_sha256")
                .map(parse_digest)
                .transpose()?;
            let basis_dependency_causes_json: Option<String> =
                row.get("basis_dependency_causes_json");
            let basis_dependency_causes_root = row
                .get::<_, Option<String>>("basis_dependency_causes_root")
                .map(parse_digest)
                .transpose()?;
            let basis_dependency_causes = parse_navigation_record_dependency_refresh_cause_columns(
                &basis_kind,
                basis_dependency_causes_json.as_deref(),
                basis_dependency_causes_root,
                &source_path,
                limits.max_row_bytes as usize,
            )?;
            let basis_history_path: Option<String> = row.get("basis_history_path");
            let basis_history_sha256 = row
                .get::<_, Option<String>>("basis_history_sha256")
                .map(parse_digest)
                .transpose()?;
            let basis_claim_id: Option<String> = row.get("basis_claim_id");
            let basis_revision_proof_root = row
                .get::<_, Option<String>>("basis_revision_proof_root")
                .map(parse_digest)
                .transpose()?;
            let claim_owner_inventory_root = row
                .get::<_, Option<String>>("claim_owner_inventory_root")
                .map(parse_digest)
                .transpose()?;
            let claim_stream_rows_scanned = row
                .get::<_, Option<i64>>("claim_stream_rows_scanned")
                .map(as_u64)
                .transpose()?;
            let claim_event_basis_root = row
                .get::<_, Option<String>>("claim_event_basis_root")
                .map(parse_digest)
                .transpose()?;
            let claim_navigation_manifest_root = row
                .get::<_, Option<String>>("claim_navigation_manifest_root")
                .map(parse_digest)
                .transpose()?;
            let predecessor_cohort_root = row
                .get::<_, Option<String>>("predecessor_cohort_root")
                .map(parse_digest)
                .transpose()?;
            let dependency_root = parse_digest(row.get("dependency_root"))?;
            let input_root = parse_digest(row.get("input_root"))?;
            let output_root = parse_digest(row.get("output_root"))?;
            let Some(coverage_event) = coverage_events_by_sequence.get(&event_sequence) else {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                ));
            };
            let manifest_cohort = coverage_event
                .1
                .get(&(source_path.clone(), record_id.clone()));
            if coverage_event.0 != event_id
                || manifest_cohort.is_none_or(|manifest_cohort| {
                    manifest_cohort.source_revision != source_revision
                        || manifest_cohort.owner_kind
                            != if matches!(
                                basis_kind.as_str(),
                                "claim-mutation"
                                    | "claim-dependency-refresh"
                                    | "claim-range-reconciliation"
                            ) {
                                "claim"
                            } else {
                                "record"
                            }
                        || manifest_cohort.basis_kind != basis_kind
                        || manifest_cohort.basis_dependency_path != basis_dependency_path
                        || manifest_cohort.basis_dependency_sha256 != basis_dependency_sha256
                        || manifest_cohort.basis_dependency_causes != basis_dependency_causes
                        || manifest_cohort.basis_dependency_causes_root
                            != basis_dependency_causes_root
                        || manifest_cohort.basis_history_path != basis_history_path
                        || manifest_cohort.basis_history_sha256 != basis_history_sha256
                        || manifest_cohort.basis_claim_id != basis_claim_id
                        || manifest_cohort.basis_revision_proof_root != basis_revision_proof_root
                        || manifest_cohort.claim_owner_inventory_root != claim_owner_inventory_root
                        || manifest_cohort.claim_stream_rows_scanned != claim_stream_rows_scanned
                        || manifest_cohort.claim_event_basis_root != claim_event_basis_root
                        || manifest_cohort.claim_navigation_manifest_root
                            != claim_navigation_manifest_root
                        || manifest_cohort
                            .source_event_id
                            .as_deref()
                            .is_some_and(|expected| expected != source_event_id.as_str())
                        || manifest_cohort
                            .source_event_version
                            .is_some_and(|expected| expected != source_event_version)
                        || manifest_cohort
                            .predecessor_source_revision
                            .is_some_and(|expected| expected != predecessor_source_revision)
                        || manifest_cohort
                            .predecessor_cohort_root
                            .is_some_and(|expected| Some(expected) != predecessor_cohort_root)
                        || manifest_cohort.input_root != input_root
                        || manifest_cohort.dependency_root != dependency_root
                        || manifest_cohort.output_root != output_root
                })
            {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                ));
            }
            let manifest_cohort = manifest_cohort.ok_or(DurableError::Corrupt(
                "navigation current owner manifest row absent after check",
            ))?;
            let (
                claim_source_line,
                claim_trace_sha256,
                claim_event_set_root,
                claim_mutation_receipt_path,
                claim_mutation_receipt_sha256,
                claim_stream_sha256,
                claim_owner_inventory_root,
                claim_stream_rows_scanned,
                claim_event_basis_root,
                claim_navigation_manifest_root,
            ) = if basis_kind == "claim-mutation" {
                let Some(claim_set) = coverage_event.2.as_ref() else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(source_line) = manifest_cohort.claim_source_line else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(trace_sha256) = manifest_cohort.claim_trace_sha256 else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let local_stream = claim_set
                    .streams
                    .iter()
                    .find(|stream| stream.source_path == source_path);
                let Some(local_stream) = local_stream else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                if local_stream
                    .owner_claim_ids
                    .binary_search(&record_id)
                    .is_err()
                    || claim_set.owner_claim_ids.binary_search(&record_id).is_err()
                    || manifest_cohort.claim_owner_inventory_root != claim_set.owner_inventory_root
                    || manifest_cohort.claim_stream_rows_scanned != Some(local_stream.rows_scanned)
                    || manifest_cohort.claim_event_basis_root
                        != Some(claim_set.event_claim_set_root)
                    || manifest_cohort.claim_navigation_manifest_root
                        != claim_set.affected_owner_manifest_root
                    || claim_set.revision_claim_id.as_deref() == Some(&record_id)
                        && claim_set.revision_proof_root != basis_revision_proof_root
                    || claim_set.revision_claim_id.as_deref() != Some(&record_id)
                        && basis_revision_proof_root.is_some()
                {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                }
                (
                    Some(source_line),
                    Some(trace_sha256),
                    Some(claim_set.event_claim_set_root),
                    Some(claim_set.mutation_receipt_path.clone()),
                    Some(claim_set.mutation_receipt_sha256),
                    Some(local_stream.stream_sha256),
                    manifest_cohort.claim_owner_inventory_root,
                    manifest_cohort.claim_stream_rows_scanned,
                    manifest_cohort.claim_event_basis_root,
                    manifest_cohort.claim_navigation_manifest_root,
                )
            } else if basis_kind == "claim-dependency-refresh" {
                let Some(refresh_set) = coverage_event.3.as_ref() else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(source_line) = manifest_cohort.claim_source_line else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(trace_sha256) = manifest_cohort.claim_trace_sha256 else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(local_stream) = refresh_set
                    .streams
                    .iter()
                    .find(|stream| stream.source_path == source_path)
                else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                if local_stream
                    .owner_claim_ids
                    .binary_search(&record_id)
                    .is_err()
                    || manifest_cohort.claim_owner_inventory_root
                        != Some(refresh_set.owner_inventory_root)
                    || manifest_cohort.claim_stream_rows_scanned != Some(local_stream.rows_scanned)
                    || manifest_cohort.claim_navigation_manifest_root
                        != Some(refresh_set.affected_owner_manifest_root)
                    || manifest_cohort.claim_event_basis_root.is_none()
                    || manifest_cohort.basis_revision_proof_root.is_some()
                {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                }
                (
                    Some(source_line),
                    Some(trace_sha256),
                    None,
                    None,
                    None,
                    Some(local_stream.stream_sha256),
                    manifest_cohort.claim_owner_inventory_root,
                    manifest_cohort.claim_stream_rows_scanned,
                    manifest_cohort.claim_event_basis_root,
                    manifest_cohort.claim_navigation_manifest_root,
                )
            } else if basis_kind == "claim-range-reconciliation" {
                let Some(range_set) = coverage_event.5.as_ref() else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(source_line) = manifest_cohort.claim_source_line else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(trace_sha256) = manifest_cohort.claim_trace_sha256 else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                let Some(local_stream) = range_set
                    .terminal_streams
                    .iter()
                    .find(|stream| stream.source_path == source_path)
                else {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                };
                if local_stream
                    .owner_claim_ids
                    .binary_search(&record_id)
                    .is_err()
                    || range_set
                        .terminal_owner_ids
                        .binary_search(&record_id)
                        .is_err()
                    || manifest_cohort.claim_owner_inventory_root
                        != Some(range_set.owner_inventory_root)
                    || manifest_cohort.claim_stream_rows_scanned != Some(local_stream.rows_scanned)
                    || manifest_cohort.claim_event_basis_root.is_none()
                    || manifest_cohort.claim_navigation_manifest_root
                        != Some(range_set.range_manifest_root)
                    || manifest_cohort.basis_revision_proof_root.is_some()
                {
                    tx.commit()?;
                    return Ok(navigation_not_ready(
                        head,
                        Some(cursor),
                        DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                    ));
                }
                (
                    Some(source_line),
                    Some(trace_sha256),
                    None,
                    None,
                    None,
                    Some(local_stream.stream_sha256),
                    manifest_cohort.claim_owner_inventory_root,
                    manifest_cohort.claim_stream_rows_scanned,
                    manifest_cohort.claim_event_basis_root,
                    manifest_cohort.claim_navigation_manifest_root,
                )
            } else {
                (None, None, None, None, None, None, None, None, None, None)
            };
            let (nodes, node_visibility) = read_rows(
                &mut tx,
                "cmd2_projection_navigation_node",
                "node_id",
                domain,
                consumer_id,
                &source_path,
                &record_id,
                limits,
            )?;
            let (edges, edge_visibility) = read_rows(
                &mut tx,
                "cmd2_projection_navigation_edge",
                "edge_id",
                domain,
                consumer_id,
                &source_path,
                &record_id,
                limits,
            )?;
            let cohort = DurableNavigationEventCohortReadV1 {
                event_id,
                event_sequence,
                source_event_id,
                source_event_version,
                predecessor_source_revision,
                source_revision,
                basis_kind,
                basis_dependency_path,
                basis_dependency_sha256,
                basis_dependency_causes: basis_dependency_causes
                    .iter()
                    .map(|cause| DurableNavigationDependencyRefreshCauseReadV1 {
                        path: cause.path.as_str().to_owned(),
                        sha256: cause.sha256,
                    })
                    .collect(),
                basis_dependency_causes_root,
                basis_history_path,
                basis_history_sha256,
                basis_claim_id,
                basis_revision_proof_root,
                claim_source_line,
                claim_trace_sha256,
                claim_event_set_root,
                claim_mutation_receipt_path,
                claim_mutation_receipt_sha256,
                claim_stream_sha256,
                claim_owner_inventory_root,
                claim_stream_rows_scanned,
                claim_event_basis_root,
                claim_navigation_manifest_root,
                source_path: source_path.clone(),
                record_id: record_id.clone(),
                predecessor_cohort_root,
                dependency_root,
                input_root,
                output_root,
                dependencies: read_dependencies(
                    &mut tx,
                    domain,
                    consumer_id,
                    &source_path,
                    &record_id,
                    limits,
                )?,
                nodes,
                node_visibility,
                edges,
                edge_visibility,
                diagnostics: read_diagnostics(
                    &mut tx,
                    domain,
                    consumer_id,
                    &source_path,
                    &record_id,
                    limits,
                )?,
            };
            if verify_read_roots(&cohort, limits).is_err() {
                tx.commit()?;
                return Ok(navigation_not_ready(
                    head,
                    Some(cursor),
                    DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
                ));
            }
            cohorts.push(cohort);
        }
        let coverage_root = coverage_root.ok_or(DurableError::Corrupt(
            "navigation coverage root absent after readiness check",
        ))?;
        let index_root = navigation_current_index_root(
            stored_bootstrap_root,
            coverage_root,
            currentness_guard.adjacency_root(),
            currentness_guard.cohort_root,
        );
        if index_root != currentness_guard.index_root() {
            tx.commit()?;
            return Ok(navigation_not_ready(
                head,
                Some(cursor),
                DurableNavigationCurrentIndexNotReadyReasonV1::IndexRootMismatch,
            ));
        }
        tx.commit()?;
        Ok(DurableNavigationCurrentIndexV1 {
            status: DurableNavigationCurrentIndexStatusV1::Ready(
                DurableNavigationCurrentIndexReadyV1 {
                    domain_head_sequence: head,
                    coverage_sequence: coverage_seq,
                    coverage_root,
                    adjacency_root: currentness_guard.adjacency_root(),
                    source_revision: currentness_guard.source_revision(),
                    index_root,
                    owner_binding,
                    base_rows,
                    header_raw,
                    header_sha256,
                    cohorts,
                },
            ),
        })
    }

    /// Read and re-hash one bounded stored cohort. This is an event-local
    /// derivative read; it does not certify the completeness of other events.
    pub fn read_navigation_event_cohort_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        source_path: &str,
        record_id: &str,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Option<DurableNavigationEventCohortReadV1>> {
        let limits = limits.validate()?;
        validate_identifier(domain, super::MAX_DOMAIN_BYTES, "navigation event domain")?;
        validate_identifier(
            consumer_id,
            super::MAX_CONSUMER_ID_BYTES,
            "navigation event consumer ID",
        )?;
        validate_relative_path(source_path)?;
        validate_identifier(
            record_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation event record ID",
        )?;
        profile.validate()?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        verify_navigation_consumer(&mut tx, domain, consumer_id, profile)?;
        let row = tx.query_opt(
            "SELECT event_id,event_sequence,source_event_id,source_event_version,
                    predecessor_source_revision,source_revision,basis_kind,
                    basis_dependency_path,basis_dependency_sha256,
                    basis_dependency_causes_json,basis_dependency_causes_root,basis_history_path,
                    basis_history_sha256,basis_claim_id,basis_revision_proof_root,
                    claim_owner_inventory_root,claim_stream_rows_scanned,
                    claim_event_basis_root,claim_navigation_manifest_root,
                    predecessor_cohort_root,dependency_root,input_root,output_root
             FROM cmd2_projection_navigation_cohort
             WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4",
            &[&domain, &consumer_id, &source_path, &record_id],
        )?;
        let Some(row) = row else {
            tx.commit()?;
            return Ok(None);
        };
        let event_id: String = row.get("event_id");
        let event_sequence = as_u64(row.get("event_sequence"))?;
        let source_event_id: String = row.get("source_event_id");
        let source_event_version = as_u64(row.get("source_event_version"))?;
        let predecessor_source_revision = parse_digest(row.get("predecessor_source_revision"))?;
        let source_revision = parse_digest(row.get("source_revision"))?;
        let basis_kind: String = row.get("basis_kind");
        let basis_dependency_path: Option<String> = row.get("basis_dependency_path");
        let basis_dependency_sha256 = row
            .get::<_, Option<String>>("basis_dependency_sha256")
            .map(parse_digest)
            .transpose()?;
        let basis_dependency_causes_json: Option<String> = row.get("basis_dependency_causes_json");
        let basis_dependency_causes_root = row
            .get::<_, Option<String>>("basis_dependency_causes_root")
            .map(parse_digest)
            .transpose()?;
        let basis_dependency_causes = parse_navigation_record_dependency_refresh_cause_columns(
            &basis_kind,
            basis_dependency_causes_json.as_deref(),
            basis_dependency_causes_root,
            source_path,
            limits.max_row_bytes as usize,
        )?;
        let basis_history_path: Option<String> = row.get("basis_history_path");
        let basis_history_sha256 = row
            .get::<_, Option<String>>("basis_history_sha256")
            .map(parse_digest)
            .transpose()?;
        let basis_claim_id: Option<String> = row.get("basis_claim_id");
        let basis_revision_proof_root = row
            .get::<_, Option<String>>("basis_revision_proof_root")
            .map(parse_digest)
            .transpose()?;
        let claim_owner_inventory_root = row
            .get::<_, Option<String>>("claim_owner_inventory_root")
            .map(parse_digest)
            .transpose()?;
        let claim_stream_rows_scanned = row
            .get::<_, Option<i64>>("claim_stream_rows_scanned")
            .map(as_u64)
            .transpose()?;
        let claim_event_basis_root = row
            .get::<_, Option<String>>("claim_event_basis_root")
            .map(parse_digest)
            .transpose()?;
        let claim_navigation_manifest_root = row
            .get::<_, Option<String>>("claim_navigation_manifest_root")
            .map(parse_digest)
            .transpose()?;
        let predecessor_cohort_root = row
            .get::<_, Option<String>>("predecessor_cohort_root")
            .map(parse_digest)
            .transpose()?;
        let dependency_root = parse_digest(row.get("dependency_root"))?;
        let input_root = parse_digest(row.get("input_root"))?;
        let output_root = parse_digest(row.get("output_root"))?;
        let range_manifest = if matches!(
            basis_kind.as_str(),
            "range-reconciliation" | "claim-range-reconciliation"
        ) {
            Some(read_navigation_event_range_owner_manifest_in_tx(
                &mut tx,
                domain,
                consumer_id,
                event_sequence,
                &event_id,
                &source_path,
                &record_id,
                limits,
            )?)
        } else {
            None
        };
        let (
            claim_source_line,
            claim_trace_sha256,
            claim_event_set_root,
            claim_mutation_receipt_path,
            claim_mutation_receipt_sha256,
            claim_stream_sha256,
            manifest_claim_owner_inventory_root,
            manifest_claim_stream_rows_scanned,
            manifest_claim_event_basis_root,
            manifest_claim_navigation_root,
        ) = if basis_kind == "range-reconciliation" {
            let manifest = range_manifest.as_ref().ok_or(DurableError::Corrupt(
                "stored Record range manifest is absent",
            ))?;
            let manifest_cohort = manifest
                .cohorts
                .get(&(source_path.to_owned(), record_id.to_owned()))
                .ok_or(DurableError::Conflict(
                    "stored Record range owner is absent from terminal manifest",
                ))?;
            if manifest_cohort.owner_kind != "record"
                || manifest_cohort.basis_kind != basis_kind
                || manifest_cohort.source_revision != source_revision
                || manifest_cohort.source_event_id.as_deref() != Some(source_event_id.as_str())
                || manifest_cohort.source_event_version != Some(source_event_version)
                || manifest_cohort.predecessor_source_revision != Some(predecessor_source_revision)
                || manifest_cohort.predecessor_cohort_root != predecessor_cohort_root
                || manifest_cohort.basis_dependency_path != basis_dependency_path
                || manifest_cohort.basis_dependency_sha256 != basis_dependency_sha256
                || manifest_cohort.basis_history_path != basis_history_path
                || manifest_cohort.basis_history_sha256 != basis_history_sha256
                || manifest_cohort.basis_claim_id != basis_claim_id
                || manifest_cohort.basis_revision_proof_root != basis_revision_proof_root
                || manifest_cohort.claim_owner_inventory_root.is_some()
                || manifest_cohort.claim_stream_rows_scanned.is_some()
                || manifest_cohort.claim_event_basis_root.is_some()
                || manifest_cohort.claim_navigation_manifest_root.is_some()
                || manifest_cohort.claim_source_line.is_some()
                || manifest_cohort.claim_trace_sha256.is_some()
                || manifest_cohort.input_root != input_root
                || manifest_cohort.dependency_root != dependency_root
                || manifest_cohort.output_root != output_root
            {
                return Err(DurableError::Conflict(
                    "stored Record range cohort differs from terminal manifest",
                ));
            }
            (None, None, None, None, None, None, None, None, None, None)
        } else if basis_kind == "dependency-refresh-set" {
            let coverage = tx
                .query_opt(
                    "SELECT cohort_count,cohort_set_root,cohort_manifest
                       FROM cmd2_projection_navigation_coverage_event
                      WHERE domain=$1 AND consumer_id=$2 AND event_sequence=$3 AND event_id=$4",
                    &[&domain, &consumer_id, &as_i64(event_sequence)?, &event_id],
                )?
                .ok_or(DurableError::Conflict(
                    "stored Record refresh set has no coverage manifest",
                ))?;
            let stored_count = u32::try_from(coverage.get::<_, i32>("cohort_count"))
                .map_err(|_| DurableError::Corrupt("navigation cohort manifest count"))?;
            let stored_root = coverage
                .get::<_, Option<String>>("cohort_set_root")
                .map(parse_digest)
                .transpose()?
                .ok_or(DurableError::Conflict(
                    "stored Record refresh-set coverage root is absent",
                ))?;
            let manifest_raw = coverage
                .get::<_, Option<Vec<u8>>>("cohort_manifest")
                .ok_or(DurableError::Conflict(
                    "stored Record refresh-set coverage manifest is absent",
                ))?;
            let manifest = verify_navigation_event_cohort_manifest(
                &manifest_raw,
                stored_root,
                stored_count,
                &event_id,
                event_sequence,
                limits.max_row_bytes as usize,
            )?;
            let manifest_cohort = manifest
                .cohorts
                .get(&(source_path.to_owned(), record_id.to_owned()))
                .ok_or(DurableError::Conflict(
                    "stored Record refresh-set owner is absent from coverage manifest",
                ))?;
            let causes_match_refresh =
                manifest
                    .claim_refresh_set
                    .as_ref()
                    .is_some_and(|refresh_set| {
                        basis_dependency_causes.iter().all(|cause| {
                            refresh_set.causes.iter().any(|event_cause| {
                                event_cause.changed_dependency_path == cause.path.as_str()
                                    && event_cause.changed_dependency_sha256 == cause.sha256
                            })
                        })
                    });
            let causes_match_empty = manifest.claim_empty_closure.as_ref().is_some_and(|empty| {
                basis_dependency_causes.iter().all(|cause| {
                    empty
                        .covered_source_paths
                        .iter()
                        .any(|covered| covered == cause.path.as_str())
                })
            });
            if manifest_cohort.owner_kind != "record"
                || manifest_cohort.basis_kind != basis_kind
                || manifest_cohort.source_revision != source_revision
                || manifest_cohort.basis_dependency_path != basis_dependency_path
                || manifest_cohort.basis_dependency_sha256 != basis_dependency_sha256
                || manifest_cohort.basis_dependency_causes != basis_dependency_causes
                || manifest_cohort.basis_dependency_causes_root != basis_dependency_causes_root
                || manifest_cohort.basis_history_path != basis_history_path
                || manifest_cohort.basis_history_sha256 != basis_history_sha256
                || manifest_cohort.basis_claim_id != basis_claim_id
                || manifest_cohort.basis_revision_proof_root != basis_revision_proof_root
                || manifest_cohort.claim_owner_inventory_root.is_some()
                || manifest_cohort.claim_stream_rows_scanned.is_some()
                || manifest_cohort.claim_event_basis_root.is_some()
                || manifest_cohort.claim_navigation_manifest_root.is_some()
                || manifest_cohort.claim_source_line.is_some()
                || manifest_cohort.claim_trace_sha256.is_some()
                || manifest_cohort.input_root != input_root
                || manifest_cohort.dependency_root != dependency_root
                || manifest_cohort.output_root != output_root
                || !causes_match_refresh && !causes_match_empty
            {
                return Err(DurableError::Conflict(
                    "stored Record refresh-set basis differs from its event closure",
                ));
            }
            (None, None, None, None, None, None, None, None, None, None)
        } else if basis_kind == "claim-range-reconciliation" {
            let manifest = range_manifest.as_ref().ok_or(DurableError::Corrupt(
                "stored Claim range manifest is absent",
            ))?;
            let manifest_cohort = manifest
                .cohorts
                .get(&(source_path.to_owned(), record_id.to_owned()))
                .ok_or(DurableError::Conflict(
                    "stored Claim range owner is absent from terminal manifest",
                ))?;
            let range_set = manifest
                .claim_range_set
                .as_ref()
                .ok_or(DurableError::Conflict(
                    "stored Claim range has no terminal inventory proof",
                ))?;
            let local_stream = range_set
                .terminal_streams
                .iter()
                .find(|stream| stream.source_path == source_path)
                .ok_or(DurableError::Conflict(
                    "stored Claim range stream is absent from terminal inventory",
                ))?;
            if manifest_cohort.owner_kind != "claim"
                || manifest_cohort.basis_kind != basis_kind
                || manifest_cohort.source_revision != source_revision
                || manifest_cohort.source_event_id.as_deref() != Some(source_event_id.as_str())
                || manifest_cohort.source_event_version != Some(source_event_version)
                || manifest_cohort.predecessor_source_revision != Some(predecessor_source_revision)
                || manifest_cohort.predecessor_cohort_root != predecessor_cohort_root
                || manifest_cohort.input_root != input_root
                || manifest_cohort.dependency_root != dependency_root
                || manifest_cohort.output_root != output_root
                || manifest_cohort.claim_owner_inventory_root
                    != Some(range_set.owner_inventory_root)
                || manifest_cohort.claim_stream_rows_scanned != Some(local_stream.rows_scanned)
                || manifest_cohort.claim_event_basis_root.is_none()
                || manifest_cohort.claim_navigation_manifest_root
                    != Some(range_set.range_manifest_root)
                || manifest_cohort.claim_source_line.is_none()
                || manifest_cohort.claim_trace_sha256.is_none()
                || local_stream
                    .owner_claim_ids
                    .binary_search_by(|id| id.as_str().cmp(record_id))
                    .is_err()
                || range_set
                    .terminal_owner_ids
                    .binary_search_by(|id| id.as_str().cmp(record_id))
                    .is_err()
                || basis_revision_proof_root.is_some()
            {
                return Err(DurableError::Conflict(
                    "stored Claim range cohort differs from terminal manifest",
                ));
            }
            (
                manifest_cohort.claim_source_line,
                manifest_cohort.claim_trace_sha256,
                None,
                None,
                None,
                Some(local_stream.stream_sha256),
                manifest_cohort.claim_owner_inventory_root,
                manifest_cohort.claim_stream_rows_scanned,
                manifest_cohort.claim_event_basis_root,
                manifest_cohort.claim_navigation_manifest_root,
            )
        } else if matches!(
            basis_kind.as_str(),
            "claim-mutation" | "claim-dependency-refresh"
        ) {
            let coverage = tx
                .query_opt(
                    "SELECT cohort_count,cohort_set_root,cohort_manifest
                       FROM cmd2_projection_navigation_coverage_event
                      WHERE domain=$1 AND consumer_id=$2 AND event_sequence=$3 AND event_id=$4",
                    &[&domain, &consumer_id, &as_i64(event_sequence)?, &event_id],
                )?
                .ok_or(DurableError::Conflict(
                    "stored Claim cohort has no coverage manifest",
                ))?;
            let stored_count = u32::try_from(coverage.get::<_, i32>("cohort_count"))
                .map_err(|_| DurableError::Corrupt("navigation cohort manifest count"))?;
            let stored_root = coverage
                .get::<_, Option<String>>("cohort_set_root")
                .map(parse_digest)
                .transpose()?
                .ok_or(DurableError::Conflict(
                    "stored Claim coverage root is absent",
                ))?;
            let manifest_raw = coverage
                .get::<_, Option<Vec<u8>>>("cohort_manifest")
                .ok_or(DurableError::Conflict(
                    "stored Claim coverage manifest is absent",
                ))?;
            let manifest = verify_navigation_event_cohort_manifest(
                &manifest_raw,
                stored_root,
                stored_count,
                &event_id,
                event_sequence,
                limits.max_row_bytes as usize,
            )?;
            let manifest_cohort = manifest
                .cohorts
                .get(&(source_path.to_owned(), record_id.to_owned()))
                .ok_or(DurableError::Conflict(
                    "stored Claim owner is absent from coverage manifest",
                ))?;
            if manifest_cohort.owner_kind != "claim"
                || manifest_cohort.basis_kind != basis_kind
                || manifest_cohort.source_revision != source_revision
                || manifest_cohort.basis_dependency_path != basis_dependency_path
                || manifest_cohort.basis_dependency_sha256 != basis_dependency_sha256
                || manifest_cohort.basis_dependency_causes != basis_dependency_causes
                || manifest_cohort.basis_dependency_causes_root != basis_dependency_causes_root
                || manifest_cohort.basis_history_path != basis_history_path
                || manifest_cohort.basis_history_sha256 != basis_history_sha256
                || manifest_cohort.basis_claim_id != basis_claim_id
                || manifest_cohort.basis_revision_proof_root != basis_revision_proof_root
                || manifest_cohort.input_root != input_root
                || manifest_cohort.dependency_root != dependency_root
                || manifest_cohort.output_root != output_root
                || manifest_cohort.claim_owner_inventory_root != claim_owner_inventory_root
                || manifest_cohort.claim_stream_rows_scanned != claim_stream_rows_scanned
                || manifest_cohort.claim_event_basis_root != claim_event_basis_root
                || manifest_cohort.claim_navigation_manifest_root != claim_navigation_manifest_root
            {
                return Err(DurableError::Conflict(
                    "stored Claim cohort differs from its coverage manifest",
                ));
            }
            let source_line = manifest_cohort.claim_source_line;
            let trace_sha256 = manifest_cohort.claim_trace_sha256;
            if basis_kind == "claim-mutation" {
                let claim_set = manifest.claim_set.ok_or(DurableError::Conflict(
                    "stored Claim coverage lacks a mutation owner manifest",
                ))?;
                let local_stream = claim_set
                    .streams
                    .iter()
                    .find(|stream| stream.source_path == source_path)
                    .ok_or(DurableError::Conflict(
                        "stored Claim owner stream is absent from mutation set",
                    ))?;
                if local_stream
                    .owner_claim_ids
                    .binary_search(&record_id.to_owned())
                    .is_err()
                    || claim_set
                        .owner_claim_ids
                        .binary_search(&record_id.to_owned())
                        .is_err()
                    || manifest_cohort.claim_event_basis_root
                        != Some(claim_set.event_claim_set_root)
                    || manifest_cohort.claim_navigation_manifest_root
                        != claim_set.affected_owner_manifest_root
                {
                    return Err(DurableError::Conflict(
                        "stored Claim owner differs from mutation stream manifest",
                    ));
                }
                (
                    source_line,
                    trace_sha256,
                    Some(claim_set.event_claim_set_root),
                    Some(claim_set.mutation_receipt_path),
                    Some(claim_set.mutation_receipt_sha256),
                    Some(local_stream.stream_sha256),
                    manifest_cohort.claim_owner_inventory_root,
                    manifest_cohort.claim_stream_rows_scanned,
                    manifest_cohort.claim_event_basis_root,
                    manifest_cohort.claim_navigation_manifest_root,
                )
            } else {
                let refresh_set = manifest.claim_refresh_set.ok_or(DurableError::Conflict(
                    "stored Claim coverage lacks a dependency-refresh manifest",
                ))?;
                let local_stream = refresh_set
                    .streams
                    .iter()
                    .find(|stream| stream.source_path == source_path)
                    .ok_or(DurableError::Conflict(
                        "stored Claim owner stream is absent from refresh set",
                    ))?;
                if local_stream
                    .owner_claim_ids
                    .binary_search(&record_id.to_owned())
                    .is_err()
                    || manifest_cohort.claim_navigation_manifest_root
                        != Some(refresh_set.affected_owner_manifest_root)
                {
                    return Err(DurableError::Conflict(
                        "stored Claim owner differs from refresh stream manifest",
                    ));
                }
                (
                    source_line,
                    trace_sha256,
                    None,
                    None,
                    None,
                    Some(local_stream.stream_sha256),
                    manifest_cohort.claim_owner_inventory_root,
                    manifest_cohort.claim_stream_rows_scanned,
                    manifest_cohort.claim_event_basis_root,
                    manifest_cohort.claim_navigation_manifest_root,
                )
            }
        } else {
            (None, None, None, None, None, None, None, None, None, None)
        };
        let dependencies =
            read_dependencies(&mut tx, domain, consumer_id, source_path, record_id, limits)?;
        let (nodes, node_visibility) = read_rows(
            &mut tx,
            "cmd2_projection_navigation_node",
            "node_id",
            domain,
            consumer_id,
            source_path,
            record_id,
            limits,
        )?;
        let (edges, edge_visibility) = read_rows(
            &mut tx,
            "cmd2_projection_navigation_edge",
            "edge_id",
            domain,
            consumer_id,
            source_path,
            record_id,
            limits,
        )?;
        let diagnostics =
            read_diagnostics(&mut tx, domain, consumer_id, source_path, record_id, limits)?;
        tx.commit()?;
        let read = DurableNavigationEventCohortReadV1 {
            event_id,
            event_sequence,
            source_event_id,
            source_event_version,
            predecessor_source_revision,
            source_revision,
            basis_kind,
            basis_dependency_path,
            basis_dependency_sha256,
            basis_dependency_causes: basis_dependency_causes
                .iter()
                .map(|cause| DurableNavigationDependencyRefreshCauseReadV1 {
                    path: cause.path.as_str().to_owned(),
                    sha256: cause.sha256,
                })
                .collect(),
            basis_dependency_causes_root,
            basis_history_path,
            basis_history_sha256,
            basis_claim_id,
            basis_revision_proof_root,
            claim_source_line,
            claim_trace_sha256,
            claim_event_set_root,
            claim_mutation_receipt_path,
            claim_mutation_receipt_sha256,
            claim_stream_sha256,
            claim_owner_inventory_root: manifest_claim_owner_inventory_root,
            claim_stream_rows_scanned: manifest_claim_stream_rows_scanned,
            claim_event_basis_root: manifest_claim_event_basis_root,
            claim_navigation_manifest_root: manifest_claim_navigation_root,
            source_path: source_path.to_owned(),
            record_id: record_id.to_owned(),
            predecessor_cohort_root,
            dependency_root,
            input_root,
            output_root,
            dependencies,
            nodes,
            node_visibility,
            edges,
            edge_visibility,
            diagnostics,
        };
        verify_read_roots(&read, limits)?;
        Ok(Some(read))
    }

    fn publish_validated_navigation_cohort(
        &mut self,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        cohort: &NavigationRecordEventCohortV1,
        rows: PersistedRows,
        witness: &EventWitness,
        limits: DurableNavigationEventLimits,
        deadline: Instant,
        allow_historical_cut: bool,
        cohort_manifest_raw: Vec<u8>,
    ) -> DurableResult<DurableNavigationEventApply> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = lock_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        if witness.event.commit_seq > snapshot.head_seq()?
            || (!allow_historical_cut && witness.event.commit_seq != snapshot.head_seq()?)
        {
            return Err(DurableError::Conflict(
                "navigation event is outside the accepted CMD source cut",
            ));
        }
        let profile_version = profile.version.to_string();
        let source_contract = profile.source_contract.to_hex();
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_consumer(
                domain,consumer_id,profile_id,profile_version,source_contract_digest)
             VALUES($1,$2,$3,$4,$5) ON CONFLICT(domain,consumer_id) DO NOTHING",
            &[
                &domain,
                &consumer_id,
                &profile.id,
                &profile_version,
                &source_contract,
            ],
        )?;
        let cursor_row = tx.query_one(
            "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,
                    source_revision,last_event_id,last_input_root,last_output_root,
                    coverage_seq,coverage_root,adjacency_seq,adjacency_root,adjacency_ready,
                    verified_prefix_version,verified_prefix_seq,verified_prefix_coverage_root,
                    verified_prefix_cohort_root,verified_prefix_digest
             FROM cmd2_projection_navigation_consumer
             WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?;
        if cursor_row.get::<_, String>("profile_id") != profile.id
            || cursor_row.get::<_, String>("profile_version") != profile_version
            || cursor_row.get::<_, String>("source_contract_digest") != source_contract
        {
            return Err(DurableError::Conflict(
                "navigation event consumer profile differs",
            ));
        }
        let cursor = as_u64(cursor_row.get("cursor_seq"))?;
        let sequence = cohort.event_sequence;
        if cursor == sequence {
            let last_event_id: Option<String> = cursor_row.get("last_event_id");
            let last_input_root: Option<String> = cursor_row.get("last_input_root");
            let last_output_root: Option<String> = cursor_row.get("last_output_root");
            if last_event_id.as_deref() == Some(cohort.event_id.as_str())
                && last_input_root.as_deref() == Some(cohort.input_root.to_hex().as_str())
                && last_output_root.as_deref() == Some(cohort.output_root.to_hex().as_str())
                && as_u64(cursor_row.get("coverage_seq"))? == sequence
                && as_u64(cursor_row.get("adjacency_seq"))? == sequence
                && cursor_row.get::<_, bool>("adjacency_ready")
            {
                if let Some(bootstrap) = tx.query_opt(
                    "SELECT through_seq,bootstrap_root,owner_binding_json
                       FROM cmd2_projection_navigation_bootstrap
                      WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
                    &[&domain, &consumer_id],
                )? {
                    verify_navigation_verified_prefix_in_tx(
                        &mut tx,
                        domain,
                        consumer_id,
                        profile,
                        as_u64(bootstrap.get("through_seq"))?,
                        parse_digest(bootstrap.get("bootstrap_root"))?,
                        &bootstrap.get::<_, String>("owner_binding_json"),
                        &cursor_row,
                        limits,
                    )?;
                } else if cursor_row.get::<_, Option<i16>>("verified_prefix_version")
                    == Some(VERIFIED_PREFIX_FORMAT_VERSION)
                {
                    return Err(DurableError::Conflict(
                        "navigation verified-prefix replay requires its bootstrap",
                    ));
                }
                tx.commit()?;
                return Ok(DurableNavigationEventApply {
                    event_sequence: sequence,
                    event_id: cohort.event_id.clone(),
                    output_root: cohort.output_root,
                    replayed: true,
                });
            }
            return Err(DurableError::Conflict(
                "navigation event cursor already owns a different cohort",
            ));
        }
        if cursor.checked_add(1) != Some(sequence) {
            return Err(DurableError::Conflict(
                "navigation event cursor is not the predecessor",
            ));
        }
        if cursor > 0 {
            let predecessor_revision = cursor_row
                .get::<_, Option<String>>("source_revision")
                .ok_or(DurableError::Corrupt(
                    "navigation event cursor source revision absent",
                ))?;
            if predecessor_revision.trim_end() != cohort.predecessor_source_revision.to_hex() {
                return Err(DurableError::Conflict(
                    "navigation predecessor source revision differs",
                ));
            }
        } else if sequence != 1
            || cursor_row
                .get::<_, Option<String>>("source_revision")
                .is_some()
        {
            return Err(DurableError::Conflict(
                "navigation event cursor has no trusted predecessor baseline",
            ));
        }

        let bootstrap = tx.query_opt(
            "SELECT through_seq,bootstrap_root,owner_binding_json FROM cmd2_projection_navigation_bootstrap
             WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?;
        let checkpoint_bootstrap = if let Some(row) = bootstrap.as_ref() {
            Some((
                as_u64(row.get("through_seq"))?,
                parse_digest(row.get("bootstrap_root"))?,
                row.get::<_, String>("owner_binding_json"),
            ))
        } else {
            None
        };
        if let Some((bootstrap_sequence, bootstrap_root, owner_binding_json)) =
            checkpoint_bootstrap.as_ref()
        {
            verify_navigation_verified_prefix_in_tx(
                &mut tx,
                domain,
                consumer_id,
                profile,
                *bootstrap_sequence,
                *bootstrap_root,
                owner_binding_json,
                &cursor_row,
                limits,
            )?;
        }
        if allow_historical_cut && bootstrap.is_none() {
            return Err(DurableError::Conflict(
                "historical navigation publication requires a complete bootstrap",
            ));
        }
        let next_coverage = if let Some(bootstrap) = bootstrap {
            let bootstrap_sequence = as_u64(bootstrap.get("through_seq"))?;
            let coverage_sequence = as_u64(cursor_row.get("coverage_seq"))?;
            let previous_coverage_root = cursor_row
                .get::<_, Option<String>>("coverage_root")
                .ok_or(DurableError::Conflict(
                    "navigation coverage root is absent before event update",
                ))
                .and_then(parse_digest)?;
            if bootstrap_sequence > cursor || coverage_sequence != cursor {
                return Err(DurableError::Conflict(
                    "navigation event cursor lacks complete bootstrap coverage",
                ));
            }
            let adjacency_sequence = as_u64(cursor_row.get("adjacency_seq"))?;
            let previous_adjacency_root = cursor_row
                .get::<_, Option<String>>("adjacency_root")
                .ok_or(DurableError::Conflict(
                    "navigation adjacency root is absent before event update",
                ))
                .and_then(parse_digest)?;
            if adjacency_sequence != cursor || !cursor_row.get::<_, bool>("adjacency_ready") {
                return Err(DurableError::Conflict(
                    "navigation adjacency cursor is not the current predecessor",
                ));
            }
            let changed_paths = witness
                .members
                .iter()
                .filter(|member| member.is_source)
                .map(|member| member.source_path.clone())
                .collect::<Vec<_>>();
            verify_navigation_reverse_dependency_closure(
                &mut tx,
                domain,
                consumer_id,
                cohort.source_path.as_str(),
                &cohort.record_id,
                &changed_paths,
            )?;
            verify_navigation_endpoint_closure(
                &mut tx,
                domain,
                consumer_id,
                cohort.source_path.as_str(),
                &cohort.record_id,
                &rows.nodes,
            )?;
            let (adjacency_updates, adjacency_root) = prepare_navigation_adjacency_update(
                &mut tx,
                domain,
                consumer_id,
                cohort.source_path.as_str(),
                &cohort.record_id,
                parse_digest(bootstrap.get("bootstrap_root"))?,
                previous_adjacency_root,
                sequence,
                &rows,
                limits,
            )?;
            let cohort_manifest_sha256 = Digest256::of_bytes(&cohort_manifest_raw);
            Some((
                previous_coverage_root,
                navigation_coverage_root_next(
                    previous_coverage_root,
                    sequence,
                    &cohort.event_id,
                    cohort.source_path.as_str(),
                    &cohort.record_id,
                    cohort.source_revision,
                    cohort.input_root,
                    cohort.dependency_root,
                    cohort.output_root,
                    cohort_manifest_sha256,
                ),
                adjacency_updates,
                adjacency_root,
                cohort_manifest_raw,
                cohort_manifest_sha256,
            ))
        } else {
            None
        };

        let existing_root = tx.query_opt(
            "SELECT output_root FROM cmd2_projection_navigation_cohort
             WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4 FOR UPDATE",
            &[
                &domain,
                &consumer_id,
                &cohort.source_path.as_str(),
                &cohort.record_id,
            ],
        )?;
        match existing_root {
            Some(row) => {
                let output_root = parse_digest(row.get("output_root"))?;
                if cohort.predecessor_cohort_root != Some(output_root) {
                    return Err(DurableError::Conflict(
                        "navigation record predecessor root differs",
                    ));
                }
            }
            None if cohort.predecessor_cohort_root.is_some() => {
                return Err(DurableError::Conflict(
                    "navigation predecessor cohort is absent",
                ));
            }
            None => {}
        }

        tx.execute(
            "DELETE FROM cmd2_projection_navigation_cohort
             WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4",
            &[
                &domain,
                &consumer_id,
                &cohort.source_path.as_str(),
                &cohort.record_id,
            ],
        )?;
        let coverage_manifest_sha256 = next_coverage.as_ref().map(|coverage| coverage.5.to_hex());
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_cohort(
                domain,consumer_id,source_path,record_id,event_id,event_sequence,
                source_event_id,source_event_version,
                predecessor_source_revision,source_revision,predecessor_cohort_root,
                dependency_root,input_root,output_root,dependency_count,node_count,
                edge_count,diagnostic_count,cohort_bytes,coverage_manifest_sha256)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)",
            &[
                &domain,
                &consumer_id,
                &cohort.source_path.as_str(),
                &cohort.record_id,
                &cohort.event_id,
                &as_i64(sequence)?,
                &cohort.source_event_id,
                &as_i64(cohort.source_event_version)?,
                &cohort.predecessor_source_revision.to_hex(),
                &cohort.source_revision.to_hex(),
                &cohort.predecessor_cohort_root.map(Digest256::to_hex),
                &cohort.dependency_root.to_hex(),
                &cohort.input_root.to_hex(),
                &cohort.output_root.to_hex(),
                &i32::try_from(rows.dependencies.len())
                    .map_err(|_| DurableError::Refused("navigation dependency count"))?,
                &i32::try_from(rows.nodes.len())
                    .map_err(|_| DurableError::Refused("navigation node count"))?,
                &i32::try_from(rows.edges.len())
                    .map_err(|_| DurableError::Refused("navigation edge count"))?,
                &i32::try_from(rows.diagnostics.len())
                    .map_err(|_| DurableError::Refused("navigation diagnostic count"))?,
                &as_i64(rows.cohort_bytes)?,
                &coverage_manifest_sha256,
            ],
        )?;
        for dependency in &rows.dependencies {
            let (state, sha256, size_bytes, absent_reason) = match &dependency.state {
                NavigationDependencyStateV1::Present { sha256, size_bytes } => (
                    "present",
                    Some(sha256.to_hex()),
                    Some(as_i64(*size_bytes)?),
                    None,
                ),
                NavigationDependencyStateV1::Absent { reason } => {
                    ("absent", None, None, Some(reason.as_str()))
                }
            };
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_dependency(
                    domain,consumer_id,source_path,record_id,dependency_path,
                    dependency_role,observation_state,sha256,size_bytes,absent_reason)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                &[
                    &domain,
                    &consumer_id,
                    &cohort.source_path.as_str(),
                    &cohort.record_id,
                    &dependency.path.as_str(),
                    &dependency_role(dependency.role),
                    &state,
                    &sha256,
                    &size_bytes,
                    &absent_reason,
                ],
            )?;
        }
        for node in &rows.nodes {
            let row_json = std::str::from_utf8(&node.row_json)
                .map_err(|_| DurableError::Invalid("navigation event node carrier UTF-8"))?;
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_node(
                    domain,consumer_id,source_path,record_id,node_id,visible,row_sha256,row_json)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
                &[
                    &domain,
                    &consumer_id,
                    &cohort.source_path.as_str(),
                    &cohort.record_id,
                    &node.node_id,
                    &node.visible,
                    &node.sha256.to_hex(),
                    &row_json,
                ],
            )?;
        }
        for edge in &rows.edges {
            let row_json = std::str::from_utf8(&edge.row_json)
                .map_err(|_| DurableError::Invalid("navigation event edge carrier UTF-8"))?;
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_edge(
                    domain,consumer_id,source_path,record_id,edge_id,from_id,to_id,
                    visible,row_sha256,row_json)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                &[
                    &domain,
                    &consumer_id,
                    &cohort.source_path.as_str(),
                    &cohort.record_id,
                    &edge.edge_id,
                    &edge.from_id,
                    &edge.to_id,
                    &edge.visible,
                    &edge.sha256.to_hex(),
                    &row_json,
                ],
            )?;
        }
        for (ordinal, row_json) in rows.diagnostics.iter().enumerate() {
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_diagnostic(
                    domain,consumer_id,source_path,record_id,diagnostic_ordinal,row_json)
                 VALUES($1,$2,$3,$4,$5,$6)",
                &[
                    &domain,
                    &consumer_id,
                    &cohort.source_path.as_str(),
                    &cohort.record_id,
                    &i32::try_from(ordinal)
                        .map_err(|_| DurableError::Refused("navigation diagnostic ordinal"))?,
                    &String::from_utf8_lossy(row_json).as_ref(),
                ],
            )?;
        }
        if let Some((
            previous_coverage_root,
            coverage_root,
            adjacency_updates,
            _,
            manifest_raw,
            manifest_root,
        )) = &next_coverage
        {
            tx.execute(
                "UPDATE cmd2_projection_navigation_bootstrap_row
                 SET active=false
                 WHERE domain=$1 AND consumer_id=$2 AND owner_source_path=$3
                   AND owner_record_id=$4",
                &[
                    &domain,
                    &consumer_id,
                    &cohort.source_path.as_str(),
                    &cohort.record_id,
                ],
            )?;
            tx.execute(
                "INSERT INTO cmd2_projection_navigation_coverage_event(
                    domain,consumer_id,event_sequence,event_id,source_path,record_id,
                    source_revision,input_root,dependency_root,output_root,
                    previous_coverage_root,coverage_root,cohort_count,cohort_set_root,
                    cohort_manifest)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,1,$13,$14)",
                &[
                    &domain,
                    &consumer_id,
                    &as_i64(sequence)?,
                    &cohort.event_id,
                    &cohort.source_path.as_str(),
                    &cohort.record_id,
                    &cohort.source_revision.to_hex(),
                    &cohort.input_root.to_hex(),
                    &cohort.dependency_root.to_hex(),
                    &cohort.output_root.to_hex(),
                    &previous_coverage_root.to_hex(),
                    &coverage_root.to_hex(),
                    &manifest_root.to_hex(),
                    manifest_raw,
                ],
            )?;
        }
        if let Some((_, _, adjacency_updates, _, _, _)) = &next_coverage {
            let affected_ids = adjacency_updates.keys().cloned().collect::<Vec<_>>();
            if !affected_ids.is_empty() {
                tx.execute(
                    "DELETE FROM cmd2_projection_navigation_adjacency_certificate
                     WHERE domain=$1 AND consumer_id=$2 AND node_id=ANY($3)",
                    &[&domain, &consumer_id, &affected_ids],
                )?;
            }
            for certificate in adjacency_updates.values().flatten() {
                tx.execute(
                    "INSERT INTO cmd2_projection_navigation_adjacency_certificate(
                        domain,consumer_id,node_id,edge_count,edge_digest,certificate_root)
                     VALUES($1,$2,$3,$4,$5,$6)",
                    &[
                        &domain,
                        &consumer_id,
                        &certificate.node_id,
                        &as_i64(certificate.edge_count)?,
                        &certificate.edge_digest.to_hex(),
                        &certificate.certificate_root.to_hex(),
                    ],
                )?;
            }
        }
        active_navigation(deadline)?;
        let changed = if let Some((_, coverage_root, _, adjacency_root, _, _)) = next_coverage {
            let cohort_root =
                navigation_current_cohort_root_in_tx(&mut tx, domain, consumer_id, limits)?;
            let (bootstrap_sequence, bootstrap_root, owner_binding_json) =
                checkpoint_bootstrap.as_ref().ok_or(DurableError::Conflict(
                    "navigation checkpoint lacks bootstrap",
                ))?;
            let checkpoint_digest = navigation_verified_prefix_digest(
                domain,
                consumer_id,
                profile,
                *bootstrap_sequence,
                *bootstrap_root,
                owner_binding_json,
                sequence,
                coverage_root,
                cohort_root,
                adjacency_root,
                &cohort.event_id,
                cohort.source_revision,
                cohort.input_root,
                cohort.output_root,
            );
            tx.execute(
                "UPDATE cmd2_projection_navigation_consumer
                 SET cursor_seq=$3,source_revision=$4,last_event_id=$5,
                     last_input_root=$6,last_output_root=$7,
                     coverage_seq=$3,coverage_root=$8,
                     adjacency_seq=$3,adjacency_root=$9,adjacency_ready=true,
                     verified_prefix_version=$14,
                     verified_prefix_seq=$3,verified_prefix_coverage_root=$8,
                     verified_prefix_cohort_root=$10,verified_prefix_digest=$11
                 WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=$12
                   AND adjacency_seq=$13",
                &[
                    &domain,
                    &consumer_id,
                    &as_i64(sequence)?,
                    &cohort.source_revision.to_hex(),
                    &cohort.event_id,
                    &cohort.input_root.to_hex(),
                    &cohort.output_root.to_hex(),
                    &coverage_root.to_hex(),
                    &adjacency_root.to_hex(),
                    &cohort_root.to_hex(),
                    &checkpoint_digest.to_hex(),
                    &as_i64(cursor)?,
                    &as_i64(cursor)?,
                    &VERIFIED_PREFIX_FORMAT_VERSION,
                ],
            )?
        } else {
            tx.execute(
                "UPDATE cmd2_projection_navigation_consumer
                 SET cursor_seq=$3,source_revision=$4,last_event_id=$5,
                     last_input_root=$6,last_output_root=$7
                 WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=$8",
                &[
                    &domain,
                    &consumer_id,
                    &as_i64(sequence)?,
                    &cohort.source_revision.to_hex(),
                    &cohort.event_id,
                    &cohort.input_root.to_hex(),
                    &cohort.output_root.to_hex(),
                    &as_i64(cursor)?,
                ],
            )?
        };
        if changed != 1 {
            return Err(DurableError::Conflict(
                "navigation event cursor compare-and-set failed",
            ));
        }
        tx.commit()?;
        Ok(DurableNavigationEventApply {
            event_sequence: sequence,
            event_id: cohort.event_id.clone(),
            output_root: cohort.output_root,
            replayed: false,
        })
    }
}

fn resolve_event_witness(
    client: &mut postgres::Client,
    domain: &str,
    sequence: u64,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
) -> DurableResult<EventWitness> {
    if sequence == 0 {
        return Err(DurableError::Invalid("navigation event sequence"));
    }
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let snapshot = read_domain_snapshot(&mut tx, domain)?;
    snapshot.require_live_projection(profile)?;
    let source_head_sequence = snapshot.head_seq()?;
    if sequence > source_head_sequence {
        return Err(DurableError::Conflict(
            "navigation event exceeds source head",
        ));
    }
    let row = tx
        .query_opt(
            "SELECT l.event_kind,l.command_id,l.members_root,o.event_id
             FROM cmd2_log l JOIN cmd2_outbox o USING(domain,commit_seq)
             WHERE l.domain=$1 AND l.commit_seq=$2",
            &[&domain, &as_i64(sequence)?],
        )?
        .ok_or(DurableError::Corrupt(
            "navigation event lacks committed log/outbox pair",
        ))?;
    let event_kind: String = row.get("event_kind");
    if !matches!(event_kind.as_str(), "command" | "rights" | "rule") {
        return Err(DurableError::Conflict(
            "navigation event kind is unsupported",
        ));
    }
    let event_id: String = row.get("event_id");
    let event_id: String = event_id.trim_end().to_owned();
    if event_id.is_empty() {
        return Err(DurableError::Corrupt("navigation event identity is empty"));
    }
    let event = ProjectionEvent {
        commit_seq: sequence,
        event_kind,
    };
    let empty_members_root = Digest256::of_bytes(b"").to_hex();
    let command_members_root: String = row.get("members_root");
    if event.event_kind != "command" && command_members_root != empty_members_root {
        return Err(DurableError::Corrupt(
            "non-source navigation event has a non-empty member commitment",
        ));
    }
    tx.commit()?;
    let projection_limits = DurableProjectionLimits {
        max_events: 1,
        max_event_members: 64,
        max_event_documents: limits.max_cohort_rows,
        max_event_bytes: limits.max_total_bytes,
        max_document_bytes: limits.max_row_bytes,
        max_dependencies: 1,
        max_aliases: 0,
        max_query_bytes: 1,
        max_results: 1,
        statement_timeout: limits.statement_timeout,
    };
    let members = super::event_members(client, domain, &event, projection_limits)?;
    let expected_id = format!("{domain}:{sequence}");
    if event.event_kind == "command" && event_id != expected_id {
        return Err(DurableError::Corrupt(
            "navigation event outbox identity differs",
        ));
    }
    if event.event_kind == "rights" && event_id != format!("rights.revoke:{sequence}") {
        return Err(DurableError::Corrupt(
            "navigation rights event identity differs",
        ));
    }
    if event.event_kind != "command" && !members.is_empty() {
        return Err(DurableError::Corrupt(
            "classified no-effect navigation event has source members",
        ));
    }
    Ok(EventWitness {
        event_id,
        event,
        source_head_sequence,
        members,
    })
}

fn read_selected_sources_at_cut(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    domain: &str,
    witness: &EventWitness,
    paths: Vec<String>,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
    event_path: &str,
    deadline: Instant,
) -> DurableResult<(Vec<DurableNavigationEventSourceV1>, u64)> {
    let mut unique = BTreeSet::new();
    let mut all_paths = Vec::new();
    for path in paths {
        validate_relative_path(&path)?;
        if unique.insert(path.clone()) {
            all_paths.push(path);
        }
    }
    for member in &witness.members {
        if unique.insert(member.source_path.clone()) {
            all_paths.push(member.source_path.clone());
        }
    }
    if all_paths.len() > limits.max_dependencies.saturating_add(64) {
        return Err(DurableError::Refused(
            "navigation event source selection count budget",
        ));
    }
    all_paths.sort();
    let (selected, source_head_sequence) = {
        let mut tx = coordinator
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        let source_head_sequence = snapshot.head_seq()?;
        if witness.event.commit_seq > snapshot.head_seq()? {
            return Err(DurableError::Conflict(
                "navigation event exceeded source head during selection",
            ));
        }
        let mut versions = BTreeMap::new();
        for path in &all_paths {
            versions.insert(
                path.clone(),
                selected_version_at_cut(&mut tx, domain, path, witness.event.commit_seq)?,
            );
        }
        tx.commit()?;
        (versions, source_head_sequence)
    };
    let members_by_path = witness
        .members
        .iter()
        .map(|member| (member.source_path.as_str(), member))
        .collect::<BTreeMap<_, _>>();
    let event_member_paths = members_by_path.keys().copied().collect::<BTreeSet<_>>();
    let mut total_bytes = 0u64;
    let mut output = Vec::with_capacity(selected.len());
    for (path, version) in selected {
        active_navigation(deadline)?;
        let source = match version {
            None => DurableNavigationEventSourceV1 {
                source_path: path.clone(),
                revision: None,
                commit_sequence: None,
                content_digest: None,
                content_length: None,
                is_source: false,
                exact_source_bytes: None,
            },
            Some(version) => {
                let event_member = members_by_path.get(path.as_str()).copied();
                if let Some(member) = event_member {
                    if member.revision != version.revision
                        || member.content_length != version.content_length
                        || member.is_source != version.is_source
                    {
                        return Err(DurableError::Corrupt(
                            "navigation event member differs from selected source cut",
                        ));
                    }
                }
                let exact_source_bytes = if version.is_source {
                    let source_byte_limit =
                        if path == event_path || event_member_paths.contains(path.as_str()) {
                            limits.max_total_bytes
                        } else {
                            limits.max_row_bytes as u64
                        };
                    if version.content_length == 0 || version.content_length > source_byte_limit {
                        return Err(DurableError::Refused("navigation event source byte budget"));
                    }
                    let mut source_work = super::DurableProjectionWork::default();
                    let mut event_bytes = 0;
                    let projection_limits = DurableProjectionLimits {
                        max_event_bytes: limits.max_total_bytes,
                        max_document_bytes: usize::try_from(source_byte_limit)
                            .map_err(|_| DurableError::Refused("navigation source byte limit"))?,
                        statement_timeout: limits.statement_timeout,
                        ..DurableProjectionLimits::default()
                    };
                    let source = super::read_projection_source(
                        coordinator,
                        store,
                        domain,
                        &path,
                        version.revision,
                        version.content_length,
                        version.commit_sequence,
                        projection_limits,
                        false,
                        deadline,
                        &mut event_bytes,
                        &mut source_work,
                    )?;
                    if Digest256::of_bytes(&source.exact_source_bytes) != version.content_digest {
                        return Err(DurableError::Corrupt(
                            "navigation event STO bytes differ from selected history digest",
                        ));
                    }
                    total_bytes = total_bytes.checked_add(version.content_length).ok_or(
                        DurableError::Refused("navigation event selected source byte overflow"),
                    )?;
                    if total_bytes > limits.max_total_bytes {
                        return Err(DurableError::Refused(
                            "navigation event selected source bytes exceed budget",
                        ));
                    }
                    Some(source.exact_source_bytes)
                } else {
                    None
                };
                DurableNavigationEventSourceV1 {
                    source_path: path.clone(),
                    revision: Some(version.revision),
                    commit_sequence: Some(version.commit_sequence),
                    content_digest: Some(version.content_digest),
                    content_length: Some(version.content_length),
                    is_source: version.is_source,
                    exact_source_bytes,
                }
            }
        };
        output.push(source);
    }
    Ok((output, source_head_sequence))
}

fn selected_version_at_cut<C: GenericClient>(
    client: &mut C,
    domain: &str,
    path: &str,
    sequence: u64,
) -> DurableResult<Option<SelectedVersion>> {
    let row = client.query_opt(
        "SELECT revision,commit_seq,content_length,content_digest,source_mode
         FROM cmd2_history WHERE domain=$1 AND subject=$2 AND commit_seq<=$3
         ORDER BY commit_seq DESC LIMIT 1",
        &[&domain, &path, &as_i64(sequence)?],
    )?;
    row.map(|row| -> DurableResult<SelectedVersion> {
        let content_digest: String = row.get("content_digest");
        Ok(SelectedVersion {
            revision: as_u64(row.get("revision"))?,
            commit_sequence: as_u64(row.get("commit_seq"))?,
            content_length: as_u64(row.get("content_length"))?,
            content_digest: Digest256::from_hex(content_digest.trim_end())
                .map_err(|_| DurableError::Corrupt("navigation source digest is malformed"))?,
            is_source: row.get::<_, Option<i32>>("source_mode").is_some(),
        })
    })
    .transpose()
}

fn verify_dependency_cut(
    cohort: &NavigationRecordEventCohortV1,
    selected: &[DurableNavigationEventSourceV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let selected = selected
        .iter()
        .map(|source| (source.source_path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    for dependency in &cohort.dependencies {
        let source =
            selected
                .get(dependency.path.as_str())
                .copied()
                .ok_or(DurableError::Corrupt(
                    "navigation dependency absent from selected event cut",
                ))?;
        match &dependency.state {
            NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                let raw = source
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation present dependency lacks exact source bytes",
                    ))?;
                if !source.is_source
                    || source.content_digest != Some(*sha256)
                    || source.content_length != Some(*size_bytes)
                    || *size_bytes
                        > if dependency.role == NavigationDependencyRoleV1::Event {
                            limits.max_total_bytes
                        } else {
                            limits.max_row_bytes as u64
                        }
                    || raw.len() as u64 != *size_bytes
                    || Digest256::of_bytes(raw) != *sha256
                {
                    return Err(DurableError::Conflict(
                        "navigation dependency differs from selected exact source",
                    ));
                }
            }
            NavigationDependencyStateV1::Absent { .. } => {
                if source.is_source {
                    return Err(DurableError::Conflict(
                        "navigation absent dependency is present at event cut",
                    ));
                }
            }
        }
    }
    let event_member = selected
        .get(
            cohort
                .dependencies
                .iter()
                .find(|dependency| dependency.role == NavigationDependencyRoleV1::Event)
                .ok_or(DurableError::Invalid("navigation event dependency absent"))?
                .path
                .as_str(),
        )
        .copied()
        .ok_or(DurableError::Corrupt("navigation event dependency absent"))?;
    let historical_record_basis = matches!(
        cohort.basis(),
        NavigationRecordEventBasisV1::ClaimRevisionRefresh { .. }
    );
    if (historical_record_basis
        && event_member
            .commit_sequence
            .is_none_or(|sequence| sequence > cohort.event_sequence))
        || (!historical_record_basis && event_member.commit_sequence != Some(cohort.event_sequence))
    {
        return Err(DurableError::Conflict(
            "navigation event source is outside its committed source cut",
        ));
    }
    if historical_record_basis {
        let receipt_member = selected
            .get(
                cohort
                    .dependencies
                    .iter()
                    .find(|dependency| dependency.role == NavigationDependencyRoleV1::EventReceipt)
                    .ok_or(DurableError::Invalid(
                        "navigation event receipt dependency absent",
                    ))?
                    .path
                    .as_str(),
            )
            .copied()
            .ok_or(DurableError::Corrupt(
                "navigation event receipt dependency absent",
            ))?;
        if receipt_member
            .commit_sequence
            .is_none_or(|sequence| sequence > cohort.event_sequence)
        {
            return Err(DurableError::Conflict(
                "navigation Record creation receipt is outside the selected source cut",
            ));
        }
    }
    let record_member = selected
        .get(cohort.source_path.as_str())
        .copied()
        .ok_or(DurableError::Corrupt("navigation record dependency absent"))?;
    match cohort.basis() {
        NavigationRecordEventBasisV1::Mutation => {
            if record_member.commit_sequence != Some(cohort.event_sequence) {
                return Err(DurableError::Conflict(
                    "navigation record path is not changed by this committed event",
                ));
            }
        }
        NavigationRecordEventBasisV1::DependencyRefresh {
            changed_dependency_path,
            changed_dependency_sha256,
        } => {
            let changed = selected
                .get(changed_dependency_path.as_str())
                .copied()
                .ok_or(DurableError::Corrupt(
                    "navigation refresh dependency absent from selected cut",
                ))?;
            if !record_member.is_source
                || record_member.commit_sequence.is_none_or(|sequence| sequence > cohort.event_sequence)
                || changed.commit_sequence != Some(cohort.event_sequence)
                || !changed.is_source
                || changed.content_digest != Some(*changed_dependency_sha256)
                || cohort.dependencies.iter().find(|dependency| dependency.path == *changed_dependency_path)
                    .is_none_or(|dependency| {
                        dependency.role != NavigationDependencyRoleV1::Additional
                            || !matches!(dependency.state, NavigationDependencyStateV1::Present { sha256, .. } if sha256 == *changed_dependency_sha256)
                    })
            {
                return Err(DurableError::Conflict(
                    "navigation dependency refresh lacks exact changed-member proof",
                ));
            }
        }
        NavigationRecordEventBasisV1::DependencyRefreshSet {
            changed_dependencies,
        } => {
            navigation_record_dependency_refresh_causes_value(
                changed_dependencies,
                cohort.source_path.as_str(),
                limits.max_row_bytes as usize,
            )?;
            for cause in changed_dependencies {
                let changed =
                    selected
                        .get(cause.path.as_str())
                        .copied()
                        .ok_or(DurableError::Corrupt(
                            "navigation refresh-set cause absent from selected event cut",
                        ))?;
                let dependency = cohort
                    .dependencies
                    .iter()
                    .find(|dependency| dependency.path == cause.path)
                    .ok_or(DurableError::Conflict(
                        "navigation refresh-set cause is not a Record dependency",
                    ))?;
                let raw = changed
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation refresh-set source bytes are absent",
                    ))?;
                if !changed.is_source
                    || changed.commit_sequence != Some(cohort.event_sequence)
                    || changed.content_digest != Some(cause.sha256)
                    || Digest256::of_bytes(raw) != cause.sha256
                    || !matches!(
                        &dependency.state,
                        NavigationDependencyStateV1::Present { sha256, size_bytes }
                            if *sha256 == cause.sha256 && *size_bytes == raw.len() as u64
                    )
                {
                    return Err(DurableError::Conflict(
                        "navigation refresh-set cause differs from exact changed dependency",
                    ));
                }
            }
        }
        NavigationRecordEventBasisV1::ClaimRevisionRefresh {
            changed_dependency_path,
            changed_dependency_sha256,
            history_path,
            history_sha256,
            ..
        } => {
            let changed = selected
                .get(changed_dependency_path.as_str())
                .copied()
                .ok_or(DurableError::Corrupt(
                    "navigation Claim stream absent from selected cut",
                ))?;
            let history =
                selected
                    .get(history_path.as_str())
                    .copied()
                    .ok_or(DurableError::Corrupt(
                        "navigation Claim history absent from selected cut",
                    ))?;
            let dependency_matches = |path: &RelativePath, digest: Digest256| {
                cohort.dependencies.iter().any(|dependency| {
                    dependency.path == *path
                        && dependency.role == NavigationDependencyRoleV1::Additional
                        && matches!(dependency.state, NavigationDependencyStateV1::Present { sha256, .. } if sha256 == digest)
                })
            };
            if !record_member.is_source
                || record_member
                    .commit_sequence
                    .is_none_or(|sequence| sequence > cohort.event_sequence)
                || changed.commit_sequence != Some(cohort.event_sequence)
                || !changed.is_source
                || changed.content_digest != Some(*changed_dependency_sha256)
                || history.commit_sequence != Some(cohort.event_sequence)
                || !history.is_source
                || history.content_digest != Some(*history_sha256)
                || !dependency_matches(changed_dependency_path, *changed_dependency_sha256)
                || !dependency_matches(history_path, *history_sha256)
            {
                return Err(DurableError::Conflict(
                    "navigation Claim revision refresh lacks exact current dependency proof",
                ));
            }
        }
        NavigationRecordEventBasisV1::RangeReconciliation { .. } => {
            return Err(DurableError::Conflict(
                "range reconciliation requires the exact range publisher",
            ));
        }
    }
    Ok(())
}

fn verify_navigation_claim_dependency_cut(
    cohort: &NavigationClaimEventCohortV1,
    selected: &[DurableNavigationEventSourceV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let selected = selected
        .iter()
        .map(|source| (source.source_path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    for dependency in cohort.dependencies() {
        let source =
            selected
                .get(dependency.path.as_str())
                .copied()
                .ok_or(DurableError::Corrupt(
                    "navigation Claim dependency absent from selected event cut",
                ))?;
        match &dependency.state {
            NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                let raw = source
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Claim present dependency lacks exact source bytes",
                    ))?;
                if !source.is_source
                    || source.content_digest != Some(*sha256)
                    || source.content_length != Some(*size_bytes)
                    || raw.len() as u64 != *size_bytes
                    || Digest256::of_bytes(raw) != *sha256
                    || *size_bytes
                        > if dependency.role == NavigationDependencyRoleV1::Event {
                            limits.max_total_bytes
                        } else {
                            limits.max_row_bytes as u64
                        }
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim dependency differs from exact selected source",
                    ));
                }
            }
            NavigationDependencyStateV1::Absent { .. } if source.is_source => {
                return Err(DurableError::Conflict(
                    "navigation Claim absent dependency exists at event cut",
                ));
            }
            NavigationDependencyStateV1::Absent { .. } => {}
        }
    }
    for required_path in [
        cohort
            .dependencies()
            .iter()
            .find(|dependency| dependency.role == NavigationDependencyRoleV1::Event)
            .map(|dependency| dependency.path.as_str())
            .ok_or(DurableError::Invalid(
                "navigation Claim event dependency absent",
            ))?,
        cohort.source_path().as_str(),
        cohort.mutation_receipt_path().as_str(),
    ] {
        let source = selected
            .get(required_path)
            .copied()
            .ok_or(DurableError::Corrupt(
                "navigation Claim event owner source absent",
            ))?;
        if source.commit_sequence != Some(cohort.event_sequence()) || !source.is_source {
            return Err(DurableError::Conflict(
                "navigation Claim owner source is not at its committed event sequence",
            ));
        }
    }
    Ok(())
}

fn verify_event_manifest(
    cohort: &NavigationRecordEventCohortV1,
    selected: &[DurableNavigationEventSourceV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let dependency_source = |role| {
        let dependency = cohort
            .dependencies
            .iter()
            .find(|dependency| dependency.role == role)
            .ok_or(DurableError::Invalid(
                "navigation event dependency role absent",
            ))?;
        selected
            .iter()
            .find(|source| source.source_path == dependency.path.as_str())
            .ok_or(DurableError::Corrupt(
                "navigation exact dependency selection absent",
            ))
    };
    let event = dependency_source(NavigationDependencyRoleV1::Event)?;
    let event_raw = event
        .exact_source_bytes
        .as_deref()
        .ok_or(DurableError::Conflict(
            "navigation event source is not retained",
        ))?;
    let historical_record_basis = matches!(
        cohort.basis(),
        NavigationRecordEventBasisV1::ClaimRevisionRefresh { .. }
    );
    if (historical_record_basis
        && event
            .commit_sequence
            .is_none_or(|sequence| sequence > cohort.event_sequence))
        || (!historical_record_basis && event.commit_sequence != Some(cohort.event_sequence))
    {
        return Err(DurableError::Conflict(
            "navigation authored event bytes are outside the committed source cut",
        ));
    }
    let receipt_dependency = cohort
        .dependencies
        .iter()
        .find(|dependency| dependency.role == NavigationDependencyRoleV1::EventReceipt)
        .ok_or(DurableError::Invalid(
            "navigation event receipt dependency absent",
        ))?;
    let value =
        select_navigation_event_record(event_raw, receipt_dependency.path.as_str(), limits)?;
    if value.get("schema_version").and_then(Value::as_str) != Some("tos_provenance_event_v2")
        || value.get("event_id").and_then(Value::as_str) != Some(cohort.source_event_id.as_str())
        || value.get("event_version").and_then(Value::as_u64) != Some(cohort.source_event_version)
    {
        return Err(DurableError::Conflict(
            "authored navigation event identity differs from exact v2 bytes",
        ));
    }

    let event_schema_dependency = cohort
        .dependencies
        .iter()
        .find(|dependency| dependency.role == NavigationDependencyRoleV1::EventSchema)
        .ok_or(DurableError::Invalid(
            "navigation event schema dependency absent",
        ))?;
    if event_schema_dependency.path.as_str() != "ToS/contracts/provenance-event-v2.schema.json" {
        return Err(DurableError::Conflict(
            "navigation event schema path differs from its owner",
        ));
    }
    let event_schema = dependency_source(NavigationDependencyRoleV1::EventSchema)?
        .exact_source_bytes
        .as_deref()
        .ok_or(DurableError::Conflict("navigation event schema is absent"))?;
    let event_schema = strict_json_value(event_schema, limits.max_row_bytes)?;
    if event_schema.get("$id").and_then(Value::as_str)
        != Some("https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json")
        || value.get("$schema").and_then(Value::as_str)
            != event_schema.get("$id").and_then(Value::as_str)
    {
        return Err(DurableError::Conflict(
            "navigation v2 event schema binding differs",
        ));
    }

    let source = selected
        .iter()
        .find(|source| source.source_path == cohort.source_path.as_str())
        .ok_or(DurableError::Corrupt(
            "navigation record source selection absent",
        ))?;
    let record_raw = source
        .exact_source_bytes
        .as_deref()
        .ok_or(DurableError::Conflict("navigation record source is absent"))?;
    match cohort.basis() {
        NavigationRecordEventBasisV1::Mutation
            if source.commit_sequence != Some(cohort.event_sequence) =>
        {
            return Err(DurableError::Conflict(
                "navigation record bytes are not from the committed event sequence",
            ));
        }
        NavigationRecordEventBasisV1::DependencyRefresh { .. }
        | NavigationRecordEventBasisV1::DependencyRefreshSet { .. }
        | NavigationRecordEventBasisV1::ClaimRevisionRefresh { .. }
            if !source.is_source
                || source
                    .commit_sequence
                    .is_none_or(|sequence| sequence > cohort.event_sequence) =>
        {
            return Err(DurableError::Conflict(
                "navigation refresh record is outside the selected current cut",
            ));
        }
        _ => {}
    }
    let record = strict_json_value(record_raw, limits.max_row_bytes)?;
    if record.get("record_id").and_then(Value::as_str) != Some(cohort.record_id.as_str()) {
        return Err(DurableError::Conflict(
            "navigation record ID differs from committed bytes",
        ));
    }

    let receipt_ref = value
        .pointer("/record_binding/manifest_ref")
        .and_then(Value::as_str)
        .ok_or(DurableError::Conflict(
            "navigation event receipt ref absent",
        ))?;
    if receipt_ref != receipt_dependency.path.as_str()
        || value
            .pointer("/record_binding/digest_algorithm")
            .and_then(Value::as_str)
            != Some("sha256")
        || value
            .pointer("/record_binding/digest_scope")
            .and_then(Value::as_str)
            != Some("exact_event_record_bytes")
    {
        return Err(DurableError::Conflict(
            "navigation event receipt binding differs",
        ));
    }
    let receipt_raw = dependency_source(NavigationDependencyRoleV1::EventReceipt)?
        .exact_source_bytes
        .as_deref()
        .ok_or(DurableError::Conflict("navigation event receipt is absent"))?;
    let receipt = strict_json_value(receipt_raw, limits.max_row_bytes)?;
    match cohort.basis() {
        NavigationRecordEventBasisV1::Mutation => {
            if receipt.get("schema_version").and_then(Value::as_str)
                != Some("tos_local_source_create_receipt_v1")
                || receipt.get("source_path").and_then(Value::as_str)
                    != Some(cohort.source_path.as_str())
                || receipt.pointer("/source/id").and_then(Value::as_str)
                    != Some(cohort.record_id.as_str())
            {
                return Err(DurableError::Conflict(
                    "navigation event receipt owner binding differs",
                ));
            }
            let outputs = value
                .pointer("/entities/outputs")
                .and_then(Value::as_array)
                .ok_or(DurableError::Conflict("navigation event outputs absent"))?;
            verify_event_output(
                outputs,
                cohort.source_path.as_str(),
                record_raw,
                cohort.event_sequence,
                selected,
            )?;
            let forms = cohort
                .dependencies
                .iter()
                .find(|dependency| dependency.role == NavigationDependencyRoleV1::HumanForms)
                .ok_or(DurableError::Invalid(
                    "navigation forms dependency role absent",
                ))?;
            let forms_path = cohort
                .source_path
                .as_str()
                .strip_suffix(".json")
                .map(|stem| format!("{stem}.human-forms.json"))
                .ok_or(DurableError::Invalid("navigation record path suffix"))?;
            if forms.path.as_str() != forms_path {
                return Err(DurableError::Conflict(
                    "navigation forms dependency path differs from record binding",
                ));
            }
            match &forms.state {
                NavigationDependencyStateV1::Present { .. } => {
                    let form_source = selected
                        .iter()
                        .find(|source| source.source_path == forms_path)
                        .ok_or(DurableError::Corrupt("navigation forms selection absent"))?;
                    let form_raw = form_source
                        .exact_source_bytes
                        .as_deref()
                        .ok_or(DurableError::Conflict("navigation forms bytes absent"))?;
                    if form_source.commit_sequence != Some(cohort.event_sequence) {
                        return Err(DurableError::Conflict(
                            "navigation forms are not from the committed event sequence",
                        ));
                    }
                    verify_event_output(
                        outputs,
                        &forms_path,
                        form_raw,
                        cohort.event_sequence,
                        selected,
                    )?;
                }
                NavigationDependencyStateV1::Absent { .. } => {
                    if outputs.iter().any(|output| {
                        output.get("entity_ref").and_then(Value::as_str)
                            == Some(forms_path.as_str())
                    }) {
                        return Err(DurableError::Conflict(
                            "navigation event binds forms without a selected source",
                        ));
                    }
                }
            }
            if outputs.len()
                != 1 + usize::from(matches!(
                    &forms.state,
                    NavigationDependencyStateV1::Present { .. }
                ))
            {
                return Err(DurableError::Conflict(
                    "navigation event has an unsupported output set",
                ));
            }
            let files =
                receipt
                    .get("files")
                    .and_then(Value::as_object)
                    .ok_or(DurableError::Conflict(
                        "navigation event receipt files absent",
                    ))?;
            verify_receipt_file(files, event.source_path.as_str(), event_raw)?;
            verify_receipt_file(files, cohort.source_path.as_str(), record_raw)?;
            if matches!(&forms.state, NavigationDependencyStateV1::Present { .. }) {
                let form_raw = selected
                    .iter()
                    .find(|source| source.source_path == forms_path)
                    .and_then(|source| source.exact_source_bytes.as_deref())
                    .ok_or(DurableError::Conflict("navigation forms bytes absent"))?;
                verify_receipt_file(files, &forms_path, form_raw)?;
            }
        }
        NavigationRecordEventBasisV1::DependencyRefresh {
            changed_dependency_path,
            changed_dependency_sha256,
        } => {
            if receipt.get("schema_version").and_then(Value::as_str)
                != Some("tos_local_claim_create_receipt_v1")
                || receipt.get("source_path").and_then(Value::as_str)
                    != Some(changed_dependency_path.as_str())
            {
                return Err(DurableError::Conflict(
                    "navigation refresh receipt owner binding differs",
                ));
            }
            let changed = selected
                .iter()
                .find(|source| source.source_path == changed_dependency_path.as_str())
                .ok_or(DurableError::Corrupt(
                    "navigation refresh source selection absent",
                ))?;
            let changed_raw =
                changed
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation refresh dependency bytes absent",
                    ))?;
            if changed.commit_sequence != Some(cohort.event_sequence)
                || !changed.is_source
                || changed.content_digest != Some(*changed_dependency_sha256)
                || Digest256::of_bytes(changed_raw) != *changed_dependency_sha256
                || value
                    .pointer("/record_binding/manifest_ref")
                    .and_then(Value::as_str)
                    != Some(receipt_ref)
            {
                return Err(DurableError::Conflict(
                    "navigation refresh exact changed source differs",
                ));
            }
            let outputs = value
                .pointer("/entities/outputs")
                .and_then(Value::as_array)
                .ok_or(DurableError::Conflict("navigation refresh outputs absent"))?;
            verify_event_output(
                outputs,
                changed_dependency_path.as_str(),
                changed_raw,
                cohort.event_sequence,
                selected,
            )?;
            if outputs.iter().any(|output| {
                output.get("entity_ref").and_then(Value::as_str)
                    == Some(cohort.source_path.as_str())
            }) {
                return Err(DurableError::Conflict(
                    "navigation refresh must not claim a record mutation",
                ));
            }
            let files =
                receipt
                    .get("files")
                    .and_then(Value::as_object)
                    .ok_or(DurableError::Conflict(
                        "navigation refresh receipt files absent",
                    ))?;
            verify_receipt_file(files, event.source_path.as_str(), event_raw)?;
            verify_receipt_file(files, changed_dependency_path.as_str(), changed_raw)?;
        }
        NavigationRecordEventBasisV1::DependencyRefreshSet { .. } => {
            verify_navigation_record_creation_binding(
                cohort,
                &value,
                &receipt,
                receipt_ref,
                &event,
                event_raw,
                source,
                record_raw,
                selected,
            )?;
        }
        NavigationRecordEventBasisV1::ClaimRevisionRefresh {
            changed_dependency_path,
            changed_dependency_sha256,
            history_path,
            history_sha256,
            claim_id,
            ..
        } => {
            if receipt.get("schema_version").and_then(Value::as_str)
                != Some("tos_local_source_create_receipt_v1")
                || receipt.get("source_path").and_then(Value::as_str)
                    != Some(cohort.source_path.as_str())
                || receipt.pointer("/source/id").and_then(Value::as_str)
                    != Some(cohort.record_id.as_str())
                || value
                    .pointer("/record_binding/manifest_ref")
                    .and_then(Value::as_str)
                    != Some(receipt_ref)
            {
                return Err(DurableError::Conflict(
                    "navigation Record creation receipt owner binding differs",
                ));
            }
            let creation_sequence = source.commit_sequence.ok_or(DurableError::Conflict(
                "navigation Record source sequence absent",
            ))?;
            let outputs = value
                .pointer("/entities/outputs")
                .and_then(Value::as_array)
                .ok_or(DurableError::Conflict(
                    "navigation Record creation outputs absent",
                ))?;
            verify_event_output(
                outputs,
                cohort.source_path.as_str(),
                record_raw,
                creation_sequence,
                selected,
            )?;
            let forms = cohort
                .dependencies
                .iter()
                .find(|dependency| dependency.role == NavigationDependencyRoleV1::HumanForms)
                .ok_or(DurableError::Invalid(
                    "navigation forms dependency role absent",
                ))?;
            let forms_path = cohort
                .source_path
                .as_str()
                .strip_suffix(".json")
                .map(|stem| format!("{stem}.human-forms.json"))
                .ok_or(DurableError::Invalid("navigation record path suffix"))?;
            if forms.path.as_str() != forms_path {
                return Err(DurableError::Conflict(
                    "navigation Record forms path differs from creation binding",
                ));
            }
            let form_raw =
                match &forms.state {
                    NavigationDependencyStateV1::Present { .. } => {
                        let form_source = selected
                            .iter()
                            .find(|source| source.source_path == forms_path)
                            .ok_or(DurableError::Corrupt(
                                "navigation Record forms source selection absent",
                            ))?;
                        let form_raw = form_source.exact_source_bytes.as_deref().ok_or(
                            DurableError::Conflict("navigation Record forms bytes absent"),
                        )?;
                        if form_source.commit_sequence != Some(creation_sequence) {
                            return Err(DurableError::Conflict(
                                "Claim refresh cannot reuse forms changed after Record creation",
                            ));
                        }
                        verify_event_output(
                            outputs,
                            &forms_path,
                            form_raw,
                            creation_sequence,
                            selected,
                        )?;
                        Some(form_raw)
                    }
                    NavigationDependencyStateV1::Absent { .. } => {
                        if outputs.iter().any(|output| {
                            output.get("entity_ref").and_then(Value::as_str)
                                == Some(forms_path.as_str())
                        }) {
                            return Err(DurableError::Conflict(
                                "Record creation output binds absent forms",
                            ));
                        }
                        None
                    }
                };
            if outputs.len() != 1 + usize::from(form_raw.is_some()) {
                return Err(DurableError::Conflict(
                    "Record creation event has an unsupported output set",
                ));
            }
            let files =
                receipt
                    .get("files")
                    .and_then(Value::as_object)
                    .ok_or(DurableError::Conflict(
                        "navigation Record creation receipt files absent",
                    ))?;
            verify_receipt_file(files, event.source_path.as_str(), event_raw)?;
            verify_receipt_file(files, cohort.source_path.as_str(), record_raw)?;
            if let Some(form_raw) = form_raw {
                verify_receipt_file(files, &forms_path, form_raw)?;
            }

            let changed = selected
                .iter()
                .find(|source| source.source_path == changed_dependency_path.as_str())
                .ok_or(DurableError::Corrupt(
                    "navigation Claim stream selection absent",
                ))?;
            let history = selected
                .iter()
                .find(|source| source.source_path == history_path.as_str())
                .ok_or(DurableError::Corrupt(
                    "navigation Claim history selection absent",
                ))?;
            let changed_raw =
                changed
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Claim stream bytes absent",
                    ))?;
            let history_raw =
                history
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Claim history bytes absent",
                    ))?;
            let exact_dependency = |path: &RelativePath, digest: Digest256| {
                cohort.dependencies.iter().any(|dependency| {
                    dependency.path == *path
                        && dependency.role == NavigationDependencyRoleV1::Additional
                        && matches!(&dependency.state, NavigationDependencyStateV1::Present { sha256, size_bytes } if *sha256 == digest && *size_bytes == if path.as_str() == changed_dependency_path.as_str() { changed_raw.len() as u64 } else { history_raw.len() as u64 })
                })
            };
            if changed.commit_sequence != Some(cohort.event_sequence)
                || !changed.is_source
                || changed.content_digest != Some(*changed_dependency_sha256)
                || Digest256::of_bytes(changed_raw) != *changed_dependency_sha256
                || history.commit_sequence != Some(cohort.event_sequence)
                || !history.is_source
                || history.content_digest != Some(*history_sha256)
                || Digest256::of_bytes(history_raw) != *history_sha256
                || !exact_dependency(changed_dependency_path, *changed_dependency_sha256)
                || !exact_dependency(history_path, *history_sha256)
            {
                return Err(DurableError::Conflict(
                    "Claim revision stream or history differs from the exact CMD event",
                ));
            }
            let current_claim = navigation_claim_current_row_from_stream(
                changed_raw,
                claim_id,
                limits.max_row_bytes,
                limits.max_total_bytes,
            )?;
            let history_value = strict_json_value(history_raw, limits.max_row_bytes)?;
            let receipt_file = history_value
                .get("receipts")
                .and_then(Value::as_array)
                .and_then(|rows| rows.last())
                .ok_or(DurableError::Conflict(
                    "Claim revision history has no retained receipt",
                ))?;
            if history_value.get("schema_version").and_then(Value::as_str)
                != Some("tos_claim_revision_history_v1")
                || history_value.get("source_path").and_then(Value::as_str)
                    != Some(changed_dependency_path.as_str())
                || receipt_file.pointer("/source/id").and_then(Value::as_str)
                    != Some(claim_id.as_str())
                || receipt_file
                    .pointer("/previous_source/id")
                    .and_then(Value::as_str)
                    != Some(claim_id.as_str())
            {
                return Err(DurableError::Conflict(
                    "Claim revision history owner differs from its proof basis",
                ));
            }
            if current_claim.get("claim_version").and_then(Value::as_u64)
                != receipt_file
                    .pointer("/source/version")
                    .and_then(Value::as_u64)
                || current_claim.get("claim_version").and_then(Value::as_u64)
                    != receipt_file
                        .pointer("/previous_source/version")
                        .and_then(Value::as_u64)
                        .and_then(|version| version.checked_add(1))
            {
                return Err(DurableError::Conflict(
                    "Claim revision current version differs from retained history",
                ));
            }
        }
        NavigationRecordEventBasisV1::RangeReconciliation { .. } => {
            return Err(DurableError::Conflict(
                "range Record evidence requires the exact range manifest",
            ));
        }
    }
    Ok(())
}

fn verify_navigation_record_creation_binding(
    cohort: &NavigationRecordEventCohortV1,
    event_value: &Value,
    receipt: &Value,
    receipt_ref: &str,
    event: &DurableNavigationEventSourceV1,
    event_raw: &[u8],
    source: &DurableNavigationEventSourceV1,
    record_raw: &[u8],
    selected: &[DurableNavigationEventSourceV1],
) -> DurableResult<()> {
    if receipt.get("schema_version").and_then(Value::as_str)
        != Some("tos_local_source_create_receipt_v1")
        || receipt.get("source_path").and_then(Value::as_str) != Some(cohort.source_path.as_str())
        || receipt.pointer("/source/id").and_then(Value::as_str) != Some(cohort.record_id.as_str())
        || event_value
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            != Some(receipt_ref)
    {
        return Err(DurableError::Conflict(
            "navigation Record creation receipt owner binding differs",
        ));
    }
    let creation_sequence = source.commit_sequence.ok_or(DurableError::Conflict(
        "navigation Record source sequence absent",
    ))?;
    let outputs = event_value
        .pointer("/entities/outputs")
        .and_then(Value::as_array)
        .ok_or(DurableError::Conflict(
            "navigation Record creation outputs absent",
        ))?;
    verify_event_output(
        outputs,
        cohort.source_path.as_str(),
        record_raw,
        creation_sequence,
        selected,
    )?;
    let forms = cohort
        .dependencies
        .iter()
        .find(|dependency| dependency.role == NavigationDependencyRoleV1::HumanForms)
        .ok_or(DurableError::Invalid(
            "navigation forms dependency role absent",
        ))?;
    let forms_path = cohort
        .source_path
        .as_str()
        .strip_suffix(".json")
        .map(|stem| format!("{stem}.human-forms.json"))
        .ok_or(DurableError::Invalid("navigation record path suffix"))?;
    if forms.path.as_str() != forms_path {
        return Err(DurableError::Conflict(
            "navigation Record forms path differs from creation binding",
        ));
    }
    let form_raw = match &forms.state {
        NavigationDependencyStateV1::Present { .. } => {
            let form_source = selected
                .iter()
                .find(|source| source.source_path == forms_path)
                .ok_or(DurableError::Corrupt(
                    "navigation Record forms source selection absent",
                ))?;
            let form_raw =
                form_source
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Record forms bytes absent",
                    ))?;
            if form_source.commit_sequence != Some(creation_sequence) {
                return Err(DurableError::Conflict(
                    "dependency refresh cannot reuse forms changed after Record creation",
                ));
            }
            verify_event_output(outputs, &forms_path, form_raw, creation_sequence, selected)?;
            Some(form_raw)
        }
        NavigationDependencyStateV1::Absent { .. } => {
            if outputs.iter().any(|output| {
                output.get("entity_ref").and_then(Value::as_str) == Some(forms_path.as_str())
            }) {
                return Err(DurableError::Conflict(
                    "Record creation output binds absent forms",
                ));
            }
            None
        }
    };
    if outputs.len() != 1 + usize::from(form_raw.is_some()) {
        return Err(DurableError::Conflict(
            "Record creation event has an unsupported output set",
        ));
    }
    let files = receipt
        .get("files")
        .and_then(Value::as_object)
        .ok_or(DurableError::Conflict(
            "navigation Record creation receipt files absent",
        ))?;
    verify_receipt_file(files, event.source_path.as_str(), event_raw)?;
    verify_receipt_file(files, cohort.source_path.as_str(), record_raw)?;
    if let Some(form_raw) = form_raw {
        verify_receipt_file(files, &forms_path, form_raw)?;
    }
    Ok(())
}

/// Mirror the source compiler's bounded JSONL selection: validate every
/// nonblank event record, select exactly one by its receipt reference, then
/// bind that selected record to the cohort's authored event identity.
fn select_navigation_event_record(
    file_bytes: &[u8],
    receipt_path: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Value> {
    if file_bytes.is_empty()
        || file_bytes.len() as u64 > limits.max_total_bytes
        || receipt_path.is_empty()
    {
        return Err(DurableError::Refused(
            "navigation provenance event file byte budget",
        ));
    }
    let mut selected = None;
    let mut record_count = 0usize;
    let line_count = file_bytes.split(|byte| *byte == b'\n').count();
    let has_final_terminator = file_bytes.ends_with(b"\n");
    for (index, raw_line) in file_bytes.split(|byte| *byte == b'\n').enumerate() {
        let is_final_terminator =
            raw_line.is_empty() && index + 1 == line_count && has_final_terminator;
        if raw_line.is_empty() {
            if is_final_terminator {
                continue;
            }
            return Err(DurableError::Corrupt(
                "navigation provenance event has a blank JSONL record",
            ));
        }
        record_count += 1;
        if record_count > MAX_NAVIGATION_EVENT_LOG_ROWS {
            return Err(DurableError::Refused(
                "navigation provenance event row count budget",
            ));
        }
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        let value = strict_json_value(line, limits.max_row_bytes)?;
        if value
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            != Some(receipt_path)
        {
            continue;
        }
        if selected.replace(value).is_some() {
            return Err(DurableError::Conflict(
                "navigation provenance event receipt reference is ambiguous",
            ));
        }
    }
    selected.ok_or(DurableError::Conflict(
        "navigation provenance event receipt reference is absent",
    ))
}

fn verify_event_output(
    outputs: &[Value],
    path: &str,
    exact_bytes: &[u8],
    event_sequence: u64,
    selected: &[DurableNavigationEventSourceV1],
) -> DurableResult<()> {
    let matched = outputs
        .iter()
        .filter(|output| output.get("entity_ref").and_then(Value::as_str) == Some(path))
        .collect::<Vec<_>>();
    let Some(output) = matched.first().copied() else {
        return Err(DurableError::Conflict(
            "navigation exact event output absent",
        ));
    };
    let digest = Digest256::of_bytes(exact_bytes).to_hex();
    let source = selected
        .iter()
        .find(|source| source.source_path == path)
        .ok_or(DurableError::Corrupt(
            "navigation event output source absent",
        ))?;
    if matched.len() != 1
        || output.get("sha256").and_then(Value::as_str) != Some(digest.as_str())
        || output.get("size_bytes").and_then(Value::as_u64) != Some(exact_bytes.len() as u64)
        || source.commit_sequence != Some(event_sequence)
        || !source.is_source
        || source.exact_source_bytes.as_deref() != Some(exact_bytes)
    {
        return Err(DurableError::Conflict(
            "navigation exact event output binding differs",
        ));
    }
    Ok(())
}

fn verify_receipt_file(
    files: &serde_json::Map<String, Value>,
    path: &str,
    exact_bytes: &[u8],
) -> DurableResult<()> {
    let name = path
        .rsplit('/')
        .next()
        .ok_or(DurableError::Invalid("navigation receipt path basename"))?;
    let file = files.get(name).ok_or(DurableError::Conflict(
        "navigation exact receipt file absent",
    ))?;
    let digest = format!("sha256:{}", Digest256::of_bytes(exact_bytes).to_hex());
    if file.get("sha256").and_then(Value::as_str) != Some(digest.as_str())
        || file.get("bytes").and_then(Value::as_u64) != Some(exact_bytes.len() as u64)
    {
        return Err(DurableError::Conflict(
            "navigation exact receipt file binding differs",
        ));
    }
    Ok(())
}

fn validate_cohort(
    cohort: &NavigationRecordEventCohortV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<PersistedRows> {
    validate_identifier(
        &cohort.event_id,
        MAX_NAVIGATION_EVENT_ID_BYTES,
        "navigation event ID",
    )?;
    validate_identifier(
        &cohort.source_event_id,
        MAX_NAVIGATION_EVENT_ID_BYTES,
        "authored navigation event ID",
    )?;
    if cohort.event_id.bytes().any(|byte| byte.is_ascii_control())
        || cohort
            .source_event_id
            .bytes()
            .any(|byte| byte.is_ascii_control())
        || cohort.event_sequence == 0
        || cohort.event_sequence > i64::MAX as u64
        || cohort.source_event_version == 0
        || cohort.source_event_version > i64::MAX as u64
        || cohort.predecessor_source_revision == cohort.source_revision
    {
        return Err(DurableError::Invalid(
            "navigation event identity or revision",
        ));
    }
    validate_relative_path(cohort.source_path.as_str())?;
    validate_identifier(
        &cohort.record_id,
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "navigation record ID",
    )?;
    if cohort.dependencies.is_empty() || cohort.dependencies.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation event dependency count budget",
        ));
    }
    let total_bytes = validate_navigation_dependency_envelope(
        &cohort.dependencies,
        cohort.source_path.as_str(),
        limits,
    )?;
    persist_navigation_projection_rows(
        &cohort.dependencies,
        &cohort.projection.nodes,
        &cohort.projection.edges,
        &cohort.projection.diagnostics,
        cohort.node_visibility(),
        cohort.edge_visibility(),
        &[],
        total_bytes,
        limits,
    )
}

fn validate_navigation_dependency_envelope(
    dependencies: &[NavigationDependencyV1],
    source_path: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<u64> {
    if dependencies.is_empty() || dependencies.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation event dependency count budget",
        ));
    }
    let mut total_bytes = 0u64;
    let mut seen_dependencies = BTreeSet::new();
    let mut previous_path = None;
    let mut event_count = 0usize;
    let mut event_schema_count = 0usize;
    let mut event_receipt_count = 0usize;
    let mut record_count = 0usize;
    for dependency in dependencies {
        let path = dependency.path.as_str();
        validate_relative_path(path)?;
        if !seen_dependencies.insert(path.to_owned())
            || previous_path.is_some_and(|previous: &str| previous >= path)
        {
            return Err(DurableError::Invalid(
                "navigation dependencies are not uniquely path-sorted",
            ));
        }
        previous_path = Some(path);
        event_count += usize::from(dependency.role == NavigationDependencyRoleV1::Event);
        event_schema_count +=
            usize::from(dependency.role == NavigationDependencyRoleV1::EventSchema);
        event_receipt_count +=
            usize::from(dependency.role == NavigationDependencyRoleV1::EventReceipt);
        record_count += usize::from(
            dependency.role == NavigationDependencyRoleV1::Record && path == source_path,
        );
        if path.len() > MAX_NAVIGATION_ROW_BYTES {
            return Err(DurableError::Refused(
                "navigation dependency path byte budget",
            ));
        }
        total_bytes = total_bytes
            .checked_add(path.len() as u64)
            .ok_or(DurableError::Refused("navigation event byte overflow"))?;
        if let NavigationDependencyStateV1::Present { size_bytes, .. } = &dependency.state {
            let source_byte_limit = if dependency.role == NavigationDependencyRoleV1::Event {
                limits.max_total_bytes
            } else {
                limits.max_row_bytes as u64
            };
            if *size_bytes == 0 || *size_bytes > source_byte_limit {
                return Err(DurableError::Refused(
                    "navigation dependency source byte budget",
                ));
            }
        }
        if let NavigationDependencyStateV1::Absent { reason } = &dependency.state
            && (reason.is_empty() || reason.len() > 128)
        {
            return Err(DurableError::Invalid("navigation absent dependency reason"));
        }
    }
    if event_count != 1 || event_schema_count != 1 || event_receipt_count != 1 || record_count != 1
    {
        return Err(DurableError::Invalid(
            "navigation event and record dependency roles",
        ));
    }
    Ok(total_bytes)
}

fn persist_navigation_projection_rows(
    dependencies: &[NavigationDependencyV1],
    node_values: &[Value],
    edge_values: &[Value],
    diagnostic_values: &[Value],
    node_visibility: &[bool],
    edge_visibility: &[bool],
    external_endpoint_ids: &[String],
    mut total_bytes: u64,
    limits: DurableNavigationEventLimits,
) -> DurableResult<PersistedRows> {
    let mut nodes = Vec::with_capacity(node_values.len());
    let mut edges = Vec::with_capacity(edge_values.len());
    let mut diagnostics = Vec::with_capacity(diagnostic_values.len());
    if node_visibility.len() != node_values.len() || edge_visibility.len() != edge_values.len() {
        return Err(DurableError::Invalid(
            "navigation event visibility witness count",
        ));
    }
    if external_endpoint_ids
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(DurableError::Invalid(
            "navigation external endpoint IDs are not sorted and unique",
        ));
    }
    let external_endpoint_ids = external_endpoint_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut node_ids = BTreeSet::new();
    let mut node_visibility_by_id = BTreeMap::new();
    for (value, visible) in node_values.iter().zip(node_visibility) {
        if !value.is_object() {
            return Err(DurableError::Invalid(
                "navigation node output row must be an object",
            ));
        }
        let id = value
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Invalid("navigation output node identity"))?;
        validate_identifier(
            id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation output node ID",
        )?;
        if !node_ids.insert(id.to_owned()) {
            return Err(DurableError::Invalid("duplicate navigation node ID"));
        }
        node_visibility_by_id.insert(id.to_owned(), *visible);
        let encoded = canonical_value(value, limits.max_row_bytes)?;
        total_bytes = total_bytes
            .checked_add(encoded.len() as u64)
            .ok_or(DurableError::Refused("navigation event byte overflow"))?;
        nodes.push(PersistedNavigationNode {
            node_id: id.to_owned(),
            visible: *visible,
            sha256: Digest256::of_bytes(&encoded),
            row_json: encoded,
        });
    }
    let mut edge_ids = BTreeSet::new();
    for (value, visible) in edge_values.iter().zip(edge_visibility) {
        if !value.is_object() {
            return Err(DurableError::Invalid(
                "navigation edge output row must be an object",
            ));
        }
        let edge_id = value
            .get("edge_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Invalid("navigation output edge identity"))?;
        validate_identifier(
            edge_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation output edge ID",
        )?;
        if !edge_ids.insert(edge_id.to_owned()) {
            return Err(DurableError::Invalid("duplicate navigation edge ID"));
        }
        let from_id = value
            .get("from_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Invalid("navigation output edge source"))?;
        let to_id = value
            .get("to_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Invalid("navigation output edge target"))?;
        validate_identifier(
            from_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation output edge source ID",
        )?;
        validate_identifier(
            to_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation output edge target ID",
        )?;
        let from_visible = node_visibility_by_id.get(from_id).copied();
        let to_visible = node_visibility_by_id.get(to_id).copied();
        if from_visible.is_none() && !external_endpoint_ids.contains(from_id)
            || to_visible.is_none() && !external_endpoint_ids.contains(to_id)
        {
            return Err(DurableError::Invalid(
                "navigation event edge endpoint receipt is absent",
            ));
        }
        if let (Some(from_visible), Some(to_visible)) = (from_visible, to_visible)
            && *visible != (from_visible && to_visible)
        {
            return Err(DurableError::Conflict(
                "navigation event edge visibility witness differs from endpoints",
            ));
        }
        let encoded = canonical_value(value, limits.max_row_bytes)?;
        total_bytes = total_bytes
            .checked_add(encoded.len() as u64)
            .ok_or(DurableError::Refused("navigation event byte overflow"))?;
        edges.push(PersistedNavigationEdge {
            edge_id: edge_id.to_owned(),
            from_id: from_id.to_owned(),
            to_id: to_id.to_owned(),
            visible: *visible,
            sha256: Digest256::of_bytes(&encoded),
            row_json: encoded,
        });
    }
    for value in diagnostic_values {
        if !value.is_object() {
            return Err(DurableError::Invalid(
                "navigation diagnostic row must be an object",
            ));
        }
        let encoded = canonical_value(value, limits.max_row_bytes)?;
        total_bytes = total_bytes
            .checked_add(encoded.len() as u64)
            .ok_or(DurableError::Refused("navigation event byte overflow"))?;
        diagnostics.push(encoded);
    }
    let row_count = nodes
        .len()
        .checked_add(edges.len())
        .and_then(|count| count.checked_add(diagnostics.len()))
        .ok_or(DurableError::Refused("navigation event row count overflow"))?;
    if row_count > limits.max_cohort_rows || total_bytes > limits.max_total_bytes {
        return Err(DurableError::Refused(
            "navigation event cohort exceeds storage budget",
        ));
    }
    Ok(PersistedRows {
        dependencies: dependencies.to_vec(),
        nodes,
        edges,
        diagnostics,
        cohort_bytes: total_bytes,
    })
}

fn validate_navigation_claim_cohort(
    cohort: &NavigationClaimEventCohortV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<PersistedRows> {
    let integrity = cohort
        .verify_integrity()
        .map_err(|_| DurableError::Conflict("navigation Claim cohort integrity differs"))?;
    validate_identifier(
        cohort.event_id(),
        MAX_NAVIGATION_EVENT_ID_BYTES,
        "navigation event ID",
    )?;
    validate_identifier(
        cohort.source_event_id(),
        MAX_NAVIGATION_EVENT_ID_BYTES,
        "authored navigation event ID",
    )?;
    if cohort
        .event_id()
        .bytes()
        .any(|byte| byte.is_ascii_control())
        || cohort
            .source_event_id()
            .bytes()
            .any(|byte| byte.is_ascii_control())
        || cohort.event_sequence() == 0
        || cohort.event_sequence() > i64::MAX as u64
        || cohort.source_event_version() == 0
        || cohort.source_event_version() > i64::MAX as u64
        || cohort.predecessor_source_revision() == cohort.source_revision()
        || cohort.source_line() == 0
        || cohort.event_claim_ids().is_empty()
        || cohort.event_claim_ids().len() > limits.max_current_cohorts
        || cohort
            .event_claim_ids()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || cohort
            .event_claim_ids()
            .binary_search_by(|value| value.as_str().cmp(cohort.record_id()))
            .is_err()
    {
        return Err(DurableError::Invalid(
            "navigation Claim event identity or source set",
        ));
    }
    validate_relative_path(cohort.source_path().as_str())?;
    validate_identifier(
        cohort.record_id(),
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "navigation Claim ID",
    )?;
    let total_bytes = validate_navigation_dependency_envelope(
        cohort.dependencies(),
        cohort.source_path().as_str(),
        limits,
    )?;
    let mutation_receipt = cohort.mutation_receipt_path().as_str();
    validate_relative_path(mutation_receipt)?;
    let mutation_receipt_dependency = cohort
        .dependencies()
        .iter()
        .find(|dependency| dependency.path.as_str() == mutation_receipt)
        .ok_or(DurableError::Conflict(
            "navigation Claim mutation receipt dependency absent",
        ))?;
    match &mutation_receipt_dependency.state {
        NavigationDependencyStateV1::Present { sha256, size_bytes }
            if *sha256 == integrity.event_claim_set_binding
                && *size_bytes > 0
                && *size_bytes <= limits.max_row_bytes as u64 => {}
        _ => {
            return Err(DurableError::Conflict(
                "navigation Claim mutation receipt binding differs",
            ));
        }
    }
    persist_navigation_projection_rows(
        cohort.dependencies(),
        &cohort.projection().nodes,
        &cohort.projection().edges,
        &cohort.projection().diagnostics,
        integrity.node_visibility,
        integrity.edge_visibility,
        integrity.external_endpoint_ids,
        total_bytes,
        limits,
    )
}

fn validate_navigation_claim_refresh_cohort(
    cohort: &tos_compiler::source_navigation_claim_event::NavigationClaimDependencyRefreshCohortV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<PersistedRows> {
    let integrity = cohort
        .verify_integrity()
        .map_err(|_| DurableError::Conflict("navigation Claim refresh integrity differs"))?;
    validate_identifier(
        cohort.event_id(),
        MAX_NAVIGATION_EVENT_ID_BYTES,
        "navigation event ID",
    )?;
    validate_identifier(
        cohort.source_event_id(),
        MAX_NAVIGATION_EVENT_ID_BYTES,
        "authored navigation event ID",
    )?;
    if cohort
        .event_id()
        .bytes()
        .any(|byte| byte.is_ascii_control())
        || cohort
            .source_event_id()
            .bytes()
            .any(|byte| byte.is_ascii_control())
        || cohort.event_sequence() == 0
        || cohort.event_sequence() > i64::MAX as u64
        || cohort.source_event_version() == 0
        || cohort.source_event_version() > i64::MAX as u64
        || cohort.predecessor_source_revision() == cohort.source_revision()
        || cohort.source_line() == 0
        || cohort.event_claim_owner_ids().is_empty()
        || cohort.event_claim_owner_ids().len() > limits.max_current_cohorts
        || cohort
            .event_claim_owner_ids()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || cohort
            .event_claim_owner_ids()
            .binary_search_by(|value| value.as_str().cmp(cohort.record_id()))
            .is_err()
    {
        return Err(DurableError::Invalid(
            "navigation Claim refresh event identity or source set",
        ));
    }
    validate_relative_path(cohort.source_path().as_str())?;
    validate_identifier(
        cohort.record_id(),
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "navigation Claim ID",
    )?;
    let total_bytes = validate_navigation_dependency_envelope(
        cohort.dependencies(),
        cohort.source_path().as_str(),
        limits,
    )?;
    persist_navigation_projection_rows(
        cohort.dependencies(),
        &cohort.projection().nodes,
        &cohort.projection().edges,
        &cohort.projection().diagnostics,
        integrity.node_visibility,
        integrity.edge_visibility,
        integrity.external_endpoint_ids,
        total_bytes,
        limits,
    )
}

fn verify_navigation_claim_refresh_dependency_cut(
    cohort: &tos_compiler::source_navigation_claim_event::NavigationClaimDependencyRefreshCohortV1,
    selected: &[DurableNavigationEventSourceV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let selected = selected
        .iter()
        .map(|source| (source.source_path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    for dependency in cohort.dependencies() {
        let source =
            selected
                .get(dependency.path.as_str())
                .copied()
                .ok_or(DurableError::Corrupt(
                    "navigation Claim refresh dependency absent from selected cut",
                ))?;
        match &dependency.state {
            NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                let raw = source
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation Claim refresh present dependency lacks exact bytes",
                    ))?;
                if !source.is_source
                    || source.content_digest != Some(*sha256)
                    || source.content_length != Some(*size_bytes)
                    || raw.len() as u64 != *size_bytes
                    || Digest256::of_bytes(raw) != *sha256
                    || source
                        .commit_sequence
                        .is_none_or(|sequence| sequence > cohort.event_sequence())
                    || *size_bytes
                        > if dependency.role == NavigationDependencyRoleV1::Event {
                            limits.max_total_bytes
                        } else {
                            limits.max_row_bytes as u64
                        }
                {
                    return Err(DurableError::Conflict(
                        "navigation Claim refresh dependency differs from selected bytes",
                    ));
                }
            }
            NavigationDependencyStateV1::Absent { .. } if source.is_source => {
                return Err(DurableError::Conflict(
                    "navigation Claim refresh absent dependency exists at selected cut",
                ));
            }
            NavigationDependencyStateV1::Absent { .. } => {}
        }
    }
    for cause in cohort.proof().changed_dependencies() {
        let path = cause.changed_dependency_path().as_str();
        let source = selected.get(path).copied().ok_or(DurableError::Corrupt(
            "navigation Claim refresh changed member absent",
        ))?;
        let dependency_matches = cohort.dependencies().iter().any(|dependency| {
            dependency.path.as_str() == path
                && matches!(
                    &dependency.state,
                    NavigationDependencyStateV1::Present { sha256, .. }
                        if *sha256 == cause.changed_dependency_sha256()
                )
        });
        if !source.is_source
            || source.commit_sequence != Some(cohort.event_sequence())
            || source.content_digest != Some(cause.changed_dependency_sha256())
            || !dependency_matches
        {
            return Err(DurableError::Conflict(
                "navigation Claim refresh cause differs from exact changed member",
            ));
        }
    }
    let claim_stream =
        selected
            .get(cohort.source_path().as_str())
            .copied()
            .ok_or(DurableError::Corrupt(
                "navigation Claim refresh source stream absent",
            ))?;
    if !claim_stream.is_source
        || claim_stream
            .commit_sequence
            .is_none_or(|sequence| sequence > cohort.event_sequence())
        || claim_stream.content_digest != Some(cohort.claim_stream_sha256())
    {
        return Err(DurableError::Conflict(
            "navigation Claim refresh source stream differs from selected cut",
        ));
    }
    Ok(())
}

fn cohort_roots(
    cohort: &NavigationRecordEventCohortV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<(Digest256, Digest256, Digest256)> {
    cohort_roots_parts(
        &cohort.event_id,
        cohort.event_sequence,
        &cohort.source_event_id,
        cohort.source_event_version,
        cohort.predecessor_source_revision,
        cohort.source_revision,
        cohort.predecessor_cohort_root,
        cohort.source_path.as_str(),
        &cohort.record_id,
        cohort.basis(),
        &cohort.dependencies,
        &cohort.projection,
        cohort.node_visibility(),
        cohort.edge_visibility(),
        limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn cohort_roots_parts(
    event_id: &str,
    event_sequence: u64,
    source_event_id: &str,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    predecessor_cohort_root: Option<Digest256>,
    source_path: &str,
    record_id: &str,
    basis: &NavigationRecordEventBasisV1,
    dependencies: &[NavigationDependencyV1],
    projection: &tos_compiler::source_bibliographic::SuppliedNavigationRecordProjection,
    node_visibility: &[bool],
    edge_visibility: &[bool],
    limits: DurableNavigationEventLimits,
) -> DurableResult<(Digest256, Digest256, Digest256)> {
    let dependency_rows = dependencies
        .iter()
        .map(|dependency| {
            let observation = match &dependency.state {
                NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                    json!({"state":"present","sha256":sha256.to_hex(),"size_bytes":size_bytes})
                }
                NavigationDependencyStateV1::Absent { reason } => {
                    json!({"state":"absent","reason":reason})
                }
            };
            json!({
                "path":dependency.path.as_str(),
                "role":dependency_role(dependency.role),
                "observation":observation
            })
        })
        .collect::<Vec<_>>();
    let dependency_root = digest_json(
        &json!({
            "schema":"tos_source_navigation_dependencies_v1",
            "dependencies":dependency_rows
        }),
        limits.max_total_bytes as usize,
    )?;
    if node_visibility.len() != projection.nodes.len()
        || edge_visibility.len() != projection.edges.len()
    {
        return Err(DurableError::Invalid(
            "navigation event visibility witness count",
        ));
    }
    let mut rows = BTreeMap::<(String, String), (Digest256, bool)>::new();
    for (collection, values, id_field, visibility) in [
        ("nodes", &projection.nodes, "node_id", node_visibility),
        ("edges", &projection.edges, "edge_id", edge_visibility),
    ] {
        for (row, visible) in values.iter().zip(visibility) {
            let id = row
                .get(id_field)
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or(DurableError::Invalid("navigation output row identity"))?;
            if rows
                .insert(
                    (collection.to_owned(), id.to_owned()),
                    (digest_json(row, limits.max_row_bytes)?, *visible),
                )
                .is_some()
            {
                return Err(DurableError::Invalid(
                    "duplicate navigation output row identity",
                ));
            }
        }
    }
    let diagnostic_digests = projection
        .diagnostics
        .iter()
        .map(|row| digest_json(row, limits.max_row_bytes).map(|digest| digest.to_hex()))
        .collect::<DurableResult<Vec<_>>>()?;
    let manifest = rows
        .into_iter()
        .map(|((collection, id), (digest, visible))| {
            json!([collection, id, digest.to_hex(), visible])
        })
        .collect::<Vec<_>>();
    let output_root = digest_json(
        &json!({
            "schema":"tos_source_navigation_record_cohort_v1",
            "rows":manifest,
            "diagnostics":diagnostic_digests
        }),
        limits.max_total_bytes as usize,
    )?;
    let event_path = dependencies
        .iter()
        .find(|dependency| dependency.role == NavigationDependencyRoleV1::Event)
        .ok_or(DurableError::Invalid("navigation event dependency absent"))?;
    let record_dependency = dependencies
        .iter()
        .find(|dependency| {
            dependency.path.as_str() == source_path
                && dependency.role == NavigationDependencyRoleV1::Record
        })
        .ok_or(DurableError::Invalid("navigation record dependency absent"))?;
    let source_sha256 = match &record_dependency.state {
        NavigationDependencyStateV1::Present { sha256, .. } => *sha256,
        NavigationDependencyStateV1::Absent { .. } => {
            return Err(DurableError::Invalid(
                "navigation record dependency is absent",
            ));
        }
    };
    let (input_schema, basis) = match basis {
        NavigationRecordEventBasisV1::Mutation => (
            "tos_source_navigation_event_input_v2",
            json!({"kind":"record-mutation"}),
        ),
        NavigationRecordEventBasisV1::RangeReconciliation {
            coverage_root,
            cause_root,
        } => (
            "tos_source_navigation_event_input_v5",
            json!({
                "kind":"range-reconciliation",
                "coverage_root":coverage_root.to_hex(),
                "cause_root":cause_root.to_hex(),
            }),
        ),
        NavigationRecordEventBasisV1::DependencyRefresh {
            changed_dependency_path,
            changed_dependency_sha256,
        } => (
            "tos_source_navigation_event_input_v2",
            json!({
                "kind":"dependency-refresh",
                "changed_dependency_path":changed_dependency_path.as_str(),
                "changed_dependency_sha256":changed_dependency_sha256.to_hex(),
            }),
        ),
        NavigationRecordEventBasisV1::DependencyRefreshSet {
            changed_dependencies,
        } => {
            let (causes, _) = navigation_record_dependency_refresh_causes_value(
                changed_dependencies,
                source_path,
                limits.max_total_bytes as usize,
            )?;
            (
                "tos_source_navigation_event_input_v4",
                json!({
                    "kind":"dependency-refresh-set",
                    "changed_dependencies":causes,
                }),
            )
        }
        NavigationRecordEventBasisV1::ClaimRevisionRefresh {
            changed_dependency_path,
            changed_dependency_sha256,
            history_path,
            history_sha256,
            claim_id,
            revision_proof_root,
        } => (
            "tos_source_navigation_event_input_v3",
            json!({
                "kind":"claim-revision-refresh",
                "changed_dependency_path":changed_dependency_path.as_str(),
                "changed_dependency_sha256":changed_dependency_sha256.to_hex(),
                "history_path":history_path.as_str(),
                "history_sha256":history_sha256.to_hex(),
                "claim_id":claim_id,
                "revision_proof_root":revision_proof_root.to_hex(),
            }),
        ),
    };
    let input_root = digest_json(
        &json!({
            "schema":input_schema,
            "event_id":event_id,
            "event_sequence":event_sequence,
            "source_event_id":source_event_id,
            "source_event_version":source_event_version,
            "predecessor_source_revision":predecessor_source_revision.to_hex(),
            "source_revision":source_revision.to_hex(),
            "predecessor_cohort_root":predecessor_cohort_root.map(Digest256::to_hex),
            "event_path":event_path.path.as_str(),
            "source_path":source_path,
            "source_sha256":source_sha256.to_hex(),
            "record_id":record_id,
            "basis":basis,
            "dependency_root":dependency_root.to_hex(),
            "output_root":output_root.to_hex()
        }),
        limits.max_total_bytes as usize,
    )?;
    Ok((dependency_root, output_root, input_root))
}

fn verify_navigation_consumer<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
) -> DurableResult<()> {
    let row = tx
        .query_opt(
            "SELECT profile_id,profile_version,source_contract_digest
             FROM cmd2_projection_navigation_consumer WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?
        .ok_or(DurableError::Invalid(
            "navigation event consumer is not initialized",
        ))?;
    let profile_version = profile.version.to_string();
    if row.get::<_, String>("profile_id") != profile.id
        || row.get::<_, String>("profile_version") != profile_version
        || row.get::<_, String>("source_contract_digest") != profile.source_contract.to_hex()
    {
        return Err(DurableError::Conflict(
            "navigation event consumer profile differs",
        ));
    }
    Ok(())
}

fn read_navigation_event_head(
    client: &mut postgres::Client,
    domain: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
) -> DurableResult<u64> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let snapshot = read_domain_snapshot(&mut tx, domain)?;
    snapshot.require_live_projection(profile)?;
    let head = snapshot.head_seq()?;
    tx.commit()?;
    Ok(head)
}

fn read_navigation_event_cursor(
    client: &mut postgres::Client,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationEventCursorV1> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let row = tx.query_opt(
        "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,
                source_revision,last_event_id,last_input_root,last_output_root
         FROM cmd2_projection_navigation_consumer WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer_id],
    )?;
    let Some(row) = row else {
        tx.commit()?;
        return Ok(DurableNavigationEventCursorV1 {
            sequence: 0,
            source_revision: None,
            last_event_id: None,
            last_input_root: None,
            last_output_root: None,
        });
    };
    let profile_version = profile.version.to_string();
    if row.get::<_, String>("profile_id") != profile.id
        || row.get::<_, String>("profile_version") != profile_version
        || row.get::<_, String>("source_contract_digest") != profile.source_contract.to_hex()
    {
        return Err(DurableError::Conflict(
            "navigation event consumer profile differs",
        ));
    }
    let sequence = as_u64(row.get("cursor_seq"))?;
    let source_revision = row
        .get::<_, Option<String>>("source_revision")
        .map(parse_digest)
        .transpose()?;
    let last_event_id: Option<String> = row.get("last_event_id");
    let last_input_root = row
        .get::<_, Option<String>>("last_input_root")
        .map(parse_digest)
        .transpose()?;
    let last_output_root = row
        .get::<_, Option<String>>("last_output_root")
        .map(parse_digest)
        .transpose()?;
    let coherent = if sequence == 0 {
        source_revision.is_none()
            && last_event_id.is_none()
            && last_input_root.is_none()
            && last_output_root.is_none()
    } else {
        source_revision.is_some()
            && last_event_id.as_ref().is_some_and(|id| !id.is_empty())
            && last_input_root.is_some()
            && last_output_root.is_some()
    };
    if !coherent {
        return Err(DurableError::Corrupt(
            "navigation event consumer cursor state is inconsistent",
        ));
    }
    tx.commit()?;
    Ok(DurableNavigationEventCursorV1 {
        sequence,
        source_revision,
        last_event_id,
        last_input_root,
        last_output_root,
    })
}

fn read_dependencies<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    source_path: &str,
    record_id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Vec<DurableNavigationEventDependencyReadV1>> {
    let cap = i64::try_from(limits.max_dependencies.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation dependency count limit"))?;
    let rows = tx.query(
        "SELECT dependency_path,dependency_role,observation_state,sha256,size_bytes,absent_reason
         FROM cmd2_projection_navigation_dependency
         WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4
         ORDER BY dependency_path COLLATE \"C\" LIMIT $5",
        &[&domain, &consumer_id, &source_path, &record_id, &cap],
    )?;
    if rows.len() > limits.max_dependencies {
        return Err(DurableError::Corrupt(
            "stored navigation dependency count exceeds limit",
        ));
    }
    rows.into_iter()
        .map(|row| {
            let sha256: Option<String> = row.get("sha256");
            let size_bytes: Option<i64> = row.get("size_bytes");
            Ok(DurableNavigationEventDependencyReadV1 {
                path: row.get("dependency_path"),
                role: row.get("dependency_role"),
                sha256: sha256.map(parse_digest).transpose()?,
                size_bytes: size_bytes.map(as_u64).transpose()?,
                absent_reason: row.get("absent_reason"),
            })
        })
        .collect()
}

fn read_rows<C: GenericClient>(
    tx: &mut C,
    table: &str,
    id_column: &str,
    domain: &str,
    consumer_id: &str,
    source_path: &str,
    record_id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<(Vec<Value>, Vec<bool>)> {
    // These identifiers are static call-site constants, not request values.
    let query = format!(
        "SELECT {id_column},visible,row_json,row_sha256,from_id,to_id FROM {table}
         WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4
         ORDER BY {id_column} COLLATE \"C\" LIMIT $5"
    );
    let cap = i64::try_from(limits.max_cohort_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation row count limit"))?;
    let rows = tx.query(
        &query,
        &[&domain, &consumer_id, &source_path, &record_id, &cap],
    )?;
    if rows.len() > limits.max_cohort_rows {
        return Err(DurableError::Corrupt(
            "stored navigation row count exceeds limit",
        ));
    }
    let mut values = Vec::with_capacity(rows.len());
    let mut visibility = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get(id_column);
        let visible: Option<bool> = row.get("visible");
        let raw: String = row.get("row_json");
        let raw_bytes = raw.as_bytes();
        let row_sha256 = parse_digest(row.get("row_sha256"))?;
        let from_id: Option<String> = row.get("from_id");
        let to_id: Option<String> = row.get("to_id");
        if raw_bytes.len() > limits.max_row_bytes
            || visible.is_none()
            || Digest256::of_bytes(raw_bytes) != row_sha256
        {
            return Err(DurableError::Corrupt(
                "stored navigation row metadata differs",
            ));
        }
        let value = strict_json_value(raw_bytes, limits.max_row_bytes)?;
        if canonical_value(&value, limits.max_row_bytes)? != raw_bytes
            || value.get(id_column).and_then(Value::as_str) != Some(id.as_str())
        {
            return Err(DurableError::Corrupt(
                "stored navigation row is not canonical",
            ));
        }
        if id_column == "edge_id" {
            let expected_from = value.get("from_id").and_then(Value::as_str);
            let expected_to = value.get("to_id").and_then(Value::as_str);
            if expected_from.is_none()
                || expected_to.is_none()
                || from_id.as_deref() != expected_from
                || to_id.as_deref() != expected_to
            {
                return Err(DurableError::Corrupt(
                    "stored navigation edge endpoint metadata differs",
                ));
            }
        } else if from_id.is_some() || to_id.is_some() {
            return Err(DurableError::Corrupt(
                "stored navigation node has edge endpoint metadata",
            ));
        }
        values.push(value);
        visibility.push(visible.unwrap_or(false));
    }
    Ok((values, visibility))
}

fn read_diagnostics<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    source_path: &str,
    record_id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Vec<Value>> {
    let cap = i64::try_from(limits.max_cohort_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation row count limit"))?;
    let rows = tx.query(
        "SELECT diagnostic_ordinal,row_json
         FROM cmd2_projection_navigation_diagnostic
         WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4
         ORDER BY diagnostic_ordinal LIMIT $5",
        &[&domain, &consumer_id, &source_path, &record_id, &cap],
    )?;
    if rows.len() > limits.max_cohort_rows {
        return Err(DurableError::Corrupt(
            "stored navigation diagnostic count exceeds limit",
        ));
    }
    let mut diagnostics = Vec::with_capacity(rows.len());
    for (expected, row) in rows.into_iter().enumerate() {
        if row.get::<_, i32>("diagnostic_ordinal") != expected as i32 {
            return Err(DurableError::Corrupt(
                "stored navigation diagnostic order has a gap",
            ));
        }
        let raw: String = row.get("row_json");
        if raw.len() > limits.max_row_bytes {
            return Err(DurableError::Corrupt(
                "stored navigation diagnostic exceeds byte limit",
            ));
        }
        let value = strict_json_value(raw.as_bytes(), limits.max_row_bytes)?;
        if canonical_value(&value, limits.max_row_bytes)? != raw.as_bytes() {
            return Err(DurableError::Corrupt(
                "stored navigation diagnostic is not canonical",
            ));
        }
        diagnostics.push(value);
    }
    Ok(diagnostics)
}

fn verify_read_roots(
    read: &DurableNavigationEventCohortReadV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    if read.dependencies.is_empty() || read.dependencies.len() > limits.max_dependencies {
        return Err(DurableError::Corrupt(
            "stored navigation dependency count is invalid",
        ));
    }
    let mut dependencies = Vec::with_capacity(read.dependencies.len());
    for dependency in &read.dependencies {
        let path = RelativePath::parse(&dependency.path)
            .map_err(|_| DurableError::Corrupt("stored navigation dependency path"))?;
        let role = parse_dependency_role(&dependency.role)?;
        let state = match (
            dependency.sha256,
            dependency.size_bytes,
            dependency.absent_reason.as_deref(),
        ) {
            (Some(sha256), Some(size_bytes), None) => {
                NavigationDependencyStateV1::Present { sha256, size_bytes }
            }
            (None, None, Some(reason)) => NavigationDependencyStateV1::Absent {
                reason: reason.to_owned(),
            },
            _ => {
                return Err(DurableError::Corrupt(
                    "stored navigation dependency state is inconsistent",
                ));
            }
        };
        dependencies.push(NavigationDependencyV1 { path, role, state });
    }
    if dependencies
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(DurableError::Corrupt(
            "stored navigation dependencies are not sorted",
        ));
    }
    if matches!(
        read.basis_kind.as_str(),
        "claim-mutation" | "claim-dependency-refresh"
    ) {
        return verify_claim_read_roots(read, &dependencies, limits);
    }
    let projection = tos_compiler::source_bibliographic::SuppliedNavigationRecordProjection {
        nodes: read.nodes.clone(),
        edges: read.edges.clone(),
        diagnostics: read.diagnostics.clone(),
    };
    let basis = match read.basis_kind.as_str() {
        "record-mutation"
            if read.basis_dependency_path.is_none()
                && read.basis_dependency_sha256.is_none()
                && read.basis_dependency_causes.is_empty()
                && read.basis_dependency_causes_root.is_none() =>
        {
            NavigationRecordEventBasisV1::Mutation
        }
        "dependency-refresh" => NavigationRecordEventBasisV1::DependencyRefresh {
            changed_dependency_path: RelativePath::parse(
                read.basis_dependency_path
                    .as_deref()
                    .ok_or(DurableError::Corrupt(
                        "stored navigation refresh dependency path absent",
                    ))?,
            )
            .map_err(|_| DurableError::Corrupt("stored navigation refresh path invalid"))?,
            changed_dependency_sha256: read.basis_dependency_sha256.ok_or(
                DurableError::Corrupt("stored navigation refresh dependency digest absent"),
            )?,
        },
        "dependency-refresh-set"
            if read.basis_dependency_path.is_none()
                && read.basis_dependency_sha256.is_none()
                && read.basis_dependency_causes_root.is_some() =>
        {
            let causes = read
                .basis_dependency_causes
                .iter()
                .map(|cause| {
                    Ok(NavigationRecordDependencyRefreshCauseV1 {
                        path: RelativePath::parse(&cause.path).map_err(|_| {
                            DurableError::Corrupt(
                                "stored Record dependency-refresh cause path is invalid",
                            )
                        })?,
                        sha256: cause.sha256,
                    })
                })
                .collect::<DurableResult<Vec<_>>>()?;
            let (_, causes_root) = navigation_record_dependency_refresh_causes_value(
                &causes,
                &read.source_path,
                limits.max_row_bytes as usize,
            )?;
            if Some(causes_root) != read.basis_dependency_causes_root {
                return Err(DurableError::Corrupt(
                    "stored Record dependency-refresh cause root differs",
                ));
            }
            NavigationRecordEventBasisV1::DependencyRefreshSet {
                changed_dependencies: causes,
            }
        }
        _ => {
            return Err(DurableError::Corrupt(
                "stored navigation record basis is unsupported",
            ));
        }
    };
    let (dependency_root, output_root, input_root) = cohort_roots_parts(
        &read.event_id,
        read.event_sequence,
        &read.source_event_id,
        read.source_event_version,
        read.predecessor_source_revision,
        read.source_revision,
        read.predecessor_cohort_root,
        &read.source_path,
        &read.record_id,
        &basis,
        &dependencies,
        &projection,
        &read.node_visibility,
        &read.edge_visibility,
        limits,
    )?;
    if dependency_root != read.dependency_root
        || output_root != read.output_root
        || input_root != read.input_root
    {
        return Err(DurableError::Corrupt(
            "stored navigation cohort roots differ",
        ));
    }
    Ok(())
}

fn verify_claim_read_roots(
    read: &DurableNavigationEventCohortReadV1,
    dependencies: &[NavigationDependencyV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    if read.basis_dependency_path.is_some()
        || read.basis_dependency_sha256.is_some()
        || !read.basis_dependency_causes.is_empty()
        || read.basis_dependency_causes_root.is_some()
    {
        return Err(DurableError::Corrupt(
            "stored navigation Claim basis has record dependency fields",
        ));
    }
    let source_line = read.claim_source_line.ok_or(DurableError::Corrupt(
        "stored navigation Claim source line is absent",
    ))?;
    let trace_sha256 = read.claim_trace_sha256.ok_or(DurableError::Corrupt(
        "stored navigation Claim trace digest is absent",
    ))?;
    let claim_event_basis_root = read.claim_event_basis_root.ok_or(DurableError::Corrupt(
        "stored navigation Claim event basis root is absent",
    ))?;
    let is_refresh = read.basis_kind == "claim-dependency-refresh";
    if !is_refresh && read.basis_kind != "claim-mutation" {
        return Err(DurableError::Corrupt(
            "stored navigation Claim basis is unsupported",
        ));
    }
    let event_claim_set_root = if is_refresh {
        if read.claim_event_set_root.is_some()
            || read.claim_mutation_receipt_path.is_some()
            || read.claim_mutation_receipt_sha256.is_some()
            || read.claim_owner_inventory_root.is_none()
            || read.claim_stream_rows_scanned.is_none()
            || read.claim_navigation_manifest_root.is_none()
        {
            return Err(DurableError::Corrupt(
                "stored navigation Claim refresh metadata is inconsistent",
            ));
        }
        None
    } else {
        let root = read.claim_event_set_root.ok_or(DurableError::Corrupt(
            "stored navigation Claim-set root is absent",
        ))?;
        if root != claim_event_basis_root {
            return Err(DurableError::Corrupt(
                "stored navigation Claim event basis root differs",
            ));
        }
        Some(root)
    };
    let mutation_receipt_path = read.claim_mutation_receipt_path.as_deref();
    let mutation_receipt_sha256 = read.claim_mutation_receipt_sha256;
    if !is_refresh {
        let path = mutation_receipt_path.ok_or(DurableError::Corrupt(
            "stored navigation Claim mutation receipt path is absent",
        ))?;
        validate_relative_path(path)?;
        if mutation_receipt_sha256.is_none() {
            return Err(DurableError::Corrupt(
                "stored navigation Claim receipt digest is absent",
            ));
        }
    }
    let claim_stream_sha256 = read.claim_stream_sha256.ok_or(DurableError::Corrupt(
        "stored navigation Claim stream digest is absent",
    ))?;
    if mutation_receipt_path == Some(read.source_path.as_str()) {
        return Err(DurableError::Corrupt(
            "stored navigation Claim receipt aliases the Claim stream",
        ));
    }
    let verify_present_dependency = |path: &str,
                                     role: NavigationDependencyRoleV1,
                                     expected_sha256: Digest256|
     -> DurableResult<()> {
        let dependency = dependencies
            .iter()
            .find(|dependency| dependency.path.as_str() == path)
            .ok_or(DurableError::Corrupt(
                "stored navigation Claim dependency is absent",
            ))?;
        if dependency.role != role
            || !matches!(
                &dependency.state,
                NavigationDependencyStateV1::Present { sha256, .. } if *sha256 == expected_sha256
            )
        {
            return Err(DurableError::Corrupt(
                "stored navigation Claim dependency digest differs",
            ));
        }
        Ok(())
    };
    verify_present_dependency(
        &read.source_path,
        NavigationDependencyRoleV1::Record,
        claim_stream_sha256,
    )?;
    if let (Some(path), Some(sha256)) = (mutation_receipt_path, mutation_receipt_sha256) {
        verify_present_dependency(path, NavigationDependencyRoleV1::Additional, sha256)?;
    }
    let dependency_rows = dependencies
        .iter()
        .map(|dependency| {
            let observation = match &dependency.state {
                NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                    json!({"state":"present","sha256":sha256.to_hex(),"size_bytes":size_bytes})
                }
                NavigationDependencyStateV1::Absent { reason } => {
                    json!({"state":"absent","reason":reason})
                }
            };
            json!({
                "path":dependency.path.as_str(),
                "role":dependency_role(dependency.role),
                "observation":observation
            })
        })
        .collect::<Vec<_>>();
    let dependency_root = digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_dependencies_v1",
            "dependencies":dependency_rows,
        }),
        limits.max_total_bytes as usize,
    )?;

    if read.nodes.len() != read.node_visibility.len()
        || read.edges.len() != read.edge_visibility.len()
    {
        return Err(DurableError::Corrupt(
            "stored navigation Claim visibility witness count differs",
        ));
    }
    let mut rows = BTreeMap::<(String, String), (Digest256, bool)>::new();
    let mut node_ids = BTreeSet::new();
    for (row, visible) in read.nodes.iter().zip(&read.node_visibility) {
        let id = row
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt(
                "stored navigation Claim node ID is absent",
            ))?;
        validate_identifier(
            id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "stored navigation Claim node ID",
        )?;
        if !node_ids.insert(id.to_owned())
            || rows
                .insert(
                    ("nodes".to_owned(), id.to_owned()),
                    (digest_json(row, limits.max_row_bytes)?, *visible),
                )
                .is_some()
        {
            return Err(DurableError::Corrupt(
                "stored navigation Claim node ID is duplicated",
            ));
        }
    }
    let mut external_endpoint_ids = BTreeSet::new();
    for (row, visible) in read.edges.iter().zip(&read.edge_visibility) {
        let id = row
            .get("edge_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt(
                "stored navigation Claim edge ID is absent",
            ))?;
        validate_identifier(
            id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "stored navigation Claim edge ID",
        )?;
        let from_id = row
            .get("from_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt(
                "stored navigation Claim edge source is absent",
            ))?;
        let to_id = row
            .get("to_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt(
                "stored navigation Claim edge target is absent",
            ))?;
        validate_identifier(
            from_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "stored Claim edge source",
        )?;
        validate_identifier(
            to_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "stored Claim edge target",
        )?;
        if !node_ids.contains(from_id) {
            if tos_compiler::source_bibliographic::navigation_claim_subject_identity(
                row,
                &read.nodes,
                &read.edges,
            )
            .map_err(|_| DurableError::Corrupt("stored Claim subject anchor differs"))?
            .is_none()
            {
                return Err(DurableError::Corrupt(
                    "stored navigation Claim edge source is not in its cohort",
                ));
            }
            external_endpoint_ids.insert(from_id.to_owned());
        }
        if !node_ids.contains(to_id) {
            if to_id.starts_with("claim:") {
                external_endpoint_ids.insert(to_id.to_owned());
            } else {
                return Err(DurableError::Corrupt(
                    "stored navigation Claim edge target is not in its cohort",
                ));
            }
        }
        if rows
            .insert(
                ("edges".to_owned(), id.to_owned()),
                (digest_json(row, limits.max_row_bytes)?, *visible),
            )
            .is_some()
        {
            return Err(DurableError::Corrupt(
                "stored navigation Claim edge ID is duplicated",
            ));
        }
    }
    let diagnostics = read
        .diagnostics
        .iter()
        .map(|row| digest_json(row, limits.max_row_bytes).map(Digest256::to_hex))
        .collect::<DurableResult<Vec<_>>>()?;
    let output_manifest = rows
        .into_iter()
        .map(|((collection, id), (digest, visible))| {
            json!([collection, id, digest.to_hex(), visible])
        })
        .collect::<Vec<_>>();
    let external_endpoint_ids = external_endpoint_ids.into_iter().collect::<Vec<_>>();
    let output_root = digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_cohort_v1",
            "rows":output_manifest,
            "diagnostics":diagnostics,
            "trace_sha256":trace_sha256.to_hex(),
            "external_endpoint_ids":external_endpoint_ids,
        }),
        limits.max_total_bytes as usize,
    )?;
    let input_root = if is_refresh {
        digest_json(
            &json!({
                "schema":"tos_source_navigation_claim_dependency_refresh_input_v1",
                "event_id":read.event_id,
                "event_sequence":read.event_sequence,
                "source_event_id":read.source_event_id,
                "source_event_version":read.source_event_version,
                "predecessor_source_revision":read.predecessor_source_revision.to_hex(),
                "source_revision":read.source_revision.to_hex(),
                "predecessor_cohort_root":read.predecessor_cohort_root.map(Digest256::to_hex),
                "source_path":read.source_path,
                "source_line":source_line,
                "record_id":read.record_id,
                "dependency_root":dependency_root.to_hex(),
                "output_root":output_root.to_hex(),
                "event_basis_root":claim_event_basis_root.to_hex(),
                "owner_inventory_root":read.claim_owner_inventory_root
                    .ok_or(DurableError::Corrupt("stored Claim refresh inventory root absent"))?
                    .to_hex(),
                "claim_stream_rows_scanned":read.claim_stream_rows_scanned
                    .ok_or(DurableError::Corrupt("stored Claim refresh stream row count absent"))?,
            }),
            limits.max_total_bytes as usize,
        )?
    } else {
        digest_json(
            &json!({
                "schema":"tos_source_navigation_claim_event_input_v1",
                "event_id":read.event_id,
                "event_sequence":read.event_sequence,
                "source_event_id":read.source_event_id,
                "source_event_version":read.source_event_version,
                "predecessor_source_revision":read.predecessor_source_revision.to_hex(),
                "source_revision":read.source_revision.to_hex(),
                "predecessor_cohort_root":read.predecessor_cohort_root.map(Digest256::to_hex),
                "source_path":read.source_path,
                "source_line":source_line,
                "record_id":read.record_id,
                "mutation_receipt_path":mutation_receipt_path.ok_or(
                    DurableError::Corrupt("stored Claim mutation receipt path absent"),
                )?,
                "dependency_root":dependency_root.to_hex(),
                "output_root":output_root.to_hex(),
                "event_claim_set_root":event_claim_set_root.ok_or(
                    DurableError::Corrupt("stored Claim event set root absent"),
                )?.to_hex(),
            }),
            limits.max_total_bytes as usize,
        )?
    };
    if dependency_root != read.dependency_root
        || output_root != read.output_root
        || input_root != read.input_root
    {
        return Err(DurableError::Corrupt(
            "stored navigation Claim cohort roots differ",
        ));
    }
    Ok(())
}

fn parse_dependency_role(role: &str) -> DurableResult<NavigationDependencyRoleV1> {
    match role {
        "event" => Ok(NavigationDependencyRoleV1::Event),
        "event-schema" => Ok(NavigationDependencyRoleV1::EventSchema),
        "event-receipt" => Ok(NavigationDependencyRoleV1::EventReceipt),
        "record" => Ok(NavigationDependencyRoleV1::Record),
        "entity-registry" => Ok(NavigationDependencyRoleV1::EntityRegistry),
        "entity-registry-schema" => Ok(NavigationDependencyRoleV1::EntityRegistrySchema),
        "record-schema" => Ok(NavigationDependencyRoleV1::RecordSchema),
        "human-forms" => Ok(NavigationDependencyRoleV1::HumanForms),
        "forms-schema" => Ok(NavigationDependencyRoleV1::FormsSchema),
        "record-history" => Ok(NavigationDependencyRoleV1::RecordHistory),
        "version-source" => Ok(NavigationDependencyRoleV1::VersionSource),
        "version-schema" => Ok(NavigationDependencyRoleV1::VersionSchema),
        "additional" => Ok(NavigationDependencyRoleV1::Additional),
        _ => Err(DurableError::Corrupt("stored navigation dependency role")),
    }
}

fn dependency_role(role: NavigationDependencyRoleV1) -> &'static str {
    match role {
        NavigationDependencyRoleV1::Event => "event",
        NavigationDependencyRoleV1::EventSchema => "event-schema",
        NavigationDependencyRoleV1::EventReceipt => "event-receipt",
        NavigationDependencyRoleV1::Record => "record",
        NavigationDependencyRoleV1::EntityRegistry => "entity-registry",
        NavigationDependencyRoleV1::EntityRegistrySchema => "entity-registry-schema",
        NavigationDependencyRoleV1::RecordSchema => "record-schema",
        NavigationDependencyRoleV1::HumanForms => "human-forms",
        NavigationDependencyRoleV1::FormsSchema => "forms-schema",
        NavigationDependencyRoleV1::RecordHistory => "record-history",
        NavigationDependencyRoleV1::VersionSource => "version-source",
        NavigationDependencyRoleV1::VersionSchema => "version-schema",
        NavigationDependencyRoleV1::Additional => "additional",
    }
}

fn digest_json(value: &Value, cap: usize) -> DurableResult<Digest256> {
    Ok(Digest256::of_bytes(&canonical_value(value, cap)?))
}

fn canonical_value(value: &Value, cap: usize) -> DurableResult<Vec<u8>> {
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| DurableError::Refused("navigation canonical JSON limit"))?;
    let raw = serde_json::to_vec(value)
        .map_err(|_| DurableError::Invalid("navigation projection JSON"))?;
    let document = parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| DurableError::Invalid("navigation projection JSON"))?;
    canonical_bytes_v1(
        &document.into_root(),
        CanonicalProfile::CorpusSnapshotV1,
        limits,
    )
    .map_err(|_| DurableError::Refused("navigation canonical JSON budget"))
}

fn strict_json_value(raw: &[u8], cap: usize) -> DurableResult<Value> {
    if raw.is_empty() || raw.len() > cap {
        return Err(DurableError::Refused("navigation source JSON byte budget"));
    }
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| DurableError::Refused("navigation source JSON limit"))?;
    let document = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| DurableError::Invalid("navigation exact source JSON"))?;
    let normalized = tos_foundation::emit_value_preserved_json(document.root(), limits)
        .map_err(|_| DurableError::Refused("navigation source JSON byte budget"))?;
    serde_json::from_slice(&normalized)
        .map_err(|_| DurableError::Invalid("navigation exact source JSON value"))
}

fn parse_digest(value: String) -> DurableResult<Digest256> {
    Digest256::from_hex(value.trim_end())
        .map_err(|_| DurableError::Corrupt("navigation stored digest is invalid"))
}

fn active_navigation(deadline: Instant) -> DurableResult<()> {
    if Instant::now() >= deadline {
        Err(DurableError::Refused(
            "navigation event projection deadline exceeded",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod verified_prefix_tests {
    use super::*;

    fn test_digest(label: &[u8]) -> Digest256 {
        Digest256::of_bytes(label)
    }

    fn checkpoint_digest(owner_binding_json: &str, cohort_root: Digest256) -> Digest256 {
        let profile = DurableProjectionProfile {
            id: "test-profile".to_owned(),
            version: 2,
            source_contract: test_digest(b"source-contract"),
        };
        navigation_verified_prefix_digest(
            "test-domain",
            "test-consumer",
            &profile,
            10,
            test_digest(b"bootstrap-root"),
            owner_binding_json,
            20,
            test_digest(b"coverage-root"),
            cohort_root,
            test_digest(b"adjacency-root"),
            "event-20",
            test_digest(b"source-revision"),
            test_digest(b"input-root"),
            test_digest(b"output-root"),
        )
    }

    #[test]
    fn verified_prefix_binds_owner_binding_and_current_cohorts() {
        let original_manifest = Digest256::of_bytes(b"retained event manifest");
        let changed_manifest = Digest256::of_bytes(b"mutated event manifest");
        let original_rows = vec![format!(
            r#"{{"event_sequence":20,"coverage_manifest_sha256":"{}"}}"#,
            original_manifest.to_hex()
        )];
        let changed_rows = vec![format!(
            r#"{{"event_sequence":20,"coverage_manifest_sha256":"{}"}}"#,
            changed_manifest.to_hex()
        )];
        let cohort_root = navigation_current_cohort_root_from_row_json(&original_rows, 4096)
            .expect("current cohort commitment");
        let changed_cohort_root = navigation_current_cohort_root_from_row_json(&changed_rows, 4096)
            .expect("mutated current cohort commitment");
        let reordered_rows = vec![format!(
            r#"{{"coverage_manifest_sha256":"{}","event_sequence":20}}"#,
            original_manifest.to_hex()
        )];
        let reordered_cohort_root =
            navigation_current_cohort_root_from_row_json(&reordered_rows, 4096)
                .expect("reordered current cohort commitment");
        let baseline = checkpoint_digest(r#"{"owner":"alpha"}"#, cohort_root);

        assert_eq!(cohort_root, reordered_cohort_root);
        assert_ne!(
            baseline,
            checkpoint_digest(r#"{"owner":"beta"}"#, cohort_root)
        );
        assert_ne!(
            baseline,
            checkpoint_digest(r#"{"owner":"alpha"}"#, changed_cohort_root)
        );

        let range_root = durable_navigation_range::navigation_range_coverage_root_next(
            test_digest(b"previous coverage"),
            10,
            20,
            20,
            "range-terminal",
            test_digest(b"range manifest"),
            original_manifest,
            test_digest(b"source revision"),
            test_digest(b"dependency root"),
            test_digest(b"output root"),
        );
        assert_ne!(
            range_root,
            durable_navigation_range::navigation_range_coverage_root_next(
                test_digest(b"previous coverage"),
                10,
                20,
                20,
                "range-terminal",
                test_digest(b"range manifest"),
                changed_manifest,
                test_digest(b"source revision"),
                test_digest(b"dependency root"),
                test_digest(b"output root"),
            )
        );
    }
}
