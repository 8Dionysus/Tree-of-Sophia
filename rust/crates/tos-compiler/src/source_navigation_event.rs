//! Bounded replacement cohorts for event-local source-navigation updates.
//!
//! This is a compiler result, not a source receipt or publication. The source
//! owner supplies exact event-cut inputs and dependency witnesses; the PG
//! publisher must verify those witnesses against its selected event, check the
//! predecessor root, replace the entire cohort and advance its cursor in one
//! transaction. A cohort does not establish global graph completeness.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, RelativePath, canonical_bytes_v1};

use crate::{
    Error, Result,
    source_bibliographic::{
        BibliographicDocumentLimits, BibliographicForms, BibliographicLimits,
        SuppliedNavigationRecordInput, SuppliedNavigationRecordProjection,
        materialize_checked_form_set, render_supplied_navigation_record,
        supplied_metadata_descriptor,
    },
    source_bibliographic_render::digest as source_record_digest,
    source_witness_catalog::render_catalog_record,
};

const EVENT_ID_BYTES: usize = 256;
const DEPENDENCY_PATH_BYTES: usize = 4096;
const TOTAL_EVENT_BYTES: usize = 128 * 1024 * 1024;
const MAX_EVENT_RANGE_EVENTS: usize = 256;
const MAX_EVENT_RANGE_EVENT_MEMBERS: usize = 64;
const MAX_EVENT_RANGE_TOTAL_MEMBERS: usize = 65_536;
const MAX_EVENT_RANGE_COVERAGE_BYTES: usize = 64 * 1024 * 1024;

/// One exact source member carried by a committed event in a contiguous
/// navigation range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationEventRangeMemberV1 {
    pub source_path: String,
    pub revision: u64,
    pub is_source: bool,
    pub size_bytes: u64,
    pub sha256: Digest256,
}

/// Exact durable event facts. `command_id` is populated only for command
/// events; event identity and command identity remain separate fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationEventRangeEventV1 {
    pub sequence: u64,
    pub event_id: String,
    pub command_id: Option<String>,
    pub event_kind: String,
    pub member_root: Digest256,
    pub members: Vec<NavigationEventRangeMemberV1>,
}

/// Untrusted range facts copied from the durable event reader. The sealer
/// recomputes canonical coverage bytes and root before producing the private
/// compiler-side capability.
pub struct NavigationEventRangeCoverageInputV1<'a> {
    pub cursor_before: u64,
    pub through_sequence: u64,
    pub terminal_event_id: &'a str,
    pub events: &'a [NavigationEventRangeEventV1],
    pub coverage_root: Digest256,
    pub manifest_raw: &'a [u8],
}

/// Sealed exact `cursor_before + 1 ..= through_sequence` source-event range.
/// It never combines several events into a synthetic event identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationEventRangeCoverageV1 {
    cursor_before: u64,
    through_sequence: u64,
    terminal_event_id: String,
    events: Vec<NavigationEventRangeEventV1>,
    coverage_root: Digest256,
    manifest_raw: Vec<u8>,
}

impl NavigationEventRangeCoverageV1 {
    pub fn cursor_before(&self) -> u64 {
        self.cursor_before
    }

    pub fn through_sequence(&self) -> u64 {
        self.through_sequence
    }

    pub fn terminal_event_id(&self) -> &str {
        &self.terminal_event_id
    }

    pub fn events(&self) -> &[NavigationEventRangeEventV1] {
        &self.events
    }

    pub fn coverage_root(&self) -> Digest256 {
        self.coverage_root
    }

    pub fn manifest_raw(&self) -> &[u8] {
        &self.manifest_raw
    }

    pub fn event(&self, sequence: u64) -> Option<&NavigationEventRangeEventV1> {
        self.events.iter().find(|event| event.sequence == sequence)
    }

    pub fn member(
        &self,
        sequence: u64,
        source_path: &str,
    ) -> Option<&NavigationEventRangeMemberV1> {
        self.event(sequence)?
            .members
            .iter()
            .find(|member| member.source_path == source_path)
    }

    pub fn verify_integrity(&self) -> Result<()> {
        let (manifest_raw, coverage_root) = range_coverage_manifest_v1(
            self.cursor_before,
            self.through_sequence,
            &self.terminal_event_id,
            &self.events,
        )?;
        if manifest_raw != self.manifest_raw || coverage_root != self.coverage_root {
            return Err(Error::Invalid("navigation event range coverage root"));
        }
        Ok(())
    }
}

/// Re-seal a durable range only when its canonical facts and exact root agree.
pub fn seal_navigation_event_range_coverage_v1(
    input: NavigationEventRangeCoverageInputV1<'_>,
) -> Result<NavigationEventRangeCoverageV1> {
    let (manifest_raw, coverage_root) = range_coverage_manifest_v1(
        input.cursor_before,
        input.through_sequence,
        input.terminal_event_id,
        input.events,
    )?;
    if manifest_raw != input.manifest_raw || coverage_root != input.coverage_root {
        return Err(Error::Invalid("navigation event range coverage binding"));
    }
    let sealed = NavigationEventRangeCoverageV1 {
        cursor_before: input.cursor_before,
        through_sequence: input.through_sequence,
        terminal_event_id: input.terminal_event_id.to_owned(),
        events: input.events.to_vec(),
        coverage_root,
        manifest_raw,
    };
    sealed.verify_integrity()?;
    Ok(sealed)
}

