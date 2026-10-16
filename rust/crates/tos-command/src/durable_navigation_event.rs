//! PostgreSQL persistence and retained-history verification for navigation.
//!
//! Current publication uses complete event ranges through durable_navigation_range.
//! Historical event-cohort readers retain their exact integrity checks.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use postgres::{GenericClient, IsolationLevel};
use serde_json::{Value, json};
use tos_compiler::VerifiedKnowledgeModel;

use tos_compiler::NavigationOriginalReceipt;
use tos_compiler::source_navigation_event::{
    NavigationDependencyRoleV1, NavigationDependencyStateV1, NavigationDependencyV1,
    NavigationRecordDependencyRefreshCauseV1,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath,
    canonical_bytes_v1, parse_json,
};
use tos_segment_store::SegmentStore;

use crate::source_managed_selection::{ManagedAgentSelectedParent, ManagedSelectedProof};

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
    read_domain_snapshot, set_local_timeouts, validate_identifier, validate_relative_path,
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
use crate::source_navigation_bootstrap::MAX_NAVIGATION_EDGE_ID_BYTES;
const MAX_NAVIGATION_EVENT_STEPS_PER_CALL: usize = 256;
pub(super) const VERIFIED_PREFIX_FORMAT_VERSION: i16 = 3;

#[path = "durable_navigation_event/cohort_tree.rs"]
mod cohort_tree;

#[path = "durable_navigation_event/bootstrap_capture.rs"]
mod bootstrap_capture;

/// Independent resource ceilings for one event-local cohort or selected cut.
#[derive(Clone, Copy, Debug)]
pub struct DurableNavigationEventLimits {
    /// Dependencies in one owner cohort or exact source-path selection.
    /// A batch can contain many such owners; all old/new dependency rows are
    /// charged to its aggregate max_current_rows and max_total_bytes ceilings.
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
struct DurableNavigationClaimRefreshCauseBindingV1 {
    changed_dependency_path: String,
    changed_dependency_sha256: Digest256,
    reverse_claim_ids: Vec<String>,
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

/// The carrier and dependency rules shared by one-shot and paged initial
/// construction. Storage adapters choose their own bounded transaction shape.
fn prepare_navigation_bootstrap_row(
    row: DurableNavigationBootstrapRowV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<(DurableNavigationBaseRowReadV1, BTreeSet<String>)> {
    validate_identifier(
        &row.id,
        if row.kind == DurableNavigationBootstrapRowKindV1::Edge {
            MAX_NAVIGATION_EDGE_ID_BYTES
        } else {
            MAX_NAVIGATION_RECORD_ID_BYTES
        },
        "navigation bootstrap row ID",
    )?;
    validate_relative_path(&row.owner_source_path)?;
    validate_identifier(
        &row.owner_record_id,
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "navigation bootstrap owner record ID",
    )?;
    if row.raw.len() > limits.max_row_bytes {
        return Err(DurableError::Refused("navigation bootstrap row size"));
    }
    if matches!(
        row.kind,
        DurableNavigationBootstrapRowKindV1::Node | DurableNavigationBootstrapRowKindV1::Edge
    ) != row.visible.is_some()
    {
        return Err(DurableError::Invalid(
            "navigation bootstrap visibility witness",
        ));
    }
    let value = strict_json_value(&row.raw, limits.max_row_bytes)?;
    if let Some(field) = row.kind.id_field()
        && value.get(field).and_then(Value::as_str) != Some(row.id.as_str())
    {
        return Err(DurableError::Invalid(
            "navigation bootstrap carrier ID differs",
        ));
    }
    let (mut refs, mut dependencies) =
        bootstrap_ref_parts(&value, "navigation bootstrap source ref")?;
    refs.insert(row.owner_source_path.clone());
    dependencies.insert(row.owner_source_path.clone());
    if dependencies.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation bootstrap carrier dependencies",
        ));
    }
    let from_id = value
        .get("from_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let to_id = value
        .get("to_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if row.kind == DurableNavigationBootstrapRowKindV1::Edge {
        for endpoint in [&from_id, &to_id] {
            validate_identifier(
                endpoint.as_deref().ok_or(DurableError::Corrupt(
                    "navigation bootstrap edge endpoint absent",
                ))?,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation bootstrap edge endpoint",
            )?;
        }
    }
    Ok((
        DurableNavigationBaseRowReadV1 {
            kind: row.kind,
            id: row.id,
            owner_source_path: row.owner_source_path,
            owner_record_id: row.owner_record_id,
            active: true,
            visible: row.visible,
            sha256: Digest256::of_bytes(&row.raw),
            from_id,
            to_id,
            source_refs: refs.into_iter().collect(),
            raw: row.raw,
        },
        dependencies,
    ))
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
    let mut hash = NavigationBootstrapRootHasher::new(binding, header_raw, max_bytes)?;
    for source in owner_sources.values() {
        hash.source(source)?;
    }
    for row in rows {
        hash.row(row)?;
    }
    Ok(hash.finish())
}

/// V1 framing over a source-ordered then row-ordered stream. Retain only the
/// previous keys, so an explicit initial seal need not materialize all bytes.
struct NavigationBootstrapRootHasher {
    hash: Digest256Hasher,
    last_source: Option<String>,
    last_row: Option<(DurableNavigationBootstrapRowKindV1, String, String, String)>,
}

impl NavigationBootstrapRootHasher {
    fn new(
        binding: &DurableNavigationCurrentIndexOwnerBindingV1,
        header_raw: &[u8],
        max_bytes: usize,
    ) -> DurableResult<Self> {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-source-navigation-bootstrap-v1");
        let binding = canonical_value(&navigation_owner_binding_json(binding), max_bytes)?;
        navigation_hash_part(&mut hash, &binding);
        navigation_hash_part(&mut hash, header_raw);
        Ok(Self {
            hash,
            last_source: None,
            last_row: None,
        })
    }

