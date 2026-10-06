//! Current source-record and Item mechanics for the maintained ToS foundation
//! validator. This composes existing bounded owner kernels over one captured
//! cut; its result is a district report, never whole-source admission.

use crate::item_rules::{
    ItemFamilyReport, ItemIssue, ItemLimits, ItemPayload, ItemRefusal, ItemRules, ItemSource,
};
use crate::record_biblio_cut::{
    BiblioCurrentRecord, BiblioRecordExecutor, SourceCutInput, SourceCutInputCoverage,
    SourceCutInputWithIdentity, SourceCutMemberMeta, SourceCutRecordFactBudget,
    SourceCutRecordReport, SourceCutRecordUsage, SourceCutStreamedRecordSummary,
    inspect_records_from_cut, inspect_records_from_input_stored,
};
use crate::source_cut::{
    CandidateCutSchemaDiagnostic, CutPayloadReader, CutPreparedSchemaExecutionBinding,
    CutSchemaDiagnosticsCumulativeCost, CutSchemaExecutor, CutWorkerSchemaExecutor,
};
use crate::source_foundation_discovery::SourcePhysicalFacts;
use crate::source_witness_foundation::SourceFileMembershipIndex;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1, SourcePresenceV1};

#[path = "source_foundation_records_storage.rs"]
mod source_foundation_records_storage;
pub use source_foundation_records_storage::{
    NATIVE_ARTIFACT_RECORD_SCHEMA_URI, SourceFoundationArtifactRecordPathSummary,
    SourceFoundationCandidateArtifactProofPathPage, SourceFoundationCandidateSchemaIdentity,
    SourceFoundationCurrentRecordPathLookup, SourceFoundationCurrentRecordsPage,
    SourceFoundationFileDescriptorLookup, SourceFoundationGlobalIdFact,
    SourceFoundationGlobalIdFactPage, SourceFoundationItemEditionLookup,
    SourceFoundationItemSelectionLookup, SourceFoundationLinkUriFact, SourceFoundationRecordFact,
    SourceFoundationRecordFactCollection, SourceFoundationRecordFactPage,
    SourceFoundationRecordIdCarrier, SourceFoundationRecordObservation,
    SourceFoundationRecordPathReference, SourceFoundationRecordSchemaDiagnostic,
    SourceFoundationRecordsCollection, SourceFoundationRecordsCursor,
    SourceFoundationRecordsCursorPage, SourceFoundationRecordsIndex, SourceFoundationRecordsLookup,
    SourceFoundationRecordsPageBudget, SourceFoundationRecordsStore,
    SourceFoundationRecordsStoredFact, SourceFoundationRecordsStreamedItemSummary,
    SourceFoundationRecordsStreamedReport, SourceFoundationTypedIdRefFact,
    SourceFoundationUriOwnerLookup,
};

/// One exact resource selected into the candidate-bound Item schema closure.
/// The input receiver compares this tuple with the same `SourceCutInput` and
/// verifies the raw bytes before allowing the unchanged Item rules to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFoundationCandidateSchemaResource<'a> {
    pub path: &'a str,
    pub size_bytes: u64,
    pub sha256: Digest256,
}

/// Candidate-bound schema executor contract consumed by the source receiver.
/// The identity is opaque and typed by the command owner; it is not a
/// `SourceRevision`. Resource enumeration is in strictly ascending path order
/// and covers the complete selected closure.
pub trait SourceFoundationCandidateSchemaBinding<I: Eq + Clone>: CutSchemaExecutor {
    fn input_identity(&self) -> &I;
    fn prepared_execution_binding(&self) -> CutPreparedSchemaExecutionBinding;
    fn profile(&self) -> crate::FormatProfile;
    fn schema_set_digest(&self) -> Digest256;
    fn contract_selection_digest(&self) -> Digest256;
    fn contract_digest(&self, contract: &str) -> Option<Digest256>;
    fn for_each_selected_resource(
        &self,
        visit: &mut dyn FnMut(
            SourceFoundationCandidateSchemaResource<'_>,
        ) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;

    /// Execute the unchanged schema selector, including any JSON-pointer
    /// fragment in `contract`. The default forwards the contract byte-for-byte
    /// through the established scalar executor.
    fn check_with_fragment(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.check(path, raw, contract, deadline, cancelled)
    }

    /// Return one complete typed diagnostics-v2 result from the exact prepared
    /// candidate schema binding. Scalar implementations must not silently
    /// synthesize this report.
    fn check_diagnostics_v2(
        &mut self,
        _path: &str,
        _raw: &[u8],
        _contract: &str,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<CandidateCutSchemaDiagnostic<I>, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "candidate schema diagnostics v2 unavailable".into(),
        ))
    }

    /// Move the next complete invalid diagnostics report retained by a bool
    /// projection. Implementations without this protocol fail closed.
    fn take_schema_diagnostic_rejection(
        &mut self,
    ) -> Result<Option<CandidateCutSchemaDiagnostic<I>>, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "candidate schema diagnostics rejection drain unavailable".into(),
        ))
    }

    /// Report the actual number of diagnostics-v2 executions observed by the
    /// prepared worker. A scalar default must not masquerade as zero work.
    fn diagnostic_execution_count(&self) -> Result<usize, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "candidate schema diagnostics execution count unavailable".into(),
        ))
    }

    /// Return checked cumulative cost for complete diagnostics-v2 exchanges.
    fn diagnostics_v2_cumulative_cost(
        &self,
    ) -> Result<CutSchemaDiagnosticsCumulativeCost, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "candidate schema diagnostics cumulative cost unavailable".into(),
        ))
    }
}

const ITEM_MANIFEST_SUFFIX: &str = "/item.manifest.json";
const SOURCE_HOME: &str = "ToS/source-witnesses/";
const CATALOG_HOME: &str = "ToS/source-witnesses/catalog/";
const RECORD_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const SCHEMA_HOME: &str = "ToS/contracts/";
const SCHEMA_SUFFIX: &str = ".schema.json";
const REQUIRED_ITEM_SCHEMAS: [&str; 4] = [
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/source-resource-inventory.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/provenance-event.schema.json",
];
const LEGACY_RECORD_BASENAMES: [&str; 8] = [
    "agent.json",
    "place.json",
    "organization.json",
    "work.json",
    "expression.json",
    "edition.json",
    "collection.json",
    "item.json",
];
const LEGACY_RECORD_TYPES: [&str; 8] = [
    "agent",
    "place",
    "organization",
    "work",
    "expression",
    "edition",
    "collection",
    "item",
];

/// Caller-owned limits for one record+Item invocation. The record and Item
/// sublimits are explicit because both kernels reread members from the same
/// immutable cut. The fixed entry requires family ceilings to fit under the
/// operation ceiling; the additive rolling entry rolls actual completed
/// Records usage into Item. This type adds no corpus-size defaults.
#[derive(Debug, Clone, Copy)]
pub struct SourceFoundationRecordsLimits {
    pub operation: ItemLimits,
    pub records: ItemLimits,
    pub items: ItemLimits,
    /// Sum of current schema-resource bytes selected by the record and Item
    /// families, counted once for each family's resource closure.
    pub max_schema_resource_bytes: u64,
    /// Caller-reserved retained auxiliary state for diagnostic-v2 decoded
    /// Item schema requests, direct ordered owner issues, rights/event
    /// indexes and the direct Item district's peak temporary work area. This
    /// is removed from the ItemRules quota.
    pub max_schema_request_state_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationRecordsCost {
    /// Unique current-cut member size from authenticated metadata, not a claim
    /// about how many times those bytes were read by the two kernels.
    pub selected_current_member_bytes: u64,
    pub record_schema_resource_bytes: u64,
    pub item_schema_resource_bytes: u64,
    pub aggregate_schema_resource_bytes: u64,
    /// Aggregate ceiling shared by the direct owner scan and record kernel.
    pub record_read_limit_bytes: u64,
    /// Remaining record-family bytes assigned to the record kernel after the
    /// direct owner scan's exact reads are reserved.
    pub record_kernel_read_limit_bytes: u64,
    pub item_read_limit_bytes: u64,
    pub item_record_read_bytes: u64,
    /// Separate authenticated reads used to recover Python's all-decoded
    /// `item_records` selection (last value per id, first insertion order).
    pub item_selection_read_bytes: u64,
    /// Separate authenticated reads used by ItemRules after selection.
    pub item_rule_record_read_bytes: u64,
    /// Reads used to build the reusable Item-to-File relation index.
    pub file_membership_read_bytes: u64,
    /// Exact extra current reads for fixity companions and final Item-record
    /// manifest references used by direct owner predicates.
    pub item_owner_read_bytes: u64,
    pub item_metadata_read_limit_bytes: u64,
    pub combined_family_read_limit_bytes: u64,
    pub operation_read_limit_bytes: u64,
    /// In rolling mode, exact source bytes read by the completed record kernel
    /// plus the direct current-record pass. The fixed legacy entry retains its
    /// prior metadata-derived estimate. A refused kernel discards its internal
    /// partial-read counter, so no combined actual total is represented.
    pub record_observed_read_bytes: Option<u64>,
    /// Additional bounded decoded traversal which restores `_record_paths`
    /// basename order and direct Python owner checks over current bytes.
    pub record_owner_read_bytes: u64,
    pub record_owner_state_bytes: usize,
    /// Item records are read directly; ItemRules separately reports metadata
    /// reads. Payload hashing remains owned by the payload reader.
    pub item_observed_read_bytes: u64,
    pub item_observed_metadata_bytes: u64,
    /// ItemRules family prefix-scan work for set(resource_ids).
    pub item_inventory_set_scan_steps: usize,
    /// Ordered direct owner prefix-scan work for set(resource_ids).
    pub item_owner_inventory_set_scan_steps: usize,
    /// Aggregate family and direct-owner work, bounded by the Item state cap.
    pub aggregate_inventory_set_scan_steps: usize,
    pub inventory_set_scan_step_limit: usize,
    pub record_state_limit_bytes: usize,
    pub item_state_limit_bytes: usize,
    pub item_index_state_bytes: usize,
    pub file_membership_state_bytes: usize,
    pub schema_request_state_limit_bytes: usize,
    pub schema_request_state_bytes: usize,
    pub rights_identity_state_bytes: usize,
    pub source_event_state_bytes: usize,
    /// Peak direct Item owner indexes plus one manifest's temporary state.
    pub item_owner_index_state_bytes: usize,
    pub item_auxiliary_state_bytes: usize,
    pub item_rule_state_limit_bytes: usize,
    pub combined_family_state_limit_bytes: usize,
    pub operation_state_limit_bytes: usize,
    pub ordered_issue_state_bytes: usize,
    pub record_issue_limit: usize,
    pub item_issue_limit: usize,
    pub combined_family_issue_limit: usize,
    pub operation_issue_limit: usize,
    /// Present only when the additive Records→Item rolling entry completed.
    /// This is the combined logical state charge for the retained source
    /// report, not a process-memory or RSS measurement. Scheduled schema-check
    /// inputs are included; the later diagnostics bridge must add its output
    /// reports. The operation limit remains available separately above.
    pub rolling_accounted_state_upper_bound_bytes: Option<usize>,
    /// Exact issue observations seen by a completed rolling entry: Biblio
    /// kernel issues, direct ordered owner issues, and ItemRules issues. The
    /// later diagnostics bridge still has to add verdicts for scheduled
    /// `schema_checks`; they have not executed at this boundary.
    pub rolling_observed_issue_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationRecordsIssueFamily {
    Record,
    Item,
}

/// Direct owner-rule issues from `_record_paths`, links, record joins and the
/// current Item companion traversal. Family kernel reports remain separate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationRecordsIssue {
    pub family: SourceFoundationRecordsIssueFamily,
    pub location: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationRecordsOwnerIssue {
    JsonRootMustBeObject,
    JsonlRecordMustBeObject,
    InvalidJson,
    InvalidJsonl,
    BlankJsonlLine,
    InvalidJsonlUtf8,
}

impl SourceFoundationRecordsOwnerIssue {
    fn message(self) -> &'static str {
        match self {
            Self::JsonRootMustBeObject => "JSON root must be an object",
            Self::JsonlRecordMustBeObject => "JSONL record must be an object",
            Self::InvalidJson => "cannot read JSON: invalid JSON",
            Self::InvalidJsonl => "invalid JSON: invalid JSON syntax",
            Self::BlankJsonlLine => "blank JSONL line is not allowed",
            Self::InvalidJsonlUtf8 => "cannot read JSONL: invalid UTF-8",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationRecordsSchemaFamily {
    Record,
    Item,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationRecordsSchemaCheck {
    pub family: SourceFoundationRecordsSchemaFamily,
    pub before_issue: usize,
    pub location: String,
    pub contract: String,
    /// Exactly one of the finite decoded value or original legacy raw bytes is
    /// present for a schema-worker request. Both are absent only for a closed
    /// parser/JSONL owner issue; decoded null/array remains an input and can
    /// also carry its independent object-root owner diagnostic.
    pub decoded_instance: Option<Value>,
    pub legacy_raw_instance: Option<Vec<u8>>,
    pub owner_issue: Option<SourceFoundationRecordsOwnerIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationRecordKernelCause {
    MalformedJson,
    NonObjectRoot,
    MixedInvalidCandidates,
}

/// The record-family kernel can refuse a malformed/non-object candidate before
/// completing its mechanical scan. That refusal remains distinct from fatal
/// budget, deadline, cancellation, source, and worker failures; callers must
/// not use a refused family as positive evidence.
pub enum SourceFoundationRecordKernelOutcome {
    Complete(SourceCutRecordReport),
    Refused {
        refusal: ItemRefusal,
        cause: SourceFoundationRecordKernelCause,
    },
}

/// Ordered Python `events_by_id[event_id] = event` insertions from the Item
/// provenance scan. Repeated IDs intentionally remain in this vector so the
/// caller can preserve last-value replacement and first insertion position.
pub type SourceFoundationEventInsertion = (String, Value);

pub struct SourceFoundationRecordsReport {
    pub source_revision: SourceRevision,
    pub source_membership: SourceMembershipV1,
    pub records: SourceFoundationRecordKernelOutcome,
    pub items: ItemFamilyReport,
    /// Authenticated direct reconstruction of Python `records_by_id`,
    /// independent of the record-family kernel's complete/refused outcome.
    /// Duplicate IDs retain their first value; links are inserted after the
    /// `_record_paths` records, matching the maintained owner order.
    pub current_records: BTreeMap<String, BiblioCurrentRecord>,
    /// Dict insertion order for `current_records`; the map itself is keyed for
    /// bounded lookups by Gold and Closure helpers.
    pub current_record_order: Vec<String>,
    /// Declared profile kinds that are actually used by selected current
    /// records. This is the maintained validator's `any(record_type in
    /// profiles.profiles)` signal, not the full configured profile set.
    pub used_declared_profile_kinds: BTreeSet<String>,
    /// Python `_record_paths` selects decoded `item` records by ID. Duplicate
    /// IDs replace the value without changing the first insertion position;
    /// this vector preserves that order and the final selected path.
    pub item_records: Vec<SourceFoundationItemRecordSelection>,
    /// Direct owner-row insertion point for the registry and each record/Link schema plan.
    /// Indices refer to `ordered_issues` before schema diagnostics are spliced;
    /// repeated positions retain `_record_paths` evaluation order.
    pub record_schema_positions: Vec<(String, usize)>,
    /// Exact source Item-to-File memberships and first File descriptors. This
    /// is content relation state, not a filesystem or source-cut certificate.
    pub file_memberships: SourceFileMembershipIndex,
    /// IDs observed in object-valued rights metadata during sorted manifest
    /// traversal, matching the maintained set used by later gold-set rules.
    pub rights_ids: BTreeSet<String>,
    /// The maintained owner initializes this set empty at the end of the
    /// records loop; source-claim rows are first admitted by the later Gold
    /// district, so this is intentionally empty here.
    pub claim_ids: BTreeSet<String>,
    /// Last `embodiment_ref` per manifest item_id, in the maintained sorted
    /// manifest traversal's first-insertion order.
    pub item_editions: BTreeMap<String, String>,
    pub source_event_insertions: Vec<SourceFoundationEventInsertion>,
    pub ordered_issues: Vec<SourceFoundationRecordsIssue>,
    pub schema_checks: Vec<SourceFoundationRecordsSchemaCheck>,
    /// Known route-native gaps in this source district. A family kernel code
    /// report never stands in for the Python owner message or ordering.
    pub unimplemented: Vec<&'static str>,
    pub cost: SourceFoundationRecordsCost,
}

struct SourceFoundationRecordsCoreReport {
    source_membership: SourceMembershipV1,
    records: Option<SourceFoundationRecordKernelOutcome>,
    record_usage: Option<SourceCutRecordUsage>,
    items: ItemFamilyReport,
    current_records: BTreeMap<String, BiblioCurrentRecord>,
    current_record_order: Vec<String>,
    used_declared_profile_kinds: BTreeSet<String>,
    item_records: Vec<SourceFoundationItemRecordSelection>,
    record_schema_positions: Vec<(String, usize)>,
    file_memberships: SourceFileMembershipIndex,
    rights_ids: BTreeSet<String>,
    claim_ids: BTreeSet<String>,
    item_editions: BTreeMap<String, String>,
    source_event_insertions: Vec<SourceFoundationEventInsertion>,
    ordered_issues: Vec<SourceFoundationRecordsIssue>,
    schema_checks: Vec<SourceFoundationRecordsSchemaCheck>,
    unimplemented: Vec<&'static str>,
    cost: SourceFoundationRecordsCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationItemRecordSelection {
    pub record_id: String,
    pub path: String,
    pub value: Value,
}

struct SelectedItemRecord {
    selection: SourceFoundationItemRecordSelection,
}

// A retained source reference authenticates a physical member; it does not
// select that other Item and all of its companions for semantic admission.
fn item_manifest_in_scope(
    selected_scope: bool,
    records: &[SelectedItemRecord],
    path: &str,
) -> bool {
    !selected_scope
        || records.iter().any(|record| {
            record
                .selection
                .value
                .get("item_manifest_ref")
                .and_then(Value::as_str)
                == Some(path)
        })
}

struct SelectedItemRecords {
    records: Vec<SelectedItemRecord>,
    read_bytes: u64,
    state_bytes: usize,
}

#[derive(Clone, Copy)]
enum SourceFoundationCurrentInput<'a> {
    Cut {
        cut: &'a CorpusCutReader,
        revision: SourceRevision,
    },
    Stream(&'a dyn SourceCutInput),
}

struct SourceFoundationCurrentMember {
    size_bytes: u64,
    raw: Vec<u8>,
}

impl SourceFoundationCurrentInput<'_> {
    fn selection(
        &self,
    ) -> Option<std::sync::Arc<crate::source_record_selection::SourceRecordSelection>> {
        self.input().and_then(SourceCutInput::record_selection)
    }
    fn semantic_member(&self, path: &str) -> Result<bool, ItemRefusal> {
        self.input()
            .map_or(Ok(true), |input| input.selects_required_member(path))
    }
    fn selected_record_member(&self, path: &str) -> Result<bool, ItemRefusal> {
        self.input()
            .map_or(Ok(true), |input| input.selects_semantic_member(path))
    }
    fn cut(&self) -> Option<&CorpusCutReader> {
        match self {
            Self::Cut { cut, .. } => Some(cut),
            Self::Stream(_) => None,
        }
    }

    fn input(&self) -> Option<&dyn SourceCutInput> {
        match self {
            Self::Cut { .. } => None,
            Self::Stream(input) => Some(*input),
        }
    }

    fn verify_current_fence(
        &self,
        coverage: &SourceCutInputCoverage,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        check_deadline(deadline, cancelled)?;
        if let Self::Stream(input) = self {
            input.verify_current_fence(coverage, deadline, cancelled)?;
        }
        check_deadline(deadline, cancelled)
    }

    fn for_each_member_meta(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        check_deadline(deadline, cancelled)?;
        match self {
            Self::Cut { cut, .. } => {
                for member in cut.current().members() {
                    check_deadline(deadline, cancelled)?;
                    visit(SourceCutMemberMeta {
                        path: member.path.as_str(),
                        size_bytes: member.size_bytes,
                    })?;
                }
                Ok(())
            }
            Self::Stream(input) => {
                input.for_each_current_member_meta(deadline, cancelled, visit)?;
                check_deadline(deadline, cancelled)
            }
        }
    }

    fn presence(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourcePresenceV1>, ItemRefusal> {
        check_deadline(deadline, cancelled)?;
        if let Some(selection) = self.selection() {
            if let Some((file, line)) = path.rsplit_once(':') {
                if file.ends_with(".jsonl") {
                    let selected = line
                        .parse::<u64>()
                        .ok()
                        .is_some_and(|line| selection.selected_row(file, line));
                    if !selected {
                        return Ok(None);
                    }
                    return self.presence(file, deadline, cancelled);
                }
            }
            if !selection.contains_member(path)
                && !selection.members().any(|m| {
                    m.source_ref
                        .strip_prefix(path)
                        .is_some_and(|tail| tail.starts_with('/'))
                })
            {
                return Ok(None);
            }
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source-foundation current path".into()))?;
        let found = match self {
            Self::Cut { cut, revision } => cut.presence(*revision, &relative),
            Self::Stream(input) => input.path_presence(relative.as_str(), deadline, cancelled)?,
        };
        check_deadline(deadline, cancelled)?;
        Ok(found)
    }

    fn member_size(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<u64>, ItemRefusal> {
        check_deadline(deadline, cancelled)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source-foundation current path".into()))?;
        let size = match self {
            Self::Cut { cut, .. } => cut
                .current()
                .member(&relative)
                .map(|member| member.size_bytes),
            Self::Stream(input) => {
                input.current_member_size(relative.as_str(), deadline, cancelled)?
            }
        };
        check_deadline(deadline, cancelled)?;
        Ok(size)
    }

    fn read_member(
        &self,
        path: &str,
        max_bytes: usize,
        prior_state_bytes: usize,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationCurrentMember, ItemRefusal> {
        check_deadline(deadline, cancelled)?;
        let relative_path_state_bytes = std::mem::size_of::<RelativePath>()
            .checked_add(path.len())
            .ok_or(crate::item_budget_origin!())?;
        let relative_path_peak = prior_state_bytes
            .checked_add(relative_path_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        if relative_path_peak > max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation current path parse state",
                used: Some(relative_path_peak as u64),
                limit: Some(max_state_bytes as u64),
            });
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source-foundation current path".into()))?;
        match self {
            Self::Cut { cut, revision } => {
                let metadata_size = cut
                    .current()
                    .member(&relative)
                    .ok_or_else(|| {
                        ItemRefusal::Source(format!(
                            "source-foundation current member missing from captured cut: {path}"
                        ))
                    })?
                    .size_bytes;
                let expected_len =
                    usize::try_from(metadata_size).map_err(|_| crate::item_budget_origin!())?;
                if metadata_size > max_bytes as u64 {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "source-foundation current member bytes",
                        used: Some(metadata_size),
                        limit: Some(max_bytes as u64),
                    });
                }
                let precharged = prior_state_bytes
                    .checked_add(relative_path_state_bytes)
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<u8>>()))
                    .and_then(|bytes| bytes.checked_add(expected_len))
                    .ok_or(crate::item_budget_origin!())?;
                if precharged > max_state_bytes {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "source-foundation current member read precharge",
                        used: Some(precharged as u64),
                        limit: Some(max_state_bytes as u64),
                    });
                }
                let member = cut
                    .read_member(*revision, &relative, max_bytes as u64, deadline, cancelled)
                    .map_err(store_error)?;
                let size_bytes =
                    u64::try_from(member.raw.len()).map_err(|_| crate::item_budget_origin!())?;
                let overlap = prior_state_bytes
                    .checked_add(relative_path_state_bytes)
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<u8>>()))
                    .and_then(|bytes| bytes.checked_add(member.raw.capacity()))
                    .ok_or(crate::item_budget_origin!())?;
                if size_bytes != metadata_size
                    || member.raw.len() > max_bytes
                    || overlap > max_state_bytes
                {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "source-foundation current member raw state",
                        used: Some(overlap as u64),
                        limit: Some(max_state_bytes as u64),
                    });
                }
                check_deadline(deadline, cancelled)?;
                Ok(SourceFoundationCurrentMember {
                    size_bytes,
                    raw: member.raw,
                })
            }
            Self::Stream(input) => {
                let mut found = None;
                input.with_current_member(
                    path,
                    max_bytes,
                    deadline,
                    cancelled,
                    &mut |meta, bytes| {
                        check_deadline(deadline, cancelled)?;
                        if found.is_some()
                            || meta.path != path
                            || bytes.len() > max_bytes
                            || u64::try_from(bytes.len()).ok() != Some(meta.size_bytes)
                        {
                            return Err(ItemRefusal::Source(
                                "source input returned an inexact current member".into(),
                            ));
                        }
                        let overlap = bytes
                            .len()
                            .checked_mul(2)
                            .and_then(|bytes| bytes.checked_add(prior_state_bytes))
                            .and_then(|used| used.checked_add(relative_path_state_bytes))
                            .and_then(|used| used.checked_add(std::mem::size_of::<Vec<u8>>()))
                            .and_then(|used| {
                                used.checked_add(std::mem::size_of::<
                                    Option<SourceFoundationCurrentMember>,
                                >())
                            })
                            .ok_or(crate::item_budget_origin!())?;
                        if overlap > max_state_bytes {
                            return Err(ItemRefusal::BudgetCheck {
                                check: "source-foundation current member copy state",
                                used: Some(overlap as u64),
                                limit: Some(max_state_bytes as u64),
                            });
                        }
                        if let Some(selection) = input.record_selection() {
                            if selection.contains_member(path) {
                                selection.verify_metadata_member(path, bytes)?;
                            } else if !input.selects_required_member(path)?
                                && !path.starts_with(SCHEMA_HOME)
                                && !path.starts_with("ToS/doctrine/semantic-interchange/")
                            {
                                return Err(ItemRefusal::Source(
                                    "Records member outside selected semantic closure".into(),
                                ));
                            }
                            if selection.record(path).is_some() {
                                let scratch =
                                    crate::source_record_selection::selection_state_upper_bound(
                                        bytes.len(),
                                    )?;
                                let peak = overlap
                                    .checked_add(scratch)
                                    .ok_or(crate::item_budget_origin!())?;
                                if peak > max_state_bytes {
                                    return Err(crate::item_budget_origin!());
                                }
                                selection.verify_record(path, bytes, deadline, cancelled)?;
                            }
                        }
                        let mut raw = Vec::new();
                        raw.try_reserve_exact(bytes.len())
                            .map_err(|_| crate::item_budget_origin!())?;
                        let actual_overlap = bytes
                            .len()
                            .checked_add(prior_state_bytes)
                            .and_then(|used| used.checked_add(relative_path_state_bytes))
                            .and_then(|used| used.checked_add(std::mem::size_of::<Vec<u8>>()))
                            .and_then(|used| {
                                used.checked_add(std::mem::size_of::<
                                    Option<SourceFoundationCurrentMember>,
                                >())
                            })
                            .and_then(|used| used.checked_add(raw.capacity()))
                            .ok_or(crate::item_budget_origin!())?;
                        if actual_overlap > max_state_bytes {
                            return Err(ItemRefusal::BudgetCheck {
                                check: "source-foundation current member copy capacity",
                                used: Some(actual_overlap as u64),
                                limit: Some(max_state_bytes as u64),
                            });
                        }
                        raw.extend_from_slice(bytes);
                        found = Some(SourceFoundationCurrentMember {
                            size_bytes: meta.size_bytes,
                            raw,
                        });
                        Ok(())
                    },
                )?;
                check_deadline(deadline, cancelled)?;
                found.ok_or_else(|| {
                    ItemRefusal::Source("source input omitted the requested current member".into())
                })
            }
        }
    }

    fn member_digest(
        &self,
        path: &str,
        max_bytes: usize,
        prior_state_bytes: usize,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Digest256>, ItemRefusal> {
        self.member_digest_with_accounting(
            path,
            max_bytes,
            None,
            prior_state_bytes,
            max_state_bytes,
            deadline,
            cancelled,
        )
    }

    fn member_digest_with_accounting(
        &self,
        path: &str,
        max_bytes: usize,
        direct_read: Option<(&mut u64, u64)>,
        prior_state_bytes: usize,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Digest256>, ItemRefusal> {
        let Some(expected_size) = self.member_size(path, deadline, cancelled)? else {
            return Ok(None);
        };
        if expected_size > max_bytes as u64 {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation digest member bytes",
                used: Some(expected_size),
                limit: Some(max_bytes as u64),
            });
        }
        if let Some((direct_read_bytes, direct_read_limit)) = direct_read {
            account_direct_item_read(
                expected_size,
                direct_read_bytes,
                direct_read_limit,
                max_bytes,
                "source-foundation inventory digest reads",
            )?;
        }
        let member = self.read_member(
            path,
            max_bytes,
            prior_state_bytes,
            max_state_bytes,
            deadline,
            cancelled,
        )?;
        if member.size_bytes != expected_size {
            return Err(ItemRefusal::Source(
                "source member size changed during digest read".into(),
            ));
        }
        check_deadline(deadline, cancelled)?;
        Ok(Some(Digest256::of_bytes(&member.raw)))
    }
}

struct DirectCurrentRecordScan<'a> {
    records: SelectedItemRecords,
    issues: DirectIssueBuffer<'a>,
    schema_checks: Vec<SourceFoundationRecordsSchemaCheck>,
    schema_request_state_bytes: usize,
    candidate_kernel_cause: Option<SourceFoundationRecordKernelCause>,
    record_schema_positions: Vec<(String, usize)>,
    current_records: BTreeMap<String, BiblioCurrentRecord>,
    current_record_order: Vec<String>,
    used_declared_profile_kinds: BTreeSet<String>,
    read_bytes: u64,
    state_bytes: usize,
}

#[derive(Clone)]
struct SourceFoundationRecordsStoreHandle<'a>(
    Rc<RefCell<&'a mut (dyn SourceFoundationRecordsStore + 'a)>>,
);

impl<'a> SourceFoundationRecordsStoreHandle<'a> {
    fn new(store: &'a mut dyn SourceFoundationRecordsStore) -> Self {
        Self(Rc::new(RefCell::new(store)))
    }

    fn with<T>(
        &self,
        action: impl FnOnce(&mut dyn SourceFoundationRecordsStore) -> Result<T, ItemRefusal>,
    ) -> Result<T, ItemRefusal> {
        let mut store = self.0.borrow_mut();
        action(&mut **store)
    }
}