fn range_coverage_manifest_v1(
    cursor_before: u64,
    through_sequence: u64,
    terminal_event_id: &str,
    events: &[NavigationEventRangeEventV1],
) -> Result<(Vec<u8>, Digest256)> {
    if cursor_before >= through_sequence
        || events.is_empty()
        || events.len() > MAX_EVENT_RANGE_EVENTS
        || events.len() as u64 != through_sequence - cursor_before
        || events.first().map(|event| event.sequence) != Some(cursor_before + 1)
        || events.last().map(|event| event.sequence) != Some(through_sequence)
        || events.last().map(|event| event.event_id.as_str()) != Some(terminal_event_id)
        || terminal_event_id.is_empty()
        || terminal_event_id.len() > EVENT_ID_BYTES
        || terminal_event_id
            .bytes()
            .any(|byte| byte.is_ascii_control())
    {
        return Err(Error::Invalid(
            "navigation event range is not complete and contiguous",
        ));
    }
    let mut previous_sequence = cursor_before;
    let mut total_members = 0usize;
    for event in events {
        if event.event_id.is_empty()
            || event.event_id.len() > EVENT_ID_BYTES
            || event.event_id.bytes().any(|byte| byte.is_ascii_control())
            || event.sequence != previous_sequence + 1
            || !matches!(event.event_kind.as_str(), "command" | "rights" | "rule")
            || (event.event_kind == "command"
                && event
                    .command_id
                    .as_deref()
                    .is_none_or(|value| value.is_empty() || value.len() > EVENT_ID_BYTES))
            || (event.event_kind != "command" && event.command_id.is_some())
            || (event.event_kind != "command" && !event.members.is_empty())
            || event.members.len() > MAX_EVENT_RANGE_EVENT_MEMBERS
        {
            return Err(Error::Invalid("navigation event range event shape"));
        }
        previous_sequence = event.sequence;
        total_members = total_members
            .checked_add(event.members.len())
            .ok_or(Error::Budget("navigation event range member count"))?;
        let mut previous_path: Option<&str> = None;
        for member in &event.members {
            RelativePath::parse(&member.source_path)
                .map_err(|_| Error::Invalid("navigation event range member path"))?;
            if member.source_path.len() > DEPENDENCY_PATH_BYTES
                || member.revision == 0
                || previous_path.is_some_and(|previous| previous >= member.source_path.as_str())
            {
                return Err(Error::Invalid(
                    "navigation event range member ordering or identity",
                ));
            }
            previous_path = Some(member.source_path.as_str());
        }
    }
    if total_members > MAX_EVENT_RANGE_TOTAL_MEMBERS {
        return Err(Error::Budget("navigation event range member count"));
    }
    let value = json!({
        "schema_version":"tos_source_navigation_event_range_coverage_v1",
        "cursor_before":cursor_before,
        "through_sequence":through_sequence,
        "terminal_event_id":terminal_event_id,
        "events":events.iter().map(|event| json!({
            "sequence":event.sequence,
            "event_id":event.event_id,
            "event_kind":event.event_kind,
            "command_id":event.command_id,
            "member_root":event.member_root.to_hex(),
            "members":event.members.iter().map(|member| json!({
                "source_path":member.source_path,
                "revision":member.revision,
                "is_source":member.is_source,
                "size_bytes":member.size_bytes,
                "sha256":member.sha256.to_hex(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    let manifest_raw = canonical(&value, MAX_EVENT_RANGE_COVERAGE_BYTES)?;
    let coverage_root = Digest256::of_bytes(&manifest_raw);
    Ok((manifest_raw, coverage_root))
}

/// Why one exact source address participates in a record cohort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationDependencyRoleV1 {
    Event,
    EventSchema,
    EventReceipt,
    Record,
    EntityRegistry,
    EntityRegistrySchema,
    RecordSchema,
    HumanForms,
    FormsSchema,
    RecordHistory,
    VersionSource,
    VersionSchema,
    Additional,
}

impl NavigationDependencyRoleV1 {
    fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::EventSchema => "event-schema",
            Self::EventReceipt => "event-receipt",
            Self::Record => "record",
            Self::EntityRegistry => "entity-registry",
            Self::EntityRegistrySchema => "entity-registry-schema",
            Self::RecordSchema => "record-schema",
            Self::HumanForms => "human-forms",
            Self::FormsSchema => "forms-schema",
            Self::RecordHistory => "record-history",
            Self::VersionSource => "version-source",
            Self::VersionSchema => "version-schema",
            Self::Additional => "additional",
        }
    }
}

/// Exact event-cut source observation. An absent row is an explicit negative
/// witness supplied by the owner, not an inference made by this renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NavigationDependencyStateV1 {
    Present { sha256: Digest256, size_bytes: u64 },
    Absent { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationDependencyV1 {
    pub path: RelativePath,
    pub role: NavigationDependencyRoleV1,
    pub state: NavigationDependencyStateV1,
}

/// One exact source-member cause for a compound event-local dependency
/// refresh. The PG publisher separately proves that every path and digest is
/// a current committed event member and that the complete reverse-owner
/// closure was replaced in the same event set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRecordDependencyRefreshCauseV1 {
    pub path: RelativePath,
    pub sha256: Digest256,
}

/// Exact source and schema inputs aligned with one entry in `versions`.
/// Unavailable versions carry `None`; every available version must carry its
/// original raw bytes and the schema selected by its maintained owner route.
pub struct NavigationVersionSourceInputV1<'a> {
    pub source_record_bytes: &'a [u8],
    pub schema_path: &'a str,
    pub schema_bytes: &'a [u8],
}

/// Bounded source-owner envelope for replacing one metadata record's graph
/// contribution after one committed event. Values are exact selected bytes
/// parsed by the caller; `source_sha256` binds the untouched record bytes.
pub struct NavigationRecordEventInputV1<'a, 'validator> {
    pub validator: &'a crate::source_witness_catalog::SourceCatalogValidator<'validator>,
    /// Committed CMD outbox ID from the owner-resolved event context. The
    /// publisher rechecks it against the event sequence; it is distinct from
    /// the authored provenance `event_id`.
    pub event_id: &'a str,
    pub event_sequence: u64,
    /// Source head observed in the same owner read cut as the event and its
    /// selected member bytes. This first event-local producer is current-only:
    /// compiling an older event against a later ToS schema/catalog cut would
    /// misstate the selected source revision.
    pub source_head_sequence: u64,
    pub predecessor_source_revision: Digest256,
    /// Revision pinned by the selected schema worker. The publisher links it
    /// to the event cursor and rejects a stale predecessor chain.
    pub predecessor_cohort_root: Option<Digest256>,
    pub event_path: &'a str,
    /// Exact JSONL source member that contains the authored provenance event.
    /// The selected event row is derived by its receipt reference below.
    pub event_file_bytes: &'a [u8],
    pub event_schema_path: &'a str,
    pub event_schema_bytes: &'a [u8],
    pub event_receipt_path: &'a str,
    pub event_receipt_bytes: &'a [u8],
    pub source_path: &'a str,
    pub source_record_bytes: &'a [u8],
    pub entity_registry_path: &'a str,
    pub entity_registry_bytes: &'a [u8],
    pub entity_registry_schema_path: &'a str,
    pub entity_registry_schema_bytes: &'a [u8],
    pub schema_path: &'a str,
    pub schema_bytes: &'a [u8],
    pub forms_path: &'a str,
    pub forms_bytes: Option<&'a [u8]>,
    pub forms_schema_path: &'a str,
    pub forms_schema_bytes: Option<&'a [u8]>,
    pub forms_materializer: &'a mut dyn BibliographicForms,
    pub history_path: &'a str,
    pub history_bytes: Option<&'a [u8]>,
    pub entry: &'a Value,
    pub source_record: &'a Value,
    pub entities: &'a Value,
    pub history: Option<&'a Value>,
    pub versions: &'a [(Value, Value)],
    pub version_sources: &'a [Option<NavigationVersionSourceInputV1<'a>>],
    pub dependencies: &'a [NavigationDependencyV1],
}

/// Complete replaceable output for one event-owned metadata contribution.
/// The publisher may use `output_root` as the CAS successor only after it has
/// verified the supplied exact inputs, source currentness and predecessor.
pub struct NavigationRecordEventCohortV1 {
    /// CMD event identity; the PG publisher checks this against outbox/log.
    pub event_id: String,
    pub event_sequence: u64,
    /// Authored ToS provenance identity derived from the exact event bytes.
    pub source_event_id: String,
    pub source_event_version: u64,
    pub predecessor_source_revision: Digest256,
    pub source_revision: Digest256,
    pub source_path: RelativePath,
    pub record_id: String,
    pub predecessor_cohort_root: Option<Digest256>,
    pub dependency_root: Digest256,
    pub input_root: Digest256,
    pub output_root: Digest256,
    pub dependencies: Vec<NavigationDependencyV1>,
    pub projection: SuppliedNavigationRecordProjection,
    basis: NavigationRecordEventBasisV1,
    node_visibility: Vec<bool>,
    edge_visibility: Vec<bool>,
    _sealed: NavigationRecordEventCohortSeal,
}

/// Deterministic incremental admission cost for one sealed Record cohort.
/// `encoded_bytes` is canonical output rows plus dependency paths, matching
/// the persisted-row budget. `dependency_bytes` sums exact selected source
/// sizes; `dependency_count` is an input-count work proxy, not observed I/O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NavigationRecordEventBudgetCostV1 {
    pub rows: u64,
    pub encoded_bytes: u64,
    pub dependency_bytes: u64,
    pub dependency_count: u64,
}

/// Why an unchanged or changed record is being reprojected for one committed
/// CMD event. A dependency refresh is separately sealed and never claims that
/// the record itself was written by that event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NavigationRecordEventBasisV1 {
    Mutation,
    /// One terminal owner replacement assembled from a contiguous set of
    /// real source events. The terminal event identity stays real; these
    /// roots bind the full range and owner-specific causal set.
    RangeReconciliation {
        coverage_root: Digest256,
        cause_root: Digest256,
    },
    DependencyRefresh {
        changed_dependency_path: RelativePath,
        changed_dependency_sha256: Digest256,
    },
    /// One Record refresh caused by the complete sorted set of exact changed
    /// source members that are dependencies of this owner. The set-level PG
    /// manifest binds the reverse-owner closure across all affected owners.
    DependencyRefreshSet {
        changed_dependencies: Vec<NavigationRecordDependencyRefreshCauseV1>,
    },
    ClaimRevisionRefresh {
        changed_dependency_path: RelativePath,
        changed_dependency_sha256: Digest256,
        history_path: RelativePath,
        history_sha256: Digest256,
        claim_id: String,
        revision_proof_root: Digest256,
    },
}

/// External crates may inspect a produced cohort but cannot construct or
/// mutate one independently of the checked renderer.
#[derive(Clone, Copy, Debug)]
struct NavigationRecordEventCohortSeal;

impl NavigationRecordEventCohortV1 {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
    pub fn source_event_id(&self) -> &str {
        &self.source_event_id
    }
    pub fn source_event_version(&self) -> u64 {
        self.source_event_version
    }
    pub fn predecessor_source_revision(&self) -> Digest256 {
        self.predecessor_source_revision
    }
    pub fn source_revision(&self) -> Digest256 {
        self.source_revision
    }
    pub fn source_path(&self) -> &RelativePath {
        &self.source_path
    }
    pub fn record_id(&self) -> &str {
        &self.record_id
    }
    pub fn predecessor_cohort_root(&self) -> Option<Digest256> {
        self.predecessor_cohort_root
    }
    pub fn dependency_root(&self) -> Digest256 {
        self.dependency_root
    }
    pub fn input_root(&self) -> Digest256 {
        self.input_root
    }
    pub fn output_root(&self) -> Digest256 {
        self.output_root
    }
    pub fn dependencies(&self) -> &[NavigationDependencyV1] {
        &self.dependencies
    }
    pub fn projection(&self) -> &SuppliedNavigationRecordProjection {
        &self.projection
    }
    pub fn basis(&self) -> &NavigationRecordEventBasisV1 {
        &self.basis
    }
    /// Exact graph-query visibility for each node, aligned with `projection().nodes`.
    pub fn node_visibility(&self) -> &[bool] {
        &self.node_visibility
    }
    /// Exact graph-query visibility for each edge, aligned with `projection().edges`.
    pub fn edge_visibility(&self) -> &[bool] {
        &self.edge_visibility
    }

    /// Recompute and validate the sealed roots before returning the stable
    /// per-cohort cost used for incremental set admission.
    pub fn budget_cost(&self) -> Result<NavigationRecordEventBudgetCostV1> {
        let dependency_root = dependency_root_with_cap(&self.dependencies, TOTAL_EVENT_BYTES)?;
        let output_root = output_root_with_caps(
            &self.projection,
            &self.node_visibility,
            &self.edge_visibility,
            TOTAL_EVENT_BYTES,
            TOTAL_EVENT_BYTES,
        )?;
        let event_path = self
            .dependencies
            .iter()
            .find(|dependency| dependency.role == NavigationDependencyRoleV1::Event)
            .map(|dependency| &dependency.path)
            .ok_or(Error::Invalid(
                "navigation Record budget event dependency absent",
            ))?;
        let source_sha256 = self
            .dependencies
            .iter()
            .find(|dependency| {
                dependency.role == NavigationDependencyRoleV1::Record
                    && dependency.path == self.source_path
            })
            .and_then(|dependency| match &dependency.state {
                NavigationDependencyStateV1::Present { sha256, .. } => Some(*sha256),
                NavigationDependencyStateV1::Absent { .. } => None,
            })
            .ok_or(Error::Invalid(
                "navigation Record budget source dependency absent",
            ))?;
        let input_root = input_root_with_cap(
            &self.event_id,
            self.event_sequence,
            &self.source_event_id,
            self.source_event_version,
            self.predecessor_source_revision,
            self.source_revision,
            self.predecessor_cohort_root,
            event_path,
            &self.source_path,
            source_sha256,
            &self.record_id,
            &self.basis,
            dependency_root,
            output_root,
            TOTAL_EVENT_BYTES,
        )?;
        if dependency_root != self.dependency_root
            || output_root != self.output_root
            || input_root != self.input_root
        {
            return Err(Error::Invalid(
                "navigation Record budget cohort roots differ",
            ));
        }
        navigation_record_event_budget_cost(&self.projection, &self.dependencies, TOTAL_EVENT_BYTES)
    }
}

/// Render one terminal Record contribution for an exact contiguous event
/// range. `input.event_id` and sequence remain the real final event in the
/// coverage; the basis commits the range root and this owner's cause root.
/// `origin_forms_*` are the exact creation-time form bytes retained by the
/// selected Record history, when the creation receipt declared them.
pub fn project_navigation_record_range_terminal_event_from_event_cut_v1(
    input: NavigationRecordEventInputV1<'_, '_>,
    coverage: &NavigationEventRangeCoverageV1,
    cause_root: Digest256,
    origin_forms_bytes: Option<&[u8]>,
    limits: BibliographicLimits,
) -> Result<NavigationRecordEventCohortV1> {
    coverage.verify_integrity()?;
    if input.event_id != coverage.terminal_event_id()
        || input.event_sequence != coverage.through_sequence()
        || input.source_head_sequence != coverage.through_sequence()
        || cause_root == Digest256::of_bytes(&[])
        || origin_forms_bytes.is_some_and(|bytes| bytes.is_empty())
    {
        return Err(Error::Invalid(
            "navigation Record range terminal event identity",
        ));
    }
    project_navigation_record_event_with_basis(
        input,
        limits,
        NavigationRecordEventBasisV1::RangeReconciliation {
            coverage_root: coverage.coverage_root(),
            cause_root,
        },
        origin_forms_bytes,
    )
}

fn check_selected_navigation_document(
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    binding: Digest256,
    path: &str,
    schema_path: &str,
    schema_bytes: &[u8],
    raw: &[u8],
) -> Result<()> {
    validator.check_selected_event_document_exact(binding, path, schema_path, schema_bytes, raw)
}

fn project_navigation_record_event_with_basis(
    input: NavigationRecordEventInputV1<'_, '_>,
    limits: BibliographicLimits,
    basis: NavigationRecordEventBasisV1,
    origin_forms_bytes: Option<&[u8]>,
) -> Result<NavigationRecordEventCohortV1> {
    limits.validate()?;
    let selected_source_binding = input.validator.selected_event_source_revision()?;
    let source_revision = selected_source_binding;
    if input.event_id.is_empty()
        || input.event_id.len() > EVENT_ID_BYTES
        || input.event_id.bytes().any(|byte| byte.is_ascii_control())
        || !current_event_head_matches(input.event_sequence, input.source_head_sequence)
        || input.predecessor_source_revision == source_revision
    {
        return Err(Error::Invalid("navigation event identity or revision"));
    }
    let event_path = checked_path(input.event_path)?;
    let event_schema_path = checked_path(input.event_schema_path)?;
    let event_receipt_path = checked_path(input.event_receipt_path)?;
    let source_path = checked_path(input.source_path)?;
    let entity_registry_path = checked_path(input.entity_registry_path)?;
    let entity_registry_schema_path = checked_path(input.entity_registry_schema_path)?;
    let schema_path = checked_path(input.schema_path)?;
    let forms_path = checked_path(input.forms_path)?;
    let history_path = checked_path(input.history_path)?;
    if entity_registry_schema_path.as_str()
        != "ToS/contracts/semantic-entity-type-registry.schema.json"
    {
        return Err(Error::Invalid(
            "navigation event entity registry schema owner path",
        ));
    }
    if input.event_file_bytes.is_empty()
        || input.source_record_bytes.is_empty()
        || input.entity_registry_bytes.is_empty()
        || input.entity_registry_schema_bytes.is_empty()
        || input.event_schema_bytes.is_empty()
        || input.event_receipt_bytes.is_empty()
        || input.schema_bytes.is_empty()
        || input.forms_bytes.is_some_and(|bytes| bytes.is_empty())
        || input.history_bytes.is_some_and(|bytes| bytes.is_empty())
    {
        return Err(Error::Invalid("navigation event exact source bytes"));
    }
    let source_sha256 = Digest256::of_bytes(input.source_record_bytes);
    let event_record_bytes = select_provenance_event_record(
        input.event_file_bytes,
        event_receipt_path.as_str(),
        limits.catalog.max_row_bytes,
        limits.max_claim_cohort_bytes,
    )?;
    let event_value = strict_value(&event_record_bytes, limits.catalog.max_row_bytes)?;
    let event_id_sequence = input
        .event_id
        .rsplit_once(':')
        .and_then(|(domain, sequence)| sequence.parse::<u64>().ok().map(|n| (domain, n)));
    if event_id_sequence
        .is_none_or(|(domain, sequence)| domain.is_empty() || sequence != input.event_sequence)
        || event_value.get("schema_version").and_then(Value::as_str)
            != Some("tos_provenance_event_v2")
    {
        return Err(Error::Invalid("navigation committed event identity"));
    }
    let source_event_id = event_value
        .get("event_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= EVENT_ID_BYTES)
        .ok_or(Error::Invalid("navigation source event identity"))?
        .to_owned();
    let source_event_version = event_value
        .get("event_version")
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or(Error::Invalid("navigation source event version"))?;
    let event_schema = strict_value(input.event_schema_bytes, limits.catalog.max_row_bytes)?;
    let event_receipt = strict_value(input.event_receipt_bytes, limits.catalog.max_row_bytes)?;
    if event_schema_path.as_str() != "ToS/contracts/provenance-event-v2.schema.json"
        || event_receipt_path.as_str()
            != event_value
                .pointer("/record_binding/manifest_ref")
                .and_then(Value::as_str)
                .ok_or(Error::Invalid("navigation event receipt ref absent"))?
        || event_schema.get("$id").and_then(Value::as_str)
            != Some("https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json")
    {
        return Err(Error::Invalid("navigation event schema or receipt binding"));
    }
    let source_from_bytes = strict_value(input.source_record_bytes, limits.catalog.max_row_bytes)?;
    let entities_from_bytes =
        strict_value(input.entity_registry_bytes, limits.catalog.max_row_bytes)?;
    let _schema_value = strict_value(input.schema_bytes, limits.catalog.max_row_bytes)?;
    let _entity_schema_value = strict_value(
        input.entity_registry_schema_bytes,
        limits.catalog.max_row_bytes,
    )?;
    if !same_canonical(
        &source_from_bytes,
        input.source_record,
        limits.catalog.max_row_bytes,
    )? || !same_canonical(
        &entities_from_bytes,
        input.entities,
        limits.catalog.max_row_bytes,
    )? {
        return Err(Error::Invalid("navigation event parsed source differs"));
    }
    let (origin_record_bytes, origin_record) =
        match (input.versions.first(), input.version_sources.first()) {
            (Some((_, resolution)), Some(Some(version_source)))
                if resolution.get("status").and_then(Value::as_str) == Some("available") =>
            {
                let record = resolution
                    .get("record")
                    .filter(|record| record.is_object())
                    .ok_or(Error::Invalid(
                        "navigation Record range origin version record absent",
                    ))?;
                let raw = version_source.source_record_bytes;
                let parsed = strict_value(raw, limits.catalog.max_row_bytes)?;
                if !same_canonical(&parsed, record, limits.catalog.max_row_bytes)? {
                    return Err(Error::Invalid(
                        "navigation Record range origin version bytes differ",
                    ));
                }
                (raw, record)
            }
            (None, None) if input.history.is_none() && input.history_bytes.is_none() => {
                (input.source_record_bytes, input.source_record)
            }
            _ => {
                return Err(Error::Invalid(
                    "navigation Record range origin version is not retained",
                ));
            }
        };
    validate_event_record_binding(
        &event_value,
        &event_receipt,
        &event_path,
        &source_path,
        origin_record_bytes,
        origin_forms_bytes,
        &forms_path,
        input.event_file_bytes,
        origin_record,
    )?;
    check_selected_navigation_document(
        input.validator,
        selected_source_binding,
        event_path.as_str(),
        event_schema_path.as_str(),
        input.event_schema_bytes,
        &event_record_bytes,
    )?;
    crate::source_bibliographic_versions::validate_supplied_metadata_record(
        input.entry,
        input.source_record,
        input.entities,
        limits.catalog.max_row_bytes,
    )?;
    if let Some(bytes) = input.forms_bytes {
        let _form_set_value = strict_value(bytes, limits.catalog.max_row_bytes)?;
    }
    if input.forms_bytes.is_some() != input.forms_schema_bytes.is_some() {
        return Err(Error::Invalid(
            "navigation event optional source bytes differ",
        ));
    }
    let forms = input
        .forms_bytes
        .map(|raw| {
            materialize_checked_form_set(
                raw,
                input.forms_path,
                input.source_record,
                input.forms_materializer,
                limits,
            )
        })
        .transpose()?
        .flatten();
    validate_history_view(
        input.history,
        input.history_bytes,
        &source_path,
        &history_path,
        input.entry,
        input.source_record,
        input.source_record_bytes,
        input.versions,
        limits,
    )?;

    let descriptor_limits = BibliographicDocumentLimits {
        input_document_bytes: limits.catalog.max_row_bytes,
        output_row_bytes: limits.catalog.max_output_row_bytes,
    };
    let descriptor = supplied_metadata_descriptor(
        input.entry,
        input.source_record,
        input.entities,
        descriptor_limits,
    )?;
    if descriptor.get("record_type") != input.entry.get("record_type")
        || descriptor.get("source_schema_ref").and_then(Value::as_str) != Some(schema_path.as_str())
        || input.entry.get("source_record_ref").and_then(Value::as_str)
            != Some(source_path.as_str())
    {
        return Err(Error::Invalid("navigation event record binding"));
    }
    if input.entry.get("record_type").and_then(Value::as_str) == Some("sign") {
        // The Sign promotion-basis Claim is resolved by the cut-bound full
        // producer. This metadata-only event profile cannot replace it.
        return Err(Error::Invalid(
            "navigation event Sign basis requires full owner",
        ));
    }
    let catalog_entry = render_catalog_record(
        input.source_record,
        source_path.as_str(),
        Some(schema_path.as_str()),
        limits.catalog.max_output_row_bytes,
    )?;
    if !same_canonical(
        &catalog_entry,
        input.entry,
        limits.catalog.max_output_row_bytes,
    )? {
        return Err(Error::Invalid("navigation event catalog entry differs"));
    }

    let expected_forms_ref = source_path
        .as_str()
        .strip_suffix(".json")
        .map(|stem| format!("{stem}.human-forms.json"))
        .ok_or(Error::Invalid("navigation event record path suffix"))?;
    if forms_path.as_str() != expected_forms_ref {
        return Err(Error::Invalid("navigation event forms binding"));
    }
    if let Some(schema_bytes) = input.forms_schema_bytes {
        let forms_schema_path = checked_path(input.forms_schema_path)?;
        if forms_schema_path.as_str() != "ToS/contracts/human-form-set.schema.json" {
            return Err(Error::Invalid("navigation event forms schema path"));
        }
        let _forms_schema = strict_value(schema_bytes, limits.catalog.max_row_bytes)?;
    } else if !input.forms_schema_path.is_empty() {
        return Err(Error::Invalid(
            "navigation event unexpected forms schema path",
        ));
    }
    let parent = source_path
        .as_str()
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let expected_history_ref = if parent.is_empty() {
        "source-revision-history.json".to_owned()
    } else {
        format!("{parent}/source-revision-history.json")
    };
    if history_path.as_str() != expected_history_ref {
        return Err(Error::Invalid("navigation event history path binding"));
    }
    let dependencies = validate_dependencies(
        &input,
        &event_path,
        &event_schema_path,
        &event_receipt_path,
        &source_path,
        &entity_registry_path,
        &entity_registry_schema_path,
        &schema_path,
        &forms_path,
        &history_path,
        selected_source_binding,
        limits,
    )?;
    validate_version_pairs(input.versions, limits)?;

    let kind = input
        .entry
        .get("record_type")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("navigation event record kind"))?;
    let native_composite =
        kind == "composite" && source_path.as_str().ends_with("/composite-witness.json");
    let projection = render_supplied_navigation_record(
        SuppliedNavigationRecordInput {
            entry: input.entry,
            source_record: input.source_record,
            forms: forms.as_ref().map(|(path, value)| (path.as_str(), value)),
            history: input.history,
            versions: input.versions,
            native_composite,
        },
        limits,
    )?;
    // The record and every available historical record have already passed
    // the exact public `RecordRoute` profile above. Visibility of the emitted
    // graph carriers then follows the same packet-node and typed-endpoint law
    // as the full compiler, never from an event caller-supplied boolean.
    let (node_visibility, edge_visibility) = projection_visibility(&projection)?;
    let record_id = input
        .entry
        .get("record_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or(Error::Invalid("navigation event record identity"))?
        .to_owned();
    let dependency_root = dependency_root(&dependencies, limits)?;
    let output_root = output_root(&projection, &node_visibility, &edge_visibility, limits)?;
    let input_root = input_root(
        input.event_id,
        input.event_sequence,
        &source_event_id,
        source_event_version,
        input.predecessor_source_revision,
        source_revision,
        input.predecessor_cohort_root,
        &event_path,
        &source_path,
        source_sha256,
        &record_id,
        &basis,
        dependency_root,
        output_root,
        limits,
    )?;
    Ok(NavigationRecordEventCohortV1 {
        event_id: input.event_id.to_owned(),
        event_sequence: input.event_sequence,
        source_event_id,
        source_event_version,
        predecessor_source_revision: input.predecessor_source_revision,
        source_revision,
        source_path,
        record_id,
        predecessor_cohort_root: input.predecessor_cohort_root,
        dependency_root,
        input_root,
        output_root,
        dependencies,
        projection,
        basis,
        node_visibility,
        edge_visibility,
        _sealed: NavigationRecordEventCohortSeal,
    })
}

#[cfg(test)]
mod current_event_head_tests {
    use super::current_event_head_matches;

    #[test]
    fn event_local_producer_requires_the_selected_source_head() {
        assert!(current_event_head_matches(7, 7));
        assert!(!current_event_head_matches(7, 8));
        assert!(!current_event_head_matches(8, 7));
    }
}

fn current_event_head_matches(event_sequence: u64, source_head_sequence: u64) -> bool {
    event_sequence != 0 && source_head_sequence == event_sequence
}

fn checked_path(value: &str) -> Result<RelativePath> {
    if value.is_empty() || value.len() > DEPENDENCY_PATH_BYTES {
        return Err(Error::Invalid("navigation dependency path bounds"));
    }
    RelativePath::parse(value).map_err(|_| Error::Invalid("navigation dependency path"))
}

fn validate_event_record_binding(
    event: &Value,
    receipt: &Value,
    event_path: &RelativePath,
    source_path: &RelativePath,
    source_bytes: &[u8],
    forms_bytes: Option<&[u8]>,
    forms_path: &RelativePath,
    event_file_bytes: &[u8],
    source_record: &Value,
) -> Result<()> {
    if event
        .pointer("/record_binding/digest_algorithm")
        .and_then(Value::as_str)
        != Some("sha256")
        || event
            .pointer("/record_binding/digest_scope")
            .and_then(Value::as_str)
            != Some("exact_event_record_bytes")
        || receipt.get("schema_version").and_then(Value::as_str)
            != Some("tos_local_source_create_receipt_v1")
        || receipt.get("source_path").and_then(Value::as_str) != Some(source_path.as_str())
        || receipt.pointer("/source/id").and_then(Value::as_str)
            != source_record.get("record_id").and_then(Value::as_str)
    {
        return Err(Error::Invalid("navigation event receipt owner binding"));
    }

    let outputs = event
        .pointer("/entities/outputs")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("navigation event outputs absent"))?;
    verify_event_output(outputs, source_path, source_bytes)?;
    if let Some(bytes) = forms_bytes {
        verify_event_output(outputs, forms_path, bytes)?;
    } else if outputs
        .iter()
        .any(|output| output.get("entity_ref").and_then(Value::as_str) == Some(forms_path.as_str()))
    {
        return Err(Error::Invalid(
            "navigation event forms output has no source",
        ));
    }

    let files = receipt
        .get("files")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("navigation event receipt files absent"))?;
    verify_receipt_file(files, event_path, event_file_bytes)?;
    verify_receipt_file(files, source_path, source_bytes)?;
    if let Some(bytes) = forms_bytes {
        verify_receipt_file(files, forms_path, bytes)?;
    }
    Ok(())
}

/// Select exactly one authored v2 event from its JSONL source file by the
/// receipt reference carried in that event. The file itself remains the
/// dependency and receipt fixity subject; schema execution applies to the
/// individual event record.
fn select_provenance_event_record(
    file_bytes: &[u8],
    receipt_path: &str,
    max_record_bytes: usize,
    max_file_bytes: usize,
) -> Result<Vec<u8>> {
    if file_bytes.is_empty() || file_bytes.len() > max_file_bytes || receipt_path.is_empty() {
        return Err(Error::Budget("navigation provenance event file"));
    }
    let mut selected: Option<Vec<u8>> = None;
    for raw_line in file_bytes.split(|byte| *byte == b'\n') {
        if raw_line.is_empty() {
            // Permit the conventional final line terminator, but reject blank
            // records because they make line identity and exact selection
            // ambiguous to downstream JSONL readers.
            if raw_line.as_ptr_range().start == file_bytes.as_ptr_range().end {
                continue;
            }
            return Err(Error::Invalid("navigation provenance event blank line"));
        }
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.len() > max_record_bytes {
            return Err(Error::Budget("navigation provenance event record"));
        }
        let value = strict_value(line, max_record_bytes)?;
        if value
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            != Some(receipt_path)
        {
            continue;
        }
        if selected.replace(line.to_vec()).is_some() {
            return Err(Error::Invalid(
                "navigation provenance event receipt is ambiguous",
            ));
        }
    }
    selected.ok_or(Error::Invalid(
        "navigation provenance event receipt is absent",
    ))
}

fn verify_event_output(outputs: &[Value], path: &RelativePath, bytes: &[u8]) -> Result<()> {
    let matched = outputs
        .iter()
        .filter(|output| output.get("entity_ref").and_then(Value::as_str) == Some(path.as_str()))
        .collect::<Vec<_>>();
    let Some(output) = matched.first().copied() else {
        return Err(Error::Invalid("navigation event exact output absent"));
    };
    if matched.len() != 1
        || output.get("sha256").and_then(Value::as_str)
            != Some(Digest256::of_bytes(bytes).to_hex().as_str())
        || output.get("size_bytes").and_then(Value::as_u64) != Some(bytes.len() as u64)
    {
        return Err(Error::Invalid("navigation event exact output binding"));
    }
    Ok(())
}

fn verify_receipt_file(
    files: &serde_json::Map<String, Value>,
    path: &RelativePath,
    bytes: &[u8],
) -> Result<()> {
    let name = path
        .as_str()
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("navigation receipt path basename"))?;
    let file = files
        .get(name)
        .ok_or(Error::Invalid("navigation receipt exact file absent"))?;
    let expected_digest = format!("sha256:{}", Digest256::of_bytes(bytes).to_hex());
    if file.get("sha256").and_then(Value::as_str) != Some(expected_digest.as_str())
        || file.get("bytes").and_then(Value::as_u64) != Some(bytes.len() as u64)
    {
        return Err(Error::Invalid("navigation receipt exact file binding"));
    }
    Ok(())
}