    fn source(&mut self, source: &DurableNavigationBootstrapOwnerSourceV1) -> DurableResult<()> {
        if self.last_row.is_some()
            || self
                .last_source
                .as_ref()
                .is_some_and(|path| *path >= source.source_path)
        {
            return Err(DurableError::Corrupt(
                "navigation bootstrap seal source order",
            ));
        }
        navigation_hash_part(&mut self.hash, source.source_path.as_bytes());
        navigation_hash_part(&mut self.hash, source.sha256.to_hex().as_bytes());
        navigation_hash_part(&mut self.hash, &source.size_bytes.to_be_bytes());
        self.last_source = Some(source.source_path.clone());
        Ok(())
    }

    fn row(&mut self, row: &DurableNavigationBaseRowReadV1) -> DurableResult<()> {
        let key = (
            row.kind,
            row.id.clone(),
            row.owner_source_path.clone(),
            row.owner_record_id.clone(),
        );
        if self
            .last_row
            .as_ref()
            .is_some_and(|previous| *previous >= key)
        {
            return Err(DurableError::Corrupt("navigation bootstrap seal row order"));
        }
        navigation_hash_part(&mut self.hash, row.kind.as_str().as_bytes());
        navigation_hash_part(&mut self.hash, row.id.as_bytes());
        navigation_hash_part(&mut self.hash, row.owner_source_path.as_bytes());
        navigation_hash_part(&mut self.hash, row.owner_record_id.as_bytes());
        navigation_hash_part(
            &mut self.hash,
            &[match row.visible {
                Some(false) => 0,
                Some(true) => 1,
                None => 2,
            }],
        );
        navigation_hash_part(&mut self.hash, row.sha256.to_hex().as_bytes());
        navigation_hash_part(&mut self.hash, &row.raw);
        self.last_row = Some(key);
        Ok(())
    }