struct DirectIssueBuffer<'a> {
    rows: Vec<SourceFoundationRecordsIssue>,
    state_bytes: usize,
    hard_max_state_bytes: usize,
    max_state_bytes: usize,
    max_issues: usize,
    store: Option<SourceFoundationRecordsStoreHandle<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyJsonParseFailure {
    Malformed,
    Nonfinite,
}

impl<'a> DirectIssueBuffer<'a> {
    fn new(
        limits: ItemLimits,
        state_limit: usize,
        store: Option<SourceFoundationRecordsStoreHandle<'a>>,
    ) -> Self {
        Self {
            rows: Vec::new(),
            state_bytes: std::mem::size_of::<Vec<SourceFoundationRecordsIssue>>(),
            hard_max_state_bytes: state_limit.min(limits.max_state_bytes),
            max_state_bytes: state_limit.min(limits.max_state_bytes),
            max_issues: limits.max_issues,
            store,
        }
    }

    fn push(
        &mut self,
        family: SourceFoundationRecordsIssueFamily,
        location: &str,
        message: &str,
    ) -> Result<(), ItemRefusal> {
        if self.rows.len() >= self.max_issues {
            return Err(crate::item_budget_origin!());
        }
        let retained = std::mem::size_of::<SourceFoundationRecordsIssue>()
            .checked_add(location.len())
            .and_then(|bytes| bytes.checked_add(message.len()))
            .ok_or(crate::item_budget_origin!())?;
        let temporary = location
            .len()
            .checked_add(message.len())
            .ok_or(crate::item_budget_origin!())?;
        self.state_bytes = self
            .state_bytes
            .checked_add(retained)
            .and_then(|used| used.checked_add(temporary))
            .filter(|used| *used <= self.max_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        self.state_bytes = self
            .state_bytes
            .checked_sub(temporary)
            .ok_or(crate::item_budget_origin!())?;
        let issue = SourceFoundationRecordsIssue {
            family,
            location: location.to_owned(),
            message: message.to_owned(),
        };
        if let Some(store) = &self.store {
            store.with(|store| store.ordered_issue(&issue))?;
        }
        self.rows.push(issue);
        Ok(())
    }

    fn current_record_first(
        &self,
        id: &str,
        record: &BiblioCurrentRecord,
    ) -> Result<(), ItemRefusal> {
        if let Some(store) = &self.store {
            store.with(|store| store.current_record_first(id, record))?;
        }
        Ok(())
    }

    fn item_record_selection(
        &self,
        row: &SourceFoundationItemRecordSelection,
    ) -> Result<(), ItemRefusal> {
        if let Some(store) = &self.store {
            store.with(|store| store.item_record_selection(row))?;
        }
        Ok(())
    }

    fn store_fact(
        &self,
        write: impl FnOnce(&mut dyn SourceFoundationRecordsStore) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        if let Some(store) = &self.store {
            store.with(write)?;
        }
        Ok(())
    }

    fn family_state_bytes(&self, family: SourceFoundationRecordsIssueFamily) -> usize {
        self.rows
            .iter()
            .filter(|issue| issue.family == family)
            .fold(
                std::mem::size_of::<Vec<SourceFoundationRecordsIssue>>(),
                |total, issue| {
                    total
                        .saturating_add(std::mem::size_of::<SourceFoundationRecordsIssue>())
                        .saturating_add(issue.location.len())
                        .saturating_add(issue.message.len())
                },
            )
    }
}

fn legacy_parse_failure(
    raw: &[u8],
    limits: ItemLimits,
    available: usize,
    cancelled: &AtomicBool,
) -> Result<LegacyJsonParseFailure, ItemRefusal> {
    check(limits, cancelled)?;
    let json_limits = tos_foundation::JsonLimits::new(
        limits.max_member_bytes,
        128,
        available.max(1),
        limits.max_member_bytes.max(1),
    )
    .map_err(|_| crate::item_budget_origin!())?;
    match tos_foundation::parse_json_with_state_budget(
        raw,
        tos_foundation::JsonMode::LegacyPythonObserved,
        json_limits,
        available.max(1),
    ) {
        Ok(document) => {
            fn contains_nonfinite(
                value: &tos_foundation::JsonValue,
                limits: ItemLimits,
                cancelled: &AtomicBool,
            ) -> Result<bool, ItemRefusal> {
                check(limits, cancelled)?;
                match value {
                    tos_foundation::JsonValue::Number(number) => Ok(number
                        .as_python_float()
                        .is_some_and(|value| !value.is_finite())),
                    tos_foundation::JsonValue::Array(values) => {
                        for value in values {
                            if contains_nonfinite(value, limits, cancelled)? {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    tos_foundation::JsonValue::Object(entries) => {
                        for (_, value) in entries {
                            if contains_nonfinite(value, limits, cancelled)? {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    _ => Ok(false),
                }
            }
            if contains_nonfinite(document.root(), limits, cancelled)?
                || raw_contains_nonfinite_legacy_number(raw, limits, cancelled)?
            {
                Ok(LegacyJsonParseFailure::Nonfinite)
            } else {
                Err(ItemRefusal::Unsupported(
                    "legacy JSON parser/serde representation disagreement".into(),
                ))
            }
        }
        Err(error) => match error.code {
            tos_foundation::FoundationErrorCode::InvalidUtf8
            | tos_foundation::FoundationErrorCode::InvalidJson
            | tos_foundation::FoundationErrorCode::InvalidUnicodeScalar
            | tos_foundation::FoundationErrorCode::InvalidNumber => {
                Ok(LegacyJsonParseFailure::Malformed)
            }
            tos_foundation::FoundationErrorCode::NonfiniteFloat => {
                Ok(LegacyJsonParseFailure::Nonfinite)
            }
            tos_foundation::FoundationErrorCode::BudgetExceeded => Err(ItemRefusal::BudgetCheck {
                check: "legacy JSON parse classification workspace",
                used: None,
                limit: Some(available as u64),
            }),
            _ => Err(ItemRefusal::Unsupported(
                "legacy JSON parser failure category is not mapped".into(),
            )),
        },
    }
}

/// The observed Python loader visits each value before a duplicate object key
/// replaces its prior value. Inspect the already-validated source spelling so
/// a discarded NaN/Infinity/overflowing float still explains why serde's
/// finite decoder refused the bytes, while strings containing those words do
/// not opt into the compatibility path.
fn raw_contains_nonfinite_legacy_number(
    raw: &[u8],
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<bool, ItemRefusal> {
    let mut cursor = 0usize;
    while cursor < raw.len() {
        if cursor & 0x0fff == 0 {
            check(limits, cancelled)?;
        }
        match raw[cursor] {
            b'"' => {
                cursor += 1;
                while cursor < raw.len() {
                    if cursor & 0x0fff == 0 {
                        check(limits, cancelled)?;
                    }
                    match raw[cursor] {
                        b'\\' => cursor = cursor.saturating_add(2),
                        b'"' => {
                            cursor += 1;
                            break;
                        }
                        _ => cursor += 1,
                    }
                }
            }
            b'N' if raw[cursor..].starts_with(b"NaN") => return Ok(true),
            b'I' if raw[cursor..].starts_with(b"Infinity") => return Ok(true),
            b'-' if raw[cursor..].starts_with(b"-Infinity") => return Ok(true),
            b'-' | b'0'..=b'9' => {
                let start = cursor;
                cursor += 1;
                while cursor < raw.len()
                    && matches!(raw[cursor], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
                {
                    if cursor & 0x0fff == 0 {
                        check(limits, cancelled)?;
                    }
                    cursor += 1;
                }
                let token = &raw[start..cursor];
                if token.iter().any(|byte| matches!(byte, b'.' | b'e' | b'E'))
                    && std::str::from_utf8(token)
                        .ok()
                        .and_then(|token| token.parse::<f64>().ok())
                        .is_some_and(|value| !value.is_finite())
                {
                    return Ok(true);
                }
            }
            _ => cursor += 1,
        }
    }
    check(limits, cancelled)?;
    Ok(false)
}

fn bounded_legacy_observed_inventory(
    raw: &[u8],
    limits: ItemLimits,
    available: usize,
    cancelled: &AtomicBool,
) -> Result<Option<tos_foundation::JsonValue>, ItemRefusal> {
    check(limits, cancelled)?;
    if available == 0 {
        return Err(crate::item_budget_origin!());
    }
    let json_limits = tos_foundation::JsonLimits::new(
        limits.max_member_bytes,
        128,
        available,
        limits.max_member_bytes.max(1),
    )
    .map_err(|_| crate::item_budget_origin!())?;
    match tos_foundation::parse_json_with_state_budget(
        raw,
        tos_foundation::JsonMode::LegacyPythonObserved,
        json_limits,
        available,
    ) {
        Ok(document) => {
            fn contains_nonfinite(
                value: &tos_foundation::JsonValue,
                limits: ItemLimits,
                cancelled: &AtomicBool,
            ) -> Result<bool, ItemRefusal> {
                check(limits, cancelled)?;
                match value {
                    tos_foundation::JsonValue::Number(number) => Ok(number
                        .as_python_float()
                        .is_some_and(|value| !value.is_finite())),
                    tos_foundation::JsonValue::Array(values) => {
                        for value in values {
                            if contains_nonfinite(value, limits, cancelled)? {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    tos_foundation::JsonValue::Object(entries) => {
                        for (_, value) in entries {
                            if contains_nonfinite(value, limits, cancelled)? {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    _ => Ok(false),
                }
            }
            check(limits, cancelled)?;
            if contains_nonfinite(document.root(), limits, cancelled)?
                || raw_contains_nonfinite_legacy_number(raw, limits, cancelled)?
            {
                Ok(Some(document.into_root()))
            } else {
                Err(ItemRefusal::Unsupported(
                    "legacy inventory representation disagreement without nonfinite value".into(),
                ))
            }
        }
        Err(error) => match error.code {
            tos_foundation::FoundationErrorCode::InvalidUtf8
            | tos_foundation::FoundationErrorCode::InvalidJson
            | tos_foundation::FoundationErrorCode::InvalidUnicodeScalar
            | tos_foundation::FoundationErrorCode::InvalidNumber => Ok(None),
            tos_foundation::FoundationErrorCode::BudgetExceeded => Err(ItemRefusal::BudgetCheck {
                check: "legacy inventory JSON parse workspace",
                used: None,
                limit: Some(available as u64),
            }),
            _ => Err(ItemRefusal::Unsupported(
                "legacy inventory parser failure category is not mapped".into(),
            )),
        },
    }
}

fn retain_direct_schema_check(
    checks: &mut Vec<SourceFoundationRecordsSchemaCheck>,
    state_bytes: &mut usize,
    state_limit: usize,
    before_issue: usize,
    family: SourceFoundationRecordsSchemaFamily,
    location: &str,
    contract: &str,
    decoded_instance: Option<Value>,
    owner_issue: Option<SourceFoundationRecordsOwnerIssue>,
) -> Result<(), ItemRefusal> {
    retain_direct_schema_check_input(
        checks,
        state_bytes,
        state_limit,
        before_issue,
        family,
        location,
        contract,
        decoded_instance,
        None,
        owner_issue,
    )
}

fn retain_direct_legacy_schema_check(
    checks: &mut Vec<SourceFoundationRecordsSchemaCheck>,
    state_bytes: &mut usize,
    state_limit: usize,
    before_issue: usize,
    family: SourceFoundationRecordsSchemaFamily,
    location: &str,
    contract: &str,
    legacy_raw_instance: &[u8],
    owner_issue: Option<SourceFoundationRecordsOwnerIssue>,
) -> Result<(), ItemRefusal> {
    retain_direct_schema_check_input(
        checks,
        state_bytes,
        state_limit,
        before_issue,
        family,
        location,
        contract,
        None,
        Some(legacy_raw_instance),
        owner_issue,
    )
}

fn retain_direct_schema_check_input(
    checks: &mut Vec<SourceFoundationRecordsSchemaCheck>,
    state_bytes: &mut usize,
    state_limit: usize,
    before_issue: usize,
    family: SourceFoundationRecordsSchemaFamily,
    location: &str,
    contract: &str,
    decoded_instance: Option<Value>,
    legacy_raw_instance: Option<&[u8]>,
    owner_issue: Option<SourceFoundationRecordsOwnerIssue>,
) -> Result<(), ItemRefusal> {
    if decoded_instance.is_some() && legacy_raw_instance.is_some()
        || decoded_instance.is_none() && legacy_raw_instance.is_none() && owner_issue.is_none()
    {
        return Err(ItemRefusal::Source(
            "source-foundation schema request has ambiguous instance representation".into(),
        ));
    }
    let decoded_bytes = decoded_instance
        .as_ref()
        .map(crate::record_biblio_cut::decoded_state)
        .transpose()?
        .unwrap_or_default();
    let raw_bytes = legacy_raw_instance.map_or(0, <[u8]>::len);
    let retained = std::mem::size_of::<SourceFoundationRecordsSchemaCheck>()
        .checked_add(location.len())
        .and_then(|bytes| bytes.checked_add(contract.len()))
        .and_then(|bytes| bytes.checked_add(decoded_bytes))
        .and_then(|bytes| bytes.checked_add(raw_bytes))
        .ok_or(crate::item_budget_origin!())?;
    let next_state = state_bytes
        .checked_add(retained)
        .filter(|used| *used <= state_limit)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "source-foundation decoded schema request state",
            used: state_bytes.checked_add(retained).map(|used| used as u64),
            limit: Some(state_limit as u64),
        })?;
    let check = SourceFoundationRecordsSchemaCheck {
        family,
        before_issue,
        location: location.to_owned(),
        contract: contract.to_owned(),
        decoded_instance,
        legacy_raw_instance: legacy_raw_instance.map(<[u8]>::to_vec),
        owner_issue,
    };
    let insertion =
        source_jsonl_check_insertion(checks, &check.location, check.family).unwrap_or(checks.len());
    checks.insert(insertion, check);
    *state_bytes = next_state;
    Ok(())
}

fn source_jsonl_check_insertion(
    checks: &[SourceFoundationRecordsSchemaCheck],
    location: &str,
    family: SourceFoundationRecordsSchemaFamily,
) -> Option<usize> {
    let (path, ordinal) = location.rsplit_once(':')?;
    if !path.ends_with(".jsonl") {
        return None;
    }
    let ordinal = ordinal.parse::<usize>().ok()?;
    checks.iter().position(|check| {
        if check.family != family {
            return false;
        }
        check
            .location
            .rsplit_once(':')
            .filter(|(previous_path, previous)| *previous_path == path && path.ends_with(".jsonl"))
            .and_then(|(_, previous)| previous.parse::<usize>().ok())
            .is_some_and(|previous| previous > ordinal)
    })
}

fn current_record_root_contract(path: &str) -> &'static str {
    if path.rsplit('/').next() == Some("link.json") {
        "ToS/contracts/source-link.schema.json"
    } else {
        "ToS/contracts/corpus-record.schema.json"
    }
}

fn note_record_kernel_candidate_cause(
    current: &mut Option<SourceFoundationRecordKernelCause>,
    incoming: SourceFoundationRecordKernelCause,
) {
    *current = Some(match *current {
        None => incoming,
        Some(existing) if existing == incoming => existing,
        Some(_) => SourceFoundationRecordKernelCause::MixedInvalidCandidates,
    });
}

fn is_candidate_record_kernel_refusal(
    refusal: &ItemRefusal,
    scan: &DirectCurrentRecordScan<'_>,
) -> bool {
    let ItemRefusal::Unsupported(reason) = refusal else {
        return false;
    };
    if reason.contains("code: \"record_json\"") {
        return scan.issues.rows.iter().any(|issue| {
            issue.family == SourceFoundationRecordsIssueFamily::Record
                && matches!(
                    issue.message.as_str(),
                    "cannot read JSON: invalid JSON" | "JSON root must be an object"
                )
        });
    }

    let reason_has_path = |expected_code: &str, expected_message: &str| {
        reason.contains(expected_code)
            && scan.issues.rows.iter().any(|issue| {
                issue.family == SourceFoundationRecordsIssueFamily::Record
                    && issue.message == expected_message
                    && reason.contains(&issue.location)
            })
    };
    reason_has_path(
        "code: \"native_record_json\"",
        "cannot read JSON: invalid JSON",
    ) || reason_has_path(
        "code: \"native_record_json\"",
        "JSON root must be an object",
    ) || reason_has_path(
        "code: \"native_schema_version\"",
        "JSON root must be an object",
    )
}

fn retain_record_schema_position(
    positions: &mut Vec<(String, usize)>,
    state_bytes: &mut usize,
    path: &str,
    before_issue: usize,
    state_limit: usize,
    store: Option<&SourceFoundationRecordsStoreHandle<'_>>,
) -> Result<(), ItemRefusal> {
    let retained = std::mem::size_of::<(String, usize)>()
        .checked_add(path.len())
        .ok_or(crate::item_budget_origin!())?;
    *state_bytes = state_bytes
        .checked_add(retained)
        .filter(|used| *used <= state_limit)
        .ok_or(crate::item_budget_origin!())?;
    positions.push((path.to_owned(), before_issue));
    if let Some(store) = store {
        store.with(|store| store.record_schema_position(path, before_issue))?;
    }
    Ok(())
}

fn current_source_paths_matching(
    source: SourceFoundationCurrentInput<'_>,
    prefix: &str,
    excluded_prefix: Option<&str>,
    basename: Option<&str>,
    suffix: Option<&str>,
    base_state_bytes: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(Vec<(String, u64)>, usize), ItemRefusal> {
    let mut paths = Vec::<(String, u64)>::new();
    let mut path_bytes = 0usize;
    source.for_each_member_meta(deadline, cancelled, &mut |meta| {
        let path = meta.path;
        if !source.semantic_member(path)?
            || basename.is_some() && !source.selected_record_member(path)?
            || !path.starts_with(prefix)
            || excluded_prefix.is_some_and(|excluded| path.starts_with(excluded))
            || basename.is_some_and(|name| path.rsplit('/').next() != Some(name))
            || suffix.is_some_and(|ending| !path.ends_with(ending))
        {
            return Ok(());
        }
        RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Source("source metadata contained an invalid path".into()))?;
        let path_payload_bytes = path.len();
        let next_len = paths
            .len()
            .checked_add(1)
            .ok_or(crate::item_budget_origin!())?;
        let requested_capacity = paths.capacity().max(next_len);
        let requested_slots = requested_capacity
            .checked_mul(std::mem::size_of::<(String, u64)>())
            .ok_or(crate::item_budget_origin!())?;
        let vector_overlap = if requested_capacity > paths.capacity() {
            paths
                .capacity()
                .checked_add(requested_capacity)
                .and_then(|slots| slots.checked_mul(std::mem::size_of::<(String, u64)>()))
                .ok_or(crate::item_budget_origin!())?
        } else {
            requested_slots
        };
        let allocation_peak = base_state_bytes
            .checked_add(std::mem::size_of::<Vec<(String, u64)>>())
            .and_then(|bytes| bytes.checked_add(vector_overlap))
            .and_then(|bytes| bytes.checked_add(path_bytes))
            .and_then(|bytes| bytes.checked_add(path_payload_bytes))
            .ok_or(crate::item_budget_origin!())?;
        if allocation_peak > max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation current path index allocation",
                used: Some(allocation_peak as u64),
                limit: Some(max_state_bytes as u64),
            });
        }
        paths
            .try_reserve_exact(1)
            .map_err(|_| crate::item_budget_origin!())?;
        let actual_slots = paths
            .capacity()
            .checked_mul(std::mem::size_of::<(String, u64)>())
            .ok_or(crate::item_budget_origin!())?;
        let actual_peak = base_state_bytes
            .checked_add(std::mem::size_of::<Vec<(String, u64)>>())
            .and_then(|bytes| bytes.checked_add(actual_slots))
            .and_then(|bytes| bytes.checked_add(path_bytes))
            .and_then(|bytes| bytes.checked_add(path_payload_bytes))
            .ok_or(crate::item_budget_origin!())?;
        if actual_peak > max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation current path index capacity",
                used: Some(actual_peak as u64),
                limit: Some(max_state_bytes as u64),
            });
        }
        let mut owned_path = String::new();
        owned_path
            .try_reserve_exact(path.len())
            .map_err(|_| crate::item_budget_origin!())?;
        owned_path.push_str(path);
        paths.push((owned_path, meta.size_bytes));
        path_bytes = path_bytes
            .checked_add(path_payload_bytes)
            .ok_or(crate::item_budget_origin!())?;
        Ok(())
    })?;
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    let retained = std::mem::size_of::<Vec<(String, u64)>>()
        .checked_add(
            paths
                .capacity()
                .checked_mul(std::mem::size_of::<(String, u64)>())
                .ok_or(crate::item_budget_origin!())?,
        )
        .and_then(|bytes| bytes.checked_add(path_bytes))
        .ok_or(crate::item_budget_origin!())?;
    let peak = base_state_bytes
        .checked_add(retained)
        .ok_or(crate::item_budget_origin!())?;
    if peak > max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation current path index state",
            used: Some(peak as u64),
            limit: Some(max_state_bytes as u64),
        });
    }
    Ok((paths, retained))
}

fn scan_direct_current_records<'a>(
    source: SourceFoundationCurrentInput<'_>,
    limits: ItemLimits,
    issue_limits: ItemLimits,
    issue_state_limit: usize,
    cancelled: &AtomicBool,
    store: Option<SourceFoundationRecordsStoreHandle<'a>>,
) -> Result<DirectCurrentRecordScan<'a>, ItemRefusal> {
    let registry_size = source
        .member_size(RECORD_REGISTRY, limits.deadline, cancelled)?
        .ok_or_else(|| {
            ItemRefusal::Source("source record registry absent from captured cut".into())
        })?;
    if registry_size > limits.max_member_bytes as u64 {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation record registry member bytes",
            used: Some(registry_size),
            limit: Some(limits.max_member_bytes as u64),
        });
    }
    let registry = source.read_member(
        RECORD_REGISTRY,
        limits.max_member_bytes,
        0,
        limits.max_state_bytes,
        limits.deadline,
        cancelled,
    )?;
    let registry_value =
        bounded_native_value(&registry.raw, limits, limits.max_state_bytes, cancelled)?;

    let mut basenames = LEGACY_RECORD_TYPES
        .into_iter()
        .zip(LEGACY_RECORD_BASENAMES)
        .map(|(record_type, basename)| (record_type.to_owned(), basename.to_owned()))
        .collect::<Vec<_>>();
    basenames.push(("link".to_owned(), "link.json".to_owned()));
    let mut declared_profile_kinds = BTreeSet::<String>::new();
    if let Some(types) = registry_value.get("types").and_then(Value::as_array) {
        for entry in types {
            let Some(profile) = entry.get("source_record_profile") else {
                continue;
            };
            let (Some(record_type), Some(basename)) = (
                profile.get("record_type").and_then(Value::as_str),
                profile.get("source_basename").and_then(Value::as_str),
            ) else {
                continue;
            };
            if let Some((_, existing_basename)) =
                basenames.iter_mut().find(|(kind, _)| kind == record_type)
            {
                *existing_basename = basename.to_owned();
            } else {
                basenames.push((record_type.to_owned(), basename.to_owned()));
            }
            declared_profile_kinds.insert(record_type.to_owned());
        }
    }
    let declared_profile_kinds_state_bytes = declared_profile_kinds.iter().try_fold(
        std::mem::size_of::<BTreeSet<String>>(),
        |total, kind| {
            total
                .checked_add(kind.len())
                .and_then(|bytes| {
                    bytes.checked_add(
                        std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>(),
                    )
                })
                .ok_or(crate::item_budget_origin!())
        },
    )?;
    let mut state_bytes = crate::record_biblio_cut::decoded_state(&registry_value)?
        .checked_add(
            basenames
                .iter()
                .map(|(record_type, basename)| record_type.len() + basename.len())
                .sum::<usize>(),
        )
        .and_then(|bytes| bytes.checked_add(declared_profile_kinds_state_bytes))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<(String, String)>>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<String>>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<(String, usize)>>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<SelectedItemRecord>>()))
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<BTreeMap<String, BiblioCurrentRecord>>())
        })
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<BTreeMap<String, String>>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<BTreeMap<String, usize>>()))
        .ok_or(crate::item_budget_origin!())?;
    if state_bytes > limits.max_state_bytes {
        return Err(crate::item_budget_origin!());
    }
    let registry_bytes = registry.size_bytes;
    drop(registry_value);
    drop(registry);

    let mut read_bytes = registry_bytes;
    let mut issues = DirectIssueBuffer::new(issue_limits, issue_state_limit, store.clone());
    let mut schema_checks = Vec::<SourceFoundationRecordsSchemaCheck>::new();
    let mut schema_request_state_bytes =
        std::mem::size_of::<Vec<SourceFoundationRecordsSchemaCheck>>();
    let mut record_schema_positions = Vec::<(String, usize)>::new();
    retain_record_schema_position(
        &mut record_schema_positions,
        &mut state_bytes,
        RECORD_REGISTRY,
        issues.rows.len(),
        limits.max_state_bytes,
        store.as_ref(),
    )?;
    let mut id_owners = BTreeMap::<String, BiblioCurrentRecord>::new();
    let mut id_order = Vec::<String>::new();
    let mut uri_owners = BTreeMap::<String, String>::new();
    let mut selected_items = Vec::<SelectedItemRecord>::new();
    let mut selected_positions = BTreeMap::<String, usize>::new();
    let mut candidate_kernel_cause = None;
    let mut selected_bytes = 0u64;
    let mut selected_state_bytes = std::mem::size_of::<Vec<SelectedItemRecord>>()
        .checked_add(std::mem::size_of::<BTreeMap<String, usize>>())
        .ok_or(crate::item_budget_origin!())?;

    for (record_type, basename) in basenames {
        if record_type == "link" {
            continue;
        }
        let (paths, paths_state_bytes) = current_source_paths_matching(
            source,
            SOURCE_HOME,
            Some(CATALOG_HOME),
            Some(&basename),
            None,
            state_bytes
                .checked_add(selected_state_bytes)
                .ok_or(crate::item_budget_origin!())?,
            limits.max_state_bytes,
            limits.deadline,
            cancelled,
        )?;
        for (path, member_size) in paths {
            check(limits, cancelled)?;
            if member_size > limits.max_member_bytes as u64 {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation direct record member bytes",
                    used: Some(member_size),
                    limit: Some(limits.max_member_bytes as u64),
                });
            }
            read_bytes = read_bytes
                .checked_add(member_size)
                .filter(|used| *used <= limits.max_total_bytes)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation direct record reads",
                    used: read_bytes.checked_add(member_size),
                    limit: Some(limits.max_total_bytes),
                })?;
            let member = source.read_member(
                &path,
                limits.max_member_bytes,
                state_bytes
                    .checked_add(selected_state_bytes)
                    .and_then(|bytes| bytes.checked_add(paths_state_bytes))
                    .ok_or(crate::item_budget_origin!())?,
                limits.max_state_bytes,
                limits.deadline,
                cancelled,
            )?;
            if member.size_bytes != member_size {
                return Err(ItemRefusal::Source(
                    "direct record member size changed after metadata traversal".into(),
                ));
            }
            let available = limits
                .max_state_bytes
                .checked_sub(state_bytes)
                .and_then(|remaining| remaining.checked_sub(selected_state_bytes))
                .and_then(|remaining| remaining.checked_sub(paths_state_bytes))
                .ok_or(crate::item_budget_origin!())?;
            let value = match bounded_native_value(&member.raw, limits, available, cancelled) {
                Ok(value) => value,
                Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                    match legacy_parse_failure(&member.raw, limits, available, cancelled)? {
                        LegacyJsonParseFailure::Malformed => {
                            issues.push(
                                SourceFoundationRecordsIssueFamily::Record,
                                &path,
                                "cannot read JSON: invalid JSON",
                            )?;
                            issues
                                .state_bytes
                                .checked_add(schema_request_state_bytes)
                                .filter(|used| *used <= issue_state_limit)
                                .ok_or(crate::item_budget_origin!())?;
                            note_record_kernel_candidate_cause(
                                &mut candidate_kernel_cause,
                                SourceFoundationRecordKernelCause::MalformedJson,
                            );
                            continue;
                        }
                        LegacyJsonParseFailure::Nonfinite => {
                            return Err(ItemRefusal::Unsupported(
                                "source-foundation nonfinite legacy JSON cannot be represented by serde_json::Value".into(),
                            ));
                        }
                    }
                }
                Err(error) => return Err(error),
            };
            let Some(object) = value.as_object() else {
                let available_schema_state = issue_state_limit
                    .checked_sub(issues.state_bytes)
                    .ok_or(crate::item_budget_origin!())?;
                let before_issue = issues.rows.len();
                retain_direct_schema_check(
                    &mut schema_checks,
                    &mut schema_request_state_bytes,
                    available_schema_state,
                    before_issue,
                    SourceFoundationRecordsSchemaFamily::Record,
                    &path,
                    current_record_root_contract(&path),
                    Some(value),
                    None,
                )?;
                issues.push(
                    SourceFoundationRecordsIssueFamily::Record,
                    &path,
                    "JSON root must be an object",
                )?;
                note_record_kernel_candidate_cause(
                    &mut candidate_kernel_cause,
                    SourceFoundationRecordKernelCause::NonObjectRoot,
                );
                continue;
            };
            retain_record_schema_position(
                &mut record_schema_positions,
                &mut state_bytes,
                &path,
                issues.rows.len(),
                limits.max_state_bytes,
                store.as_ref(),
            )?;
            append_direct_source_refs(
                &mut issues,
                SourceFoundationRecordsIssueFamily::Record,
                &path,
                &value,
                source,
                limits.deadline,
                cancelled,
            )?;
            let record_id = object.get("record_id").and_then(Value::as_str);
            if let Some(record_id) = record_id {
                if id_owners.contains_key(record_id) {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Record,
                        &path,
                        &format!("duplicate record_id: {record_id}"),
                    )?;
                } else {
                    let record_type = object.get("record_type").unwrap_or(&Value::Null);
                    let kind_bytes = python_value_string_len(record_type)?;
                    let retained = crate::record_biblio_cut::decoded_state(&value)?
                        .checked_add(record_id.len())
                        .and_then(|bytes| bytes.checked_add(path.len()))
                        .and_then(|bytes| bytes.checked_add(record_id.len()))
                        .and_then(|bytes| bytes.checked_add(kind_bytes))
                        .and_then(|bytes| {
                            bytes.checked_add(
                                std::mem::size_of::<BiblioCurrentRecord>()
                                    + std::mem::size_of::<(String, usize)>()
                                    + 3 * std::mem::size_of::<usize>(),
                            )
                        })
                        .ok_or(crate::item_budget_origin!())?;
                    state_bytes = state_bytes
                        .checked_add(retained)
                        .filter(|used| *used <= limits.max_state_bytes)
                        .ok_or(crate::item_budget_origin!())?;
                    let current = BiblioCurrentRecord {
                        path: path.clone(),
                        kind: python_value_string(record_type),
                        value: value.clone(),
                    };
                    issues.current_record_first(record_id, &current)?;
                    id_owners.insert(record_id.to_owned(), current);
                    id_order.push(record_id.to_owned());
                }
                if object.get("record_type").and_then(Value::as_str) == Some("item") {
                    let value_bytes = crate::record_biblio_cut::decoded_state(&value)?;
                    if let Some(index) = selected_positions.get(record_id).copied() {
                        let previous = &selected_items[index].selection;
                        let old_bytes = crate::record_biblio_cut::decoded_state(&previous.value)?
                            .checked_add(previous.path.len())
                            .ok_or(crate::item_budget_origin!())?;
                        selected_state_bytes = selected_state_bytes
                            .checked_sub(old_bytes)
                            .and_then(|used| used.checked_add(value_bytes))
                            .and_then(|used| used.checked_add(path.len()))
                            .filter(|used| *used <= limits.max_state_bytes)
                            .ok_or(crate::item_budget_origin!())?;
                        selected_items[index].selection.path = path.clone();
                        selected_items[index].selection.value = value.clone();
                    } else {
                        let retained = std::mem::size_of::<SelectedItemRecord>()
                            .checked_add(record_id.len())
                            .and_then(|bytes| bytes.checked_add(path.len()))
                            .and_then(|bytes| bytes.checked_add(value_bytes))
                            .and_then(|bytes| {
                                bytes.checked_add(
                                    std::mem::size_of::<(String, usize)>()
                                        + std::mem::size_of::<String>()
                                        + 3 * std::mem::size_of::<usize>(),
                                )
                            })
                            .ok_or(crate::item_budget_origin!())?;
                        selected_state_bytes = selected_state_bytes
                            .checked_add(retained)
                            .filter(|used| *used <= limits.max_state_bytes)
                            .ok_or(crate::item_budget_origin!())?;
                        selected_positions.insert(record_id.to_owned(), selected_items.len());
                        selected_items.push(SelectedItemRecord {
                            selection: SourceFoundationItemRecordSelection {
                                record_id: record_id.to_owned(),
                                path: path.clone(),
                                value: value.clone(),
                            },
                        });
                    }
                    let index = selected_positions
                        .get(record_id)
                        .copied()
                        .ok_or_else(|| ItemRefusal::Source("selected Item index drift".into()))?;
                    issues.item_record_selection(&selected_items[index].selection)?;
                    selected_bytes = selected_bytes
                        .checked_add(member_size)
                        .ok_or(crate::item_budget_origin!())?;
                }
            }
            state_bytes = state_bytes
                .checked_add(path.len())
                .filter(|used| *used <= limits.max_state_bytes)
                .ok_or(crate::item_budget_origin!())?;
            if state_bytes
                .checked_add(selected_state_bytes)
                .and_then(|used| used.checked_add(paths_state_bytes))
                .is_none_or(|used| used > limits.max_state_bytes)
            {
                return Err(crate::item_budget_origin!());
            }
            drop(value);
        }
    }

    let (link_paths, link_paths_state_bytes) = current_source_paths_matching(
        source,
        SOURCE_HOME,
        Some(CATALOG_HOME),
        None,
        Some("/link.json"),
        state_bytes
            .checked_add(selected_state_bytes)
            .ok_or(crate::item_budget_origin!())?,
        limits.max_state_bytes,
        limits.deadline,
        cancelled,
    )?;
    for (path, member_size) in link_paths {
        check(limits, cancelled)?;
        read_bytes = read_bytes
            .checked_add(member_size)
            .filter(|used| *used <= limits.max_total_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation direct Link reads",
                used: read_bytes.checked_add(member_size),
                limit: Some(limits.max_total_bytes),
            })?;
        let member = source.read_member(
            &path,
            limits.max_member_bytes,
            state_bytes
                .checked_add(selected_state_bytes)
                .and_then(|bytes| bytes.checked_add(link_paths_state_bytes))
                .ok_or(crate::item_budget_origin!())?,
            limits.max_state_bytes,
            limits.deadline,
            cancelled,
        )?;
        if member.size_bytes != member_size {
            return Err(ItemRefusal::Source(
                "direct Link member size changed after metadata traversal".into(),
            ));
        }
        let available = limits
            .max_state_bytes
            .checked_sub(state_bytes)
            .and_then(|remaining| remaining.checked_sub(selected_state_bytes))
            .and_then(|remaining| remaining.checked_sub(link_paths_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        let value = match bounded_native_value(&member.raw, limits, available, cancelled) {
            Ok(value) => value,
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                match legacy_parse_failure(&member.raw, limits, available, cancelled)? {
                    LegacyJsonParseFailure::Malformed => {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Record,
                            &path,
                            "cannot read JSON: invalid JSON",
                        )?;
                        issues
                            .state_bytes
                            .checked_add(schema_request_state_bytes)
                            .filter(|used| *used <= issue_state_limit)
                            .ok_or(crate::item_budget_origin!())?;
                        note_record_kernel_candidate_cause(
                            &mut candidate_kernel_cause,
                            SourceFoundationRecordKernelCause::MalformedJson,
                        );
                        continue;
                    }
                    LegacyJsonParseFailure::Nonfinite => {
                        return Err(ItemRefusal::Unsupported(
                            "source-foundation nonfinite legacy JSON cannot be represented by serde_json::Value".into(),
                        ));
                    }
                }
            }
            Err(error) => return Err(error),
        };
        let Some(object) = value.as_object() else {
            let available_schema_state = issue_state_limit
                .checked_sub(issues.state_bytes)
                .ok_or(crate::item_budget_origin!())?;
            let before_issue = issues.rows.len();
            retain_direct_schema_check(
                &mut schema_checks,
                &mut schema_request_state_bytes,
                available_schema_state,
                before_issue,
                SourceFoundationRecordsSchemaFamily::Record,
                &path,
                current_record_root_contract(&path),
                Some(value),
                None,
            )?;
            issues.push(
                SourceFoundationRecordsIssueFamily::Record,
                &path,
                "JSON root must be an object",
            )?;
            note_record_kernel_candidate_cause(
                &mut candidate_kernel_cause,
                SourceFoundationRecordKernelCause::NonObjectRoot,
            );
            continue;
        };
        retain_record_schema_position(
            &mut record_schema_positions,
            &mut state_bytes,
            &path,
            issues.rows.len(),
            limits.max_state_bytes,
            store.as_ref(),
        )?;
        append_direct_source_refs(
            &mut issues,
            SourceFoundationRecordsIssueFamily::Record,
            &path,
            &value,
            source,
            limits.deadline,
            cancelled,
        )?;
        if let Some(observation) = object.get("observation_ref").and_then(Value::as_str) {
            if observation.starts_with("ToS/") {
                if source.presence(observation, limits.deadline, cancelled)?
                    != Some(SourcePresenceV1::File)
                {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Record,
                        &path,
                        &format!("unresolved Link observation_ref: {observation}"),
                    )?;
                }
            }
        }
        let record_id = object.get("record_id").and_then(Value::as_str);
        let uri = object.get("uri").and_then(Value::as_str);
        if let Some(record_id) = record_id {
            if id_owners.contains_key(record_id) {
                issues.push(
                    SourceFoundationRecordsIssueFamily::Record,
                    &path,
                    &format!("duplicate record_id: {record_id}"),
                )?;
            } else {
                let record_type = object.get("record_type").unwrap_or(&Value::Null);
                let kind_bytes = python_value_string_len(record_type)?;
                let retained = crate::record_biblio_cut::decoded_state(&value)?
                    .checked_add(record_id.len())
                    .and_then(|bytes| bytes.checked_add(path.len()))
                    .and_then(|bytes| bytes.checked_add(record_id.len()))
                    .and_then(|bytes| bytes.checked_add(kind_bytes))
                    .and_then(|bytes| {
                        bytes.checked_add(
                            std::mem::size_of::<BiblioCurrentRecord>()
                                + std::mem::size_of::<(String, usize)>()
                                + 3 * std::mem::size_of::<usize>(),
                        )
                    })
                    .ok_or(crate::item_budget_origin!())?;
                state_bytes = state_bytes
                    .checked_add(retained)
                    .filter(|used| *used <= limits.max_state_bytes)
                    .ok_or(crate::item_budget_origin!())?;
                let current = BiblioCurrentRecord {
                    path: path.clone(),
                    kind: python_value_string(record_type),
                    value: value.clone(),
                };
                issues.current_record_first(record_id, &current)?;
                id_owners.insert(record_id.to_owned(), current);
                id_order.push(record_id.to_owned());
            }
        }
        if let Some(uri) = uri {
            if let Some(previous) = uri_owners.get(uri) {
                issues.push(
                    SourceFoundationRecordsIssueFamily::Record,
                    &path,
                    &format!("duplicate Link uri; first used by {previous}"),
                )?;
            } else if let Some(record_id) = record_id {
                uri_owners.insert(uri.to_owned(), record_id.to_owned());
                state_bytes = state_bytes
                    .checked_add(uri.len())
                    .and_then(|bytes| bytes.checked_add(record_id.len()))
                    .and_then(|bytes| {
                        bytes.checked_add(
                            std::mem::size_of::<(String, String)>()
                                + 3 * std::mem::size_of::<usize>(),
                        )
                    })
                    .filter(|used| *used <= limits.max_state_bytes)
                    .ok_or(crate::item_budget_origin!())?;
            }
        }
        state_bytes = state_bytes
            .checked_add(path.len())
            .filter(|used| *used <= limits.max_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
    }

    // The explicit semantic selection is complete independently of Item
    // companion traversal: every selected source slot earns a real schema
    // request using its declared owned version and its exact physical row.
    if let Some(selection) = source.selection() {
        let mut previous_path = None::<&str>;
        let mut verified_count = 0usize;
        for slot_source in selection.slots() {
            let path = slot_source.source.source_ref.as_str();
            if previous_path == Some(path) {
                continue;
            }
            previous_path = Some(path);
            let size = source
                .member_size(path, limits.deadline, cancelled)?
                .ok_or_else(|| ItemRefusal::Source("selected slot member missing".into()))?;
            read_bytes = read_bytes
                .checked_add(size)
                .filter(|n| *n <= limits.max_total_bytes)
                .ok_or(crate::item_budget_origin!())?;
            let prior = state_bytes
                .checked_add(selected_state_bytes)
                .and_then(|n| n.checked_add(issues.state_bytes))
                .ok_or(crate::item_budget_origin!())?;
            let member = source.read_member(
                path,
                limits.max_member_bytes,
                prior
                    .checked_add(schema_request_state_bytes)
                    .ok_or(crate::item_budget_origin!())?,
                limits.max_state_bytes,
                limits.deadline,
                cancelled,
            )?;
            let verified = selection.verify_file(path, &member.raw, limits.deadline, cancelled)?;
            let mut cursor = verified.row_cursor();
            let scratch = selection
                .file_slots(path)
                .iter()
                .map(|slot| slot.verification_state_upper_bound())
                .try_fold(0usize, |max, n| n.map(|n| max.max(n)))?;
            loop {
                // Verification scratch is transient but must fit alongside
                // the authenticated whole file and already retained requests.
                let available = limits
                    .max_state_bytes
                    .checked_sub(prior)
                    .and_then(|n| n.checked_sub(member.raw.len()))
                    .and_then(|n| n.checked_sub(schema_request_state_bytes))
                    .ok_or(crate::item_budget_origin!())?;
                if scratch > available {
                    return Err(crate::item_budget_origin!());
                }
                let Some(row) = cursor.next_checked(limits.deadline, cancelled) else {
                    break;
                };
                let (line, raw, slot) = row?;
                let value = bounded_native_value(raw, limits, available, cancelled)?;
                let version = value.get("schema_version").and_then(Value::as_str);
                let contract = match (slot.kind.as_str(), version) {
                    ("claim", Some(_)) => "ToS/contracts/source-claim-record.schema.json",
                    ("provenance_event", Some("tos_provenance_event_v1")) => {
                        "ToS/contracts/provenance-event.schema.json"
                    }
                    ("provenance_event", Some("tos_provenance_event_v2")) => {
                        "ToS/contracts/provenance-event-v2.schema.json"
                    }
                    ("anchor", Some("tos_source_anchor_v1")) => {
                        "ToS/contracts/source-anchor.schema.json"
                    }
                    _ => {
                        return Err(ItemRefusal::Unsupported(
                            "selected source slot owned schema version".into(),
                        ));
                    }
                };
                let location = format!("{path}:{line}");
                if schema_checks.len() >= limits.max_issues {
                    return Err(crate::item_budget_origin!());
                }
                retain_direct_schema_check(
                    &mut schema_checks,
                    &mut schema_request_state_bytes,
                    issue_state_limit
                        .checked_sub(issues.state_bytes)
                        .ok_or(crate::item_budget_origin!())?,
                    issues.rows.len(),
                    SourceFoundationRecordsSchemaFamily::Record,
                    &location,
                    contract,
                    Some(value),
                    None,
                )?;
                verified_count = verified_count
                    .checked_add(1)
                    .ok_or(crate::item_budget_origin!())?;
            }
        }
        if verified_count != selection.slot_count() {
            return Err(ItemRefusal::Source(
                "selected source slot execution census".into(),
            ));
        }
    }

    for record_id in &id_order {
        check(limits, cancelled)?;
        let owner = id_owners
            .get(record_id)
            .ok_or_else(|| ItemRefusal::Source("direct record owner index drift".into()))?;
        let record_type = owner.value.get("record_type");
        let require = |reference: &Value,
                       expected: &str,
                       issues: &mut DirectIssueBuffer<'_>|
         -> Result<(), ItemRefusal> {
            let Some(reference) = reference.as_str() else {
                return Ok(());
            };
            let Some(target) = id_owners.get(reference) else {
                return issues.push(
                    SourceFoundationRecordsIssueFamily::Record,
                    &owner.path,
                    &format!("unresolved {expected} reference: {reference}"),
                );
            };
            if target.value.get("record_type").and_then(Value::as_str) != Some(expected) {
                return issues.push(
                    SourceFoundationRecordsIssueFamily::Record,
                    &owner.path,
                    &format!(
                        "{reference} resolves to {}, expected {expected}",
                        target
                            .value
                            .get("record_type")
                            .map(python_value_string)
                            .unwrap_or_else(|| "None".into())
                    ),
                );
            }
            Ok(())
        };
        match record_type.and_then(Value::as_str) {
            Some("expression") => {
                require(
                    owner.value.get("work_ref").unwrap_or(&Value::Null),
                    "work",
                    &mut issues,
                )?;
            }
            Some("edition") => {
                if let Some(references) = owner
                    .value
                    .get("embodies_expression_refs")
                    .and_then(Value::as_array)
                {
                    for reference in references {
                        require(reference, "expression", &mut issues)?;
                    }
                }
                if let Some(reference) = owner.value.get("collection_ref") {
                    require(reference, "collection", &mut issues)?;
                }
            }
            _ => {}
        }
    }
    let mut used_declared_profile_kinds_state_bytes = std::mem::size_of::<BTreeSet<String>>();
    for kind in &declared_profile_kinds {
        check(limits, cancelled)?;
        let mut is_used = false;
        for owner in id_owners.values() {
            check(limits, cancelled)?;
            if owner.value.get("record_type").and_then(Value::as_str) == Some(kind.as_str()) {
                is_used = true;
                break;
            }
        }
        if is_used {
            used_declared_profile_kinds_state_bytes = used_declared_profile_kinds_state_bytes
                .checked_add(kind.len())
                .and_then(|bytes| {
                    bytes.checked_add(
                        std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>(),
                    )
                })
                .ok_or(crate::item_budget_origin!())?;
        }
    }
    state_bytes = state_bytes
        .checked_add(used_declared_profile_kinds_state_bytes)
        .and_then(|bytes| bytes.checked_add(selected_state_bytes))
        .filter(|used| *used <= limits.max_state_bytes)
        .ok_or(crate::item_budget_origin!())?;
    let mut used_declared_profile_kinds = BTreeSet::<String>::new();
    for kind in &declared_profile_kinds {
        check(limits, cancelled)?;
        let mut is_used = false;
        for owner in id_owners.values() {
            check(limits, cancelled)?;
            if owner.value.get("record_type").and_then(Value::as_str) == Some(kind.as_str()) {
                is_used = true;
                break;
            }
        }
        if is_used {
            used_declared_profile_kinds.insert(kind.clone());
            issues.store_fact(|store| store.used_profile_kind(kind))?;
        }
    }
    Ok(DirectCurrentRecordScan {
        records: SelectedItemRecords {
            records: selected_items,
            read_bytes: selected_bytes,
            state_bytes: selected_state_bytes,
        },
        issues,
        schema_checks,
        schema_request_state_bytes,
        candidate_kernel_cause,
        record_schema_positions,
        current_records: id_owners,
        current_record_order: id_order,
        used_declared_profile_kinds,
        read_bytes,
        state_bytes,
    })
}