fn validate_history_view(
    history: Option<&Value>,
    history_bytes: Option<&[u8]>,
    source_path: &RelativePath,
    history_path: &RelativePath,
    entry: &Value,
    source_record: &Value,
    source_record_bytes: &[u8],
    versions: &[(Value, Value)],
    limits: BibliographicLimits,
) -> Result<()> {
    let Some(history) = history else {
        if history_bytes.is_some() || !versions.is_empty() {
            return Err(Error::Invalid("navigation unsupported history has inputs"));
        }
        return Ok(());
    };
    if history.get("status").and_then(Value::as_str) != Some("available")
        || history.get("reason").and_then(Value::as_str) != Some("verified-record-references")
        || history.get("record_id") != entry.get("record_id")
        || history.get("grants_current_use").and_then(Value::as_bool) != Some(false)
        || history.get("performs_assessment").and_then(Value::as_bool) != Some(false)
        || history.get("writes_to_source").and_then(Value::as_bool) != Some(false)
    {
        return Err(Error::Invalid("navigation history view identity"));
    }
    let refs = history
        .get("refs")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("navigation history view refs"))?;
    if refs.len() != versions.len()
        || refs
            .iter()
            .zip(versions)
            .any(|(reference, (resolved_reference, _))| reference != resolved_reference)
    {
        return Err(Error::Invalid("navigation history view ordered refs"));
    }
    let (current_ref, current_resolution) = versions
        .last()
        .ok_or(Error::Invalid("navigation current history ref absent"))?;
    if history.get("current_ref") != Some(current_ref)
        || current_resolution.get("status").and_then(Value::as_str) != Some("available")
        || current_resolution
            .get("version_status")
            .and_then(Value::as_str)
            != Some("current")
        || !same_canonical(
            current_resolution
                .get("record")
                .ok_or(Error::Invalid("navigation current history record absent"))?,
            source_record,
            limits.catalog.max_row_bytes,
        )?
    {
        return Err(Error::Invalid("navigation current history binding"));
    }
    let mut expected_provenance = current_resolution
        .get("provenance")
        .and_then(Value::as_object)
        .cloned()
        .ok_or(Error::Invalid("navigation current history provenance"))?;
    expected_provenance.remove("source");
    expected_provenance.remove("transition");
    let expected_provenance = Value::Object(expected_provenance);
    if !same_canonical(
        &expected_provenance,
        history
            .get("provenance")
            .ok_or(Error::Invalid("navigation history provenance absent"))?,
        limits.max_claim_cohort_bytes,
    )? {
        return Err(Error::Invalid("navigation history provenance binding"));
    }

    let provenance_history = current_resolution
        .pointer("/provenance/history")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid(
            "navigation retained history provenance absent",
        ))?;
    match history_bytes {
        Some(raw) => {
            let exact = strict_value(raw, limits.catalog.max_row_bytes)?;
            let receipts = exact
                .get("receipts")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("navigation retained history receipts"))?;
            let mut expected_refs = receipts
                .iter()
                .map(|receipt| {
                    receipt
                        .get("previous_source")
                        .cloned()
                        .ok_or(Error::Invalid("navigation retained history previous ref"))
                })
                .collect::<Result<Vec<_>>>()?;
            expected_refs.push(current_ref.clone());
            if !matches!(
                exact.get("schema_version").and_then(Value::as_str),
                Some("tos_source_revision_history_v1" | "tos_source_revision_history_v2")
            ) || exact.get("record_id") != entry.get("record_id")
                || receipts.is_empty()
                || receipts.len() > 128
                || refs != &expected_refs
                || provenance_history
                    .get("receipt_count")
                    .and_then(Value::as_u64)
                    != Some(receipts.len() as u64)
                || provenance_history.get("source_ref").and_then(Value::as_str)
                    != Some(history_path.as_str())
                || provenance_history.get("sha256").and_then(Value::as_str)
                    != Some(format!("sha256:{}", Digest256::of_bytes(raw).to_hex()).as_str())
            {
                return Err(Error::Invalid("navigation retained history source binding"));
            }
        }
        None => {
            if refs.len() != 1
                || refs.first() != Some(current_ref)
                || !provenance_history
                    .get("source_ref")
                    .is_some_and(Value::is_null)
                || !provenance_history.get("sha256").is_some_and(Value::is_null)
                || provenance_history
                    .get("receipt_count")
                    .and_then(Value::as_u64)
                    != Some(0)
            {
                return Err(Error::Invalid(
                    "navigation retained history absence binding",
                ));
            }
        }
    }
    let current_source = current_resolution
        .pointer("/provenance/source")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid(
            "navigation current source provenance absent",
        ))?;
    if current_source.get("source_ref").and_then(Value::as_str) != Some(source_path.as_str())
        || current_source
            .get("archive_blob_ref")
            .is_some_and(|value| !value.is_null())
        || current_source.get("record_bytes").and_then(Value::as_u64)
            != Some(source_record_bytes.len() as u64)
        || current_source.get("record_sha256").and_then(Value::as_str)
            != Some(
                format!(
                    "sha256:{}",
                    Digest256::of_bytes(source_record_bytes).to_hex()
                )
                .as_str(),
            )
    {
        return Err(Error::Invalid("navigation current raw source binding"));
    }
    Ok(())
}