    fn finish(self) -> Digest256 {
        self.hash.finalize()
    }
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
    hash.update(b"tos-source-navigation-current-index-v3");
    navigation_hash_part(&mut hash, bootstrap_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, coverage_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, adjacency_root.to_hex().as_bytes());
    navigation_hash_part(&mut hash, cohort_root.to_hex().as_bytes());
    hash.finalize()
}

/// Read the authenticated current-map root. Full table coverage is an explicit
/// audit; ordinary delivery proves the metadata of its selected owners.
pub(super) fn navigation_current_cohort_root_in_tx(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Digest256> {
    cohort_tree::current_root(tx, domain, consumer_id, limits)
}

pub(crate) fn verify_navigation_selected_cohort_metadata_v3(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    owners: &BTreeSet<(String, String)>,
    limits: DurableNavigationEventLimits,
    max_probes: u64,
    max_rows: u64,
) -> DurableResult<(u64, u64, u64)> {
    let checkpoint = cohort_tree::verify_owners_with_work(
        tx,
        domain,
        consumer_id,
        owners,
        limits,
        max_probes,
        max_rows,
    )?;
    Ok(checkpoint.work())
}

#[cfg(test)]
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
    hash.update(b"tos-source-navigation-verified-prefix-v3");
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
    source_event_id: Option<String>,
    source_event_version: Option<u64>,
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
            &owner.source_event_version.map(as_i64).transpose()?,
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
    // Expose the selected values as restrictions on each indexed relation.
    // A join to a materialized unnest CTE under LIMIT otherwise lets PostgreSQL
    // estimate that every owner matches and scan an unrelated bootstrap cohort.
    // The join still preserves exact path/record pairs; these filters change
    // planning selectivity, not membership or the aggregate budget.
    let selected = tx.query_one(
        "WITH target AS (SELECT * FROM unnest($3::text[],$4::text[]) AS t(source_path,record_id)),
         carriers_raw AS (
           SELECT octet_length(b.row_json)::bigint AS bytes
             FROM target t JOIN cmd2_projection_navigation_bootstrap_row b
               ON b.owner_source_path=t.source_path AND b.owner_record_id=t.record_id
            WHERE b.domain=$1 AND b.consumer_id=$2 AND b.owner_source_path=ANY($3) AND b.owner_record_id=ANY($4) AND b.active
           UNION ALL
           SELECT octet_length(n.row_json)::bigint FROM target t JOIN cmd2_projection_navigation_node n
             ON n.source_path=t.source_path AND n.record_id=t.record_id
            WHERE n.domain=$1 AND n.consumer_id=$2 AND n.source_path=ANY($3) AND n.record_id=ANY($4)
           UNION ALL
           SELECT octet_length(e.row_json)::bigint FROM target t JOIN cmd2_projection_navigation_edge e
             ON e.source_path=t.source_path AND e.record_id=t.record_id
            WHERE e.domain=$1 AND e.consumer_id=$2 AND e.source_path=ANY($3) AND e.record_id=ANY($4)
           UNION ALL
           SELECT octet_length(d.row_json)::bigint FROM target t JOIN cmd2_projection_navigation_diagnostic d
             ON d.source_path=t.source_path AND d.record_id=t.record_id
            WHERE d.domain=$1 AND d.consumer_id=$2 AND d.source_path=ANY($3) AND d.record_id=ANY($4)
         ), carriers AS (SELECT * FROM carriers_raw LIMIT $5), dependencies_raw AS (
           SELECT (octet_length(d.dependency_path)+octet_length(d.dependency_role)
                  +COALESCE(octet_length(d.sha256),0)+COALESCE(octet_length(d.absent_reason),0)+64)::bigint AS bytes
             FROM target t JOIN cmd2_projection_navigation_dependency d
               ON d.source_path=t.source_path AND d.record_id=t.record_id
            WHERE d.domain=$1 AND d.consumer_id=$2 AND d.source_path=ANY($3) AND d.record_id=ANY($4)
         ), dependencies AS (SELECT * FROM dependencies_raw LIMIT $6)
         SELECT (SELECT count(*) FROM carriers) AS carrier_count,
                  (SELECT COALESCE(sum(bytes),0)::bigint FROM carriers) AS carrier_bytes,
                  (SELECT count(*) FROM dependencies) AS dependency_count,
                  (SELECT COALESCE(sum(bytes),0)::bigint FROM dependencies) AS dependency_bytes",
        &[&domain,&consumer_id,&target_paths,&target_ids,
          &i64::try_from(limits.max_current_rows.saturating_add(1)).map_err(|_| DurableError::Refused("navigation batch row cap"))?,
          &i64::try_from(limits.max_current_rows.saturating_add(1)).map_err(|_| DurableError::Refused("navigation batch dependency row cap"))?],
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
    let selected_rows = as_u64(selected.get("carrier_count"))?
        .checked_add(as_u64(selected.get("dependency_count"))?)
        .and_then(|n| n.checked_add(new_carrier_count))
        .and_then(|n| n.checked_add(new_dependency_count))
        .ok_or(DurableError::Refused("navigation batch selected row count"))?;
    let selected_bytes = header_bytes
        .checked_add(as_u64(selected.get("carrier_bytes"))?)
        .and_then(|n| n.checked_add(as_u64(selected.get("dependency_bytes")).ok()?))
        .and_then(|n| n.checked_add(new_carrier_bytes))
        .and_then(|n| n.checked_add(new_dependency_bytes))
        .ok_or(DurableError::Refused(
            "navigation batch selected byte count",
        ))?;
    if selected_rows > limits.max_current_rows as u64
        || replacements
            .iter()
            .any(|rows| rows.dependencies.len() > limits.max_dependencies)
        || target_owners.len() > limits.max_current_cohorts
        || replacements.len() > limits.max_current_cohorts
        || selected_bytes > limits.max_total_bytes
    {
        return Err(DurableError::Source(
            crate::source_command::SourceCommandError::DeniedWithReason(format!(
                "navigation batch selected mutation budget: rows={selected_rows}/{} bytes={selected_bytes}/{} old_dependency_rows={} new_dependency_rows={new_dependency_count} max_owner_dependencies={}/{} target_owners={}/{} replacements={}/{}",
                limits.max_current_rows,
                limits.max_total_bytes,
                as_u64(selected.get("dependency_count"))?,
                replacements
                    .iter()
                    .map(|rows| rows.dependencies.len())
                    .max()
                    .unwrap_or(0),
                limits.max_dependencies,
                target_owners.len(),
                limits.max_current_cohorts,
                replacements.len(),
                limits.max_current_cohorts,
            )),
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

/// Compare already authenticated projection carriers across their two framing
/// profiles: Original packets have no LF; durable event rows have one LF.
/// This does not normalize source bytes, JSON values, ordering or whitespace.
/// Callers must verify each carrier against its own digest before comparing.
pub(crate) fn same_navigation_carrier_payload(left: &[u8], right: &[u8]) -> bool {
    fn payload(raw: &[u8]) -> &[u8] {
        raw.strip_suffix(b"\n")
            .filter(|body| !body.ends_with(b"\n"))
            .unwrap_or(raw)
    }
    payload(left) == payload(right)
}

fn same_effective_node(left: &NavigationEffectiveNode, right: &NavigationEffectiveNode) -> bool {
    left.visible == right.visible && same_navigation_carrier_payload(&left.raw, &right.raw)
}

fn same_effective_edge(left: &NavigationEffectiveEdge, right: &NavigationEffectiveEdge) -> bool {
    left.from_id == right.from_id
        && left.to_id == right.to_id
        && left.visible == right.visible
        && same_navigation_carrier_payload(&left.raw, &right.raw)
}

#[cfg(test)]
mod carrier_framing_tests {
    use super::*;

    #[test]
    fn original_and_event_frames_share_payload_but_preserve_real_conflicts() {
        let value = json!({"node_id":"identity:кружок","label":"Венский кружок"});
        let event = canonical_value(&value, 4096).unwrap();
        let original = event.strip_suffix(b"\n").unwrap().to_vec();
        let node = |raw: Vec<u8>| NavigationEffectiveNode {
            node_id: "identity:кружок".into(),
            visible: true,
            sha256: Digest256::of_bytes(&raw),
            raw,
        };
        let old = node(original.clone());
        let new = node(event.clone());
        assert_ne!(old.sha256, new.sha256);
        assert!(same_effective_node(&old, &new));
        assert!(same_effective_node(&new, &old));
        let mut hidden = node(event.clone());
        hidden.visible = false;
        assert!(!same_effective_node(&old, &hidden));
        for raw in [
            canonical_value(
                &json!({"node_id":"identity:кружок","label":"changed"}),
                4096,
            )
            .unwrap(),
            [original.as_slice(), b" "].concat(),
            [original.as_slice(), b"\r\n"].concat(),
            [original.as_slice(), b"\n\n"].concat(),
        ] {
            assert!(!same_effective_node(&old, &node(raw)));
        }
        let edge_value = json!({"edge_id":"e","from_id":"a","to_id":"b"});
        let edge_event = canonical_value(&edge_value, 4096).unwrap();
        let edge = |raw: Vec<u8>| NavigationEffectiveEdge {
            edge_id: "e".into(),
            from_id: "a".into(),
            to_id: "b".into(),
            visible: true,
            sha256: Digest256::of_bytes(&raw),
            raw,
        };
        let old_edge = edge(edge_event.strip_suffix(b"\n").unwrap().to_vec());
        let mut new_edge = edge(edge_event);
        assert!(same_effective_edge(&old_edge, &new_edge));
        new_edge.to_id = "c".into();
        assert!(!same_effective_edge(&old_edge, &new_edge));
    }
}

/// Select only changed owners, their shared carriers, incident edges and the
/// complete outgoing neighborhoods whose certificates can change. Unrelated
/// graph rows and certificates are never inputs to an ordinary batch.
fn load_navigation_batch_neighborhood<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    target_owners: &BTreeSet<(String, String)>,
    replacements: &[&PersistedRows],
    limits: DurableNavigationEventLimits,
) -> DurableResult<(Vec<postgres::Row>, Vec<postgres::Row>, BTreeSet<String>)> {
    let (paths, records): (Vec<_>, Vec<_>) = target_owners.iter().cloned().unzip();
    let row_cap = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation neighborhood row cap"))?;
    let edge_cap = i64::try_from(limits.max_adjacency_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation neighborhood edge cap"))?;
    let row_bytes = as_i64(limits.max_row_bytes as u64)?;
    let total_bytes = as_i64(limits.max_total_bytes)?;
    let seeds = tx.query(
        "WITH target AS (SELECT * FROM unnest($3::text[],$4::text[]) AS t(path,id))
         SELECT row_kind,row_id,from_id FROM (
           SELECT b.row_kind,b.row_id,b.from_id FROM target t
             JOIN cmd2_projection_navigation_bootstrap_row b
               ON b.owner_source_path=t.path AND b.owner_record_id=t.id
            WHERE b.domain=$1 AND b.consumer_id=$2 AND b.owner_source_path=ANY($3) AND b.owner_record_id=ANY($4) AND b.active AND b.row_kind IN ('node','edge')
           UNION ALL
           SELECT 'node',n.node_id,NULL::text FROM target t
             JOIN cmd2_projection_navigation_node n ON n.source_path=t.path AND n.record_id=t.id
            WHERE n.domain=$1 AND n.consumer_id=$2 AND n.source_path=ANY($3) AND n.record_id=ANY($4)
           UNION ALL
           SELECT 'edge',e.edge_id,e.from_id FROM target t
             JOIN cmd2_projection_navigation_edge e ON e.source_path=t.path AND e.record_id=t.id
            WHERE e.domain=$1 AND e.consumer_id=$2 AND e.source_path=ANY($3) AND e.record_id=ANY($4)
         ) selected LIMIT $5",
        &[&domain, &consumer_id, &paths, &records, &row_cap],
    )?;
    if seeds.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation affected owner row budget",
        ));
    }
    let mut node_ids = BTreeSet::new();
    let mut edge_ids = BTreeSet::new();
    let mut affected = BTreeSet::new();
    for row in seeds {
        let id: String = row.get("row_id");
        if row.get::<_, String>("row_kind") == "node" {
            node_ids.insert(id.clone());
            affected.insert(id);
        } else {
            edge_ids.insert(id);
            affected.insert(row.get::<_, Option<String>>("from_id").ok_or(
                DurableError::Corrupt("navigation affected edge source absent"),
            )?);
        }
    }
    for rows in replacements {
        for node in &rows.nodes {
            node_ids.insert(node.node_id.clone());
            affected.insert(node.node_id.clone());
        }
        for edge in &rows.edges {
            edge_ids.insert(edge.edge_id.clone());
            affected.insert(edge.from_id.clone());
        }
    }
    if node_ids.len() > limits.max_current_rows || edge_ids.len() > limits.max_adjacency_rows {
        return Err(DurableError::Refused("navigation affected identity budget"));
    }
    // Incoming edges observe node disappearance/visibility changes. Existing
    // contributions to a replacement edge must also agree on its endpoints.
    let node_ids: Vec<_> = node_ids.into_iter().collect();
    let edge_ids: Vec<_> = edge_ids.into_iter().collect();
    let incoming = tx.query(
        "SELECT from_id FROM (
           SELECT from_id FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND active AND row_kind='edge'
              AND (to_id COLLATE \"C\"=ANY($3) OR row_id COLLATE \"C\"=ANY($4))
           UNION ALL
           SELECT from_id FROM cmd2_projection_navigation_edge
            WHERE domain=$1 AND consumer_id=$2
              AND (to_id COLLATE \"C\"=ANY($3) OR edge_id COLLATE \"C\"=ANY($4))
         ) incident LIMIT $5",
        &[&domain, &consumer_id, &node_ids, &edge_ids, &edge_cap],
    )?;
    if incoming.len() > limits.max_adjacency_rows {
        return Err(DurableError::Refused("navigation incident edge budget"));
    }
    for row in incoming {
        affected.insert(
            row.get::<_, Option<String>>(0)
                .ok_or(DurableError::Corrupt(
                    "navigation incident edge source absent",
                ))?,
        );
    }
    if affected.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation affected certificate budget",
        ));
    }
    let affected_ids: Vec<_> = affected.iter().cloned().collect();
    // Length gates apply before PostgreSQL returns carrier text. The window
    // covers at most edge_cap candidates and refuses overflow via NULL, so a
    // row-count cap cannot materialize max_rows * max_row_bytes in the client.
    let edges = tx.query(
        "WITH selected AS (
           SELECT * FROM (
             SELECT row_id AS edge_id,from_id,to_id,visible,row_sha256,row_json,
                    owner_source_path,owner_record_id
               FROM cmd2_projection_navigation_bootstrap_row
              WHERE domain=$1 AND consumer_id=$2 AND active AND row_kind='edge'
                AND from_id COLLATE \"C\"=ANY($3)
             UNION ALL
             SELECT edge_id,from_id,to_id,visible,row_sha256,row_json,
                    source_path AS owner_source_path,record_id AS owner_record_id
               FROM cmd2_projection_navigation_edge
              WHERE domain=$1 AND consumer_id=$2 AND from_id COLLATE \"C\"=ANY($3)
           ) outgoing LIMIT $4
         ), charged AS (
           SELECT *,sum(octet_length(row_json)::bigint) OVER (
             ORDER BY edge_id COLLATE \"C\",owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"
             ROWS UNBOUNDED PRECEDING) AS charged_bytes FROM selected
         ) SELECT edge_id,from_id,to_id,visible,row_sha256,owner_source_path,owner_record_id,
                  CASE WHEN octet_length(row_json)<=$5::bigint AND charged_bytes<=$6::bigint THEN row_json END AS row_json
             FROM charged",
        &[&domain, &consumer_id, &affected_ids, &edge_cap, &row_bytes, &total_bytes],
    )?;
    if edges.len() > limits.max_adjacency_rows {
        return Err(DurableError::Refused(
            "navigation outgoing neighborhood row budget",
        ));
    }
    let mut endpoints = affected.clone();
    for row in &edges {
        if row.get::<_, Option<&str>>("row_json").is_none() {
            return Err(DurableError::Refused(
                "navigation outgoing neighborhood byte budget",
            ));
        }
        for field in ["from_id", "to_id"] {
            endpoints.insert(
                row.get::<_, Option<String>>(field)
                    .ok_or(DurableError::Corrupt(
                        "navigation neighborhood endpoint absent",
                    ))?,
            );
        }
    }
    for rows in replacements {
        for edge in &rows.edges {
            endpoints.insert(edge.from_id.clone());
            endpoints.insert(edge.to_id.clone());
        }
    }
    if endpoints.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation neighborhood endpoint budget",
        ));
    }
    let endpoints: Vec<_> = endpoints.into_iter().collect();
    let nodes = tx.query(
        "WITH selected AS (
           SELECT * FROM (
             SELECT row_id AS node_id,visible,row_sha256,row_json,owner_source_path,owner_record_id
               FROM cmd2_projection_navigation_bootstrap_row
              WHERE domain=$1 AND consumer_id=$2 AND active AND row_kind='node'
                AND row_id COLLATE \"C\"=ANY($3)
             UNION ALL
             SELECT node_id,visible,row_sha256,row_json,
                    source_path AS owner_source_path,record_id AS owner_record_id
               FROM cmd2_projection_navigation_node
              WHERE domain=$1 AND consumer_id=$2 AND node_id COLLATE \"C\"=ANY($3)
           ) endpoint_nodes LIMIT $4
         ), charged AS (
           SELECT *,sum(octet_length(row_json)::bigint) OVER (
             ORDER BY node_id COLLATE \"C\",owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"
             ROWS UNBOUNDED PRECEDING) AS charged_bytes FROM selected
         ) SELECT node_id,visible,row_sha256,owner_source_path,owner_record_id,
                  CASE WHEN octet_length(row_json)<=$5::bigint AND charged_bytes<=$6::bigint THEN row_json END AS row_json
             FROM charged",
        &[&domain, &consumer_id, &endpoints, &row_cap, &row_bytes, &total_bytes],
    )?;
    if nodes.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation neighborhood node row budget",
        ));
    }
    if nodes
        .iter()
        .any(|row| row.get::<_, Option<&str>>("row_json").is_none())
    {
        return Err(DurableError::Refused(
            "navigation neighborhood node byte budget",
        ));
    }
    Ok((nodes, edges, affected))
}