fn append_direct_source_refs(
    issues: &mut DirectIssueBuffer<'_>,
    family: SourceFoundationRecordsIssueFamily,
    location: &str,
    value: &Value,
    source: SourceFoundationCurrentInput<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    append_direct_source_refs_fields(
        issues,
        family,
        location,
        |field| value.get(field),
        source,
        deadline,
        cancelled,
    )
}

fn append_direct_source_refs_observed(
    issues: &mut DirectIssueBuffer<'_>,
    family: SourceFoundationRecordsIssueFamily,
    location: &str,
    value: &tos_foundation::JsonValue,
    source: SourceFoundationCurrentInput<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    for field in ["source_refs", "source_record_refs", "receipt_refs"] {
        if let Some(tos_foundation::JsonValue::Array(references)) = value.object_get(field) {
            for reference in references {
                append_direct_repo_ref_observed(
                    issues, family, location, reference, source, deadline, cancelled,
                )?;
            }
        }
    }
    for field in [
        "rights_ref",
        "provenance_ref",
        "forensic_report_ref",
        "resource_inventory_ref",
        "generated_from_manifest_ref",
        "item_manifest_ref",
    ] {
        if let Some(reference) = value.object_get(field) {
            append_direct_repo_ref_observed(
                issues, family, location, reference, source, deadline, cancelled,
            )?;
        }
    }
    Ok(())
}

fn append_direct_repo_ref_observed(
    issues: &mut DirectIssueBuffer<'_>,
    family: SourceFoundationRecordsIssueFamily,
    location: &str,
    reference: &tos_foundation::JsonValue,
    source: SourceFoundationCurrentInput<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    let exists = match reference {
        tos_foundation::JsonValue::String(reference) => {
            let has_tos_prefix = reference
                .units()
                .starts_with(&['T' as u16, 'o' as u16, 'S' as u16, '/' as u16]);
            if !has_tos_prefix {
                true
            } else {
                let reference = reference.as_str().ok_or_else(|| {
                    ItemRefusal::Unsupported(
                        "legacy repository reference contains a lone surrogate".into(),
                    )
                })?;
                let path = RelativePath::parse(reference).map_err(|_| {
                    ItemRefusal::Unsupported("source-foundation repository reference path".into())
                })?;
                source
                    .presence(path.as_str(), deadline, cancelled)?
                    .is_some()
            }
        }
        _ => false,
    };
    if !exists {
        let rendered = legacy_python_display(reference)?;
        issues.push(
            family,
            location,
            &format!("repository reference does not exist: {rendered}"),
        )?;
    }
    Ok(())
}

fn append_direct_legacy_inventory_issues(
    issues: &mut DirectIssueBuffer<'_>,
    location: &str,
    manifest_location: &str,
    manifest: &Value,
    inventory: &tos_foundation::JsonValue,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    inventory_set_scan_steps: &mut usize,
) -> Result<(), ItemRefusal> {
    if !crate::item_rules::legacy_json_native_equal(
        inventory
            .object_get("item_id")
            .unwrap_or(&tos_foundation::JsonValue::Null),
        manifest.get("item_id").unwrap_or(&Value::Null),
    ) {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            location,
            "resource inventory item_id differs from manifest",
        )?;
    }
    let expected_manifest_ref = Value::String(manifest_location.to_owned());
    if !crate::item_rules::legacy_json_native_equal(
        inventory
            .object_get("generated_from_manifest_ref")
            .unwrap_or(&tos_foundation::JsonValue::Null),
        &expected_manifest_ref,
    ) {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            location,
            "resource inventory does not cite its current item manifest",
        )?;
    }

    let expected_files = manifest.get("payload_files").and_then(Value::as_array);
    if manifest.get("payload_files").is_some_and(|value| {
        !matches!(value, Value::Array(_) | Value::String(_) | Value::Object(_))
    }) {
        return Err(ItemRefusal::Unsupported(
            "maintained inventory owner iterates a non-iterable manifest field".into(),
        ));
    }
    let actual_files = inventory
        .object_get("files")
        .and_then(tos_foundation::JsonValue::as_array);
    if inventory.object_get("files").is_some_and(|value| {
        !matches!(
            value,
            tos_foundation::JsonValue::Array(_)
                | tos_foundation::JsonValue::String(_)
                | tos_foundation::JsonValue::Object(_)
        )
    }) {
        return Err(ItemRefusal::Unsupported(
            "maintained inventory owner iterates a non-iterable JSON value".into(),
        ));
    }
    let mut expected = expected_files
        .into_iter()
        .flatten()
        .filter(|entry| entry.is_object());
    let mut actual = actual_files
        .into_iter()
        .flatten()
        .filter(|entry| entry.as_object().is_some());
    let same_files = loop {
        check(limits, cancelled)?;
        match (actual.next(), expected.next()) {
            (None, None) => break true,
            (Some(actual), Some(expected))
                if crate::item_rules::legacy_json_native_equal(
                    actual
                        .object_get("file_id")
                        .unwrap_or(&tos_foundation::JsonValue::Null),
                    expected.get("file_id").unwrap_or(&Value::Null),
                ) && crate::item_rules::legacy_json_native_equal(
                    actual
                        .object_get("file_sha256")
                        .unwrap_or(&tos_foundation::JsonValue::Null),
                    expected.get("sha256").unwrap_or(&Value::Null),
                ) && crate::item_rules::legacy_json_native_equal(
                    actual
                        .object_get("media_type")
                        .unwrap_or(&tos_foundation::JsonValue::Null),
                    expected.get("media_type").unwrap_or(&Value::Null),
                ) => {}
            _ => break false,
        }
    };
    if !same_files {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            location,
            "resource inventory file identity differs from manifest payload files",
        )?;
    }

    for entry in actual_files.into_iter().flatten() {
        check(limits, cancelled)?;
        let Some(_) = entry.as_object() else {
            continue;
        };
        let resources = entry.object_get("resources");
        if resources.is_some_and(|value| {
            !matches!(
                value,
                tos_foundation::JsonValue::Array(_)
                    | tos_foundation::JsonValue::String(_)
                    | tos_foundation::JsonValue::Object(_)
            )
        }) {
            return Err(ItemRefusal::Unsupported(
                "maintained inventory owner iterates a non-iterable resources value".into(),
            ));
        }
        let resource_count = python_legacy_iter_len(resources)?;
        let resource_array = resources.and_then(tos_foundation::JsonValue::as_array);
        let mut duplicate = false;
        if let Some(resource_array) = resource_array {
            for resource in resource_array {
                check(limits, cancelled)?;
                if resource.as_object().is_none() {
                    continue;
                }
                let id = resource
                    .object_get("resource_id")
                    .unwrap_or(&tos_foundation::JsonValue::Null);
                if matches!(
                    id,
                    tos_foundation::JsonValue::Array(_) | tos_foundation::JsonValue::Object(_)
                ) {
                    return Err(ItemRefusal::Unsupported(
                        "legacy inventory set membership has an unhashable resource_id".into(),
                    ));
                }
            }
            for (resource_index, resource) in resource_array.iter().enumerate() {
                if resource.as_object().is_none() {
                    continue;
                }
                check(limits, cancelled)?;
                let id = resource
                    .object_get("resource_id")
                    .unwrap_or(&tos_foundation::JsonValue::Null);
                for previous in &resource_array[..resource_index] {
                    charge_inventory_set_scan_step(
                        inventory_set_scan_steps,
                        limits.max_state_bytes,
                        limits,
                        cancelled,
                    )?;
                    if previous.as_object().is_none() {
                        continue;
                    }
                    let previous_id = previous
                        .object_get("resource_id")
                        .unwrap_or(&tos_foundation::JsonValue::Null);
                    if crate::item_rules::legacy_json_set_members_equal(previous_id, id) {
                        duplicate = true;
                        break;
                    }
                }
                if duplicate {
                    break;
                }
            }
        }
        if duplicate {
            let file_id = entry
                .object_get("file_id")
                .unwrap_or(&tos_foundation::JsonValue::Null);
            issues.push(
                SourceFoundationRecordsIssueFamily::Item,
                location,
                &format!(
                    "duplicate resource_id in {}",
                    legacy_python_display(file_id)?
                ),
            )?;
        }
        if let Some(summary) = entry
            .object_get("summary")
            .and_then(tos_foundation::JsonValue::as_object)
        {
            let expected_count = Value::from(resource_count);
            let value = summary
                .iter()
                .find(|(key, _)| key.as_str() == Some("resource_count"))
                .map(|(_, value)| value)
                .unwrap_or(&tos_foundation::JsonValue::Null);
            if !crate::item_rules::legacy_json_native_equal(value, &expected_count) {
                let file_id = entry
                    .object_get("file_id")
                    .unwrap_or(&tos_foundation::JsonValue::Null);
                issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    location,
                    &format!(
                        "resource_count differs from resources for {}",
                        legacy_python_display(file_id)?
                    ),
                )?;
            }
        }
    }
    Ok(())
}

fn python_legacy_iter_len(value: Option<&tos_foundation::JsonValue>) -> Result<u64, ItemRefusal> {
    let length = match value {
        None => 0,
        Some(tos_foundation::JsonValue::Array(values)) => values.len(),
        Some(tos_foundation::JsonValue::String(value)) => {
            char::decode_utf16(value.units().iter().copied()).count()
        }
        Some(tos_foundation::JsonValue::Object(entries)) => entries.len(),
        Some(_) => {
            return Err(ItemRefusal::Unsupported(
                "maintained inventory owner asks length of a non-iterable JSON value".into(),
            ));
        }
    };
    u64::try_from(length).map_err(|_| crate::item_budget_origin!())
}

fn legacy_python_display(value: &tos_foundation::JsonValue) -> Result<String, ItemRefusal> {
    match value {
        tos_foundation::JsonValue::Null => Ok("None".into()),
        tos_foundation::JsonValue::Bool(value) => Ok(if *value { "True" } else { "False" }.into()),
        tos_foundation::JsonValue::Number(number) => match number.kind {
            tos_foundation::JsonNumberKind::Int => {
                let negative = number.lexeme.starts_with('-');
                let digits = number.lexeme.strip_prefix('-').unwrap_or(&number.lexeme);
                let digits = digits.trim_start_matches('0');
                let digits = if digits.is_empty() { "0" } else { digits };
                Ok(if negative && digits != "0" {
                    format!("-{digits}")
                } else {
                    digits.to_owned()
                })
            }
            tos_foundation::JsonNumberKind::Float => {
                let value = number.as_python_float().ok_or_else(|| {
                    ItemRefusal::Unsupported("legacy float display is unrepresentable".into())
                })?;
                if value.is_nan() {
                    Ok("nan".into())
                } else if value == f64::INFINITY {
                    Ok("inf".into())
                } else if value == f64::NEG_INFINITY {
                    Ok("-inf".into())
                } else {
                    Err(ItemRefusal::Unsupported(
                        "finite Python float display is not yet source-exact".into(),
                    ))
                }
            }
        },
        tos_foundation::JsonValue::String(value) => {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                ItemRefusal::Unsupported("legacy string display contains a lone surrogate".into())
            })
        }
        tos_foundation::JsonValue::Array(_) | tos_foundation::JsonValue::Object(_) => {
            Err(ItemRefusal::Unsupported(
                "legacy container display is outside this inventory seam".into(),
            ))
        }
    }
}

fn append_direct_source_refs_fields<'a>(
    issues: &mut DirectIssueBuffer<'_>,
    family: SourceFoundationRecordsIssueFamily,
    location: &str,
    get: impl Fn(&str) -> Option<&'a Value>,
    source: SourceFoundationCurrentInput<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    for field in ["source_refs", "source_record_refs", "receipt_refs"] {
        if let Some(references) = get(field).and_then(Value::as_array) {
            for reference in references {
                append_direct_repo_ref(
                    issues, family, location, reference, source, deadline, cancelled,
                )?;
            }
        }
    }
    for field in [
        "rights_ref",
        "provenance_ref",
        "forensic_report_ref",
        "resource_inventory_ref",
        "generated_from_manifest_ref",
        "item_manifest_ref",
    ] {
        if let Some(reference) = get(field) {
            append_direct_repo_ref(
                issues, family, location, reference, source, deadline, cancelled,
            )?;
        }
    }
    Ok(())
}

fn append_direct_repo_ref(
    issues: &mut DirectIssueBuffer<'_>,
    family: SourceFoundationRecordsIssueFamily,
    location: &str,
    reference: &Value,
    source: SourceFoundationCurrentInput<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    let exists = match reference.as_str() {
        Some(reference) if !reference.starts_with("ToS/") => true,
        Some(reference) => {
            let path = RelativePath::parse(reference).map_err(|_| {
                ItemRefusal::Unsupported("source-foundation repository reference path".into())
            })?;
            source
                .presence(path.as_str(), deadline, cancelled)?
                .is_some()
        }
        None => false,
    };
    if !exists {
        issues.push(
            family,
            location,
            &format!(
                "repository reference does not exist: {}",
                python_value_string(reference)
            ),
        )?;
    }
    Ok(())
}