fn validate_dependencies(
    input: &NavigationRecordEventInputV1<'_, '_>,
    event_path: &RelativePath,
    event_schema_path: &RelativePath,
    event_receipt_path: &RelativePath,
    record_path: &RelativePath,
    entity_registry_path: &RelativePath,
    entity_registry_schema_path: &RelativePath,
    schema_path: &RelativePath,
    forms_path: &RelativePath,
    history_path: &RelativePath,
    selected_source_binding: Digest256,
    limits: BibliographicLimits,
) -> Result<Vec<NavigationDependencyV1>> {
    let supplied = input.dependencies;
    let supplied_count = u64::try_from(supplied.len())
        .map_err(|_| Error::Budget("navigation event dependency count"))?;
    if supplied.is_empty() || supplied_count > limits.catalog.max_files {
        return Err(Error::Budget("navigation event dependency count"));
    }
    let mut dependencies = supplied.to_vec();
    dependencies.sort_by(|left, right| left.path.cmp(&right.path));
    if dependencies
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path)
    {
        return Err(Error::Invalid("navigation event duplicate dependency"));
    }
    let mut by_path = BTreeMap::new();
    let mut retained = 0usize;
    for dependency in &dependencies {
        let path = dependency.path.as_str();
        if path.len() > DEPENDENCY_PATH_BYTES {
            return Err(Error::Budget("navigation dependency path bytes"));
        }
        retained = retained
            .checked_add(path.len())
            .filter(|bytes| *bytes <= limits.max_claim_cohort_bytes)
            .ok_or(Error::Budget("navigation dependency manifest bytes"))?;
        match &dependency.state {
            NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                if *size_bytes == 0 && *sha256 != Digest256::of_bytes(&[]) {
                    return Err(Error::Invalid("navigation empty dependency digest"));
                }
                retained = retained
                    .checked_add(64 + std::mem::size_of::<u64>())
                    .filter(|bytes| *bytes <= limits.max_claim_cohort_bytes)
                    .ok_or(Error::Budget("navigation dependency manifest bytes"))?;
                by_path.insert(path, (dependency.role, Some((*sha256, *size_bytes))));
            }
            NavigationDependencyStateV1::Absent { reason } => {
                if reason.is_empty() || reason.len() > 128 {
                    return Err(Error::Invalid("navigation absent dependency reason"));
                }
                retained = retained
                    .checked_add(reason.len())
                    .filter(|bytes| *bytes <= limits.max_claim_cohort_bytes)
                    .ok_or(Error::Budget("navigation dependency manifest bytes"))?;
                by_path.insert(path, (dependency.role, None));
            }
        }
    }
    let exact_required = [
        (
            event_path,
            NavigationDependencyRoleV1::Event,
            input.event_file_bytes,
        ),
        (
            event_schema_path,
            NavigationDependencyRoleV1::EventSchema,
            input.event_schema_bytes,
        ),
        (
            event_receipt_path,
            NavigationDependencyRoleV1::EventReceipt,
            input.event_receipt_bytes,
        ),
        (
            record_path,
            NavigationDependencyRoleV1::Record,
            input.source_record_bytes,
        ),
        (
            entity_registry_path,
            NavigationDependencyRoleV1::EntityRegistry,
            input.entity_registry_bytes,
        ),
        (
            entity_registry_schema_path,
            NavigationDependencyRoleV1::EntityRegistrySchema,
            input.entity_registry_schema_bytes,
        ),
        (
            schema_path,
            NavigationDependencyRoleV1::RecordSchema,
            input.schema_bytes,
        ),
    ];
    for (path, role, expected_bytes) in exact_required {
        let (observed_role, observed) = by_path.get(path.as_str()).copied().ok_or(
            Error::Invalid("navigation event required dependency absent"),
        )?;
        if observed_role != role || observed.is_none() {
            return Err(Error::Invalid("navigation event required dependency state"));
        }
        if observed.map(|value| value.0) != Some(Digest256::of_bytes(expected_bytes))
            || observed.map(|value| value.1) != Some(expected_bytes.len() as u64)
        {
            return Err(Error::Invalid("navigation event source digest differs"));
        }
    }
    let (forms_role, forms_state) = by_path
        .get(forms_path.as_str())
        .copied()
        .ok_or(Error::Invalid("navigation event forms witness absent"))?;
    if forms_role != NavigationDependencyRoleV1::HumanForms
        || forms_state.is_some() != input.forms_bytes.is_some()
        || input.forms_bytes.is_some_and(|bytes| {
            forms_state.map(|state| state.0) != Some(Digest256::of_bytes(bytes))
                || forms_state.map(|state| state.1) != Some(bytes.len() as u64)
        })
    {
        return Err(Error::Invalid("navigation event forms witness differs"));
    }
    match (input.forms_bytes, input.forms_schema_bytes) {
        (Some(_), Some(schema_bytes)) => {
            let path = checked_path(input.forms_schema_path)?;
            let (role, state) = by_path.get(path.as_str()).copied().ok_or(Error::Invalid(
                "navigation event forms schema witness absent",
            ))?;
            if role != NavigationDependencyRoleV1::FormsSchema
                || state != Some((Digest256::of_bytes(schema_bytes), schema_bytes.len() as u64))
            {
                return Err(Error::Invalid(
                    "navigation event forms schema witness differs",
                ));
            }
        }
        (None, None) => {
            if dependencies
                .iter()
                .any(|dependency| dependency.role == NavigationDependencyRoleV1::FormsSchema)
            {
                return Err(Error::Invalid(
                    "navigation event unexpected forms schema input",
                ));
            }
        }
        _ => return Err(Error::Invalid("navigation event forms schema state")),
    }
    let (history_role, history_state) = by_path
        .get(history_path.as_str())
        .copied()
        .ok_or(Error::Invalid("navigation event history witness absent"))?;
    if history_role != NavigationDependencyRoleV1::RecordHistory
        || history_state.is_some() != input.history_bytes.is_some()
        || input.history_bytes.is_some_and(|bytes| {
            history_state.map(|state| state.0) != Some(Digest256::of_bytes(bytes))
                || history_state.map(|state| state.1) != Some(bytes.len() as u64)
        })
    {
        return Err(Error::Invalid("navigation event history witness differs"));
    }
    if input.version_sources.len() != input.versions.len() {
        return Err(Error::Invalid("navigation version source input count"));
    }
    check_selected_navigation_document(
        input.validator,
        selected_source_binding,
        input.source_path,
        schema_path.as_str(),
        input.schema_bytes,
        input.source_record_bytes,
    )?;
    check_selected_navigation_document(
        input.validator,
        selected_source_binding,
        entity_registry_path.as_str(),
        entity_registry_schema_path.as_str(),
        input.entity_registry_schema_bytes,
        input.entity_registry_bytes,
    )?;
    if let (Some(forms_bytes), Some(forms_schema_bytes)) =
        (input.forms_bytes, input.forms_schema_bytes)
    {
        check_selected_navigation_document(
            input.validator,
            selected_source_binding,
            forms_path.as_str(),
            input.forms_schema_path,
            forms_schema_bytes,
            forms_bytes,
        )?;
    }
    for (index, ((reference, resolution), version_source)) in
        input.versions.iter().zip(input.version_sources).enumerate()
    {
        let reference_digest = reference
            .get("digest")
            .and_then(Value::as_str)
            .filter(|digest| digest.starts_with("sha256:"))
            .ok_or(Error::Invalid("navigation version reference digest"))?;
        if resolution.get("record_digest").and_then(Value::as_str) != Some(reference_digest) {
            return Err(Error::Invalid("navigation version resolved digest differs"));
        }
        if resolution.get("status").and_then(Value::as_str) != Some("available") {
            if version_source.is_some() {
                return Err(Error::Invalid(
                    "navigation unavailable version has source bytes",
                ));
            }
            continue;
        }
        let version_source = version_source
            .as_ref()
            .ok_or(Error::Invalid("navigation available version source absent"))?;
        let record = resolution
            .get("record")
            .filter(|record| record.is_object())
            .ok_or(Error::Invalid("navigation available version record absent"))?;
        let canonical_digest = format!(
            "sha256:{}",
            source_record_digest(record, limits.catalog.max_row_bytes)?
        );
        if canonical_digest != reference_digest {
            return Err(Error::Invalid(
                "navigation version canonical digest differs",
            ));
        }
        let expected_schema =
            crate::source_bibliographic_versions::validate_supplied_metadata_version(
                input.entry,
                reference,
                record,
                input.entities,
                limits.catalog.max_row_bytes,
            )?;
        let version_path = checked_path(version_source.schema_path)?;
        if version_path.as_str() != expected_schema
            || version_source.schema_bytes.is_empty()
            || version_source.source_record_bytes.is_empty()
        {
            return Err(Error::Invalid("navigation version schema input binding"));
        }
        let source_from_bytes = strict_value(
            version_source.source_record_bytes,
            limits.catalog.max_row_bytes,
        )?;
        if !same_canonical(&source_from_bytes, record, limits.catalog.max_row_bytes)? {
            return Err(Error::Invalid(
                "navigation version exact source bytes differ",
            ));
        }
        let source_provenance = resolution
            .pointer("/provenance/source")
            .ok_or(Error::Invalid(
                "navigation version source provenance absent",
            ))?;
        let physical_ref = source_provenance
            .get("archive_blob_ref")
            .filter(|value| !value.is_null())
            .or_else(|| source_provenance.get("source_ref"))
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation version physical source absent"))?;
        let physical_path = checked_path(physical_ref)?;
        let raw_digest = Digest256::of_bytes(version_source.source_record_bytes);
        if index + 1 == input.versions.len()
            && (version_path != *schema_path
                || version_source.source_record_bytes != input.source_record_bytes
                || version_source.schema_bytes != input.schema_bytes)
        {
            return Err(Error::Invalid(
                "navigation current version exact inputs differ",
            ));
        }
        let (schema_role, schema_state) =
            by_path
                .get(version_path.as_str())
                .copied()
                .ok_or(Error::Invalid(
                    "navigation version schema dependency absent",
                ))?;
        let expected_schema_role = if version_path == *schema_path {
            NavigationDependencyRoleV1::RecordSchema
        } else {
            NavigationDependencyRoleV1::VersionSchema
        };
        if schema_role != expected_schema_role
            || schema_state
                != Some((
                    Digest256::of_bytes(version_source.schema_bytes),
                    version_source.schema_bytes.len() as u64,
                ))
        {
            return Err(Error::Invalid(
                "navigation version schema dependency differs",
            ));
        }
        let dependency_role = if physical_path == *record_path {
            NavigationDependencyRoleV1::Record
        } else {
            NavigationDependencyRoleV1::VersionSource
        };
        if by_path.get(physical_path.as_str()).copied()
            != Some((
                dependency_role,
                Some((raw_digest, version_source.source_record_bytes.len() as u64)),
            ))
        {
            return Err(Error::Invalid(
                "navigation version exact source dependency differs",
            ));
        }
        check_selected_navigation_document(
            input.validator,
            selected_source_binding,
            physical_path.as_str(),
            version_path.as_str(),
            version_source.schema_bytes,
            version_source.source_record_bytes,
        )?;
        validate_version_source_dependencies(resolution, record_path, history_path, &by_path)?;
    }
    Ok(dependencies)
}