#[cfg(all(test, feature = "postgres-lab"))]
#[path = "durable_navigation_event/bounded_batch_pg_tests.rs"]
mod bounded_batch_pg_tests;

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
    let (event_nodes, event_edges, affected) = load_navigation_batch_neighborhood(
        tx,
        domain,
        consumer_id,
        target_owners,
        replacements,
        limits,
    )?;
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
            // Choose the same physical representative as the query adapter.
            // Its exact digest remains the adjacency certificate input.
            if node.sha256.as_bytes() < existing.sha256.as_bytes() {
                nodes.insert(node.node_id.clone(), node);
            }
            return Ok(());
        }
        nodes.insert(node.node_id.clone(), node);
        Ok(())
    };
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
            if edge.sha256.as_bytes() < existing.sha256.as_bytes() {
                edges.insert(edge.edge_id.clone(), edge);
            }
            return Ok(());
        }
        edges.insert(edge.edge_id.clone(), edge);
        Ok(())
    };
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
        if node.visible && affected.contains(node_id) {
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

impl DurablePgCoordinator {
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
                    // Exact archived public writer locks are present empty members.
                    // Their bytes still pass custody and digest verification below;
                    // source-specific parsers own nonempty document requirements.
                    if version.content_length > source_byte_limit {
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
        if let NavigationDependencyStateV1::Present { sha256, size_bytes } = &dependency.state {
            let source_byte_limit = if dependency.role == NavigationDependencyRoleV1::Event {
                limits.max_total_bytes
            } else {
                limits.max_row_bytes as u64
            };
            if *size_bytes == 0 && *sha256 != Digest256::of_bytes(&[]) {
                return Err(DurableError::Conflict(
                    "navigation empty dependency digest differs",
                ));
            }
            if *size_bytes > source_byte_limit {
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
            MAX_NAVIGATION_EDGE_ID_BYTES,
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