pub(crate) fn python_value_string(value: &Value) -> String {
    match value {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(python_value_repr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!(
                    "{}: {}",
                    python_string_repr(key),
                    python_value_repr(value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn python_value_repr(value: &Value) -> String {
    match value {
        Value::String(text) => python_string_repr(text),
        _ => python_value_string(value),
    }
}

fn python_string_repr(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut rendered = String::with_capacity(value.len().saturating_add(2));
    rendered.push(quote);
    for character in value.chars() {
        match character {
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            value if value == quote => {
                rendered.push('\\');
                rendered.push(value);
            }
            value if value.is_control() => rendered.push_str(&format!("\\u{:04x}", value as u32)),
            value => rendered.push(value),
        }
    }
    rendered.push(quote);
    rendered
}

struct DirectItemManifest {
    manifest_index: usize,
    inventory_index: Option<usize>,
    rights_index: Option<usize>,
}

fn has_direct_item_schema(
    schema_checks: &[SourceFoundationRecordsSchemaCheck],
    after_index: usize,
    contract: &str,
    path: &str,
) -> bool {
    for check in schema_checks.iter().skip(after_index.saturating_add(1)) {
        if check.contract == "ToS/contracts/source-item-manifest.schema.json" {
            break;
        }
        if check.family == SourceFoundationRecordsSchemaFamily::Item
            && check.owner_issue.is_none()
            && (check.decoded_instance.is_some() || check.legacy_raw_instance.is_some())
            && check.contract == contract
            && check.location == path
        {
            return true;
        }
    }
    false
}

fn append_item_direct_issues(
    source: SourceFoundationCurrentInput<'_>,
    limits: SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    physical: &SourcePhysicalFacts,
    current_records: &BTreeMap<String, BiblioCurrentRecord>,
    manifest_item_ids: &BTreeSet<String>,
    selected_items: &[SelectedItemRecord],
    membership_issues: &[SourceFoundationRecordsIssue],
    direct_read_limit: u64,
    direct_issue_state_limit: usize,
    schema_checks: &mut Vec<SourceFoundationRecordsSchemaCheck>,
    schema_request_state_bytes: &mut usize,
    schema_request_state_limit: usize,
    inventory_set_scan_steps_start: usize,
    issues: &mut DirectIssueBuffer<'_>,
    store: Option<SourceFoundationRecordsStoreHandle<'_>>,
) -> Result<(BTreeMap<String, String>, u64, usize, usize), ItemRefusal> {
    let mut item_editions = BTreeMap::<String, String>::new();
    let mut inventory_set_scan_steps = inventory_set_scan_steps_start;
    let mut global_event_ids = BTreeSet::new();
    let mut owner_index_state_bytes = std::mem::size_of::<BTreeMap<String, String>>()
        .checked_add(std::mem::size_of::<BTreeSet<String>>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<DirectItemManifest>>()))
        .ok_or(crate::item_budget_origin!())?;
    let mut owner_temporary_state_bytes = 0usize;
    let mut owner_peak_state_bytes = 0usize;
    reserve_direct_item_index_state(
        issues,
        &mut owner_index_state_bytes,
        owner_temporary_state_bytes,
        &mut owner_peak_state_bytes,
        direct_issue_state_limit,
    )?;
    let mut context: Option<DirectItemManifest> = None;
    let mut membership_index = 0usize;
    let mut direct_read_bytes = 0u64;

    for index in 0..schema_checks.len() {
        check(limits.items, cancelled)?;
        if schema_checks[index].family != SourceFoundationRecordsSchemaFamily::Item {
            continue;
        }
        let contract = schema_checks[index].contract.clone();
        if contract == "ToS/contracts/source-item-manifest.schema.json" {
            if let Some(previous) = context.take() {
                finish_direct_item_manifest(
                    source,
                    limits,
                    require_local_payloads,
                    cancelled,
                    physical,
                    schema_checks,
                    schema_request_state_bytes,
                    previous,
                    index,
                    membership_issues,
                    &mut membership_index,
                    &mut direct_read_bytes,
                    direct_read_limit,
                    &mut owner_index_state_bytes,
                    &mut owner_temporary_state_bytes,
                    &mut owner_peak_state_bytes,
                    direct_issue_state_limit,
                    issues,
                )?;
            }
            context = Some(DirectItemManifest {
                manifest_index: index,
                inventory_index: None,
                rights_index: None,
            });
        }

        schema_checks[index].before_issue = issues.rows.len();
        if schema_checks[index].decoded_instance.is_some()
            && schema_checks[index].legacy_raw_instance.is_some()
            || schema_checks[index].decoded_instance.is_none()
                && schema_checks[index].legacy_raw_instance.is_none()
                && schema_checks[index].owner_issue.is_none()
        {
            return Err(ItemRefusal::Source(
                "Item schema request has ambiguous instance representation".into(),
            ));
        }
        if let Some(owner_issue) = schema_checks[index].owner_issue {
            match contract.as_str() {
                "ToS/contracts/source-item-manifest.schema.json" => {}
                "ToS/contracts/source-resource-inventory.schema.json" => {
                    if context.is_none() {
                        return Err(ItemRefusal::Source(
                            "Item inventory root issue lacks its manifest owner".into(),
                        ));
                    }
                }
                "ToS/contracts/rights-record.schema.json" => {
                    if context.is_none() {
                        return Err(ItemRefusal::Source(
                            "Item rights root issue lacks its manifest owner".into(),
                        ));
                    }
                }
                "ToS/contracts/provenance-event.schema.json" if context.is_some() => {}
                "ToS/contracts/provenance-event.schema.json" => {
                    return Err(ItemRefusal::Source(
                        "Item provenance root issue lacks its manifest owner".into(),
                    ));
                }
                _ => continue,
            }
            issues.push(
                SourceFoundationRecordsIssueFamily::Item,
                &schema_checks[index].location,
                owner_issue.message(),
            )?;
            continue;
        }
        if let Some(raw) = schema_checks[index].legacy_raw_instance.as_deref() {
            if contract != "ToS/contracts/source-resource-inventory.schema.json" {
                return Err(ItemRefusal::Source(
                    "legacy raw schema input is outside the Item inventory contract".into(),
                ));
            }
            let Some(current) = context.as_mut() else {
                return Err(ItemRefusal::Source(
                    "Item inventory schema request lacks its manifest owner".into(),
                ));
            };
            current.inventory_index = Some(index);
            let manifest_index = current.manifest_index;
            let manifest = schema_checks[manifest_index]
                .decoded_instance
                .as_ref()
                .filter(|manifest| manifest.is_object())
                .ok_or_else(|| {
                    ItemRefusal::Source("Item manifest decoded instance unavailable".into())
                })?;
            let parse_state_limit = direct_issue_state_limit
                .checked_sub(owner_index_state_bytes)
                .and_then(|bytes| bytes.checked_sub(owner_temporary_state_bytes))
                .and_then(|bytes| bytes.checked_sub(issues.state_bytes))
                .ok_or(crate::item_budget_origin!())?;
            let inventory =
                bounded_legacy_observed_inventory(raw, limits.items, parse_state_limit, cancelled)?
                    .ok_or_else(|| {
                        ItemRefusal::Source(
                            "retained legacy inventory no longer has its observed nonfinite value"
                                .into(),
                        )
                    })?;
            if inventory.as_object().is_none() {
                return Err(ItemRefusal::Source(
                    "non-object legacy inventory lacks its closed root owner issue".into(),
                ));
            }
            let retained = crate::item_rules::observed_json_retained_bytes(&inventory)?;
            set_direct_item_temporary_state(
                issues,
                owner_index_state_bytes,
                &mut owner_temporary_state_bytes,
                &mut owner_peak_state_bytes,
                direct_issue_state_limit,
                retained,
            )?;
            append_direct_source_refs_observed(
                issues,
                SourceFoundationRecordsIssueFamily::Item,
                &schema_checks[index].location,
                &inventory,
                source,
                limits.items.deadline,
                cancelled,
            )?;
            append_direct_legacy_inventory_issues(
                issues,
                &schema_checks[index].location,
                &schema_checks[manifest_index].location,
                manifest,
                &inventory,
                limits.items,
                cancelled,
                &mut inventory_set_scan_steps,
            )?;
            set_direct_item_temporary_state(
                issues,
                owner_index_state_bytes,
                &mut owner_temporary_state_bytes,
                &mut owner_peak_state_bytes,
                direct_issue_state_limit,
                0,
            )?;
            let rights_path = manifest.get("rights_ref").and_then(Value::as_str);
            let rights_has_check = rights_path.is_some_and(|path| {
                has_direct_item_schema(
                    schema_checks,
                    index,
                    "ToS/contracts/rights-record.schema.json",
                    path,
                )
            });
            if !rights_has_check {
                if let Some(rights_path) = rights_path
                    && current_file_presence(source, rights_path, limits.items.deadline, cancelled)?
                        .is_none()
                {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        rights_path,
                        "file is missing",
                    )?;
                }
                if let Some(provenance_path) =
                    manifest.get("provenance_ref").and_then(Value::as_str)
                    && current_file_presence(
                        source,
                        provenance_path,
                        limits.items.deadline,
                        cancelled,
                    )?
                    .is_none()
                {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        provenance_path,
                        "file is missing",
                    )?;
                }
            }
            continue;
        }
        if schema_checks[index].decoded_instance.is_none() {
            return Err(ItemRefusal::Source(
                "Item schema request lacks a decoded instance or closed owner issue".into(),
            ));
        }
        let decoded_instance = schema_checks[index]
            .decoded_instance
            .as_ref()
            .ok_or_else(|| ItemRefusal::Source("Item decoded instance disappeared".into()))?;
        match contract.as_str() {
            "ToS/contracts/source-item-manifest.schema.json" => {
                let request = &schema_checks[index];
                let manifest = decoded_instance;
                append_direct_source_refs(
                    issues,
                    SourceFoundationRecordsIssueFamily::Item,
                    &request.location,
                    manifest,
                    source,
                    limits.items.deadline,
                    cancelled,
                )?;
                let item_id = manifest.get("item_id").unwrap_or(&Value::Null);
                if let Some(item_id) = item_id.as_str() {
                    direct_require_record(
                        issues,
                        current_records,
                        item_id,
                        "item",
                        &request.location,
                        limits.operation.max_issues,
                    )?;
                    if let Some(edition) = manifest.get("embodiment_ref").and_then(Value::as_str) {
                        if let Some(store) = &store {
                            store.with(|store| store.item_edition(item_id, edition))?;
                        }
                        if let Some(previous) = item_editions.get(item_id) {
                            let previous_len = previous.len();
                            set_direct_item_temporary_state(
                                issues,
                                owner_index_state_bytes,
                                &mut owner_temporary_state_bytes,
                                &mut owner_peak_state_bytes,
                                direct_issue_state_limit,
                                std::mem::size_of::<String>()
                                    .checked_add(edition.len())
                                    .ok_or(crate::item_budget_origin!())?,
                            )?;
                            let mut replacement = edition.to_owned();
                            let retained = item_editions.get_mut(item_id).ok_or_else(|| {
                                ItemRefusal::Source(
                                    "Item edition map changed during replacement".into(),
                                )
                            })?;
                            std::mem::swap(retained, &mut replacement);
                            let new_len = retained.len();
                            let replacement_state_bytes = std::mem::size_of::<String>()
                                .checked_add(previous_len)
                                .ok_or(crate::item_budget_origin!())?;
                            let new_persistent_state_bytes = if new_len > previous_len {
                                owner_index_state_bytes
                                    .checked_add(new_len - previous_len)
                                    .ok_or(crate::item_budget_origin!())?
                            } else {
                                owner_index_state_bytes
                                    .checked_sub(previous_len - new_len)
                                    .ok_or(crate::item_budget_origin!())?
                            };
                            set_direct_item_live_state(
                                issues,
                                &mut owner_index_state_bytes,
                                &mut owner_temporary_state_bytes,
                                &mut owner_peak_state_bytes,
                                direct_issue_state_limit,
                                new_persistent_state_bytes,
                                replacement_state_bytes,
                            )?;
                            drop(replacement);
                            set_direct_item_temporary_state(
                                issues,
                                owner_index_state_bytes,
                                &mut owner_temporary_state_bytes,
                                &mut owner_peak_state_bytes,
                                direct_issue_state_limit,
                                0,
                            )?;
                        } else {
                            let retained = std::mem::size_of::<(String, String)>()
                                .checked_add(item_id.len())
                                .and_then(|bytes| bytes.checked_add(edition.len()))
                                .and_then(|bytes| {
                                    bytes.checked_add(3 * std::mem::size_of::<usize>())
                                })
                                .ok_or(crate::item_budget_origin!())?;
                            retain_direct_item_index_state_by(
                                issues,
                                &mut owner_index_state_bytes,
                                owner_temporary_state_bytes,
                                &mut owner_peak_state_bytes,
                                direct_issue_state_limit,
                                retained,
                            )?;
                            item_editions.insert(item_id.to_owned(), edition.to_owned());
                        }
                    }
                }
                direct_require_record_value(
                    issues,
                    current_records,
                    manifest.get("embodiment_ref").unwrap_or(&Value::Null),
                    "edition",
                    &request.location,
                )?;
                let inventory_missing = match manifest
                    .get("resource_inventory_ref")
                    .and_then(Value::as_str)
                {
                    Some(path) => {
                        current_file_presence(source, path, limits.items.deadline, cancelled)?
                            .is_none()
                    }
                    None => false,
                };
                let rights_missing = match manifest.get("rights_ref").and_then(Value::as_str) {
                    Some(path) => {
                        current_file_presence(source, path, limits.items.deadline, cancelled)?
                            .is_none()
                    }
                    None => false,
                };
                let provenance_missing =
                    match manifest.get("provenance_ref").and_then(Value::as_str) {
                        Some(path) => {
                            current_file_presence(source, path, limits.items.deadline, cancelled)?
                                .is_none()
                        }
                        None => false,
                    };
                let inventory_has_check = manifest
                    .get("resource_inventory_ref")
                    .and_then(Value::as_str)
                    .is_some_and(|path| {
                        has_direct_item_schema(
                            schema_checks,
                            index,
                            "ToS/contracts/source-resource-inventory.schema.json",
                            path,
                        )
                    });
                let rights_has_check = manifest
                    .get("rights_ref")
                    .and_then(Value::as_str)
                    .is_some_and(|path| {
                        has_direct_item_schema(
                            schema_checks,
                            index,
                            "ToS/contracts/rights-record.schema.json",
                            path,
                        )
                    });
                if inventory_missing {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        manifest
                            .get("resource_inventory_ref")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                        "file is missing",
                    )?;
                }
                if !inventory_has_check {
                    if rights_missing {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            manifest
                                .get("rights_ref")
                                .and_then(Value::as_str)
                                .unwrap_or_default(),
                            "file is missing",
                        )?;
                    }
                    if !rights_has_check && provenance_missing {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            manifest
                                .get("provenance_ref")
                                .and_then(Value::as_str)
                                .unwrap_or_default(),
                            "file is missing",
                        )?;
                    }
                }
            }
            "ToS/contracts/source-resource-inventory.schema.json" => {
                let Some(current) = context.as_mut() else {
                    return Err(ItemRefusal::Source(
                        "Item inventory schema request lacks its manifest owner".into(),
                    ));
                };
                current.inventory_index = Some(index);
                let request = &schema_checks[index];
                let inventory = decoded_instance;
                append_direct_source_refs(
                    issues,
                    SourceFoundationRecordsIssueFamily::Item,
                    &request.location,
                    inventory,
                    source,
                    limits.items.deadline,
                    cancelled,
                )?;
                let manifest = schema_checks[current.manifest_index]
                    .decoded_instance
                    .as_ref()
                    .ok_or_else(|| {
                        ItemRefusal::Source("Item manifest decoded instance unavailable".into())
                    })?;
                let item_id = manifest.get("item_id").unwrap_or(&Value::Null);
                if !python_values_equal(inventory.get("item_id").unwrap_or(&Value::Null), item_id) {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        &request.location,
                        "resource inventory item_id differs from manifest",
                    )?;
                }
                let expected_manifest_ref =
                    Value::String(schema_checks[current.manifest_index].location.clone());
                if !python_values_equal(
                    inventory
                        .get("generated_from_manifest_ref")
                        .unwrap_or(&Value::Null),
                    &expected_manifest_ref,
                ) {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        &request.location,
                        "resource inventory does not cite its current item manifest",
                    )?;
                }
                let mut expected = manifest
                    .get("payload_files")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|entry| entry.is_object());
                let mut actual = inventory
                    .get("files")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|entry| entry.is_object());
                let same_files = loop {
                    check(limits.items, cancelled)?;
                    match (actual.next(), expected.next()) {
                        (None, None) => break true,
                        (Some(actual), Some(expected))
                            if python_values_equal(
                                actual.get("file_id").unwrap_or(&Value::Null),
                                expected.get("file_id").unwrap_or(&Value::Null),
                            ) && python_values_equal(
                                actual.get("file_sha256").unwrap_or(&Value::Null),
                                expected.get("sha256").unwrap_or(&Value::Null),
                            ) && python_values_equal(
                                actual.get("media_type").unwrap_or(&Value::Null),
                                expected.get("media_type").unwrap_or(&Value::Null),
                            ) => {}
                        _ => break false,
                    }
                };
                if !same_files {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        &request.location,
                        "resource inventory file identity differs from manifest payload files",
                    )?;
                }
                for entry in inventory
                    .get("files")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_object)
                {
                    check(limits.items, cancelled)?;
                    let resources = entry.get("resources").and_then(Value::as_array);
                    let mut duplicate = false;
                    if let Some(resources) = resources {
                        for resource in resources {
                            check(limits.items, cancelled)?;
                            if !resource.is_object() {
                                continue;
                            }
                            let id = resource.get("resource_id").unwrap_or(&Value::Null);
                            if matches!(id, Value::Array(_) | Value::Object(_)) {
                                return Err(ItemRefusal::Unsupported(
                                    "legacy inventory set membership has an unhashable resource_id"
                                        .into(),
                                ));
                            }
                        }
                        for (offset, resource) in resources.iter().enumerate() {
                            if !resource.is_object() {
                                continue;
                            }
                            check(limits.items, cancelled)?;
                            let id = resource.get("resource_id").unwrap_or(&Value::Null);
                            for previous in &resources[..offset] {
                                charge_inventory_set_scan_step(
                                    &mut inventory_set_scan_steps,
                                    limits.items.max_state_bytes,
                                    limits.items,
                                    cancelled,
                                )?;
                                if !previous.is_object() {
                                    continue;
                                }
                                let previous_id =
                                    previous.get("resource_id").unwrap_or(&Value::Null);
                                if crate::item_rules::native_json_set_members_equal(previous_id, id)
                                {
                                    duplicate = true;
                                    break;
                                }
                            }
                            if duplicate {
                                break;
                            }
                        }
                    }
                    if duplicate {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            &request.location,
                            &format!(
                                "duplicate resource_id in {}",
                                python_value_string(entry.get("file_id").unwrap_or(&Value::Null))
                            ),
                        )?;
                    }
                    if let Some(summary) = entry.get("summary").and_then(Value::as_object) {
                        let expected_count =
                            Value::from(resources.map_or(0, |resources| resources.len()) as u64);
                        if !python_values_equal(
                            summary.get("resource_count").unwrap_or(&Value::Null),
                            &expected_count,
                        ) {
                            issues.push(
                                SourceFoundationRecordsIssueFamily::Item,
                                &request.location,
                                &format!(
                                    "resource_count differs from resources for {}",
                                    python_value_string(
                                        entry.get("file_id").unwrap_or(&Value::Null)
                                    )
                                ),
                            )?;
                        }
                    }
                }
                let rights_path = manifest.get("rights_ref").and_then(Value::as_str);
                let rights_has_check = rights_path.is_some_and(|path| {
                    has_direct_item_schema(
                        schema_checks,
                        index,
                        "ToS/contracts/rights-record.schema.json",
                        path,
                    )
                });
                if !rights_has_check {
                    if let Some(rights_path) = rights_path
                        && current_file_presence(
                            source,
                            rights_path,
                            limits.items.deadline,
                            cancelled,
                        )?
                        .is_none()
                    {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            rights_path,
                            "file is missing",
                        )?;
                    }
                    if !rights_has_check
                        && let Some(provenance_path) =
                            manifest.get("provenance_ref").and_then(Value::as_str)
                        && current_file_presence(
                            source,
                            provenance_path,
                            limits.items.deadline,
                            cancelled,
                        )?
                        .is_none()
                    {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            provenance_path,
                            "file is missing",
                        )?;
                    }
                }
            }
            "ToS/contracts/rights-record.schema.json" => {
                let Some(current) = context.as_mut() else {
                    return Err(ItemRefusal::Source(
                        "Item rights schema request lacks its manifest owner".into(),
                    ));
                };
                current.rights_index = Some(index);
                let request = &schema_checks[index];
                let rights = decoded_instance;
                append_direct_source_refs(
                    issues,
                    SourceFoundationRecordsIssueFamily::Item,
                    &request.location,
                    rights,
                    source,
                    limits.items.deadline,
                    cancelled,
                )?;
                let manifest = schema_checks[current.manifest_index]
                    .decoded_instance
                    .as_ref()
                    .ok_or_else(|| {
                        ItemRefusal::Source("Item manifest decoded instance unavailable".into())
                    })?;
                let item_id = manifest.get("item_id").unwrap_or(&Value::Null);
                if let Some(scopes) = rights.get("scope_refs").and_then(Value::as_array) {
                    if !scopes
                        .iter()
                        .any(|scope| python_values_equal(scope, item_id))
                    {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            &request.location,
                            &format!(
                                "scope_refs does not include item_id {}",
                                python_value_string(item_id)
                            ),
                        )?;
                    }
                }
                if !python_values_equal(
                    rights.get("visibility").unwrap_or(&Value::Null),
                    manifest.get("visibility").unwrap_or(&Value::Null),
                ) {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        &request.location,
                        "rights visibility differs from item manifest visibility",
                    )?;
                }
                if let Some(layers) = rights.get("layer_assessments").and_then(Value::as_array) {
                    for (index, layer) in layers.iter().enumerate() {
                        check(limits.items, cancelled)?;
                        let Some(layer) = layer.as_object() else {
                            continue;
                        };
                        let layer_location =
                            format!("{}#layer_assessments/{}", request.location, index + 1);
                        if let Some(id) = layer.get("layer_id").and_then(Value::as_str) {
                            let duplicate = layers[..index].iter().any(|previous| {
                                previous
                                    .as_object()
                                    .and_then(|previous| previous.get("layer_id"))
                                    .and_then(Value::as_str)
                                    == Some(id)
                            });
                            if duplicate {
                                issues.push(
                                    SourceFoundationRecordsIssueFamily::Item,
                                    &layer_location,
                                    &format!("duplicate layer_id: {id}"),
                                )?;
                            }
                        }
                        append_direct_source_refs_fields(
                            issues,
                            SourceFoundationRecordsIssueFamily::Item,
                            &layer_location,
                            |field| layer.get(field),
                            source,
                            limits.items.deadline,
                            cancelled,
                        )?;
                    }
                }
                let manifest = schema_checks[current.manifest_index]
                    .decoded_instance
                    .as_ref()
                    .ok_or_else(|| {
                        ItemRefusal::Source("Item manifest decoded instance unavailable".into())
                    })?;
                if let Some(provenance_path) =
                    manifest.get("provenance_ref").and_then(Value::as_str)
                    && current_file_presence(
                        source,
                        provenance_path,
                        limits.items.deadline,
                        cancelled,
                    )?
                    .is_none()
                {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        provenance_path,
                        "file is missing",
                    )?;
                }
            }
            "ToS/contracts/provenance-event.schema.json" => {
                if context.is_none() {
                    return Err(ItemRefusal::Source(
                        "Item provenance schema request lacks its manifest owner".into(),
                    ));
                }
                let request = &schema_checks[index];
                let event = decoded_instance;
                append_direct_source_refs(
                    issues,
                    SourceFoundationRecordsIssueFamily::Item,
                    &request.location,
                    event,
                    source,
                    limits.items.deadline,
                    cancelled,
                )?;
                if let Some(id) = event.get("event_id").and_then(Value::as_str) {
                    if global_event_ids.contains(id) {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            &request.location,
                            &format!("duplicate event_id: {id}"),
                        )?;
                    } else {
                        let retained = id
                            .len()
                            .checked_add(std::mem::size_of::<String>())
                            .and_then(|bytes| bytes.checked_add(3 * std::mem::size_of::<usize>()))
                            .ok_or(crate::item_budget_origin!())?;
                        retain_direct_item_index_state_by(
                            issues,
                            &mut owner_index_state_bytes,
                            owner_temporary_state_bytes,
                            &mut owner_peak_state_bytes,
                            direct_issue_state_limit,
                            retained,
                        )?;
                        global_event_ids.insert(id.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(previous) = context.take() {
        finish_direct_item_manifest(
            source,
            limits,
            require_local_payloads,
            cancelled,
            physical,
            schema_checks,
            schema_request_state_bytes,
            previous,
            schema_checks.len(),
            membership_issues,
            &mut membership_index,
            &mut direct_read_bytes,
            direct_read_limit,
            &mut owner_index_state_bytes,
            &mut owner_temporary_state_bytes,
            &mut owner_peak_state_bytes,
            direct_issue_state_limit,
            issues,
        )?;
    }

    for selected in selected_items {
        check(limits.items, cancelled)?;
        let item = &selected.selection.value;
        let item_id = &selected.selection.record_id;
        let location = &selected.selection.path;
        if !manifest_item_ids.contains(item_id) {
            issues.push(
                SourceFoundationRecordsIssueFamily::Item,
                location,
                "item record has no validated item.manifest.json",
            )?;
        }
        let Some(manifest_ref) = item.get("item_manifest_ref").and_then(Value::as_str) else {
            continue;
        };
        let relative = match RelativePath::parse(manifest_ref) {
            Ok(path) => path,
            Err(_) => {
                return Err(ItemRefusal::Unsupported(
                    "final Item manifest reference leaves the captured source route".into(),
                ));
            }
        };
        let Some(metadata_size_bytes) =
            source.member_size(relative.as_str(), limits.items.deadline, cancelled)?
        else {
            return match source.presence(relative.as_str(), limits.items.deadline, cancelled)? {
                None => {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        manifest_ref,
                        "file is missing",
                    )?;
                    continue;
                }
                Some(SourcePresenceV1::MaterializedDirectory) => {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        manifest_ref,
                        "cannot read JSON: path is a directory",
                    )?;
                    continue;
                }
                Some(SourcePresenceV1::File) => Err(ItemRefusal::Source(
                    "final Item manifest presence/member disagreement".into(),
                )),
            };
        };
        account_direct_item_read(
            metadata_size_bytes,
            &mut direct_read_bytes,
            direct_read_limit,
            limits.items.max_member_bytes,
            "source-foundation final Item manifest reads",
        )?;
        let member = source.read_member(
            relative.as_str(),
            limits.items.max_member_bytes,
            direct_item_source_state_bytes(schema_request_state_bytes, issues)?,
            limits.items.max_state_bytes,
            limits.items.deadline,
            cancelled,
        )?;
        if member.size_bytes != metadata_size_bytes {
            return Err(ItemRefusal::Source(
                "final Item manifest size changed after metadata traversal".into(),
            ));
        }
        let value = match bounded_native_value(
            &member.raw,
            limits.items,
            limits.items.max_state_bytes,
            cancelled,
        ) {
            Ok(value) => value,
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                match legacy_parse_failure(
                    &member.raw,
                    limits.items,
                    limits.items.max_state_bytes,
                    cancelled,
                )? {
                    LegacyJsonParseFailure::Malformed => {
                        issues.push(
                            SourceFoundationRecordsIssueFamily::Item,
                            manifest_ref,
                            "cannot read JSON: invalid JSON",
                        )?;
                        continue;
                    }
                    LegacyJsonParseFailure::Nonfinite => {
                        return Err(ItemRefusal::Unsupported(
                            "source-foundation nonfinite legacy JSON cannot be represented by serde_json::Value".into(),
                        ));
                    }
                }
            }
            Err(error) => return Err(error),
        };
        let Some(manifest) = value.as_object() else {
            if schema_checks.len() >= limits.items.max_issues {
                return Err(crate::item_budget_origin!());
            }
            let before_issue = issues.rows.len();
            retain_direct_schema_check(
                schema_checks,
                schema_request_state_bytes,
                schema_request_state_limit,
                before_issue,
                SourceFoundationRecordsSchemaFamily::Item,
                manifest_ref,
                "ToS/contracts/source-item-manifest.schema.json",
                Some(value),
                None,
            )?;
            issues.push(
                SourceFoundationRecordsIssueFamily::Item,
                manifest_ref,
                "JSON root must be an object",
            )?;
            continue;
        };
        if !python_values_equal(
            manifest.get("item_id").unwrap_or(&Value::Null),
            &Value::String(item_id.clone()),
        ) {
            issues.push(
                SourceFoundationRecordsIssueFamily::Item,
                manifest_ref,
                &format!("manifest item_id does not match {item_id}"),
            )?;
        }
    }
    let record_issue_state = issues.family_state_bytes(SourceFoundationRecordsIssueFamily::Record);
    let record_issue_count = issues
        .rows
        .iter()
        .filter(|issue| issue.family == SourceFoundationRecordsIssueFamily::Record)
        .count();
    if record_issue_state > limits.records.max_state_bytes
        || record_issue_count > limits.records.max_issues
    {
        return Err(crate::item_budget_origin!());
    }
    let item_issue_state = issues.family_state_bytes(SourceFoundationRecordsIssueFamily::Item);
    let item_issue_count = issues
        .rows
        .iter()
        .filter(|issue| issue.family == SourceFoundationRecordsIssueFamily::Item)
        .count();
    if item_issue_state > limits.items.max_state_bytes || item_issue_count > limits.items.max_issues
    {
        return Err(crate::item_budget_origin!());
    }
    let direct_inventory_set_scan_steps = inventory_set_scan_steps
        .checked_sub(inventory_set_scan_steps_start)
        .ok_or(crate::item_budget_origin!())?;
    Ok((
        item_editions,
        direct_read_bytes,
        owner_peak_state_bytes,
        direct_inventory_set_scan_steps,
    ))
}

fn direct_require_record(
    issues: &mut DirectIssueBuffer<'_>,
    current_records: &BTreeMap<String, BiblioCurrentRecord>,
    reference: &str,
    expected: &str,
    location: &str,
    issue_limit: usize,
) -> Result<(), ItemRefusal> {
    if issues.rows.len() >= issue_limit {
        return Err(crate::item_budget_origin!());
    }
    let Some(actual) = current_records.get(reference) else {
        return issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            location,
            &format!("unresolved {expected} reference: {reference}"),
        );
    };
    if actual.kind != expected {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            location,
            &format!(
                "{reference} resolves to {}, expected {expected}",
                actual.kind
            ),
        )?;
    }
    Ok(())
}

fn direct_require_record_value(
    issues: &mut DirectIssueBuffer<'_>,
    current_records: &BTreeMap<String, BiblioCurrentRecord>,
    reference: &Value,
    expected: &str,
    location: &str,
) -> Result<(), ItemRefusal> {
    if let Some(reference) = reference.as_str() {
        direct_require_record(
            issues,
            current_records,
            reference,
            expected,
            location,
            usize::MAX,
        )?;
    }
    Ok(())
}