fn validate_version_source_dependencies(
    resolution: &Value,
    record_path: &RelativePath,
    history_path: &RelativePath,
    dependencies: &BTreeMap<&str, (NavigationDependencyRoleV1, Option<(Digest256, u64)>)>,
) -> Result<()> {
    let source = resolution
        .pointer("/provenance/source")
        .filter(|source| source.is_object())
        .ok_or(Error::Invalid(
            "navigation version source provenance absent",
        ))?;
    let physical_ref = source
        .get("archive_blob_ref")
        .filter(|value| !value.is_null())
        .or_else(|| source.get("source_ref"))
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("navigation version physical source absent"))?;
    let physical_path = checked_path(physical_ref)?;
    let raw_digest = source
        .get("record_sha256")
        .filter(|value| !value.is_null())
        .or_else(|| source.get("stream_sha256"))
        .and_then(Value::as_str)
        .ok_or(Error::Invalid(
            "navigation version raw source digest absent",
        ))?;
    let raw_digest = Digest256::from_prefixed(raw_digest)
        .map_err(|_| Error::Invalid("navigation version raw source digest"))?;
    let raw_size = source
        .get("record_bytes")
        .filter(|value| !value.is_null())
        .or_else(|| source.get("stream_bytes"))
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("navigation version raw source size absent"))?;
    let (role, state) = dependencies
        .get(physical_path.as_str())
        .copied()
        .ok_or(Error::Invalid(
            "navigation version physical dependency absent",
        ))?;
    let expected_role = if physical_path == *record_path {
        NavigationDependencyRoleV1::Record
    } else {
        NavigationDependencyRoleV1::VersionSource
    };
    if role != expected_role || state != Some((raw_digest, raw_size)) {
        return Err(Error::Invalid(
            "navigation version physical dependency differs",
        ));
    }

    let manifest_ref = source
        .get("archive_manifest_ref")
        .filter(|value| !value.is_null());
    let manifest_digest = source
        .get("archive_manifest_sha256")
        .filter(|value| !value.is_null());
    match (manifest_ref, manifest_digest) {
        (None, None) => {}
        (Some(path), Some(digest)) => {
            let path = checked_path(
                path.as_str()
                    .ok_or(Error::Invalid("navigation version manifest path"))?,
            )?;
            let digest = Digest256::from_prefixed(
                digest
                    .as_str()
                    .ok_or(Error::Invalid("navigation version manifest digest"))?,
            )
            .map_err(|_| Error::Invalid("navigation version manifest digest"))?;
            require_dependency_digest(
                dependencies,
                &path,
                digest,
                &[NavigationDependencyRoleV1::VersionSource],
            )?;
        }
        _ => return Err(Error::Invalid("navigation version manifest binding")),
    }

    let catalog_ref = resolution
        .pointer("/provenance/catalog/source_ref")
        .filter(|value| !value.is_null());
    let catalog_digest = resolution
        .pointer("/provenance/catalog/sha256")
        .filter(|value| !value.is_null());
    match (catalog_ref, catalog_digest) {
        (None, None) => {}
        (Some(path), Some(digest)) => {
            let path = checked_path(
                path.as_str()
                    .ok_or(Error::Invalid("navigation version catalog path"))?,
            )?;
            let digest = Digest256::from_prefixed(
                digest
                    .as_str()
                    .ok_or(Error::Invalid("navigation version catalog digest"))?,
            )
            .map_err(|_| Error::Invalid("navigation version catalog digest"))?;
            require_dependency_digest(
                dependencies,
                &path,
                digest,
                &[
                    NavigationDependencyRoleV1::VersionSource,
                    NavigationDependencyRoleV1::RecordHistory,
                    NavigationDependencyRoleV1::Record,
                ],
            )?;
        }
        _ => return Err(Error::Invalid("navigation version catalog binding")),
    }

    let version_history_ref = resolution
        .pointer("/provenance/history/source_ref")
        .filter(|value| !value.is_null());
    let version_history_digest = resolution
        .pointer("/provenance/history/sha256")
        .filter(|value| !value.is_null());
    match (version_history_ref, version_history_digest) {
        (None, None) => {}
        (Some(path), Some(digest)) => {
            let path = checked_path(
                path.as_str()
                    .ok_or(Error::Invalid("navigation version history path"))?,
            )?;
            let digest = Digest256::from_prefixed(
                digest
                    .as_str()
                    .ok_or(Error::Invalid("navigation version history digest"))?,
            )
            .map_err(|_| Error::Invalid("navigation version history digest"))?;
            require_dependency_digest(
                dependencies,
                &path,
                digest,
                &[NavigationDependencyRoleV1::RecordHistory],
            )?;
            if path != *history_path {
                return Err(Error::Invalid(
                    "navigation version history identity differs",
                ));
            }
        }
        _ => return Err(Error::Invalid("navigation version history binding")),
    }
    Ok(())
}

fn require_dependency_digest(
    dependencies: &BTreeMap<&str, (NavigationDependencyRoleV1, Option<(Digest256, u64)>)>,
    path: &RelativePath,
    digest: Digest256,
    roles: &[NavigationDependencyRoleV1],
) -> Result<()> {
    let (role, state) = dependencies
        .get(path.as_str())
        .copied()
        .ok_or(Error::Invalid(
            "navigation version provenance dependency absent",
        ))?;
    if !roles.contains(&role) || state.is_none_or(|state| state.0 != digest) {
        return Err(Error::Invalid(
            "navigation version provenance dependency differs",
        ));
    }
    Ok(())
}

fn validate_version_pairs(versions: &[(Value, Value)], limits: BibliographicLimits) -> Result<()> {
    if versions.len() > 129 {
        return Err(Error::Budget("navigation event version count"));
    }
    let mut refs = BTreeSet::new();
    let mut bytes = 0usize;
    for (reference, resolution) in versions {
        bytes = bytes
            .checked_add(canonical(reference, limits.catalog.max_row_bytes)?.len())
            .and_then(|value| {
                value.checked_add(
                    canonical(resolution, limits.catalog.max_output_row_bytes)
                        .ok()?
                        .len(),
                )
            })
            .filter(|value| *value <= limits.max_claim_cohort_bytes)
            .ok_or(Error::Budget("navigation event version bytes"))?;
        if resolution.get("exact_ref") != Some(reference)
            || !refs.insert(canonical(reference, limits.catalog.max_row_bytes)?)
        {
            return Err(Error::Invalid("navigation event version identity"));
        }
        if reference.get("digest").and_then(Value::as_str)
            != resolution.get("record_digest").and_then(Value::as_str)
        {
            return Err(Error::Invalid("navigation event version digest binding"));
        }
    }
    Ok(())
}