fn reserve_direct_item_index_state(
    issues: &mut DirectIssueBuffer<'_>,
    persistent_state_bytes: &mut usize,
    temporary_state_bytes: usize,
    peak_state_bytes: &mut usize,
    state_limit: usize,
) -> Result<(), ItemRefusal> {
    let live_state_bytes = persistent_state_bytes
        .checked_add(temporary_state_bytes)
        .filter(|used| *used <= state_limit)
        .ok_or(crate::item_budget_origin!())?;
    *peak_state_bytes = (*peak_state_bytes).max(live_state_bytes);
    let issue_limit = state_limit
        .checked_sub(live_state_bytes)
        .ok_or(crate::item_budget_origin!())?;
    if issues.state_bytes > issue_limit {
        return Err(crate::item_budget_origin!());
    }
    issues.max_state_bytes = issue_limit.min(issues.hard_max_state_bytes);
    Ok(())
}

fn retain_direct_item_index_state_by(
    issues: &mut DirectIssueBuffer<'_>,
    persistent_state_bytes: &mut usize,
    temporary_state_bytes: usize,
    peak_state_bytes: &mut usize,
    state_limit: usize,
    additional_bytes: usize,
) -> Result<(), ItemRefusal> {
    *persistent_state_bytes = persistent_state_bytes
        .checked_add(additional_bytes)
        .filter(|used| *used <= state_limit)
        .ok_or(crate::item_budget_origin!())?;
    reserve_direct_item_index_state(
        issues,
        persistent_state_bytes,
        temporary_state_bytes,
        peak_state_bytes,
        state_limit,
    )
}

fn set_direct_item_temporary_state(
    issues: &mut DirectIssueBuffer<'_>,
    persistent_state_bytes: usize,
    temporary_state_bytes: &mut usize,
    peak_state_bytes: &mut usize,
    state_limit: usize,
    new_temporary_state_bytes: usize,
) -> Result<(), ItemRefusal> {
    let live_state_bytes = persistent_state_bytes
        .checked_add(new_temporary_state_bytes)
        .filter(|used| *used <= state_limit)
        .ok_or(crate::item_budget_origin!())?;
    if issues.state_bytes > state_limit.saturating_sub(live_state_bytes) {
        return Err(crate::item_budget_origin!());
    }
    *temporary_state_bytes = new_temporary_state_bytes;
    *peak_state_bytes = (*peak_state_bytes).max(live_state_bytes);
    issues.max_state_bytes = state_limit
        .checked_sub(live_state_bytes)
        .ok_or(crate::item_budget_origin!())?
        .min(issues.hard_max_state_bytes);
    Ok(())
}

fn set_direct_item_live_state(
    issues: &mut DirectIssueBuffer<'_>,
    persistent_state_bytes: &mut usize,
    temporary_state_bytes: &mut usize,
    peak_state_bytes: &mut usize,
    state_limit: usize,
    new_persistent_state_bytes: usize,
    new_temporary_state_bytes: usize,
) -> Result<(), ItemRefusal> {
    let live_state_bytes = new_persistent_state_bytes
        .checked_add(new_temporary_state_bytes)
        .filter(|used| *used <= state_limit)
        .ok_or(crate::item_budget_origin!())?;
    *persistent_state_bytes = new_persistent_state_bytes;
    *temporary_state_bytes = new_temporary_state_bytes;
    *peak_state_bytes = (*peak_state_bytes).max(live_state_bytes);
    let issue_limit = state_limit
        .checked_sub(live_state_bytes)
        .ok_or(crate::item_budget_origin!())?;
    if issues.state_bytes > issue_limit {
        return Err(crate::item_budget_origin!());
    }
    issues.max_state_bytes = issue_limit.min(issues.hard_max_state_bytes);
    Ok(())
}

fn finish_direct_item_manifest(
    source: SourceFoundationCurrentInput<'_>,
    limits: SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    physical: &SourcePhysicalFacts,
    schema_checks: &[SourceFoundationRecordsSchemaCheck],
    schema_request_state_bytes: &usize,
    context: DirectItemManifest,
    event_end_index: usize,
    membership_issues: &[SourceFoundationRecordsIssue],
    membership_index: &mut usize,
    direct_read_bytes: &mut u64,
    direct_read_limit: u64,
    owner_index_state_bytes: &mut usize,
    owner_temporary_state_bytes: &mut usize,
    owner_peak_state_bytes: &mut usize,
    owner_state_limit: usize,
    issues: &mut DirectIssueBuffer<'_>,
) -> Result<(), ItemRefusal> {
    let manifest_request = &schema_checks[context.manifest_index];
    let manifest_path = &manifest_request.location;
    let Some(manifest) = manifest_request
        .decoded_instance
        .as_ref()
        .filter(|manifest| manifest.is_object())
    else {
        return Ok(());
    };
    let Some((item_directory_source, _)) = manifest_path.rsplit_once('/') else {
        return Err(ItemRefusal::Unsupported("Item manifest directory".into()));
    };
    let fixity_path_len = item_directory_source
        .len()
        .checked_add("/fixity.sha256".len())
        .ok_or(crate::item_budget_origin!())?;
    let path_temporary_state_bytes = std::mem::size_of::<String>()
        .checked_add(item_directory_source.len())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>()))
        .and_then(|bytes| bytes.checked_add(fixity_path_len))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<RelativePath>()))
        .and_then(|bytes| bytes.checked_add(fixity_path_len))
        .ok_or(crate::item_budget_origin!())?;
    set_direct_item_temporary_state(
        issues,
        *owner_index_state_bytes,
        owner_temporary_state_bytes,
        owner_peak_state_bytes,
        owner_state_limit,
        path_temporary_state_bytes,
    )?;
    let item_directory = item_directory_source.to_owned();
    let fixity_path = format!("{item_directory}/fixity.sha256");
    let fixity_relative = RelativePath::parse(&fixity_path)
        .map_err(|_| ItemRefusal::Unsupported("Item fixity path".into()))?;

    let acquisition = manifest
        .get("acquisition_event_ref")
        .unwrap_or(&Value::Null);
    let acquisition_local = if let Some(acquisition_id) = acquisition.as_str() {
        let mut found = false;
        for event_index in context.manifest_index.saturating_add(1)..event_end_index {
            check(limits.items, cancelled)?;
            let event_check = &schema_checks[event_index];
            if event_check.contract == "ToS/contracts/provenance-event.schema.json"
                && event_check
                    .decoded_instance
                    .as_ref()
                    .and_then(|event| event.get("event_id"))
                    .and_then(Value::as_str)
                    == Some(acquisition_id)
            {
                found = true;
                break;
            }
        }
        found
    } else {
        false
    };
    if !acquisition_local {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            manifest_path,
            &format!(
                "acquisition_event_ref is absent from provenance: {}",
                python_value_string(acquisition)
            ),
        )?;
    }

    if let Some(inventory_index) = context.inventory_index {
        let inventory_request = &schema_checks[inventory_index];
        if let Some(raw) = inventory_request.legacy_raw_instance.as_deref() {
            let parse_state_limit = owner_state_limit
                .checked_sub(*owner_index_state_bytes)
                .and_then(|bytes| bytes.checked_sub(*owner_temporary_state_bytes))
                .and_then(|bytes| bytes.checked_sub(issues.state_bytes))
                .ok_or(crate::item_budget_origin!())?;
            let inventory =
                bounded_legacy_observed_inventory(raw, limits.items, parse_state_limit, cancelled)?
                    .ok_or_else(|| {
                        ItemRefusal::Source(
                            "retained legacy inventory no longer has its observed nonfinite value"
                                .into(),
                        )
                    })?;
            if inventory.as_object().is_none() {
                return Ok(());
            }
            let retained = crate::item_rules::observed_json_retained_bytes(&inventory)?;
            set_direct_item_temporary_state(
                issues,
                *owner_index_state_bytes,
                owner_temporary_state_bytes,
                owner_peak_state_bytes,
                owner_state_limit,
                retained,
            )?;
            let inventory_ref = inventory
                .object_get("provenance_event_ref")
                .unwrap_or(&tos_foundation::JsonValue::Null);
            append_legacy_inventory_provenance_issue(
                source,
                limits.items,
                cancelled,
                schema_checks,
                inventory_index,
                context.manifest_index,
                event_end_index,
                inventory_ref,
                direct_read_bytes,
                direct_read_limit,
                direct_item_source_state_bytes(schema_request_state_bytes, issues)?,
                limits.items.max_state_bytes,
                issues,
            )?;
            set_direct_item_temporary_state(
                issues,
                *owner_index_state_bytes,
                owner_temporary_state_bytes,
                owner_peak_state_bytes,
                owner_state_limit,
                0,
            )?;
        } else {
            let Some(inventory) = inventory_request
                .decoded_instance
                .as_ref()
                .filter(|inventory| inventory.is_object())
            else {
                return Ok(());
            };
            let inventory_ref = inventory
                .get("provenance_event_ref")
                .unwrap_or(&Value::Null);
            let mut event = None;
            if let Some(inventory_event_id) = inventory_ref.as_str() {
                for event_index in context.manifest_index.saturating_add(1)..event_end_index {
                    check(limits.items, cancelled)?;
                    let event_check = &schema_checks[event_index];
                    if event_check.contract == "ToS/contracts/provenance-event.schema.json"
                        && event_check
                            .decoded_instance
                            .as_ref()
                            .and_then(|event| event.get("event_id"))
                            .and_then(Value::as_str)
                            == Some(inventory_event_id)
                    {
                        event = event_check.decoded_instance.as_ref();
                    }
                }
            }
            if let Some(event) = event {
                let digest = source
                    .member_digest_with_accounting(
                        &inventory_request.location,
                        limits.items.max_member_bytes,
                        Some((direct_read_bytes, direct_read_limit)),
                        direct_item_source_state_bytes(schema_request_state_bytes, issues)?,
                        limits.items.max_state_bytes,
                        limits.items.deadline,
                        cancelled,
                    )?
                    .map(|digest| digest.to_hex())
                    .unwrap_or_default();
                let expected_output = serde_json::json!({
                    "ref": inventory_request.location,
                    "role": "tracked_text_free_resource_inventory",
                    "sha256": digest,
                });
                let has_output =
                    event
                        .get("outputs")
                        .and_then(Value::as_array)
                        .is_some_and(|outputs| {
                            outputs.iter().any(|output| {
                                output.as_object().is_some_and(|object| {
                                    object.len() == 3
                                        && python_values_equal(output, &expected_output)
                                })
                            })
                        });
                if !has_output {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        &inventory_request.location,
                        "resource inventory provenance event lacks its digest-bound output",
                    )?;
                }
            } else {
                issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    &inventory_request.location,
                    &format!(
                        "provenance_event_ref is absent from item provenance: {}",
                        python_value_string(inventory_ref)
                    ),
                )?;
            }
        }
    }

    let mut actual_fixity_state_bytes = 0usize;
    let actual_fixity = if let Some(metadata_size_bytes) =
        source.member_size(&fixity_path, limits.items.deadline, cancelled)?
    {
        account_direct_item_read(
            metadata_size_bytes,
            direct_read_bytes,
            direct_read_limit,
            limits.items.max_member_bytes,
            "source-foundation Item fixity reads",
        )?;
        let raw_size =
            usize::try_from(metadata_size_bytes).map_err(|_| crate::item_budget_origin!())?;
        actual_fixity_state_bytes = std::mem::size_of::<String>()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(raw_size.checked_mul(3)?))
            .ok_or(crate::item_budget_origin!())?;
        set_direct_item_temporary_state(
            issues,
            *owner_index_state_bytes,
            owner_temporary_state_bytes,
            owner_peak_state_bytes,
            owner_state_limit,
            path_temporary_state_bytes
                .checked_add(actual_fixity_state_bytes)
                .ok_or(crate::item_budget_origin!())?,
        )?;
        let member = source.read_member(
            &fixity_path,
            limits.items.max_member_bytes,
            direct_item_source_state_bytes(schema_request_state_bytes, issues)?,
            limits.items.max_state_bytes,
            limits.items.deadline,
            cancelled,
        )?;
        let text = String::from_utf8(member.raw)
            .map_err(|_| ItemRefusal::Unsupported("Item fixity UTF-8 decode".into()))?;
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        actual_fixity_state_bytes = std::mem::size_of::<String>()
            .checked_add(normalized.len())
            .ok_or(crate::item_budget_origin!())?;
        set_direct_item_temporary_state(
            issues,
            *owner_index_state_bytes,
            owner_temporary_state_bytes,
            owner_peak_state_bytes,
            owner_state_limit,
            path_temporary_state_bytes
                .checked_add(actual_fixity_state_bytes)
                .ok_or(crate::item_budget_origin!())?,
        )?;
        Some(normalized)
    } else {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            &fixity_path,
            "fixity companion is missing",
        )?;
        None
    };

    let mut expected_fixity = String::new();
    let expected_fixity_base_state_bytes = path_temporary_state_bytes
        .checked_add(actual_fixity_state_bytes)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>()))
        .ok_or(crate::item_budget_origin!())?;
    set_direct_item_temporary_state(
        issues,
        *owner_index_state_bytes,
        owner_temporary_state_bytes,
        owner_peak_state_bytes,
        owner_state_limit,
        expected_fixity_base_state_bytes,
    )?;
    if let Some(payload_files) = manifest.get("payload_files").and_then(Value::as_array) {
        for entry in payload_files.iter().filter(|entry| entry.is_object()) {
            check(limits.items, cancelled)?;
            let digest_value = entry.get("sha256").unwrap_or(&Value::Null);
            let relative_value = entry.get("relative_path").unwrap_or(&Value::Null);
            let digest_len = python_value_string_len(digest_value)?;
            let relative_len = python_value_string_len(relative_value)?;
            let expected_fixity_len = expected_fixity
                .len()
                .checked_add(digest_len)
                .and_then(|bytes| bytes.checked_add(2))
                .and_then(|bytes| bytes.checked_add(relative_len))
                .and_then(|bytes| bytes.checked_add(1))
                .ok_or(crate::item_budget_origin!())?;
            let entry_temporary_state = expected_fixity_base_state_bytes
                .checked_add(expected_fixity_len)
                .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<String>()))
                .and_then(|bytes| bytes.checked_add(digest_len))
                .and_then(|bytes| bytes.checked_add(relative_len))
                .ok_or(crate::item_budget_origin!())?;
            set_direct_item_temporary_state(
                issues,
                *owner_index_state_bytes,
                owner_temporary_state_bytes,
                owner_peak_state_bytes,
                owner_state_limit,
                entry_temporary_state,
            )?;
            let digest = python_value_string(digest_value);
            let relative_repr = python_value_string(relative_value);
            write!(expected_fixity, "{digest}  {relative_repr}\n")
                .map_err(|_| crate::item_budget_origin!())?;
            drop(digest);
            drop(relative_repr);
            set_direct_item_temporary_state(
                issues,
                *owner_index_state_bytes,
                owner_temporary_state_bytes,
                owner_peak_state_bytes,
                owner_state_limit,
                expected_fixity_base_state_bytes
                    .checked_add(expected_fixity.len())
                    .ok_or(crate::item_budget_origin!())?,
            )?;
            let file_id = entry.get("file_id").unwrap_or(&Value::Null);
            if let Some(file_id) = file_id.as_str() {
                while *membership_index < membership_issues.len() {
                    let issue = &membership_issues[*membership_index];
                    if issue.location != *manifest_path
                        || !membership_issue_for_file_id(&issue.message, file_id)
                    {
                        break;
                    }
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        &issue.location,
                        &issue.message,
                    )?;
                    *membership_index += 1;
                }
            }
            let Some(relative) = relative_value.as_str() else {
                issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    manifest_path,
                    "payload relative_path is not a string",
                )?;
                continue;
            };
            let payload_path_state_bytes = expected_fixity_base_state_bytes
                .checked_add(expected_fixity.len())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>()))
                .and_then(|bytes| bytes.checked_add(item_directory.len()))
                .and_then(|bytes| bytes.checked_add(relative.len()))
                .and_then(|bytes| bytes.checked_add(1))
                .ok_or(crate::item_budget_origin!())?;
            set_direct_item_temporary_state(
                issues,
                *owner_index_state_bytes,
                owner_temporary_state_bytes,
                owner_peak_state_bytes,
                owner_state_limit,
                payload_path_state_bytes,
            )?;
            let payload_path = format!("{item_directory}/{relative}");
            let Some(facts) = physical.payloads.get(&payload_path) else {
                return Err(ItemRefusal::Unsupported(format!(
                    "source-foundation physical payload observation is unavailable: {payload_path}"
                )));
            };
            if !facts.exists || !facts.regular_file || facts.symlink {
                if require_local_payloads {
                    issues.push(
                        SourceFoundationRecordsIssueFamily::Item,
                        &payload_path,
                        "required local payload is missing or is a symlink",
                    )?;
                }
                drop(payload_path);
                set_direct_item_temporary_state(
                    issues,
                    *owner_index_state_bytes,
                    owner_temporary_state_bytes,
                    owner_peak_state_bytes,
                    owner_state_limit,
                    expected_fixity_base_state_bytes
                        .checked_add(expected_fixity.len())
                        .ok_or(crate::item_budget_origin!())?,
                )?;
                continue;
            }
            let actual_size = facts.byte_size.ok_or_else(|| {
                ItemRefusal::Unsupported(format!(
                    "source-foundation physical payload size is unavailable: {payload_path}"
                ))
            })?;
            let actual_digest = facts.sha256.as_deref().ok_or_else(|| {
                ItemRefusal::Unsupported(format!(
                    "source-foundation physical payload digest is unavailable: {payload_path}"
                ))
            })?;
            let expected_size = entry.get("byte_size").unwrap_or(&Value::Null);
            let expected_digest = entry.get("sha256").unwrap_or(&Value::Null);
            if !python_values_equal(expected_size, &Value::from(actual_size)) {
                issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    &payload_path,
                    &format!(
                        "byte size {actual_size} != manifest {}",
                        python_value_string(expected_size)
                    ),
                )?;
            }
            if expected_digest.as_str() != Some(actual_digest) {
                issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    &payload_path,
                    &format!(
                        "sha256 {actual_digest} != manifest {}",
                        python_value_string(expected_digest)
                    ),
                )?;
            }
            match facts.git_ignored {
                Some(false) => issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    &payload_path,
                    "local payload is not ignored by Git",
                )?,
                None if physical.git_available == Some(true) => issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    &payload_path,
                    "could not determine Git ignore posture",
                )?,
                _ => {}
            }
            drop(payload_path);
            set_direct_item_temporary_state(
                issues,
                *owner_index_state_bytes,
                owner_temporary_state_bytes,
                owner_peak_state_bytes,
                owner_state_limit,
                expected_fixity_base_state_bytes
                    .checked_add(expected_fixity.len())
                    .ok_or(crate::item_budget_origin!())?,
            )?;
        }
    }
    if actual_fixity
        .as_ref()
        .is_some_and(|actual| !actual.is_empty() && actual != &expected_fixity)
    {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            &fixity_path,
            "fixity companion differs from item manifest",
        )?;
    }
    drop(expected_fixity);
    drop(actual_fixity);
    drop(item_directory);
    drop(fixity_relative);
    drop(fixity_path);
    set_direct_item_temporary_state(
        issues,
        *owner_index_state_bytes,
        owner_temporary_state_bytes,
        owner_peak_state_bytes,
        owner_state_limit,
        0,
    )?;

    if let Some(rights_index) = context.rights_index {
        let rights_request = &schema_checks[rights_index];
        if let Some(scopes) = rights_request
            .decoded_instance
            .as_ref()
            .and_then(|rights| rights.get("scope_refs").and_then(Value::as_array))
        {
            let mut missing_message = String::from("scope_refs omits payload files: [");
            set_direct_item_temporary_state(
                issues,
                *owner_index_state_bytes,
                owner_temporary_state_bytes,
                owner_peak_state_bytes,
                owner_state_limit,
                std::mem::size_of::<String>()
                    .checked_add(missing_message.len())
                    .ok_or(crate::item_budget_origin!())?,
            )?;
            let mut previous_missing: Option<&str> = None;
            let mut missing_count = 0usize;
            loop {
                check(limits.items, cancelled)?;
                let next_missing = manifest
                    .get("payload_files")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| entry.get("file_id").and_then(Value::as_str))
                    .filter(|file_id| {
                        previous_missing.is_none_or(|previous| *file_id > previous)
                            && !scopes.iter().any(|scope| scope.as_str() == Some(*file_id))
                    })
                    .min();
                let Some(file_id) = next_missing else {
                    break;
                };
                let repr_len = python_string_repr_len(file_id)?;
                let separator_len = usize::from(missing_count > 0) * 2;
                let next_temporary_state = std::mem::size_of::<String>()
                    .checked_add(missing_message.len())
                    .and_then(|bytes| bytes.checked_add(separator_len))
                    .and_then(|bytes| bytes.checked_add(repr_len))
                    .and_then(|bytes| bytes.checked_add(1))
                    .ok_or(crate::item_budget_origin!())?;
                set_direct_item_temporary_state(
                    issues,
                    *owner_index_state_bytes,
                    owner_temporary_state_bytes,
                    owner_peak_state_bytes,
                    owner_state_limit,
                    next_temporary_state,
                )?;
                if missing_count > 0 {
                    missing_message.push_str(", ");
                }
                write_python_string_repr(&mut missing_message, file_id);
                missing_count = missing_count
                    .checked_add(1)
                    .ok_or(crate::item_budget_origin!())?;
                previous_missing = Some(file_id);
            }
            if missing_count > 0 {
                missing_message.push(']');
                issues.push(
                    SourceFoundationRecordsIssueFamily::Item,
                    &rights_request.location,
                    &missing_message,
                )?;
            }
        }
    }
    set_direct_item_temporary_state(
        issues,
        *owner_index_state_bytes,
        owner_temporary_state_bytes,
        owner_peak_state_bytes,
        owner_state_limit,
        0,
    )?;
    Ok(())
}

fn append_legacy_inventory_provenance_issue(
    source: SourceFoundationCurrentInput<'_>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    schema_checks: &[SourceFoundationRecordsSchemaCheck],
    inventory_index: usize,
    manifest_index: usize,
    event_end_index: usize,
    inventory_ref: &tos_foundation::JsonValue,
    direct_read_bytes: &mut u64,
    direct_read_limit: u64,
    prior_state_bytes: usize,
    max_state_bytes: usize,
    issues: &mut DirectIssueBuffer<'_>,
) -> Result<(), ItemRefusal> {
    let event_id = match inventory_ref {
        tos_foundation::JsonValue::String(value) => Some(value.as_str().ok_or_else(|| {
            ItemRefusal::Unsupported(
                "legacy inventory provenance reference contains a lone surrogate".into(),
            )
        })?),
        tos_foundation::JsonValue::Array(_) | tos_foundation::JsonValue::Object(_) => {
            return Err(ItemRefusal::Unsupported(
                "legacy inventory provenance lookup uses an unhashable reference".into(),
            ));
        }
        _ => None,
    };
    let mut event = None;
    if let Some(inventory_event_id) = event_id {
        for event_index in manifest_index.saturating_add(1)..event_end_index {
            check(limits, cancelled)?;
            let event_check = &schema_checks[event_index];
            if event_check.contract == "ToS/contracts/provenance-event.schema.json"
                && event_check
                    .decoded_instance
                    .as_ref()
                    .and_then(|event| event.get("event_id"))
                    .and_then(Value::as_str)
                    == Some(inventory_event_id)
            {
                event = event_check.decoded_instance.as_ref();
            }
        }
    }
    let inventory_request = &schema_checks[inventory_index];
    if let Some(event) = event {
        let digest = source
            .member_digest_with_accounting(
                &inventory_request.location,
                limits.max_member_bytes,
                Some((direct_read_bytes, direct_read_limit)),
                prior_state_bytes,
                max_state_bytes,
                limits.deadline,
                cancelled,
            )?
            .map(|digest| digest.to_hex())
            .unwrap_or_default();
        let expected_output = serde_json::json!({
            "ref": inventory_request.location,
            "role": "tracked_text_free_resource_inventory",
            "sha256": digest,
        });
        let has_output = event
            .get("outputs")
            .and_then(Value::as_array)
            .is_some_and(|outputs| {
                outputs.iter().any(|output| {
                    output.as_object().is_some_and(|object| {
                        object.len() == 3 && python_values_equal(output, &expected_output)
                    })
                })
            });
        if !has_output {
            issues.push(
                SourceFoundationRecordsIssueFamily::Item,
                &inventory_request.location,
                "resource inventory provenance event lacks its digest-bound output",
            )?;
        }
    } else {
        issues.push(
            SourceFoundationRecordsIssueFamily::Item,
            &inventory_request.location,
            &format!(
                "provenance_event_ref is absent from item provenance: {}",
                legacy_python_display(inventory_ref)?
            ),
        )?;
    }
    Ok(())
}

fn current_file_presence(
    source: SourceFoundationCurrentInput<'_>,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<SourcePresenceV1>, ItemRefusal> {
    source.presence(path, deadline, cancelled)
}

fn direct_item_source_state_bytes(
    schema_request_state_bytes: &usize,
    issues: &DirectIssueBuffer<'_>,
) -> Result<usize, ItemRefusal> {
    schema_request_state_bytes
        .checked_add(issues.state_bytes)
        .ok_or(crate::item_budget_origin!())
}

fn account_direct_item_read(
    size_bytes: u64,
    used: &mut u64,
    limit: u64,
    max_member_bytes: usize,
    check_name: &'static str,
) -> Result<(), ItemRefusal> {
    if size_bytes > max_member_bytes as u64 {
        return Err(ItemRefusal::BudgetCheck {
            check: check_name,
            used: Some(size_bytes),
            limit: Some(max_member_bytes as u64),
        });
    }
    *used = used
        .checked_add(size_bytes)
        .filter(|used| *used <= limit)
        .ok_or(ItemRefusal::BudgetCheck {
            check: check_name,
            used: used.checked_add(size_bytes),
            limit: Some(limit),
        })?;
    Ok(())
}

fn membership_issue_for_file_id(message: &str, file_id: &str) -> bool {
    message.strip_prefix("file_id ").is_some_and(|suffix| {
        suffix.starts_with(file_id) && suffix.as_bytes().get(file_id.len()) == Some(&b' ')
    })
}

pub(crate) fn python_value_string_len(value: &Value) -> Result<usize, ItemRefusal> {
    match value {
        Value::Null => Ok(4),
        Value::Bool(true) => Ok(4),
        Value::Bool(false) => Ok(5),
        Value::String(text) => Ok(text.len()),
        Value::Number(number) => Ok(number.to_string().len()),
        Value::Array(values) => {
            let mut length = 2usize;
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    length = length.checked_add(2).ok_or(crate::item_budget_origin!())?;
                }
                length = length
                    .checked_add(python_value_repr_len(value)?)
                    .ok_or(crate::item_budget_origin!())?;
            }
            Ok(length)
        }
        Value::Object(values) => {
            let mut length = 2usize;
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    length = length.checked_add(2).ok_or(crate::item_budget_origin!())?;
                }
                let key_len = python_string_repr_len(key)?;
                let value_len = python_value_repr_len(value)?;
                length = length
                    .checked_add(key_len)
                    .and_then(|bytes| bytes.checked_add(2))
                    .and_then(|bytes| bytes.checked_add(value_len))
                    .ok_or(crate::item_budget_origin!())?;
            }
            Ok(length)
        }
    }
}

fn python_value_repr_len(value: &Value) -> Result<usize, ItemRefusal> {
    if let Value::String(text) = value {
        python_string_repr_len(text)
    } else {
        python_value_string_len(value)
    }
}

fn python_string_repr_len(value: &str) -> Result<usize, ItemRefusal> {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut length = 2usize;
    for character in value.chars() {
        let additional = match character {
            '\\' | '\n' | '\r' | '\t' => 2,
            value if value == quote => 2,
            value if value.is_control() => 2usize
                .checked_add(format!("{:04x}", value as u32).len())
                .ok_or(crate::item_budget_origin!())?,
            value => value.len_utf8(),
        };
        length = length
            .checked_add(additional)
            .ok_or(crate::item_budget_origin!())?;
    }
    Ok(length)
}

fn write_python_string_repr(output: &mut String, value: &str) {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    output.push(quote);
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value == quote => {
                output.push('\\');
                output.push(value);
            }
            value if value.is_control() => {
                let _ = write!(output, "\\u{:04x}", value as u32);
            }
            value => output.push(value),
        }
    }
    output.push(quote);
}

fn python_values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Bool(left), Value::Number(right)) | (Value::Number(right), Value::Bool(left)) => {
            right.as_f64() == Some(u8::from(*left) as f64)
        }
        (Value::Number(left), Value::Number(right)) => {
            left == right
                || left
                    .as_f64()
                    .zip(right.as_f64())
                    .is_some_and(|(a, b)| a == b)
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| python_values_equal(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| python_values_equal(left, right))
                })
        }
        _ => left == right,
    }
}

fn build_file_membership_index(
    source: SourceFoundationCurrentInput<'_>,
    selected_items: &[SelectedItemRecord],
    limits: ItemLimits,
    max_read_bytes: u64,
    cancelled: &AtomicBool,
    store: Option<SourceFoundationRecordsStoreHandle<'_>>,
) -> Result<
    (
        SourceFileMembershipIndex,
        Vec<SourceFoundationRecordsIssue>,
        u64,
        usize,
    ),
    ItemRefusal,