fn dependency_root(
    dependencies: &[NavigationDependencyV1],
    limits: BibliographicLimits,
) -> Result<Digest256> {
    dependency_root_with_cap(dependencies, limits.max_claim_cohort_bytes)
}

fn dependency_root_with_cap(
    dependencies: &[NavigationDependencyV1],
    byte_cap: usize,
) -> Result<Digest256> {
    let rows = dependencies
        .iter()
        .map(|dependency| {
            let state = match &dependency.state {
                NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                    json!({"state":"present","sha256":sha256.to_hex(),"size_bytes":size_bytes})
                }
                NavigationDependencyStateV1::Absent { reason } => {
                    json!({"state":"absent","reason":reason})
                }
            };
            json!({"path":dependency.path.as_str(),"role":dependency.role.as_str(),"observation":state})
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({"schema":"tos_source_navigation_dependencies_v1","dependencies":rows}),
        byte_cap,
    )
}

fn output_root(
    projection: &SuppliedNavigationRecordProjection,
    node_visibility: &[bool],
    edge_visibility: &[bool],
    limits: BibliographicLimits,
) -> Result<Digest256> {
    output_root_with_caps(
        projection,
        node_visibility,
        edge_visibility,
        limits.catalog.max_output_row_bytes,
        limits.max_claim_cohort_bytes,
    )
}

fn output_root_with_caps(
    projection: &SuppliedNavigationRecordProjection,
    node_visibility: &[bool],
    edge_visibility: &[bool],
    row_byte_cap: usize,
    root_byte_cap: usize,
) -> Result<Digest256> {
    if node_visibility.len() != projection.nodes.len()
        || edge_visibility.len() != projection.edges.len()
    {
        return Err(Error::Invalid("navigation event visibility witness count"));
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
                .ok_or(Error::Invalid("navigation event output identity"))?;
            if rows
                .insert(
                    (collection.to_owned(), id.to_owned()),
                    (digest_json(row, row_byte_cap)?, *visible),
                )
                .is_some()
            {
                return Err(Error::Invalid("navigation event duplicate output identity"));
            }
        }
    }
    let diagnostics = projection
        .diagnostics
        .iter()
        .map(|row| digest_json(row, row_byte_cap).map(Digest256::to_hex))
        .collect::<Result<Vec<_>>>()?;
    let row_manifest = rows
        .into_iter()
        .map(|((collection, id), (digest, visible))| {
            json!([collection, id, digest.to_hex(), visible])
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({"schema":"tos_source_navigation_record_cohort_v1","rows":row_manifest,"diagnostics":diagnostics}),
        root_byte_cap,
    )
}