> {
    let mut index = SourceFileMembershipIndex::default();
    let mut issues = Vec::new();
    let mut read_bytes = 0u64;
    let mut state_bytes = std::mem::size_of::<SourceFileMembershipIndex>();
    if state_bytes > limits.max_state_bytes {
        return Err(crate::item_budget_origin!());
    }

    let (manifest_paths, manifest_paths_state_bytes) = current_source_paths_matching(
        source,
        SOURCE_HOME,
        None,
        None,
        Some(ITEM_MANIFEST_SUFFIX),
        state_bytes,
        limits.max_state_bytes,
        limits.deadline,
        cancelled,
    )?;
    for (path_owned, metadata_size_bytes) in manifest_paths {
        check(limits, cancelled)?;
        let path = path_owned.as_str();
        if !item_manifest_in_scope(source.selection().is_some(), selected_items, path) {
            continue;
        }
        if metadata_size_bytes > limits.max_member_bytes as u64 {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation item manifest membership member bytes",
                used: Some(metadata_size_bytes),
                limit: Some(limits.max_member_bytes as u64),
            });
        }
        let next_read = read_bytes
            .checked_add(metadata_size_bytes)
            .filter(|used| *used <= max_read_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation item manifest membership reads",
                used: read_bytes.checked_add(metadata_size_bytes),
                limit: Some(max_read_bytes),
            })?;
        let member = source.read_member(
            path,
            limits.max_member_bytes,
            state_bytes
                .checked_add(manifest_paths_state_bytes)
                .ok_or(crate::item_budget_origin!())?,
            limits.max_state_bytes,
            limits.deadline,
            cancelled,
        )?;
        if member.size_bytes != metadata_size_bytes {
            return Err(ItemRefusal::Source(
                "Item manifest size changed after metadata traversal".into(),
            ));
        }
        read_bytes = next_read;
        let available = limits
            .max_state_bytes
            .checked_sub(state_bytes)
            .and_then(|remaining| remaining.checked_sub(manifest_paths_state_bytes))
            .and_then(|remaining| remaining.checked_sub(member.raw.len()))
            .ok_or(crate::item_budget_origin!())?;
        let value = match bounded_native_value(&member.raw, limits, available, cancelled) {
            Ok(value) => value,
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                match legacy_parse_failure(&member.raw, limits, available, cancelled)? {
                    LegacyJsonParseFailure::Malformed => continue,
                    LegacyJsonParseFailure::Nonfinite => {
                        return Err(ItemRefusal::Unsupported(
                            "source-foundation nonfinite legacy JSON cannot be represented by serde_json::Value".into(),
                        ));
                    }
                }
            }
            Err(error) => return Err(error),
        };
        let Some(object) = value.as_object() else {
            continue;
        };
        let item_id = object.get("item_id").unwrap_or(&Value::Null);
        let Some(payload_files) = object.get("payload_files").and_then(Value::as_array) else {
            continue;
        };
        for entry in payload_files.iter().filter(|entry| entry.is_object()) {
            check(limits, cancelled)?;
            let file_id = entry.get("file_id").unwrap_or(&Value::Null);
            let existed = index.contains(item_id, file_id);
            let descriptor_existed = index.sha256_for(file_id).is_some();
            let sha256 = entry.get("sha256").unwrap_or(&Value::Null);
            let byte_size = entry.get("byte_size").unwrap_or(&Value::Null);
            let media_type = entry.get("media_type").unwrap_or(&Value::Null);
            let conflicts = index
                .add(item_id, file_id, sha256, byte_size, media_type)
                .map_err(|error| {
                    ItemRefusal::Unsupported(format!(
                        "source-foundation File descriptor equality: {error:?}"
                    ))
                })?;
            if let (Some(item), Some(file)) = (item_id.as_str(), file_id.as_str()) {
                if let Some(store) = &store {
                    if !descriptor_existed {
                        store.with(|store| {
                            store.file_descriptor_first(file, sha256, byte_size, media_type)
                        })?;
                    }
                    if !existed {
                        store.with(|store| store.item_file_membership(file, item))?;
                    }
                }
                if !existed {
                    state_bytes = state_bytes
                        .checked_add(item.len())
                        .and_then(|bytes| bytes.checked_add(file.len()))
                        .and_then(|bytes| {
                            bytes.checked_add(
                                std::mem::size_of::<String>()
                                    + std::mem::size_of::<BTreeMap<String, ()>>(),
                            )
                        })
                        .ok_or(crate::item_budget_origin!())?;
                }
                if !descriptor_existed {
                    let descriptor_bytes = [sha256, byte_size, media_type].into_iter().try_fold(
                        file.len() + std::mem::size_of::<[Value; 3]>(),
                        |used, value| {
                            used.checked_add(crate::record_biblio_cut::decoded_state(value)?)
                                .ok_or(crate::item_budget_origin!())
                        },
                    )?;
                    state_bytes = state_bytes
                        .checked_add(descriptor_bytes)
                        .ok_or(crate::item_budget_origin!())?;
                }
            }
            if !conflicts.is_empty() {
                if issues.len() >= limits.max_issues {
                    return Err(crate::item_budget_origin!());
                }
                let file_label = file_id.as_str().unwrap_or("None");
                issues.push(SourceFoundationRecordsIssue {
                    family: SourceFoundationRecordsIssueFamily::Item,
                    location: path.to_owned(),
                    message: format!(
                        "file_id {file_label} has conflicting File identity fields: {}",
                        conflicts.join(", ")
                    ),
                });
            }
            if state_bytes
                .checked_add(manifest_paths_state_bytes)
                .is_none_or(|used| used > limits.max_state_bytes)
            {
                return Err(crate::item_budget_origin!());
            }
        }
        drop(value);
    }
    Ok((index, issues, read_bytes, state_bytes))
}

fn bounded_native_value(
    raw: &[u8],
    limits: ItemLimits,
    available: usize,
    cancelled: &AtomicBool,
) -> Result<Value, ItemRefusal> {
    let json_limits = tos_foundation::JsonLimits::new(
        limits.max_member_bytes,
        128,
        available.max(1),
        limits.max_member_bytes.max(1),
    )
    .map_err(|_| crate::item_budget_origin!())?;
    crate::record_biblio_cut::bounded_legacy_item_decoded_state(
        raw,
        json_limits,
        available,
        limits.deadline,
        cancelled,
    )
    .map(|(value, _)| value)
}

/// Check the maintained current record/Item district over one caller-captured
/// immutable cut. `expected_revision` and `expected_membership` must come from
/// the independent source-capture owner; they are checked against the record
/// family's complete EOF result before Item traversal begins.
///
/// The record family uses the bounded `inspect_records_from_cut` route, which
/// reconstructs record identity from exact current bytes. The Item family then
/// uses that report's current record map for typed Item/Edition lookups and
/// runs the existing `ItemRules` mechanics over the same current cut.
///
/// This function is current-only. It refuses cuts with retained base revisions
/// because the maintained Python foundation route checks the selected current
/// publication, not historical record profiles. It does not assert that this
/// district covers every normative source profile or companion.
/// `require_local_payloads` is an explicit caller posture: metadata-only mode
/// may report unavailable payloads, while local mode requires custody checks.
/// Physical payload, Git tracking and ignore evidence comes from the explicit
/// host-owned `SourcePhysicalFacts` input; the authenticated source cut does
/// not stand in for any of those observations.
pub fn inspect_source_foundation_records_from_cut(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    limits: SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_executor: &mut BiblioRecordExecutor,
    item_schemas: &mut CutWorkerSchemaExecutor,
    physical_facts: &SourcePhysicalFacts,
    payloads: &mut impl CutPayloadReader,
) -> Result<SourceFoundationRecordsReport, ItemRefusal> {
    if item_schemas.source_revision() != expected_revision {
        return Err(ItemRefusal::Source(
            "source-foundation Item schema worker belongs to another cut".into(),
        ));
    }
    let schema_cost = schema_resource_cost(cut, item_schemas)?;
    let core = inspect_source_foundation_records_with_mode(
        SourceFoundationCurrentInput::Cut {
            cut,
            revision: expected_revision,
        },
        Some(expected_revision),
        expected_membership,
        limits,
        require_local_payloads,
        cancelled,
        record_executor,
        item_schemas,
        physical_facts,
        payloads,
        false,
        schema_cost,
        None,
        None,
    )?;
    resident_source_foundation_report(core, expected_revision)
}

/// Additive rolling Records→Item route. It preserves the fixed entry above,
/// but assigns Item the operation capacity left after the completed Records
/// pass's actual read, issue and logical-state observations.
pub fn inspect_source_foundation_records_from_cut_rolling(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    limits: SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_executor: &mut BiblioRecordExecutor,
    item_schemas: &mut CutWorkerSchemaExecutor,
    physical_facts: &SourcePhysicalFacts,
    payloads: &mut impl CutPayloadReader,
) -> Result<SourceFoundationRecordsReport, ItemRefusal> {
    if item_schemas.source_revision() != expected_revision {
        return Err(ItemRefusal::Source(
            "source-foundation Item schema worker belongs to another cut".into(),
        ));
    }
    let schema_cost = schema_resource_cost(cut, item_schemas)?;
    let core = inspect_source_foundation_records_with_mode(
        SourceFoundationCurrentInput::Cut {
            cut,
            revision: expected_revision,
        },
        Some(expected_revision),
        expected_membership,
        limits,
        require_local_payloads,
        cancelled,
        record_executor,
        item_schemas,
        physical_facts,
        payloads,
        true,
        schema_cost,
        None,
        None,
    )?;
    resident_source_foundation_report(core, expected_revision)
}

/// Additive rolling entry that persists Records and Item owner facts through a
/// caller-owned bounded store while the existing source kernel discovers
/// them. The returned index borrows that same store; it cannot outlive the
/// candidate scope that owns it.
pub fn inspect_source_foundation_records_from_cut_rolling_stored<'a>(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    limits: SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_executor: &mut BiblioRecordExecutor,
    item_schemas: &mut CutWorkerSchemaExecutor,
    physical_facts: &SourcePhysicalFacts,
    payloads: &mut impl CutPayloadReader,
    store: &'a mut dyn SourceFoundationRecordsStore,
) -> Result<SourceFoundationRecordsStreamedReport<'a, SourceRevision>, ItemRefusal> {
    if item_schemas.source_revision() != expected_revision {
        return Err(ItemRefusal::Source(
            "source-foundation Item schema worker belongs to another cut".into(),
        ));
    }
    let schema_cost = schema_resource_cost(cut, item_schemas)?;
    let report = inspect_source_foundation_records_with_mode(
        SourceFoundationCurrentInput::Cut {
            cut,
            revision: expected_revision,
        },
        Some(expected_revision),
        expected_membership,
        limits,
        require_local_payloads,
        cancelled,
        record_executor,
        item_schemas,
        physical_facts,
        payloads,
        true,
        schema_cost,
        None,
        Some(&mut *store),
    )?;
    let record_usage = report.record_usage.ok_or(crate::item_budget_origin!())?;
    let items = streamed_item_summary(&report.items);
    Ok(SourceFoundationRecordsStreamedReport::new_completed(
        expected_revision,
        report.source_membership,
        record_usage,
        items,
        report.cost,
        None,
        store,
    ))
}

fn source_refusal_origin(error: ItemRefusal, stage: &'static str) -> ItemRefusal {
    match error {
        // This district composes several owners. If an older owner still
        // supplies only its budget class, retain the actual dispatch stage
        // without fabricating a guard counter or replacing a richer refusal.
        ItemRefusal::Budget => ItemRefusal::BudgetCheck {
            check: stage,
            used: None,
            limit: None,
        },
        ItemRefusal::Source(reason) if !reason.starts_with("source-cause:") => {
            // Complete formatted-Source constructor shapes in this receiver.
            // Context values remain private; the fixed originating operation
            // and full cause digest are enough to match the owned source/input.
            let origin = if reason.starts_with("Biblio adapter omitted required member: ") {
                "biblio-adapter-member-missing"
            } else if reason.starts_with("Biblio ")
                && reason.ends_with(" fact cursor coverage differs from emitted observations")
            {
                "biblio-fact-coverage"
            } else if reason
                .starts_with("source-foundation current member missing from captured cut: ")
            {
                "current-member-missing"
            } else if reason.starts_with("Item schema is absent from captured cut: ") {
                "item-schema-absent-cut"
            } else if reason.starts_with("Item schema worker does not use captured bytes: ") {
                "item-schema-worker-not-captured"
            } else if reason
                .starts_with("Item schema worker resource differs from captured bytes: ")
            {
                "item-schema-worker-resource-differs"
            } else if reason.starts_with("selected Item schema is absent from candidate input: ") {
                "selected-item-schema-absent"
            } else if reason
                .starts_with("candidate schema binding differs from selected source resource: ")
            {
                "candidate-schema-resource-binding"
            } else if reason.starts_with("selected Item schema disappeared during digest read: ") {
                "selected-item-schema-disappeared"
            } else if reason.starts_with("candidate schema bytes differ from selected digest: ") {
                "candidate-schema-digest-differs"
            } else if reason.starts_with("record member missing from captured cut: ") {
                "record-member-missing"
            } else if reason
                .starts_with("source-foundation Item member missing from captured cut: ")
            {
                "item-member-missing"
            } else if reason
                .starts_with("source-foundation payload reader disagrees with physical facts: ")
            {
                "payload-reader-facts-disagree"
            } else if reason
                .starts_with("source-foundation payload reader differs from physical facts: ")
            {
                "payload-reader-facts-differ"
            } else {
                stage
            };
            ItemRefusal::Source(format!(
                "source-cause:receiver-source:{origin}:{}",
                Digest256::of_bytes(reason.as_bytes()).to_hex()
            ))
        }
        other => other,
    }
}

/// Candidate-fenced, current-only Records+Item receiver. The command adapter
/// supplies one immutable input view, its matching prepared schema binding,
/// and the already-owned bounded store. The returned identity remains opaque
/// and is never translated into a `SourceRevision`.
#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_records_from_input_stored<'a, I: Eq + Clone>(
    input: &dyn SourceCutInputWithIdentity<I>,
    schema_binding: &mut impl SourceFoundationCandidateSchemaBinding<I>,
    limits: SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_executor: &mut BiblioRecordExecutor,
    physical_facts: &SourcePhysicalFacts,
    payloads: &mut impl CutPayloadReader,
    fact_budget: SourceCutRecordFactBudget,
    page_budget: SourceFoundationRecordsPageBudget,
    store: &'a mut dyn SourceFoundationRecordsStore,
) -> Result<SourceFoundationRecordsStreamedReport<'a, I>, ItemRefusal> {
    check(limits.operation, cancelled)?;
    validate_rolling_limits(limits)?;
    if input.input_identity() != schema_binding.input_identity() {
        return Err(ItemRefusal::Source(
            "candidate Item schema binding has another source input identity".into(),
        ));
    }
    let prepared = schema_binding.prepared_execution_binding();
    if prepared.schema_profile != schema_binding.profile()
        || prepared.schema_set_sha256 != schema_binding.schema_set_digest()
        || prepared.schema_profile != record_executor.profile
    {
        return Err(ItemRefusal::Source(
            "candidate Item schema worker binding differs from the prepared profile or set".into(),
        ));
    }

    let source_input = input.source_input();
    let source = SourceFoundationCurrentInput::Stream(source_input);
    let report_header_state_bytes = std::mem::size_of::<SourceFoundationRecordsReport>();
    let record_state_ceiling = limits
        .operation
        .max_state_bytes
        .checked_sub(report_header_state_bytes)
        .ok_or(crate::item_budget_origin!())?;
    let mut record_limits = limits.records;
    record_limits.max_state_bytes = record_limits.max_state_bytes.min(record_state_ceiling);
    record_limits.max_total_bytes = record_limits
        .max_total_bytes
        .min(limits.operation.max_total_bytes);
    record_limits.max_issues = record_limits.max_issues.min(limits.operation.max_issues);

    let schema_cost = candidate_schema_resource_cost(
        source,
        schema_binding,
        limits.items,
        limits.max_schema_resource_bytes,
        report_header_state_bytes,
        cancelled,
    )
    .map_err(|error| source_refusal_origin(error, "candidate-schema-cost"))?;
    let record_summary = inspect_records_from_input_stored(
        source_input,
        store,
        record_limits,
        fact_budget,
        page_budget,
        cancelled,
        record_executor,
    )
    .map_err(|error| source_refusal_origin(error, "biblio-record-scan"))?;
    if record_summary.current_member_count() != record_summary.input_coverage().member_count()
        || record_summary.current_source_bytes()
            != record_summary.input_coverage().source_bytes_read()
        || record_summary.current_membership() != &record_summary.input_coverage().membership()
    {
        return Err(ItemRefusal::Source(
            "candidate Records summary differs from its verified source coverage".into(),
        ));
    }

    let candidate_schema_identity = SourceFoundationCandidateSchemaIdentity::new(
        prepared,
        schema_binding.contract_selection_digest(),
        schema_cost.item_resource_count,
        schema_cost.items,
    );
    let input_identity = input.input_identity().clone();
    let source_membership = *record_summary.current_membership();
    let core = inspect_source_foundation_records_with_mode(
        source,
        None,
        source_membership,
        limits,
        require_local_payloads,
        cancelled,
        record_executor,
        schema_binding,
        physical_facts,
        payloads,
        true,
        schema_cost,
        Some(&record_summary),
        Some(&mut *store),
    )
    .map_err(|error| source_refusal_origin(error, "records-item-core"))?;
    let record_usage = core.record_usage.ok_or(crate::item_budget_origin!())?;
    let items = streamed_item_summary(&core.items);
    Ok(SourceFoundationRecordsStreamedReport::new_completed(
        input_identity,
        core.source_membership,
        record_usage,
        items,
        core.cost,
        Some(candidate_schema_identity),
        store,
    ))
}

fn resident_source_foundation_report(
    report: SourceFoundationRecordsCoreReport,
    source_revision: SourceRevision,
) -> Result<SourceFoundationRecordsReport, ItemRefusal> {
    Ok(SourceFoundationRecordsReport {
        source_revision,
        source_membership: report.source_membership,
        records: report.records.ok_or_else(|| {
            ItemRefusal::Source("resident source-foundation report omitted Records output".into())
        })?,
        items: report.items,
        current_records: report.current_records,
        current_record_order: report.current_record_order,
        used_declared_profile_kinds: report.used_declared_profile_kinds,
        item_records: report.item_records,
        record_schema_positions: report.record_schema_positions,
        file_memberships: report.file_memberships,
        rights_ids: report.rights_ids,
        claim_ids: report.claim_ids,
        item_editions: report.item_editions,
        source_event_insertions: report.source_event_insertions,
        ordered_issues: report.ordered_issues,
        schema_checks: report.schema_checks,
        unimplemented: report.unimplemented,
        cost: report.cost,
    })
}

fn streamed_item_summary(items: &ItemFamilyReport) -> SourceFoundationRecordsStreamedItemSummary {
    SourceFoundationRecordsStreamedItemSummary {
        issue_count: items.issues.len(),
        manifest_item_id_count: items.manifest_item_ids.len(),
        metadata_bytes: items.metadata_bytes,
        unavailable_payloads: items.unavailable_payloads,
        accounted_state_upper_bound_bytes: items.accounted_state_upper_bound_bytes,
        inventory_set_scan_steps: items.inventory_set_scan_steps,
        source_admission_complete: items.source_admission_complete,
    }
}