fn projection_visibility(
    projection: &SuppliedNavigationRecordProjection,
) -> Result<(Vec<bool>, Vec<bool>)> {
    let mut node_visibility = Vec::with_capacity(projection.nodes.len());
    let mut by_node_id = BTreeMap::<String, bool>::new();
    for row in &projection.nodes {
        let id = row
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(Error::Invalid("navigation event node visibility identity"))?;
        let visible = visible_node(row)?;
        if by_node_id.insert(id.to_owned(), visible).is_some() {
            return Err(Error::Invalid(
                "navigation event duplicate node visibility identity",
            ));
        }
        node_visibility.push(visible);
    }
    let mut edge_visibility = Vec::with_capacity(projection.edges.len());
    for row in &projection.edges {
        let from_id = row
            .get("from_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(Error::Invalid("navigation event edge visibility source"))?;
        let to_id = row
            .get("to_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(Error::Invalid("navigation event edge visibility target"))?;
        let from_visible = by_node_id.get(from_id).copied().ok_or(Error::Invalid(
            "navigation event edge visibility source absent",
        ))?;
        let to_visible = by_node_id.get(to_id).copied().ok_or(Error::Invalid(
            "navigation event edge visibility target absent",
        ))?;
        edge_visibility.push(from_visible && to_visible);
    }
    Ok((node_visibility, edge_visibility))
}

fn visible_node(row: &Value) -> Result<bool> {
    let raw = serde_json::to_vec(row)
        .map_err(|_| Error::Invalid("navigation event node visibility JSON"))?;
    let limits = JsonLimits::new(raw.len().max(1), 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation event node visibility limits"))?;
    let parsed =
        tos_foundation::parse_json(&raw, tos_foundation::JsonMode::PublishedStrict, limits)
            .map_err(|_| Error::Invalid("navigation event node visibility JSON"))?;
    Ok(crate::visible_node(&parsed.into_root()))
}

#[allow(clippy::too_many_arguments)]
fn input_root(
    event_id: &str,
    sequence: u64,
    source_event_id: &str,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    predecessor_cohort_root: Option<Digest256>,
    event_path: &RelativePath,
    source_path: &RelativePath,
    source_sha256: Digest256,
    record_id: &str,
    basis: &NavigationRecordEventBasisV1,
    dependency_root: Digest256,
    output_root: Digest256,
    limits: BibliographicLimits,
) -> Result<Digest256> {
    input_root_with_cap(
        event_id,
        sequence,
        source_event_id,
        source_event_version,
        predecessor_source_revision,
        source_revision,
        predecessor_cohort_root,
        event_path,
        source_path,
        source_sha256,
        record_id,
        basis,
        dependency_root,
        output_root,
        limits.max_claim_cohort_bytes,
    )
}

#[allow(clippy::too_many_arguments)]
fn input_root_with_cap(
    event_id: &str,
    sequence: u64,
    source_event_id: &str,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    predecessor_cohort_root: Option<Digest256>,
    event_path: &RelativePath,
    source_path: &RelativePath,
    source_sha256: Digest256,
    record_id: &str,
    basis: &NavigationRecordEventBasisV1,
    dependency_root: Digest256,
    output_root: Digest256,
    byte_cap: usize,
) -> Result<Digest256> {
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
        } => (
            "tos_source_navigation_event_input_v4",
            json!({
                "kind":"dependency-refresh-set",
                "changed_dependencies":changed_dependencies.iter().map(|cause| json!({
                    "path":cause.path.as_str(),
                    "sha256":cause.sha256.to_hex(),
                })).collect::<Vec<_>>(),
            }),
        ),
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
    digest_json(
        &json!({
            "schema":input_schema,
            "event_id":event_id,
            "event_sequence":sequence,
            "source_event_id":source_event_id,
            "source_event_version":source_event_version,
            "predecessor_source_revision":predecessor_source_revision.to_hex(),
            "source_revision":source_revision.to_hex(),
            "predecessor_cohort_root":predecessor_cohort_root.map(Digest256::to_hex),
            "event_path":event_path.as_str(),
            "source_path":source_path.as_str(),
            "source_sha256":source_sha256.to_hex(),
            "record_id":record_id,
            "basis":basis,
            "dependency_root":dependency_root.to_hex(),
            "output_root":output_root.to_hex(),
        }),
        byte_cap,
    )
}

fn navigation_record_event_budget_cost(
    projection: &SuppliedNavigationRecordProjection,
    dependencies: &[NavigationDependencyV1],
    byte_cap: usize,
) -> Result<NavigationRecordEventBudgetCostV1> {
    let output_rows = projection
        .nodes
        .len()
        .checked_add(projection.edges.len())
        .and_then(|count| count.checked_add(projection.diagnostics.len()))
        .ok_or(Error::Budget("navigation Record aggregate row count"))?;
    let rows = output_rows
        .checked_add(dependencies.len())
        .ok_or(Error::Budget("navigation Record aggregate row count"))?;
    let mut dependency_bytes = 0u64;
    let mut encoded_bytes = 0u64;
    for dependency in dependencies {
        encoded_bytes = encoded_bytes
            .checked_add(dependency.path.as_str().len() as u64)
            .ok_or(Error::Budget("navigation Record aggregate encoded bytes"))?;
        if let NavigationDependencyStateV1::Present { size_bytes, .. } = &dependency.state {
            dependency_bytes = dependency_bytes
                .checked_add(*size_bytes)
                .ok_or(Error::Budget(
                    "navigation Record aggregate dependency bytes",
                ))?;
        }
    }
    for row in projection
        .nodes
        .iter()
        .chain(&projection.edges)
        .chain(&projection.diagnostics)
    {
        encoded_bytes = encoded_bytes
            .checked_add(canonical(row, byte_cap)?.len() as u64)
            .ok_or(Error::Budget("navigation Record aggregate encoded bytes"))?;
    }
    Ok(NavigationRecordEventBudgetCostV1 {
        rows: u64::try_from(rows)
            .map_err(|_| Error::Budget("navigation Record aggregate row count"))?,
        encoded_bytes,
        dependency_bytes,
        dependency_count: u64::try_from(dependencies.len())
            .map_err(|_| Error::Budget("navigation Record dependency count"))?,
    })
}

fn same_canonical(left: &Value, right: &Value, cap: usize) -> Result<bool> {
    Ok(canonical(left, cap)? == canonical(right, cap)?)
}

fn digest_json(value: &Value, cap: usize) -> Result<Digest256> {
    Ok(Digest256::of_bytes(&canonical(value, cap)?))
}

fn canonical(value: &Value, cap: usize) -> Result<Vec<u8>> {
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation event JSON limits"))?;
    let typed = tos_foundation::parse_json(
        &serde_json::to_vec(value).map_err(|_| Error::Invalid("navigation event JSON"))?,
        tos_foundation::JsonMode::PublishedStrict,
        limits,
    )
    .map_err(|_| Error::Invalid("navigation event JSON"))?;
    canonical_bytes_v1(
        &typed.into_root(),
        CanonicalProfile::CorpusSnapshotV1,
        limits,
    )
    .map_err(|_| Error::Budget("navigation event canonical bytes"))
}

fn strict_value(raw: &[u8], cap: usize) -> Result<Value> {
    if raw.is_empty() || raw.len() > cap {
        return Err(Error::Budget("navigation event source document bytes"));
    }
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation event source JSON limits"))?;
    let document =
        tos_foundation::parse_json(raw, tos_foundation::JsonMode::PublishedStrict, limits)
            .map_err(|_| Error::Invalid("navigation event strict source JSON"))?;
    let normalized = tos_foundation::emit_value_preserved_json(document.root(), limits)
        .map_err(|_| Error::Budget("navigation event source JSON bytes"))?;
    serde_json::from_slice(&normalized)
        .map_err(|_| Error::Invalid("navigation event source JSON value"))
}

#[cfg(test)]
mod tests {
    use super::{output_root, projection_visibility, select_provenance_event_record};
    use crate::source_bibliographic::{BibliographicLimits, SuppliedNavigationRecordProjection};
    use crate::source_witness_catalog::SourceCatalogLimits;
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn test_limits() -> BibliographicLimits {
        BibliographicLimits {
            catalog: SourceCatalogLimits {
                max_files: 16,
                max_rows: 16,
                max_file_bytes: 16_384,
                max_row_bytes: 16_384,
                max_contract_bytes: 16_384,
                max_output_row_bytes: 16_384,
            },
            max_claim_cohort_rows: 16,
            max_claim_cohort_bytes: 65_536,
            max_output_rows: 16,
            max_output_bytes: 65_536,
            deadline: Instant::now() + Duration::from_secs(5),
        }
    }

    #[test]
    fn selects_one_event_jsonl_row_by_source_receipt() {
        let file = br#"{"event_id":"old","event_version":1,"record_binding":{"manifest_ref":"old-receipt.json"}}
{"event_id":"wanted","event_version":2,"record_binding":{"manifest_ref":"wanted-receipt.json"}}
"#;
        let selected = select_provenance_event_record(file, "wanted-receipt.json", 1024, 4096)
            .expect("one exact source event row");
        assert_eq!(
            selected,
            br#"{"event_id":"wanted","event_version":2,"record_binding":{"manifest_ref":"wanted-receipt.json"}}"#
        );
    }

    #[test]
    fn rejects_duplicate_receipt_rows_in_event_jsonl() {
        let file =
            br#"{"event_id":"one","event_version":1,"record_binding":{"manifest_ref":"same.json"}}
{"event_id":"two","event_version":2,"record_binding":{"manifest_ref":"same.json"}}"#;
        assert!(select_provenance_event_record(file, "same.json", 1024, 4096).is_err());
    }

    #[test]
    fn event_visibility_uses_node_and_typed_endpoint_contract() {
        let projection = SuppliedNavigationRecordProjection {
            nodes: vec![
                json!({"node_id":"hidden","properties":{"packet_id":"packet-1"}}),
                json!({"node_id":"public","properties":{}}),
            ],
            edges: vec![json!({"edge_id":"e1","from_id":"hidden","to_id":"public"})],
            diagnostics: Vec::new(),
        };
        let (nodes, edges) = projection_visibility(&projection).expect("checked visibility");
        assert_eq!(nodes, [false, true]);
        assert_eq!(edges, [false]);

        let limits = test_limits();
        let root = output_root(&projection, &nodes, &edges, limits).expect("sealed root");
        let changed = output_root(&projection, &[true, true], &[true], limits)
            .expect("alternate visibility root");
        assert_ne!(root, changed);
    }
}