fn inspect_source_foundation_records_with_mode(
    source: SourceFoundationCurrentInput<'_>,
    expected_revision: Option<SourceRevision>,
    expected_membership: SourceMembershipV1,
    input_limits: SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_executor: &mut BiblioRecordExecutor,
    item_schemas: &mut impl CutSchemaExecutor,
    physical_facts: &SourcePhysicalFacts,
    payloads: &mut impl CutPayloadReader,
    rolling: bool,
    schema_cost: SchemaResourceCost,
    streamed_record_summary: Option<&SourceCutStreamedRecordSummary>,
    store: Option<&mut dyn SourceFoundationRecordsStore>,
) -> Result<SourceFoundationRecordsCoreReport, ItemRefusal> {
    let store = store.map(SourceFoundationRecordsStoreHandle::new);
    let original_limits = input_limits;
    let mut limits = input_limits;
    check(limits.operation, cancelled)?;
    let report_header_state_bytes = if rolling {
        std::mem::size_of::<SourceFoundationRecordsReport>()
    } else {
        0
    };
    if rolling {
        validate_rolling_limits(limits)?;
        let record_state_ceiling = limits
            .operation
            .max_state_bytes
            .checked_sub(report_header_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        limits.records.max_state_bytes = limits.records.max_state_bytes.min(record_state_ceiling);
        limits.records.max_total_bytes = limits
            .records
            .max_total_bytes
            .min(limits.operation.max_total_bytes);
        limits.records.max_issues = limits.records.max_issues.min(limits.operation.max_issues);
        limits.max_schema_request_state_bytes = limits
            .max_schema_request_state_bytes
            .min(limits.items.max_state_bytes)
            .min(record_state_ceiling);
    } else {
        validate_limits(limits)?;
    }
    match (&source, expected_revision, streamed_record_summary) {
        (SourceFoundationCurrentInput::Cut { cut, revision }, Some(expected), None) => {
            if *revision != expected || cut.current().revision() != expected {
                return Err(ItemRefusal::Source(
                    "source-foundation expected revision differs from captured cut".into(),
                ));
            }
            if cut.revisions().count() != 1 {
                return Err(ItemRefusal::Unsupported(
                    "source-foundation records district requires current-only cut".into(),
                ));
            }
        }
        (SourceFoundationCurrentInput::Stream(_), None, Some(summary)) => {
            if *summary.current_membership() != expected_membership {
                return Err(ItemRefusal::Source(
                    "source-foundation candidate Biblio membership changed".into(),
                ));
            }
        }
        _ => {
            return Err(ItemRefusal::Source(
                "source-foundation input mode lacks an exact source identity".into(),
            ));
        }
    }

    let selected_current_member_bytes = if let Some(summary) = streamed_record_summary {
        summary.current_source_bytes()
    } else {
        let mut total = 0u64;
        source.for_each_member_meta(limits.operation.deadline, cancelled, &mut |member| {
            total = total
                .checked_add(member.size_bytes)
                .ok_or(crate::item_budget_origin!())?;
            Ok(())
        })?;
        total
    };
    if schema_cost.aggregate > limits.max_schema_resource_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation aggregate schema resource bytes",
            used: Some(schema_cost.aggregate),
            limit: Some(limits.max_schema_resource_bytes),
        });
    }
    let record_registry_bytes = source
        .member_size(RECORD_REGISTRY, limits.records.deadline, cancelled)?
        .ok_or_else(|| {
            ItemRefusal::Source("source record registry absent from current input".into())
        })?;

    let direct_records = scan_direct_current_records(
        source,
        limits.records,
        limits.operation,
        limits.max_schema_request_state_bytes,
        cancelled,
        store.clone(),
    )?;
    let record_kernel_read_limit_bytes = limits
        .records
        .max_total_bytes
        .checked_sub(direct_records.read_bytes)
        .ok_or(crate::item_budget_origin!())?;
    let mut record_kernel_limits = limits.records;
    record_kernel_limits.max_total_bytes = record_kernel_read_limit_bytes;
    if rolling {
        let direct_pre_kernel_state = report_header_state_bytes
            .checked_add(direct_records.state_bytes)
            .and_then(|used| used.checked_add(direct_records.issues.state_bytes))
            .and_then(|used| used.checked_add(direct_records.schema_request_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        let kernel_state_remaining = limits
            .operation
            .max_state_bytes
            .checked_sub(direct_pre_kernel_state)
            .ok_or(crate::item_budget_origin!())?;
        let kernel_issue_remaining = limits
            .operation
            .max_issues
            .checked_sub(direct_records.issues.rows.len())
            .ok_or(crate::item_budget_origin!())?;
        record_kernel_limits.max_state_bytes = record_kernel_limits
            .max_state_bytes
            .min(kernel_state_remaining);
        record_kernel_limits.max_issues =
            record_kernel_limits.max_issues.min(kernel_issue_remaining);
        if record_kernel_limits.max_state_bytes == 0 || record_kernel_limits.max_issues == 0 {
            return Err(crate::item_budget_origin!());
        }
    }
    let (record_kernel_outcome, record_usage) = if let Some(summary) = streamed_record_summary {
        (None, Some(summary.usage().clone()))
    } else {
        let cut = source.cut().ok_or_else(|| {
            ItemRefusal::Source("resident Record kernel requires an anchored cut".into())
        })?;
        let expected_revision = expected_revision.ok_or_else(|| {
            ItemRefusal::Source("resident Record kernel has no SourceRevision".into())
        })?;
        let outcome = match inspect_records_from_cut(
            cut,
            record_kernel_limits,
            cancelled,
            record_executor,
        ) {
            Ok(records) => {
                if records.source_revision != expected_revision
                    || records.current_membership != expected_membership
                    || !records.retained_memberships.is_empty()
                {
                    return Err(ItemRefusal::Source(
                        "source-foundation record traversal differs from captured current membership"
                            .into(),
                    ));
                }
                SourceFoundationRecordKernelOutcome::Complete(records)
            }
            Err(refusal) if is_candidate_record_kernel_refusal(&refusal, &direct_records) => {
                SourceFoundationRecordKernelOutcome::Refused {
                    refusal,
                    cause: direct_records
                        .candidate_kernel_cause
                        .ok_or(crate::item_budget_origin!())?,
                }
            }
            Err(refusal) => return Err(refusal),
        };
        let usage = match &outcome {
            SourceFoundationRecordKernelOutcome::Complete(records) => Some(records.usage.clone()),
            SourceFoundationRecordKernelOutcome::Refused { refusal, .. } if rolling => {
                return Err(refusal.clone());
            }
            SourceFoundationRecordKernelOutcome::Refused { .. } => None,
        };
        (Some(outcome), usage)
    };
    let direct_record_issue_state_bytes = direct_records.issues.state_bytes;
    let direct_record_issue_count = direct_records.issues.rows.len();
    if direct_record_issue_state_bytes > limits.records.max_state_bytes
        || direct_record_issue_count > limits.records.max_issues
    {
        return Err(crate::item_budget_origin!());
    }
    let mut schema_request_state_limit_bytes = limits
        .max_schema_request_state_bytes
        .checked_sub(direct_record_issue_state_bytes)
        .ok_or(crate::item_budget_origin!())?;
    let mut schema_checks = direct_records.schema_checks;
    let mut schema_request_state_bytes = direct_records.schema_request_state_bytes;
    if schema_request_state_bytes > schema_request_state_limit_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation schema request vector state",
            used: Some(schema_request_state_bytes as u64),
            limit: Some(schema_request_state_limit_bytes as u64),
        });
    }
    let selected_items = direct_records.records;
    let record_owner_read_bytes = direct_records.read_bytes;
    let record_owner_state_bytes = direct_records.state_bytes;
    let item_index_state_bytes = selected_items.state_bytes;
    let mut rolling_record_observed_read_bytes = None;
    let mut rolling_record_issue_count = None;
    if rolling {
        let usage = record_usage.as_ref().ok_or(crate::item_budget_origin!())?;
        let record_read_bytes = usage
            .source_bytes_read
            .checked_add(record_owner_read_bytes)
            .ok_or(crate::item_budget_origin!())?;
        let record_issue_count = usage
            .observed_issue_count
            .checked_add(direct_record_issue_count)
            .ok_or(crate::item_budget_origin!())?;
        if record_issue_count > limits.operation.max_issues {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation Records observed issue count",
                used: Some(record_issue_count as u64),
                limit: Some(limits.operation.max_issues as u64),
            });
        }
        let record_accounted_state = report_header_state_bytes
            .checked_add(usage.accounted_state_upper_bound_bytes)
            .and_then(|used| used.checked_add(record_owner_state_bytes))
            .and_then(|used| used.checked_add(direct_record_issue_state_bytes))
            .and_then(|used| used.checked_add(schema_request_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        let remaining_item_state = limits
            .operation
            .max_state_bytes
            .checked_sub(record_accounted_state)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling Records retained state",
                used: Some(record_accounted_state as u64),
                limit: Some(limits.operation.max_state_bytes as u64),
            })?;
        let precharged_item_state = item_index_state_bytes
            .checked_add(direct_record_issue_state_bytes)
            .and_then(|used| used.checked_add(schema_request_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        limits.items.max_state_bytes = original_limits.items.max_state_bytes.min(
            remaining_item_state
                .checked_add(precharged_item_state)
                .ok_or(crate::item_budget_origin!())?,
        );
        if limits.items.max_state_bytes < precharged_item_state {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling Item precharged state",
                used: Some(precharged_item_state as u64),
                limit: Some(limits.items.max_state_bytes as u64),
            });
        }
        limits.max_schema_request_state_bytes = original_limits.max_schema_request_state_bytes.min(
            limits
                .items
                .max_state_bytes
                .checked_sub(item_index_state_bytes)
                .ok_or(crate::item_budget_origin!())?,
        );
        let precharged_auxiliary_state = direct_record_issue_state_bytes
            .checked_add(schema_request_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        if precharged_auxiliary_state > limits.max_schema_request_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling Item auxiliary precharge",
                used: Some(precharged_auxiliary_state as u64),
                limit: Some(limits.max_schema_request_state_bytes as u64),
            });
        }
        schema_request_state_limit_bytes = limits
            .max_schema_request_state_bytes
            .checked_sub(direct_record_issue_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        if schema_request_state_bytes > schema_request_state_limit_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling schema request precharge",
                used: Some(schema_request_state_bytes as u64),
                limit: Some(schema_request_state_limit_bytes as u64),
            });
        }
        let remaining_item_read = limits
            .operation
            .max_total_bytes
            .checked_sub(record_read_bytes)
            .ok_or(crate::item_budget_origin!())?;
        limits.items.max_total_bytes = original_limits
            .items
            .max_total_bytes
            .min(remaining_item_read);
        let remaining_item_issues = limits
            .operation
            .max_issues
            .checked_sub(record_issue_count)
            .ok_or(crate::item_budget_origin!())?;
        limits.items.max_issues = original_limits.items.max_issues.min(remaining_item_issues);
        rolling_record_observed_read_bytes = Some(record_read_bytes);
        rolling_record_issue_count = Some(usage.observed_issue_count);
    }
    let item_records = selected_items
        .records
        .iter()
        .map(|record| record.selection.path.as_str())
        .collect::<Vec<_>>();
    let item_record_read_bytes = item_records.iter().try_fold(0u64, |total, record| {
        let size_bytes = source
            .member_size(record, limits.items.deadline, cancelled)?
            .ok_or_else(|| ItemRefusal::Source("Item record missing from current input".into()))?;
        total
            .checked_add(size_bytes)
            .ok_or(crate::item_budget_origin!())
    })?;
    let item_rule_record_read_bytes = item_record_read_bytes;
    let item_scans_read_bytes = selected_items
        .read_bytes
        .checked_add(item_rule_record_read_bytes)
        .ok_or(crate::item_budget_origin!())?;
    let file_membership_read_limit_bytes = limits
        .items
        .max_total_bytes
        .checked_sub(item_scans_read_bytes)
        .ok_or(crate::item_budget_origin!())?;
    let (
        file_memberships,
        membership_issues,
        file_membership_read_bytes,
        file_membership_state_bytes,
    ) = build_file_membership_index(
        source,
        &selected_items.records,
        limits.items,
        file_membership_read_limit_bytes,
        cancelled,
        store.clone(),
    )?;
    if rolling {
        // The auxiliary ceiling is an exclusive reservation, not retained
        // usage. Split the remaining simultaneous workspace after both real
        // indexes are known, leaving a positive ItemRules share.
        let available_workspace = limits
            .items
            .max_state_bytes
            .checked_sub(item_index_state_bytes)
            .and_then(|bytes| bytes.checked_sub(file_membership_state_bytes))
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling Item indexes workspace",
                used: item_index_state_bytes
                    .checked_add(file_membership_state_bytes)
                    .map(|bytes| bytes as u64),
                limit: Some(limits.items.max_state_bytes as u64),
            })?;
        limits.max_schema_request_state_bytes = limits
            .max_schema_request_state_bytes
            .min(available_workspace / 2);
        let auxiliary_precharge =
            direct_record_issue_state_bytes.checked_add(schema_request_state_bytes);
        schema_request_state_limit_bytes = limits
            .max_schema_request_state_bytes
            .checked_sub(direct_record_issue_state_bytes)
            .filter(|limit| schema_request_state_bytes <= *limit)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling Item auxiliary workspace",
                used: auxiliary_precharge.map(|bytes| bytes as u64),
                limit: Some(limits.max_schema_request_state_bytes as u64),
            })?;
    }
    let item_rule_index_state_bytes = item_index_state_bytes
        .checked_add(file_membership_state_bytes)
        .and_then(|bytes| bytes.checked_add(limits.max_schema_request_state_bytes))
        .ok_or(crate::item_budget_origin!())?;
    let item_metadata_read_limit_bytes = limits
        .items
        .max_total_bytes
        .checked_sub(selected_items.read_bytes)
        .and_then(|remaining| remaining.checked_sub(item_record_read_bytes))
        .and_then(|remaining| remaining.checked_sub(file_membership_read_bytes))
        .ok_or(crate::item_budget_origin!())?;
    let mut item_limits = limits.items;
    item_limits.max_total_bytes = item_metadata_read_limit_bytes;
    item_limits.max_state_bytes = item_limits
        .max_state_bytes
        .checked_sub(item_rule_index_state_bytes)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "source-foundation ItemRules simultaneous workspace",
            used: Some(item_rule_index_state_bytes as u64),
            limit: Some(limits.items.max_state_bytes as u64),
        })?;
    let mut item_rules = ItemRules::new(item_limits, require_local_payloads);
    let candidate_store = streamed_record_summary.is_some();
    let mut item_source = CurrentItemSource {
        cut: source.cut(),
        input: source.input(),
        current_records: (!candidate_store).then_some(&direct_records.current_records),
        indexed_records: candidate_store.then(|| store.clone()).flatten(),
        record_cache: None,
        payloads,
        physical_facts,
        cancelled,
        limits: limits.items,
        membership_issues: &membership_issues,
        schema_checks: &mut schema_checks,
        schema_request_state_bytes: &mut schema_request_state_bytes,
        schema_request_state_limit_bytes,
        active_manifest: None,
        active_manifest_state_bytes: 0,
        companion_paths: Vec::new(),
        companion_state_bytes: 0,
        item_record_phase: false,
    };
    source.for_each_member_meta(limits.items.deadline, cancelled, &mut |member| {
        let path = member.path;
        if source.semantic_member(path)?
            && path.starts_with(SOURCE_HOME)
            && path.ends_with(ITEM_MANIFEST_SUFFIX)
        {
            check(limits.items, cancelled)?;
            item_rules.inspect_manifest(&mut item_source, path)?;
        }
        Ok(())
    })?;

    for record in item_records {
        check(limits.items, cancelled)?;
        let read_limit = item_rules.item_record_read_limit(record)?;
        let relative = RelativePath::parse(record)
            .map_err(|_| ItemRefusal::Unsupported("source-foundation Item record path".into()))?;
        let read_limit = usize::try_from(read_limit).map_err(|_| crate::item_budget_origin!())?;
        let member = source.read_member(
            relative.as_str(),
            read_limit,
            item_rule_index_state_bytes
                .checked_add(*item_source.schema_request_state_bytes)
                .ok_or(crate::item_budget_origin!())?,
            limits.items.max_state_bytes,
            limits.items.deadline,
            cancelled,
        )?;
        item_source.item_record_phase = true;
        item_rules.inspect_item_record(&mut item_source, record, &member.raw)?;
        item_source.item_record_phase = false;
    }
    check(limits.items, cancelled)?;
    let items = item_rules.finish();
    drop(item_source);
    if let Some(store) = &store {
        for issue in &items.issues {
            store.with(|store| store.item_issue(issue))?;
        }
        for item_id in &items.manifest_item_ids {
            store.with(|store| store.manifest_item_id(item_id))?;
        }
    }
    let mut ordered_issues = direct_records.issues;
    if rolling {
        let kernel_issues = rolling_record_issue_count.ok_or(crate::item_budget_origin!())?;
        let item_issues = items.issues.len();
        ordered_issues.max_issues = limits
            .operation
            .max_issues
            .checked_sub(kernel_issues)
            .and_then(|remaining| remaining.checked_sub(item_issues))
            .ok_or(crate::item_budget_origin!())?;
        if ordered_issues.rows.len() > ordered_issues.max_issues {
            return Err(crate::item_budget_origin!());
        }
    }
    let mut rights_ids = BTreeSet::<String>::new();
    let mut rights_identity_state_bytes = std::mem::size_of::<BTreeSet<String>>();
    schema_request_state_bytes
        .checked_add(rights_identity_state_bytes)
        .filter(|used| *used <= schema_request_state_limit_bytes)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "source-foundation rights-ID index state",
            used: schema_request_state_bytes
                .checked_add(rights_identity_state_bytes)
                .map(|used| used as u64),
            limit: Some(schema_request_state_limit_bytes as u64),
        })?;
    for request in &schema_checks {
        check(limits.items, cancelled)?;
        if request.family == SourceFoundationRecordsSchemaFamily::Item
            && request.contract == "ToS/contracts/rights-record.schema.json"
            && request.owner_issue.is_none()
            && let Some(id) = request
                .decoded_instance
                .as_ref()
                .and_then(|rights| rights.get("rights_id").and_then(Value::as_str))
        {
            if rights_ids.contains(id) {
                continue;
            }
            let addition = id
                .len()
                .checked_add(std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>())
                .ok_or(crate::item_budget_origin!())?;
            let next_state_bytes = rights_identity_state_bytes
                .checked_add(addition)
                .ok_or(crate::item_budget_origin!())?;
            schema_request_state_bytes
                .checked_add(next_state_bytes)
                .filter(|used| *used <= schema_request_state_limit_bytes)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation rights-ID index state",
                    used: schema_request_state_bytes
                        .checked_add(next_state_bytes)
                        .map(|used| used as u64),
                    limit: Some(schema_request_state_limit_bytes as u64),
                })?;
            rights_ids.insert(id.to_owned());
            if let Some(store) = &store {
                store.with(|store| store.rights_id(id))?;
            }
            rights_identity_state_bytes = next_state_bytes;
        }
    }
    let mut source_event_state_bytes = std::mem::size_of::<Vec<SourceFoundationEventInsertion>>();
    if schema_request_state_bytes
        .checked_add(rights_identity_state_bytes)
        .and_then(|used| used.checked_add(source_event_state_bytes))
        .is_none_or(|used| used > schema_request_state_limit_bytes)
    {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation provenance event insertion state",
            used: schema_request_state_bytes
                .checked_add(rights_identity_state_bytes)
                .and_then(|used| used.checked_add(source_event_state_bytes))
                .map(|used| used as u64),
            limit: Some(schema_request_state_limit_bytes as u64),
        });
    }
    let mut source_event_insertions = Vec::<SourceFoundationEventInsertion>::new();
    for request in &schema_checks {
        check(limits.items, cancelled)?;
        if request.contract != "ToS/contracts/provenance-event.schema.json" {
            continue;
        }
        if request.family != SourceFoundationRecordsSchemaFamily::Item
            || request.owner_issue.is_some()
        {
            continue;
        }
        let Some(decoded_instance) = request.decoded_instance.as_ref() else {
            continue;
        };
        let Some(id) = decoded_instance.get("event_id").and_then(Value::as_str) else {
            continue;
        };
        let event_decoded_state = crate::record_biblio_cut::decoded_state(decoded_instance)?;
        let addition = std::mem::size_of::<SourceFoundationEventInsertion>()
            .checked_add(id.len())
            .and_then(|bytes| bytes.checked_add(event_decoded_state))
            .ok_or(crate::item_budget_origin!())?;
        let next_event_state = source_event_state_bytes
            .checked_add(addition)
            .ok_or(crate::item_budget_origin!())?;
        schema_request_state_bytes
            .checked_add(rights_identity_state_bytes)
            .and_then(|used| used.checked_add(next_event_state))
            .filter(|used| *used <= schema_request_state_limit_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation provenance event insertion state",
                used: schema_request_state_bytes
                    .checked_add(rights_identity_state_bytes)
                    .and_then(|used| used.checked_add(next_event_state))
                    .map(|used| used as u64),
                limit: Some(schema_request_state_limit_bytes as u64),
            })?;
        let insertion = (id.to_owned(), decoded_instance.clone());
        if let Some(store) = &store {
            store.with(|store| store.source_event_insertion(&insertion))?;
        }
        source_event_insertions.push(insertion);
        source_event_state_bytes = next_event_state;
        check(limits.items, cancelled)?;
    }
    let item_reads_before_owner = selected_items
        .read_bytes
        .checked_add(item_rule_record_read_bytes)
        .and_then(|bytes| bytes.checked_add(file_membership_read_bytes))
        .and_then(|bytes| bytes.checked_add(items.metadata_bytes))
        .ok_or(crate::item_budget_origin!())?;
    let direct_item_read_limit = limits
        .items
        .max_total_bytes
        .checked_sub(item_reads_before_owner)
        .ok_or(crate::item_budget_origin!())?;
    let direct_item_issue_state_limit = limits
        .max_schema_request_state_bytes
        .checked_sub(schema_request_state_bytes)
        .and_then(|remaining| remaining.checked_sub(rights_identity_state_bytes))
        .and_then(|remaining| remaining.checked_sub(source_event_state_bytes))
        .ok_or(crate::item_budget_origin!())?;
    if ordered_issues.state_bytes > direct_item_issue_state_limit {
        return Err(crate::item_budget_origin!());
    }
    ordered_issues.max_state_bytes = direct_item_issue_state_limit;
    let (
        item_editions,
        direct_item_read_bytes,
        item_owner_index_state_bytes,
        item_owner_inventory_set_scan_steps,
    ) = append_item_direct_issues(
        source,
        limits,
        require_local_payloads,
        cancelled,
        physical_facts,
        &direct_records.current_records,
        &items.manifest_item_ids,
        &selected_items.records,
        &membership_issues,
        direct_item_read_limit,
        direct_item_issue_state_limit,
        &mut schema_checks,
        &mut schema_request_state_bytes,
        schema_request_state_limit_bytes,
        items.inventory_set_scan_steps,
        &mut ordered_issues,
        store.clone(),
    )?;
    if let Some(store) = &store {
        for check in &schema_checks {
            store.with(|store| store.schema_check(check))?;
        }
    }
    let aggregate_inventory_set_scan_steps = items
        .inventory_set_scan_steps
        .checked_add(item_owner_inventory_set_scan_steps)
        .filter(|used| *used <= limits.items.max_state_bytes)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "source-foundation Item inventory set membership scan steps",
            used: items
                .inventory_set_scan_steps
                .checked_add(item_owner_inventory_set_scan_steps)
                .map(|used| used as u64),
            limit: Some(limits.items.max_state_bytes as u64),
        })?;
    let ordered_issue_state_bytes = ordered_issues.state_bytes;
    let item_auxiliary_state_bytes = schema_request_state_bytes
        .checked_add(rights_identity_state_bytes)
        .and_then(|used| used.checked_add(source_event_state_bytes))
        .and_then(|used| used.checked_add(item_owner_index_state_bytes))
        .and_then(|used| used.checked_add(ordered_issue_state_bytes))
        .filter(|used| *used <= limits.max_schema_request_state_bytes)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "source-foundation schema requests and derived Item indexes state",
            used: schema_request_state_bytes
                .checked_add(rights_identity_state_bytes)
                .and_then(|used| used.checked_add(source_event_state_bytes))
                .and_then(|used| used.checked_add(item_owner_index_state_bytes))
                .and_then(|used| used.checked_add(ordered_issue_state_bytes))
                .map(|used| used as u64),
            limit: Some(limits.max_schema_request_state_bytes as u64),
        })?;
    let item_observed_read_bytes = selected_items
        .read_bytes
        .checked_add(item_rule_record_read_bytes)
        .and_then(|bytes| bytes.checked_add(file_membership_read_bytes))
        .and_then(|bytes| bytes.checked_add(items.metadata_bytes))
        .and_then(|bytes| bytes.checked_add(direct_item_read_bytes))
        .ok_or(crate::item_budget_origin!())?;
    if item_observed_read_bytes > limits.items.max_total_bytes {
        return Err(crate::item_budget_origin!());
    }
    let record_observed_read_bytes = if rolling {
        rolling_record_observed_read_bytes
    } else {
        match record_kernel_outcome.as_ref() {
            Some(SourceFoundationRecordKernelOutcome::Complete(_)) => Some(
                selected_current_member_bytes
                    .checked_add(schema_cost.records)
                    .and_then(|bytes| bytes.checked_add(record_registry_bytes))
                    .and_then(|bytes| bytes.checked_add(record_owner_read_bytes))
                    .ok_or(crate::item_budget_origin!())?,
            ),
            Some(SourceFoundationRecordKernelOutcome::Refused { .. }) | None => None,
        }
    };
    let operation_read_bound = if rolling {
        record_observed_read_bytes
            .and_then(|bytes| bytes.checked_add(item_observed_read_bytes))
            .ok_or(crate::item_budget_origin!())?
    } else {
        limits
            .records
            .max_total_bytes
            .checked_add(item_observed_read_bytes)
            .ok_or(crate::item_budget_origin!())?
    };
    if operation_read_bound > limits.operation.max_total_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation record and Item aggregate read bound",
            used: Some(operation_read_bound),
            limit: Some(limits.operation.max_total_bytes),
        });
    }

    let ordered_issue_count = ordered_issues.rows.len();
    let rolling_observed_issue_count = if rolling {
        let kernel_issues = rolling_record_issue_count.ok_or(crate::item_budget_origin!())?;
        let observed = kernel_issues
            .checked_add(items.issues.len())
            .and_then(|count| count.checked_add(ordered_issue_count))
            .ok_or(crate::item_budget_origin!())?;
        if observed > limits.operation.max_issues {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling combined issue count",
                used: Some(observed as u64),
                limit: Some(limits.operation.max_issues as u64),
            });
        }
        Some(observed)
    } else {
        None
    };
    if ordered_issue_count > limits.operation.max_issues {
        return Err(crate::item_budget_origin!());
    }
    let combined_family_state_limit_bytes = if rolling {
        limits.operation.max_state_bytes
    } else {
        limits
            .records
            .max_state_bytes
            .checked_add(limits.items.max_state_bytes)
            .ok_or(crate::item_budget_origin!())?
    };
    if !rolling && combined_family_state_limit_bytes > limits.operation.max_state_bytes {
        return Err(crate::item_budget_origin!());
    }
    let rolling_accounted_state_upper_bound_bytes = if rolling {
        let usage = record_usage.as_ref().ok_or(crate::item_budget_origin!())?;
        let accounted = report_header_state_bytes
            .checked_add(usage.accounted_state_upper_bound_bytes)
            .and_then(|used| used.checked_add(record_owner_state_bytes))
            .and_then(|used| used.checked_add(file_membership_state_bytes))
            .and_then(|used| used.checked_add(items.accounted_state_upper_bound_bytes))
            .and_then(|used| used.checked_add(item_auxiliary_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        if accounted > limits.operation.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation rolling combined accounted state",
                used: Some(accounted as u64),
                limit: Some(limits.operation.max_state_bytes as u64),
            });
        }
        Some(accounted)
    } else {
        None
    };
    let cost = SourceFoundationRecordsCost {
        selected_current_member_bytes,
        record_schema_resource_bytes: schema_cost.records,
        item_schema_resource_bytes: schema_cost.items,
        aggregate_schema_resource_bytes: schema_cost.aggregate,
        record_read_limit_bytes: limits.records.max_total_bytes,
        record_kernel_read_limit_bytes,
        item_read_limit_bytes: limits.items.max_total_bytes,
        item_record_read_bytes,
        item_selection_read_bytes: selected_items.read_bytes,
        item_rule_record_read_bytes,
        file_membership_read_bytes,
        item_owner_read_bytes: direct_item_read_bytes,
        item_metadata_read_limit_bytes,
        combined_family_read_limit_bytes: if rolling {
            record_observed_read_bytes
                .and_then(|bytes| bytes.checked_add(limits.items.max_total_bytes))
                .ok_or(crate::item_budget_origin!())?
        } else {
            limits
                .records
                .max_total_bytes
                .checked_add(limits.items.max_total_bytes)
                .ok_or(crate::item_budget_origin!())?
        },
        operation_read_limit_bytes: limits.operation.max_total_bytes,
        record_observed_read_bytes,
        record_owner_read_bytes,
        record_owner_state_bytes,
        item_observed_read_bytes,
        item_observed_metadata_bytes: items.metadata_bytes,
        item_inventory_set_scan_steps: items.inventory_set_scan_steps,
        item_owner_inventory_set_scan_steps,
        aggregate_inventory_set_scan_steps,
        inventory_set_scan_step_limit: limits.items.max_state_bytes,
        record_state_limit_bytes: limits.records.max_state_bytes,
        item_state_limit_bytes: limits.items.max_state_bytes,
        item_index_state_bytes,
        file_membership_state_bytes,
        schema_request_state_limit_bytes,
        schema_request_state_bytes,
        rights_identity_state_bytes,
        source_event_state_bytes,
        item_owner_index_state_bytes,
        item_auxiliary_state_bytes,
        item_rule_state_limit_bytes: limits.items.max_state_bytes - item_rule_index_state_bytes,
        combined_family_state_limit_bytes,
        operation_state_limit_bytes: limits.operation.max_state_bytes,
        ordered_issue_state_bytes,
        record_issue_limit: if rolling {
            record_kernel_limits.max_issues
        } else {
            limits.records.max_issues
        },
        item_issue_limit: limits.items.max_issues,
        combined_family_issue_limit: if rolling {
            limits.operation.max_issues
        } else {
            limits
                .records
                .max_issues
                .checked_add(limits.items.max_issues)
                .ok_or(crate::item_budget_origin!())?
        },
        operation_issue_limit: limits.operation.max_issues,
        rolling_accounted_state_upper_bound_bytes,
        rolling_observed_issue_count,
    };

    if let Some(summary) = streamed_record_summary {
        source.verify_current_fence(
            summary.input_coverage(),
            limits.operation.deadline,
            cancelled,
        )?;
    }

    Ok(SourceFoundationRecordsCoreReport {
        source_membership: expected_membership,
        records: record_kernel_outcome,
        record_usage,
        items,
        current_records: direct_records.current_records,
        current_record_order: direct_records.current_record_order,
        used_declared_profile_kinds: direct_records.used_declared_profile_kinds,
        item_records: selected_items
            .records
            .into_iter()
            .map(|record| record.selection)
            .collect(),
        record_schema_positions: direct_records.record_schema_positions,
        file_memberships,
        rights_ids,
        claim_ids: BTreeSet::new(),
        item_editions,
        source_event_insertions,
        ordered_issues: ordered_issues.rows,
        schema_checks,
        // Schema checks and CMD-owned physical facts are explicit inputs or
        // handoff rows above. Truly unrepresentable non-finite numbers fail the
        // invocation with a typed refusal before this report can be returned.
        unimplemented: Vec::new(),
        cost,
    })
}

struct SchemaResourceCost {
    records: u64,
    items: u64,
    aggregate: u64,
    item_resource_count: u64,
}

fn schema_resource_cost(
    cut: &CorpusCutReader,
    item_schemas: &CutWorkerSchemaExecutor,
) -> Result<SchemaResourceCost, ItemRefusal> {
    for path in REQUIRED_ITEM_SCHEMAS {
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("Item schema path".into()))?;
        let member = cut.current().member(&relative).ok_or_else(|| {
            ItemRefusal::Source(format!("Item schema is absent from captured cut: {path}"))
        })?;
        if item_schemas.contract_digest(path) != Some(member.sha256) {
            return Err(ItemRefusal::Source(format!(
                "Item schema worker does not use captured bytes: {path}"
            )));
        }
    }
    let mut records = 0u64;
    let mut items = 0u64;
    let mut item_resource_count = 0u64;
    for member in cut.current().members() {
        let path = member.path.as_str();
        if !path.starts_with(SCHEMA_HOME) || !path.ends_with(SCHEMA_SUFFIX) {
            continue;
        }
        records = records
            .checked_add(member.size_bytes)
            .ok_or(crate::item_budget_origin!())?;
        if let Some(digest) = item_schemas.contract_digest(path) {
            if digest != member.sha256 {
                return Err(ItemRefusal::Source(format!(
                    "Item schema worker resource differs from captured bytes: {path}"
                )));
            }
            items = items
                .checked_add(member.size_bytes)
                .ok_or(crate::item_budget_origin!())?;
            item_resource_count = item_resource_count
                .checked_add(1)
                .ok_or(crate::item_budget_origin!())?;
        }
    }
    Ok(SchemaResourceCost {
        records,
        items,
        aggregate: records
            .checked_add(items)
            .ok_or(crate::item_budget_origin!())?,
        item_resource_count,
    })
}

fn candidate_schema_resource_cost<I: Eq + Clone>(
    source: SourceFoundationCurrentInput<'_>,
    schema_binding: &impl SourceFoundationCandidateSchemaBinding<I>,
    limits: ItemLimits,
    max_schema_resource_bytes: u64,
    base_state_bytes: usize,
    cancelled: &AtomicBool,
) -> Result<SchemaResourceCost, ItemRefusal> {
    let mut records = 0u64;
    let mut previous_metadata_path = String::new();
    let mut previous_metadata_state_bytes = 0usize;
    source.for_each_member_meta(limits.deadline, cancelled, &mut |meta| {
        check(limits, cancelled)?;
        let relative_path_peak = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<RelativePath>()))
            .and_then(|bytes| bytes.checked_add(meta.path.len()))
            .ok_or(crate::item_budget_origin!())?;
        if relative_path_peak > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "candidate schema RelativePath validation state",
                used: Some(relative_path_peak as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        RelativePath::parse(meta.path)
            .map_err(|_| ItemRefusal::Source("schema metadata path is invalid".into()))?;
        if !previous_metadata_path.is_empty() && meta.path <= previous_metadata_path.as_str() {
            return Err(ItemRefusal::Source(
                "candidate schema source metadata is not strictly ordered".into(),
            ));
        }
        let next_path_state_bytes = std::mem::size_of::<String>()
            .checked_add(meta.path.len())
            .ok_or(crate::item_budget_origin!())?;
        let overlap_state = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .and_then(|bytes| bytes.checked_add(next_path_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        if overlap_state > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "candidate schema metadata path state",
                used: Some(overlap_state as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        if meta.path.starts_with(SCHEMA_HOME) && meta.path.ends_with(SCHEMA_SUFFIX) {
            records = records
                .checked_add(meta.size_bytes)
                .ok_or(crate::item_budget_origin!())?;
        }
        let mut next_path = String::new();
        next_path
            .try_reserve_exact(meta.path.len())
            .map_err(|_| crate::item_budget_origin!())?;
        next_path.push_str(meta.path);
        let actual_next_path_state_bytes = std::mem::size_of::<String>()
            .checked_add(next_path.capacity())
            .ok_or(crate::item_budget_origin!())?;
        let actual_overlap = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .and_then(|bytes| bytes.checked_add(actual_next_path_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        if actual_overlap > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "candidate schema metadata path capacity",
                used: Some(actual_overlap as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        previous_metadata_path = next_path;
        previous_metadata_state_bytes = actual_next_path_state_bytes;
        let live_state = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        if live_state > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "candidate schema metadata path retention",
                used: Some(live_state as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        check(limits, cancelled)
    })?;

    let mut items = 0u64;
    let mut item_resource_count = 0u64;
    let mut previous_resource_path = String::new();
    let mut previous_resource_state_bytes = 0usize;
    let mut required_found = [false; REQUIRED_ITEM_SCHEMAS.len()];
    schema_binding.for_each_selected_resource(&mut |resource| {
        check(limits, cancelled)?;
        let relative_path_peak = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .and_then(|bytes| bytes.checked_add(previous_resource_state_bytes))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<RelativePath>()))
            .and_then(|bytes| bytes.checked_add(resource.path.len()))
            .ok_or(crate::item_budget_origin!())?;
        if relative_path_peak > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "selected Item schema RelativePath validation state",
                used: Some(relative_path_peak as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        RelativePath::parse(resource.path)
            .map_err(|_| ItemRefusal::Source("selected Item schema path is invalid".into()))?;
        if !resource.path.starts_with(SCHEMA_HOME)
            || !resource.path.ends_with(SCHEMA_SUFFIX)
            || (!previous_resource_path.is_empty()
                && resource.path <= previous_resource_path.as_str())
        {
            return Err(ItemRefusal::Source(
                "selected Item schema resources are not a sorted exact schema closure".into(),
            ));
        }
        let retained_path_state = std::mem::size_of::<String>()
            .checked_add(resource.path.len())
            .ok_or(crate::item_budget_origin!())?;
        let live_state = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .and_then(|bytes| bytes.checked_add(previous_resource_state_bytes))
            .and_then(|bytes| bytes.checked_add(retained_path_state))
            .ok_or(crate::item_budget_origin!())?;
        if live_state > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "selected Item schema path and metadata state",
                used: Some(live_state as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        let actual_size = source
            .member_size(resource.path, limits.deadline, cancelled)?
            .ok_or_else(|| {
                ItemRefusal::Source(format!(
                    "selected Item schema is absent from candidate input: {}",
                    resource.path
                ))
            })?;
        // Every resource carries an authenticated authored schema declaration.
        // Its contract binding and raw bytes must agree with this exact input;
        // record-family selection remains a separate owner decision.
        let selected_contract_digest = schema_binding.contract_digest(resource.path);
        if actual_size != resource.size_bytes || selected_contract_digest != Some(resource.sha256) {
            return Err(ItemRefusal::Source(format!(
                "candidate schema binding differs from selected source resource: {}",
                resource.path
            )));
        }
        let digest = source
            .member_digest(
                resource.path,
                limits.max_member_bytes,
                live_state,
                limits.max_state_bytes,
                limits.deadline,
                cancelled,
            )?
            .ok_or_else(|| {
                ItemRefusal::Source(format!(
                    "selected Item schema disappeared during digest read: {}",
                    resource.path
                ))
            })?;
        if digest != resource.sha256 {
            return Err(ItemRefusal::Source(format!(
                "candidate schema bytes differ from selected digest: {}",
                resource.path
            )));
        }
        for (index, required) in REQUIRED_ITEM_SCHEMAS.iter().enumerate() {
            if resource.path == *required {
                required_found[index] = true;
            }
        }
        items = items
            .checked_add(resource.size_bytes)
            .ok_or(crate::item_budget_origin!())?;
        item_resource_count = item_resource_count
            .checked_add(1)
            .ok_or(crate::item_budget_origin!())?;
        let path_overlap = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .and_then(|bytes| bytes.checked_add(previous_resource_state_bytes))
            .and_then(|bytes| bytes.checked_add(retained_path_state))
            .ok_or(crate::item_budget_origin!())?;
        if path_overlap > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "selected Item schema path overlap",
                used: Some(path_overlap as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        let mut next_path = String::new();
        next_path
            .try_reserve_exact(resource.path.len())
            .map_err(|_| crate::item_budget_origin!())?;
        next_path.push_str(resource.path);
        let next_path_state_bytes = std::mem::size_of::<String>()
            .checked_add(next_path.capacity())
            .ok_or(crate::item_budget_origin!())?;
        let actual_path_overlap = base_state_bytes
            .checked_add(previous_metadata_state_bytes)
            .and_then(|bytes| bytes.checked_add(previous_resource_state_bytes))
            .and_then(|bytes| bytes.checked_add(next_path_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        if actual_path_overlap > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "selected Item schema path capacity",
                used: Some(actual_path_overlap as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        previous_resource_path = next_path;
        previous_resource_state_bytes = next_path_state_bytes;
        check(limits, cancelled)
    })?;
    if required_found.iter().any(|found| !found) {
        return Err(ItemRefusal::Source(
            "candidate Item schema closure omitted a required contract".into(),
        ));
    }
    let aggregate = records
        .checked_add(items)
        .ok_or(crate::item_budget_origin!())?;
    if aggregate > max_schema_resource_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation aggregate schema resource bytes",
            used: Some(aggregate),
            limit: Some(max_schema_resource_bytes),
        });
    }
    Ok(SchemaResourceCost {
        records,
        items,
        aggregate,
        item_resource_count,
    })
}

fn current_member_size(cut: &CorpusCutReader, path: &str) -> Result<u64, ItemRefusal> {
    let relative = RelativePath::parse(path)
        .map_err(|_| ItemRefusal::Unsupported("source-foundation record path".into()))?;
    cut.current()
        .member(&relative)
        .map(|member| member.size_bytes)
        .ok_or_else(|| {
            ItemRefusal::Source(format!("record member missing from captured cut: {path}"))
        })
}

struct CurrentItemSource<'a, 'store, P: CutPayloadReader> {
    cut: Option<&'a CorpusCutReader>,
    input: Option<&'a dyn SourceCutInput>,
    current_records: Option<&'a BTreeMap<String, BiblioCurrentRecord>>,
    indexed_records: Option<SourceFoundationRecordsStoreHandle<'store>>,
    record_cache: Option<(String, BiblioCurrentRecord, usize)>,
    payloads: &'a mut P,
    physical_facts: &'a SourcePhysicalFacts,
    cancelled: &'a AtomicBool,
    limits: ItemLimits,
    membership_issues: &'a [SourceFoundationRecordsIssue],
    schema_checks: &'a mut Vec<SourceFoundationRecordsSchemaCheck>,
    schema_request_state_bytes: &'a mut usize,
    schema_request_state_limit_bytes: usize,
    active_manifest: Option<String>,
    active_manifest_state_bytes: usize,
    companion_paths: Vec<ItemCompanionSchemaPath>,
    companion_state_bytes: usize,
    item_record_phase: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemCompanionContract {
    Manifest,
    Inventory,
    Rights,
    Provenance,
}

impl ItemCompanionContract {
    fn path(self) -> &'static str {
        match self {
            Self::Manifest => "ToS/contracts/source-item-manifest.schema.json",
            Self::Inventory => "ToS/contracts/source-resource-inventory.schema.json",
            Self::Rights => "ToS/contracts/rights-record.schema.json",
            Self::Provenance => "ToS/contracts/provenance-event.schema.json",
        }
    }
}

struct ItemCompanionSchemaPath {
    path: String,
    contract: ItemCompanionContract,
    consumed: bool,
}

impl<P: CutPayloadReader> CurrentItemSource<'_, '_, P> {
    fn path_presence(
        &self,
        path: &str,
        deadline: Instant,
    ) -> Result<Option<SourcePresenceV1>, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        if let Some(input) = self.input {
            if let Some(selection) = input.record_selection() {
                if let Some((file, line)) = path.rsplit_once(':') {
                    if file.ends_with(".jsonl") {
                        if !line
                            .parse::<u64>()
                            .ok()
                            .is_some_and(|line| selection.selected_row(file, line))
                        {
                            return Ok(None);
                        }
                        return self.path_presence(file, deadline);
                    }
                }
                if !selection.contains_member(path)
                    && !selection.members().any(|m| {
                        m.source_ref
                            .strip_prefix(path)
                            .is_some_and(|tail| tail.starts_with('/'))
                    })
                {
                    return Ok(None);
                }
            }
            let found = input.path_presence(path, deadline, self.cancelled)?;
            check_deadline(deadline, self.cancelled)?;
            return Ok(found);
        }
        let cut = self.cut.ok_or_else(|| {
            ItemRefusal::Source("source-foundation Item input is unavailable".into())
        })?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source-foundation Item path".into()))?;
        let found = cut.presence(cut.current().revision(), &relative);
        check_deadline(deadline, self.cancelled)?;
        Ok(found)
    }

    fn live_source_state_bytes(&self) -> Result<usize, ItemRefusal> {
        let cache = match &self.record_cache {
            Some((key, record, charged)) => {
                let minimum = std::mem::size_of::<(String, BiblioCurrentRecord, usize)>()
                    .checked_add(key.len())
                    .and_then(|bytes| bytes.checked_add(record.path.len()))
                    .and_then(|bytes| bytes.checked_add(record.kind.len()))
                    .and_then(|bytes| {
                        crate::record_biblio_cut::decoded_state(&record.value)
                            .ok()
                            .and_then(|value| bytes.checked_add(value))
                    })
                    .ok_or(crate::item_budget_origin!())?;
                minimum.max(*charged)
            }
            None => 0,
        };
        self.schema_request_state_bytes
            .checked_add(cache)
            .ok_or(crate::item_budget_origin!())
    }

    fn available_parse_state_bytes(
        &self,
        retained_input_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        let schema_available = self
            .schema_request_state_limit_bytes
            .checked_sub(*self.schema_request_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        let item_live = self
            .live_source_state_bytes()?
            .checked_add(retained_input_bytes)
            .ok_or(crate::item_budget_origin!())?;
        let item_available =
            self.limits
                .max_state_bytes
                .checked_sub(item_live)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation Item parse input state",
                    used: Some(item_live as u64),
                    limit: Some(self.limits.max_state_bytes as u64),
                })?;
        Ok(schema_available.min(item_available))
    }

    fn remember_active_manifest(
        &mut self,
        path: &str,
        raw_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        let retained = std::mem::size_of::<String>()
            .checked_add(path.len())
            .ok_or(crate::item_budget_origin!())?;
        let peak_aux = self
            .schema_request_state_bytes
            .checked_add(retained)
            .ok_or(crate::item_budget_origin!())?;
        let peak = self
            .live_source_state_bytes()?
            .checked_add(retained)
            .and_then(|bytes| bytes.checked_add(raw_bytes))
            .ok_or(crate::item_budget_origin!())?;
        if peak_aux > self.schema_request_state_limit_bytes || peak > self.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation active Item manifest state",
                used: Some(peak as u64),
                limit: Some(
                    self.limits
                        .max_state_bytes
                        .min(self.schema_request_state_limit_bytes) as u64,
                ),
            });
        }
        let mut active = String::new();
        active
            .try_reserve_exact(path.len())
            .map_err(|_| crate::item_budget_origin!())?;
        active.push_str(path);
        let next_aux = self
            .schema_request_state_bytes
            .checked_sub(self.active_manifest_state_bytes)
            .and_then(|bytes| bytes.checked_add(retained))
            .ok_or(crate::item_budget_origin!())?;
        self.active_manifest = Some(active);
        self.active_manifest_state_bytes = retained;
        *self.schema_request_state_bytes = next_aux;
        Ok(())
    }

    fn current_member_bytes(
        &self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source-foundation metadata path".into()))?;
        if self.path_presence(path, deadline)? != Some(SourcePresenceV1::File) {
            return Ok(None);
        }
        let mut raw = None;
        if let Some(input) = self.input {
            input.with_current_member(
                path,
                max_bytes,
                deadline,
                self.cancelled,
                &mut |meta: SourceCutMemberMeta<'_>, bytes| {
                    check_deadline(deadline, self.cancelled)?;
                    if raw.is_some()
                        || meta.path != path
                        || bytes.len() > max_bytes
                        || u64::try_from(bytes.len()).ok() != Some(meta.size_bytes)
                    {
                        return Err(ItemRefusal::Source(
                            "source-foundation Item input returned an inexact member".into(),
                        ));
                    }
                    let overlap = bytes
                        .len()
                        .checked_mul(2)
                        .and_then(|both| both.checked_add(std::mem::size_of::<Option<Vec<u8>>>()))
                        .ok_or(crate::item_budget_origin!())?;
                    let peak = self
                        .live_source_state_bytes()?
                        .checked_add(overlap)
                        .ok_or(crate::item_budget_origin!())?;
                    if peak > self.limits.max_state_bytes {
                        return Err(ItemRefusal::BudgetCheck {
                            check: "source-foundation Item borrowed and owned member overlap",
                            used: Some(peak as u64),
                            limit: Some(self.limits.max_state_bytes as u64),
                        });
                    }
                    if let Some(selection) = input.record_selection() {
                        selection.verify_metadata_member(path, bytes)?;
                        if selection.record(path).is_some() {
                            let scratch =
                                crate::source_record_selection::selection_state_upper_bound(
                                    bytes.len(),
                                )?;
                            let verify_peak = peak
                                .checked_add(scratch)
                                .ok_or(crate::item_budget_origin!())?;
                            if verify_peak > self.limits.max_state_bytes {
                                return Err(crate::item_budget_origin!());
                            }
                            selection.verify_record(path, bytes, deadline, self.cancelled)?;
                        }
                    }
                    let mut owned = Vec::new();
                    owned
                        .try_reserve_exact(bytes.len())
                        .map_err(|_| crate::item_budget_origin!())?;
                    owned.extend_from_slice(bytes);
                    raw = Some(owned);
                    Ok(())
                },
            )?;
            if raw.is_none() {
                return Err(ItemRefusal::Source(
                    "source-foundation Item input omitted a current file member".into(),
                ));
            }
        } else {
            let cut = self.cut.ok_or_else(|| {
                ItemRefusal::Source("source-foundation Item input is unavailable".into())
            })?;
            let metadata = cut.current().member(&relative).ok_or_else(|| {
                ItemRefusal::Source(format!(
                    "source-foundation Item member missing from captured cut: {path}"
                ))
            })?;
            let member_len =
                usize::try_from(metadata.size_bytes).map_err(|_| crate::item_budget_origin!())?;
            let peak = self
                .live_source_state_bytes()?
                .checked_add(member_len)
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<Vec<u8>>>()))
                .ok_or(crate::item_budget_origin!())?;
            if metadata.size_bytes > max_bytes as u64 || peak > self.limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation Item current member precharge",
                    used: Some(peak as u64),
                    limit: Some(self.limits.max_state_bytes.min(max_bytes) as u64),
                });
            }
            let member = cut
                .read_member(
                    cut.current().revision(),
                    &relative,
                    max_bytes as u64,
                    deadline,
                    self.cancelled,
                )
                .map_err(store_error)?;
            if member.raw.len() > max_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation Item current member bytes",
                    used: Some(member.raw.len() as u64),
                    limit: Some(max_bytes as u64),
                });
            }
            raw = Some(member.raw);
        }
        check_deadline(deadline, self.cancelled)?;
        Ok(raw)
    }

    fn current_record_kind(
        &mut self,
        id: &str,
        deadline: Instant,
    ) -> Result<Option<&str>, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        if let Some(records) = self.current_records {
            return Ok(records.get(id).map(|record| record.kind.as_str()));
        }
        if self
            .record_cache
            .as_ref()
            .is_some_and(|(key, _, _)| key == id)
        {
            return Ok(self
                .record_cache
                .as_ref()
                .map(|(_, record, _)| record.kind.as_str()));
        }
        let store = self.indexed_records.as_ref().ok_or_else(|| {
            ItemRefusal::Source("source-foundation Item record index is unavailable".into())
        })?;
        self.record_cache = None;
        let key_state = std::mem::size_of::<(String, BiblioCurrentRecord, usize)>()
            .checked_add(id.len())
            .and_then(|bytes| bytes.checked_add(*self.schema_request_state_bytes))
            .ok_or(crate::item_budget_origin!())?;
        let row_limit =
            self.limits
                .max_state_bytes
                .checked_sub(key_state)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation Item current-record lookup precharge",
                    used: Some(key_state as u64),
                    limit: Some(self.limits.max_state_bytes as u64),
                })?;
        let maximum =
            std::num::NonZeroUsize::new(row_limit.max(1)).ok_or(crate::item_budget_origin!())?;
        let mut key = String::new();
        key.try_reserve_exact(id.len())
            .map_err(|_| crate::item_budget_origin!())?;
        key.push_str(id);
        let found = store.with(|store| {
            SourceFoundationRecordsIndex::new(&*store).lookup_current_record(
                id,
                maximum,
                deadline,
                self.cancelled,
            )
        })?;
        check_deadline(deadline, self.cancelled)?;
        let found_state = found.as_ref().map(|lookup| lookup.charged_state_bytes);
        if found_state.is_some_and(|bytes| bytes > row_limit) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation Item current-record lookup state",
                used: found_state.map(|bytes| bytes as u64),
                limit: Some(row_limit as u64),
            });
        }
        self.record_cache = found.map(|lookup| (key, lookup.record, lookup.charged_state_bytes));
        Ok(self
            .record_cache
            .as_ref()
            .map(|(_, record, _)| record.kind.as_str()))
    }

    fn clear_companion_paths(&mut self) -> Result<(), ItemRefusal> {
        *self.schema_request_state_bytes = self
            .schema_request_state_bytes
            .checked_sub(self.companion_state_bytes)
            .ok_or(crate::item_budget_origin!())?;
        self.companion_state_bytes = 0;
        self.companion_paths = Vec::new();
        Ok(())
    }

    fn remember_companion(
        &mut self,
        path: &str,
        contract: ItemCompanionContract,
        additional_live_state_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        let retained = std::mem::size_of::<ItemCompanionSchemaPath>()
            .checked_add(path.len())
            .and_then(|bytes| {
                if self.companion_paths.is_empty() {
                    bytes.checked_add(std::mem::size_of::<Vec<ItemCompanionSchemaPath>>())
                } else {
                    Some(bytes)
                }
            })
            .ok_or(crate::item_budget_origin!())?;
        let next = self
            .schema_request_state_bytes
            .checked_add(retained)
            .filter(|used| *used <= self.schema_request_state_limit_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation Item companion selector state",
                used: self
                    .schema_request_state_bytes
                    .checked_add(retained)
                    .map(|used| used as u64),
                limit: Some(self.schema_request_state_limit_bytes as u64),
            })?;
        let live_peak = self
            .live_source_state_bytes()?
            .checked_add(additional_live_state_bytes)
            .and_then(|bytes| bytes.checked_add(retained))
            .ok_or(crate::item_budget_origin!())?;
        if live_peak > self.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation Item companion copy overlap",
                used: Some(live_peak as u64),
                limit: Some(self.limits.max_state_bytes as u64),
            });
        }
        let mut owned_path = String::new();
        owned_path
            .try_reserve_exact(path.len())
            .map_err(|_| crate::item_budget_origin!())?;
        owned_path.push_str(path);
        self.companion_paths
            .try_reserve_exact(1)
            .map_err(|_| crate::item_budget_origin!())?;
        self.companion_paths.push(ItemCompanionSchemaPath {
            path: owned_path,
            contract,
            consumed: false,
        });
        self.companion_state_bytes = self
            .companion_state_bytes
            .checked_add(retained)
            .ok_or(crate::item_budget_origin!())?;
        *self.schema_request_state_bytes = next;
        Ok(())
    }

    fn companion_for_path(&mut self, path: &str) -> Option<ItemCompanionContract> {
        let entry = self
            .companion_paths
            .iter_mut()
            .find(|entry| !entry.consumed && entry.path == path)?;
        entry.consumed = true;
        Some(entry.contract)
    }

    fn schedule_item_root_check(
        &mut self,
        location: &str,
        contract: ItemCompanionContract,
        instance: Option<Value>,
        owner_issue: SourceFoundationRecordsOwnerIssue,
    ) -> Result<(), ItemRefusal> {
        if self.schema_checks.len() >= self.limits.max_issues {
            return Err(crate::item_budget_origin!());
        }
        let before_issue = self.membership_issues.len();
        retain_direct_schema_check(
            self.schema_checks,
            self.schema_request_state_bytes,
            self.schema_request_state_limit_bytes,
            before_issue,
            SourceFoundationRecordsSchemaFamily::Item,
            location,
            contract.path(),
            instance,
            Some(owner_issue),
        )
    }

    fn schedule_jsonl_roots(
        &mut self,
        path: &str,
        contract: ItemCompanionContract,
        raw: &[u8],
        deadline: Instant,
    ) -> Result<(), ItemRefusal> {
        if let Some(selection) = self.input.and_then(SourceCutInput::record_selection) {
            for slot in selection.file_slots(path) {
                if slot.verification_state_upper_bound()?
                    > self.available_parse_state_bytes(raw.len())?
                {
                    return Err(crate::item_budget_origin!());
                }
            }
            let verified = selection.verify_file(path, raw, deadline, self.cancelled)?;
            let mut cursor = verified.row_cursor();
            while let Some(row) = cursor.next_checked(deadline, self.cancelled) {
                let (_, _, slot) = row?;
                if slot.kind != "provenance_event" {
                    return Err(ItemRefusal::Source(
                        "Item selected provenance slot kind".into(),
                    ));
                }
            }
            return Ok(());
        }
        let Ok(text) = std::str::from_utf8(raw) else {
            self.schedule_item_root_check(
                path,
                contract,
                None,
                SourceFoundationRecordsOwnerIssue::InvalidJsonlUtf8,
            )?;
            check_deadline(deadline, self.cancelled)?;
            return Ok(());
        };
        let mut item_limits = self.limits;
        item_limits.deadline = item_limits.deadline.min(deadline);
        for (offset, line) in text.lines().enumerate() {
            check(item_limits, self.cancelled)?;
            let ordinal = offset.checked_add(1).ok_or(crate::item_budget_origin!())?;
            let location = format!("{path}:{ordinal}");
            if line.trim().is_empty() {
                self.schedule_item_root_check(
                    &location,
                    contract,
                    None,
                    SourceFoundationRecordsOwnerIssue::BlankJsonlLine,
                )?;
                continue;
            }
            let available = self.available_parse_state_bytes(raw.len())?;
            match bounded_native_value(line.as_bytes(), item_limits, available, self.cancelled) {
                Ok(value) if value.is_object() => {}
                Ok(value) => self.schedule_item_root_check(
                    &location,
                    contract,
                    Some(value),
                    SourceFoundationRecordsOwnerIssue::JsonlRecordMustBeObject,
                )?,
                Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                    match legacy_parse_failure(
                        line.as_bytes(),
                        item_limits,
                        available,
                        self.cancelled,
                    )? {
                        LegacyJsonParseFailure::Malformed => self.schedule_item_root_check(
                            &location,
                            contract,
                            None,
                            SourceFoundationRecordsOwnerIssue::InvalidJsonl,
                        )?,
                        LegacyJsonParseFailure::Nonfinite => {
                            return Err(ItemRefusal::Unsupported(
                                "source-foundation nonfinite legacy JSONL cannot be represented by serde_json::Value".into(),
                            ));
                        }
                    }
                }
                Err(error) => return Err(error),
            }
        }
        check_deadline(deadline, self.cancelled)?;
        Ok(())
    }
}

impl<P: CutPayloadReader> ItemSource for CurrentItemSource<'_, '_, P> {
    fn record_selection(
        &self,
    ) -> Option<std::sync::Arc<crate::source_record_selection::SourceRecordSelection>> {
        self.input.and_then(SourceCutInput::record_selection)
    }
    fn cancellation_flag(&self) -> &AtomicBool {
        self.cancelled
    }

    fn check_cancelled(&self) -> Result<(), ItemRefusal> {
        check_deadline(self.limits.deadline, self.cancelled)
    }

    fn metadata(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        let manifest_owner = !self.item_record_phase && path.ends_with(ITEM_MANIFEST_SUFFIX);
        let Some(raw) = self.current_member_bytes(path, max_bytes, deadline)? else {
            return Ok(None);
        };
        if manifest_owner {
            self.clear_companion_paths()?;
            self.remember_active_manifest(path, raw.len())?;
            check_deadline(deadline, self.cancelled)?;
        }
        if self.item_record_phase {
            check_deadline(deadline, self.cancelled)?;
            return Ok(Some(raw));
        }
        let contract = if manifest_owner {
            Some(ItemCompanionContract::Manifest)
        } else {
            self.companion_for_path(path)
        };
        let Some(contract) = contract else {
            check_deadline(deadline, self.cancelled)?;
            return Ok(Some(raw));
        };
        if contract == ItemCompanionContract::Provenance {
            self.schedule_jsonl_roots(path, contract, &raw, deadline)?;
            check_deadline(deadline, self.cancelled)?;
            return Ok(Some(raw));
        }
        let mut item_limits = self.limits;
        item_limits.deadline = item_limits.deadline.min(deadline);
        let available = self.available_parse_state_bytes(raw.len())?;
        match bounded_native_value(&raw, item_limits, available, self.cancelled) {
            Ok(value) if !value.is_object() => self.schedule_item_root_check(
                path,
                contract,
                Some(value),
                SourceFoundationRecordsOwnerIssue::JsonRootMustBeObject,
            )?,
            Ok(value) => {
                if manifest_owner {
                    let decoded_value_bytes = crate::record_biblio_cut::decoded_state(&value)?;
                    let additional_live_state_bytes = raw
                        .len()
                        .checked_add(decoded_value_bytes)
                        .ok_or(crate::item_budget_origin!())?;
                    for (field, companion) in [
                        ("resource_inventory_ref", ItemCompanionContract::Inventory),
                        ("rights_ref", ItemCompanionContract::Rights),
                        ("provenance_ref", ItemCompanionContract::Provenance),
                    ] {
                        if let Some(reference) = value.get(field).and_then(Value::as_str) {
                            self.remember_companion(
                                reference,
                                companion,
                                additional_live_state_bytes,
                            )?;
                        }
                    }
                }
                drop(value);
            }
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                match legacy_parse_failure(&raw, item_limits, available, self.cancelled)? {
                    LegacyJsonParseFailure::Malformed => self.schedule_item_root_check(
                        path,
                        contract,
                        None,
                        SourceFoundationRecordsOwnerIssue::InvalidJson,
                    )?,
                    LegacyJsonParseFailure::Nonfinite
                        if contract == ItemCompanionContract::Inventory =>
                    {
                        // The ItemRules inventory route will retain a typed
                        // LegacyPythonObserved tree and call `schema` with
                        // these same original bytes. Other companion and
                        // command decoders remain on their finite-only route.
                    }
                    LegacyJsonParseFailure::Nonfinite => {
                        return Err(ItemRefusal::Unsupported(
                            "source-foundation nonfinite legacy JSON cannot be represented by serde_json::Value".into(),
                        ));
                    }
                }
            }
            Err(error) => return Err(error),
        }
        check_deadline(deadline, self.cancelled)?;
        Ok(Some(raw))
    }

    fn legacy_observed_inventory(
        &mut self,
        path: &str,
        raw: &[u8],
        max_member_bytes: usize,
        available_state_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<tos_foundation::JsonValue>, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        if self.item_record_phase
            || self.active_manifest.is_none()
            || !self.companion_paths.iter().any(|entry| {
                entry.consumed
                    && entry.path == path
                    && entry.contract == ItemCompanionContract::Inventory
            })
        {
            return Err(ItemRefusal::Unsupported(
                "legacy observed decode is outside the selected current Item inventory".into(),
            ));
        }
        if raw.len() > max_member_bytes || raw.len() > self.limits.max_member_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "legacy observed inventory member bytes",
                used: Some(raw.len() as u64),
                limit: Some(max_member_bytes.min(self.limits.max_member_bytes) as u64),
            });
        }
        let mut limits = self.limits;
        limits.max_member_bytes = limits.max_member_bytes.min(max_member_bytes);
        limits.deadline = limits.deadline.min(deadline);
        let available = self.available_parse_state_bytes(raw.len())?.min(
            available_state_bytes
                .checked_sub(raw.len())
                .ok_or(crate::item_budget_origin!())?,
        );
        let observed = bounded_legacy_observed_inventory(raw, limits, available, self.cancelled)?;
        check_deadline(deadline, self.cancelled)?;
        Ok(observed)
    }

    fn exists(&mut self, path: &str, deadline: Instant) -> Result<bool, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        Ok(self.path_presence(path, deadline)?.is_some())
    }

    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        // ItemRules' final `item_manifest_ref` lookup currently routes through
        // its general object helper, which adds a second schema/ref check not
        // present in the maintained Python final Item-record loop. Keep that
        // kernel observation separate and schedule only owner-route checks.
        if self.item_record_phase {
            return Ok(true);
        }
        if self.schema_checks.len() >= self.limits.max_issues {
            return Err(crate::item_budget_origin!());
        }
        let mut item_limits = self.limits;
        item_limits.deadline = item_limits.deadline.min(deadline);
        let available = self.available_parse_state_bytes(raw.len())?;
        let before_issue = if let Some(manifest) = &self.active_manifest {
            self.membership_issues
                .iter()
                .take_while(|issue| issue.location.as_str() < manifest.as_str())
                .count()
        } else {
            self.membership_issues.len()
        };
        match bounded_native_value(raw, item_limits, available, self.cancelled) {
            Ok(decoded_instance) => retain_direct_schema_check(
                self.schema_checks,
                self.schema_request_state_bytes,
                self.schema_request_state_limit_bytes,
                before_issue,
                SourceFoundationRecordsSchemaFamily::Item,
                path,
                contract,
                Some(decoded_instance),
                None,
            )?,
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                if contract != "ToS/contracts/source-resource-inventory.schema.json" {
                    return Err(ItemRefusal::Source(
                        "legacy observed schema input is outside Item inventory".into(),
                    ));
                }
                match bounded_legacy_observed_inventory(
                    raw,
                    item_limits,
                    available,
                    self.cancelled,
                )? {
                    Some(value) => {
                        let owner_issue = value
                            .as_object()
                            .is_none()
                            .then_some(SourceFoundationRecordsOwnerIssue::JsonRootMustBeObject);
                        drop(value);
                        retain_direct_legacy_schema_check(
                            self.schema_checks,
                            self.schema_request_state_bytes,
                            self.schema_request_state_limit_bytes,
                            before_issue,
                            SourceFoundationRecordsSchemaFamily::Item,
                            path,
                            contract,
                            raw,
                            owner_issue,
                        )?;
                    }
                    None => retain_direct_schema_check(
                        self.schema_checks,
                        self.schema_request_state_bytes,
                        self.schema_request_state_limit_bytes,
                        before_issue,
                        SourceFoundationRecordsSchemaFamily::Item,
                        path,
                        contract,
                        None,
                        Some(SourceFoundationRecordsOwnerIssue::InvalidJson),
                    )?,
                }
            }
            Err(error) => return Err(error),
        }
        check_deadline(deadline, self.cancelled)?;
        Ok(true)
    }

    fn payload(&mut self, path: &str, deadline: Instant) -> Result<ItemPayload, ItemRefusal> {
        check_deadline(deadline, self.cancelled)?;
        let facts = self.physical_facts.payloads.get(path).ok_or_else(|| {
            ItemRefusal::Unsupported(format!(
                "source-foundation physical payload observation is unavailable: {path}"
            ))
        })?;
        if !facts.exists || !facts.regular_file || facts.symlink {
            let observed = self.payloads.inspect(path, deadline, self.cancelled)?;
            if matches!(observed, ItemPayload::File { .. }) {
                return Err(ItemRefusal::Source(format!(
                    "source-foundation payload reader disagrees with physical facts: {path}"
                )));
            }
            return Ok(ItemPayload::Unavailable);
        }
        let byte_size = facts.byte_size.ok_or_else(|| {
            ItemRefusal::Unsupported(format!(
                "source-foundation physical payload size is unavailable: {path}"
            ))
        })?;
        let sha256 = facts.sha256.as_ref().ok_or_else(|| {
            ItemRefusal::Unsupported(format!(
                "source-foundation physical payload digest is unavailable: {path}"
            ))
        })?;
        match self.payloads.inspect(path, deadline, self.cancelled)? {
            ItemPayload::Unavailable => Err(ItemRefusal::Unsupported(format!(
                "source-foundation payload bytes were not supplied for a physical file: {path}"
            ))),
            ItemPayload::File {
                byte_size: observed_size,
                sha256: observed_sha256,
                excluded_from_source,
            } => {
                if observed_size != byte_size || observed_sha256.as_str() != sha256.as_str() {
                    return Err(ItemRefusal::Source(format!(
                        "source-foundation payload reader differs from physical facts: {path}"
                    )));
                }
                Ok(ItemPayload::File {
                    byte_size,
                    sha256: sha256.clone(),
                    excluded_from_source,
                })
            }
        }
    }

    fn record_kind(&mut self, id: &str, deadline: Instant) -> Result<Option<&str>, ItemRefusal> {
        self.current_record_kind(id, deadline)
    }
}

fn validate_limits(limits: SourceFoundationRecordsLimits) -> Result<(), ItemRefusal> {
    let operation = limits.operation;
    let records = limits.records;
    let items = limits.items;
    for current in [operation, records, items] {
        if current.max_member_bytes == 0
            || current.max_member_bytes == usize::MAX
            || current.max_total_bytes == 0
            || current.max_total_bytes == u64::MAX
            || current.max_state_bytes == 0
            || current.max_state_bytes == usize::MAX
            || current.max_issues == 0
            || current.max_issues == usize::MAX
        {
            return Err(crate::item_budget_origin!());
        }
        if current.deadline != operation.deadline {
            return Err(ItemRefusal::Unsupported(
                "source-foundation families require one caller deadline".into(),
            ));
        }
    }
    if limits.max_schema_resource_bytes == 0
        || limits.max_schema_resource_bytes == u64::MAX
        || limits.max_schema_request_state_bytes == 0
        || limits.max_schema_request_state_bytes == usize::MAX
        || limits.max_schema_request_state_bytes > items.max_state_bytes
    {
        return Err(crate::item_budget_origin!());
    }
    if records.max_member_bytes > operation.max_member_bytes
        || items.max_member_bytes > operation.max_member_bytes
        || records
            .max_total_bytes
            .checked_add(items.max_total_bytes)
            .is_none_or(|sum| sum > operation.max_total_bytes)
        || records
            .max_state_bytes
            .checked_add(items.max_state_bytes)
            .is_none_or(|sum| sum > operation.max_state_bytes)
        || records
            .max_issues
            .checked_add(items.max_issues)
            .is_none_or(|sum| sum > operation.max_issues)
    {
        return Err(crate::item_budget_origin!());
    }
    Ok(())
}

fn validate_rolling_limits(limits: SourceFoundationRecordsLimits) -> Result<(), ItemRefusal> {
    let operation = limits.operation;
    let records = limits.records;
    let items = limits.items;
    for current in [operation, records, items] {
        if current.max_member_bytes == 0
            || current.max_member_bytes == usize::MAX
            || current.max_total_bytes == 0
            || current.max_total_bytes == u64::MAX
            || current.max_state_bytes == 0
            || current.max_state_bytes == usize::MAX
            || current.max_issues == 0
            || current.max_issues == usize::MAX
        {
            return Err(crate::item_budget_origin!());
        }
        if current.deadline != operation.deadline {
            return Err(ItemRefusal::Unsupported(
                "source-foundation families require one caller deadline".into(),
            ));
        }
    }
    if limits.max_schema_resource_bytes == 0
        || limits.max_schema_resource_bytes == u64::MAX
        || limits.max_schema_request_state_bytes == 0
        || limits.max_schema_request_state_bytes == usize::MAX
        || limits.max_schema_request_state_bytes > items.max_state_bytes
        || records.max_member_bytes > operation.max_member_bytes
        || items.max_member_bytes > operation.max_member_bytes
    {
        return Err(crate::item_budget_origin!());
    }
    Ok(())
}

fn check(limits: ItemLimits, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    check_deadline(limits.deadline, cancelled)
}

fn charge_inventory_set_scan_step(
    used: &mut usize,
    limit: usize,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    check(limits, cancelled)?;
    let next = used.checked_add(1).ok_or(crate::item_budget_origin!())?;
    if next > limit {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation Item inventory set membership scan steps",
            used: Some(next as u64),
            limit: Some(limit as u64),
        });
    }
    *used = next;
    Ok(())
}

fn check_deadline(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "source-foundation record district cancelled".into(),
        ));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}

fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    ItemRefusal::Source(crate::record_biblio_cut::source_store_cause(&error))
}

#[cfg(test)]
mod item_manifest_scope_tests {
    use super::*;

    #[test]
    fn retained_foreign_manifest_does_not_expand_selected_item_scope() {
        let owned = "ToS/source-witnesses/selected/items/item/item.manifest.json";
        let foreign = "ToS/source-witnesses/referenced/items/item/item.manifest.json";
        let records = vec![SelectedItemRecord {
            selection: SourceFoundationItemRecordSelection {
                record_id: "selected-item".into(),
                path: "ToS/source-witnesses/selected/items/item/item.json".into(),
                value: serde_json::json!({"item_manifest_ref": owned}),
            },
        }];
        assert!(item_manifest_in_scope(true, &records, owned));
        assert!(!item_manifest_in_scope(true, &records, foreign));
        assert!(!item_manifest_in_scope(true, &[], foreign));
        assert!(item_manifest_in_scope(false, &records, foreign));
    }
}
