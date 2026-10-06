//! Bounded cross-stream closure for the maintained source-witness foundation.
//!
//! This module checks only the current source cut supplied by its caller. It
//! is a source-only closure district, not a family-complete report or a source
//! admission result. Current record identity comes from the independently
//! checked record district; current path membership comes from the captured
//! source cut. The caller binds the record district's revision and membership
//! to that cut before supplying its maps. Ordinary schema and byte reads stay
//! behind `LayerFamilySource`; addressed historical and Artifact replay checks
//! retain their own conditional owner routes.

use crate::biblio_rules::BiblioClaim;
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::layer_family_rules::LayerFamilySource;
use crate::record_biblio_cut::{
    BiblioCurrentRecord, SourceCutInputCoverage, SourceCutInputWithIdentity,
};
use crate::source_foundation_default_rules::{
    BorrowedDefaultRecords, SliceDefaultClaims, SliceDefaultPaths, SourceFoundationDefaultClaims,
    SourceFoundationDefaultEventLookup, SourceFoundationDefaultPaths,
    SourceFoundationDefaultRecordsLookup, estimate_string_storage, estimate_value_storage,
};
use crate::source_witness_foundation::SourceFileMembershipIndex;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, RelativePath, canonical_bytes_v1};
use tos_source_store::CorpusCutReader;

use crate::source_foundation_default_rules::SourceFoundationDefaultRuleScope;

const SOURCE_HOME: &str = "ToS/source-witnesses/";
const CLAIM_SCHEMA: &str = "ToS/contracts/claim-packet.schema.json";
const PROVENANCE_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const PROVENANCE_V2_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const ANCHOR_SCHEMA: &str = "ToS/contracts/source-anchor.schema.json";
const BOUNDARY_MAP_SCHEMA: &str = "ToS/contracts/collection-work-boundary-map.schema.json";
const DERIVATION_SCHEMA: &str = "ToS/contracts/expression-derivation.schema.json";
const CHRONOLOGY_SCHEMA: &str = "ToS/contracts/first-publication-chronology.schema.json";
const OBJECT_LINK_SCHEMA: &str = "ToS/contracts/object-link-claim.schema.json";
const PROVISION_SCHEMA: &str = "ToS/contracts/provision-activity.schema.json";
const TOPOLOGY_EVENT: &str =
    "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31";
const TOPOLOGY_PROVENANCE: &str = "ToS/source-witnesses/relations/provenance.jsonl";
const DERIVATION_CLAIMS: &str =
    "ToS/source-witnesses/relations/expression-derivation/expression-derivation-claims.jsonl";
const DERIVATION_PROVENANCE: &str =
    "ToS/source-witnesses/relations/expression-derivation/provenance.jsonl";
const DERIVATION_EVENT: &str =
    "tos.event.annotation.expression-derivation.antonovsky-revision-lineage.2026-08-01";
const CHRONOLOGY_CLAIMS: &str = "ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication/work-chronology-claims.jsonl";
const CHRONOLOGY_PROVENANCE: &str =
    "ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication/provenance.jsonl";
const CHRONOLOGY_EVENT: &str =
    "tos.event.annotation.friedrich-nietzsche.first-publication-chronology.2026-07-31";
const WORK_CHRONOLOGY_SCHEMA: &str = "ToS/contracts/first-publication-chronology.schema.json";
const PROVISION_EVENT_BASENAME: &str = "provision-activity-provenance.jsonl";

/// Conservative pre-decode state bound for one source JSON value. Keep the
/// same bound used by the candidate command spool so both owners preflight
/// raw JSON before serde can allocate nested maps and strings.
pub fn source_foundation_closure_json_state_upper_bound(
    raw_bytes: usize,
) -> Result<usize, ItemRefusal> {
    raw_bytes
        .checked_mul(128)
        .and_then(|bytes| bytes.checked_add(8192))
        .ok_or(ItemRefusal::Budget)
}

fn estimate_fixed_json_object_storage(
    fields: &[(&str, Option<&str>)],
) -> Result<usize, ItemRefusal> {
    let node_storage = fields
        .len()
        .checked_mul(std::mem::size_of::<(String, Value)>() + 96)
        .and_then(|bytes| {
            fields
                .len()
                .checked_mul(std::mem::size_of::<(&str, Option<&str>)>())
                .and_then(|descriptors| bytes.checked_add(descriptors))
        })
        .ok_or(ItemRefusal::Budget)?;
    let mut storage = std::mem::size_of::<Value>()
        .checked_add(node_storage)
        .ok_or(ItemRefusal::Budget)?;
    for (key, string_value) in fields {
        let value_storage = std::mem::size_of::<Value>()
            .checked_add(match string_value {
                Some(value) => estimate_string_storage(value)?,
                None => 32,
            })
            .ok_or(ItemRefusal::Budget)?;
        storage = storage
            .checked_add(estimate_string_storage(key)?)
            .and_then(|bytes| bytes.checked_add(value_storage))
            .ok_or(ItemRefusal::Budget)?;
    }
    Ok(storage)
}

const TOPOLOGY_ROUTES: [(&str, &str, &str, &str, &str, &str); 3] = [
    (
        "ToS/source-witnesses/relations/work-expression/work-expression-claims.jsonl",
        "has_expression",
        "work",
        "expression",
        "expression_claim_refs",
        "unreviewed-work-expression-topology-claims",
    ),
    (
        "ToS/source-witnesses/relations/expression-edition/expression-edition-claims.jsonl",
        "embodied_by",
        "expression",
        "edition",
        "embodiment_claim_refs",
        "unreviewed-expression-edition-topology-claims",
    ),
    (
        "ToS/source-witnesses/relations/edition-item/edition-item-claims.jsonl",
        "exemplified_by",
        "edition",
        "item",
        "exemplar_claim_refs",
        "unreviewed-edition-item-topology-claims",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFoundationClosureCost {
    /// Bytes returned by the exact-cut current and retained read adapter.
    pub current_bytes_read: u64,
    /// Current-source read calls attempted, including successful empty or
    /// absent bodies. `files_read` continues to count only returned bodies.
    pub current_read_operations: u64,
    pub recorded_bytes_read: u64,
    /// Exact retained-history lookup calls, including absent results. This
    /// separates worker reads from the count of file bodies returned.
    pub recorded_read_operations: u64,
    /// Exact retained-history calls that returned a body; absent results do
    /// not synthesize file or byte counts.
    pub recorded_files_read: u64,
    pub files_read: u64,
    pub schema_requests: u64,
    pub decoded_rows: u64,
    pub reserved_state_bytes: usize,
    pub emitted_issues: usize,
    /// Candidate-local Link rows inserted into the invocation's bounded
    /// scratch table. Persistent bytes remain disk custody in CMD.
    pub candidate_link_rows: u64,
    pub candidate_link_serialized_write_bytes: u64,
    pub candidate_link_serialized_read_bytes: u64,
    pub candidate_link_scan_row_operations: u64,
    pub candidate_link_peak_workspace_state_bytes: usize,
    /// Candidate-local Closure schema requests stored until the existing
    /// diagnostic worker drains them in encounter order.
    pub candidate_schema_request_count: u64,
    pub candidate_schema_request_serialized_write_bytes: u64,
    pub candidate_schema_request_serialized_read_bytes: u64,
    pub candidate_schema_request_scan_row_operations: u64,
    pub candidate_schema_request_peak_workspace_state_bytes: usize,
    /// Candidate-local, exact-cut document digests bound to held source-row
    /// projections without retaining a path-to-rows map in memory.
    pub candidate_loaded_document_count: u64,
    pub candidate_loaded_document_serialized_read_bytes: u64,
    pub candidate_loaded_document_serialized_write_bytes: u64,
    pub candidate_loaded_document_scan_row_operations: u64,
    pub candidate_loaded_document_peak_workspace_state_bytes: usize,
    /// Exact physical rows copied into the held loaded-document spool. The
    /// bytes remain a source-derived scratch projection bound to the same
    /// document digest and current-input fence.
    pub candidate_loaded_row_store: SourceFoundationClosureLoadedRowStoreCost,
    /// Candidate Closure event rows retained as plain source-derived JSON
    /// projections in the invocation-scoped event store.
    pub candidate_event_count: u64,
    pub candidate_event_serialized_read_bytes: u64,
    pub candidate_event_serialized_write_bytes: u64,
    pub candidate_event_scan_row_operations: u64,
    pub candidate_event_peak_workspace_state_bytes: usize,
    /// Candidate event-path locators spooled before source reads begin.
    pub candidate_event_path_count: u64,
    pub candidate_event_path_serialized_read_bytes: u64,
    pub candidate_event_path_serialized_write_bytes: u64,
    pub candidate_event_path_scan_row_operations: u64,
    pub candidate_event_path_peak_workspace_state_bytes: usize,
    /// Candidate Closure claim IDs held in the invocation-scoped exact-key
    /// uniqueness store; only the finite compatibility path keeps this set
    /// in process memory.
    pub candidate_claim_id_count: u64,
    pub candidate_claim_id_serialized_read_bytes: u64,
    pub candidate_claim_id_serialized_write_bytes: u64,
    pub candidate_claim_id_scan_row_operations: u64,
    pub candidate_claim_id_peak_workspace_state_bytes: usize,
    /// Candidate Closure membership IDs and subject join keys held in the
    /// invocation-scoped exact-key store; the finite compatibility path keeps
    /// its original in-process map.
    pub candidate_membership_claim_count: u64,
    pub candidate_membership_claim_serialized_read_bytes: u64,
    pub candidate_membership_claim_serialized_write_bytes: u64,
    pub candidate_membership_claim_scan_row_operations: u64,
    pub candidate_membership_claim_peak_workspace_state_bytes: usize,
    /// Candidate Responsibility claim projections kept as plain source rows
    /// in the invocation-scoped exact-ID store. The finite compatibility path
    /// keeps its BTreeMap.
    pub candidate_responsibility_claim_count: u64,
    pub candidate_responsibility_claim_serialized_read_bytes: u64,
    pub candidate_responsibility_claim_serialized_write_bytes: u64,
    pub candidate_responsibility_claim_scan_row_operations: u64,
    pub candidate_responsibility_claim_peak_workspace_state_bytes: usize,
    /// Candidate Publication ClaimRef projections stored as plain source rows.
    /// The finite compatibility path keeps the original BTreeMap.
    pub candidate_publication_claim_count: u64,
    pub candidate_publication_claim_serialized_read_bytes: u64,
    pub candidate_publication_claim_serialized_write_bytes: u64,
    pub candidate_publication_claim_scan_row_operations: u64,
    pub candidate_publication_claim_peak_workspace_state_bytes: usize,
    /// Candidate Provision ClaimRef projections stored by exact ID.
    pub candidate_provision_claim_count: u64,
    pub candidate_provision_claim_serialized_read_bytes: u64,
    pub candidate_provision_claim_serialized_write_bytes: u64,
    pub candidate_provision_claim_scan_row_operations: u64,
    pub candidate_provision_claim_peak_workspace_state_bytes: usize,
    pub candidate_provision_claim_drained_rows: u64,
    pub candidate_provision_claim_eof_seen: bool,
    pub candidate_provision_claim_count_verified: bool,
    /// Candidate Provision event IDs and their separate used/validated sets.
    pub candidate_provision_event_id_count: u64,
    pub candidate_provision_event_id_serialized_read_bytes: u64,
    pub candidate_provision_event_id_serialized_write_bytes: u64,
    pub candidate_provision_event_id_scan_row_operations: u64,
    pub candidate_provision_event_id_peak_workspace_state_bytes: usize,
    pub candidate_provision_event_id_drained_rows: u64,
    pub candidate_provision_event_id_lookup_rows: u64,
    pub candidate_provision_event_id_eof_seen: bool,
    pub candidate_provision_event_id_count_verified: bool,
    pub candidate_provision_used_event_count: u64,
    pub candidate_provision_used_event_serialized_read_bytes: u64,
    pub candidate_provision_used_event_serialized_write_bytes: u64,
    pub candidate_provision_used_event_scan_row_operations: u64,
    pub candidate_provision_used_event_peak_workspace_state_bytes: usize,
    pub candidate_provision_validated_event_count: u64,
    pub candidate_provision_validated_event_serialized_read_bytes: u64,
    pub candidate_provision_validated_event_serialized_write_bytes: u64,
    pub candidate_provision_validated_event_scan_row_operations: u64,
    pub candidate_provision_validated_event_peak_workspace_state_bytes: usize,
    pub candidate_provision_unused_event_count: u64,
    pub candidate_provision_unused_event_serialized_read_bytes: u64,
    pub candidate_provision_unused_event_scan_row_operations: u64,
    pub candidate_provision_unused_event_peak_workspace_state_bytes: usize,
    pub candidate_provision_unused_event_drained_rows: u64,
    pub candidate_provision_unused_event_eof_seen: bool,
    pub candidate_provision_unused_event_count_verified: bool,
    /// Candidate event-input checks already performed for Responsibility
    /// Claims, keyed by check kind and exact event ID in the same held store.
    pub candidate_responsibility_validated_event_count: u64,
    pub candidate_responsibility_validated_event_serialized_read_bytes: u64,
    pub candidate_responsibility_validated_event_serialized_write_bytes: u64,
    pub candidate_responsibility_validated_event_scan_row_operations: u64,
    pub candidate_responsibility_validated_event_peak_workspace_state_bytes: usize,
    /// Candidate event-input checks already performed for Publication Claims,
    /// stored in a separate exact check-kind namespace.
    pub candidate_publication_validated_event_count: u64,
    pub candidate_publication_validated_event_serialized_read_bytes: u64,
    pub candidate_publication_validated_event_serialized_write_bytes: u64,
    pub candidate_publication_validated_event_scan_row_operations: u64,
    pub candidate_publication_validated_event_peak_workspace_state_bytes: usize,
    /// Unique boundary Responsibility ClaimRef IDs held in the candidate AUX
    /// store. The finite compatibility path keeps the original BTreeSet.
    pub candidate_boundary_responsibility_ref_count: u64,
    pub candidate_boundary_responsibility_ref_serialized_read_bytes: u64,
    pub candidate_boundary_responsibility_ref_serialized_write_bytes: u64,
    pub candidate_boundary_responsibility_ref_scan_row_operations: u64,
    pub candidate_boundary_responsibility_ref_peak_workspace_state_bytes: usize,
    /// Unique boundary Membership ClaimRef IDs held in the candidate AUX
    /// store. The finite compatibility path keeps its original BTreeSet.
    pub candidate_boundary_membership_refs: SourceFoundationClosureBoundaryMembershipRefStoreCost,
    /// Candidate evidence-anchor IDs held in the invocation-scoped exact-key
    /// store. The finite compatibility path keeps its in-process set.
    pub candidate_anchor_store: SourceFoundationClosureAnchorStoreCost,
    /// Candidate Expression-derivation graph projections and external DFS
    /// state in the invocation-scoped Closure store.
    pub candidate_derivation_store: SourceFoundationClosureDerivationStoreCost,
    /// Candidate bibliographic-topology ClaimRef projections held in the
    /// invocation-scoped Closure store.
    pub candidate_topology_store: SourceFoundationClosureTopologyStoreCost,
    /// Candidate object-Link ClaimRefs held in the invocation-scoped Closure
    /// store. The finite compatibility path keeps its BTreeMap.
    pub candidate_object_link_store: SourceFoundationClosureObjectLinkStoreCost,
}

/// Plain current-record projection used only by the source-foundation Link
/// join. It carries no proof, capability, or admission authority.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureLink {
    pub id: String,
    pub path: String,
    pub value: Value,
}

/// Source-derived current event row retained by the candidate Closure store.
/// Its document digest binds the projection to the actual member read during
/// collection; the candidate source fence remains the authority for currentness.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureEvent {
    pub id: String,
    pub path: String,
    pub line: usize,
    pub document_sha256: String,
    pub value: Value,
}

/// Original physical source row addressed through the sealed event index.
/// The digest binds both the indexed event and raw row to one loaded document.
#[derive(Debug)]
pub struct SourceFoundationClosureRawEvent {
    pub path: String,
    pub line: u64,
    pub document_sha256: String,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureLinkStoreCost {
    pub inserted_rows: u64,
    pub drained_rows: u64,
    pub serialized_write_bytes: u64,
    pub serialized_read_bytes: u64,
    pub workspace_state_bytes: usize,
    pub scan_row_operations: u64,
}

/// Measured source-derived Expression-derivation scratch. These counters bind
/// the exact rows and ordered EOFs used by the candidate graph kernel; they
/// are not claims, proofs, or admission authority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureDerivationStoreCost {
    pub derivation_id_rows: u64,
    pub derivation_subject_rows: u64,
    pub derivation_pair_rows: u64,
    pub derivation_endpoint_rows: u64,
    pub derivation_evidence_path_rows: u64,
    pub derivation_expected_input_rows: u64,
    pub derivation_color_rows: u64,
    pub derivation_root_rows: u64,
    pub derivation_root_drained_rows: u64,
    pub derivation_root_eof_seen: bool,
    pub derivation_id_drained_rows: u64,
    pub derivation_id_eof_seen: bool,
    pub derivation_endpoint_drained_rows: u64,
    pub derivation_endpoint_eof_seen: bool,
    pub derivation_evidence_path_drained_rows: u64,
    pub derivation_evidence_path_eof_seen: bool,
    pub derivation_expected_input_drained_rows: u64,
    pub derivation_expected_input_eof_seen: bool,
    pub derivation_adjacency_rows: u64,
    pub derivation_adjacency_eof_count: u64,
    pub derivation_subject_stream_rows: u64,
    pub derivation_subject_stream_eof_count: u64,
    pub derivation_subject_stream_count_verified: bool,
    pub derivation_stack_push_rows: u64,
    pub derivation_stack_pop_rows: u64,
    pub derivation_stack_peak_rows: u64,
    pub derivation_duplicate_pair_rows: u64,
    pub derivation_claim_count_verified: bool,
    pub derivation_subject_count_verified: bool,
    pub derivation_pair_count_verified: bool,
    pub derivation_endpoint_count_verified: bool,
    pub derivation_evidence_path_count_verified: bool,
    pub derivation_expected_input_count_verified: bool,
    pub derivation_color_count_verified: bool,
    pub derivation_stack_empty: bool,
    pub derivation_finished: bool,
    pub serialized_read_bytes: u64,
    pub serialized_write_bytes: u64,
    pub scan_row_operations: u64,
    pub peak_workspace_state_bytes: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureTopologyStoreCost {
    pub claim_rows: u64,
    pub drained_rows: u64,
    pub subject_stream_rows: u64,
    pub subject_stream_eof_count: u64,
    pub serialized_read_bytes: u64,
    pub serialized_write_bytes: u64,
    pub scan_row_operations: u64,
    pub peak_workspace_state_bytes: usize,
    pub eof_seen: bool,
    pub count_verified: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureObjectLinkStoreCost {
    pub claim_rows: u64,
    pub drained_rows: u64,
    pub point_lookup_operations: u64,
    pub point_lookup_rows: u64,
    pub target_stream_count: u64,
    pub target_stream_rows: u64,
    pub target_stream_eof_count: u64,
    pub serialized_read_bytes: u64,
    pub serialized_write_bytes: u64,
    pub scan_row_operations: u64,
    pub peak_workspace_state_bytes: usize,
    pub eof_seen: bool,
    pub count_verified: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureBoundaryMembershipRefStoreCost {
    pub rows: u64,
    pub drained_rows: u64,
    pub serialized_read_bytes: u64,
    pub serialized_write_bytes: u64,
    pub scan_row_operations: u64,
    pub peak_workspace_state_bytes: usize,
    pub eof_seen: bool,
    pub count_verified: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureAnchorStoreCost {
    pub id_rows: u64,
    pub drained_rows: u64,
    pub serialized_read_bytes: u64,
    pub serialized_write_bytes: u64,
    pub scan_row_operations: u64,
    pub peak_workspace_state_bytes: usize,
    pub eof_seen: bool,
    pub count_verified: bool,
}

/// Exact ordered set owned by the held Closure derivation scratch store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationClosureDerivationKeySet {
    ClaimIds,
    Endpoints,
    EvidencePaths,
    ExpectedInputs,
    Roots,
}

/// One plain operational frame in the candidate Expression-derivation DFS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationClosureDerivationFrame {
    pub node: String,
    pub leaving: bool,
}

/// Portable row/point interface for candidate Closure Link joins. Implemented
/// storage remains an invocation-scoped CMD concern; the validator never
/// depends on SQLite and can retain its compatible finite map path.
pub trait SourceFoundationClosureLinkStore {
    fn insert_link(
        &mut self,
        id: &str,
        path: &str,
        value: &Value,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal>;

    fn contains_link(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn next_link(
        &mut self,
        after_id: Option<&str>,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureLink>, usize), ItemRefusal>;

    fn finish_links(
        &mut self,
        expected_rows: u64,
        max_state_bytes: usize,
    ) -> Result<SourceFoundationClosureLinkStoreCost, ItemRefusal>;

    fn verify_finished(&self) -> Result<(), ItemRefusal>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureSchemaRequestStoreCost {
    pub observation_rows: u64,
    pub serialized_write_bytes: u64,
    pub serialized_read_bytes: u64,
    pub workspace_state_bytes: usize,
    pub scan_row_operations: u64,
    pub loaded_document_rows: u64,
    pub loaded_document_serialized_read_bytes: u64,
    pub loaded_document_serialized_write_bytes: u64,
    pub loaded_document_scan_row_operations: u64,
    pub loaded_document_workspace_state_bytes: usize,
    pub loaded_rows: SourceFoundationClosureLoadedRowStoreCost,
    pub event_rows: u64,
    pub event_serialized_read_bytes: u64,
    pub event_serialized_write_bytes: u64,
    pub event_scan_row_operations: u64,
    pub event_workspace_state_bytes: usize,
    pub event_path_rows: u64,
    pub event_path_serialized_read_bytes: u64,
    pub event_path_serialized_write_bytes: u64,
    pub event_path_scan_row_operations: u64,
    pub event_path_workspace_state_bytes: usize,
    pub claim_id_rows: u64,
    pub claim_id_drained_rows: u64,
    pub claim_id_serialized_read_bytes: u64,
    pub claim_id_serialized_write_bytes: u64,
    pub claim_id_scan_row_operations: u64,
    pub claim_id_workspace_state_bytes: usize,
    pub claim_id_eof_seen: bool,
    pub membership_claim_rows: u64,
    pub membership_claim_drained_rows: u64,
    pub membership_claim_serialized_read_bytes: u64,
    pub membership_claim_serialized_write_bytes: u64,
    pub membership_claim_scan_row_operations: u64,
    pub membership_claim_workspace_state_bytes: usize,
    pub membership_claim_eof_seen: bool,
    pub responsibility_claim_rows: u64,
    pub responsibility_claim_drained_rows: u64,
    pub responsibility_claim_serialized_read_bytes: u64,
    pub responsibility_claim_serialized_write_bytes: u64,
    pub responsibility_claim_scan_row_operations: u64,
    pub responsibility_claim_workspace_state_bytes: usize,
    pub responsibility_claim_eof_seen: bool,
    pub publication_claim_rows: u64,
    pub publication_claim_drained_rows: u64,
    pub publication_claim_serialized_read_bytes: u64,
    pub publication_claim_serialized_write_bytes: u64,
    pub publication_claim_scan_row_operations: u64,
    pub publication_claim_workspace_state_bytes: usize,
    pub publication_claim_eof_seen: bool,
    pub publication_claim_count_verified: bool,
    pub provision_claim_rows: u64,
    pub provision_claim_drained_rows: u64,
    pub provision_claim_serialized_read_bytes: u64,
    pub provision_claim_serialized_write_bytes: u64,
    pub provision_claim_scan_row_operations: u64,
    pub provision_claim_workspace_state_bytes: usize,
    pub provision_claim_eof_seen: bool,
    pub provision_claim_count_verified: bool,
    pub provision_event_id_rows: u64,
    pub provision_event_id_drained_rows: u64,
    pub provision_event_id_serialized_read_bytes: u64,
    pub provision_event_id_serialized_write_bytes: u64,
    pub provision_event_id_scan_row_operations: u64,
    pub provision_event_id_workspace_state_bytes: usize,
    pub provision_event_id_lookup_rows: u64,
    pub provision_event_id_eof_seen: bool,
    pub provision_event_id_count_verified: bool,
    pub provision_unused_event_rows: u64,
    pub provision_unused_event_drained_rows: u64,
    pub provision_unused_event_serialized_read_bytes: u64,
    pub provision_unused_event_scan_row_operations: u64,
    pub provision_unused_event_workspace_state_bytes: usize,
    pub provision_unused_event_eof_seen: bool,
    pub provision_unused_event_count_verified: bool,
    pub provision_used_event_rows: u64,
    pub provision_used_event_serialized_read_bytes: u64,
    pub provision_used_event_serialized_write_bytes: u64,
    pub provision_used_event_scan_row_operations: u64,
    pub provision_used_event_workspace_state_bytes: usize,
    pub provision_used_event_count_verified: bool,
    pub provision_validated_event_rows: u64,
    pub provision_validated_event_serialized_read_bytes: u64,
    pub provision_validated_event_serialized_write_bytes: u64,
    pub provision_validated_event_scan_row_operations: u64,
    pub provision_validated_event_workspace_state_bytes: usize,
    pub provision_validated_event_count_verified: bool,
    pub responsibility_validated_event_rows: u64,
    pub responsibility_validated_event_serialized_read_bytes: u64,
    pub responsibility_validated_event_serialized_write_bytes: u64,
    pub responsibility_validated_event_scan_row_operations: u64,
    pub responsibility_validated_event_workspace_state_bytes: usize,
    pub responsibility_validated_event_count_verified: bool,
    pub publication_validated_event_rows: u64,
    pub publication_validated_event_serialized_read_bytes: u64,
    pub publication_validated_event_serialized_write_bytes: u64,
    pub publication_validated_event_scan_row_operations: u64,
    pub publication_validated_event_workspace_state_bytes: usize,
    pub publication_validated_event_count_verified: bool,
    pub boundary_responsibility_ref_rows: u64,
    pub boundary_responsibility_ref_drained_rows: u64,
    pub boundary_responsibility_ref_serialized_read_bytes: u64,
    pub boundary_responsibility_ref_serialized_write_bytes: u64,
    pub boundary_responsibility_ref_scan_row_operations: u64,
    pub boundary_responsibility_ref_workspace_state_bytes: usize,
    pub boundary_responsibility_ref_eof_seen: bool,
    pub boundary_responsibility_ref_count_verified: bool,
    pub boundary_membership_refs: SourceFoundationClosureBoundaryMembershipRefStoreCost,
    pub anchors: SourceFoundationClosureAnchorStoreCost,
    pub derivation: SourceFoundationClosureDerivationStoreCost,
    pub topology: SourceFoundationClosureTopologyStoreCost,
    pub object_links: SourceFoundationClosureObjectLinkStoreCost,
}

/// Actual source-line bytes retained for bounded point and ordered reads.
/// This is a mechanical projection, not a parsed-row authority or proof.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceFoundationClosureLoadedRowStoreCost {
    pub inserted_rows: u64,
    pub point_lookup_operations: u64,
    pub point_read_rows: u64,
    pub streamed_rows: u64,
    pub stream_eof_count: u64,
    pub serialized_write_bytes: u64,
    pub serialized_read_bytes: u64,
    pub scan_row_operations: u64,
    pub peak_workspace_state_bytes: usize,
    pub count_verified: bool,
}

/// Portable candidate spool for authentic Closure schema requests,
/// source-derived loaded-document digests, and current event projections.
/// Request encounter order, first-event identity, and issue insertion offsets
/// remain explicit; these rows carry no proof or admission authority.
pub trait SourceFoundationClosureSchemaRequestStore {
    /// Insert a claim ID once while preserving the first unique key. A false
    /// result means this ID was already encountered in the current claim
    /// stream.
    fn remember_claim_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Exact-key presence query used by later relation checks.
    fn contains_claim_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Upsert one ID-to-subject projection with BTreeMap replacement
    /// semantics. The result is true only for the first unique ID.
    fn remember_membership_claim(
        &mut self,
        id: &str,
        subject: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Exact-key membership claim lookup used by boundary references.
    fn contains_membership_claim(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Stream membership claim IDs for one subject in binary key order and
    /// return only after observing the selected query's true EOF.
    fn for_each_membership_claim_for_subject(
        &mut self,
        subject: &str,
        max_state_bytes: usize,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(u64, usize), ItemRefusal>;

    /// Upsert one plain Responsibility ClaimRef projection by exact ID. This
    /// follows the Closure BTreeMap's replacement behavior for repeated keys.
    fn remember_responsibility_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Exact-key lookup for boundary references and record backlink joins.
    fn responsibility_claim_by_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureClaimRef>, usize), ItemRefusal>;

    /// Exact-key existence query for work-boundary references.
    fn contains_responsibility_claim(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Stream one subject's claims in exact binary ID order. The callback
    /// receives the live provider workspace so callers can charge any retained
    /// projection before cloning the row.
    fn for_each_responsibility_claim_for_subject(
        &mut self,
        subject: &str,
        max_state_bytes: usize,
        visit: &mut dyn FnMut(
            &str,
            &SourceFoundationClosureClaimRef,
            usize,
        ) -> Result<(), ItemRefusal>,
    ) -> Result<(u64, usize), ItemRefusal>;

    /// Upsert one plain Publication ClaimRef projection by exact ID with the
    /// BTreeMap's last-write replacement behavior.
    fn remember_publication_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Seal the unique Publication ClaimRef count before its ordered drain.
    fn begin_publication_claims(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    /// Read one Publication ClaimRef in exact binary ID order. Returned row
    /// and retained cursor state are independently charged by the caller.
    fn next_publication_claim(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<
        (
            Option<SourceFoundationClosurePublicationClaim>,
            usize,
            usize,
            usize,
        ),
        ItemRefusal,
    >;

    /// Exact-key lookup for current-record backlink checks.
    fn publication_claim_by_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureClaimRef>, usize), ItemRefusal>;

    /// Stream one edition's Publication claims in exact binary ID order.
    fn for_each_publication_claim_for_subject(
        &mut self,
        subject: &str,
        max_state_bytes: usize,
        visit: &mut dyn FnMut(
            &str,
            &SourceFoundationClosureClaimRef,
            usize,
        ) -> Result<(), ItemRefusal>,
    ) -> Result<(u64, usize), ItemRefusal>;

    /// Upsert one plain Provision ClaimRef by exact ID with the BTreeMap's
    /// replacement behavior, then seal and drain unique rows in binary ID order.
    fn remember_provision_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn begin_provision_claims(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    fn next_provision_claim(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<
        (
            Option<SourceFoundationClosureProvisionClaim>,
            usize,
            usize,
            usize,
        ),
        ItemRefusal,
    >;

    /// Keep the source Provision-event set distinct from events merely used by
    /// claims and from events whose inputs have already been checked.
    fn remember_provision_event_id(
        &mut self,
        event_id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn remember_provision_used_event(
        &mut self,
        event_id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn contains_provision_used_event(
        &mut self,
        event_id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn remember_provision_validated_event(
        &mut self,
        event_id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn begin_provision_event_ids(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    fn next_provision_event_id(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize, usize, usize), ItemRefusal>;

    fn finish_provision(
        &mut self,
        expected_claim_rows: u64,
        expected_event_id_rows: u64,
        expected_used_event_rows: u64,
        expected_validated_event_rows: u64,
        expected_unused_event_rows: u64,
        max_state_bytes: usize,
    ) -> Result<(), ItemRefusal>;

    /// Store the candidate's source-derived derivation map key. Repeated IDs
    /// retain the map's last-write projection semantics.
    fn remember_derivation_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn contains_derivation_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn remember_derivation_subject(
        &mut self,
        id: &str,
        subject: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn remember_derivation_pair(
        &mut self,
        subject: &str,
        object: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn remember_derivation_endpoint(
        &mut self,
        endpoint: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn remember_derivation_evidence_path(
        &mut self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn remember_derivation_expected_input(
        &mut self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn begin_derivation_keyset(
        &mut self,
        set: SourceFoundationClosureDerivationKeySet,
        expected_rows: u64,
    ) -> Result<(), ItemRefusal>;

    fn next_derivation_key(
        &mut self,
        set: SourceFoundationClosureDerivationKeySet,
        after: Option<&str>,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize, usize), ItemRefusal>;

    fn for_each_derivation_subject_claim(
        &mut self,
        subject: &str,
        max_state_bytes: usize,
        visit: &mut dyn FnMut(&str, usize) -> Result<(), ItemRefusal>,
    ) -> Result<(u64, usize), ItemRefusal>;

    fn derivation_root_count(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(u64, usize), ItemRefusal>;

    fn next_derivation_child(
        &mut self,
        subject: &str,
        after_descending: Option<&str>,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize, usize), ItemRefusal>;

    fn derivation_color(
        &mut self,
        node: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<u8>, usize), ItemRefusal>;

    fn set_derivation_color(
        &mut self,
        node: &str,
        color: u8,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal>;

    fn push_derivation_frame(
        &mut self,
        frame: &SourceFoundationClosureDerivationFrame,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal>;

    fn pop_derivation_frame(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureDerivationFrame>, usize, usize), ItemRefusal>;

    fn finish_derivation(
        &mut self,
        expected_subject_streams: u64,
        max_state_bytes: usize,
    ) -> Result<SourceFoundationClosureDerivationStoreCost, ItemRefusal>;

    /// Remember an event whose inputs have already been checked for this
    /// Responsibility pass. The key includes this check kind so other Closure
    /// passes can share the held mechanism without sharing validation state.
    fn remember_responsibility_validated_event(
        &mut self,
        event_id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Remember a provenance event already validated for Publication, in a
    /// distinct key namespace from other Closure passes.
    fn remember_publication_validated_event(
        &mut self,
        event_id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Insert one boundary reference into the exact unique-ID set. Duplicate
    /// references are retained once, matching the finite BTreeSet.
    fn remember_boundary_responsibility_ref(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Seal the exact number of unique boundary references before draining.
    fn begin_boundary_responsibility_refs(&mut self, expected_rows: u64)
    -> Result<(), ItemRefusal>;

    /// Return one boundary reference in binary order. The returned row and
    /// retained cursor state are caller-charged until the next read/EOF.
    fn next_boundary_responsibility_ref(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize, usize, usize), ItemRefusal>;

    /// Insert one unique boundary Membership reference. The binary key order
    /// and duplicate law match the finite BTreeSet projection.
    fn remember_boundary_membership_ref(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn begin_boundary_membership_refs(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    fn next_boundary_membership_ref(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize, usize, usize), ItemRefusal>;

    /// Insert or test one unique source evidence anchor ID in the held set.
    /// Duplicate observations return false and preserve the set's first key.
    fn remember_anchor_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Exact membership lookup after the collected anchor set is sealed.
    fn contains_anchor_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Seal and verify a binary-ordered traversal of the unique anchor IDs.
    fn begin_anchor_ids(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    fn next_anchor_id(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize, usize, usize), ItemRefusal>;

    fn finish_anchor_ids(
        &mut self,
        expected_rows: u64,
        max_state_bytes: usize,
    ) -> Result<SourceFoundationClosureAnchorStoreCost, ItemRefusal>;

    /// Seal the unique Responsibility ClaimRef count before its ordered drain.
    fn begin_responsibility_claims(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    /// Read the next Responsibility ClaimRef in exact binary ID order. The
    /// workspace is the provider peak; row state stays live for the caller,
    /// and cursor state remains provider-owned until the next read or EOF.
    fn next_responsibility_claim(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureClaimRef>, usize, usize, usize), ItemRefusal>;

    /// Record a source-derived document digest once. Repeated paths must
    /// carry the same digest; a mismatch means the exact current cut moved.
    fn observe_loaded_document(
        &mut self,
        path: &str,
        digest: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Point lookup for later Closure passes. The digest comes from the
    /// actual first-load current bytes. Later lookups use that invocation's
    /// held source projection; the enclosing inspection verifies the same
    /// candidate input identity and current fence before and after the walk.
    /// This does not re-read or re-hash the member on each point lookup.
    fn loaded_document_digest(
        &mut self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize), ItemRefusal>;

    /// Store one successfully parsed physical source line without changing
    /// its bytes. Rows are keyed by exact path and physical line and carry the
    /// digest of the document from the same first-load read.
    fn remember_loaded_row(
        &mut self,
        path: &str,
        line: usize,
        document_sha256: &str,
        raw_line: &[u8],
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Bind a document's loaded-row count to the actual rows stored for its
    /// first-load digest. This is called after schema and issue traversal.
    fn finish_loaded_document_rows(
        &mut self,
        path: &str,
        document_sha256: &str,
        expected_rows: u64,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal>;

    /// Return the sealed row count for a previously loaded document.
    fn loaded_document_row_count(
        &mut self,
        path: &str,
        document_sha256: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<u64>, usize), ItemRefusal>;

    /// Exact physical-line point lookup. A missing line is distinct from a
    /// missing document; digest mismatch refuses the candidate projection.
    fn loaded_row(
        &mut self,
        path: &str,
        line: usize,
        document_sha256: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<Vec<u8>>, usize), ItemRefusal>;

    /// Read the next valid source line in strict physical-line order. `None`
    /// is returned only after the selected document query reaches EOF.
    fn next_loaded_row(
        &mut self,
        path: &str,
        document_sha256: &str,
        after_line: Option<usize>,
        max_state_bytes: usize,
    ) -> Result<(Option<(usize, Vec<u8>)>, usize), ItemRefusal>;

    /// Insert the first current event under its exact ID. Duplicate IDs are
    /// reported as `false` without replacing the first source row.
    fn remember_event(
        &mut self,
        id: &str,
        path: &str,
        line: usize,
        document_sha256: &str,
        value: &Value,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Exact-key presence check for a current candidate event.
    fn contains_event(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Point-read one event projection. The caller checks its path and digest
    /// against the same candidate's loaded-document marker and source fence.
    fn event_value(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureEvent>, usize), ItemRefusal>;

    /// Read an exact event's original source line only after collection is sealed.
    /// Missing IDs return None; missing rows or mismatched document bindings refuse.
    /// Costs remain cumulative, including reads performed after `finish`.
    fn sealed_event_raw(
        &mut self,
        id: &str,
        max_raw_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureRawEvent>, usize), ItemRefusal>;

    /// Queue one selected current event path during the authenticated path
    /// walk; source reads happen only after that walk reaches EOF.
    fn remember_event_path(
        &mut self,
        path: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Seal the exact number of queued current event paths before ordered
    /// point processing begins.
    fn seal_event_paths(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    /// Read one selected path in strict binary order, or `None` only after the
    /// store observes the actual ordered EOF.
    fn next_event_path(
        &mut self,
        after_path: Option<&str>,
        max_state_bytes: usize,
    ) -> Result<(Option<String>, usize, usize), ItemRefusal>;

    /// Bind the path queue's drain count to its sealed insert count and EOF.
    fn finish_event_paths(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    fn record_request(
        &mut self,
        request: &SourceFoundationClosureSchemaRequest,
        max_document_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<usize, ItemRefusal>;

    fn finish(
        &mut self,
        expected_rows: u64,
        expected_loaded_documents: u64,
        expected_loaded_row_store: SourceFoundationClosureLoadedRowStoreCost,
        expected_event_rows: u64,
        expected_event_path_rows: u64,
        expected_claim_id_rows: u64,
        expected_membership_claim_rows: u64,
        expected_responsibility_claim_rows: u64,
        expected_publication_claim_rows: u64,
        expected_responsibility_validated_event_rows: u64,
        expected_publication_validated_event_rows: u64,
        expected_boundary_responsibility_ref_rows: u64,
        expected_boundary_membership_ref_rows: u64,
        expected_anchor_id_rows: u64,
        expected_provision_claim_rows: u64,
        expected_provision_event_id_rows: u64,
        expected_provision_used_event_rows: u64,
        expected_provision_validated_event_rows: u64,
        expected_provision_unused_event_rows: u64,
        expected_topology_claim_rows: u64,
        expected_object_link_claim_rows: u64,
        direct_issue_count: usize,
        max_state_bytes: usize,
    ) -> Result<SourceFoundationClosureSchemaRequestStoreCost, ItemRefusal>;

    /// Upsert one source-derived topology ClaimRef using the old map's
    /// last-write projection semantics.
    fn remember_topology_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    fn topology_claim_by_id(
        &mut self,
        id: &str,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureClaimRef>, usize), ItemRefusal>;

    /// Stream one predicate/subject projection in strict binary Claim ID
    /// order; the returned count is valid only after actual query EOF.
    fn for_each_topology_claim_for_subject(
        &mut self,
        subject: &str,
        predicate: &str,
        max_state_bytes: usize,
        visit: &mut dyn FnMut(&str, usize) -> Result<(), ItemRefusal>,
    ) -> Result<(u64, usize), ItemRefusal>;

    /// Upsert one source-derived object-Link ClaimRef using the old map's
    /// last-write behavior.
    fn remember_object_link_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
        max_state_bytes: usize,
    ) -> Result<(bool, usize), ItemRefusal>;

    /// Seal the unique object-Link projection before its ordered drain.
    fn begin_object_link_claims(&mut self, expected_rows: u64) -> Result<(), ItemRefusal>;

    /// Read the next object-Link ClaimRef in exact binary ID order. The store
    /// retains and validates its keyset cursor; the caller accounts for the
    /// returned row and retained cursor until the next read or EOF.
    fn next_object_link_claim(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<
        (
            Option<(String, SourceFoundationClosureClaimRef)>,
            usize,
            usize,
            usize,
        ),
        ItemRefusal,
    >;

    /// Check whether an association exists and whether it targets this Link
    /// and cites the Link's provenance event.
    fn object_link_relation(
        &mut self,
        id: &str,
        link_id: &str,
        event_id: &str,
        max_state_bytes: usize,
    ) -> Result<(bool, bool, bool, usize), ItemRefusal>;

    /// Stream object-Link IDs targeting one Link in binary order. The count is
    /// returned only after the selected index query reaches true EOF.
    fn for_each_object_link_target(
        &mut self,
        link_id: &str,
        max_state_bytes: usize,
        visit: &mut dyn FnMut(&str, usize) -> Result<(), ItemRefusal>,
    ) -> Result<(u64, usize), ItemRefusal>;

    fn next_request(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<(Option<SourceFoundationClosureSchemaRequest>, usize), ItemRefusal>;

    fn cost(&self) -> SourceFoundationClosureSchemaRequestStoreCost;

    fn verify_finished(&self) -> Result<(), ItemRefusal>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationClosureGap {
    pub location: String,
    pub profile: String,
}

/// Ordered source-only findings. `requires_bibliographic` is a route hint for
/// the caller, based on source-declared profile kinds and exact current file
/// membership; it is not a validation verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureReport {
    pub issues: Vec<(String, String)>,
    /// Exact decoded documents awaiting the caller's diagnostic-v2 schema
    /// worker. Schema output is inserted immediately before `before_issue`.
    pub schema_requests: Vec<SourceFoundationClosureSchemaRequest>,
    pub unsupported: Vec<SourceFoundationClosureGap>,
    pub requires_bibliographic: bool,
    pub cost: SourceFoundationClosureCost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureSchemaRequest {
    pub before_issue: usize,
    pub location: String,
    pub contract: String,
    pub document: Value,
}

#[derive(Debug, Clone)]
struct LoadedRows {
    digest: String,
    rows: Vec<(usize, Value)>,
    temporary_state_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureClaimRef {
    pub location: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub event: String,
    pub native: bool,
}

/// Plain Publication ClaimRef row selected from the exact current cut.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosurePublicationClaim {
    pub id: String,
    pub reference: SourceFoundationClosureClaimRef,
}

/// Plain Provision ClaimRef projection selected from the exact current cut.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureProvisionClaim {
    pub id: String,
    pub reference: SourceFoundationClosureClaimRef,
}

/// Cold compatibility entry for the cross-stream closure district. The
/// caller supplies materialized views from the exact captured cut; the
/// identity-bearing entry below runs the same predicates over bounded stored
/// lookups without rebuilding those maps. No repository walk, mutable
/// checkout read, catalog read, or source admission is performed here.
pub fn inspect_source_foundation_closure<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    cut: &CorpusCutReader,
    source_events: &BTreeMap<String, Value>,
    current_records: &BTreeMap<String, BiblioCurrentRecord>,
    item_editions: &BTreeMap<String, String>,
    current_paths: &[String],
    file_memberships: &SourceFileMembershipIndex,
    rights_ids: &BTreeSet<String>,
    declared_profile_kinds: &BTreeSet<String>,
    bibliographic_claims: &[BiblioClaim],
    limits: ItemLimits,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    validate_legacy_cut_paths(cut, current_paths, limits.deadline, source.cancellation())?;
    let records = BorrowedDefaultRecords {
        current_records,
        item_editions,
        rights_ids,
        file_memberships,
        declared_profile_kinds,
    };
    let paths = SliceDefaultPaths(current_paths);
    let claims = SliceDefaultClaims(bibliographic_claims);
    run_source_foundation_closure(
        source,
        source_events,
        &records,
        &paths,
        &claims,
        None,
        None,
        limits,
        true,
        true,
        SourceFoundationDefaultRuleScope::FullAudit,
    )
}

/// Run the same closure predicates against a current candidate carrying its
/// real input identity and the exact EOF coverage already produced by the
/// Records pass. The owner supplies bounded lookup views over its completed
/// Records, event, claim and path indexes; this adapter creates no revision
/// and reconstructs no resident corpus maps.
#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_closure_with_identity<I: Eq, S: LayerFamilySource + ?Sized>(
    source: &mut S,
    input: &dyn SourceCutInputWithIdentity<I>,
    expected_identity: &I,
    coverage: &SourceCutInputCoverage,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    claims: &dyn SourceFoundationDefaultClaims,
    limits: ItemLimits,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    inspect_source_foundation_closure_with_identity_and_link_store_cache(
        source,
        input,
        expected_identity,
        coverage,
        source_events,
        records,
        paths,
        claims,
        None,
        None,
        limits,
        true,
        SourceFoundationDefaultRuleScope::FullAudit,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_closure_with_identity_and_link_store<
    I: Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn SourceCutInputWithIdentity<I>,
    expected_identity: &I,
    coverage: &SourceCutInputCoverage,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    claims: &dyn SourceFoundationDefaultClaims,
    link_store: Option<&mut dyn SourceFoundationClosureLinkStore>,
    limits: ItemLimits,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    inspect_source_foundation_closure_with_identity_and_link_store_cache(
        source,
        input,
        expected_identity,
        coverage,
        source_events,
        records,
        paths,
        claims,
        link_store,
        None,
        limits,
        false,
        SourceFoundationDefaultRuleScope::FullAudit,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_closure_with_identity_and_candidate_stores<
    I: Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn SourceCutInputWithIdentity<I>,
    expected_identity: &I,
    coverage: &SourceCutInputCoverage,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    claims: &dyn SourceFoundationDefaultClaims,
    link_store: Option<&mut dyn SourceFoundationClosureLinkStore>,
    schema_request_store: Option<&mut dyn SourceFoundationClosureSchemaRequestStore>,
    limits: ItemLimits,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    inspect_source_foundation_closure_with_identity_and_link_store_cache(
        source,
        input,
        expected_identity,
        coverage,
        source_events,
        records,
        paths,
        claims,
        link_store,
        schema_request_store,
        limits,
        false,
        SourceFoundationDefaultRuleScope::FullAudit,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_closure_with_identity_and_candidate_stores_with_scope<
    I: Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn SourceCutInputWithIdentity<I>,
    expected_identity: &I,
    coverage: &SourceCutInputCoverage,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    claims: &dyn SourceFoundationDefaultClaims,
    link_store: Option<&mut dyn SourceFoundationClosureLinkStore>,
    schema_request_store: Option<&mut dyn SourceFoundationClosureSchemaRequestStore>,
    limits: ItemLimits,
    scope: SourceFoundationDefaultRuleScope,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    inspect_source_foundation_closure_with_identity_and_link_store_cache(
        source,
        input,
        expected_identity,
        coverage,
        source_events,
        records,
        paths,
        claims,
        link_store,
        schema_request_store,
        limits,
        false,
        scope,
    )
}

#[allow(clippy::too_many_arguments)]
fn inspect_source_foundation_closure_with_identity_and_link_store_cache<
    I: Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn SourceCutInputWithIdentity<I>,
    expected_identity: &I,
    coverage: &SourceCutInputCoverage,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    claims: &dyn SourceFoundationDefaultClaims,
    link_store: Option<&mut dyn SourceFoundationClosureLinkStore>,
    schema_request_store: Option<&mut dyn SourceFoundationClosureSchemaRequestStore>,
    limits: ItemLimits,
    cache_recorded_checks: bool,
    scope: SourceFoundationDefaultRuleScope,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    let cancelled = source.cancellation();
    check(limits.deadline, cancelled)?;
    if input.input_identity() != expected_identity {
        return Err(ItemRefusal::Source(
            "source-foundation closure input identity differs from completed Records".into(),
        ));
    }
    input
        .source_input()
        .verify_current_fence(coverage, limits.deadline, cancelled)?;
    let result = run_source_foundation_closure(
        source,
        source_events,
        records,
        paths,
        claims,
        link_store,
        schema_request_store,
        limits,
        false,
        cache_recorded_checks,
        scope,
    );
    check(limits.deadline, source.cancellation())?;
    if input.input_identity() != expected_identity {
        return Err(ItemRefusal::Source(
            "source-foundation closure input identity changed during inspection".into(),
        ));
    }
    input
        .source_input()
        .verify_current_fence(coverage, limits.deadline, source.cancellation())?;
    result
}

fn run_source_foundation_closure<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    claims: &dyn SourceFoundationDefaultClaims,
    link_store: Option<&mut dyn SourceFoundationClosureLinkStore>,
    schema_request_store: Option<&mut dyn SourceFoundationClosureSchemaRequestStore>,
    limits: ItemLimits,
    cache_digests: bool,
    cache_recorded_checks: bool,
    scope: SourceFoundationDefaultRuleScope,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    let mut has_declared_profile_kind = false;
    records.for_each_profile_kind(&mut |kind| {
        check(limits.deadline, source.cancellation())?;
        let _ = kind;
        has_declared_profile_kind = true;
        Ok(())
    })?;
    let mut requires_bibliographic = has_declared_profile_kind;
    paths.for_each_path(&mut |path| {
        check(limits.deadline, source.cancellation())?;
        if !source.selects_semantic_member(path)? {
            return Ok(());
        }
        requires_bibliographic |= path.starts_with(SOURCE_HOME)
            && (path.ends_with("/historical-claims.jsonl")
                || path.ends_with("/source-claims.jsonl"));
        Ok(())
    })?;
    let mut rules = ClosureRules::new(
        source,
        source_events,
        records,
        paths,
        claims,
        link_store,
        schema_request_store,
        limits,
        cache_digests,
        cache_recorded_checks,
        scope,
    )?;
    rules.check_records_map()?;
    rules.collect_events()?;
    rules.check_boundary_maps_and_anchors()?;
    rules.check_claim_streams()?;
    if scope.is_scoped() {
        rules.check_selected_derivation_graph()?;
        for (_, predicate, subject_kind, _, backref, _) in TOPOLOGY_ROUTES {
            rules.check_topology_backrefs(subject_kind, backref, predicate)?;
        }
    }
    if scope == SourceFoundationDefaultRuleScope::FullAudit {
        rules.check_topology()?;
        rules.check_derivation()?;
    }
    rules.check_responsibility_claims()?;
    rules.check_publication_claims()?;
    rules.check_provision_activity()?;
    if scope == SourceFoundationDefaultRuleScope::FullAudit {
        rules.check_chronology()?;
    }
    rules.check_object_links()?;
    rules.check_record_backlinks()?;

    let expected_schema_rows = rules.cost.schema_requests;
    let expected_loaded_documents = rules.cost.candidate_loaded_document_count;
    let expected_loaded_row_store = rules.cost.candidate_loaded_row_store;
    let expected_event_rows = rules.cost.candidate_event_count;
    let expected_event_path_rows = rules.cost.candidate_event_path_count;
    let expected_claim_id_rows = rules.cost.candidate_claim_id_count;
    let expected_membership_claim_rows = rules.cost.candidate_membership_claim_count;
    let expected_responsibility_claim_rows = rules.cost.candidate_responsibility_claim_count;
    let expected_publication_claim_rows = rules.cost.candidate_publication_claim_count;
    let expected_responsibility_validated_event_rows =
        rules.cost.candidate_responsibility_validated_event_count;
    let expected_publication_validated_event_rows =
        rules.cost.candidate_publication_validated_event_count;
    let expected_boundary_responsibility_ref_rows =
        rules.cost.candidate_boundary_responsibility_ref_count;
    let expected_boundary_membership_ref_rows = rules.cost.candidate_boundary_membership_refs.rows;
    let expected_anchor_store = rules.cost.candidate_anchor_store;
    let expected_provision_claim_rows = rules.cost.candidate_provision_claim_count;
    let expected_provision_event_id_rows = rules.cost.candidate_provision_event_id_count;
    let expected_provision_used_event_rows = rules.cost.candidate_provision_used_event_count;
    let expected_provision_validated_event_rows =
        rules.cost.candidate_provision_validated_event_count;
    let expected_provision_unused_event_rows = rules.cost.candidate_provision_unused_event_count;
    let expected_topology_claim_rows = rules.cost.candidate_topology_store.claim_rows;
    let expected_object_link_claim_rows = rules.cost.candidate_object_link_store.claim_rows;
    let schema_request_finish = if rules.schema_request_store.is_some() {
        let direct_issue_count = rules.issues.len();
        let remaining = rules.remaining_state()?;
        let finished = rules
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .finish(
                expected_schema_rows,
                expected_loaded_documents,
                expected_loaded_row_store,
                expected_event_rows,
                expected_event_path_rows,
                expected_claim_id_rows,
                expected_membership_claim_rows,
                expected_responsibility_claim_rows,
                expected_publication_claim_rows,
                expected_responsibility_validated_event_rows,
                expected_publication_validated_event_rows,
                expected_boundary_responsibility_ref_rows,
                expected_boundary_membership_ref_rows,
                expected_anchor_store.id_rows,
                expected_provision_claim_rows,
                expected_provision_event_id_rows,
                expected_provision_used_event_rows,
                expected_provision_validated_event_rows,
                expected_provision_unused_event_rows,
                expected_topology_claim_rows,
                expected_object_link_claim_rows,
                direct_issue_count,
                remaining,
            )?;
        Some(finished)
    } else {
        None
    };
    if let Some(finished) = schema_request_finish {
        let combined = rules
            .retained_state_bytes
            .checked_add(rules.temporary_state_bytes)
            .and_then(|state| {
                state.checked_add(
                    finished
                        .workspace_state_bytes
                        .max(finished.loaded_document_workspace_state_bytes)
                        .max(finished.loaded_rows.peak_workspace_state_bytes)
                        .max(finished.event_workspace_state_bytes)
                        .max(finished.event_path_workspace_state_bytes)
                        .max(finished.claim_id_workspace_state_bytes)
                        .max(finished.membership_claim_workspace_state_bytes)
                        .max(finished.responsibility_claim_workspace_state_bytes)
                        .max(finished.publication_claim_workspace_state_bytes)
                        .max(finished.responsibility_validated_event_workspace_state_bytes)
                        .max(finished.publication_validated_event_workspace_state_bytes)
                        .max(finished.boundary_responsibility_ref_workspace_state_bytes)
                        .max(finished.boundary_membership_refs.peak_workspace_state_bytes)
                        .max(finished.anchors.peak_workspace_state_bytes)
                        .max(finished.provision_claim_workspace_state_bytes)
                        .max(finished.provision_event_id_workspace_state_bytes)
                        .max(finished.provision_unused_event_workspace_state_bytes)
                        .max(finished.provision_used_event_workspace_state_bytes)
                        .max(finished.provision_validated_event_workspace_state_bytes)
                        .max(finished.topology.peak_workspace_state_bytes)
                        .max(finished.object_links.peak_workspace_state_bytes)
                        .max(finished.derivation.peak_workspace_state_bytes),
                )
            })
            .ok_or(ItemRefusal::Budget)?;
        if combined > rules.limits.max_state_bytes
            || finished.observation_rows != expected_schema_rows
            || finished.claim_id_rows != expected_claim_id_rows
            || finished.claim_id_drained_rows != expected_claim_id_rows
            || !finished.claim_id_eof_seen
            || finished.membership_claim_rows != expected_membership_claim_rows
            || finished.membership_claim_drained_rows != expected_membership_claim_rows
            || !finished.membership_claim_eof_seen
            || finished.responsibility_claim_rows != expected_responsibility_claim_rows
            || finished.responsibility_claim_drained_rows != expected_responsibility_claim_rows
            || !finished.responsibility_claim_eof_seen
            || finished.publication_claim_rows != expected_publication_claim_rows
            || finished.publication_claim_drained_rows != expected_publication_claim_rows
            || !finished.publication_claim_eof_seen
            || !finished.publication_claim_count_verified
            || finished.responsibility_validated_event_rows
                != expected_responsibility_validated_event_rows
            || !finished.responsibility_validated_event_count_verified
            || finished.publication_validated_event_rows
                != expected_publication_validated_event_rows
            || !finished.publication_validated_event_count_verified
            || finished.boundary_responsibility_ref_rows
                != expected_boundary_responsibility_ref_rows
            || finished.boundary_responsibility_ref_drained_rows
                != expected_boundary_responsibility_ref_rows
            || !finished.boundary_responsibility_ref_eof_seen
            || !finished.boundary_responsibility_ref_count_verified
            || finished.boundary_membership_refs.rows != expected_boundary_membership_ref_rows
            || finished.boundary_membership_refs.drained_rows
                != expected_boundary_membership_ref_rows
            || !finished.boundary_membership_refs.eof_seen
            || !finished.boundary_membership_refs.count_verified
            || finished.anchors != expected_anchor_store
            || finished.provision_claim_rows != expected_provision_claim_rows
            || finished.provision_claim_drained_rows != expected_provision_claim_rows
            || !finished.provision_claim_eof_seen
            || !finished.provision_claim_count_verified
            || finished.provision_event_id_rows != expected_provision_event_id_rows
            || finished.provision_event_id_drained_rows != expected_provision_event_id_rows
            || finished.provision_event_id_lookup_rows != expected_provision_event_id_rows
            || !finished.provision_event_id_eof_seen
            || !finished.provision_event_id_count_verified
            || finished.provision_used_event_rows != expected_provision_used_event_rows
            || !finished.provision_used_event_count_verified
            || finished.provision_validated_event_rows != expected_provision_validated_event_rows
            || !finished.provision_validated_event_count_verified
            || finished.provision_unused_event_rows != expected_provision_unused_event_rows
            || finished.provision_unused_event_drained_rows != expected_provision_unused_event_rows
            || !finished.provision_unused_event_eof_seen
            || !finished.provision_unused_event_count_verified
            || finished.topology.claim_rows != expected_topology_claim_rows
            || finished.topology.drained_rows != expected_topology_claim_rows
            || !finished.topology.eof_seen
            || !finished.topology.count_verified
            || finished.object_links.claim_rows != expected_object_link_claim_rows
            || finished.object_links.drained_rows != expected_object_link_claim_rows
            || !finished.object_links.eof_seen
            || !finished.object_links.count_verified
            || finished.derivation != rules.cost.candidate_derivation_store
            || finished.loaded_rows.inserted_rows != expected_loaded_row_store.inserted_rows
            || finished.loaded_rows.point_lookup_operations
                != expected_loaded_row_store.point_lookup_operations
            || finished.loaded_rows.point_read_rows != expected_loaded_row_store.point_read_rows
            || finished.loaded_rows.streamed_rows != expected_loaded_row_store.streamed_rows
            || finished.loaded_rows.stream_eof_count != expected_loaded_row_store.stream_eof_count
            || !finished.loaded_rows.count_verified
        {
            return Err(ItemRefusal::Source(
                "source-foundation Closure schema request store count or state differs".into(),
            ));
        }
        rules.cost.reserved_state_bytes = rules.cost.reserved_state_bytes.max(combined);
        rules.cost.candidate_object_link_store = finished.object_links;
        rules.cost.candidate_schema_request_count = finished.observation_rows;
        rules.cost.candidate_schema_request_serialized_write_bytes =
            finished.serialized_write_bytes;
        rules.cost.candidate_schema_request_scan_row_operations = finished.scan_row_operations;
        rules
            .cost
            .candidate_schema_request_peak_workspace_state_bytes = finished.workspace_state_bytes;
        rules.cost.candidate_loaded_document_count = finished.loaded_document_rows;
        rules.cost.candidate_loaded_document_serialized_read_bytes =
            finished.loaded_document_serialized_read_bytes;
        rules.cost.candidate_loaded_document_serialized_write_bytes =
            finished.loaded_document_serialized_write_bytes;
        rules.cost.candidate_loaded_document_scan_row_operations =
            finished.loaded_document_scan_row_operations;
        rules
            .cost
            .candidate_loaded_document_peak_workspace_state_bytes =
            finished.loaded_document_workspace_state_bytes;
        rules.cost.candidate_loaded_row_store = finished.loaded_rows;
        rules.cost.candidate_event_count = finished.event_rows;
        rules.cost.candidate_event_serialized_read_bytes = finished.event_serialized_read_bytes;
        rules.cost.candidate_event_serialized_write_bytes = finished.event_serialized_write_bytes;
        rules.cost.candidate_event_scan_row_operations = finished.event_scan_row_operations;
        rules.cost.candidate_event_peak_workspace_state_bytes =
            finished.event_workspace_state_bytes;
        rules.cost.candidate_event_path_count = finished.event_path_rows;
        rules.cost.candidate_event_path_serialized_read_bytes =
            finished.event_path_serialized_read_bytes;
        rules.cost.candidate_event_path_serialized_write_bytes =
            finished.event_path_serialized_write_bytes;
        rules.cost.candidate_event_path_scan_row_operations =
            finished.event_path_scan_row_operations;
        rules.cost.candidate_event_path_peak_workspace_state_bytes =
            finished.event_path_workspace_state_bytes;
        rules.cost.candidate_claim_id_count = finished.claim_id_rows;
        rules.cost.candidate_claim_id_serialized_read_bytes =
            finished.claim_id_serialized_read_bytes;
        rules.cost.candidate_claim_id_serialized_write_bytes =
            finished.claim_id_serialized_write_bytes;
        rules.cost.candidate_claim_id_scan_row_operations = finished.claim_id_scan_row_operations;
        rules.cost.candidate_claim_id_peak_workspace_state_bytes =
            finished.claim_id_workspace_state_bytes;
        rules.cost.candidate_membership_claim_count = finished.membership_claim_rows;
        rules.cost.candidate_membership_claim_serialized_read_bytes =
            finished.membership_claim_serialized_read_bytes;
        rules.cost.candidate_membership_claim_serialized_write_bytes =
            finished.membership_claim_serialized_write_bytes;
        rules.cost.candidate_membership_claim_scan_row_operations =
            finished.membership_claim_scan_row_operations;
        rules
            .cost
            .candidate_membership_claim_peak_workspace_state_bytes =
            finished.membership_claim_workspace_state_bytes;
        rules.cost.candidate_responsibility_claim_count = finished.responsibility_claim_rows;
        rules
            .cost
            .candidate_responsibility_claim_serialized_read_bytes =
            finished.responsibility_claim_serialized_read_bytes;
        rules
            .cost
            .candidate_responsibility_claim_serialized_write_bytes =
            finished.responsibility_claim_serialized_write_bytes;
        rules
            .cost
            .candidate_responsibility_claim_scan_row_operations =
            finished.responsibility_claim_scan_row_operations;
        rules
            .cost
            .candidate_responsibility_claim_peak_workspace_state_bytes =
            finished.responsibility_claim_workspace_state_bytes;
        rules.cost.candidate_publication_claim_count = finished.publication_claim_rows;
        rules.cost.candidate_publication_claim_serialized_read_bytes =
            finished.publication_claim_serialized_read_bytes;
        rules
            .cost
            .candidate_publication_claim_serialized_write_bytes =
            finished.publication_claim_serialized_write_bytes;
        rules.cost.candidate_publication_claim_scan_row_operations =
            finished.publication_claim_scan_row_operations;
        rules
            .cost
            .candidate_publication_claim_peak_workspace_state_bytes =
            finished.publication_claim_workspace_state_bytes;
        rules.cost.candidate_responsibility_validated_event_count =
            finished.responsibility_validated_event_rows;
        rules
            .cost
            .candidate_responsibility_validated_event_serialized_read_bytes =
            finished.responsibility_validated_event_serialized_read_bytes;
        rules
            .cost
            .candidate_responsibility_validated_event_serialized_write_bytes =
            finished.responsibility_validated_event_serialized_write_bytes;
        rules
            .cost
            .candidate_responsibility_validated_event_scan_row_operations =
            finished.responsibility_validated_event_scan_row_operations;
        rules
            .cost
            .candidate_responsibility_validated_event_peak_workspace_state_bytes =
            finished.responsibility_validated_event_workspace_state_bytes;
        rules.cost.candidate_publication_validated_event_count =
            finished.publication_validated_event_rows;
        rules
            .cost
            .candidate_publication_validated_event_serialized_read_bytes =
            finished.publication_validated_event_serialized_read_bytes;
        rules
            .cost
            .candidate_publication_validated_event_serialized_write_bytes =
            finished.publication_validated_event_serialized_write_bytes;
        rules
            .cost
            .candidate_publication_validated_event_scan_row_operations =
            finished.publication_validated_event_scan_row_operations;
        rules
            .cost
            .candidate_publication_validated_event_peak_workspace_state_bytes =
            finished.publication_validated_event_workspace_state_bytes;
        rules.cost.candidate_boundary_responsibility_ref_count =
            finished.boundary_responsibility_ref_rows;
        rules
            .cost
            .candidate_boundary_responsibility_ref_serialized_read_bytes =
            finished.boundary_responsibility_ref_serialized_read_bytes;
        rules
            .cost
            .candidate_boundary_responsibility_ref_serialized_write_bytes =
            finished.boundary_responsibility_ref_serialized_write_bytes;
        rules
            .cost
            .candidate_boundary_responsibility_ref_scan_row_operations =
            finished.boundary_responsibility_ref_scan_row_operations;
        rules
            .cost
            .candidate_boundary_responsibility_ref_peak_workspace_state_bytes =
            finished.boundary_responsibility_ref_workspace_state_bytes;
        rules.cost.candidate_boundary_membership_refs = finished.boundary_membership_refs;
        rules.cost.candidate_anchor_store = finished.anchors;
        rules.cost.candidate_provision_claim_count = finished.provision_claim_rows;
        rules.cost.candidate_provision_claim_serialized_read_bytes =
            finished.provision_claim_serialized_read_bytes;
        rules.cost.candidate_provision_claim_serialized_write_bytes =
            finished.provision_claim_serialized_write_bytes;
        rules.cost.candidate_provision_claim_scan_row_operations =
            finished.provision_claim_scan_row_operations;
        rules
            .cost
            .candidate_provision_claim_peak_workspace_state_bytes =
            finished.provision_claim_workspace_state_bytes;
        rules.cost.candidate_provision_event_id_count = finished.provision_event_id_rows;
        rules
            .cost
            .candidate_provision_event_id_serialized_read_bytes =
            finished.provision_event_id_serialized_read_bytes;
        rules
            .cost
            .candidate_provision_event_id_serialized_write_bytes =
            finished.provision_event_id_serialized_write_bytes;
        rules.cost.candidate_provision_event_id_scan_row_operations =
            finished.provision_event_id_scan_row_operations;
        rules
            .cost
            .candidate_provision_event_id_peak_workspace_state_bytes =
            finished.provision_event_id_workspace_state_bytes;
        rules.cost.candidate_provision_used_event_count = finished.provision_used_event_rows;
        rules
            .cost
            .candidate_provision_used_event_serialized_read_bytes =
            finished.provision_used_event_serialized_read_bytes;
        rules
            .cost
            .candidate_provision_used_event_serialized_write_bytes =
            finished.provision_used_event_serialized_write_bytes;
        rules
            .cost
            .candidate_provision_used_event_scan_row_operations =
            finished.provision_used_event_scan_row_operations;
        rules
            .cost
            .candidate_provision_used_event_peak_workspace_state_bytes =
            finished.provision_used_event_workspace_state_bytes;
        rules.cost.candidate_provision_validated_event_count =
            finished.provision_validated_event_rows;
        rules
            .cost
            .candidate_provision_validated_event_serialized_read_bytes =
            finished.provision_validated_event_serialized_read_bytes;
        rules
            .cost
            .candidate_provision_validated_event_serialized_write_bytes =
            finished.provision_validated_event_serialized_write_bytes;
        rules
            .cost
            .candidate_provision_validated_event_scan_row_operations =
            finished.provision_validated_event_scan_row_operations;
        rules
            .cost
            .candidate_provision_validated_event_peak_workspace_state_bytes =
            finished.provision_validated_event_workspace_state_bytes;
        rules.cost.candidate_provision_unused_event_count = finished.provision_unused_event_rows;
        rules
            .cost
            .candidate_provision_unused_event_serialized_read_bytes =
            finished.provision_unused_event_serialized_read_bytes;
        rules
            .cost
            .candidate_provision_unused_event_scan_row_operations =
            finished.provision_unused_event_scan_row_operations;
        rules
            .cost
            .candidate_provision_unused_event_peak_workspace_state_bytes =
            finished.provision_unused_event_workspace_state_bytes;
        rules.cost.candidate_provision_claim_drained_rows = finished.provision_claim_drained_rows;
        rules.cost.candidate_provision_claim_eof_seen = finished.provision_claim_eof_seen;
        rules.cost.candidate_provision_claim_count_verified =
            finished.provision_claim_count_verified;
        rules.cost.candidate_provision_event_id_drained_rows =
            finished.provision_event_id_drained_rows;
        rules.cost.candidate_provision_event_id_lookup_rows =
            finished.provision_event_id_lookup_rows;
        rules.cost.candidate_provision_event_id_eof_seen = finished.provision_event_id_eof_seen;
        rules.cost.candidate_provision_event_id_count_verified =
            finished.provision_event_id_count_verified;
        rules.cost.candidate_provision_unused_event_drained_rows =
            finished.provision_unused_event_drained_rows;
        rules.cost.candidate_provision_unused_event_eof_seen =
            finished.provision_unused_event_eof_seen;
        rules.cost.candidate_provision_unused_event_count_verified =
            finished.provision_unused_event_count_verified;
        rules.cost.candidate_topology_store = finished.topology;
    }

    Ok(SourceFoundationClosureReport {
        cost: rules.cost,
        issues: rules.issues,
        schema_requests: rules.schema_requests,
        unsupported: rules.unsupported,
        requires_bibliographic,
    })
}

fn validate_legacy_cut_paths(
    cut: &CorpusCutReader,
    current_paths: &[String],
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), ItemRefusal> {
    let mut members = cut.current().members();
    for supplied in current_paths {
        check(deadline, cancelled)?;
        let Some(member) = members.next() else {
            return Err(ItemRefusal::Source(
                "source-foundation current paths differ from the exact captured cut".into(),
            ));
        };
        if member.path.as_str() != supplied {
            return Err(ItemRefusal::Source(
                "source-foundation current paths differ from the exact captured cut".into(),
            ));
        }
    }
    check(deadline, cancelled)?;
    if members.next().is_some() {
        return Err(ItemRefusal::Source(
            "source-foundation current paths differ from the exact captured cut".into(),
        ));
    }
    Ok(())
}

fn push_bounded_issue(
    issues: &mut Vec<(String, String)>,
    cost: &mut SourceFoundationClosureCost,
    retained_state_bytes: &mut usize,
    temporary_state_bytes: usize,
    limits: ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
    location: &str,
    message: String,
) -> Result<(), ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if issues.len() >= limits.max_issues {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation closure issue count",
            used: Some(issues.len() as u64 + 1),
            limit: Some(limits.max_issues as u64),
        });
    }
    let location = location.to_owned();
    let amount = location
        .len()
        .checked_add(message.len())
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<String>()))
        .ok_or(ItemRefusal::Budget)?;
    let retained = retained_state_bytes
        .checked_add(amount)
        .filter(|used| {
            used.checked_add(temporary_state_bytes)
                .is_some_and(|total| total <= limits.max_state_bytes)
        })
        .ok_or(ItemRefusal::BudgetCheck {
            check: "source-foundation closure state",
            used: None,
            limit: Some(limits.max_state_bytes as u64),
        })?;
    *retained_state_bytes = retained;
    cost.reserved_state_bytes = cost
        .reserved_state_bytes
        .max(retained.saturating_add(temporary_state_bytes));
    issues.push((location, message));
    cost.emitted_issues = issues.len();
    Ok(())
}

fn exact_backref_messages(
    record: &Value,
    field: &str,
    record_id: &str,
    label: &str,
    claims: &BTreeMap<String, SourceFoundationClosureClaimRef>,
) -> Vec<String> {
    let refs = value_strings(record, field);
    let actual: BTreeSet<String> = refs.iter().cloned().collect();
    let missing: Vec<String> = actual
        .iter()
        .filter(|claim_id| !claims.contains_key(*claim_id))
        .cloned()
        .collect();
    let misbound: Vec<String> = actual
        .iter()
        .filter(|claim_id| {
            claims
                .get(*claim_id)
                .is_some_and(|claim| claim.subject != record_id)
        })
        .cloned()
        .collect();
    let unreferenced: Vec<String> = claims
        .iter()
        .filter(|(_, claim)| claim.subject == record_id)
        .filter(|(claim_id, _)| !actual.contains(*claim_id))
        .map(|(claim_id, _)| claim_id.clone())
        .collect();
    let mut findings = Vec::new();
    if !missing.is_empty() {
        findings.push(format!(
            "unresolved {label} claims: {}",
            python_string_list(&missing)
        ));
    }
    if !misbound.is_empty() {
        findings.push(format!(
            "{label} claims belong to another subject: {}",
            python_string_list(&misbound)
        ));
    }
    if !unreferenced.is_empty() {
        findings.push(format!(
            "subject {label} claims are not referenced: {}",
            python_string_list(&unreferenced)
        ));
    }
    if refs.len() != actual.len() {
        findings.push(format!("{field} contains duplicate claim references"));
    }
    findings
}

fn bounded_string_capacity_state(bytes: usize) -> Result<usize, ItemRefusal> {
    bytes
        .checked_mul(2)
        .and_then(|state| state.checked_add(std::mem::size_of::<String>() + 32))
        .ok_or(ItemRefusal::Budget)
}

fn bounded_string_vec_state(strings: &[String], capacity: usize) -> Result<usize, ItemRefusal> {
    strings.iter().try_fold(
        std::mem::size_of::<Vec<String>>()
            .checked_add(
                capacity
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?,
        |state, value| {
            state
                .checked_add(estimate_string_storage(value)?)
                .ok_or(ItemRefusal::Budget)
        },
    )
}

fn refresh_candidate_findings_state(
    findings: &Vec<String>,
    findings_state_bytes: &mut usize,
    temporary_state_bytes: &mut usize,
) -> Result<(), ItemRefusal> {
    let updated = bounded_string_vec_state(findings, findings.capacity())?;
    let without_findings = temporary_state_bytes
        .checked_sub(*findings_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    *temporary_state_bytes = without_findings
        .checked_add(updated)
        .ok_or(ItemRefusal::Budget)?;
    *findings_state_bytes = updated;
    Ok(())
}

fn python_string_list_workspace(
    count: usize,
    source_bytes: usize,
) -> Result<(usize, usize, usize), ItemRefusal> {
    let repr_bytes = source_bytes
        .checked_mul(6)
        .and_then(|bytes| bytes.checked_add(count.checked_mul(2)?))
        .ok_or(ItemRefusal::Budget)?;
    let repr_state = source_bytes
        .checked_add(count.checked_mul(2).ok_or(ItemRefusal::Budget)?)
        .and_then(|initial_capacity| {
            bounded_string_capacity_state(repr_bytes)
                .ok()?
                .checked_add(initial_capacity)
        })
        .and_then(|state| state.checked_add(count.checked_mul(std::mem::size_of::<String>())?))
        .ok_or(ItemRefusal::Budget)?;
    let repr_vector_state = std::mem::size_of::<Vec<String>>()
        .checked_add(
            count
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    let joined_bytes = repr_bytes
        .checked_add(
            count
                .saturating_sub(1)
                .checked_mul(2)
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    let joined_state = bounded_string_capacity_state(joined_bytes)?;
    let list_bytes = joined_bytes.checked_add(2).ok_or(ItemRefusal::Budget)?;
    let list_state = bounded_string_capacity_state(list_bytes)?;
    let render_vector_peak = repr_vector_state
        .checked_add(repr_state)
        .and_then(|state| state.checked_add(joined_state))
        .ok_or(ItemRefusal::Budget)?;
    let render_wrapped_peak = joined_state
        .checked_add(list_state)
        .ok_or(ItemRefusal::Budget)?;
    Ok((
        render_vector_peak.max(render_wrapped_peak),
        list_bytes,
        list_state,
    ))
}

fn exact_backref_messages_additional_state(
    record: &Value,
    field: &str,
    record_id: &str,
    label: &str,
    claims: &BTreeMap<String, SourceFoundationClosureClaimRef>,
    prior_findings_len: usize,
    prior_findings_capacity: usize,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<usize, ItemRefusal> {
    let refs = record
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut ref_count = 0usize;
    let mut ref_text_bytes = 0usize;
    let mut ref_storage_bytes = 0usize;
    for (index, value) in refs.iter().enumerate() {
        if index % 128 == 0 {
            check(deadline, cancelled)?;
        }
        if let Some(reference) = value.as_str() {
            ref_count = ref_count.checked_add(1).ok_or(ItemRefusal::Budget)?;
            ref_text_bytes = ref_text_bytes
                .checked_add(reference.len())
                .ok_or(ItemRefusal::Budget)?;
            ref_storage_bytes = ref_storage_bytes
                .checked_add(estimate_string_storage(reference)?)
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    let refs_vector_state = std::mem::size_of::<Vec<String>>()
        .checked_add(
            refs.len()
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .and_then(|state| state.checked_add(ref_storage_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let actual_set_state = ref_storage_bytes
        .checked_add(
            ref_count
                .checked_mul(std::mem::size_of::<String>() + 8 * std::mem::size_of::<usize>() + 96)
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    let missing_vector_state = std::mem::size_of::<Vec<String>>()
        .checked_add(
            ref_count
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .and_then(|state| state.checked_add(ref_storage_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let misbound_vector_state = missing_vector_state;

    let mut subject_claim_count = 0usize;
    let mut subject_claim_text_bytes = 0usize;
    let mut subject_claim_storage_bytes = 0usize;
    for (index, (claim_id, claim)) in claims.iter().enumerate() {
        if index % 128 == 0 {
            check(deadline, cancelled)?;
        }
        if claim.subject == record_id {
            subject_claim_count = subject_claim_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            subject_claim_text_bytes = subject_claim_text_bytes
                .checked_add(claim_id.len())
                .ok_or(ItemRefusal::Budget)?;
            subject_claim_storage_bytes = subject_claim_storage_bytes
                .checked_add(estimate_string_storage(claim_id)?)
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    let unreferenced_vector_state = std::mem::size_of::<Vec<String>>()
        .checked_add(
            claims
                .len()
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .and_then(|state| state.checked_add(subject_claim_storage_bytes))
        .ok_or(ItemRefusal::Budget)?;

    let (missing_list_peak, missing_list_bytes, missing_list_state) =
        python_string_list_workspace(ref_count, ref_text_bytes)?;
    let (misbound_list_peak, misbound_list_bytes, misbound_list_state) =
        python_string_list_workspace(ref_count, ref_text_bytes)?;
    let (unreferenced_list_peak, unreferenced_list_bytes, unreferenced_list_state) =
        python_string_list_workspace(subject_claim_count, subject_claim_text_bytes)?;
    let missing_message_bytes = b"unresolved "
        .len()
        .checked_add(label.len())
        .and_then(|bytes| bytes.checked_add(b" claims: ".len()))
        .and_then(|bytes| bytes.checked_add(missing_list_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let misbound_message_bytes = label
        .len()
        .checked_add(b" claims belong to another subject: ".len())
        .and_then(|bytes| bytes.checked_add(misbound_list_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let unreferenced_message_bytes = b"subject "
        .len()
        .checked_add(label.len())
        .and_then(|bytes| bytes.checked_add(b" claims are not referenced: ".len()))
        .and_then(|bytes| bytes.checked_add(unreferenced_list_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let duplicate_message_bytes = field
        .len()
        .checked_add(b" contains duplicate claim references".len())
        .ok_or(ItemRefusal::Budget)?;
    let message_state = [
        missing_message_bytes,
        misbound_message_bytes,
        unreferenced_message_bytes,
        duplicate_message_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |state, bytes| {
        state
            .checked_add(bounded_string_capacity_state(bytes)?)
            .ok_or(ItemRefusal::Budget)
    })?;
    let result_vector_state = std::mem::size_of::<Vec<String>>()
        .checked_add(
            8usize
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    let missing_message_state = bounded_string_capacity_state(missing_message_bytes)?;
    let misbound_message_state = bounded_string_capacity_state(misbound_message_bytes)?;
    let unreferenced_message_state = bounded_string_capacity_state(unreferenced_message_bytes)?;
    let mut state = refs_vector_state
        .checked_add(actual_set_state)
        .and_then(|state| state.checked_add(missing_vector_state))
        .and_then(|state| state.checked_add(misbound_vector_state))
        .and_then(|state| state.checked_add(unreferenced_vector_state))
        .and_then(|state| state.checked_add(message_state))
        .and_then(|state| state.checked_add(result_vector_state))
        .and_then(|state| {
            state.checked_add(
                missing_list_peak
                    .max(misbound_list_peak)
                    .max(unreferenced_list_peak)
                    .max(missing_list_state.checked_add(missing_message_state)?)
                    .max(misbound_list_state.checked_add(misbound_message_state)?)
                    .max(unreferenced_list_state.checked_add(unreferenced_message_state)?),
            )
        })
        .ok_or(ItemRefusal::Budget)?;

    let worst_append_len = prior_findings_len
        .checked_add(4)
        .ok_or(ItemRefusal::Budget)?;
    if worst_append_len > prior_findings_capacity {
        let replacement_capacity = worst_append_len.checked_mul(2).ok_or(ItemRefusal::Budget)?;
        state = state
            .checked_add(
                replacement_capacity
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;
    }
    Ok(state)
}

fn append_candidate_backref_messages(
    findings: &mut Vec<String>,
    findings_state_bytes: &mut usize,
    temporary_state_bytes: &mut usize,
    retained_state_bytes: usize,
    additional_live_state_bytes: usize,
    limits: ItemLimits,
    cost: &mut SourceFoundationClosureCost,
    record: &Value,
    field: &str,
    record_id: &str,
    label: &str,
    claims: &BTreeMap<String, SourceFoundationClosureClaimRef>,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), ItemRefusal> {
    let scratch = exact_backref_messages_additional_state(
        record,
        field,
        record_id,
        label,
        claims,
        findings.len(),
        findings.capacity(),
        deadline,
        cancelled,
    )?;
    let preflight = retained_state_bytes
        .checked_add(*temporary_state_bytes)
        .and_then(|state| state.checked_add(additional_live_state_bytes))
        .and_then(|state| state.checked_add(scratch))
        .ok_or(ItemRefusal::Budget)?;
    if preflight > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation closure backlink message workspace",
            used: Some(preflight as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    cost.reserved_state_bytes = cost.reserved_state_bytes.max(preflight);

    let messages = exact_backref_messages(record, field, record_id, label, claims);
    findings.extend(messages);
    let updated_findings_state = bounded_string_vec_state(findings, findings.capacity())?;
    let temporary_without_findings = temporary_state_bytes
        .checked_sub(*findings_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    *temporary_state_bytes = temporary_without_findings
        .checked_add(updated_findings_state)
        .ok_or(ItemRefusal::Budget)?;
    *findings_state_bytes = updated_findings_state;
    let retained_total = retained_state_bytes
        .checked_add(*temporary_state_bytes)
        .and_then(|state| state.checked_add(additional_live_state_bytes))
        .ok_or(ItemRefusal::Budget)?;
    if retained_total > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation closure backlink findings state",
            used: Some(retained_total as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    cost.reserved_state_bytes = cost.reserved_state_bytes.max(retained_total);
    Ok(())
}

fn claim_reference_index_state(
    id: &str,
    reference: &SourceFoundationClosureClaimRef,
) -> Result<usize, ItemRefusal> {
    id.len()
        .checked_add(claim_reference_payload_state(reference)?)
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<String>() + 8 * std::mem::size_of::<usize>())
        })
        .ok_or(ItemRefusal::Budget)
}

fn claim_reference_map_entry_state(id: &str) -> Result<usize, ItemRefusal> {
    id.len()
        .checked_add(std::mem::size_of::<(String, SourceFoundationClosureClaimRef)>())
        .and_then(|state| state.checked_add(8 * std::mem::size_of::<usize>()))
        .ok_or(ItemRefusal::Budget)
}

fn claim_reference_payload_state(
    reference: &SourceFoundationClosureClaimRef,
) -> Result<usize, ItemRefusal> {
    reference
        .location
        .len()
        .checked_add(reference.subject.len())
        .and_then(|n| n.checked_add(reference.predicate.len()))
        .and_then(|n| n.checked_add(reference.object.len()))
        .and_then(|n| n.checked_add(reference.event.len()))
        .and_then(|n| n.checked_add(std::mem::size_of::<SourceFoundationClosureClaimRef>()))
        .ok_or(ItemRefusal::Budget)
}

fn claim_refs_vec_clone_state(
    claims: &BTreeMap<String, SourceFoundationClosureClaimRef>,
) -> Result<usize, ItemRefusal> {
    claims.iter().try_fold(
        std::mem::size_of::<Vec<SourceFoundationClosureClaimRef>>(),
        |state, (id, claim)| {
            state
                .checked_add(id.len())
                .and_then(|n| n.checked_add(claim_reference_payload_state(claim).ok()?))
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<(String, SourceFoundationClosureClaimRef)>())
                })
                .ok_or(ItemRefusal::Budget)
        },
    )
}

/// Exact source-derived condition used by the maintained Python route for its
/// optional bibliographic graph boundary.
pub fn source_foundation_requires_bibliographic(
    current_paths: &[String],
    declared_profile_kinds: &BTreeSet<String>,
) -> bool {
    !declared_profile_kinds.is_empty()
        || current_paths.iter().any(|path| {
            path.starts_with("ToS/source-witnesses/")
                && (path.ends_with("/historical-claims.jsonl")
                    || path.ends_with("/source-claims.jsonl"))
        })
}

// Store object bounds remain independent from this short inspection borrow.
fn directed_cycle<K: Ord + Clone>(
    edges: &BTreeMap<K, BTreeSet<K>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<bool, ItemRefusal> {
    let mut visited = BTreeMap::<K, u8>::new();
    let mut cycle = false;
    for start in edges.keys() {
        if visited.get(start).copied().unwrap_or_default() != 0 {
            continue;
        }
        let mut stack = vec![(start.clone(), false)];
        while let Some((node, leaving)) = stack.pop() {
            check(deadline, cancelled)?;
            if leaving {
                visited.insert(node, 2);
                continue;
            }
            match visited.get(&node).copied().unwrap_or_default() {
                1 => {
                    cycle = true;
                    continue;
                }
                2 => continue,
                _ => {}
            }
            visited.insert(node.clone(), 1);
            stack.push((node.clone(), true));
            if let Some(children) = edges.get(&node) {
                for child in children.iter().rev() {
                    match visited.get(child).copied().unwrap_or_default() {
                        1 => cycle = true,
                        0 => stack.push((child.clone(), false)),
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(cycle)
}

struct ClosureRules<'a, 'link, 'schema, S: LayerFamilySource + ?Sized> {
    source: &'a mut S,
    limits: ItemLimits,
    paths: &'a dyn SourceFoundationDefaultPaths,
    records: &'a dyn SourceFoundationDefaultRecordsLookup,
    source_events: &'a dyn SourceFoundationDefaultEventLookup,
    claims: &'a dyn SourceFoundationDefaultClaims,
    link_store: Option<&'link mut dyn SourceFoundationClosureLinkStore>,
    schema_request_store: Option<&'schema mut dyn SourceFoundationClosureSchemaRequestStore>,
    link_count: u64,
    links: BTreeMap<String, (String, Value)>,
    issues: Vec<(String, String)>,
    schema_requests: Vec<SourceFoundationClosureSchemaRequest>,
    unsupported: Vec<SourceFoundationClosureGap>,
    cost: SourceFoundationClosureCost,
    retained_state_bytes: usize,
    temporary_state_bytes: usize,
    cache_digests: bool,
    cache_recorded_checks: bool,
    scope: SourceFoundationDefaultRuleScope,
    loaded: BTreeMap<String, LoadedRows>,
    digests: BTreeMap<String, String>,
    recorded_checks: BTreeMap<(String, String), bool>,
    event_ids: BTreeSet<String>,
    events: BTreeMap<String, Value>,
    claim_ids: BTreeSet<String>,
    responsibility_claim_count: u64,
    anchors: BTreeSet<String>,
    boundary_membership_refs: BTreeSet<String>,
    boundary_responsibility_refs: BTreeSet<String>,
    membership: BTreeMap<String, SourceFoundationClosureClaimRef>,
    responsibility: BTreeMap<String, SourceFoundationClosureClaimRef>,
    publication: BTreeMap<String, SourceFoundationClosureClaimRef>,
    provision: BTreeMap<String, SourceFoundationClosureClaimRef>,
    provision_values: BTreeMap<String, Value>,
    provision_event_ids: BTreeSet<String>,
    chronology: BTreeMap<String, SourceFoundationClosureClaimRef>,
    object_links: BTreeMap<String, SourceFoundationClosureClaimRef>,
    topology: BTreeMap<String, SourceFoundationClosureClaimRef>,
    derivation: BTreeMap<String, SourceFoundationClosureClaimRef>,
}

impl<'a, 'link, 'schema, S: LayerFamilySource + ?Sized> ClosureRules<'a, 'link, 'schema, S> {
    fn new(
        source: &'a mut S,
        source_events: &'a dyn SourceFoundationDefaultEventLookup,
        records: &'a dyn SourceFoundationDefaultRecordsLookup,
        paths: &'a dyn SourceFoundationDefaultPaths,
        claims: &'a dyn SourceFoundationDefaultClaims,
        link_store: Option<&'link mut dyn SourceFoundationClosureLinkStore>,
        schema_request_store: Option<&'schema mut dyn SourceFoundationClosureSchemaRequestStore>,
        limits: ItemLimits,
        cache_digests: bool,
        cache_recorded_checks: bool,
        scope: SourceFoundationDefaultRuleScope,
    ) -> Result<Self, ItemRefusal> {
        check(limits.deadline, source.cancellation())?;
        Ok(Self {
            source,
            limits,
            paths,
            records,
            source_events,
            claims,
            link_store,
            schema_request_store,
            link_count: 0,
            links: BTreeMap::new(),
            issues: Vec::new(),
            schema_requests: Vec::new(),
            unsupported: Vec::new(),
            cost: SourceFoundationClosureCost::default(),
            retained_state_bytes: 0,
            temporary_state_bytes: 0,
            cache_digests,
            cache_recorded_checks,
            scope,
            loaded: BTreeMap::new(),
            digests: BTreeMap::new(),
            recorded_checks: BTreeMap::new(),
            event_ids: BTreeSet::new(),
            events: BTreeMap::new(),
            claim_ids: BTreeSet::new(),
            responsibility_claim_count: 0,
            anchors: BTreeSet::new(),
            boundary_membership_refs: BTreeSet::new(),
            boundary_responsibility_refs: BTreeSet::new(),
            membership: BTreeMap::new(),
            responsibility: BTreeMap::new(),
            publication: BTreeMap::new(),
            provision: BTreeMap::new(),
            provision_values: BTreeMap::new(),
            provision_event_ids: BTreeSet::new(),
            chronology: BTreeMap::new(),
            object_links: BTreeMap::new(),
            topology: BTreeMap::new(),
            derivation: BTreeMap::new(),
        })
    }

    fn reserve(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        self.retained_state_bytes = self
            .retained_state_bytes
            .checked_add(amount)
            .filter(|used| {
                used.checked_add(self.temporary_state_bytes)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation closure state",
                used: None,
                limit: Some(self.limits.max_state_bytes as u64),
            })?;
        self.cost.reserved_state_bytes = self
            .cost
            .reserved_state_bytes
            .max(self.retained_state_bytes + self.temporary_state_bytes);
        Ok(())
    }

    fn reserve_temporary(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        self.temporary_state_bytes = self
            .temporary_state_bytes
            .checked_add(amount)
            .filter(|used| {
                self.retained_state_bytes
                    .checked_add(*used)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation closure temporary state",
                used: None,
                limit: Some(self.limits.max_state_bytes as u64),
            })?;
        self.cost.reserved_state_bytes = self
            .cost
            .reserved_state_bytes
            .max(self.retained_state_bytes + self.temporary_state_bytes);
        Ok(())
    }

    fn release_temporary_since(&mut self, baseline: usize) {
        self.temporary_state_bytes = baseline;
    }

    fn reserve_loaded_state(
        &mut self,
        amount: usize,
        loaded_state_bytes: &mut usize,
    ) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            self.reserve_temporary(amount)?;
            *loaded_state_bytes = loaded_state_bytes
                .checked_add(amount)
                .ok_or(ItemRefusal::Budget)?;
        } else {
            self.reserve(amount)?;
        }
        Ok(())
    }

    fn adjust_loaded_state(
        &mut self,
        loaded_state_bytes: &mut usize,
        desired: usize,
    ) -> Result<(), ItemRefusal> {
        if desired > *loaded_state_bytes {
            self.reserve_loaded_state(desired - *loaded_state_bytes, loaded_state_bytes)?;
        } else if desired < *loaded_state_bytes {
            let released = *loaded_state_bytes - desired;
            self.temporary_state_bytes = self
                .temporary_state_bytes
                .checked_sub(released)
                .ok_or(ItemRefusal::Budget)?;
            *loaded_state_bytes = desired;
        }
        Ok(())
    }

    fn include_store_workspace(&mut self, workspace: usize) -> Result<(), ItemRefusal> {
        let total = self
            .retained_state_bytes
            .checked_add(self.temporary_state_bytes)
            .and_then(|state| state.checked_add(workspace))
            .ok_or(ItemRefusal::Budget)?;
        if total > self.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation closure state and loaded-document store workspace",
                used: Some(total as u64),
                limit: Some(self.limits.max_state_bytes as u64),
            });
        }
        self.cost.reserved_state_bytes = self.cost.reserved_state_bytes.max(total);
        Ok(())
    }

    fn candidate_loaded_digest(&mut self, path: &str) -> Result<Option<String>, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (digest, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .loaded_document_digest(path, remaining)?;
        self.include_store_workspace(workspace)?;
        Ok(digest)
    }

    fn remember_candidate_loaded_row(
        &mut self,
        path: &str,
        line: usize,
        digest: &str,
        raw_line: &[u8],
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_loaded_row(path, line, digest, raw_line, remaining)?;
        self.include_store_workspace(workspace)?;
        if !inserted {
            return Err(ItemRefusal::Source(
                "source-foundation loaded physical row was duplicated".into(),
            ));
        }
        self.cost.candidate_loaded_row_store.inserted_rows = self
            .cost
            .candidate_loaded_row_store
            .inserted_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn finish_candidate_loaded_document_rows(
        &mut self,
        path: &str,
        digest: &str,
        expected_rows: u64,
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let workspace = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .finish_loaded_document_rows(path, digest, expected_rows, remaining)?;
        self.include_store_workspace(workspace)
    }

    fn candidate_loaded_document_row_count(
        &mut self,
        path: &str,
        digest: &str,
    ) -> Result<u64, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (count, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .loaded_document_row_count(path, digest, remaining)?;
        self.include_store_workspace(workspace)?;
        count.ok_or_else(|| {
            ItemRefusal::Source(
                "source-foundation loaded document has no sealed physical-row index".into(),
            )
        })
    }

    fn candidate_loaded_row(
        &mut self,
        path: &str,
        line: usize,
        digest: &str,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (raw_line, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .loaded_row(path, line, digest, remaining)?;
        self.include_store_workspace(workspace)?;
        self.cost.candidate_loaded_row_store.point_lookup_operations = self
            .cost
            .candidate_loaded_row_store
            .point_lookup_operations
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        if raw_line.is_some() {
            self.cost.candidate_loaded_row_store.point_read_rows = self
                .cost
                .candidate_loaded_row_store
                .point_read_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(raw_line)
    }

    fn next_candidate_loaded_row(
        &mut self,
        path: &str,
        digest: &str,
        after_line: Option<usize>,
    ) -> Result<Option<(usize, Vec<u8>)>, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (row, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .next_loaded_row(path, digest, after_line, remaining)?;
        self.include_store_workspace(workspace)?;
        if row.is_some() {
            self.cost.candidate_loaded_row_store.streamed_rows = self
                .cost
                .candidate_loaded_row_store
                .streamed_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        } else {
            self.cost.candidate_loaded_row_store.stream_eof_count = self
                .cost
                .candidate_loaded_row_store
                .stream_eof_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(row)
    }

    fn observe_candidate_loaded_digest(
        &mut self,
        path: &str,
        digest: &str,
    ) -> Result<bool, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (first, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .observe_loaded_document(path, digest, remaining)?;
        self.include_store_workspace(workspace)?;
        if first {
            self.cost.candidate_loaded_document_count = self
                .cost
                .candidate_loaded_document_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(first)
    }

    fn release_loaded_rows(&mut self, loaded_state_bytes: usize) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            self.temporary_state_bytes = self
                .temporary_state_bytes
                .checked_sub(loaded_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn path_exists(&self, path: &str) -> Result<bool, ItemRefusal> {
        if !self.source.selects_required_member(path)? {
            return Ok(false);
        }
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let mut checkpoint = || check(deadline, cancelled);
        self.paths.contains_with_checkpoint(path, &mut checkpoint)
    }

    fn current_record_with_state_budget(
        &mut self,
        id: &str,
    ) -> Result<(Option<BiblioCurrentRecord>, usize), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (record, workspace) = self
            .records
            .current_record_with_state_budget(id, remaining)?;
        if workspace > remaining {
            return Err(ItemRefusal::Budget);
        }
        self.reserve_temporary(workspace)?;
        Ok((record, workspace))
    }

    fn current_record_exists_with_state_budget(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        let (record, workspace) = self.current_record_with_state_budget(id)?;
        let exists = record.is_some();
        drop(record);
        self.release_temporary_state(workspace)?;
        Ok(exists)
    }

    fn current_record_path_with_state_budget(
        &mut self,
        id: &str,
    ) -> Result<(Option<String>, usize), ItemRefusal> {
        let (record, record_workspace) = self.current_record_with_state_budget(id)?;
        let Some(record) = record else {
            self.release_temporary_state(record_workspace)?;
            return Ok((None, 0));
        };
        let path_workspace = estimate_string_storage(&record.path)?
            .checked_add(std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_temporary(path_workspace)?;
        let path = record.path.clone();
        drop(record);
        self.release_temporary_state(record_workspace)?;
        Ok((Some(path), path_workspace))
    }

    fn owner_record_matches_subject_at_location(
        &mut self,
        location: &str,
        subject: &str,
    ) -> Result<bool, ItemRefusal> {
        let owner_path = location
            .rsplit_once(':')
            .map(|(path, _)| path)
            .unwrap_or(location)
            .rsplit_once('/')
            .map(|(parent, _)| parent);
        let Some(parent) = owner_path else {
            return Ok(false);
        };

        const EDITION_SUFFIX: &str = "/edition.json";
        let owner_path_len = parent
            .len()
            .checked_add(EDITION_SUFFIX.len())
            .ok_or(ItemRefusal::Budget)?;
        let owner_path_state = owner_path_len
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>() + 32))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_temporary(owner_path_state)?;
        let mut path = String::with_capacity(owner_path_len);
        path.push_str(parent);
        path.push_str(EDITION_SUFFIX);

        let remaining = self.remaining_state()?;
        let (record, workspace) = self
            .records
            .record_by_path_with_state_budget(&path, remaining)?;
        if workspace > remaining {
            return Err(ItemRefusal::Budget);
        }
        self.reserve_temporary(workspace)?;
        let matches = record
            .as_deref()
            .and_then(|candidate| text(&candidate.value, "record_id"))
            == Some(subject);
        drop(record);
        self.release_temporary_state(workspace)?;
        drop(path);
        self.release_temporary_state(owner_path_state)?;
        Ok(matches)
    }

    fn release_temporary_state(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        self.temporary_state_bytes = self
            .temporary_state_bytes
            .checked_sub(amount)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn remaining_state(&self) -> Result<usize, ItemRefusal> {
        self.limits
            .max_state_bytes
            .checked_sub(
                self.retained_state_bytes
                    .checked_add(self.temporary_state_bytes)
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)
    }

    fn include_link_workspace(&mut self, workspace: usize) -> Result<(), ItemRefusal> {
        let total = self
            .retained_state_bytes
            .checked_add(self.temporary_state_bytes)
            .and_then(|state| state.checked_add(workspace))
            .ok_or(ItemRefusal::Budget)?;
        if total > self.limits.max_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.cost.reserved_state_bytes = self.cost.reserved_state_bytes.max(total);
        Ok(())
    }

    fn link_exists(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let remaining = self.remaining_state()?;
        let Some(store) = self.link_store.as_deref_mut() else {
            return Ok(self.links.contains_key(id));
        };
        let (found, workspace) = store.contains_link(id, remaining)?;
        self.include_link_workspace(workspace)?;
        Ok(found)
    }

    fn event(&mut self, id: &str) -> Result<(Option<Cow<'_, Value>>, usize), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (candidate_event, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .event_value(id, remaining)?;
            self.include_store_workspace(workspace)?;
            if let Some(event) = candidate_event {
                let value_state = crate::record_biblio_cut::decoded_state(&event.value)?;
                let event_state = value_state
                    .checked_add(
                        std::mem::size_of::<SourceFoundationClosureEvent>()
                            .checked_sub(std::mem::size_of::<Value>())
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .and_then(|state| state.checked_add(event.id.len()))
                    .and_then(|state| state.checked_add(event.path.len()))
                    .and_then(|state| state.checked_add(event.document_sha256.len()))
                    .ok_or(ItemRefusal::Budget)?;
                // The point-read workspace ends when the store returns, but
                // this owned DTO remains live during the path and loaded-digest
                // lookups below. Keep its decoded value and strings in the
                // caller's active state so those nested reads reserve against
                // the real overlap.
                self.reserve_temporary(event_state)?;
                let SourceFoundationClosureEvent {
                    id: event_id,
                    path: event_path,
                    line,
                    document_sha256,
                    value,
                } = event;
                if event_id != id
                    || event_path.is_empty()
                    || line == 0
                    || document_sha256.len() != 64
                    || text(&value, "event_id") != Some(id)
                    || !self.path_exists(&event_path)?
                {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate event projection differs from its exact row"
                            .into(),
                    ));
                }
                let Some(document_digest) = self.candidate_loaded_digest(&event_path)? else {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate event document is outside the loaded cut"
                            .into(),
                    ));
                };
                if document_digest != document_sha256 {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate event document digest differs".into(),
                    ));
                }
                drop((event_id, event_path, document_sha256));
                self.release_loaded_rows(
                    event_state
                        .checked_sub(value_state)
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                return Ok((Some(Cow::Owned(value)), value_state));
            }
        } else if let Some(event) = self.events.get(id) {
            return Ok((Some(Cow::Borrowed(event)), 0));
        }
        Ok((self.source_events.event(id)?, 0))
    }

    fn event_exists(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let candidate_found = if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (found, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .contains_event(id, remaining)?;
            self.include_store_workspace(workspace)?;
            found
        } else {
            self.event_ids.contains(id)
        };
        Ok(candidate_found || self.source_events.event_contains(id)?)
    }

    fn collect_current_paths(
        &mut self,
        matches: impl Fn(&str) -> bool,
        check_name: &'static str,
    ) -> Result<BTreeSet<String>, ItemRefusal> {
        let path_source = self.paths;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let base = self
            .retained_state_bytes
            .checked_add(self.temporary_state_bytes)
            .and_then(|used| used.checked_add(std::mem::size_of::<BTreeSet<String>>()))
            .ok_or(ItemRefusal::Budget)?;
        let mut used = 0usize;
        let mut found = BTreeSet::new();
        let source = &*self.source;
        path_source.for_each_path(&mut |path| {
            check(deadline, cancelled)?;
            if !matches(path) || !source.selects_semantic_member(path)? {
                return Ok(());
            }
            let row_bytes = path
                .len()
                .checked_add(std::mem::size_of::<String>())
                .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
                .ok_or(ItemRefusal::Budget)?;
            used = used.checked_add(row_bytes).ok_or(ItemRefusal::Budget)?;
            let total = base.checked_add(used).ok_or(ItemRefusal::Budget)?;
            if total > self.limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: check_name,
                    used: Some(total as u64),
                    limit: Some(self.limits.max_state_bytes as u64),
                });
            }
            found.insert(path.to_owned());
            Ok(())
        })?;
        self.reserve(
            used.checked_add(std::mem::size_of::<BTreeSet<String>>())
                .ok_or(ItemRefusal::Budget)?,
        )?;
        Ok(found)
    }

    fn issue(
        &mut self,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), ItemRefusal> {
        let location = location.into();
        push_bounded_issue(
            &mut self.issues,
            &mut self.cost,
            &mut self.retained_state_bytes,
            self.temporary_state_bytes,
            self.limits,
            self.source.cancellation(),
            &location,
            message.into(),
        )
    }

    fn python_equal(&self, left: &Value, right: &Value) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let equal = crate::assessment::py_equal(left, right).map_err(assessment_refusal)?;
        check(self.limits.deadline, self.source.cancellation())?;
        Ok(equal)
    }

    fn gap(
        &mut self,
        location: impl Into<String>,
        profile: impl Into<String>,
    ) -> Result<(), ItemRefusal> {
        let location = location.into();
        let profile = profile.into();
        self.reserve(
            location.len() + profile.len() + std::mem::size_of::<SourceFoundationClosureGap>(),
        )?;
        self.unsupported
            .push(SourceFoundationClosureGap { location, profile });
        Ok(())
    }

    fn current_raw(&mut self, path: &str) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if !self.path_exists(path)? {
            return Ok(None);
        }
        let max_bytes = self.current_member_read_limit()?;
        self.cost.current_read_operations = self
            .cost
            .current_read_operations
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        let raw = self.source.current(path, max_bytes, self.limits.deadline)?;
        if let Some(bytes) = &raw {
            if bytes.len() > self.limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            self.cost.current_bytes_read = self
                .cost
                .current_bytes_read
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= self.limits.max_total_bytes)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure current metadata bytes",
                    used: None,
                    limit: Some(self.limits.max_total_bytes),
                })?;
            self.cost.files_read = self
                .cost
                .files_read
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            if self.cache_digests {
                let digest = Digest256::of_bytes(bytes).to_hex();
                self.reserve(
                    path.len()
                        .checked_add(digest.len())
                        .and_then(|n| {
                            n.checked_add(
                                std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>(),
                            )
                        })
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                self.digests.insert(path.to_owned(), digest);
            }
        }
        Ok(raw)
    }

    fn current_member_read_limit(&self) -> Result<usize, ItemRefusal> {
        let remaining_total = self
            .limits
            .max_total_bytes
            .checked_sub(self.cost.current_bytes_read)
            .ok_or(ItemRefusal::Budget)?;
        Ok(self
            .limits
            .max_member_bytes
            .min(usize::try_from(remaining_total).unwrap_or(usize::MAX)))
    }

    fn candidate_loaded_rows_for_document(
        &mut self,
        path: &str,
        digest: String,
    ) -> Result<LoadedRows, ItemRefusal> {
        let loaded_header_state = estimate_string_storage(&digest)?
            .checked_add(std::mem::size_of::<LoadedRows>())
            .ok_or(ItemRefusal::Budget)?;
        let mut loaded_state_bytes = loaded_header_state;
        self.reserve_temporary(loaded_state_bytes)?;
        let expected_rows = self.candidate_loaded_document_row_count(path, &digest)?;
        let row_capacity = usize::try_from(expected_rows).map_err(|_| ItemRefusal::Budget)?;
        let row_slots_state = row_capacity
            .checked_mul(std::mem::size_of::<(usize, Value)>())
            .ok_or(ItemRefusal::Budget)?;
        let requested_loaded_state = loaded_header_state
            .checked_add(row_slots_state)
            .ok_or(ItemRefusal::Budget)?;
        self.adjust_loaded_state(&mut loaded_state_bytes, requested_loaded_state)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(row_capacity)
            .map_err(|_| ItemRefusal::Budget)?;
        let actual_row_slots_state = rows
            .capacity()
            .checked_mul(std::mem::size_of::<(usize, Value)>())
            .ok_or(ItemRefusal::Budget)?;
        let actual_loaded_header_state = loaded_header_state
            .checked_add(actual_row_slots_state)
            .ok_or(ItemRefusal::Budget)?;
        self.adjust_loaded_state(&mut loaded_state_bytes, actual_loaded_header_state)?;
        let mut after_line = None;
        let mut drained_rows = 0u64;
        loop {
            check(self.limits.deadline, self.source.cancellation())?;
            let Some((line, raw_line)) =
                self.next_candidate_loaded_row(path, &digest, after_line)?
            else {
                break;
            };
            if line == 0 || after_line.is_some_and(|previous| line <= previous) {
                return Err(ItemRefusal::Source(
                    "source-foundation loaded physical rows are not strictly ordered".into(),
                ));
            }
            if rows.len() >= row_capacity {
                return Err(ItemRefusal::Source(
                    "source-foundation loaded physical-row count exceeds its sealed document"
                        .into(),
                ));
            }
            let decoded_upper = source_foundation_closure_json_state_upper_bound(raw_line.len())?;
            let raw_line_state = raw_line
                .len()
                .checked_add(decoded_upper)
                .ok_or(ItemRefusal::Budget)?;
            self.reserve_temporary(raw_line_state)?;
            let value = serde_json::from_slice::<Value>(&raw_line).map_err(|_| {
                ItemRefusal::Source("source-foundation loaded physical row no longer parses".into())
            })?;
            let value_state = crate::record_biblio_cut::decoded_state(&value)?;
            if value_state > decoded_upper {
                return Err(ItemRefusal::Budget);
            }
            rows.push((line, value));
            drop(raw_line);
            let next_loaded_state_bytes = loaded_state_bytes
                .checked_add(value_state)
                .ok_or(ItemRefusal::Budget)?;
            let released_parse_state = raw_line_state
                .checked_sub(value_state)
                .ok_or(ItemRefusal::Budget)?;
            self.release_temporary_state(released_parse_state)?;
            loaded_state_bytes = next_loaded_state_bytes;
            after_line = Some(line);
            drained_rows = drained_rows.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
        if drained_rows != expected_rows {
            return Err(ItemRefusal::Source(
                "source-foundation loaded physical-row count differs from its sealed document"
                    .into(),
            ));
        }
        self.cost.decoded_rows = self
            .cost
            .decoded_rows
            .checked_add(drained_rows)
            .ok_or(ItemRefusal::Budget)?;
        let mut loaded = LoadedRows {
            digest,
            rows,
            temporary_state_bytes: loaded_state_bytes,
        };
        self.adjust_loaded_state(&mut loaded_state_bytes, loaded_clone_cost(&loaded)?)?;
        loaded.temporary_state_bytes = loaded_state_bytes;
        Ok(loaded)
    }

    fn append_selected_jsonl_value(
        &mut self,
        path: &str,
        line: u64,
        bytes: &[u8],
        schema: Option<&str>,
        loaded_state_bytes: &mut usize,
        cache_digest: Option<&str>,
        json_limits: tos_foundation::JsonLimits,
        rows: &mut Vec<(usize, Value)>,
    ) -> Result<(), ItemRefusal> {
        let line = usize::try_from(line).map_err(|_| ItemRefusal::Budget)?;
        let header_workspace = rows
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(std::mem::size_of::<(usize, Value)>()))
            .ok_or(ItemRefusal::Budget)?;
        self.include_store_workspace(header_workspace)?;
        let available = self
            .remaining_state()?
            .checked_sub(header_workspace)
            .ok_or(ItemRefusal::Budget)?;
        let (value, value_state) = crate::record_biblio_cut::bounded_decoded_state(
            bytes,
            json_limits,
            available,
            self.limits.deadline,
            self.source.cancellation(),
        )?;
        self.reserve_loaded_state(
            value_state
                .checked_add(std::mem::size_of::<(usize, Value)>())
                .ok_or(ItemRefusal::Budget)?,
            loaded_state_bytes,
        )?;
        self.include_store_workspace(header_workspace)?;
        rows.try_reserve_exact(1).map_err(|_| ItemRefusal::Budget)?;
        if let Some(schema) = schema {
            self.request_schema(&format!("{path}:{line}"), schema, &value)?;
        }
        if let Some(digest) = cache_digest {
            self.remember_candidate_loaded_row(path, line, digest, bytes)?;
        }
        rows.push((line, value));
        Ok(())
    }

    fn selected_jsonl_values(
        &mut self,
        path: &str,
        raw: &[u8],
        schema: Option<&str>,
        loaded_state_bytes: &mut usize,
        cache_digest: Option<&str>,
    ) -> Result<Vec<(usize, Value)>, ItemRefusal> {
        let selection = self.source.record_selection().ok_or(ItemRefusal::Budget)?;
        let json_limits = selection.row_json_limits()?;
        let mut rows = Vec::new();
        if selection.contains_member(path) {
            let scratch = selection
                .file_slots(path)
                .iter()
                .try_fold(0usize, |peak, slot| {
                    Ok::<_, ItemRefusal>(peak.max(slot.verification_state_upper_bound()?))
                })?;
            self.include_store_workspace(scratch)?;
            let verified = selection.verify_file(
                path,
                raw,
                self.limits.deadline,
                self.source.cancellation(),
            )?;
            let mut selected_rows = verified.row_cursor();
            loop {
                self.include_store_workspace(scratch)?;
                let Some(selected) =
                    selected_rows.next_checked(self.limits.deadline, self.source.cancellation())
                else {
                    break;
                };
                let (line, bytes, _) = selected?;
                self.append_selected_jsonl_value(
                    path,
                    line,
                    bytes,
                    schema,
                    loaded_state_bytes,
                    cache_digest,
                    json_limits,
                    &mut rows,
                )?;
                self.include_store_workspace(scratch)?;
            }
        } else {
            let generated = self
                .source
                .generated_selection()
                .ok_or(ItemRefusal::Budget)?;
            if !generated.selects_member(path)? {
                return Err(ItemRefusal::Source(
                    "Closure stream is outside the declared generated selection".into(),
                ));
            }
            let caller_state = self
                .retained_state_bytes
                .checked_add(self.temporary_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            generated.verify_member(
                path,
                raw,
                caller_state,
                self.limits.deadline,
                self.source.cancellation(),
            )?;
            for (line, bytes) in crate::source_record_selection::source_rows(raw) {
                check(self.limits.deadline, self.source.cancellation())?;
                if generated.selects_row(path, line)? {
                    self.append_selected_jsonl_value(
                        path,
                        line,
                        bytes,
                        schema,
                        loaded_state_bytes,
                        cache_digest,
                        json_limits,
                        &mut rows,
                    )?;
                }
            }
            if rows.is_empty() {
                return Err(ItemRefusal::Source(
                    "Generated Closure stream has no required selected row".into(),
                ));
            }
        }
        Ok(rows)
    }

    fn json_rows(
        &mut self,
        path: &str,
        schema: &str,
        required: bool,
    ) -> Result<Option<LoadedRows>, ItemRefusal> {
        if self.schema_request_store.is_none() && self.loaded.contains_key(path) {
            let clone_cost = self
                .loaded
                .get(path)
                .map(loaded_clone_cost)
                .transpose()?
                .unwrap_or_default();
            self.reserve(clone_cost)?;
            return Ok(self.loaded.get(path).cloned());
        }
        if !self.path_exists(path)? {
            if self.schema_request_store.is_some() && self.candidate_loaded_digest(path)?.is_some()
            {
                return Err(ItemRefusal::Source(
                    "source-foundation loaded document left the exact current cut".into(),
                ));
            }
            if required {
                self.issue(path, "required source member is missing")?;
            }
            return Ok(None);
        }
        let candidate_cached = self.schema_request_store.is_some();
        if candidate_cached {
            if let Some(digest) = self.candidate_loaded_digest(path)? {
                return self
                    .candidate_loaded_rows_for_document(path, digest)
                    .map(Some);
            }
        }
        let temporary_baseline = self.temporary_state_bytes;
        let mut loaded_state_bytes = 0usize;
        if candidate_cached {
            let read_limit = self.current_member_read_limit()?;
            self.reserve_temporary(read_limit)?;
            loaded_state_bytes = read_limit;
        }
        let raw_result = self.current_raw(path);
        let Some(raw) = (match raw_result {
            Ok(raw) => raw,
            Err(error) => {
                self.release_temporary_since(temporary_baseline);
                return Err(error);
            }
        }) else {
            self.release_temporary_since(temporary_baseline);
            if candidate_cached && self.candidate_loaded_digest(path)?.is_some() {
                return Err(ItemRefusal::Source(
                    "source-foundation loaded document body left the exact current cut".into(),
                ));
            }
            if required {
                self.issue(path, "required source member is missing")?;
            }
            return Ok(None);
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        let first_load = if candidate_cached {
            let raw_workspace = raw.len().checked_mul(6).ok_or(ItemRefusal::Budget)?;
            self.adjust_loaded_state(&mut loaded_state_bytes, raw_workspace)?;
            self.observe_candidate_loaded_digest(path, &digest)?
        } else {
            true
        };
        let mut rows = Vec::new();
        let jsonl = path.ends_with(".jsonl");
        let state_cost = raw.len().checked_mul(6).ok_or(ItemRefusal::Budget)?;
        if !candidate_cached {
            self.reserve_loaded_state(state_cost, &mut loaded_state_bytes)?;
        }
        if jsonl && self.source.record_selection().is_some() {
            rows = self.selected_jsonl_values(
                path,
                &raw,
                first_load.then_some(schema),
                &mut loaded_state_bytes,
                (candidate_cached && first_load).then_some(digest.as_str()),
            )?;
        } else if jsonl {
            let segments: Vec<&[u8]> = raw.split(|byte| *byte == b'\n').collect();
            for (zero_index, bytes) in segments.iter().enumerate() {
                check(self.limits.deadline, self.source.cancellation())?;
                if bytes.iter().all(u8::is_ascii_whitespace) {
                    if zero_index + 1 == segments.len() && raw.ends_with(b"\n") {
                        continue;
                    }
                    if first_load {
                        self.issue(
                            format!("{path}:{}", zero_index + 1),
                            "blank JSONL line is not allowed",
                        )?;
                    }
                    continue;
                }
                let line = zero_index + 1;
                match serde_json::from_slice::<Value>(bytes) {
                    Ok(value) => {
                        if first_load {
                            self.request_schema(&format!("{path}:{line}"), schema, &value)?;
                        }
                        self.reserve_loaded_state(
                            std::mem::size_of::<Value>(),
                            &mut loaded_state_bytes,
                        )?;
                        if candidate_cached && first_load {
                            self.remember_candidate_loaded_row(path, line, &digest, bytes)?;
                        }
                        rows.push((line, value));
                    }
                    Err(error) if first_load => self.issue(
                        format!("{path}:{line}"),
                        format!("invalid JSON: {}", json_parse_reason(&error)),
                    )?,
                    Err(_) => {}
                }
            }
        } else {
            match serde_json::from_slice::<Value>(&raw) {
                Ok(value) => {
                    if first_load {
                        self.request_schema(path, schema, &value)?;
                    }
                    if candidate_cached && first_load {
                        self.remember_candidate_loaded_row(path, 1, &digest, &raw)?;
                    }
                    rows.push((1, value));
                }
                Err(error) if first_load => {
                    self.issue(path, format!("invalid JSON: {}", json_parse_reason(&error)))?
                }
                Err(_) => {}
            }
        }
        self.cost.decoded_rows = self
            .cost
            .decoded_rows
            .checked_add(rows.len() as u64)
            .ok_or(ItemRefusal::Budget)?;
        if candidate_cached && first_load {
            self.finish_candidate_loaded_document_rows(path, &digest, rows.len() as u64)?;
        }
        drop(raw);
        let mut loaded = LoadedRows {
            digest,
            rows,
            temporary_state_bytes: loaded_state_bytes,
        };
        if candidate_cached {
            self.adjust_loaded_state(&mut loaded_state_bytes, loaded_clone_cost(&loaded)?)?;
            loaded.temporary_state_bytes = loaded_state_bytes;
        }
        if !candidate_cached {
            self.loaded.insert(path.to_owned(), loaded.clone());
        }
        Ok(Some(loaded))
    }

    fn unchecked_jsonl_rows(&mut self, path: &str) -> Result<Option<LoadedRows>, ItemRefusal> {
        if self.schema_request_store.is_none() && self.loaded.contains_key(path) {
            let clone_cost = self
                .loaded
                .get(path)
                .map(loaded_clone_cost)
                .transpose()?
                .unwrap_or_default();
            self.reserve(clone_cost)?;
            return Ok(self.loaded.get(path).cloned());
        }
        if !path.ends_with(".jsonl") {
            return Ok(None);
        }
        if !self.path_exists(path)? {
            if self.schema_request_store.is_some() && self.candidate_loaded_digest(path)?.is_some()
            {
                return Err(ItemRefusal::Source(
                    "source-foundation loaded document left the exact current cut".into(),
                ));
            }
            return Ok(None);
        }
        let candidate_cached = self.schema_request_store.is_some();
        if candidate_cached {
            if let Some(digest) = self.candidate_loaded_digest(path)? {
                return self
                    .candidate_loaded_rows_for_document(path, digest)
                    .map(Some);
            }
        }
        let temporary_baseline = self.temporary_state_bytes;
        let mut loaded_state_bytes = 0usize;
        if candidate_cached {
            let read_limit = self.current_member_read_limit()?;
            self.reserve_temporary(read_limit)?;
            loaded_state_bytes = read_limit;
        }
        let raw_result = self.current_raw(path);
        let Some(raw) = (match raw_result {
            Ok(raw) => raw,
            Err(error) => {
                self.release_temporary_since(temporary_baseline);
                return Err(error);
            }
        }) else {
            self.release_temporary_since(temporary_baseline);
            if candidate_cached && self.candidate_loaded_digest(path)?.is_some() {
                return Err(ItemRefusal::Source(
                    "source-foundation loaded document body left the exact current cut".into(),
                ));
            }
            return Ok(None);
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        let first_load = if candidate_cached {
            let raw_workspace = raw.len().checked_mul(6).ok_or(ItemRefusal::Budget)?;
            self.adjust_loaded_state(&mut loaded_state_bytes, raw_workspace)?;
            self.observe_candidate_loaded_digest(path, &digest)?
        } else {
            true
        };
        if !candidate_cached {
            self.reserve_loaded_state(
                raw.len().checked_mul(6).ok_or(ItemRefusal::Budget)?,
                &mut loaded_state_bytes,
            )?;
        }
        let mut rows = Vec::new();
        if self.source.record_selection().is_some() {
            rows = self.selected_jsonl_values(
                path,
                &raw,
                None,
                &mut loaded_state_bytes,
                (candidate_cached && first_load).then_some(digest.as_str()),
            )?;
        } else {
            let segments: Vec<&[u8]> = raw.split(|byte| *byte == b'\n').collect();
            for (zero_index, bytes) in segments.iter().enumerate() {
                check(self.limits.deadline, self.source.cancellation())?;
                if bytes.iter().all(u8::is_ascii_whitespace) {
                    if zero_index + 1 == segments.len() && raw.ends_with(b"\n") {
                        continue;
                    }
                    if first_load {
                        self.issue(
                            format!("{path}:{}", zero_index + 1),
                            "blank JSONL line is not allowed",
                        )?;
                    }
                    continue;
                }
                let line = zero_index + 1;
                match serde_json::from_slice::<Value>(bytes) {
                    Ok(value) => {
                        self.reserve_loaded_state(
                            std::mem::size_of::<Value>(),
                            &mut loaded_state_bytes,
                        )?;
                        if candidate_cached && first_load {
                            self.remember_candidate_loaded_row(path, line, &digest, bytes)?;
                        }
                        rows.push((line, value));
                    }
                    Err(error) if first_load => self.issue(
                        format!("{path}:{line}"),
                        format!("invalid JSON: {}", json_parse_reason(&error)),
                    )?,
                    Err(_) => {}
                }
            }
        }
        self.cost.decoded_rows = self
            .cost
            .decoded_rows
            .checked_add(rows.len() as u64)
            .ok_or(ItemRefusal::Budget)?;
        if candidate_cached && first_load {
            self.finish_candidate_loaded_document_rows(path, &digest, rows.len() as u64)?;
        }
        drop(raw);
        let mut loaded = LoadedRows {
            digest,
            rows,
            temporary_state_bytes: loaded_state_bytes,
        };
        if candidate_cached {
            self.adjust_loaded_state(&mut loaded_state_bytes, loaded_clone_cost(&loaded)?)?;
            loaded.temporary_state_bytes = loaded_state_bytes;
        }
        if !candidate_cached {
            self.loaded.insert(path.to_owned(), loaded.clone());
        }
        Ok(Some(loaded))
    }

    fn loaded_value_at(
        &mut self,
        path: &str,
        line: usize,
    ) -> Result<(Option<Value>, usize), ItemRefusal> {
        if self.schema_request_store.is_none() {
            return Ok((
                self.loaded.get(path).and_then(|rows| {
                    rows.rows
                        .iter()
                        .find(|(candidate, _)| *candidate == line)
                        .map(|(_, value)| value.clone())
                }),
                0,
            ));
        }
        let Some(digest) = self.candidate_loaded_digest(path)? else {
            return Ok((None, 0));
        };
        let digest_state = estimate_string_storage(&digest)?;
        self.reserve_temporary(digest_state)?;
        let Some(raw_line) = self.candidate_loaded_row(path, line, &digest)? else {
            drop(digest);
            self.release_temporary_state(digest_state)?;
            return Ok((None, 0));
        };
        drop(digest);
        self.release_temporary_state(digest_state)?;
        let reservation = raw_line
            .len()
            .checked_mul(6)
            .and_then(|state| state.checked_add(std::mem::size_of::<Value>()))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_temporary(reservation)?;
        let value = serde_json::from_slice::<Value>(&raw_line).map_err(|_| {
            ItemRefusal::Source("source-foundation loaded physical row no longer parses".into())
        })?;
        let value_state = crate::record_biblio_cut::decoded_state(&value)?;
        if value_state > reservation {
            self.reserve_temporary(value_state - reservation)?;
        } else if value_state < reservation {
            self.temporary_state_bytes = self
                .temporary_state_bytes
                .checked_sub(reservation - value_state)
                .ok_or(ItemRefusal::Budget)?;
        }
        drop(raw_line);
        self.cost.decoded_rows = self
            .cost
            .decoded_rows
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        Ok((Some(value), value_state))
    }

    fn expect_ref(
        &mut self,
        owner: &str,
        reference: Option<&str>,
        expected_kind: &str,
    ) -> Result<(), ItemRefusal> {
        // Maintained require_record ignores non-string references; the actual
        // schema request owns their type failure. All kinds, including Item
        // and Link, resolve through the same first-selected current map.
        let Some(reference) = reference else {
            return Ok(());
        };
        let (lookup, lookup_state) = self.current_record_with_state_budget(reference)?;
        let classification = (|| match lookup.as_ref() {
            None => Ok(None),
            Some(record) => {
                let kind = record.value.get("record_type").unwrap_or(&Value::Null);
                if text(&record.value, "record_type") == Some(expected_kind) {
                    Ok(Some(Ok(())))
                } else {
                    let length = crate::source_foundation_records::python_value_string_len(kind)?;
                    self.reserve_temporary(length)?;
                    Ok(Some(Err((
                        length,
                        crate::source_foundation_records::python_value_string(kind),
                    ))))
                }
            }
        })();
        drop(lookup);
        self.release_temporary_state(lookup_state)?;
        let classification = classification?;
        match classification {
            None => self.issue(
                owner,
                format!("unresolved {expected_kind} reference: {reference}"),
            )?,
            Some(Err((length, displayed))) => {
                self.issue(
                    owner,
                    format!("{reference} resolves to {displayed}, expected {expected_kind}"),
                )?;
                drop(displayed);
                self.release_temporary_state(length)?;
            }
            Some(Ok(())) => {}
        }
        Ok(())
    }

    fn digest_for(&mut self, path: &str) -> Result<Option<String>, ItemRefusal> {
        if self.cache_digests {
            if let Some(value) = self.digests.get(path) {
                return Ok(Some(value.clone()));
            }
        }
        if self.schema_request_store.is_some() {
            if let Some(digest) = self.candidate_loaded_digest(path)? {
                return Ok(Some(digest));
            }
        }
        if !self.path_exists(path)? {
            return Ok(None);
        }
        let Some(raw) = self.current_raw(path)? else {
            return Ok(None);
        };
        if self.cache_digests {
            return Ok(Some(Digest256::of_bytes(&raw).to_hex()));
        }
        let temporary_baseline = self.temporary_state_bytes;
        self.reserve_temporary(
            raw.len()
                .checked_add(std::mem::size_of::<Vec<u8>>() + 64)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let digest = Digest256::of_bytes(&raw).to_hex();
        drop(raw);
        self.release_temporary_since(temporary_baseline);
        Ok(Some(digest))
    }

    fn request_schema(
        &mut self,
        location: &str,
        contract: &str,
        document: &Value,
    ) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if self.schema_request_store.is_some() {
            let temporary_baseline = self.temporary_state_bytes;
            let request_state = estimate_value_storage(document)?
                .checked_add(estimate_string_storage(location)?)
                .and_then(|state| state.checked_add(estimate_string_storage(contract).ok()?))
                .and_then(|state| {
                    state.checked_add(self.limits.max_member_bytes.checked_add(
                        std::mem::size_of::<SourceFoundationClosureSchemaRequest>() + 256,
                    )?)
                })
                .ok_or(ItemRefusal::Budget)?;
            self.reserve_temporary(request_state)?;
            let request = SourceFoundationClosureSchemaRequest {
                before_issue: self.issues.len(),
                location: location.to_owned(),
                contract: contract.to_owned(),
                document: document.clone(),
            };
            let remaining = self.remaining_state()?;
            let (workspace, store_cost) = {
                let store = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?;
                let workspace =
                    store.record_request(&request, self.limits.max_member_bytes, remaining)?;
                (workspace, store.cost())
            };
            self.reserve_temporary(workspace)?;
            drop(request);
            self.release_temporary_since(temporary_baseline);
            self.cost.candidate_schema_request_count = store_cost.observation_rows;
            self.cost.candidate_schema_request_serialized_write_bytes =
                store_cost.serialized_write_bytes;
            self.cost.candidate_schema_request_scan_row_operations = store_cost.scan_row_operations;
            self.cost
                .candidate_schema_request_peak_workspace_state_bytes = self
                .cost
                .candidate_schema_request_peak_workspace_state_bytes
                .max(store_cost.workspace_state_bytes)
                .max(workspace);
            self.cost.schema_requests = self
                .cost
                .schema_requests
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            return Ok(());
        }
        let retained = serde_json::to_vec(document)
            .map_err(|_| ItemRefusal::Budget)?
            .len()
            .checked_mul(8)
            .and_then(|bytes| {
                bytes.checked_add(
                    location.len()
                        + contract.len()
                        + std::mem::size_of::<SourceFoundationClosureSchemaRequest>()
                        + 96,
                )
            })
            .ok_or(ItemRefusal::Budget)?;
        self.reserve(retained)?;
        self.schema_requests
            .push(SourceFoundationClosureSchemaRequest {
                before_issue: self.issues.len(),
                location: location.to_owned(),
                contract: contract.to_owned(),
                document: document.clone(),
            });
        self.cost.schema_requests = self
            .cost
            .schema_requests
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn recorded_matches(&mut self, path: &str, digest: &str) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let key = if self.cache_recorded_checks {
            let key = (path.to_owned(), digest.to_owned());
            if let Some(matches) = self.recorded_checks.get(&key) {
                return Ok(*matches);
            }
            Some(key)
        } else {
            None
        };
        let temporary_baseline = self.temporary_state_bytes;
        if self.cache_recorded_checks {
            self.reserve(
                path.len()
                    .checked_add(digest.len())
                    .and_then(|bytes| bytes.checked_add(96))
                    .ok_or(ItemRefusal::Budget)?,
            )?;
        } else {
            self.reserve_temporary(
                self.limits
                    .max_member_bytes
                    .checked_add(std::mem::size_of::<Vec<u8>>() + 64)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
        }
        self.cost.recorded_read_operations = self
            .cost
            .recorded_read_operations
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        let raw = self.source.recorded(
            path,
            digest,
            self.limits.max_member_bytes,
            self.limits.deadline,
        )?;
        if let Some(bytes) = &raw {
            self.cost.recorded_bytes_read = self
                .cost
                .recorded_bytes_read
                .checked_add(bytes.len() as u64)
                .filter(|n| n <= &self.limits.max_total_bytes)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure recorded metadata bytes",
                    used: None,
                    limit: Some(self.limits.max_total_bytes),
                })?;
            self.cost.files_read = self
                .cost
                .files_read
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            self.cost.recorded_files_read = self
                .cost
                .recorded_files_read
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        let matches = raw.is_some_and(|bytes| Digest256::of_bytes(&bytes).to_hex() == digest);
        if let Some(key) = key {
            self.recorded_checks.insert(key, matches);
        } else {
            self.release_temporary_since(temporary_baseline);
        }
        Ok(matches)
    }

    fn current_exists(&mut self, path: &str) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if !self.path_exists(path)? {
            return Ok(false);
        }
        self.source
            .exists(path, self.limits.max_member_bytes, self.limits.deadline)
    }

    fn claim_id(
        &mut self,
        location: &str,
        row: &Value,
    ) -> Result<(Option<String>, usize), ItemRefusal> {
        let Some(id) = row.get("claim_id").and_then(Value::as_str) else {
            return Ok((None, 0));
        };
        let candidate_id_state_bytes = estimate_string_storage(id)?;
        let duplicate = if self.schema_request_store.is_some() {
            self.reserve_temporary(candidate_id_state_bytes)?;
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_claim_id(id, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                self.cost.candidate_claim_id_count = self
                    .cost
                    .candidate_claim_id_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
            !inserted
        } else {
            self.reserve(
                id.len()
                    .checked_add(std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            !self.claim_ids.insert(id.to_owned())
        };
        if duplicate {
            self.issue(location, format!("duplicate claim_id: {id}"))?;
        }
        Ok((
            Some(id.to_owned()),
            if self.schema_request_store.is_some() {
                candidate_id_state_bytes
            } else {
                0
            },
        ))
    }

    fn contains_claim_id(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        if self.schema_request_store.is_none() {
            return Ok(self.claim_ids.contains(id));
        }
        let remaining = self.remaining_state()?;
        let (found, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .contains_claim_id(id, remaining)?;
        self.include_store_workspace(workspace)?;
        Ok(found)
    }

    fn remember_membership_claim(&mut self, id: &str, subject: &str) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_membership_claim(id, subject, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_membership_claim_count = self
                .cost
                .candidate_membership_claim_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn contains_membership_claim(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        if self.schema_request_store.is_none() {
            return Ok(self.membership.contains_key(id));
        }
        let remaining = self.remaining_state()?;
        let (found, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .contains_membership_claim(id, remaining)?;
        self.include_store_workspace(workspace)?;
        Ok(found)
    }

    fn remember_candidate_responsibility_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_responsibility_claim(id, reference, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.responsibility_claim_count = self
                .responsibility_claim_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            self.cost.candidate_responsibility_claim_count = self
                .cost
                .candidate_responsibility_claim_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn remember_candidate_publication_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_publication_claim(id, reference, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_publication_claim_count = self
                .cost
                .candidate_publication_claim_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn remember_candidate_topology_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_topology_claim(id, reference, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_topology_store.claim_rows = self
                .cost
                .candidate_topology_store
                .claim_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn remember_candidate_object_link_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_object_link_claim(id, reference, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_object_link_store.claim_rows = self
                .cost
                .candidate_object_link_store
                .claim_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn remember_candidate_provision_claim(
        &mut self,
        id: &str,
        reference: &SourceFoundationClosureClaimRef,
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_provision_claim(id, reference, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_provision_claim_count = self
                .cost
                .candidate_provision_claim_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn remember_candidate_provision_event_id(&mut self, id: &str) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_provision_event_id(id, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_provision_event_id_count = self
                .cost
                .candidate_provision_event_id_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn remember_provision_used_event(
        &mut self,
        id: &str,
        compatibility_ids: &mut BTreeSet<String>,
    ) -> Result<bool, ItemRefusal> {
        if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_provision_used_event(id, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                self.cost.candidate_provision_used_event_count = self
                    .cost
                    .candidate_provision_used_event_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
            Ok(inserted)
        } else if compatibility_ids.contains(id) {
            Ok(false)
        } else {
            self.reserve_temporary(
                id.len()
                    .checked_add(std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            compatibility_ids.insert(id.to_owned());
            Ok(true)
        }
    }

    fn remember_provision_validated_event(
        &mut self,
        id: &str,
        compatibility_ids: &mut BTreeSet<String>,
    ) -> Result<bool, ItemRefusal> {
        if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_provision_validated_event(id, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                self.cost.candidate_provision_validated_event_count = self
                    .cost
                    .candidate_provision_validated_event_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
            Ok(inserted)
        } else if compatibility_ids.contains(id) {
            Ok(false)
        } else {
            self.reserve_temporary(
                id.len()
                    .checked_add(std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            compatibility_ids.insert(id.to_owned());
            Ok(true)
        }
    }

    fn contains_candidate_provision_used_event(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (found, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .contains_provision_used_event(id, remaining)?;
        self.include_store_workspace(workspace)?;
        Ok(found)
    }

    fn remember_responsibility_validated_event(
        &mut self,
        event_id: &str,
        compatibility_ids: &mut BTreeSet<String>,
    ) -> Result<bool, ItemRefusal> {
        if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_responsibility_validated_event(event_id, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                self.cost.candidate_responsibility_validated_event_count = self
                    .cost
                    .candidate_responsibility_validated_event_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
            Ok(inserted)
        } else if compatibility_ids.contains(event_id) {
            Ok(false)
        } else {
            self.reserve_temporary(
                event_id
                    .len()
                    .checked_add(std::mem::size_of::<String>())
                    .and_then(|bytes| bytes.checked_add(4 * std::mem::size_of::<usize>()))
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            compatibility_ids.insert(event_id.to_owned());
            Ok(true)
        }
    }

    fn remember_publication_validated_event(
        &mut self,
        event_id: &str,
        compatibility_ids: &mut BTreeSet<String>,
    ) -> Result<bool, ItemRefusal> {
        if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_publication_validated_event(event_id, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                self.cost.candidate_publication_validated_event_count = self
                    .cost
                    .candidate_publication_validated_event_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
            Ok(inserted)
        } else if compatibility_ids.contains(event_id) {
            Ok(false)
        } else {
            self.reserve_temporary(
                event_id
                    .len()
                    .checked_add(std::mem::size_of::<String>())
                    .and_then(|bytes| bytes.checked_add(4 * std::mem::size_of::<usize>()))
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            compatibility_ids.insert(event_id.to_owned());
            Ok(true)
        }
    }

    fn remember_boundary_responsibility_ref(
        &mut self,
        reference: &str,
        reserve_compatibility: bool,
    ) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_boundary_responsibility_ref(reference, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                self.cost.candidate_boundary_responsibility_ref_count = self
                    .cost
                    .candidate_boundary_responsibility_ref_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
        } else {
            if reserve_compatibility {
                self.reserve(
                    reference
                        .len()
                        .checked_add(
                            std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>(),
                        )
                        .ok_or(ItemRefusal::Budget)?,
                )?;
            }
            self.boundary_responsibility_refs
                .insert(reference.to_owned());
        }
        Ok(())
    }

    fn remember_boundary_membership_ref(&mut self, reference: &str) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_boundary_membership_ref(reference, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                self.cost.candidate_boundary_membership_refs.rows = self
                    .cost
                    .candidate_boundary_membership_refs
                    .rows
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
            }
        } else {
            self.boundary_membership_refs.insert(reference.to_owned());
        }
        Ok(())
    }

    fn remember_candidate_anchor_id(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_anchor_id(id, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_anchor_store.id_rows = self
                .cost
                .candidate_anchor_store
                .id_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(inserted)
    }

    fn anchor_id_exists(&mut self, id: &str) -> Result<bool, ItemRefusal> {
        if self.schema_request_store.is_none() {
            return Ok(self.anchors.contains(id));
        }
        let remaining = self.remaining_state()?;
        let (found, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .contains_anchor_id(id, remaining)?;
        self.include_store_workspace(workspace)?;
        let cost = &mut self.cost.candidate_anchor_store;
        cost.scan_row_operations = cost
            .scan_row_operations
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        if found {
            cost.serialized_read_bytes = cost
                .serialized_read_bytes
                .checked_add(u64::try_from(id.len()).map_err(|_| ItemRefusal::Budget)?)
                .ok_or(ItemRefusal::Budget)?;
        }
        cost.peak_workspace_state_bytes = cost.peak_workspace_state_bytes.max(workspace);
        Ok(found)
    }

    fn finish_candidate_anchor_ids(&mut self) -> Result<(), ItemRefusal> {
        let expected_rows = self.cost.candidate_anchor_store.id_rows;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_anchor_ids(expected_rows)?;
        let mut cursor_state_bytes = 0usize;
        let mut drained_rows = 0u64;
        loop {
            let remaining = self.remaining_state()?;
            let (id, workspace, row_state_bytes, retained_cursor_state_bytes) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_anchor_id(remaining)?;
            self.include_store_workspace(workspace)?;
            self.release_temporary_state(cursor_state_bytes)?;
            let Some(id) = id else {
                if row_state_bytes != 0 || retained_cursor_state_bytes != 0 {
                    return Err(ItemRefusal::Budget);
                }
                break;
            };
            if drained_rows >= expected_rows {
                return Err(ItemRefusal::Source(
                    "source-foundation Closure anchor ID drain exceeded its sealed count".into(),
                ));
            }
            let active_state = row_state_bytes
                .checked_add(retained_cursor_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            self.reserve_temporary(active_state)?;
            cursor_state_bytes = retained_cursor_state_bytes;
            drop(id);
            self.release_temporary_state(row_state_bytes)?;
            drained_rows = drained_rows.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
        if drained_rows != expected_rows {
            return Err(ItemRefusal::Source(
                "source-foundation Closure anchor ID drain count differs from its insert count"
                    .into(),
            ));
        }
        let remaining = self.remaining_state()?;
        let cost = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .finish_anchor_ids(expected_rows, remaining)?;
        self.include_store_workspace(cost.peak_workspace_state_bytes)?;
        if cost.id_rows != expected_rows
            || cost.drained_rows != expected_rows
            || !cost.eof_seen
            || !cost.count_verified
        {
            return Err(ItemRefusal::Source(
                "source-foundation Closure anchor ID store count differs from its ordered drain"
                    .into(),
            ));
        }
        self.cost.candidate_anchor_store = cost;
        Ok(())
    }

    fn check_records_map(&mut self) -> Result<(), ItemRefusal> {
        // Records owns schema, reference and duplicate findings. This boundary
        // only verifies caller map shape and supplies Link join rows;
        // repeating that owner's checks would change issue coverage/order.
        let records = self.records;
        let paths = self.paths;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let mut retained = self.retained_state_bytes;
        let temporary = self.temporary_state_bytes;
        let max_state = self.limits.max_state_bytes;
        let link_store = &mut self.link_store;
        let mut link_count = self.link_count;
        let mut link_workspace_peak = 0usize;
        let links = &mut self.links;
        records.for_each_current_record(&mut |id, record| {
            check(deadline, cancelled)?;
            if record.value.get("record_id").and_then(Value::as_str) != Some(id)
                || !paths
                    .contains_with_checkpoint(&record.path, &mut || check(deadline, cancelled))?
            {
                return Err(ItemRefusal::Source(
                    "source-foundation record map differs from its selected source input".into(),
                ));
            }
            if record.path.ends_with("/link.json") {
                if let Some(store) = link_store.as_deref_mut() {
                    let used = retained.checked_add(temporary).ok_or(ItemRefusal::Budget)?;
                    let remaining = max_state.checked_sub(used).ok_or(ItemRefusal::Budget)?;
                    let workspace =
                        store.insert_link(id, &record.path, &record.value, remaining)?;
                    if used.checked_add(workspace).ok_or(ItemRefusal::Budget)? > max_state {
                        return Err(ItemRefusal::Budget);
                    }
                    link_count = link_count.checked_add(1).ok_or(ItemRefusal::Budget)?;
                    link_workspace_peak = link_workspace_peak.max(workspace);
                    return Ok(());
                }
                let bytes = crate::record_biblio_cut::decoded_state(&record.value)
                    .map_err(|_| ItemRefusal::Budget)?;
                let clone_state = id
                    .len()
                    .checked_add(record.path.len())
                    .and_then(|n| n.checked_add(bytes))
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<(String, (String, Value))>() + 64)
                    })
                    .ok_or(ItemRefusal::Budget)?;
                retained = retained
                    .checked_add(clone_state)
                    .filter(|used| {
                        used.checked_add(temporary)
                            .is_some_and(|total| total <= max_state)
                    })
                    .ok_or(ItemRefusal::BudgetCheck {
                        check: "source-foundation closure Link index",
                        used: None,
                        limit: Some(max_state as u64),
                    })?;
                links.insert(id.to_owned(), (record.path.clone(), record.value.clone()));
            }
            Ok(())
        })?;
        self.retained_state_bytes = retained;
        self.link_count = link_count;
        let link_state_peak = retained
            .checked_add(temporary)
            .and_then(|state| state.checked_add(link_workspace_peak))
            .ok_or(ItemRefusal::Budget)?;
        if link_state_peak > max_state {
            return Err(ItemRefusal::Budget);
        }
        self.cost.reserved_state_bytes = self.cost.reserved_state_bytes.max(link_state_peak);
        Ok(())
    }

    fn collect_events(&mut self) -> Result<(), ItemRefusal> {
        let mut event_key_mismatch = false;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        self.source_events.for_each_event(&mut |id, event| {
            check(deadline, cancelled)?;
            if text(event, "event_id") != Some(id) {
                event_key_mismatch = true;
            }
            Ok(())
        })?;
        if event_key_mismatch {
            self.issue(
                SOURCE_HOME,
                "earlier-district event map key differs from event_id",
            )?;
        }
        if self.schema_request_store.is_some() {
            // Queue matching paths while the owner performs its authenticated
            // full walk. Read their source bytes only after that walk has
            // reached EOF, so the source adapter never has to reenter its
            // active member callback.
            let path_source = self.paths;
            let deadline = self.limits.deadline;
            let mut event_path_rows = 0u64;
            if self.scope == SourceFoundationDefaultRuleScope::FullAudit {
                for path in [
                    TOPOLOGY_PROVENANCE,
                    DERIVATION_PROVENANCE,
                    CHRONOLOGY_PROVENANCE,
                ] {
                    self.remember_candidate_event_path(path, &mut event_path_rows)?;
                }
            }
            path_source.for_each_path(&mut |path| {
                check(deadline, self.source.cancellation())?;
                if !self.source.selects_semantic_member(path)? {
                    return Ok(());
                }
                if path.ends_with(PROVISION_EVENT_BASENAME)
                    || self.scope.is_scoped()
                        && path.starts_with(SOURCE_HOME)
                        && (path.ends_with("/provenance.jsonl")
                            || path.contains("/provenance.") && path.ends_with(".jsonl"))
                {
                    self.remember_candidate_event_path(path, &mut event_path_rows)?;
                }
                Ok(())
            })?;
            self.schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .seal_event_paths(event_path_rows)?;
            let mut after_path: Option<String> = None;
            let mut cursor_state_bytes = 0usize;
            loop {
                let remaining = self.remaining_state()?;
                let (next_path, workspace, retained_cursor_state_bytes) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .next_event_path(after_path.as_deref(), remaining)?;
                self.include_store_workspace(workspace)?;
                let Some(path) = next_path else {
                    if retained_cursor_state_bytes != 0 {
                        return Err(ItemRefusal::Budget);
                    }
                    break;
                };
                let next_local_cursor_state_bytes = path
                    .len()
                    .checked_mul(2)
                    .and_then(|state| {
                        state.checked_add(
                            std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>(),
                        )
                    })
                    .ok_or(ItemRefusal::Budget)?;
                let next_cursor_state_bytes = next_local_cursor_state_bytes
                    .checked_add(retained_cursor_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                self.reserve_temporary(next_cursor_state_bytes)?;
                drop(after_path.take());
                self.release_loaded_rows(cursor_state_bytes)?;
                after_path = Some(path);
                cursor_state_bytes = next_cursor_state_bytes;
                let event_path = after_path.as_deref().ok_or(ItemRefusal::Budget)?;
                let path_lookup_state = event_path
                    .len()
                    .checked_mul(32)
                    .and_then(|state| state.checked_add(10_240))
                    .ok_or(ItemRefusal::Budget)?;
                self.reserve_temporary(path_lookup_state)?;
                let exists = self.path_exists(event_path);
                let release = self.release_loaded_rows(path_lookup_state);
                let exists = exists?;
                release?;
                if exists {
                    self.collect_event_path(event_path)?;
                }
            }
            drop(after_path.take());
            self.release_loaded_rows(cursor_state_bytes)?;
            self.schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .finish_event_paths(event_path_rows)?;
        } else {
            self.reserve(
                std::mem::size_of::<BTreeSet<String>>()
                    + 3 * (std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
                    + TOPOLOGY_PROVENANCE.len()
                    + DERIVATION_PROVENANCE.len()
                    + CHRONOLOGY_PROVENANCE.len(),
            )?;
            let mut event_paths = BTreeSet::from([
                TOPOLOGY_PROVENANCE.to_owned(),
                DERIVATION_PROVENANCE.to_owned(),
                CHRONOLOGY_PROVENANCE.to_owned(),
            ]);
            event_paths.extend(self.collect_current_paths(
                |path| path.ends_with(PROVISION_EVENT_BASENAME),
                "source-foundation closure event-path index",
            )?);
            for path in event_paths {
                if self.path_exists(&path)? {
                    self.collect_event_path(&path)?;
                }
            }
        }
        Ok(())
    }

    fn remember_candidate_event_path(
        &mut self,
        path: &str,
        event_path_rows: &mut u64,
    ) -> Result<(), ItemRefusal> {
        let path_state_bytes = path
            .len()
            .checked_mul(2)
            .and_then(|state| {
                state.checked_add(std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
            })
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_temporary(path_state_bytes)?;
        let result = (|| {
            let remaining = self.remaining_state()?;
            let (inserted, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_event_path(path, remaining)?;
            self.include_store_workspace(workspace)?;
            if inserted {
                *event_path_rows = event_path_rows.checked_add(1).ok_or(ItemRefusal::Budget)?;
                self.cost.candidate_event_path_count = *event_path_rows;
            }
            Ok(())
        })();
        let release = self.release_loaded_rows(path_state_bytes);
        result?;
        release
    }

    fn collect_event_path(&mut self, path: &str) -> Result<(), ItemRefusal> {
        let loaded = if self.scope.is_scoped() {
            self.unchecked_jsonl_rows(path)?
        } else {
            self.json_rows(path, PROVENANCE_SCHEMA, false)?
        };
        let Some(loaded) = loaded else {
            return Ok(());
        };
        let loaded_state_bytes = loaded.temporary_state_bytes;
        for (line, event) in loaded.rows {
            check(self.limits.deadline, self.source.cancellation())?;
            let location = format!("{path}:{line}");
            if self.scope.is_scoped() {
                let contract = if text(&event, "schema_version") == Some("tos_provenance_event_v2")
                {
                    PROVENANCE_V2_SCHEMA
                } else {
                    PROVENANCE_SCHEMA
                };
                self.request_schema(&location, contract, &event)?;
            }
            self.validate_source_refs(&location, &event)?;
            let Some(id) = text(&event, "event_id").map(str::to_owned) else {
                continue;
            };
            let candidate_store_active = self.schema_request_store.is_some();
            if !candidate_store_active {
                let event_state = crate::record_biblio_cut::decoded_state(&event)?;
                self.reserve(
                    id.len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(event_state))
                        .and_then(|n| {
                            n.checked_add(
                                2 * std::mem::size_of::<String>()
                                    + 2 * std::mem::size_of::<usize>()
                                    + std::mem::size_of::<Value>(),
                            )
                        })
                        .ok_or(ItemRefusal::Budget)?,
                )?;
            }
            let source_has_id = self.source_events.event_contains(&id)?;
            let selected_first = if self.scope.is_scoped() {
                if candidate_store_active {
                    let remaining = self.remaining_state()?;
                    let (inserted, workspace) = self
                        .schema_request_store
                        .as_deref_mut()
                        .ok_or(crate::item_budget_origin!())?
                        .remember_event(&id, path, line, &loaded.digest, &event, remaining)?;
                    self.include_store_workspace(workspace)?;
                    if inserted {
                        self.cost.candidate_event_count = self
                            .cost
                            .candidate_event_count
                            .checked_add(1)
                            .ok_or(crate::item_budget_origin!())?;
                    }
                    inserted
                } else {
                    self.event_ids.insert(id.clone())
                }
            } else {
                true
            };
            let duplicate = if !selected_first {
                true
            } else if source_has_id {
                if self.scope.is_scoped() {
                    let prior_events = self.source_events;
                    match prior_events.event(&id)? {
                        Some(prior) => !self.python_equal(&prior, &event)?,
                        None => true,
                    }
                } else {
                    true
                }
            } else if self.scope.is_scoped() {
                false
            } else if candidate_store_active {
                let remaining = self.remaining_state()?;
                let (inserted, workspace) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .remember_event(&id, path, line, &loaded.digest, &event, remaining)?;
                self.include_store_workspace(workspace)?;
                if inserted {
                    self.cost.candidate_event_count = self
                        .cost
                        .candidate_event_count
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?;
                }
                !inserted
            } else {
                !self.event_ids.insert(id.to_owned())
            };
            if duplicate {
                self.issue(
                    format!("{path}:{line}"),
                    format!("duplicate event_id: {id}"),
                )?;
            }
            if path.ends_with(PROVISION_EVENT_BASENAME) {
                if candidate_store_active {
                    self.remember_candidate_provision_event_id(&id)?;
                } else {
                    self.reserve(
                        id.len() + std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>(),
                    )?;
                    self.provision_event_ids.insert(id.to_owned());
                }
            }
            // All borrowed event-id observations finish before ownership moves.
            if !duplicate && self.schema_request_store.is_none() {
                self.events.insert(id.to_owned(), event);
            }
        }
        self.release_loaded_rows(loaded_state_bytes)?;
        Ok(())
    }

    fn check_claim_streams(&mut self) -> Result<(), ItemRefusal> {
        let claims = self.claims;
        let paths = self.paths;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let limits = self.limits;
        let temporary = self.temporary_state_bytes;
        let issues = &mut self.issues;
        let cost = &mut self.cost;
        let retained = &mut self.retained_state_bytes;
        claims.for_each_claim(&mut |ordinal, claim| {
            check(deadline, cancelled)?;
            if !claim.path.starts_with(SOURCE_HOME) || !claim.path.ends_with("-claims.jsonl") {
                return push_bounded_issue(
                    issues,
                    cost,
                    retained,
                    temporary,
                    limits,
                    cancelled,
                    &claim.path,
                    "bibliography report contains a non-source Claim route".to_owned(),
                );
            }
            if !paths.contains(&claim.path)? {
                return push_bounded_issue(
                    issues,
                    cost,
                    retained,
                    temporary,
                    limits,
                    cancelled,
                    &claim.path,
                    "bibliography Claim row is outside the captured source membership".to_owned(),
                );
            }
            if claim.line == 0 {
                push_bounded_issue(
                    issues,
                    cost,
                    retained,
                    temporary,
                    limits,
                    cancelled,
                    &claim.path,
                    format!(
                        "bibliography report repeats or misnumbers Claim line {}",
                        claim.line
                    ),
                )?;
            }
            if claims
                .first_claim_at(&claim.path, claim.line)?
                .is_some_and(|(first, _)| first != ordinal)
            {
                push_bounded_issue(
                    issues,
                    cost,
                    retained,
                    temporary,
                    limits,
                    cancelled,
                    &claim.path,
                    format!(
                        "bibliography report repeats or misnumbers Claim line {}",
                        claim.line
                    ),
                )?;
            }
            Ok(())
        })?;

        let claim_paths = self.collect_current_paths(
            |path| path.ends_with("-claims.jsonl"),
            "source-foundation closure Claim path index",
        )?;
        for path in claim_paths {
            if self.claims.claim_count_for_path(&path)? > 0 {
                let Some(current) = self.unchecked_jsonl_rows(&path)? else {
                    self.issue(
                        &path,
                        "bibliography Claim file is absent from the current cut",
                    )?;
                    continue;
                };
                let loaded_state_bytes = current.temporary_state_bytes;
                let distinct = self.claims.distinct_nonzero_claim_lines_for_path(&path)?;
                let has_zero_line = self.claims.first_claim_at(&path, 0)?.is_some();
                let mut exact_lines = distinct == current.rows.len() as u64 && !has_zero_line;
                let mut digest_mismatch = false;
                for (line, current_value) in &current.rows {
                    check(self.limits.deadline, self.source.cancellation())?;
                    let Some((_, claim)) = self.claims.last_claim_at(&path, *line)? else {
                        exact_lines = false;
                        continue;
                    };
                    if claim.raw_sha256 != current.digest {
                        digest_mismatch = true;
                        break;
                    }
                    if !self.python_equal(current_value, &claim.value)? {
                        self.issue(
                            format!("{}:{}", path, line),
                            "bibliography Claim value differs from the exact current line",
                        )?;
                    }
                }
                if !exact_lines {
                    self.issue(
                        &path,
                        "bibliography Claim rows do not cover the exact current file lines",
                    )?;
                }
                if digest_mismatch {
                    self.issue(
                        &path,
                        "bibliography Claim digest differs from the exact current file",
                    )?;
                }
                for (line, claim) in current.rows {
                    let mut native = false;
                    self.claims.for_each_claim_at(&path, line, &mut |_, row| {
                        native |= row.native;
                        Ok(())
                    })?;
                    self.register_claim(&path, line, &claim, native)?;
                }
                self.release_loaded_rows(loaded_state_bytes)?;
            } else if path.ends_with("/source-claims.jsonl")
                || path.ends_with("/historical-claims.jsonl")
            {
                let Some(current) = self.unchecked_jsonl_rows(&path)? else {
                    self.issue(&path, "source Claim file is absent from the current cut")?;
                    continue;
                };
                let loaded_state_bytes = current.temporary_state_bytes;
                if !current.rows.is_empty() {
                    self.gap(&path, "this profile requires the exact-cut biblio_rules Claim report for source-declared profile and native compound validation")?;
                    self.release_loaded_rows(loaded_state_bytes)?;
                    continue;
                }
                for (line, claim) in current.rows {
                    self.register_claim(&path, line, &claim, false)?;
                }
                self.release_loaded_rows(loaded_state_bytes)?;
            } else {
                let contract = if path.ends_with("/object-link-claims.jsonl") {
                    OBJECT_LINK_SCHEMA
                } else {
                    CLAIM_SCHEMA
                };
                let Some(current) = self.json_rows(&path, contract, true)? else {
                    continue;
                };
                let loaded_state_bytes = current.temporary_state_bytes;
                for (line, claim) in current.rows {
                    self.register_claim(&path, line, &claim, false)?;
                }
                self.release_loaded_rows(loaded_state_bytes)?;
            }
        }
        if self.schema_request_store.is_some() {
            let expected_rows = self.cost.candidate_boundary_membership_refs.rows;
            self.schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .begin_boundary_membership_refs(expected_rows)?;
            let mut cursor_state_bytes = 0usize;
            let mut drained_rows = 0u64;
            loop {
                let remaining = self.remaining_state()?;
                let (reference, workspace, row_state_bytes, retained_cursor_state_bytes) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .next_boundary_membership_ref(remaining)?;
                self.include_store_workspace(workspace)?;
                self.release_temporary_state(cursor_state_bytes)?;
                let Some(reference) = reference else {
                    if row_state_bytes != 0
                        || retained_cursor_state_bytes != 0
                        || drained_rows != expected_rows
                    {
                        return Err(ItemRefusal::Source(
                            "source-foundation boundary Membership ref count differs from its ordered drain"
                                .into(),
                        ));
                    }
                    self.cost.candidate_boundary_membership_refs.drained_rows = drained_rows;
                    self.cost.candidate_boundary_membership_refs.eof_seen = true;
                    break;
                };
                let active_state_bytes = row_state_bytes
                    .checked_add(retained_cursor_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                if active_state_bytes > self.remaining_state()? {
                    return Err(ItemRefusal::Budget);
                }
                self.reserve_temporary(active_state_bytes)?;
                cursor_state_bytes = retained_cursor_state_bytes;
                if !self.contains_membership_claim(&reference)? {
                    self.issue(
                        SOURCE_HOME,
                        format!(
                            "work-boundary maps reference missing membership claims: [{reference}]"
                        ),
                    )?;
                }
                drop(reference);
                self.release_temporary_state(row_state_bytes)?;
                drained_rows = drained_rows.checked_add(1).ok_or(ItemRefusal::Budget)?;
            }
        } else {
            let membership_refs = std::mem::take(&mut self.boundary_membership_refs);
            for reference in membership_refs {
                if !self.contains_membership_claim(&reference)? {
                    self.issue(
                        SOURCE_HOME,
                        format!(
                            "work-boundary maps reference missing membership claims: [{reference}]"
                        ),
                    )?;
                }
            }
        }
        if self.schema_request_store.is_some() {
            let expected_rows = self.cost.candidate_boundary_responsibility_ref_count;
            self.schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .begin_boundary_responsibility_refs(expected_rows)?;
            let mut cursor_state_bytes = 0usize;
            loop {
                let remaining = self.remaining_state()?;
                let (reference, workspace, row_state_bytes, retained_cursor_state_bytes) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .next_boundary_responsibility_ref(remaining)?;
                self.include_store_workspace(workspace)?;
                self.release_temporary_state(cursor_state_bytes)?;
                let Some(reference) = reference else {
                    if row_state_bytes != 0 || retained_cursor_state_bytes != 0 {
                        return Err(ItemRefusal::Budget);
                    }
                    break;
                };
                let active_state_bytes = row_state_bytes
                    .checked_add(retained_cursor_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                if active_state_bytes > self.remaining_state()? {
                    return Err(ItemRefusal::Budget);
                }
                self.reserve_temporary(active_state_bytes)?;
                cursor_state_bytes = retained_cursor_state_bytes;
                let remaining = self.remaining_state()?;
                let (found, lookup_workspace) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .contains_responsibility_claim(&reference, remaining)?;
                self.include_store_workspace(lookup_workspace)?;
                if !found {
                    self.issue(
                        SOURCE_HOME,
                        format!(
                            "work-boundary maps reference missing responsibility claims: [{reference}]"
                        ),
                    )?;
                }
                self.release_temporary_state(row_state_bytes)?;
                drop(reference);
            }
        } else {
            for reference in &self.boundary_responsibility_refs {
                if !self.responsibility.contains_key(reference) {
                    push_bounded_issue(
                        &mut self.issues,
                        &mut self.cost,
                        &mut self.retained_state_bytes,
                        self.temporary_state_bytes,
                        self.limits,
                        self.source.cancellation(),
                        SOURCE_HOME,
                        format!(
                            "work-boundary maps reference missing responsibility claims: [{reference}]"
                        ),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn check_topology(&mut self) -> Result<(), ItemRefusal> {
        let Some(events) = self.json_rows(TOPOLOGY_PROVENANCE, PROVENANCE_SCHEMA, true)? else {
            return Ok(());
        };
        let events_state_bytes = events.temporary_state_bytes;
        if events.rows.len() != 1 {
            self.issue(
                TOPOLOGY_PROVENANCE,
                "bibliographic topology must have exactly one batch provenance event",
            )?;
        }
        let event_source = events.rows.first().map(|(_, event)| event);
        let event_clone_state = if self.schema_request_store.is_some() {
            event_source
                .map(crate::record_biblio_cut::decoded_state)
                .transpose()?
                .unwrap_or_default()
        } else {
            0
        };
        if event_clone_state > 0 {
            self.reserve_temporary(event_clone_state)?;
        }
        let event = event_source.cloned();
        if let Some(event) = &event {
            if text(event, "event_id") != Some(TOPOLOGY_EVENT) {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance event_id differs from the owned route",
                )?;
            }
            if text(event, "event_type") != Some("annotation") {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance must be annotation",
                )?;
            }
            if value_strings(event, "agent_refs") != vec!["model:codex".to_owned()] {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance agent must be model:codex",
                )?;
            }
            let method = event.get("method").unwrap_or(&Value::Null);
            if text(method, "maker_type") != Some("model")
                || text(method, "name") != Some("declared-bibliographic-topology-materialization")
                || text(method, "version") != Some("1")
            {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance method identity drifted",
                )?;
            }
            if text(event, "status") != Some("completed_with_warnings") {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance must retain completed-with-warnings posture",
                )?;
            }
        }

        let mut counts = BTreeMap::<String, u64>::new();
        for (path, predicate, subject_kind, object_kind, backref, role) in TOPOLOGY_ROUTES {
            let suffix = path.rsplit('/').next().unwrap_or(path);
            let actual_paths = self.collect_current_paths(
                |candidate| candidate.ends_with(suffix),
                "source-foundation closure topology path index",
            )?;
            if actual_paths.len() != 1 || actual_paths.first().map(String::as_str) != Some(path) {
                self.issue(path, "bibliographic topology claim basename must exist only at its owned relation route")?;
            }
            let Some(claim_file) = self.json_rows(path, CLAIM_SCHEMA, true)? else {
                continue;
            };
            let claim_file_state_bytes = claim_file.temporary_state_bytes;
            if let Some(event) = &event {
                if !output_binds(event, path, role, &claim_file.digest) {
                    self.issue(path, "bibliographic topology provenance event does not digest-bind the claim file")?;
                }
            }
            for (line, claim) in &claim_file.rows {
                let location = format!("{path}:{line}");
                *counts.entry((*predicate).to_owned()).or_default() += 1;
                let claim_id = text(claim, "claim_id").unwrap_or_default();
                let subject_ref = text(claim, "subject_ref");
                let object_ref = text(claim, "object");
                if text(claim, "claim_type") != Some("bibliographic") {
                    self.issue(
                        &location,
                        "bibliographic topology claim_type must be bibliographic",
                    )?;
                }
                if text(claim, "assertion_layer") != Some("bibliographic_assertion") {
                    self.issue(
                        &location,
                        "bibliographic topology assertion_layer must be bibliographic_assertion",
                    )?;
                }
                if text(claim, "predicate") != Some(predicate) {
                    self.issue(
                        &location,
                        format!(
                            "{} predicate must be {predicate}",
                            path.rsplit('/').next().unwrap_or(path)
                        ),
                    )?;
                }
                self.expect_ref(&location, subject_ref, subject_kind)?;
                self.expect_ref(&location, object_ref, object_kind)?;
                let maker_matches = if self.schema_request_store.is_some() {
                    let maker_state = estimate_fixed_json_object_storage(&[
                        ("maker_type", Some("model")),
                        ("agent_ref", Some("model:codex")),
                    ])?;
                    self.reserve_temporary(maker_state)?;
                    let expected_maker =
                        serde_json::json!({"maker_type":"model","agent_ref":"model:codex"});
                    let matches = self
                        .python_equal(claim.get("maker").unwrap_or(&Value::Null), &expected_maker);
                    drop(expected_maker);
                    self.release_temporary_state(maker_state)?;
                    matches?
                } else {
                    self.python_equal(
                        claim.get("maker").unwrap_or(&Value::Null),
                        &serde_json::json!({"maker_type":"model","agent_ref":"model:codex"}),
                    )?
                };
                if !maker_matches {
                    self.issue(
                        &location,
                        "bibliographic topology maker must be model:codex",
                    )?;
                }
                if text(claim, "provenance_event_ref") != Some(TOPOLOGY_EVENT) {
                    self.issue(
                        &location,
                        "bibliographic topology claim cites the wrong provenance event",
                    )?;
                }
                if text(claim, "epistemic_status") != Some("observed") {
                    self.issue(
                        &location,
                        "declared topology materialization must remain observed",
                    )?;
                }
                if text(claim, "review_status") != Some("unreviewed")
                    || !claim
                        .get("reviews")
                        .and_then(Value::as_array)
                        .is_some_and(Vec::is_empty)
                {
                    self.issue(
                        &location,
                        "bibliographic topology claims must remain unreviewed",
                    )?;
                }
                if text(claim, "visibility") != Some("public_metadata_only") {
                    self.issue(
                        &location,
                        "bibliographic topology claims must remain public metadata only",
                    )?;
                }

                // The maintained evidence lookup stringifies even malformed
                // endpoints; require_record above separately ignores their
                // non-string type. Preserve that negative-row distinction.
                let subject_value = claim.get("subject_ref").unwrap_or(&Value::Null);
                let object_value = claim.get("object").unwrap_or(&Value::Null);
                let endpoint_state =
                    crate::source_foundation_records::python_value_string_len(subject_value)?
                        .checked_add(crate::source_foundation_records::python_value_string_len(
                            object_value,
                        )?)
                        .ok_or(ItemRefusal::Budget)?;
                self.reserve(endpoint_state)?;
                let subject_key =
                    crate::source_foundation_records::python_value_string(subject_value);
                let object_key =
                    crate::source_foundation_records::python_value_string(object_value);
                let mut expected_evidence_state = std::mem::size_of::<BTreeSet<String>>();
                self.reserve_temporary(expected_evidence_state)?;
                let mut expected_evidence = BTreeSet::new();
                for endpoint in [&subject_key, &object_key] {
                    let (record_path, path_workspace) =
                        self.current_record_path_with_state_budget(endpoint)?;
                    if let Some(record_path) = record_path {
                        if expected_evidence.contains(&record_path) {
                            drop(record_path);
                            self.release_temporary_state(path_workspace)?;
                        } else {
                            expected_evidence_state = expected_evidence_state
                                .checked_add(path_workspace)
                                .ok_or(ItemRefusal::Budget)?;
                            expected_evidence.insert(record_path);
                        }
                    }
                }
                if object_kind == "item" {
                    let (object_record, object_workspace) =
                        self.current_record_with_state_budget(&object_key)?;
                    if let Some(manifest_ref) = object_record
                        .as_ref()
                        .and_then(|record| text(&record.value, "item_manifest_ref"))
                        && !expected_evidence.contains(manifest_ref)
                    {
                        let path_state = estimate_string_storage(manifest_ref)?
                            .checked_add(
                                std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>(),
                            )
                            .ok_or(ItemRefusal::Budget)?;
                        self.reserve_temporary(path_state)?;
                        expected_evidence_state = expected_evidence_state
                            .checked_add(path_state)
                            .ok_or(ItemRefusal::Budget)?;
                        expected_evidence.insert(manifest_ref.to_owned());
                    }
                    drop(object_record);
                    self.release_temporary_state(object_workspace)?;
                    if let (Some(item_id), Some(edition_id)) = (object_ref, subject_ref) {
                        if self.records.item_edition(item_id)?.as_deref() != Some(edition_id) {
                            self.issue(&location, "edition-item topology differs from the current item manifest embodiment")?;
                        }
                    }
                }
                let actual_evidence: BTreeSet<String> =
                    value_strings(claim, "evidence_refs").into_iter().collect();
                if actual_evidence != expected_evidence {
                    self.issue(&location, "bibliographic topology evidence must be the exact linked records and item manifest")?;
                }
                if let Some(event) = &event {
                    for evidence_ref in &expected_evidence {
                        let matches: Vec<&Value> = event
                            .get("inputs")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter(|input| text(input, "ref") == Some(evidence_ref.as_str()))
                            .collect();
                        let evidence_exists = self.current_exists(&evidence_ref)?;
                        let recorded = if matches.len() == 1 {
                            if let Some(digest) =
                                matches.first().and_then(|input| text(input, "sha256"))
                            {
                                self.recorded_matches(&evidence_ref, digest)?
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if matches.len() != 1 || !evidence_exists {
                            self.issue(&location, format!("bibliographic topology provenance does not digest-bind evidence input: {evidence_ref}"))?;
                        } else if !recorded {
                            self.issue(&location, format!("bibliographic topology provenance evidence bytes are unresolved: {evidence_ref}"))?;
                        }
                    }
                }
                drop(expected_evidence);
                self.release_temporary_state(expected_evidence_state)?;
                if !claim_id.is_empty() {
                    let differs = if self.schema_request_store.is_some() {
                        let remaining = self.remaining_state()?;
                        let (reference, workspace) = self
                            .schema_request_store
                            .as_deref_mut()
                            .ok_or(ItemRefusal::Budget)?
                            .topology_claim_by_id(claim_id, remaining)?;
                        self.include_store_workspace(workspace)?;
                        let differs = reference.as_ref().is_some_and(|reference| {
                            reference.subject != subject_ref.unwrap_or_default()
                                || reference.predicate != predicate
                                || reference.object != object_ref.unwrap_or_default()
                        });
                        drop(reference);
                        differs
                    } else {
                        self.topology.get(claim_id).is_some_and(|reference| {
                            reference.subject != subject_ref.unwrap_or_default()
                                || reference.predicate != predicate
                                || reference.object != object_ref.unwrap_or_default()
                        })
                    };
                    if differs {
                        self.issue(
                            &location,
                            "bibliographic topology claim differs from its source Claim row",
                        )?;
                    }
                }
            }
            self.check_topology_backrefs(subject_kind, backref, predicate)?;
            self.release_loaded_rows(claim_file_state_bytes)?;
        }

        if let Some(event) = &event {
            let expected_configuration_state = if self.schema_request_store.is_some() {
                Some(estimate_fixed_json_object_storage(&[
                    ("work_expression_claims_materialized", None),
                    ("expression_edition_claims_materialized", None),
                    ("edition_item_claims_materialized", None),
                    ("topology_claims_reviewed", None),
                    ("source_text_admitted", None),
                    ("human_review_performed", None),
                    ("textual_equivalence_claims_created", None),
                    ("semantic_claims_created", None),
                    ("canon_promotion_performed", None),
                ])?)
            } else {
                None
            };
            if let Some(state) = expected_configuration_state {
                self.reserve_temporary(state)?;
            }
            let expected_configuration = serde_json::json!({
                "work_expression_claims_materialized": counts.get("has_expression").copied().unwrap_or_default(),
                "expression_edition_claims_materialized": counts.get("embodied_by").copied().unwrap_or_default(),
                "edition_item_claims_materialized": counts.get("exemplified_by").copied().unwrap_or_default(),
                "topology_claims_reviewed": 0,
                "source_text_admitted": false,
                "human_review_performed": false,
                "textual_equivalence_claims_created": 0,
                "semantic_claims_created": 0,
                "canon_promotion_performed": false,
            });
            let configuration_matches = self.python_equal(
                event
                    .get("method")
                    .and_then(|method| method.get("configuration"))
                    .unwrap_or(&Value::Null),
                &expected_configuration,
            );
            drop(expected_configuration);
            if let Some(state) = expected_configuration_state {
                self.release_temporary_state(state)?;
            }
            if !configuration_matches? {
                self.issue(TOPOLOGY_PROVENANCE, "bibliographic topology provenance configuration differs from exact legacy batch counts and authority limits")?;
            }
        }
        self.release_loaded_rows(event_clone_state)?;
        self.release_loaded_rows(events_state_bytes)?;
        Ok(())
    }

    fn check_topology_backrefs(
        &mut self,
        subject_kind: &str,
        field: &str,
        predicate: &str,
    ) -> Result<(), ItemRefusal> {
        let candidate = self.schema_request_store.is_some();
        let ids_by_subject = if candidate {
            None
        } else {
            let index_state = self
                .topology
                .iter()
                .filter(|(_, claim)| claim.predicate == predicate)
                .try_fold(
                    std::mem::size_of::<BTreeMap<String, BTreeSet<String>>>(),
                    |state, (id, claim)| {
                        state
                            .checked_add(
                                id.len()
                                    .checked_add(claim.subject.len())
                                    .and_then(|n| {
                                        n.checked_add(
                                            2 * std::mem::size_of::<String>()
                                                + std::mem::size_of::<BTreeSet<String>>()
                                                + 8 * std::mem::size_of::<usize>(),
                                        )
                                    })
                                    .ok_or(ItemRefusal::Budget)?,
                            )
                            .ok_or(ItemRefusal::Budget)
                    },
                )?;
            self.reserve(index_state)?;
            let mut ids_by_subject = BTreeMap::<String, BTreeSet<String>>::new();
            for (id, claim) in self
                .topology
                .iter()
                .filter(|(_, claim)| claim.predicate == predicate)
            {
                ids_by_subject
                    .entry(claim.subject.clone())
                    .or_default()
                    .insert(id.clone());
            }
            Some(ids_by_subject)
        };
        let records = self.records;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let limits = self.limits;
        let temporary = self.temporary_state_bytes;
        let mut schema_request_store = self.schema_request_store.as_deref_mut();
        let issues = &mut self.issues;
        let cost = &mut self.cost;
        let retained = &mut self.retained_state_bytes;
        records.for_each_current_record(&mut |id, record| {
            check(deadline, cancelled)?;
            if record.kind != subject_kind {
                return Ok(());
            }
            let workspace = crate::record_biblio_cut::decoded_state(&record.value)?
                .checked_mul(2)
                .and_then(|n| n.checked_add(256))
                .ok_or(ItemRefusal::Budget)?;
            let temporary = temporary
                .checked_add(workspace)
                .ok_or(ItemRefusal::Budget)?;
            let used = retained.checked_add(temporary).ok_or(ItemRefusal::Budget)?;
            if used > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure topology backlink workspace",
                    used: Some(used as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            cost.reserved_state_bytes = cost.reserved_state_bytes.max(used);
            let actual: BTreeSet<String> =
                value_strings(&record.value, field).into_iter().collect();
            let active_temporary = temporary;
            let matches = if let Some(ids_by_subject) = ids_by_subject.as_ref() {
                ids_by_subject
                    .get(id)
                    .map_or(actual.is_empty(), |expected| expected == &actual)
            } else {
                let remaining = limits
                    .max_state_bytes
                    .checked_sub(used)
                    .ok_or(ItemRefusal::Budget)?;
                let mut actual_ids = actual.iter();
                let mut ordered_match = true;
                let mut visited = 0u64;
                let mut visit = |expected_id: &str, _workspace: usize| {
                    visited = visited.checked_add(1).ok_or(ItemRefusal::Budget)?;
                    if actual_ids.next().map(String::as_str) != Some(expected_id) {
                        ordered_match = false;
                    }
                    Ok(())
                };
                let (drained, store_workspace) = schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .for_each_topology_claim_for_subject(id, predicate, remaining, &mut visit)?;
                drop(visit);
                if actual_ids.next().is_some() {
                    ordered_match = false;
                }
                drop(actual_ids);
                if visited != drained {
                    return Err(ItemRefusal::Budget);
                }
                let total = used
                    .checked_add(store_workspace)
                    .ok_or(ItemRefusal::Budget)?;
                if total > limits.max_state_bytes {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "source-foundation closure topology stream workspace",
                        used: Some(total as u64),
                        limit: Some(limits.max_state_bytes as u64),
                    });
                }
                cost.reserved_state_bytes = cost.reserved_state_bytes.max(total);
                ordered_match
            };
            if !matches {
                push_bounded_issue(
                    issues,
                    cost,
                    retained,
                    active_temporary,
                    limits,
                    cancelled,
                    &record.path,
                    format!("{field} does not close over the exact outgoing {predicate} claims"),
                )?;
            }
            Ok(())
        })?;
        Ok(())
    }

    fn remember_candidate_derivation_key(
        &mut self,
        set: SourceFoundationClosureDerivationKeySet,
        key: &str,
    ) -> Result<bool, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = match set {
            SourceFoundationClosureDerivationKeySet::ClaimIds => self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_derivation_id(key, remaining)?,
            SourceFoundationClosureDerivationKeySet::Endpoints => self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_derivation_endpoint(key, remaining)?,
            SourceFoundationClosureDerivationKeySet::EvidencePaths => self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_derivation_evidence_path(key, remaining)?,
            SourceFoundationClosureDerivationKeySet::ExpectedInputs => self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .remember_derivation_expected_input(key, remaining)?,
            SourceFoundationClosureDerivationKeySet::Roots => return Err(ItemRefusal::Budget),
        };
        self.include_store_workspace(workspace)?;
        if inserted {
            let count = match set {
                SourceFoundationClosureDerivationKeySet::ClaimIds => {
                    &mut self.cost.candidate_derivation_store.derivation_id_rows
                }
                SourceFoundationClosureDerivationKeySet::Endpoints => {
                    &mut self
                        .cost
                        .candidate_derivation_store
                        .derivation_endpoint_rows
                }
                SourceFoundationClosureDerivationKeySet::EvidencePaths => {
                    &mut self
                        .cost
                        .candidate_derivation_store
                        .derivation_evidence_path_rows
                }
                SourceFoundationClosureDerivationKeySet::ExpectedInputs => {
                    &mut self
                        .cost
                        .candidate_derivation_store
                        .derivation_expected_input_rows
                }
                SourceFoundationClosureDerivationKeySet::Roots => unreachable!(),
            };
            *count = count.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
        Ok(inserted)
    }

    fn remember_candidate_derivation_subject(
        &mut self,
        id: &str,
        subject: &str,
    ) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_derivation_subject(id, subject, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_derivation_store.derivation_subject_rows = self
                .cost
                .candidate_derivation_store
                .derivation_subject_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn remember_candidate_derivation_pair(
        &mut self,
        subject: &str,
        object: &str,
    ) -> Result<bool, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (inserted, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .remember_derivation_pair(subject, object, remaining)?;
        self.include_store_workspace(workspace)?;
        if inserted {
            self.cost.candidate_derivation_store.derivation_pair_rows = self
                .cost
                .candidate_derivation_store
                .derivation_pair_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        } else {
            self.cost
                .candidate_derivation_store
                .derivation_duplicate_pair_rows = self
                .cost
                .candidate_derivation_store
                .derivation_duplicate_pair_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(inserted)
    }

    fn candidate_derivation_color(&mut self, node: &str) -> Result<Option<u8>, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (color, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .derivation_color(node, remaining)?;
        self.include_store_workspace(workspace)?;
        Ok(color)
    }

    fn set_candidate_derivation_color(&mut self, node: &str, color: u8) -> Result<(), ItemRefusal> {
        let remaining = self.remaining_state()?;
        let workspace = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .set_derivation_color(node, color, remaining)?;
        self.include_store_workspace(workspace)
    }

    fn push_candidate_derivation_frame(
        &mut self,
        node: &str,
        leaving: bool,
    ) -> Result<(), ItemRefusal> {
        let frame_state = estimate_string_storage(node)?
            .checked_add(std::mem::size_of::<SourceFoundationClosureDerivationFrame>())
            .ok_or(ItemRefusal::Budget)?;
        let baseline = self.temporary_state_bytes;
        self.reserve_temporary(frame_state)?;
        let result = (|| {
            let frame = SourceFoundationClosureDerivationFrame {
                node: node.to_owned(),
                leaving,
            };
            let remaining = self.remaining_state()?;
            let workspace = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .push_derivation_frame(&frame, remaining)?;
            self.include_store_workspace(workspace)
        })();
        self.release_temporary_since(baseline);
        result
    }

    fn pop_candidate_derivation_frame(
        &mut self,
    ) -> Result<Option<SourceFoundationClosureDerivationFrame>, ItemRefusal> {
        let remaining = self.remaining_state()?;
        let (frame, workspace, row_state) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .pop_derivation_frame(remaining)?;
        self.include_store_workspace(workspace)?;
        if let Some(frame) = frame {
            self.reserve_temporary(row_state)?;
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    fn drain_candidate_derivation_keyset(
        &mut self,
        set: SourceFoundationClosureDerivationKeySet,
        expected_rows: u64,
    ) -> Result<(), ItemRefusal> {
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_derivation_keyset(set, expected_rows)?;
        let mut after: Option<String> = None;
        let mut after_state = 0usize;
        loop {
            let remaining = self.remaining_state()?;
            let (next, workspace, cursor_state) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_derivation_key(set, after.as_deref(), remaining)?;
            self.include_store_workspace(workspace)?;
            let Some(next) = next else {
                if let Some(previous) = after.take() {
                    drop(previous);
                    self.release_temporary_state(after_state)?;
                }
                break;
            };
            self.reserve_temporary(cursor_state)?;
            if let Some(previous) = after.replace(next) {
                drop(previous);
                self.release_temporary_state(after_state)?;
            }
            after_state = cursor_state;
        }
        Ok(())
    }

    fn build_candidate_derivation_expected_inputs(&mut self) -> Result<(), ItemRefusal> {
        let evidence_rows = self
            .cost
            .candidate_derivation_store
            .derivation_evidence_path_rows;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::EvidencePaths,
                evidence_rows,
            )?;
        let mut after_evidence: Option<String> = None;
        let mut after_evidence_state = 0usize;
        loop {
            let remaining = self.remaining_state()?;
            let (evidence, workspace, cursor_state) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_derivation_key(
                    SourceFoundationClosureDerivationKeySet::EvidencePaths,
                    after_evidence.as_deref(),
                    remaining,
                )?;
            self.include_store_workspace(workspace)?;
            let Some(evidence) = evidence else {
                if let Some(previous) = after_evidence.take() {
                    drop(previous);
                    self.release_temporary_state(after_evidence_state)?;
                }
                break;
            };
            self.reserve_temporary(cursor_state)?;
            if let Some(previous) = after_evidence.replace(evidence) {
                drop(previous);
                self.release_temporary_state(after_evidence_state)?;
            }
            after_evidence_state = cursor_state;
            self.remember_candidate_derivation_key(
                SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                after_evidence.as_deref().ok_or(ItemRefusal::Budget)?,
            )?;
        }

        let endpoint_rows = self
            .cost
            .candidate_derivation_store
            .derivation_endpoint_rows;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::Endpoints,
                endpoint_rows,
            )?;
        let mut after_endpoint: Option<String> = None;
        let mut after_endpoint_state = 0usize;
        loop {
            let remaining = self.remaining_state()?;
            let (endpoint, workspace, cursor_state) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_derivation_key(
                    SourceFoundationClosureDerivationKeySet::Endpoints,
                    after_endpoint.as_deref(),
                    remaining,
                )?;
            self.include_store_workspace(workspace)?;
            let Some(endpoint) = endpoint else {
                if let Some(previous) = after_endpoint.take() {
                    drop(previous);
                    self.release_temporary_state(after_endpoint_state)?;
                }
                break;
            };
            self.reserve_temporary(cursor_state)?;
            if let Some(previous) = after_endpoint.replace(endpoint) {
                drop(previous);
                self.release_temporary_state(after_endpoint_state)?;
            }
            after_endpoint_state = cursor_state;
            let endpoint = after_endpoint.as_deref().ok_or(ItemRefusal::Budget)?;
            let (record_path, path_workspace) =
                self.current_record_path_with_state_budget(endpoint)?;
            if let Some(record_path) = record_path {
                self.remember_candidate_derivation_key(
                    SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                    &record_path,
                )?;
                drop(record_path);
            }
            self.release_temporary_state(path_workspace)?;
        }
        self.remember_candidate_derivation_key(
            SourceFoundationClosureDerivationKeySet::ExpectedInputs,
            CLAIM_SCHEMA,
        )?;
        self.remember_candidate_derivation_key(
            SourceFoundationClosureDerivationKeySet::ExpectedInputs,
            DERIVATION_SCHEMA,
        )?;
        Ok(())
    }

    fn check_candidate_derivation_backlinks(
        &mut self,
        temporary_baseline: usize,
    ) -> Result<u64, ItemRefusal> {
        let mut store = self
            .schema_request_store
            .take()
            .ok_or(ItemRefusal::Budget)?;
        let records = self.records;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let limits = self.limits;
        let temporary = temporary_baseline;
        let issues = &mut self.issues;
        let cost = &mut self.cost;
        let retained = &mut self.retained_state_bytes;
        let mut expression_rows = 0u64;
        let result = records.for_each_current_record(&mut |record_id, record| {
            check(deadline, cancelled)?;
            if record.kind != "expression" {
                return Ok(());
            }
            expression_rows = expression_rows.checked_add(1).ok_or(ItemRefusal::Budget)?;
            let record_workspace = crate::record_biblio_cut::decoded_state(&record.value)?
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(256))
                .ok_or(ItemRefusal::Budget)?;
            let references = record
                .value
                .get("derivation_claim_refs")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let reference_count = references
                .iter()
                .filter(|reference| reference.as_str().is_some())
                .count();
            let tree_state = reference_count
                .checked_mul(std::mem::size_of::<String>() + 8 * std::mem::size_of::<usize>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<BTreeSet<String>>()))
                .ok_or(ItemRefusal::Budget)?;
            let actual_state = value_strings_workspace(&record.value, "derivation_claim_refs")?
                .checked_add(tree_state)
                .ok_or(ItemRefusal::Budget)?;
            let active_temporary = temporary
                .checked_add(record_workspace)
                .and_then(|bytes| bytes.checked_add(actual_state))
                .ok_or(ItemRefusal::Budget)?;
            let used = retained
                .checked_add(active_temporary)
                .ok_or(ItemRefusal::Budget)?;
            if used > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure derivation backlink workspace",
                    used: Some(used as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            cost.reserved_state_bytes = cost.reserved_state_bytes.max(used);
            let actual: BTreeSet<String> = value_strings(&record.value, "derivation_claim_refs")
                .into_iter()
                .collect();
            let mut expected_actual = actual.iter();
            let mut same = true;
            let max_store_state = limits
                .max_state_bytes
                .checked_sub(used)
                .ok_or(ItemRefusal::Budget)?;
            let (expected_count, store_workspace) = store.for_each_derivation_subject_claim(
                record_id,
                max_store_state,
                &mut |expected_id, _workspace| {
                    if expected_actual.next().map(String::as_str) != Some(expected_id) {
                        same = false;
                    }
                    Ok(())
                },
            )?;
            let store_used = used
                .checked_add(store_workspace)
                .ok_or(ItemRefusal::Budget)?;
            if store_used > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure derivation backlink workspace",
                    used: Some(store_used as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            cost.reserved_state_bytes = cost.reserved_state_bytes.max(store_used);
            let matches = same
                && expected_count
                    == u64::try_from(actual.len()).map_err(|_| ItemRefusal::Budget)?
                && expected_actual.next().is_none();
            if !matches {
                push_bounded_issue(
                    issues,
                    cost,
                    retained,
                    active_temporary,
                    limits,
                    cancelled,
                    &record.path,
                    "derivation_claim_refs do not close over exact outgoing derivation claims"
                        .to_owned(),
                )?;
            }
            Ok(())
        });
        self.schema_request_store = Some(store);
        result?;
        Ok(expression_rows)
    }

    fn check_derivation(&mut self) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            self.check_derivation_candidate()
        } else {
            self.check_derivation_finite()
        }
    }

    fn check_candidate_derivation_graph(&mut self) -> Result<bool, ItemRefusal> {
        let id_rows = self.cost.candidate_derivation_store.derivation_id_rows;
        self.drain_candidate_derivation_keyset(
            SourceFoundationClosureDerivationKeySet::ClaimIds,
            id_rows,
        )?;
        let remaining = self.remaining_state()?;
        let (root_rows, root_workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .derivation_root_count(remaining)?;
        self.include_store_workspace(root_workspace)?;
        self.cost.candidate_derivation_store.derivation_root_rows = root_rows;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_derivation_keyset(SourceFoundationClosureDerivationKeySet::Roots, root_rows)?;
        let mut after_root: Option<String> = None;
        let mut after_root_state = 0usize;
        let mut cycle = false;
        loop {
            let remaining = self.remaining_state()?;
            let (root, workspace, cursor_state) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_derivation_key(
                    SourceFoundationClosureDerivationKeySet::Roots,
                    after_root.as_deref(),
                    remaining,
                )?;
            self.include_store_workspace(workspace)?;
            let Some(root) = root else {
                if let Some(previous) = after_root.take() {
                    drop(previous);
                    self.release_temporary_state(after_root_state)?;
                }
                break;
            };
            self.reserve_temporary(cursor_state)?;
            if let Some(previous) = after_root.replace(root) {
                drop(previous);
                self.release_temporary_state(after_root_state)?;
            }
            after_root_state = cursor_state;
            let node = after_root.as_deref().ok_or(ItemRefusal::Budget)?;
            if self.candidate_derivation_color(node)?.is_some() {
                continue;
            }
            self.push_candidate_derivation_frame(node, false)?;
            loop {
                let Some(frame) = self.pop_candidate_derivation_frame()? else {
                    break;
                };
                let frame_state = estimate_string_storage(&frame.node)?
                    .checked_add(std::mem::size_of::<SourceFoundationClosureDerivationFrame>())
                    .ok_or(ItemRefusal::Budget)?;
                let frame_result: Result<(), ItemRefusal> = (|| {
                    if frame.leaving {
                        self.set_candidate_derivation_color(&frame.node, 2)?;
                        return Ok(());
                    }
                    match self.candidate_derivation_color(&frame.node)? {
                        Some(1) => {
                            cycle = true;
                            return Ok(());
                        }
                        Some(2) => return Ok(()),
                        None => {}
                        _ => return Err(ItemRefusal::Budget),
                    }
                    self.set_candidate_derivation_color(&frame.node, 1)?;
                    self.push_candidate_derivation_frame(&frame.node, true)?;
                    let mut after_child: Option<String> = None;
                    let mut after_child_state = 0usize;
                    loop {
                        let remaining = self.remaining_state()?;
                        let (child, workspace, cursor_state) = self
                            .schema_request_store
                            .as_deref_mut()
                            .ok_or(ItemRefusal::Budget)?
                            .next_derivation_child(
                                &frame.node,
                                after_child.as_deref(),
                                remaining,
                            )?;
                        self.include_store_workspace(workspace)?;
                        let Some(child) = child else {
                            if let Some(previous) = after_child.take() {
                                drop(previous);
                                self.release_temporary_state(after_child_state)?;
                            }
                            break;
                        };
                        self.reserve_temporary(cursor_state)?;
                        match self.candidate_derivation_color(&child)? {
                            Some(1) => cycle = true,
                            Some(2) => {}
                            None => self.push_candidate_derivation_frame(&child, false)?,
                            _ => return Err(ItemRefusal::Budget),
                        }
                        if let Some(previous) = after_child.replace(child) {
                            drop(previous);
                            self.release_temporary_state(after_child_state)?;
                        }
                        after_child_state = cursor_state;
                    }
                    Ok(())
                })();
                drop(frame);
                self.release_temporary_state(frame_state)?;
                frame_result?;
            }
        }
        Ok(cycle)
    }

    fn check_derivation_candidate(&mut self) -> Result<(), ItemRefusal> {
        let claim_path = DERIVATION_CLAIMS;
        let claim_paths = self.collect_current_paths(
            |path| path.ends_with("/expression-derivation-claims.jsonl"),
            "source-foundation closure derivation path index",
        )?;
        if claim_paths.len() != 1 || claim_paths.first().map(String::as_str) != Some(claim_path) {
            self.issue(
                claim_path,
                "Expression-derivation claim basename must exist only at its owned route",
            )?;
        }
        let Some(loaded) = self.json_rows(claim_path, CLAIM_SCHEMA, true)? else {
            let id_rows = self.cost.candidate_derivation_store.derivation_id_rows;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::ClaimIds,
                id_rows,
            )?;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::Endpoints,
                0,
            )?;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::EvidencePaths,
                0,
            )?;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                0,
            )?;
            let remaining = self.remaining_state()?;
            let (root_rows, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .derivation_root_count(remaining)?;
            self.include_store_workspace(workspace)?;
            self.cost.candidate_derivation_store.derivation_root_rows = root_rows;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::Roots,
                root_rows,
            )?;
            let remaining = self.remaining_state()?;
            let finished = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .finish_derivation(0, remaining)?;
            self.include_store_workspace(finished.peak_workspace_state_bytes)?;
            self.cost.candidate_derivation_store = finished;
            return Ok(());
        };

        let loaded_state_bytes = loaded.temporary_state_bytes;
        let digest_state_bytes = estimate_string_storage(&loaded.digest)?;
        self.reserve_temporary(digest_state_bytes)?;
        let claim_digest = loaded.digest.clone();
        let mut revision_count = 0u64;
        let mut collated_count = 0u64;
        let mut reviewed_count = 0u64;

        for (line, claim) in &loaded.rows {
            let location_state = estimate_string_storage(claim_path)?
                .checked_add(
                    2usize
                        .checked_mul(1 + usize::MAX.ilog10() as usize + 1)
                        .ok_or(ItemRefusal::Budget)?,
                )
                .ok_or(ItemRefusal::Budget)?;
            self.reserve_temporary(location_state)?;
            let location = format!("{claim_path}:{line}");
            let Some(claim_id) = text(claim, "claim_id") else {
                self.issue(&location, "Expression derivation claim_id is missing")?;
                drop(location);
                self.release_temporary_state(location_state)?;
                continue;
            };
            if self.contains_claim_id(claim_id)? {
                let remaining = self.remaining_state()?;
                let (in_derivation, workspace) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .contains_derivation_id(claim_id, remaining)?;
                self.include_store_workspace(workspace)?;
                if !in_derivation {
                    self.issue(&location, format!("duplicate claim_id: {claim_id}"))?;
                }
            }
            let subject_ref = text(claim, "subject_ref").unwrap_or_default();
            let object_ref = text(claim, "object").unwrap_or_default();
            if text(claim, "claim_type") != Some("relation") {
                self.issue(
                    &location,
                    "Expression derivation claim_type must be relation",
                )?;
            }
            if text(claim, "assertion_layer") != Some("bibliographic_assertion") {
                self.issue(&location, "Expression derivation must remain bibliographic")?;
            }
            if text(claim, "predicate") != Some("is_derivative_of") {
                self.issue(&location, "Expression derivation predicate drifted")?;
            }
            self.expect_ref(&location, Some(subject_ref), "expression")?;
            self.expect_ref(&location, Some(object_ref), "expression")?;
            if subject_ref == object_ref {
                self.issue(&location, "Expression derivation is irreflexive")?;
            }
            let temporary_baseline = self.temporary_state_bytes;
            let same_work: Result<bool, ItemRefusal> = (|| {
                let (subject_record, subject_workspace) =
                    self.current_record_with_state_budget(subject_ref)?;
                let subject_present = subject_record.is_some();
                let subject_work_ref_value = subject_record
                    .as_ref()
                    .and_then(|subject| text(&subject.value, "work_ref"));
                let mut subject_scalar_workspace = std::mem::size_of::<Option<String>>()
                    .checked_add(2 * std::mem::size_of::<usize>())
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<bool>()))
                    .ok_or(ItemRefusal::Budget)?;
                if let Some(work_ref) = subject_work_ref_value {
                    subject_scalar_workspace = subject_scalar_workspace
                        .checked_add(estimate_string_storage(work_ref)?)
                        .ok_or(ItemRefusal::Budget)?;
                }
                self.reserve_temporary(subject_scalar_workspace)?;
                let subject_work_ref = subject_work_ref_value.map(str::to_owned);
                drop(subject_record);
                self.release_temporary_state(subject_workspace)?;

                let (object_record, object_workspace) =
                    self.current_record_with_state_budget(object_ref)?;
                let same_work = if !subject_present || object_record.is_none() {
                    true
                } else {
                    subject_work_ref.as_deref()
                        == object_record
                            .as_ref()
                            .and_then(|object| text(&object.value, "work_ref"))
                };
                drop(object_record);
                self.release_temporary_state(object_workspace)?;
                drop(subject_work_ref);
                self.release_temporary_state(subject_scalar_workspace)?;
                Ok(same_work)
            })();
            self.release_temporary_since(temporary_baseline);
            if !same_work? {
                self.issue(
                    &location,
                    "v1 Expression derivation endpoints must realize the same Work",
                )?;
            }
            if !self.remember_candidate_derivation_pair(subject_ref, object_ref)? {
                self.issue(&location, "duplicate Expression-derivation endpoint pair")?;
            }
            self.remember_candidate_derivation_key(
                SourceFoundationClosureDerivationKeySet::Endpoints,
                subject_ref,
            )?;
            self.remember_candidate_derivation_key(
                SourceFoundationClosureDerivationKeySet::Endpoints,
                object_ref,
            )?;
            self.remember_candidate_derivation_subject(claim_id, subject_ref)?;

            let maker_state = estimate_fixed_json_object_storage(&[
                ("maker_type", Some("model")),
                ("agent_ref", Some("model:codex")),
            ])?;
            self.reserve_temporary(maker_state)?;
            let expected_maker =
                serde_json::json!({"maker_type":"model","agent_ref":"model:codex"});
            let maker_matches =
                self.python_equal(claim.get("maker").unwrap_or(&Value::Null), &expected_maker);
            drop(expected_maker);
            self.release_temporary_state(maker_state)?;
            if !maker_matches? {
                self.issue(&location, "Expression derivation maker must be model:codex")?;
            }
            if text(claim, "provenance_event_ref") != Some(DERIVATION_EVENT) {
                self.issue(
                    &location,
                    "Expression derivation cites the wrong provenance event",
                )?;
            }
            if text(claim, "epistemic_status") != Some("reported") {
                self.issue(
                    &location,
                    "current Expression derivation claims must remain reported",
                )?;
            }
            if text(claim, "review_status") != Some("unreviewed")
                || !claim
                    .get("reviews")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
            {
                self.issue(
                    &location,
                    "Expression derivation claims must remain unreviewed",
                )?;
            }
            if text(claim, "visibility") != Some("public_metadata_only") {
                self.issue(
                    &location,
                    "Expression derivation must remain public metadata only",
                )?;
            }
            let qualifiers = claim.get("qualifiers").unwrap_or(&Value::Null);
            self.request_schema(
                &format!("{location}['qualifiers']"),
                DERIVATION_SCHEMA,
                qualifiers,
            )?;
            if text(qualifiers, "derivation_kind") == Some("revision") {
                revision_count += 1;
            }
            if text(qualifiers, "collation_status") != Some("not_collated") {
                collated_count += 1;
            }
            if text(claim, "review_status") != Some("unreviewed") {
                reviewed_count += 1;
            }
            let evidence_rows = claim
                .get("evidence_refs")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            if !evidence_rows
                .iter()
                .filter_map(Value::as_str)
                .any(|reference| reference.starts_with("tos.anchor."))
            {
                self.issue(
                    &location,
                    "Expression derivation lacks exact source-anchor return",
                )?;
            }
            for evidence_ref in evidence_rows.iter().filter_map(Value::as_str) {
                if evidence_ref.starts_with("tos.anchor.") {
                    if !self.anchor_id_exists(evidence_ref)? {
                        self.issue(
                            &location,
                            format!("unresolved derivation anchor: {evidence_ref}"),
                        )?;
                    }
                } else if evidence_ref.starts_with("ToS/") {
                    if !self.current_exists(evidence_ref)? {
                        self.issue(
                            &location,
                            format!("unresolved derivation evidence: {evidence_ref}"),
                        )?;
                    }
                    self.remember_candidate_derivation_key(
                        SourceFoundationClosureDerivationKeySet::EvidencePaths,
                        evidence_ref,
                    )?;
                }
            }
            drop(location);
            self.release_temporary_state(location_state)?;
        }

        let cycle = self.check_candidate_derivation_graph()?;
        if cycle {
            self.issue(claim_path, "Expression derivation cycle detected")?;
        }

        let temporary_baseline = self.temporary_state_bytes;
        let expected_subject_streams =
            self.check_candidate_derivation_backlinks(temporary_baseline)?;

        let event_rows = self.json_rows(DERIVATION_PROVENANCE, PROVENANCE_SCHEMA, true)?;
        if let Some(events) = event_rows {
            let event_state_bytes = events.temporary_state_bytes;
            if events.rows.len() != 1 {
                self.issue(
                    DERIVATION_PROVENANCE,
                    "Expression derivation must have exactly one batch provenance event",
                )?;
            }
            self.build_candidate_derivation_expected_inputs()?;
            if let Some((_, event)) = events.rows.first() {
                let mut agent_refs = event
                    .get("agent_refs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str);
                let agent_refs_match =
                    agent_refs.next() == Some("model:codex") && agent_refs.next().is_none();
                if text(event, "event_id") != Some(DERIVATION_EVENT)
                    || text(event, "event_type") != Some("annotation")
                    || !agent_refs_match
                    || text(event, "status") != Some("completed_with_warnings")
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance posture drifted",
                    )?;
                }
                let method = event.get("method").unwrap_or(&Value::Null);
                if text(method, "maker_type") != Some("model")
                    || text(method, "name")
                        != Some("source-reported-expression-derivation-materialization")
                    || text(method, "version") != Some("1")
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance method identity drifted",
                    )?;
                }
                let output_count = event
                    .get("outputs")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                if output_count != 1
                    || !output_binds(
                        event,
                        claim_path,
                        "unreviewed-source-reported-expression-derivation-claims",
                        &claim_digest,
                    )
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance output digest drifted",
                    )?;
                }

                let inputs = event
                    .get("inputs")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let input_count = inputs.len();
                let actual_input_count = inputs
                    .iter()
                    .filter(|entry| text(entry, "ref").is_some())
                    .count();
                let mut actual_input_state = std::mem::size_of::<BTreeSet<String>>();
                actual_input_state = actual_input_state
                    .checked_add(
                        actual_input_count
                            .checked_mul(
                                std::mem::size_of::<String>() + 8 * std::mem::size_of::<usize>(),
                            )
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)?;
                for reference in inputs.iter().filter_map(|entry| text(entry, "ref")) {
                    actual_input_state = actual_input_state
                        .checked_add(estimate_string_storage(reference)?)
                        .ok_or(ItemRefusal::Budget)?;
                }
                self.reserve_temporary(actual_input_state)?;
                let actual_inputs: BTreeSet<String> = inputs
                    .iter()
                    .filter_map(|entry| text(entry, "ref").map(str::to_owned))
                    .collect();
                let expected_input_rows = self
                    .cost
                    .candidate_derivation_store
                    .derivation_expected_input_rows;
                self.schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .begin_derivation_keyset(
                        SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                        expected_input_rows,
                    )?;
                let mut actual_input_iter = actual_inputs.iter();
                let mut input_sets_match = true;
                let mut after_input: Option<String> = None;
                let mut after_input_state = 0usize;
                loop {
                    let remaining = self.remaining_state()?;
                    let (expected_input, workspace, cursor_state) = self
                        .schema_request_store
                        .as_deref_mut()
                        .ok_or(ItemRefusal::Budget)?
                        .next_derivation_key(
                            SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                            after_input.as_deref(),
                            remaining,
                        )?;
                    self.include_store_workspace(workspace)?;
                    let Some(expected_input) = expected_input else {
                        if let Some(previous) = after_input.take() {
                            drop(previous);
                            self.release_temporary_state(after_input_state)?;
                        }
                        break;
                    };
                    self.reserve_temporary(cursor_state)?;
                    if let Some(previous) = after_input.replace(expected_input) {
                        drop(previous);
                        self.release_temporary_state(after_input_state)?;
                    }
                    after_input_state = cursor_state;
                    if actual_input_iter.next().map(String::as_str) != after_input.as_deref() {
                        input_sets_match = false;
                    }
                }
                let inputs_equal = input_sets_match
                    && actual_input_iter.next().is_none()
                    && input_count
                        == usize::try_from(expected_input_rows).map_err(|_| ItemRefusal::Budget)?;
                drop(actual_input_iter);
                drop(actual_inputs);
                self.release_temporary_state(actual_input_state)?;
                if !inputs_equal {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance inputs differ from exact evidence and endpoints",
                    )?;
                }
                for input in inputs {
                    let Some(reference) = text(input, "ref") else {
                        continue;
                    };
                    let digest = text(input, "sha256").unwrap_or_default();
                    if !self.current_exists(reference)?
                        || !self.recorded_matches(reference, digest)?
                    {
                        self.issue(
                            DERIVATION_PROVENANCE,
                            format!(
                                "Expression-derivation provenance input digest drifted: {reference}"
                            ),
                        )?;
                    }
                }
                let derivation_cost = self.cost.candidate_derivation_store;
                let configuration_state = estimate_fixed_json_object_storage(&[
                    ("expression_identities_materialized", None),
                    ("derivation_claims_materialized", None),
                    ("revision_claims_materialized", None),
                    ("claims_collated", None),
                    ("claims_reviewed", None),
                    ("unsupported_1911_to_1907_edge_created", None),
                    ("unsupported_2007_to_1911_edge_created", None),
                    ("source_text_admitted", None),
                    ("human_review_performed", None),
                    ("equivalence_claims_created", None),
                    ("semantic_claims_created", None),
                    ("canon_promotion_performed", None),
                ])?;
                self.reserve_temporary(configuration_state)?;
                let expected_configuration = serde_json::json!({
                    "expression_identities_materialized": derivation_cost.derivation_endpoint_rows,
                    "derivation_claims_materialized": derivation_cost.derivation_subject_rows,
                    "revision_claims_materialized": revision_count,
                    "claims_collated": collated_count,
                    "claims_reviewed": reviewed_count,
                    "unsupported_1911_to_1907_edge_created": false,
                    "unsupported_2007_to_1911_edge_created": false,
                    "source_text_admitted": false,
                    "human_review_performed": false,
                    "equivalence_claims_created": 0,
                    "semantic_claims_created": 0,
                    "canon_promotion_performed": false,
                });
                let configuration_matches = self.python_equal(
                    event
                        .get("method")
                        .and_then(|method| method.get("configuration"))
                        .unwrap_or(&Value::Null),
                    &expected_configuration,
                );
                drop(expected_configuration);
                self.release_temporary_state(configuration_state)?;
                if !configuration_matches? {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance configuration drifted",
                    )?;
                }
            }
            if events.rows.first().is_none() {
                let expected_input_rows = self
                    .cost
                    .candidate_derivation_store
                    .derivation_expected_input_rows;
                self.drain_candidate_derivation_keyset(
                    SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                    expected_input_rows,
                )?;
            }
            self.release_loaded_rows(event_state_bytes)?;
        } else {
            let evidence_path_rows = self
                .cost
                .candidate_derivation_store
                .derivation_evidence_path_rows;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::EvidencePaths,
                evidence_path_rows,
            )?;
            let endpoint_rows = self
                .cost
                .candidate_derivation_store
                .derivation_endpoint_rows;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::Endpoints,
                endpoint_rows,
            )?;
            let expected_input_rows = self
                .cost
                .candidate_derivation_store
                .derivation_expected_input_rows;
            self.drain_candidate_derivation_keyset(
                SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                expected_input_rows,
            )?;
        }
        self.release_temporary_state(digest_state_bytes)?;
        self.release_loaded_rows(loaded_state_bytes)?;
        let remaining = self.remaining_state()?;
        let finished = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .finish_derivation(expected_subject_streams, remaining)?;
        self.include_store_workspace(finished.peak_workspace_state_bytes)?;
        self.cost.candidate_derivation_store = finished;
        Ok(())
    }

    fn check_derivation_finite(&mut self) -> Result<(), ItemRefusal> {
        let claim_path = DERIVATION_CLAIMS;
        let claim_paths = self.collect_current_paths(
            |path| path.ends_with("/expression-derivation-claims.jsonl"),
            "source-foundation closure derivation path index",
        )?;
        if claim_paths.len() != 1 || claim_paths.first().map(String::as_str) != Some(claim_path) {
            self.issue(
                claim_path,
                "Expression-derivation claim basename must exist only at its owned route",
            )?;
        }
        let Some(loaded) = self.json_rows(claim_path, CLAIM_SCHEMA, true)? else {
            return Ok(());
        };
        let loaded_state_bytes = loaded.temporary_state_bytes;
        let claim_digest = loaded.digest.clone();
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut pairs = BTreeSet::new();
        let mut subjects = BTreeMap::new();
        let mut evidence_paths = BTreeSet::new();
        let mut revision_count = 0u64;
        let mut collated_count = 0u64;
        let mut reviewed_count = 0u64;
        let mut endpoint_refs = BTreeSet::new();

        for (line, claim) in &loaded.rows {
            let location = format!("{claim_path}:{line}");
            let Some(claim_id) = text(claim, "claim_id") else {
                self.issue(&location, "Expression derivation claim_id is missing")?;
                continue;
            };
            if self.contains_claim_id(claim_id)? && !self.derivation.contains_key(claim_id) {
                self.issue(&location, format!("duplicate claim_id: {claim_id}"))?;
            }
            let subject_ref = text(claim, "subject_ref").unwrap_or_default();
            let object_ref = text(claim, "object").unwrap_or_default();
            if text(claim, "claim_type") != Some("relation") {
                self.issue(
                    &location,
                    "Expression derivation claim_type must be relation",
                )?;
            }
            if text(claim, "assertion_layer") != Some("bibliographic_assertion") {
                self.issue(&location, "Expression derivation must remain bibliographic")?;
            }
            if text(claim, "predicate") != Some("is_derivative_of") {
                self.issue(&location, "Expression derivation predicate drifted")?;
            }
            self.expect_ref(&location, Some(subject_ref), "expression")?;
            self.expect_ref(&location, Some(object_ref), "expression")?;
            if subject_ref == object_ref {
                self.issue(&location, "Expression derivation is irreflexive")?;
            }
            let temporary_baseline = self.temporary_state_bytes;
            let same_work: Result<bool, ItemRefusal> = (|| {
                let (subject_record, subject_workspace) =
                    self.current_record_with_state_budget(subject_ref)?;
                let subject_present = subject_record.is_some();
                let subject_work_ref_value = subject_record
                    .as_ref()
                    .and_then(|subject| text(&subject.value, "work_ref"));
                let mut subject_scalar_workspace = std::mem::size_of::<Option<String>>()
                    .checked_add(2 * std::mem::size_of::<usize>())
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<bool>()))
                    .ok_or(ItemRefusal::Budget)?;
                if let Some(work_ref) = subject_work_ref_value {
                    subject_scalar_workspace = subject_scalar_workspace
                        .checked_add(estimate_string_storage(work_ref)?)
                        .ok_or(ItemRefusal::Budget)?;
                }
                self.reserve_temporary(subject_scalar_workspace)?;
                let subject_work_ref = subject_work_ref_value.map(str::to_owned);
                drop(subject_record);
                self.release_temporary_state(subject_workspace)?;

                let (object_record, object_workspace) =
                    self.current_record_with_state_budget(object_ref)?;
                let same_work = if !subject_present || object_record.is_none() {
                    true
                } else {
                    subject_work_ref.as_deref()
                        == object_record
                            .as_ref()
                            .and_then(|object| text(&object.value, "work_ref"))
                };
                drop(object_record);
                self.release_temporary_state(object_workspace)?;
                drop(subject_work_ref);
                self.release_temporary_state(subject_scalar_workspace)?;
                Ok(same_work)
            })();
            self.release_temporary_since(temporary_baseline);
            let same_work = same_work?;
            if !same_work {
                self.issue(
                    &location,
                    "v1 Expression derivation endpoints must realize the same Work",
                )?;
            }
            self.reserve(
                claim_id
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(subject_ref.len().checked_mul(4)?))
                    .and_then(|n| n.checked_add(object_ref.len().checked_mul(3)?))
                    .and_then(|n| {
                        n.checked_add(
                            6 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>(),
                        )
                    })
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            if !pairs.insert((subject_ref.to_owned(), object_ref.to_owned())) {
                self.issue(&location, "duplicate Expression-derivation endpoint pair")?;
            }
            edges
                .entry(subject_ref.to_owned())
                .or_default()
                .insert(object_ref.to_owned());
            endpoint_refs.insert(subject_ref.to_owned());
            endpoint_refs.insert(object_ref.to_owned());
            subjects.insert(claim_id.to_owned(), subject_ref.to_owned());

            if !self.python_equal(
                claim.get("maker").unwrap_or(&Value::Null),
                &serde_json::json!({"maker_type":"model","agent_ref":"model:codex"}),
            )? {
                self.issue(&location, "Expression derivation maker must be model:codex")?;
            }
            if text(claim, "provenance_event_ref") != Some(DERIVATION_EVENT) {
                self.issue(
                    &location,
                    "Expression derivation cites the wrong provenance event",
                )?;
            }
            if text(claim, "epistemic_status") != Some("reported") {
                self.issue(
                    &location,
                    "current Expression derivation claims must remain reported",
                )?;
            }
            if text(claim, "review_status") != Some("unreviewed")
                || !claim
                    .get("reviews")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
            {
                self.issue(
                    &location,
                    "Expression derivation claims must remain unreviewed",
                )?;
            }
            if text(claim, "visibility") != Some("public_metadata_only") {
                self.issue(
                    &location,
                    "Expression derivation must remain public metadata only",
                )?;
            }
            let qualifiers = claim.get("qualifiers").unwrap_or(&Value::Null);
            self.request_schema(
                &format!("{location}['qualifiers']"),
                DERIVATION_SCHEMA,
                qualifiers,
            )?;
            if text(qualifiers, "derivation_kind") == Some("revision") {
                revision_count += 1;
            }
            if text(qualifiers, "collation_status") != Some("not_collated") {
                collated_count += 1;
            }
            if text(claim, "review_status") != Some("unreviewed") {
                reviewed_count += 1;
            }
            let evidence = value_strings(claim, "evidence_refs");
            if !evidence
                .iter()
                .any(|reference| reference.starts_with("tos.anchor."))
            {
                self.issue(
                    &location,
                    "Expression derivation lacks exact source-anchor return",
                )?;
            }
            for evidence_ref in evidence {
                if evidence_ref.starts_with("tos.anchor.") {
                    if !self.anchor_id_exists(&evidence_ref)? {
                        self.issue(
                            &location,
                            format!("unresolved derivation anchor: {evidence_ref}"),
                        )?;
                    }
                } else if evidence_ref.starts_with("ToS/") {
                    if !self.current_exists(&evidence_ref)? {
                        self.issue(
                            &location,
                            format!("unresolved derivation evidence: {evidence_ref}"),
                        )?;
                    }
                    self.reserve(
                        evidence_ref.len()
                            + std::mem::size_of::<String>()
                            + 4 * std::mem::size_of::<usize>(),
                    )?;
                    evidence_paths.insert(evidence_ref);
                }
            }
        }

        let graph_workspace = edges.iter().try_fold(
            std::mem::size_of::<BTreeMap<String, u8>>()
                + std::mem::size_of::<Vec<(String, bool)>>(),
            |state, (subject, objects)| {
                let subject_state = subject
                    .len()
                    .checked_mul(2)
                    .and_then(|n| {
                        n.checked_add(
                            2 * std::mem::size_of::<String>()
                                + std::mem::size_of::<u8>()
                                + std::mem::size_of::<bool>()
                                + 12 * std::mem::size_of::<usize>(),
                        )
                    })
                    .ok_or(ItemRefusal::Budget)?;
                let object_state = objects.iter().try_fold(0usize, |used, object| {
                    used.checked_add(
                        object
                            .len()
                            .checked_add(
                                std::mem::size_of::<String>()
                                    + std::mem::size_of::<(String, bool)>()
                                    + 8 * std::mem::size_of::<usize>(),
                            )
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)
                })?;
                state
                    .checked_add(subject_state)
                    .and_then(|n| n.checked_add(object_state))
                    .ok_or(ItemRefusal::Budget)
            },
        )?;
        self.reserve(graph_workspace)?;
        let cycle = directed_cycle(&edges, self.limits.deadline, self.source.cancellation())?;
        if cycle {
            self.issue(claim_path, "Expression derivation cycle detected")?;
        }

        let inverse_state = subjects.iter().try_fold(
            std::mem::size_of::<BTreeMap<String, BTreeSet<String>>>(),
            |state, (claim_id, subject)| {
                state
                    .checked_add(
                        claim_id
                            .len()
                            .checked_add(subject.len())
                            .and_then(|n| {
                                n.checked_add(
                                    std::mem::size_of::<String>()
                                        + std::mem::size_of::<BTreeSet<String>>()
                                        + 8 * std::mem::size_of::<usize>(),
                                )
                            })
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)
            },
        )?;
        self.reserve(inverse_state)?;
        let mut expected_by_subject = BTreeMap::<String, BTreeSet<String>>::new();
        for (claim_id, subject) in &subjects {
            expected_by_subject
                .entry(subject.clone())
                .or_default()
                .insert(claim_id.clone());
        }
        let records = self.records;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let limits = self.limits;
        let temporary = self.temporary_state_bytes;
        let issues = &mut self.issues;
        let cost = &mut self.cost;
        let retained = &mut self.retained_state_bytes;
        records.for_each_current_record(&mut |record_id, record| {
            check(deadline, cancelled)?;
            if record.kind != "expression" {
                return Ok(());
            }
            let workspace = crate::record_biblio_cut::decoded_state(&record.value)?
                .checked_mul(2)
                .and_then(|n| n.checked_add(256))
                .ok_or(ItemRefusal::Budget)?;
            let temporary = temporary
                .checked_add(workspace)
                .ok_or(ItemRefusal::Budget)?;
            let used = retained.checked_add(temporary).ok_or(ItemRefusal::Budget)?;
            if used > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure derivation backlink workspace",
                    used: Some(used as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            cost.reserved_state_bytes = cost.reserved_state_bytes.max(used);
            let actual = value_strings(&record.value, "derivation_claim_refs")
                .into_iter()
                .collect::<BTreeSet<_>>();
            if !expected_by_subject
                .get(record_id)
                .map_or(actual.is_empty(), |expected| expected == &actual)
            {
                push_bounded_issue(
                    issues,
                    cost,
                    retained,
                    temporary,
                    limits,
                    cancelled,
                    record.path.as_str(),
                    "derivation_claim_refs do not close over exact outgoing derivation claims"
                        .to_owned(),
                )?;
            }
            Ok(())
        })?;

        let event_rows = self.json_rows(DERIVATION_PROVENANCE, PROVENANCE_SCHEMA, true)?;
        if let Some(events) = event_rows {
            let event_state_bytes = events.temporary_state_bytes;
            if events.rows.len() != 1 {
                self.issue(
                    DERIVATION_PROVENANCE,
                    "Expression derivation must have exactly one batch provenance event",
                )?;
            }
            if let Some((_, event)) = events.rows.first() {
                if text(event, "event_id") != Some(DERIVATION_EVENT)
                    || text(event, "event_type") != Some("annotation")
                    || value_strings(event, "agent_refs") != vec!["model:codex".to_owned()]
                    || text(event, "status") != Some("completed_with_warnings")
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance posture drifted",
                    )?;
                }
                let method = event.get("method").unwrap_or(&Value::Null);
                if text(method, "maker_type") != Some("model")
                    || text(method, "name")
                        != Some("source-reported-expression-derivation-materialization")
                    || text(method, "version") != Some("1")
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance method identity drifted",
                    )?;
                }
                let outputs = event
                    .get("outputs")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if outputs.len() != 1
                    || !output_binds(
                        event,
                        claim_path,
                        "unreviewed-source-reported-expression-derivation-claims",
                        &claim_digest,
                    )
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance output digest drifted",
                    )?;
                }

                let mut expected_inputs = evidence_paths;
                let mut endpoint_path_state = 0usize;
                for endpoint in &endpoint_refs {
                    let (record_path, path_workspace) =
                        self.current_record_path_with_state_budget(endpoint)?;
                    if let Some(record_path) = record_path {
                        if expected_inputs.contains(&record_path) {
                            drop(record_path);
                            self.release_temporary_state(path_workspace)?;
                        } else {
                            endpoint_path_state = endpoint_path_state
                                .checked_add(path_workspace)
                                .ok_or(ItemRefusal::Budget)?;
                            expected_inputs.insert(record_path);
                        }
                    }
                }
                expected_inputs.insert(CLAIM_SCHEMA.to_owned());
                expected_inputs.insert(DERIVATION_SCHEMA.to_owned());
                let actual_inputs: BTreeSet<String> = event
                    .get("inputs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| text(entry, "ref").map(str::to_owned))
                    .collect();
                let input_count = event
                    .get("inputs")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                if actual_inputs != expected_inputs || input_count != expected_inputs.len() {
                    self.issue(DERIVATION_PROVENANCE, "Expression-derivation provenance inputs differ from exact evidence and endpoints")?;
                }
                for input in event
                    .get("inputs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(reference) = text(input, "ref") else {
                        continue;
                    };
                    let digest = text(input, "sha256").unwrap_or_default();
                    if !self.current_exists(reference)?
                        || !self.recorded_matches(reference, digest)?
                    {
                        self.issue(
                            DERIVATION_PROVENANCE,
                            format!(
                                "Expression-derivation provenance input digest drifted: {reference}"
                            ),
                        )?;
                    }
                }
                drop(expected_inputs);
                self.release_temporary_state(endpoint_path_state)?;
                let expected_configuration = serde_json::json!({
                    "expression_identities_materialized": endpoint_refs.len(),
                    "derivation_claims_materialized": subjects.len(),
                    "revision_claims_materialized": revision_count,
                    "claims_collated": collated_count,
                    "claims_reviewed": reviewed_count,
                    "unsupported_1911_to_1907_edge_created": false,
                    "unsupported_2007_to_1911_edge_created": false,
                    "source_text_admitted": false,
                    "human_review_performed": false,
                    "equivalence_claims_created": 0,
                    "semantic_claims_created": 0,
                    "canon_promotion_performed": false,
                });
                if !self.python_equal(
                    event
                        .get("method")
                        .and_then(|method| method.get("configuration"))
                        .unwrap_or(&Value::Null),
                    &expected_configuration,
                )? {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance configuration drifted",
                    )?;
                }
            }
            self.release_loaded_rows(event_state_bytes)?;
        }
        self.release_loaded_rows(loaded_state_bytes)?;
        Ok(())
    }

    fn check_responsibility_claims(&mut self) -> Result<(), ItemRefusal> {
        let temporary_baseline = self.temporary_state_bytes;
        let mut validated_events = BTreeSet::new();
        if self.schema_request_store.is_some() {
            let expected_rows = self.responsibility_claim_count;
            self.schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .begin_responsibility_claims(expected_rows)?;
            let mut cursor_state_bytes = 0usize;
            loop {
                let remaining = self.remaining_state()?;
                let (claim, workspace, row_state_bytes, retained_cursor_state_bytes) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .next_responsibility_claim(remaining)?;
                self.include_store_workspace(workspace)?;
                self.release_loaded_rows(cursor_state_bytes)?;
                let Some(claim) = claim else {
                    if row_state_bytes != 0 || retained_cursor_state_bytes != 0 {
                        return Err(ItemRefusal::Budget);
                    }
                    break;
                };
                let active_state_bytes = row_state_bytes
                    .checked_add(retained_cursor_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                if active_state_bytes > remaining {
                    return Err(ItemRefusal::Budget);
                }
                self.reserve_temporary(active_state_bytes)?;
                cursor_state_bytes = retained_cursor_state_bytes;
                let result = self.check_responsibility_claim(&claim, &mut validated_events);
                drop(claim);
                self.release_temporary_state(row_state_bytes)?;
                result?;
            }
        } else {
            let clone_state = claim_refs_vec_clone_state(&self.responsibility)?;
            self.reserve_temporary(clone_state)?;
            let claims: Vec<SourceFoundationClosureClaimRef> =
                self.responsibility.values().cloned().collect();
            for claim in claims {
                self.check_responsibility_claim(&claim, &mut validated_events)?;
            }
        }
        self.release_temporary_since(temporary_baseline);
        Ok(())
    }

    fn check_responsibility_claim(
        &mut self,
        claim: &SourceFoundationClosureClaimRef,
        validated_events: &mut BTreeSet<String>,
    ) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let predicate_allowed = matches!(
            claim.predicate.as_str(),
            "authored_by"
                | "contributed_by"
                | "translated_by"
                | "edited_by"
                | "afterword_by"
                | "designed_by"
        );
        if !predicate_allowed {
            self.issue(
                &claim.location,
                format!("unsupported responsibility predicate: {}", claim.predicate),
            )?;
        }
        let expected_kind = match claim.predicate.as_str() {
            "authored_by" | "contributed_by" => Some("work"),
            "translated_by" => Some("expression"),
            "edited_by" | "afterword_by" | "designed_by" => Some("edition"),
            _ => None,
        };
        if let Some(expected_kind) = expected_kind {
            self.expect_ref(&claim.location, Some(&claim.subject), expected_kind)?;
        }
        self.expect_ref(&claim.location, Some(&claim.object), "agent")?;
        if claim.native {
            return Ok(());
        }
        let Some((claim_path, line_text)) = claim.location.rsplit_once(':') else {
            return Ok(());
        };
        let line = line_text.parse::<usize>().unwrap_or_default();
        let (value, value_state_bytes) = self.loaded_value_at(claim_path, line)?;
        let Some(value) = value else {
            return Ok(());
        };
        if text(&value, "claim_type") != Some("bibliographic") {
            self.issue(
                &claim.location,
                "responsibility claim claim_type must be bibliographic",
            )?;
        }
        if !matches!(
            text(&value, "assertion_layer"),
            Some("bibliographic_assertion" | "scholarly_report")
        ) {
            self.issue(&claim.location, "responsibility claim assertion_layer must be bibliographic_assertion or scholarly_report")?;
        }
        let (event, event_state_bytes) = self.event(&claim.event)?;
        let Some(event) = event.map(Cow::into_owned) else {
            self.issue(
                &claim.location,
                format!("unresolved provenance_event_ref: {}", claim.event),
            )?;
            self.release_loaded_rows(event_state_bytes)?;
            self.release_loaded_rows(value_state_bytes)?;
            return Ok(());
        };
        let Some(digest) = self.digest_for(claim_path)? else {
            self.issue(
                &claim.location,
                "responsibility Claim file is absent from the current cut",
            )?;
            self.release_loaded_rows(event_state_bytes)?;
            self.release_loaded_rows(value_state_bytes)?;
            return Ok(());
        };
        let output_role = event
            .get("outputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|output| {
                text(output, "ref") == Some(claim_path)
                    && text(output, "sha256") == Some(digest.as_str())
                    && matches!(
                        text(output, "role"),
                        Some(
                            "unreviewed-translation-responsibility-claims"
                                | "unreviewed-evidence-bearing-responsibility-claims"
                        )
                    )
            });
        if !output_role {
            self.issue(
                &claim.location,
                "responsibility claim provenance event does not digest-bind the claim file",
            )?;
        }
        if self.remember_responsibility_validated_event(&claim.event, validated_events)? {
            self.check_event_input_bindings(
                &claim.location,
                &event,
                "responsibility claim provenance input",
            )?;
        }
        self.release_loaded_rows(event_state_bytes)?;
        self.release_loaded_rows(value_state_bytes)?;
        Ok(())
    }

    fn check_publication_claims(&mut self) -> Result<(), ItemRefusal> {
        let temporary_baseline = self.temporary_state_bytes;
        let mut validated_events = BTreeSet::new();
        if self.schema_request_store.is_some() {
            let expected_rows = self.cost.candidate_publication_claim_count;
            self.schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .begin_publication_claims(expected_rows)?;
            let mut cursor_state_bytes = 0usize;
            loop {
                let remaining = self.remaining_state()?;
                let (claim, workspace, row_state_bytes, retained_cursor_state_bytes) = self
                    .schema_request_store
                    .as_deref_mut()
                    .ok_or(ItemRefusal::Budget)?
                    .next_publication_claim(remaining)?;
                self.include_store_workspace(workspace)?;
                self.release_loaded_rows(cursor_state_bytes)?;
                let Some(claim) = claim else {
                    if row_state_bytes != 0 || retained_cursor_state_bytes != 0 {
                        return Err(ItemRefusal::Budget);
                    }
                    break;
                };
                let active_state_bytes = row_state_bytes
                    .checked_add(retained_cursor_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                if active_state_bytes > remaining {
                    return Err(ItemRefusal::Budget);
                }
                self.reserve_temporary(active_state_bytes)?;
                cursor_state_bytes = retained_cursor_state_bytes;
                let result = self.check_publication_claim(&claim.reference, &mut validated_events);
                drop(claim);
                self.release_temporary_state(row_state_bytes)?;
                result?;
            }
        } else {
            let clone_state = claim_refs_vec_clone_state(&self.publication)?;
            self.reserve_temporary(clone_state)?;
            let claims: Vec<SourceFoundationClosureClaimRef> =
                self.publication.values().cloned().collect();
            for claim in claims {
                self.check_publication_claim(&claim, &mut validated_events)?;
            }
        }
        self.release_temporary_since(temporary_baseline);
        Ok(())
    }

    fn check_publication_claim(
        &mut self,
        claim: &SourceFoundationClosureClaimRef,
        validated_events: &mut BTreeSet<String>,
    ) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if !self.current_record_exists_with_state_budget(&claim.subject)? {
            return Ok(());
        }
        let owner_matches =
            self.owner_record_matches_subject_at_location(&claim.location, &claim.subject)?;
        if claim.native {
            return Ok(());
        }
        let Some((claim_path, claim_line)) = claim.location.rsplit_once(':') else {
            return Ok(());
        };
        let location = format!("{claim_path}:{claim_line}");
        let line = claim_line.parse::<usize>().unwrap_or_default();
        let (value, value_state_bytes) = self.loaded_value_at(claim_path, line)?;
        let Some(value) = value else {
            return Ok(());
        };
        if text(&value, "claim_type") != Some("bibliographic") {
            self.issue(
                &location,
                "publication claim claim_type must be bibliographic",
            )?;
        }
        if !matches!(
            text(&value, "assertion_layer"),
            Some("bibliographic_assertion" | "scholarly_report")
        ) {
            self.issue(&location, "publication claim assertion_layer must be bibliographic_assertion or scholarly_report")?;
        }
        if !owner_matches {
            self.issue(
                &location,
                "publication claim subject_ref differs from sibling edition.json",
            )?;
        }
        if claim.object.starts_with("tos.")
            && !self.current_record_exists_with_state_budget(&claim.object)?
            && !self.link_exists(&claim.object)?
            && !self.event_exists(&claim.object)?
            && !self.records.rights_contains(&claim.object)?
        {
            self.issue(
                &location,
                format!("unresolved publication claim object: {}", claim.object),
            )?;
        }
        let (event, event_state_bytes) = self.event(&claim.event)?;
        let Some(event) = event.map(Cow::into_owned) else {
            self.issue(
                &location,
                format!("unresolved provenance_event_ref: {}", claim.event),
            )?;
            self.release_loaded_rows(event_state_bytes)?;
            self.release_loaded_rows(value_state_bytes)?;
            return Ok(());
        };
        let Some(digest) = self.digest_for(claim_path)? else {
            self.issue(
                &location,
                "publication Claim file is absent from the current cut",
            )?;
            self.release_loaded_rows(event_state_bytes)?;
            self.release_loaded_rows(value_state_bytes)?;
            return Ok(());
        };
        if !output_binds(
            &event,
            claim_path,
            "unreviewed-evidence-bearing-publication-claims",
            &digest,
        ) {
            self.issue(
                &location,
                "publication claim provenance event does not digest-bind the claim file",
            )?;
        }
        if self.remember_publication_validated_event(&claim.event, validated_events)? {
            self.check_event_input_bindings(
                &location,
                &event,
                "publication claim provenance input",
            )?;
        }
        self.release_loaded_rows(event_state_bytes)?;
        self.release_loaded_rows(value_state_bytes)?;
        Ok(())
    }

    fn check_provision_activity(&mut self) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            self.check_candidate_provision_activity()
        } else {
            self.check_provision_activity_finite()
        }
    }

    fn check_candidate_provision_activity(&mut self) -> Result<(), ItemRefusal> {
        let temporary_baseline = self.temporary_state_bytes;
        let compatibility_headers = 2usize
            .checked_mul(std::mem::size_of::<BTreeSet<String>>())
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_temporary(compatibility_headers)?;
        let mut validated_events = BTreeSet::new();
        let mut used_events = BTreeSet::new();

        let expected_claim_rows = self.cost.candidate_provision_claim_count;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_provision_claims(expected_claim_rows)?;
        let mut cursor_state_bytes = 0usize;
        loop {
            check(self.limits.deadline, self.source.cancellation())?;
            let remaining = self.remaining_state()?;
            let (claim, workspace, row_state_bytes, retained_cursor_state_bytes) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_provision_claim(remaining)?;
            self.release_loaded_rows(cursor_state_bytes)?;
            let Some(claim) = claim else {
                if row_state_bytes != 0 || retained_cursor_state_bytes != 0 {
                    return Err(ItemRefusal::Budget);
                }
                self.include_store_workspace(workspace)?;
                break;
            };
            let active_state_bytes = row_state_bytes
                .checked_add(retained_cursor_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            self.reserve_temporary(active_state_bytes)?;
            cursor_state_bytes = retained_cursor_state_bytes;
            self.include_store_workspace(workspace)?;

            if claim.reference.native {
                check(self.limits.deadline, self.source.cancellation())?;
                drop(claim);
                self.release_temporary_state(row_state_bytes)?;
                continue;
            }
            let Some((claim_path, line_text)) = claim.reference.location.rsplit_once(':') else {
                return Err(ItemRefusal::Source(
                    "source-foundation Provision ClaimRef location is invalid".into(),
                ));
            };
            let line = line_text.parse::<usize>().map_err(|_| {
                ItemRefusal::Source("source-foundation Provision ClaimRef line is invalid".into())
            })?;
            let (claim_value, claim_value_state_bytes) = self.loaded_value_at(claim_path, line)?;
            let Some(claim_value) = claim_value else {
                return Err(ItemRefusal::Source(
                    "source-foundation Provision ClaimRef has no matching current row".into(),
                ));
            };
            if text(&claim_value, "claim_id") != Some(claim.id.as_str())
                || text(&claim_value, "subject_ref").unwrap_or_default() != claim.reference.subject
                || text(&claim_value, "predicate").unwrap_or_default() != claim.reference.predicate
                || text(&claim_value, "object").unwrap_or_default() != claim.reference.object
                || text(&claim_value, "provenance_event_ref").unwrap_or_default()
                    != claim.reference.event
            {
                drop(claim_value);
                self.release_loaded_rows(claim_value_state_bytes)?;
                return Err(ItemRefusal::Source(
                    "source-foundation Provision ClaimRef differs from its current source row"
                        .into(),
                ));
            }
            let result = self.check_provision_activity_claim(
                &claim.reference,
                &claim_value,
                &mut used_events,
                &mut validated_events,
            );
            drop(claim_value);
            let value_release = self.release_loaded_rows(claim_value_state_bytes);
            drop(claim);
            let row_release = self.release_temporary_state(row_state_bytes);
            result?;
            value_release?;
            row_release?;
        }
        drop(used_events);
        drop(validated_events);
        self.release_temporary_state(compatibility_headers)?;
        self.release_loaded_rows(cursor_state_bytes)?;

        let expected_event_rows = self.cost.candidate_provision_event_id_count;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_provision_event_ids(expected_event_rows)?;
        let mut event_cursor_state_bytes = 0usize;
        let mut unused_event_count = 0u64;
        let mut unused_event_source_bytes = 0usize;
        let mut unused_message: Option<String> = None;
        let mut unused_message_state_bytes = 0usize;
        let issue_prefix = "provision-activity provenance events are not referenced by claims: ";
        loop {
            check(self.limits.deadline, self.source.cancellation())?;
            let remaining = self.remaining_state()?;
            let (event_id, workspace, row_state_bytes, retained_cursor_state_bytes) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_provision_event_id(remaining)?;
            self.release_loaded_rows(event_cursor_state_bytes)?;
            let Some(event_id) = event_id else {
                if row_state_bytes != 0 || retained_cursor_state_bytes != 0 {
                    return Err(ItemRefusal::Budget);
                }
                self.include_store_workspace(workspace)?;
                break;
            };
            let active_state_bytes = row_state_bytes
                .checked_add(retained_cursor_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            self.reserve_temporary(active_state_bytes)?;
            event_cursor_state_bytes = retained_cursor_state_bytes;
            self.include_store_workspace(workspace)?;

            let is_used = self.contains_candidate_provision_used_event(&event_id)?;
            if !is_used {
                unused_event_count = unused_event_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
                unused_event_source_bytes = unused_event_source_bytes
                    .checked_add(event_id.len())
                    .ok_or(ItemRefusal::Budget)?;
                self.cost.candidate_provision_unused_event_count = unused_event_count;
                let unused_event_count =
                    usize::try_from(unused_event_count).map_err(|_| ItemRefusal::Budget)?;
                let (_, list_bytes, _) =
                    python_string_list_workspace(unused_event_count, unused_event_source_bytes)?;
                let capacity = issue_prefix
                    .len()
                    .checked_add(list_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                let next_message_state_bytes = bounded_string_capacity_state(capacity)?;
                if next_message_state_bytes > unused_message_state_bytes {
                    self.reserve_temporary(next_message_state_bytes - unused_message_state_bytes)?;
                    unused_message_state_bytes = next_message_state_bytes;
                }
                let message = unused_message.get_or_insert_with(|| {
                    let mut message = String::with_capacity(capacity);
                    message.push_str(issue_prefix);
                    message.push('[');
                    message
                });
                if unused_event_count > 1 {
                    message.push_str(", ");
                }
                push_python_string_repr(message, &event_id);
            }
            drop(event_id);
            self.release_temporary_state(row_state_bytes)?;
        }
        self.release_loaded_rows(event_cursor_state_bytes)?;

        let expected_used_rows = self.cost.candidate_provision_used_event_count;
        let expected_validated_rows = self.cost.candidate_provision_validated_event_count;
        let remaining = self.remaining_state()?;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .finish_provision(
                expected_claim_rows,
                expected_event_rows,
                expected_used_rows,
                expected_validated_rows,
                unused_event_count,
                remaining,
            )?;
        let store_cost = self
            .schema_request_store
            .as_deref()
            .ok_or(ItemRefusal::Budget)?
            .cost();
        if store_cost.provision_claim_rows != expected_claim_rows
            || store_cost.provision_claim_drained_rows != expected_claim_rows
            || !store_cost.provision_claim_eof_seen
            || !store_cost.provision_claim_count_verified
            || store_cost.provision_event_id_rows != expected_event_rows
            || store_cost.provision_event_id_drained_rows != expected_event_rows
            || store_cost.provision_event_id_lookup_rows != expected_event_rows
            || !store_cost.provision_event_id_eof_seen
            || !store_cost.provision_event_id_count_verified
            || store_cost.provision_unused_event_rows != unused_event_count
            || store_cost.provision_unused_event_drained_rows != unused_event_count
            || !store_cost.provision_unused_event_eof_seen
            || !store_cost.provision_unused_event_count_verified
            || store_cost.provision_used_event_rows != expected_used_rows
            || !store_cost.provision_used_event_count_verified
            || store_cost.provision_validated_event_rows != expected_validated_rows
            || !store_cost.provision_validated_event_count_verified
        {
            return Err(ItemRefusal::Source(
                "source-foundation Provision store count or EOF differs".into(),
            ));
        }
        self.cost.candidate_provision_claim_drained_rows = store_cost.provision_claim_drained_rows;
        self.cost.candidate_provision_claim_eof_seen = store_cost.provision_claim_eof_seen;
        self.cost.candidate_provision_claim_count_verified =
            store_cost.provision_claim_count_verified;
        self.cost.candidate_provision_event_id_drained_rows =
            store_cost.provision_event_id_drained_rows;
        self.cost.candidate_provision_event_id_lookup_rows =
            store_cost.provision_event_id_lookup_rows;
        self.cost.candidate_provision_event_id_eof_seen = store_cost.provision_event_id_eof_seen;
        self.cost.candidate_provision_event_id_count_verified =
            store_cost.provision_event_id_count_verified;
        self.cost
            .candidate_provision_unused_event_serialized_read_bytes =
            store_cost.provision_unused_event_serialized_read_bytes;
        self.cost
            .candidate_provision_unused_event_scan_row_operations =
            store_cost.provision_unused_event_scan_row_operations;
        self.cost
            .candidate_provision_unused_event_peak_workspace_state_bytes =
            store_cost.provision_unused_event_workspace_state_bytes;
        self.cost.candidate_provision_unused_event_drained_rows =
            store_cost.provision_unused_event_drained_rows;
        self.cost.candidate_provision_unused_event_eof_seen =
            store_cost.provision_unused_event_eof_seen;
        self.cost.candidate_provision_unused_event_count_verified =
            store_cost.provision_unused_event_count_verified;

        if let Some(mut message) = unused_message {
            message.push(']');
            let message_state = unused_message_state_bytes;
            let location_state = estimate_string_storage(SOURCE_HOME)?;
            self.reserve_temporary(location_state)?;
            let issue = self.issue(SOURCE_HOME, message);
            let location_release = self.release_temporary_state(location_state);
            let message_release = self.release_temporary_state(message_state);
            issue?;
            location_release?;
            message_release?;
        }
        self.release_temporary_since(temporary_baseline);
        Ok(())
    }

    fn check_provision_activity_finite(&mut self) -> Result<(), ItemRefusal> {
        let mut clone_state =
            std::mem::size_of::<Vec<(String, SourceFoundationClosureClaimRef, Value)>>();
        for (id, reference) in &self.provision {
            let Some(value) = self.provision_values.get(id) else {
                continue;
            };
            clone_state = clone_state
                .checked_add(id.len())
                .and_then(|n| n.checked_add(claim_reference_payload_state(reference).ok()?))
                .and_then(|n| n.checked_add(crate::record_biblio_cut::decoded_state(value).ok()?))
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<(String, SourceFoundationClosureClaimRef, Value)>()
                            + 8 * std::mem::size_of::<usize>(),
                    )
                })
                .ok_or(ItemRefusal::Budget)?;
        }
        let temporary_baseline = self.temporary_state_bytes;
        self.reserve_temporary(clone_state)?;
        let claims: Vec<(String, SourceFoundationClosureClaimRef, Value)> = self
            .provision
            .iter()
            .filter_map(|(id, reference)| {
                self.provision_values
                    .get(id)
                    .map(|value| (id.clone(), reference.clone(), value.clone()))
            })
            .collect();
        let mut validated_events = BTreeSet::new();
        let mut used_events = BTreeSet::new();
        for (_id, reference, claim) in claims {
            self.check_provision_activity_claim(
                &reference,
                &claim,
                &mut used_events,
                &mut validated_events,
            )?;
        }
        let unused: Vec<String> = self
            .provision_event_ids
            .difference(&used_events)
            .cloned()
            .collect();
        if !unused.is_empty() {
            self.issue(
                SOURCE_HOME,
                format!(
                    "provision-activity provenance events are not referenced by claims: {}",
                    python_string_list(&unused)
                ),
            )?;
        }
        self.release_temporary_since(temporary_baseline);
        Ok(())
    }

    fn check_provision_activity_claim(
        &mut self,
        reference: &SourceFoundationClosureClaimRef,
        claim: &Value,
        used_events: &mut BTreeSet<String>,
        validated_events: &mut BTreeSet<String>,
    ) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if reference.native {
            return Ok(());
        }
        let location = reference.location.as_str();
        if text(&claim, "claim_type") != Some("bibliographic") {
            self.issue(
                location,
                "provision-activity claim_type must be bibliographic",
            )?;
        }
        if text(&claim, "assertion_layer") != Some("bibliographic_assertion") {
            self.issue(
                location,
                "provision-activity assertion_layer must be bibliographic_assertion",
            )?;
        }
        if text(&claim, "predicate") != Some("provision_activity") {
            self.issue(
                location,
                "provision-activity predicate must be provision_activity",
            )?;
        }
        if !self
            .owner_record_matches_subject_at_location(&reference.location, &reference.subject)?
        {
            self.issue(
                location,
                "provision-activity subject_ref differs from sibling edition.json",
            )?;
        }

        let Some(activity) = claim.get("object") else {
            self.issue(location, "provision-activity object must be an object")?;
            return Ok(());
        };
        self.request_schema(&format!("{location}#object"), PROVISION_SCHEMA, activity)?;
        if let Some(temporal) = activity.get("temporal") {
            if text(temporal, "kind") == Some("interval") {
                if let (Some(start), Some(end)) = (text(temporal, "start"), text(temporal, "end")) {
                    if start > end {
                        self.issue(location, "provision-activity interval starts after it ends")?;
                    }
                }
            }
        }
        let kind = text(activity, "provision_kind").unwrap_or_default();
        let (place_role, agent_roles): (&str, &[&str]) = match kind {
            "publication" => ("publication_place", &["publisher"]),
            "production" => ("production_place", &["producer"]),
            "distribution" => ("distribution_place", &["distributor"]),
            "manufacture" => ("manufacture_place", &["manufacturer", "printer"]),
            _ => ("", &[]),
        };
        for place in activity
            .get("places")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(role) = text(place, "role") {
                if !place_role.is_empty() && role != place_role {
                    self.issue(
                        location,
                        format!("{kind} provision has incompatible place role: {role}"),
                    )?;
                }
            }
            if let Some(reference) = text(place, "normalized_place_ref") {
                self.expect_ref(location, Some(reference), "place")?;
            }
        }
        for agent in activity
            .get("agents")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(role) = text(agent, "role") {
                if !agent_roles.is_empty() && !agent_roles.contains(&role) {
                    self.issue(
                        location,
                        format!("{kind} provision has incompatible agent role: {role}"),
                    )?;
                }
            }
            if let Some(reference) = text(agent, "normalized_agent_ref") {
                let (record, record_workspace) =
                    self.current_record_with_state_budget(reference)?;
                match record.as_ref() {
                    None => self.issue(
                        location,
                        format!("unresolved provision agent reference: {reference}"),
                    )?,
                    Some(record) if !matches!(record.kind.as_str(), "agent" | "organization") => {
                        self.issue(
                            location,
                            format!(
                                "{reference} resolves to {}, expected agent or organization",
                                record.kind
                            ),
                        )?
                    }
                    Some(_) => {}
                }
                drop(record);
                self.release_temporary_state(record_workspace)?;
            }
        }
        if text(activity, "event_posture") == Some("source_statement_only")
            && activity.get("temporal").is_some_and(Value::is_object)
            && text(activity.get("temporal").unwrap_or(&Value::Null), "role")
                != Some("statement_date")
        {
            self.issue(
                location,
                "source_statement_only provision must keep its temporal role at statement_date",
            )?;
        }

        let (event, event_state_bytes) = self.event(&reference.event)?;
        let Some(event) = event.map(Cow::into_owned) else {
            self.issue(
                location,
                format!(
                    "unresolved provision-activity provenance_event_ref: {}",
                    reference.event
                ),
            )?;
            self.release_loaded_rows(event_state_bytes)?;
            return Ok(());
        };
        let claim_path = location
            .rsplit_once(':')
            .map(|(path, _)| path)
            .unwrap_or(location);
        let Some(digest) = self.digest_for(claim_path)? else {
            self.issue(
                location,
                "provision-activity Claim file is absent from the current cut",
            )?;
            self.release_loaded_rows(event_state_bytes)?;
            return Ok(());
        };
        if !output_binds(
            &event,
            claim_path,
            "unreviewed-evidence-bearing-provision-activity-claims",
            &digest,
        ) {
            self.issue(
                location,
                "provision-activity provenance event does not digest-bind the claim file",
            )?;
        }
        self.remember_provision_used_event(&reference.event, used_events)?;
        if self.remember_provision_validated_event(&reference.event, validated_events)? {
            self.check_event_input_bindings(
                location,
                &event,
                "provision-activity provenance input",
            )?;
        }
        let evidence_state = value_strings_workspace(&claim, "evidence_refs")?;
        self.reserve_temporary(evidence_state)?;
        let evidence_refs = value_strings(&claim, "evidence_refs");
        let evidence_result = (|| {
            for evidence in &evidence_refs {
                if evidence.starts_with("ToS/") && !self.current_exists(evidence)? {
                    self.issue(
                        location,
                        format!("unresolved repository evidence ref: {evidence}"),
                    )?;
                } else if evidence.starts_with("tos.")
                    && !self.current_record_exists_with_state_budget(evidence)?
                    && !self.link_exists(evidence)?
                {
                    self.issue(
                        location,
                        format!("unresolved identity evidence ref: {evidence}"),
                    )?;
                }
            }
            Ok(())
        })();
        drop(evidence_refs);
        let evidence_release = self.release_temporary_state(evidence_state);
        evidence_result?;
        evidence_release?;
        self.release_loaded_rows(event_state_bytes)?;
        Ok(())
    }

    fn check_event_input_bindings(
        &mut self,
        location: &str,
        event: &Value,
        prefix: &str,
    ) -> Result<(), ItemRefusal> {
        for input in event
            .get("inputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(reference) = text(input, "ref") else {
                continue;
            };
            if !reference.starts_with("ToS/") {
                continue;
            }
            let digest = text(input, "sha256").unwrap_or_default();
            if !self.current_exists(reference)? {
                self.issue(location, format!("{prefix} is missing: {reference}"))?;
            } else if !self.recorded_matches(reference, digest)? {
                self.issue(location, format!("{prefix} digest drifted: {reference}"))?;
            }
        }
        Ok(())
    }

    fn check_chronology(&mut self) -> Result<(), ItemRefusal> {
        let paths = self.collect_current_paths(
            |path| path.ends_with("/work-chronology-claims.jsonl"),
            "source-foundation closure chronology path index",
        )?;
        if paths.len() != 1 || paths.first().map(String::as_str) != Some(CHRONOLOGY_CLAIMS) {
            self.issue(
                CHRONOLOGY_CLAIMS,
                "work chronology claim basename must exist only at its owned route",
            )?;
        }
        let Some(claims) = self.json_rows(CHRONOLOGY_CLAIMS, CLAIM_SCHEMA, true)? else {
            return Ok(());
        };
        let claims_state_bytes = claims.temporary_state_bytes;
        let event_rows = self.json_rows(CHRONOLOGY_PROVENANCE, PROVENANCE_SCHEMA, true)?;
        let event_state_bytes = event_rows
            .as_ref()
            .map_or(0, |rows| rows.temporary_state_bytes);
        let event = event_rows
            .as_ref()
            .and_then(|rows| rows.rows.first())
            .map(|(_, event)| event);
        if event_rows.as_ref().is_some_and(|rows| rows.rows.len() != 1) {
            self.issue(
                CHRONOLOGY_PROVENANCE,
                "work chronology must have exactly one batch provenance event",
            )?;
        }
        if let Some(event) = &event {
            if text(event, "event_id") != Some(CHRONOLOGY_EVENT)
                || text(event, "event_type") != Some("annotation")
                || value_strings(event, "agent_refs") != vec!["model:codex".to_owned()]
                || text(event, "status") != Some("completed_with_warnings")
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance posture drifted",
                )?;
            }
            let method = event.get("method").unwrap_or(&Value::Null);
            if text(method, "maker_type") != Some("model")
                || text(method, "name")
                    != Some("faceted-first-publication-chronology-materialization")
                || text(method, "version") != Some("1")
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance method identity drifted",
                )?;
            }
        }
        let Some(claim_digest) = self.digest_for(CHRONOLOGY_CLAIMS)? else {
            self.release_loaded_rows(event_state_bytes)?;
            self.release_loaded_rows(claims_state_bytes)?;
            return Ok(());
        };
        let mut evidence_paths = BTreeSet::new();
        let mut subjects = BTreeMap::<String, String>::new();
        self.reserve(std::mem::size_of::<BTreeMap<String, String>>())?;
        let mut staged_count = 0u64;
        let mut single_count = 0u64;
        for (line, claim) in &claims.rows {
            let location = format!("{CHRONOLOGY_CLAIMS}:{line}");
            let claim_id = text(claim, "claim_id").unwrap_or_default().to_owned();
            let subject = text(claim, "subject_ref").unwrap_or_default().to_owned();
            self.reserve(
                claim_id.len()
                    + subject.len()
                    + std::mem::size_of::<String>()
                    + std::mem::size_of::<String>()
                    + 4 * std::mem::size_of::<usize>(),
            )?;
            subjects.insert(claim_id, subject.clone());
            if text(claim, "claim_type") != Some("bibliographic")
                || text(claim, "assertion_layer") != Some("scholarly_report")
                || text(claim, "predicate") != Some("first_publication_chronology")
            {
                self.issue(&location, "work chronology claim profile drifted")?;
            }
            self.expect_ref(&location, Some(&subject), "work")?;
            if !self.python_equal(
                claim.get("maker").unwrap_or(&Value::Null),
                &serde_json::json!({"maker_type":"model","agent_ref":"model:codex"}),
            )? {
                self.issue(&location, "work chronology maker must be model:codex")?;
            }
            if text(claim, "provenance_event_ref") != Some(CHRONOLOGY_EVENT)
                || text(claim, "epistemic_status") != Some("reported")
                || text(claim, "review_status") != Some("unreviewed")
                || !claim
                    .get("reviews")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
                || text(claim, "visibility") != Some("public_metadata_only")
            {
                self.issue(&location, "work chronology claim authority posture drifted")?;
            }
            let refs = value_strings(claim, "evidence_refs");
            for reference in &refs {
                if !reference.starts_with("ToS/") {
                    self.issue(
                        &location,
                        "work chronology evidence must be a tracked repository path",
                    )?;
                } else if !self.current_exists(reference)? {
                    self.issue(
                        &location,
                        format!("unresolved work chronology evidence: {reference}"),
                    )?;
                }
                if reference.starts_with("ToS/") {
                    self.reserve(
                        reference.len()
                            + std::mem::size_of::<String>()
                            + 4 * std::mem::size_of::<usize>(),
                    )?;
                    evidence_paths.insert(reference.clone());
                }
            }
            if !refs
                .iter()
                .any(|reference| reference.contains("authorial-witness-route"))
            {
                self.issue(
                    &location,
                    "work chronology lacks its ordered discovery receipt",
                )?;
            }
            if !refs
                .iter()
                .any(|reference| reference.contains("AUTHORIAL_WITNESS_ROUTE.md"))
            {
                self.issue(&location, "work chronology lacks its documentary synthesis")?;
            }
            let chronology = claim.get("object").unwrap_or(&Value::Null);
            self.request_schema(
                &format!("{location}['object']"),
                CHRONOLOGY_SCHEMA,
                chronology,
            )?;
            let interval = chronology.get("interval").unwrap_or(&Value::Null);
            let start = text(interval, "start");
            let end = text(interval, "end");
            if start.zip(end).is_some_and(|(start, end)| start > end) {
                self.issue(&location, "work chronology interval starts after it ends")?;
            }
            let stages = chronology
                .get("stages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let dates: Vec<String> = stages
                .iter()
                .filter_map(|stage| text(stage, "date").map(str::to_owned))
                .collect();
            if dates.windows(2).any(|pair| pair[0] > pair[1]) {
                self.issue(&location, "work chronology stages are not date ordered")?;
            }
            let posture = text(chronology, "sequence_posture");
            let boundary = text(interval, "boundary_meaning");
            if posture == Some("single_event") {
                single_count += 1;
                if stages.len() != 1 || boundary != Some("single_stage") {
                    self.issue(
                        &location,
                        "single-event chronology must contain one single-stage boundary",
                    )?;
                }
            } else if posture == Some("staged_sequence") {
                staged_count += 1;
                if stages.len() < 2 || boundary != Some("earliest_stage_to_sequence_completion") {
                    self.issue(
                        &location,
                        "staged chronology must retain multiple sequence stages",
                    )?;
                }
            }
            if let Some(first) = dates.first() {
                if start.is_some_and(|start| !first.starts_with(start)) {
                    self.issue(
                        &location,
                        "chronology interval start differs from first stage",
                    )?;
                }
            }
            if let Some(last) = dates.last() {
                if end.is_some_and(|end| !last.starts_with(end)) {
                    self.issue(&location, "chronology interval end differs from last stage")?;
                }
            }
            for stage in &stages {
                let Some(edition_ref) = text(stage, "edition_ref") else {
                    continue;
                };
                self.expect_ref(&location, Some(edition_ref), "edition")?;
                let temporary_baseline = self.temporary_state_bytes;
                let same_work: Result<Option<bool>, ItemRefusal> = (|| {
                    let (edition, edition_workspace) =
                        self.current_record_with_state_budget(edition_ref)?;
                    let Some(edition) = edition else {
                        self.release_temporary_state(edition_workspace)?;
                        return Ok(None);
                    };
                    let expression_values = edition
                        .value
                        .get("embodies_expression_refs")
                        .and_then(Value::as_array);
                    let mut expression_refs_state = std::mem::size_of::<Vec<String>>();
                    if let Some(values) = expression_values {
                        expression_refs_state = expression_refs_state
                            .checked_add(
                                values
                                    .len()
                                    .checked_mul(std::mem::size_of::<String>())
                                    .ok_or(ItemRefusal::Budget)?,
                            )
                            .ok_or(ItemRefusal::Budget)?;
                        for expression_ref in values.iter().filter_map(Value::as_str) {
                            expression_refs_state = expression_refs_state
                                .checked_add(estimate_string_storage(expression_ref)?)
                                .ok_or(ItemRefusal::Budget)?;
                        }
                    }
                    self.reserve_temporary(expression_refs_state)?;
                    let mut expression_refs =
                        Vec::with_capacity(expression_values.map_or(0, Vec::len));
                    if let Some(values) = expression_values {
                        expression_refs
                            .extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
                    }
                    drop(edition);
                    self.release_temporary_state(edition_workspace)?;

                    let mut same_work = false;
                    for expression_ref in &expression_refs {
                        let (expression, expression_workspace) =
                            self.current_record_with_state_budget(expression_ref)?;
                        let matches = expression.as_ref().is_some_and(|expression| {
                            text(&expression.value, "work_ref") == Some(subject.as_str())
                        });
                        drop(expression);
                        self.release_temporary_state(expression_workspace)?;
                        if matches {
                            same_work = true;
                            break;
                        }
                    }
                    drop(expression_refs);
                    self.release_temporary_state(expression_refs_state)?;
                    Ok(Some(same_work))
                })();
                self.release_temporary_since(temporary_baseline);
                let Some(same_work) = same_work? else {
                    continue;
                };
                if !same_work {
                    self.issue(
                        &location,
                        format!("chronology stage edition belongs to another Work: {edition_ref}"),
                    )?;
                }
            }
        }
        let mut current_works = BTreeSet::new();
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let records = self.records;
        let max_state = self.limits.max_state_bytes;
        let retained_base = self.retained_state_bytes + self.temporary_state_bytes;
        let mut current_work_state = std::mem::size_of::<BTreeSet<String>>();
        records.for_each_current_record(&mut |id, record| {
            check(deadline, cancelled)?;
            if record.kind == "work"
                && record
                    .path
                    .starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")
            {
                let row_state = id
                    .len()
                    .checked_add(std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
                    .ok_or(ItemRefusal::Budget)?;
                current_work_state = current_work_state
                    .checked_add(row_state)
                    .ok_or(ItemRefusal::Budget)?;
                let used = retained_base
                    .checked_add(current_work_state)
                    .ok_or(ItemRefusal::Budget)?;
                if used > max_state {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "source-foundation closure chronology work index",
                        used: Some(used as u64),
                        limit: Some(max_state as u64),
                    });
                }
                current_works.insert(id.to_owned());
            }
            Ok(())
        })?;
        self.reserve(current_work_state)?;
        let chronology_work_state = subjects.values().try_fold(
            std::mem::size_of::<BTreeSet<String>>(),
            |state, subject| {
                state
                    .checked_add(
                        subject
                            .len()
                            .checked_add(
                                std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>(),
                            )
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)
            },
        )?;
        self.reserve(chronology_work_state)?;
        let chronology_works: BTreeSet<String> = subjects.values().cloned().collect();
        if chronology_works != current_works {
            self.issue(
                CHRONOLOGY_CLAIMS,
                "work chronology subjects do not close over the current Nietzsche Works",
            )?;
        }
        if let Some(event) = &event {
            if !output_binds(
                event,
                CHRONOLOGY_CLAIMS,
                "unreviewed-evidence-bearing-work-chronology-claims",
                &claim_digest,
            ) || event
                .get("outputs")
                .and_then(Value::as_array)
                .is_none_or(|outputs| outputs.len() != 1)
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance does not digest-bind the exact claim file",
                )?;
            }
            let mut expected_inputs = evidence_paths;
            expected_inputs.insert(CLAIM_SCHEMA.to_owned());
            expected_inputs.insert(WORK_CHRONOLOGY_SCHEMA.to_owned());
            let inputs = event
                .get("inputs")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let actual_inputs: BTreeSet<String> = inputs
                .iter()
                .filter_map(|row| text(row, "ref").map(str::to_owned))
                .collect();
            if actual_inputs != expected_inputs || inputs.len() != expected_inputs.len() {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance inputs differ from the exact claim evidence set",
                )?;
            }
            self.check_event_input_bindings(
                CHRONOLOGY_PROVENANCE,
                event,
                "work chronology provenance input",
            )?;
            let expected_configuration = serde_json::json!({
                "works_materialized": 7,
                "chronology_claims_materialized": 7,
                "staged_sequence_claims": 1,
                "single_event_claims": 6,
                "chronology_claims_reviewed": 0,
                "composition_claims_created": 0,
                "source_text_admitted": false,
                "human_review_performed": false,
                "semantic_claims_created": 0,
                "canon_promotion_performed": false
            });
            if !self.python_equal(
                event
                    .get("method")
                    .and_then(|method| method.get("configuration"))
                    .unwrap_or(&Value::Null),
                &expected_configuration,
            )? || subjects.len() != 7
                || staged_count != 1
                || single_count != 6
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance configuration differs from the bounded profile",
                )?;
            }
        }
        self.release_loaded_rows(event_state_bytes)?;
        self.release_loaded_rows(claims_state_bytes)?;
        Ok(())
    }

    fn check_object_links(&mut self) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            return self.check_candidate_object_links();
        }
        let temporary_baseline = self.temporary_state_bytes;
        let claims_state = claim_refs_vec_clone_state(&self.object_links)?;
        let targets_state = self
            .object_links
            .iter()
            .try_fold(0usize, |state, (id, claim)| {
                state
                    .checked_add(
                        id.len()
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(claim.object.len()))
                            .and_then(|n| n.checked_add(claim.event.len()))
                            .and_then(|n| {
                                n.checked_add(
                                    2 * (std::mem::size_of::<String>()
                                        + 4 * std::mem::size_of::<usize>()),
                                )
                            })
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)
            })?;
        let links_state = if self.link_store.is_some() {
            0
        } else {
            self.links
                .iter()
                .try_fold(0usize, |state, (id, (path, value))| {
                    state
                        .checked_add(
                            id.len()
                                .checked_add(path.len())
                                .and_then(|n| {
                                    n.checked_add(
                                        crate::record_biblio_cut::decoded_state(value).ok()?,
                                    )
                                })
                                .and_then(|n| {
                                    n.checked_add(std::mem::size_of::<(String, String, Value)>())
                                })
                                .ok_or(ItemRefusal::Budget)?,
                        )
                        .ok_or(ItemRefusal::Budget)
                })?
        };
        self.reserve_temporary(
            claims_state
                .checked_add(targets_state)
                .and_then(|n| n.checked_add(links_state))
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let claims: Vec<(String, SourceFoundationClosureClaimRef)> = self
            .object_links
            .iter()
            .map(|(id, claim)| (id.clone(), claim.clone()))
            .collect();
        let mut targets = BTreeMap::<String, String>::new();
        let mut events = BTreeMap::<String, String>::new();
        for (claim_id, claim) in claims {
            check(self.limits.deadline, self.source.cancellation())?;
            let location = &claim.location;
            let (subject, subject_workspace) =
                self.current_record_with_state_budget(&claim.subject)?;
            let (subject_exists, subject_is_link, subject_is_valid_native) = {
                match subject.as_ref() {
                    None => (false, false, false),
                    Some(subject) => (
                        true,
                        subject.kind == "link",
                        matches!(
                            subject.kind.as_str(),
                            "work" | "expression" | "edition" | "collection" | "item" | "artifact"
                        ),
                    ),
                }
            };
            drop(subject);
            self.release_temporary_state(subject_workspace)?;
            if !subject_exists && !claim.native {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            } else if subject_is_link {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            } else if claim.native && !subject_is_valid_native {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid native object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            }
            if !self.link_exists(&claim.object)? {
                self.issue(
                    location,
                    format!("unresolved Link object: {}", claim.object),
                )?;
            }
            if !self.event_exists(&claim.event)? {
                self.issue(
                    location,
                    format!(
                        "unresolved object-Link provenance_event_ref: {}",
                        claim.event
                    ),
                )?;
            }
            targets.insert(claim_id.clone(), claim.object.clone());
            events.insert(claim_id, claim.event.clone());
        }
        if self.link_store.is_some() {
            self.check_stored_links(Some(&targets), Some(&events), temporary_baseline)?;
        } else {
            let links: Vec<(String, String, Value)> = self
                .links
                .iter()
                .map(|(id, (path, value))| (id.clone(), path.clone(), value.clone()))
                .collect();
            for (link_id, path, link) in links {
                self.check_link_row(&link_id, &path, &link, &targets, &events)?;
            }
        }
        self.release_temporary_since(temporary_baseline);
        Ok(())
    }

    fn check_candidate_object_links(&mut self) -> Result<(), ItemRefusal> {
        let temporary_baseline = self.temporary_state_bytes;
        let expected_rows = self.cost.candidate_object_link_store.claim_rows;
        self.schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .begin_object_link_claims(expected_rows)?;
        loop {
            check(self.limits.deadline, self.source.cancellation())?;
            let remaining = self.remaining_state()?;
            let (claim, workspace, row_state, next_cursor_state) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .next_object_link_claim(remaining)?;
            self.include_store_workspace(workspace)?;
            let Some((claim_id, claim)) = claim else {
                self.temporary_state_bytes = temporary_baseline;
                self.cost.candidate_object_link_store.eof_seen = true;
                if self.cost.candidate_object_link_store.drained_rows != expected_rows {
                    return Err(ItemRefusal::Source(
                        "source-foundation object-Link claim count differs from its ordered drain"
                            .into(),
                    ));
                }
                break;
            };
            let row_and_cursor = row_state
                .checked_add(next_cursor_state)
                .ok_or(ItemRefusal::Budget)?;
            let next_temporary = temporary_baseline
                .checked_add(row_and_cursor)
                .ok_or(ItemRefusal::Budget)?;
            if self
                .retained_state_bytes
                .checked_add(next_temporary)
                .is_none_or(|used| used > self.limits.max_state_bytes)
            {
                return Err(ItemRefusal::Budget);
            }
            self.temporary_state_bytes = next_temporary;
            self.cost.reserved_state_bytes = self
                .cost
                .reserved_state_bytes
                .max(self.retained_state_bytes + next_temporary);

            let location = claim.location.as_str();
            let (subject, subject_workspace) =
                self.current_record_with_state_budget(&claim.subject)?;
            let (subject_exists, subject_is_link, subject_is_valid_native) = {
                match subject.as_ref() {
                    None => (false, false, false),
                    Some(subject) => (
                        true,
                        subject.kind == "link",
                        matches!(
                            subject.kind.as_str(),
                            "work" | "expression" | "edition" | "collection" | "item" | "artifact"
                        ),
                    ),
                }
            };
            drop(subject);
            self.release_temporary_state(subject_workspace)?;
            if !subject_exists && !claim.native {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            } else if subject_is_link {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            } else if claim.native && !subject_is_valid_native {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid native object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            }
            if !self.link_exists(&claim.object)? {
                self.issue(
                    location,
                    format!("unresolved Link object: {}", claim.object),
                )?;
            }
            if !self.event_exists(&claim.event)? {
                self.issue(
                    location,
                    format!(
                        "unresolved object-Link provenance_event_ref: {}",
                        claim.event
                    ),
                )?;
            }
            drop(claim);
            drop(claim_id);
            self.temporary_state_bytes = temporary_baseline
                .checked_add(next_cursor_state)
                .ok_or(ItemRefusal::Budget)?;
            self.cost.candidate_object_link_store.drained_rows = self
                .cost
                .candidate_object_link_store
                .drained_rows
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        if self.link_store.is_some() {
            self.check_stored_links(None, None, temporary_baseline)?;
        } else {
            let links_state =
                self.links
                    .iter()
                    .try_fold(0usize, |state, (id, (path, value))| {
                        state
                            .checked_add(
                                id.len()
                                    .checked_add(path.len())
                                    .and_then(|bytes| {
                                        bytes.checked_add(
                                            crate::record_biblio_cut::decoded_state(value).ok()?,
                                        )
                                    })
                                    .and_then(|bytes| {
                                        bytes.checked_add(std::mem::size_of::<(
                                            String,
                                            String,
                                            Value,
                                        )>(
                                        ))
                                    })
                                    .ok_or(ItemRefusal::Budget)?,
                            )
                            .ok_or(ItemRefusal::Budget)
                    })?;
            self.reserve_temporary(links_state)?;
            let links: Vec<(String, String, Value)> = self
                .links
                .iter()
                .map(|(id, (path, value))| (id.clone(), path.clone(), value.clone()))
                .collect();
            for (link_id, path, link) in links {
                self.check_link_row_candidate(&link_id, &path, &link)?;
            }
        }
        self.temporary_state_bytes = temporary_baseline;
        Ok(())
    }

    fn check_stored_links(
        &mut self,
        targets: Option<&BTreeMap<String, String>>,
        events: Option<&BTreeMap<String, String>>,
        temporary_baseline: usize,
    ) -> Result<(), ItemRefusal> {
        let mut store = self.link_store.take().ok_or(ItemRefusal::Budget)?;
        let mut after_id: Option<String> = None;
        let mut cursor_state = 0usize;
        let mut drained = 0u64;
        loop {
            let remaining = self.remaining_state()?;
            let (link, workspace) = store.next_link(after_id.as_deref(), remaining)?;
            self.reserve_temporary(workspace)?;
            let Some(link) = link else {
                self.temporary_state_bytes = temporary_baseline
                    .checked_add(cursor_state)
                    .ok_or(ItemRefusal::Budget)?;
                let remaining = self.remaining_state()?;
                let finished = store.finish_links(self.link_count, remaining)?;
                self.reserve_temporary(finished.workspace_state_bytes)?;
                if finished.inserted_rows != self.link_count
                    || finished.drained_rows != drained
                    || drained != self.link_count
                {
                    return Err(ItemRefusal::Source(
                        "source-foundation Closure Link store count differs from its ordered drain"
                            .into(),
                    ));
                }
                self.cost.candidate_link_rows = finished.inserted_rows;
                self.cost.candidate_link_serialized_write_bytes = finished.serialized_write_bytes;
                self.cost.candidate_link_serialized_read_bytes = finished.serialized_read_bytes;
                self.cost.candidate_link_scan_row_operations = finished.scan_row_operations;
                self.cost.candidate_link_peak_workspace_state_bytes =
                    finished.workspace_state_bytes;
                self.include_link_workspace(finished.workspace_state_bytes)?;
                store.verify_finished()?;
                break;
            };
            if link.id.is_empty()
                || !link.path.ends_with("/link.json")
                || text(&link.value, "record_id") != Some(link.id.as_str())
                || after_id
                    .as_deref()
                    .is_some_and(|prior| prior >= link.id.as_str())
                || drained >= self.link_count
            {
                return Err(ItemRefusal::Source(
                    "source-foundation Closure Link rows are not exact and ID ordered".into(),
                ));
            }
            let scratch = match (targets, events) {
                (Some(targets), Some(_)) => link_validation_workspace(&link.value, targets)?,
                (None, None) => link_validation_candidate_workspace(&link.value)?,
                _ => return Err(ItemRefusal::Budget),
            };
            match (targets, events) {
                (Some(targets), Some(events)) => {
                    self.reserve_temporary(scratch)?;
                    self.check_link_row(&link.id, &link.path, &link.value, targets, events)?;
                }
                (None, None) => {
                    self.check_link_row_candidate(&link.id, &link.path, &link.value)?;
                }
                _ => return Err(ItemRefusal::Budget),
            }
            let next_cursor_state = estimate_string_storage(&link.id)?;
            self.reserve_temporary(next_cursor_state)?;
            let next_after = link.id.clone();
            drop(link);
            self.temporary_state_bytes = temporary_baseline
                .checked_add(next_cursor_state)
                .ok_or(ItemRefusal::Budget)?;
            if let Some(previous_cursor) = after_id.take() {
                drop(previous_cursor);
            }
            after_id = Some(next_after);
            cursor_state = next_cursor_state;
            drained = drained.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
        self.temporary_state_bytes = temporary_baseline;
        self.link_store = Some(store);
        Ok(())
    }

    fn check_link_row(
        &mut self,
        link_id: &str,
        path: &str,
        link: &Value,
        targets: &BTreeMap<String, String>,
        events: &BTreeMap<String, String>,
    ) -> Result<(), ItemRefusal> {
        let refs = value_strings(link, "association_claim_refs");
        let ref_set: BTreeSet<String> = refs.iter().cloned().collect();
        let missing: Vec<String> = ref_set
            .iter()
            .filter(|id| !self.object_links.contains_key(*id))
            .cloned()
            .collect();
        if !missing.is_empty() {
            self.issue(
                path,
                format!(
                    "unresolved object-Link claims: {}",
                    python_string_list(&missing)
                ),
            )?;
        }
        let misbound: Vec<String> = ref_set
            .iter()
            .filter(|id| {
                self.object_links.contains_key(*id)
                    && targets.get(*id).map(String::as_str) != Some(link_id)
            })
            .cloned()
            .collect();
        if !misbound.is_empty() {
            self.issue(
                path,
                format!(
                    "object-Link claims target another Link: {}",
                    python_string_list(&misbound)
                ),
            )?;
        }
        let event_ref = text(link, "provenance_event_ref").unwrap_or_default();
        let event_mismatch: Vec<String> = ref_set
            .iter()
            .filter(|id| {
                self.object_links.contains_key(*id)
                    && events.get(*id).map(String::as_str) != Some(event_ref)
            })
            .cloned()
            .collect();
        if !event_mismatch.is_empty() {
            self.issue(
                path,
                format!(
                    "object-Link claims cite another provenance event: {}",
                    python_string_list(&event_mismatch)
                ),
            )?;
        }
        let unreferenced: Vec<String> = targets
            .iter()
            .filter(|(id, target)| target.as_str() == link_id && !ref_set.contains(*id))
            .map(|(id, _)| id.clone())
            .collect();
        if !unreferenced.is_empty() {
            self.issue(
                path,
                format!(
                    "object-Link claims are not referenced by Link: {}",
                    python_string_list(&unreferenced)
                ),
            )?;
        }
        Ok(())
    }

    fn check_link_row_candidate(
        &mut self,
        link_id: &str,
        path: &str,
        link: &Value,
    ) -> Result<(), ItemRefusal> {
        let scratch = link_validation_candidate_workspace(link)?;
        self.reserve_temporary(scratch)?;
        let refs = value_strings(link, "association_claim_refs");
        let ref_set: BTreeSet<String> = refs.iter().cloned().collect();
        drop(refs);
        let event_ref = text(link, "provenance_event_ref").unwrap_or_default();
        let mut missing = Vec::new();
        let mut misbound = Vec::new();
        let mut event_mismatch = Vec::new();
        let mut association_bytes = 0usize;
        for id in &ref_set {
            check(self.limits.deadline, self.source.cancellation())?;
            association_bytes = association_bytes
                .checked_add(id.len())
                .ok_or(ItemRefusal::Budget)?;
            let remaining = self.remaining_state()?;
            let (exists, targets_link, matches_event, workspace) = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(ItemRefusal::Budget)?
                .object_link_relation(id, link_id, event_ref, remaining)?;
            self.include_store_workspace(workspace)?;
            if !exists {
                missing.push(id.clone());
            } else {
                if !targets_link {
                    misbound.push(id.clone());
                }
                if !matches_event {
                    event_mismatch.push(id.clone());
                }
            }
        }
        if !missing.is_empty() {
            self.issue_object_link_list(
                path,
                "unresolved object-Link claims: ",
                &missing,
                association_bytes,
            )?;
        }
        if !misbound.is_empty() {
            self.issue_object_link_list(
                path,
                "object-Link claims target another Link: ",
                &misbound,
                association_bytes,
            )?;
        }
        if !event_mismatch.is_empty() {
            self.issue_object_link_list(
                path,
                "object-Link claims cite another provenance event: ",
                &event_mismatch,
                association_bytes,
            )?;
        }
        let remaining = self.remaining_state()?;
        let mut unreferenced = Vec::<String>::new();
        let mut unreferenced_string_state = 0usize;
        let mut unreferenced_state = 0usize;
        let mut target_stream_combined_peak = 0usize;
        let (streamed, workspace) = self
            .schema_request_store
            .as_deref_mut()
            .ok_or(ItemRefusal::Budget)?
            .for_each_object_link_target(link_id, remaining, &mut |id, provider_workspace| {
                if ref_set.contains(id) {
                    return Ok(());
                }
                let next_count = unreferenced
                    .len()
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
                let next_string_state = unreferenced_string_state
                    .checked_add(estimate_string_storage(id)?)
                    .ok_or(ItemRefusal::Budget)?;
                let next_state = next_count
                    .checked_mul(2)
                    .ok_or(ItemRefusal::Budget)?
                    .checked_mul(std::mem::size_of::<String>())
                    .and_then(|state| state.checked_add(next_string_state))
                    .ok_or(ItemRefusal::Budget)?;
                if provider_workspace
                    .checked_add(next_state)
                    .is_none_or(|used| used > remaining)
                {
                    return Err(ItemRefusal::Budget);
                }
                target_stream_combined_peak = target_stream_combined_peak.max(
                    provider_workspace
                        .checked_add(next_state)
                        .ok_or(ItemRefusal::Budget)?,
                );
                unreferenced
                    .try_reserve_exact(1)
                    .map_err(|_| ItemRefusal::Budget)?;
                unreferenced.push(id.to_owned());
                unreferenced_string_state = next_string_state;
                unreferenced_state = next_state;
                Ok(())
            })?;
        let _ = streamed;
        self.include_store_workspace(workspace)?;
        let combined_peak = self
            .retained_state_bytes
            .checked_add(self.temporary_state_bytes)
            .and_then(|state| state.checked_add(target_stream_combined_peak))
            .ok_or(ItemRefusal::Budget)?;
        self.cost.reserved_state_bytes = self.cost.reserved_state_bytes.max(combined_peak);
        if !unreferenced.is_empty() {
            self.reserve_temporary(unreferenced_state)?;
            self.issue_object_link_list(
                path,
                "object-Link claims are not referenced by Link: ",
                &unreferenced,
                unreferenced.iter().try_fold(0usize, |bytes, id| {
                    bytes.checked_add(id.len()).ok_or(ItemRefusal::Budget)
                })?,
            )?;
            drop(unreferenced);
            self.release_temporary_state(unreferenced_state)?;
        }
        drop(missing);
        drop(misbound);
        drop(event_mismatch);
        drop(ref_set);
        self.release_temporary_state(scratch)?;
        Ok(())
    }

    fn issue_object_link_list(
        &mut self,
        path: &str,
        prefix: &str,
        ids: &[String],
        source_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        let (render_peak, list_bytes, list_state) =
            python_string_list_workspace(ids.len(), source_bytes)?;
        let message_state = bounded_string_capacity_state(
            prefix
                .len()
                .checked_add(list_bytes)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let format_workspace = render_peak
            .checked_add(list_state)
            .and_then(|state| state.checked_add(message_state))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_temporary(format_workspace)?;
        let rendered = python_string_list(ids);
        let message = format!("{prefix}{rendered}");
        drop(rendered);
        self.issue(path, message)?;
        self.release_temporary_state(format_workspace)?;
        Ok(())
    }

    fn check_record_backlinks(&mut self) -> Result<(), ItemRefusal> {
        let records = self.records;
        let membership = &self.membership;
        let closure_store = &mut self.schema_request_store;
        let candidate_mode = closure_store.is_some();
        let responsibility = &self.responsibility;
        let publication = &self.publication;
        let provision = &self.provision;
        let chronology = &self.chronology;
        let scope = self.scope;
        let deadline = self.limits.deadline;
        let cancelled = self.source.cancellation();
        let limits = self.limits;
        let temporary_base = self.temporary_state_bytes;
        let issues = &mut self.issues;
        let cost = &mut self.cost;
        let retained = &mut self.retained_state_bytes;
        records.for_each_current_record(&mut |id, record| {
            check(deadline, cancelled)?;
            let is_era_work = scope == SourceFoundationDefaultRuleScope::FullAudit
                && record.kind == "work"
                && record
                    .path
                    .starts_with("ToS/source-witnesses/works/friedrich-nietzsche/");
            if record.kind != "collection"
                && !matches!(record.kind.as_str(), "work" | "expression" | "edition")
            {
                return Ok(());
            }
            let workspace = crate::record_biblio_cut::decoded_state(&record.value)?
                .checked_mul(3)
                .and_then(|n| n.checked_add(512))
                .ok_or(ItemRefusal::Budget)?;
            let mut findings = Vec::<String>::new();
            let mut findings_state_bytes = if candidate_mode {
                bounded_string_vec_state(&findings, findings.capacity())?
            } else {
                0
            };
            let mut temporary = temporary_base
                .checked_add(workspace)
                .and_then(|state| state.checked_add(findings_state_bytes))
                .ok_or(ItemRefusal::Budget)?;
            let used = retained
                .checked_add(temporary)
                .ok_or(ItemRefusal::Budget)?;
            if used > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure record-backlink workspace",
                    used: Some(used as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            cost.reserved_state_bytes = cost.reserved_state_bytes.max(used);
            let location = record.path.as_str();
            if record.kind == "collection" {
                let mut refs = value_strings(&record.value, "membership_claim_refs");
                let reference_count = refs.len();
                refs.sort();
                refs.dedup();
                let mut mismatched = reference_count != refs.len();
                if let Some(store) = closure_store.as_deref_mut() {
                    let remaining = limits.max_state_bytes.checked_sub(used).ok_or(
                        ItemRefusal::BudgetCheck {
                            check: "source-foundation closure membership stream workspace",
                            used: Some(used as u64),
                            limit: Some(limits.max_state_bytes as u64),
                        },
                    )?;
                    let mut next_ref = 0usize;
                    let (drained, store_workspace) =
                        store.for_each_membership_claim_for_subject(
                            id,
                            remaining,
                            &mut |claim_id| {
                                if refs.get(next_ref).map(String::as_str) != Some(claim_id) {
                                    mismatched = true;
                                }
                                next_ref = next_ref.checked_add(1).ok_or(ItemRefusal::Budget)?;
                                Ok(())
                            },
                        )?;
                    let with_store = used
                        .checked_add(store_workspace)
                        .ok_or(ItemRefusal::Budget)?;
                    if with_store > limits.max_state_bytes {
                        return Err(ItemRefusal::Budget);
                    }
                    cost.reserved_state_bytes = cost.reserved_state_bytes.max(with_store);
                    mismatched |= usize::try_from(drained).ok() != Some(refs.len());
                } else {
                    let valid_ids: BTreeSet<&str> = membership
                        .iter()
                        .filter(|(_, claim)| claim.subject == id)
                        .map(|(claim_id, _)| claim_id.as_str())
                        .collect();
                    mismatched |= refs.iter().map(String::as_str).ne(valid_ids.iter().copied());
                }
                if mismatched {
                    findings.push("unresolved or mismatched membership claims: Collection membership refs do not close over all verified current Claims".to_owned());
                    if candidate_mode {
                        refresh_candidate_findings_state(
                            &findings,
                            &mut findings_state_bytes,
                            &mut temporary,
                        )?;
                    }
                }
            }
            let mut candidate_responsibility = BTreeMap::new();
            let mut candidate_responsibility_state = 0usize;
            let mut candidate_publication = BTreeMap::new();
            let mut candidate_publication_state = 0usize;
            if matches!(record.kind.as_str(), "work" | "expression" | "edition") {
                if let Some(store) = closure_store.as_deref_mut() {
                    let mut queried_ids = BTreeSet::new();
                    let mut queried_ids_state = 0usize;
                    if let Some(references) = record
                        .value
                        .get("responsibility_claim_refs")
                        .and_then(Value::as_array)
                    {
                        for claim_id in references.iter().filter_map(Value::as_str) {
                            if queried_ids.contains(claim_id) {
                                continue;
                            }
                            let query_node_state = std::mem::size_of::<&str>()
                                .checked_add(8 * std::mem::size_of::<usize>())
                                .ok_or(ItemRefusal::Budget)?;
                            let before_query = retained
                                .checked_add(temporary)
                                .and_then(|state| state.checked_add(candidate_responsibility_state))
                                .and_then(|state| state.checked_add(queried_ids_state))
                                .ok_or(ItemRefusal::Budget)?;
                            let after_query_set = before_query
                                .checked_add(query_node_state)
                                .ok_or(ItemRefusal::Budget)?;
                            if after_query_set > limits.max_state_bytes {
                                return Err(ItemRefusal::BudgetCheck {
                                    check: "source-foundation closure responsibility reference workspace",
                                    used: Some(after_query_set as u64),
                                    limit: Some(limits.max_state_bytes as u64),
                                });
                            }
                            queried_ids.insert(claim_id);
                            queried_ids_state = queried_ids_state
                                .checked_add(query_node_state)
                                .ok_or(ItemRefusal::Budget)?;
                            cost.reserved_state_bytes =
                                cost.reserved_state_bytes.max(after_query_set);
                            let used = retained
                                .checked_add(temporary)
                                .and_then(|state| state.checked_add(candidate_responsibility_state))
                                .and_then(|state| state.checked_add(queried_ids_state))
                                .ok_or(ItemRefusal::Budget)?;
                            let remaining = limits.max_state_bytes.checked_sub(used).ok_or(
                                ItemRefusal::BudgetCheck {
                                    check: "source-foundation closure responsibility point lookup",
                                    used: Some(used as u64),
                                    limit: Some(limits.max_state_bytes as u64),
                                },
                            )?;
                            let (candidate, store_workspace) =
                                store.responsibility_claim_by_id(claim_id, remaining)?;
                            let with_store = used
                                .checked_add(store_workspace)
                                .ok_or(ItemRefusal::Budget)?;
                            if with_store > limits.max_state_bytes {
                                return Err(ItemRefusal::Budget);
                            }
                            cost.reserved_state_bytes = cost.reserved_state_bytes.max(with_store);
                            if let Some(reference) = candidate {
                                let insert_state = claim_reference_map_entry_state(claim_id)?;
                                let row_state = claim_reference_index_state(claim_id, &reference)?;
                                let with_map_row = with_store
                                    .checked_add(insert_state)
                                    .ok_or(ItemRefusal::Budget)?;
                                if with_map_row > limits.max_state_bytes {
                                    return Err(ItemRefusal::BudgetCheck {
                                        check: "source-foundation closure responsibility point result",
                                        used: Some(with_map_row as u64),
                                        limit: Some(limits.max_state_bytes as u64),
                                    });
                                }
                                cost.reserved_state_bytes =
                                    cost.reserved_state_bytes.max(with_map_row);
                                candidate_responsibility.insert(claim_id.to_owned(), reference);
                                candidate_responsibility_state = candidate_responsibility_state
                                    .checked_add(row_state)
                                    .ok_or(ItemRefusal::Budget)?;
                            }
                        }
                    }
                    drop(queried_ids);
                    let used = retained
                        .checked_add(temporary)
                        .and_then(|state| state.checked_add(candidate_responsibility_state))
                        .ok_or(ItemRefusal::Budget)?;
                    let remaining = limits.max_state_bytes.checked_sub(used).ok_or(
                        ItemRefusal::BudgetCheck {
                            check: "source-foundation closure responsibility subject stream",
                            used: Some(used as u64),
                            limit: Some(limits.max_state_bytes as u64),
                        },
                    )?;
                    let (drained, store_workspace) = store
                        .for_each_responsibility_claim_for_subject(
                            id,
                            remaining,
                            &mut |claim_id, reference, row_workspace| {
                                let base = retained
                                    .checked_add(temporary)
                                    .and_then(|state| {
                                        state.checked_add(candidate_responsibility_state)
                                    })
                                    .ok_or(ItemRefusal::Budget)?;
                                let provider_live = base
                                    .checked_add(row_workspace)
                                    .ok_or(ItemRefusal::Budget)?;
                                if provider_live > limits.max_state_bytes {
                                    return Err(ItemRefusal::BudgetCheck {
                                        check: "source-foundation closure responsibility subject row",
                                        used: Some(provider_live as u64),
                                        limit: Some(limits.max_state_bytes as u64),
                                    });
                                }
                                cost.reserved_state_bytes =
                                    cost.reserved_state_bytes.max(provider_live);
                                if candidate_responsibility.contains_key(claim_id) {
                                    return Ok(());
                                }
                                let row_state = claim_reference_index_state(claim_id, reference)?;
                                let with_map_row = provider_live
                                    .checked_add(row_state)
                                    .ok_or(ItemRefusal::Budget)?;
                                if with_map_row > limits.max_state_bytes {
                                    return Err(ItemRefusal::BudgetCheck {
                                        check: "source-foundation closure responsibility subject result",
                                        used: Some(with_map_row as u64),
                                        limit: Some(limits.max_state_bytes as u64),
                                    });
                                }
                                cost.reserved_state_bytes =
                                    cost.reserved_state_bytes.max(with_map_row);
                                candidate_responsibility
                                    .insert(claim_id.to_owned(), reference.clone());
                                candidate_responsibility_state = candidate_responsibility_state
                                    .checked_add(row_state)
                                    .ok_or(ItemRefusal::Budget)?;
                                Ok(())
                            },
                        )?;
                    let with_store = used
                        .checked_add(store_workspace)
                        .ok_or(ItemRefusal::Budget)?;
                    if with_store > limits.max_state_bytes {
                        return Err(ItemRefusal::Budget);
                    }
                    cost.reserved_state_bytes = cost.reserved_state_bytes.max(with_store);
                    let subject_rows = candidate_responsibility
                        .values()
                        .filter(|claim| claim.subject == id)
                        .count();
                    if usize::try_from(drained).ok() != Some(subject_rows) {
                        return Err(ItemRefusal::Source(
                            "source-foundation Closure responsibility subject count differs".into(),
                        ));
                    }
                }
                let responsibility_for_record = if closure_store.is_some() {
                    &candidate_responsibility
                } else {
                    responsibility
                };
                if candidate_mode {
                    let maps_state = candidate_responsibility_state
                        .checked_add(candidate_publication_state)
                        .ok_or(ItemRefusal::Budget)?;
                    append_candidate_backref_messages(
                        &mut findings,
                        &mut findings_state_bytes,
                        &mut temporary,
                        *retained,
                        maps_state,
                        limits,
                        cost,
                        &record.value,
                        "responsibility_claim_refs",
                        id,
                        "responsibility",
                        responsibility_for_record,
                        deadline,
                        cancelled,
                    )?;
                } else {
                    findings.extend(exact_backref_messages(
                        &record.value,
                        "responsibility_claim_refs",
                        id,
                        "responsibility",
                        responsibility_for_record,
                    ));
                }
                if is_era_work {
                    let actual: BTreeSet<String> =
                        value_strings(&record.value, "responsibility_claim_refs")
                            .into_iter()
                            .collect();
                    let authored: Vec<String> = actual
                        .iter()
                        .filter(|claim_id| {
                            responsibility_for_record
                                .get(*claim_id)
                                .is_some_and(|claim| claim.predicate == "authored_by")
                        })
                        .cloned()
                        .collect();
                    if authored.len() != 1 {
                        findings.push(format!(
                            "current Nietzsche Work must reference exactly one authored_by claim; found {}",
                            python_string_list(&authored)
                        ));
                    } else if responsibility_for_record
                        .get(&authored[0])
                        .map(|claim| claim.object.as_str())
                        != Some("tos.agent.friedrich-nietzsche")
                    {
                        findings.push("current Nietzsche Work authored_by claim must resolve to tos.agent.friedrich-nietzsche".to_owned());
                    }
                }
                if candidate_mode {
                    refresh_candidate_findings_state(
                        &findings,
                        &mut findings_state_bytes,
                        &mut temporary,
                    )?;
                }
            }
            if record.kind == "edition" {
                if let Some(store) = closure_store.as_deref_mut() {
                    let mut references = value_strings(&record.value, "publication_claim_refs");
                    references.sort();
                    references.dedup();
                    for claim_id in &references {
                        let used = retained
                            .checked_add(temporary)
                            .and_then(|state| state.checked_add(candidate_publication_state))
                            .ok_or(ItemRefusal::Budget)?;
                        let remaining = limits.max_state_bytes.checked_sub(used).ok_or(
                            ItemRefusal::BudgetCheck {
                                check: "source-foundation closure publication reference point lookup",
                                used: Some(used as u64),
                                limit: Some(limits.max_state_bytes as u64),
                            },
                        )?;
                        let (candidate, store_workspace) =
                            store.publication_claim_by_id(claim_id, remaining)?;
                        let with_store = used
                            .checked_add(store_workspace)
                            .ok_or(ItemRefusal::Budget)?;
                        if with_store > limits.max_state_bytes {
                            return Err(ItemRefusal::Budget);
                        }
                        cost.reserved_state_bytes = cost.reserved_state_bytes.max(with_store);
                        if let Some(reference) = candidate {
                            let insert_state = claim_reference_map_entry_state(claim_id)?;
                            let row_state = claim_reference_index_state(claim_id, &reference)?;
                            let with_map_row = with_store
                                .checked_add(insert_state)
                                .ok_or(ItemRefusal::Budget)?;
                            if with_map_row > limits.max_state_bytes {
                                return Err(ItemRefusal::BudgetCheck {
                                    check: "source-foundation closure publication point result",
                                    used: Some(with_map_row as u64),
                                    limit: Some(limits.max_state_bytes as u64),
                                });
                            }
                            cost.reserved_state_bytes = cost.reserved_state_bytes.max(with_map_row);
                            candidate_publication.insert(claim_id.clone(), reference);
                            candidate_publication_state = candidate_publication_state
                                .checked_add(row_state)
                                .ok_or(ItemRefusal::Budget)?;
                        }
                    }
                    drop(references);
                    let used = retained
                        .checked_add(temporary)
                        .and_then(|state| state.checked_add(candidate_publication_state))
                        .ok_or(ItemRefusal::Budget)?;
                    let remaining = limits.max_state_bytes.checked_sub(used).ok_or(
                        ItemRefusal::BudgetCheck {
                            check: "source-foundation closure publication subject stream",
                            used: Some(used as u64),
                            limit: Some(limits.max_state_bytes as u64),
                        },
                    )?;
                    let (drained, store_workspace) = store
                        .for_each_publication_claim_for_subject(
                            id,
                            remaining,
                            &mut |claim_id, reference, row_workspace| {
                                let base = retained
                                    .checked_add(temporary)
                                    .and_then(|state| {
                                        state.checked_add(candidate_publication_state)
                                    })
                                    .ok_or(ItemRefusal::Budget)?;
                                let provider_live = base
                                    .checked_add(row_workspace)
                                    .ok_or(ItemRefusal::Budget)?;
                                if provider_live > limits.max_state_bytes {
                                    return Err(ItemRefusal::BudgetCheck {
                                        check: "source-foundation closure publication subject row",
                                        used: Some(provider_live as u64),
                                        limit: Some(limits.max_state_bytes as u64),
                                    });
                                }
                                cost.reserved_state_bytes =
                                    cost.reserved_state_bytes.max(provider_live);
                                if candidate_publication.contains_key(claim_id) {
                                    return Ok(());
                                }
                                let row_state = claim_reference_index_state(claim_id, reference)?;
                                let with_map_row = provider_live
                                    .checked_add(row_state)
                                    .ok_or(ItemRefusal::Budget)?;
                                if with_map_row > limits.max_state_bytes {
                                    return Err(ItemRefusal::BudgetCheck {
                                        check: "source-foundation closure publication subject result",
                                        used: Some(with_map_row as u64),
                                        limit: Some(limits.max_state_bytes as u64),
                                    });
                                }
                                cost.reserved_state_bytes =
                                    cost.reserved_state_bytes.max(with_map_row);
                                candidate_publication
                                    .insert(claim_id.to_owned(), reference.clone());
                                candidate_publication_state = candidate_publication_state
                                    .checked_add(row_state)
                                    .ok_or(ItemRefusal::Budget)?;
                                Ok(())
                            },
                        )?;
                    let with_store = used
                        .checked_add(store_workspace)
                        .ok_or(ItemRefusal::Budget)?;
                    if with_store > limits.max_state_bytes {
                        return Err(ItemRefusal::Budget);
                    }
                    cost.reserved_state_bytes = cost.reserved_state_bytes.max(with_store);
                    let subject_rows = candidate_publication
                        .values()
                        .filter(|claim| claim.subject == id)
                        .count();
                    if usize::try_from(drained).ok() != Some(subject_rows) {
                        return Err(ItemRefusal::Source(
                            "source-foundation Closure publication subject count differs".into(),
                        ));
                    }
                }
                let publication_for_record = if closure_store.is_some() {
                    &candidate_publication
                } else {
                    publication
                };
                if candidate_mode {
                    let maps_state = candidate_responsibility_state
                        .checked_add(candidate_publication_state)
                        .ok_or(ItemRefusal::Budget)?;
                    append_candidate_backref_messages(
                        &mut findings,
                        &mut findings_state_bytes,
                        &mut temporary,
                        *retained,
                        maps_state,
                        limits,
                        cost,
                        &record.value,
                        "publication_claim_refs",
                        id,
                        "publication",
                        publication_for_record,
                        deadline,
                        cancelled,
                    )?;
                    append_candidate_backref_messages(
                        &mut findings,
                        &mut findings_state_bytes,
                        &mut temporary,
                        *retained,
                        maps_state,
                        limits,
                        cost,
                        &record.value,
                        "provision_activity_claim_refs",
                        id,
                        "provision-activity",
                        provision,
                        deadline,
                        cancelled,
                    )?;
                } else {
                    findings.extend(exact_backref_messages(
                        &record.value,
                        "publication_claim_refs",
                        id,
                        "publication",
                        publication_for_record,
                    ));
                    findings.extend(exact_backref_messages(
                        &record.value,
                        "provision_activity_claim_refs",
                        id,
                        "provision-activity",
                        provision,
                    ));
                }
            }
            if is_era_work {
                let expected: BTreeSet<String> = chronology
                    .iter()
                    .filter(|(_, claim)| claim.subject == id)
                    .map(|(claim_id, _)| claim_id.clone())
                    .collect();
                let refs = value_strings(&record.value, "chronology_claim_refs");
                let actual: BTreeSet<String> = refs.iter().cloned().collect();
                if actual != expected || expected.len() != 1 || refs.len() != actual.len() {
                    findings.push(format!(
                        "current Nietzsche Work must reference exactly one first_publication_chronology claim; found {}",
                        python_string_list(&actual.iter().cloned().collect::<Vec<_>>())
                    ));
                }
            }
            if scope.is_scoped() && record.kind == "work" {
                findings.extend(exact_backref_messages(&record.value, "chronology_claim_refs", id,
                    "chronology", chronology));
            }
            if candidate_mode {
                refresh_candidate_findings_state(
                    &findings,
                    &mut findings_state_bytes,
                    &mut temporary,
                )?;
                let temporary_without_findings = temporary
                    .checked_sub(findings_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                let map_state = candidate_responsibility_state
                    .checked_add(candidate_publication_state)
                    .ok_or(ItemRefusal::Budget)?;
                let mut pending_message_state = findings.iter().try_fold(
                    0usize,
                    |state, message| {
                        state
                            .checked_add(estimate_string_storage(message)?)
                            .ok_or(ItemRefusal::Budget)
                    },
                )?;
                let findings_capacity_state = std::mem::size_of::<std::vec::IntoIter<String>>()
                    .checked_add(
                        findings
                            .capacity()
                            .checked_mul(std::mem::size_of::<String>())
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)?;
                let location_state = estimate_string_storage(location)?;
                let mut pending = findings.into_iter();
                while let Some(message) = pending.next() {
                    let message_state = estimate_string_storage(&message)?;
                    pending_message_state = pending_message_state
                        .checked_sub(message_state)
                        .ok_or(ItemRefusal::Budget)?;
                    let message_extra = message_state
                        .checked_sub(message.len())
                        .and_then(|state| {
                            state.checked_sub(std::mem::size_of::<String>())
                        })
                        .ok_or(ItemRefusal::Budget)?;
                    let location_extra = location_state
                        .checked_sub(location.len())
                        .and_then(|state| {
                            state.checked_sub(std::mem::size_of::<String>())
                        })
                        .ok_or(ItemRefusal::Budget)?;
                    let pending_state = temporary_without_findings
                        .checked_add(map_state)
                        .and_then(|state| state.checked_add(findings_capacity_state))
                        .and_then(|state| state.checked_add(pending_message_state))
                        .and_then(|state| state.checked_add(message_extra))
                        .and_then(|state| state.checked_add(location_extra))
                        .ok_or(ItemRefusal::Budget)?;
                    push_bounded_issue(
                        issues,
                        cost,
                        retained,
                        pending_state,
                        limits,
                        cancelled,
                        location,
                        message,
                    )?;
                }
            } else {
                for message in findings {
                    push_bounded_issue(
                        issues,
                        cost,
                        retained,
                        temporary
                            .checked_add(candidate_responsibility_state)
                            .and_then(|state| state.checked_add(candidate_publication_state))
                            .ok_or(ItemRefusal::Budget)?,
                        limits,
                        cancelled,
                        location,
                        message,
                    )?;
                }
            }
            Ok(())
        })?;
        Ok(())
    }

    fn check_selected_derivation_graph(&mut self) -> Result<(), ItemRefusal> {
        if self.schema_request_store.is_some() {
            let cycle = self.check_candidate_derivation_graph()?;
            if cycle {
                self.issue(SOURCE_HOME, "Expression derivation cycle detected")?;
            }
            let subject_streams =
                self.check_candidate_derivation_backlinks(self.temporary_state_bytes)?;
            for (set, rows) in [
                (
                    SourceFoundationClosureDerivationKeySet::Endpoints,
                    self.cost
                        .candidate_derivation_store
                        .derivation_endpoint_rows,
                ),
                (
                    SourceFoundationClosureDerivationKeySet::EvidencePaths,
                    self.cost
                        .candidate_derivation_store
                        .derivation_evidence_path_rows,
                ),
                (
                    SourceFoundationClosureDerivationKeySet::ExpectedInputs,
                    self.cost
                        .candidate_derivation_store
                        .derivation_expected_input_rows,
                ),
            ] {
                self.drain_candidate_derivation_keyset(set, rows)?;
            }
            let remaining = self.remaining_state()?;
            let finished = self
                .schema_request_store
                .as_deref_mut()
                .ok_or(crate::item_budget_origin!())?
                .finish_derivation(subject_streams, remaining)?;
            self.include_store_workspace(finished.peak_workspace_state_bytes)?;
            self.cost.candidate_derivation_store = finished;
            return Ok(());
        }
        // Keys and edges borrow the already charged claim index. Admit the
        // complete simultaneous tree/stack workspace before constructing it.
        // Adjacency map, per-subject edge tree and visitation map coexist.
        let graph_entry_state = 2
            * (std::mem::size_of::<BTreeMap<&str, BTreeSet<&str>>>()
                + std::mem::size_of::<BTreeSet<&str>>()
                + std::mem::size_of::<(&str, u8)>()
                + 2 * std::mem::size_of::<(&str, bool)>()
                + 3 * 12 * std::mem::size_of::<usize>());
        let workspace = self
            .derivation
            .len()
            .checked_mul(graph_entry_state)
            .and_then(|bytes| {
                bytes.checked_add(
                    std::mem::size_of::<BTreeMap<&str, BTreeSet<&str>>>()
                        + std::mem::size_of::<BTreeMap<&str, u8>>()
                        + std::mem::size_of::<Vec<(&str, bool)>>(),
                )
            })
            .ok_or(crate::item_budget_origin!())?;
        self.reserve_temporary(workspace)?;
        let cycle = {
            let mut edges = BTreeMap::<&str, BTreeSet<&str>>::new();
            for claim in self.derivation.values() {
                edges
                    .entry(claim.subject.as_str())
                    .or_default()
                    .insert(claim.object.as_str());
            }
            directed_cycle(&edges, self.limits.deadline, self.source.cancellation())?
        };
        self.release_temporary_state(workspace)?;
        if cycle {
            self.issue(SOURCE_HOME, "Expression derivation cycle detected")?;
        }
        Ok(())
    }

    /// Selected closure follows the declared semantic family, not the
    /// historical corpus path, event identity or materialization cardinality.
    fn check_selected_specialized_claim(
        &mut self,
        location: &str,
        claim: &Value,
    ) -> Result<(), ItemRefusal> {
        let predicate = text(claim, "predicate").unwrap_or_default();
        let subject = text(claim, "subject_ref");
        let object = text(claim, "object");
        for (_, topology_predicate, subject_kind, object_kind, _, _) in TOPOLOGY_ROUTES {
            if predicate == topology_predicate {
                self.expect_ref(location, subject, subject_kind)?;
                self.expect_ref(location, object, object_kind)?;
                if predicate == "exemplified_by" {
                    if let (Some(edition), Some(item)) = (subject, object) {
                        if self.records.item_edition(item)?.as_deref() != Some(edition) {
                            self.issue(
                                location,
                                "declared edition-item topology differs from its item embodiment",
                            )?;
                        }
                    }
                }
            }
        }
        if predicate == "is_derivative_of" {
            self.expect_ref(location, subject, "expression")?;
            self.expect_ref(location, object, "expression")?;
            if self.schema_request_store.is_some() {
                if let (Some(id), Some(subject), Some(object)) =
                    (text(claim, "claim_id"), subject, object)
                {
                    self.remember_candidate_derivation_subject(id, subject)?;
                    self.remember_candidate_derivation_key(
                        SourceFoundationClosureDerivationKeySet::Endpoints,
                        subject,
                    )?;
                    self.remember_candidate_derivation_key(
                        SourceFoundationClosureDerivationKeySet::Endpoints,
                        object,
                    )?;
                    if !self.remember_candidate_derivation_pair(subject, object)? {
                        self.issue(location, "duplicate Expression derivation pair")?;
                    }
                }
            }
            if subject == object {
                self.issue(location, "Expression derivation is irreflexive")?;
            }
            self.request_schema(
                &format!("{location}['qualifiers']"),
                DERIVATION_SCHEMA,
                claim.get("qualifiers").unwrap_or(&Value::Null),
            )?;
        }
        if predicate == "first_publication_chronology" {
            self.expect_ref(location, subject, "work")?;
            let chronology = claim.get("object").unwrap_or(&Value::Null);
            self.request_schema(
                &format!("{location}['object']"),
                CHRONOLOGY_SCHEMA,
                chronology,
            )?;
            let interval = chronology.get("interval").unwrap_or(&Value::Null);
            if text(interval, "start")
                .zip(text(interval, "end"))
                .is_some_and(|(start, end)| start > end)
            {
                self.issue(location, "work chronology interval starts after it ends")?;
            }
            let mut prior_date: Option<&str> = None;
            for stage in chronology
                .get("stages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(date) = text(stage, "date") {
                    if prior_date.is_some_and(|prior| prior > date) {
                        self.issue(location, "work chronology stages are not ordered")?;
                    }
                    prior_date = Some(date);
                }
                if let Some(edition) = text(stage, "edition_ref") {
                    self.expect_ref(location, Some(edition), "edition")?;
                    let (record, workspace) = self.current_record_with_state_budget(edition)?;
                    if let Some(record) = record.as_ref() {
                        if text(&record.value, "work_ref") != subject {
                            self.issue(
                                location,
                                "chronology stage edition belongs to another Work",
                            )?;
                        }
                    }
                    drop(record);
                    self.release_temporary_state(workspace)?;
                }
            }
        }
        Ok(())
    }

    fn register_claim(
        &mut self,
        path: &str,
        line: usize,
        claim: &Value,
        native: bool,
    ) -> Result<(), ItemRefusal> {
        let location = format!("{path}:{line}");
        let (id, claim_id_state_bytes) = self.claim_id(&location, claim)?;
        let mut candidate_claim_fields_state_bytes = 0usize;
        let mut candidate_membership_state_bytes = 0usize;
        let result = (|| {
            if self.scope.is_scoped() {
                self.check_selected_specialized_claim(&location, claim)?;
            }
            let subject = text(claim, "subject_ref").unwrap_or_default();
            let predicate = text(claim, "predicate").unwrap_or_default();
            let object = text(claim, "object").unwrap_or_default();
            let event = text(claim, "provenance_event_ref").unwrap_or_default();

            if !event.is_empty() && !self.event_exists(&event)? {
                self.issue(
                    &location,
                    format!("unresolved provenance_event_ref: {event}"),
                )?;
            }
            for evidence in value_strings(claim, "evidence_refs") {
                if evidence.starts_with("tos.anchor.") && !self.anchor_id_exists(&evidence)? {
                    self.issue(
                        &location,
                        format!("unresolved source evidence anchor: {evidence}"),
                    )?;
                } else if evidence.starts_with("ToS/") && !self.current_exists(&evidence)? {
                    self.issue(
                        &location,
                        format!("unresolved repository evidence ref: {evidence}"),
                    )?;
                }
            }
            let Some(id) = id else {
                return Ok(());
            };
            let claim_fields_state_bytes = subject
                .len()
                .checked_add(predicate.len())
                .and_then(|bytes| bytes.checked_add(object.len()))
                .and_then(|bytes| bytes.checked_add(event.len()))
                .and_then(|bytes| bytes.checked_add(location.len()))
                .and_then(|bytes| bytes.checked_add(128))
                .ok_or(ItemRefusal::Budget)?;
            if self.schema_request_store.is_some() {
                candidate_claim_fields_state_bytes = claim_fields_state_bytes;
                self.reserve_temporary(candidate_claim_fields_state_bytes)?;
            } else {
                self.reserve(claim_fields_state_bytes)?;
            }
            let membership_applicable =
                path.ends_with("/membership-claims.jsonl") || predicate == "contains_work";
            if membership_applicable && self.schema_request_store.is_some() {
                candidate_membership_state_bytes = subject
                    .len()
                    .checked_add(predicate.len())
                    .and_then(|bytes| bytes.checked_add(object.len()))
                    .and_then(|bytes| bytes.checked_add(event.len()))
                    .and_then(|bytes| bytes.checked_add(location.len()))
                    .and_then(|bytes| {
                        bytes.checked_add(std::mem::size_of::<SourceFoundationClosureClaimRef>())
                    })
                    .ok_or(ItemRefusal::Budget)?;
                self.reserve_temporary(candidate_membership_state_bytes)?;
            }
            let subject = subject.to_owned();
            let predicate = predicate.to_owned();
            let object = object.to_owned();
            let event = event.to_owned();
            let reference = SourceFoundationClosureClaimRef {
                location: location.clone(),
                subject: subject.clone(),
                predicate: predicate.clone(),
                object: object.clone(),
                event: event.clone(),
                native,
            };

            if membership_applicable {
                self.expect_ref(&location, Some(&subject), "collection")?;
                self.expect_ref(&location, Some(&object), "work")?;
                if self.schema_request_store.is_some() {
                    self.remember_membership_claim(&id, &subject)?;
                } else {
                    self.reserve(claim_reference_index_state(&id, &reference)?)?;
                    self.membership.insert(id.clone(), reference.clone());
                }
            }
            if path.ends_with("/responsibility-claims.jsonl")
                || matches!(
                    predicate.as_str(),
                    "authored_by"
                        | "contributed_by"
                        | "translated_by"
                        | "edited_by"
                        | "afterword_by"
                        | "designed_by"
                )
            {
                let expected_subject = match predicate.as_str() {
                    "authored_by" | "contributed_by" => "work",
                    "translated_by" => "expression",
                    "edited_by" | "afterword_by" | "designed_by" => "edition",
                    _ => "",
                };
                if !expected_subject.is_empty() {
                    self.expect_ref(&location, Some(&subject), expected_subject)?;
                }
                self.expect_ref(&location, Some(&object), "agent")?;
                if self.schema_request_store.is_some() {
                    self.remember_candidate_responsibility_claim(&id, &reference)?;
                } else {
                    self.reserve(claim_reference_index_state(&id, &reference)?)?;
                    self.responsibility.insert(id.clone(), reference.clone());
                }
            }
            if path.ends_with("/publication-claims.jsonl") {
                self.expect_ref(&location, Some(&subject), "edition")?;
                if self.schema_request_store.is_some() {
                    self.remember_candidate_publication_claim(&id, &reference)?;
                } else {
                    self.reserve(claim_reference_index_state(&id, &reference)?)?;
                    self.publication.insert(id.clone(), reference.clone());
                }
            }
            if path.ends_with("/provision-activity-claims.jsonl") {
                self.expect_ref(&location, Some(&subject), "edition")?;
                if self.schema_request_store.is_some() {
                    self.remember_candidate_provision_claim(&id, &reference)?;
                } else {
                    self.reserve(crate::record_biblio_cut::decoded_state(claim)?)?;
                    self.reserve(
                        id.len()
                            + std::mem::size_of::<String>()
                            + std::mem::size_of::<Value>()
                            + 4 * std::mem::size_of::<usize>(),
                    )?;
                    self.reserve(claim_reference_index_state(&id, &reference)?)?;
                    self.provision_values.insert(id.clone(), claim.clone());
                    self.provision.insert(id.clone(), reference.clone());
                }
            }
            if path == CHRONOLOGY_CLAIMS
                || self.scope.is_scoped() && predicate == "first_publication_chronology"
            {
                self.expect_ref(&location, Some(&subject), "work")?;
                self.reserve(claim_reference_index_state(&id, &reference)?)?;
                self.chronology.insert(id.clone(), reference.clone());
            }
            if path.ends_with("/object-link-claims.jsonl")
                || claim.get("schema_version").and_then(Value::as_str)
                    == Some("tos_object_link_claim_v2")
            {
                if self.schema_request_store.is_some() {
                    self.remember_candidate_object_link_claim(&id, &reference)?;
                } else {
                    self.reserve(claim_reference_index_state(&id, &reference)?)?;
                    self.object_links.insert(id.clone(), reference.clone());
                }
            }
            if TOPOLOGY_ROUTES.iter().any(|(route, ..)| *route == path)
                || matches!(
                    predicate.as_str(),
                    "has_expression" | "embodied_by" | "exemplified_by"
                )
            {
                if self.schema_request_store.is_some() {
                    self.remember_candidate_topology_claim(&id, &reference)?;
                } else {
                    self.reserve(claim_reference_index_state(&id, &reference)?)?;
                    self.topology.insert(id.clone(), reference.clone());
                }
            }
            if path == DERIVATION_CLAIMS || predicate == "is_derivative_of" {
                if self.schema_request_store.is_some() {
                    let remaining = self.remaining_state()?;
                    let (inserted, workspace) = self
                        .schema_request_store
                        .as_deref_mut()
                        .ok_or(ItemRefusal::Budget)?
                        .remember_derivation_id(&id, remaining)?;
                    self.include_store_workspace(workspace)?;
                    if inserted {
                        self.cost.candidate_derivation_store.derivation_id_rows = self
                            .cost
                            .candidate_derivation_store
                            .derivation_id_rows
                            .checked_add(1)
                            .ok_or(ItemRefusal::Budget)?;
                    }
                } else {
                    self.reserve(claim_reference_index_state(&id, &reference)?)?;
                    self.derivation.insert(id.clone(), reference);
                }
            }
            Ok(())
        })();
        let membership_release = self.release_loaded_rows(candidate_membership_state_bytes);
        let claim_fields_release = self.release_loaded_rows(candidate_claim_fields_state_bytes);
        let release = self.release_loaded_rows(claim_id_state_bytes);
        result?;
        membership_release?;
        claim_fields_release?;
        release
    }

    fn validate_source_refs(&mut self, location: &str, value: &Value) -> Result<(), ItemRefusal> {
        for field in ["inputs", "outputs", "evidence_refs"] {
            let Some(rows) = value.get(field).and_then(Value::as_array) else {
                continue;
            };
            for row in rows {
                let (reference, expected_digest) = if let Some(object) = row.as_object() {
                    (
                        object.get("ref").and_then(Value::as_str),
                        object.get("sha256").and_then(Value::as_str),
                    )
                } else {
                    (row.as_str(), None)
                };
                let Some(reference) = reference.filter(|reference| reference.starts_with("ToS/"))
                else {
                    continue;
                };
                if !self.current_exists(reference)? {
                    self.issue(location, format!("unresolved source ref: {reference}"))?;
                    continue;
                }
                if let Some(expected_digest) = expected_digest {
                    if !self.recorded_matches(reference, expected_digest)? {
                        self.issue(
                            location,
                            format!("source ref digest is unresolved: {reference}"),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn check_boundary_maps_and_anchors(&mut self) -> Result<(), ItemRefusal> {
        let map_paths = self.collect_current_paths(
            |path| path.ends_with("/work-boundary-map.json"),
            "source-foundation closure boundary-map path index",
        )?;
        let mut boundary_anchor_ids = BTreeSet::new();
        for map_path in &map_paths {
            let Some(loaded) = self.json_rows(map_path, BOUNDARY_MAP_SCHEMA, true)? else {
                continue;
            };
            let loaded_state_bytes = loaded.temporary_state_bytes;
            let Some((_, boundary_map)) = loaded.rows.first() else {
                drop(loaded);
                self.release_loaded_rows(loaded_state_bytes)?;
                continue;
            };
            self.expect_ref(map_path, text(boundary_map, "collection_ref"), "collection")?;
            self.expect_ref(map_path, text(boundary_map, "edition_ref"), "edition")?;
            self.expect_ref(map_path, text(boundary_map, "item_ref"), "item")?;

            if let Some(inventory_ref) = text(boundary_map, "resource_inventory_ref") {
                let inventory_ref = inventory_ref.to_owned();
                let expected = text(boundary_map, "resource_inventory_sha256")
                    .unwrap_or_default()
                    .to_owned();
                if !self.current_exists(&inventory_ref)? {
                    self.issue(
                        map_path,
                        format!("work-boundary resource inventory is missing: {inventory_ref}"),
                    )?;
                } else if self.digest_for(&inventory_ref)?.as_deref() != Some(expected.as_str()) {
                    self.issue(map_path, "work-boundary resource inventory digest drifted")?;
                }
            }

            let item_ref = boundary_map.get("item_ref").unwrap_or(&Value::Null);
            let file_id = boundary_map.get("file_id").unwrap_or(&Value::Null);
            if !self.records.file_contains(item_ref, file_id)? {
                self.issue(map_path, "work-boundary file does not belong to its item")?;
            }
            let map_sha = boundary_map.get("file_sha256").unwrap_or(&Value::Null);
            let manifest_matches = {
                let manifest_sha = self
                    .records
                    .file_sha256(file_id)?
                    .unwrap_or(Cow::Borrowed(&Value::Null));
                self.python_equal(&manifest_sha, map_sha)?
            };
            if !manifest_matches {
                self.issue(
                    map_path,
                    "work-boundary file digest differs from the item manifest",
                )?;
            }
            if !self.event_exists(text(boundary_map, "provenance_event_ref").unwrap_or_default())? {
                self.issue(map_path, "work-boundary provenance event is unresolved")?;
            }

            let candidate_local_indexes = self.schema_request_store.is_some();
            let local_indexes_baseline = self.temporary_state_bytes;
            if candidate_local_indexes {
                let anchor_path_bytes = map_path
                    .rsplit_once('/')
                    .map(|(parent, _)| {
                        parent
                            .len()
                            .checked_add("/anchors.jsonl".len())
                            .ok_or(ItemRefusal::Budget)
                    })
                    .transpose()?
                    .unwrap_or("anchors.jsonl".len());
                let anchor_path_state = anchor_path_bytes
                    .checked_mul(2)
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>() + 32))
                    .ok_or(ItemRefusal::Budget)?;
                self.reserve_temporary(
                    anchor_path_state
                        .checked_add(std::mem::size_of::<BTreeSet<String>>())
                        .and_then(|bytes| {
                            bytes.checked_add(std::mem::size_of::<BTreeMap<String, u64>>())
                        })
                        .ok_or(ItemRefusal::Budget)?,
                )?;
            }
            let anchor_path = map_path
                .rsplit_once('/')
                .map(|(parent, _)| format!("{parent}/anchors.jsonl"))
                .unwrap_or_else(|| "anchors.jsonl".to_owned());
            let anchors = self.json_rows(&anchor_path, ANCHOR_SCHEMA, true)?;
            let mut local_ids = BTreeSet::new();
            let mut page_by_id = BTreeMap::new();
            if let Some(anchors) = anchors {
                let anchors_state_bytes = anchors.temporary_state_bytes;
                for (line, anchor) in anchors.rows {
                    let location = format!("{anchor_path}:{line}");
                    self.register_anchor(
                        &location,
                        &anchor,
                        &mut boundary_anchor_ids,
                        &mut local_ids,
                        &mut page_by_id,
                    )?;
                    if !self
                        .python_equal(anchor.get("item_id").unwrap_or(&Value::Null), item_ref)?
                    {
                        self.issue(&location, "boundary anchor item_id differs from map")?;
                    }
                    if !self.python_equal(anchor.get("file_id").unwrap_or(&Value::Null), file_id)? {
                        self.issue(&location, "boundary anchor file_id differs from map")?;
                    }
                    if text(&anchor, "file_sha256") != text(boundary_map, "file_sha256") {
                        self.issue(&location, "boundary anchor file digest differs from map")?;
                    }
                    if text(&anchor, "provenance_event_ref")
                        != text(boundary_map, "provenance_event_ref")
                    {
                        self.issue(&location, "boundary anchor provenance differs from map")?;
                    }
                    if page_by_id
                        .get(text(&anchor, "anchor_id").unwrap_or_default())
                        .is_some_and(|page| {
                            boundary_map
                                .get("page_count")
                                .and_then(Value::as_u64)
                                .is_some_and(|count| *page > count)
                        })
                    {
                        self.issue(&location, "boundary anchor page exceeds page_count")?;
                    }
                }
                self.release_loaded_rows(anchors_state_bytes)?;
            }

            let members = boundary_map
                .get("members")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let sequences: Vec<u64> = members
                .iter()
                .filter_map(|member| member.get("sequence").and_then(Value::as_u64))
                .collect();
            if sequences != (1..=members.len() as u64).collect::<Vec<_>>() {
                self.issue(
                    map_path,
                    "work-boundary member sequence is not contiguous from 1",
                )?;
            }
            let coverage = text(boundary_map, "coverage_posture");
            let explicit_coverage = coverage.is_some();
            let source_sequences: Vec<u64> = members
                .iter()
                .filter_map(|member| member.get("source_sequence").and_then(Value::as_u64))
                .collect();
            if coverage == Some("partial_membership_representation")
                && (source_sequences.len() != members.len()
                    || source_sequences.windows(2).any(|pair| pair[0] >= pair[1]))
            {
                self.issue(map_path, "partial work-boundary source_sequence values must be present, strictly increasing and unique")?;
            }
            let mut previous_end = None;
            let mut represented_ranges: Vec<(u64, u64, String)> = Vec::new();
            for member in &members {
                self.check_boundary_member(
                    map_path,
                    member,
                    &local_ids,
                    &page_by_id,
                    &mut previous_end,
                    explicit_coverage,
                    &mut represented_ranges,
                )?;
            }
            let non_member_sections = boundary_map
                .get("non_member_sections")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if explicit_coverage {
                previous_end = None;
            }
            for section in &non_member_sections {
                self.check_boundary_section(
                    map_path,
                    section,
                    &local_ids,
                    &page_by_id,
                    "non-member",
                    &mut previous_end,
                    &mut represented_ranges,
                )?;
            }
            let unrepresented = boundary_map
                .get("unrepresented_sections")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if coverage == Some("complete_membership_representation") && !unrepresented.is_empty() {
                self.issue(
                    map_path,
                    "complete work-boundary representation cannot contain unrepresented sections",
                )?;
            }
            if coverage == Some("partial_membership_representation") && unrepresented.is_empty() {
                self.issue(map_path, "partial work-boundary representation requires at least one unrepresented section")?;
            }
            for section in &unrepresented {
                self.check_boundary_section(
                    map_path,
                    section,
                    &local_ids,
                    &page_by_id,
                    "unrepresented",
                    &mut previous_end,
                    &mut represented_ranges,
                )?;
            }
            if explicit_coverage {
                represented_ranges.sort();
                let mut coverage_end = 0u64;
                for (start, end, label) in represented_ranges {
                    if start != coverage_end.saturating_add(1) {
                        self.issue(map_path, format!("explicit work-boundary coverage has a gap or overlap before {label}"))?;
                    }
                    coverage_end = coverage_end.max(end);
                }
                if Some(coverage_end) != boundary_map.get("page_count").and_then(Value::as_u64) {
                    self.issue(
                        map_path,
                        "explicit work-boundary coverage does not cover the exact page_count",
                    )?;
                }
            } else if previous_end != boundary_map.get("page_count").and_then(Value::as_u64) {
                self.issue(
                    map_path,
                    "work and non-member boundaries do not cover the exact page_count",
                )?;
            }
            for reference in value_strings(boundary_map, "crosscheck_anchor_refs") {
                if !local_ids.contains(&reference) {
                    self.issue(
                        map_path,
                        format!("unresolved boundary crosscheck anchor: {reference}"),
                    )?;
                }
            }
            let candidate_membership_refs = self.schema_request_store.is_some();
            let mut membership_refs = BTreeSet::new();
            for member in members {
                if let Some(reference) = text(&member, "membership_claim_ref") {
                    if !candidate_membership_refs {
                        self.reserve(
                            reference.len()
                                + std::mem::size_of::<String>()
                                + 4 * std::mem::size_of::<usize>(),
                        )?;
                        membership_refs.insert(reference.to_owned());
                    }
                }
                if let Some(reference) = text(&member, "responsibility_claim_ref")
                    .or_else(|| text(&member, "translation_responsibility_claim_ref"))
                {
                    self.remember_boundary_responsibility_ref(reference, true)?;
                }
            }
            if !candidate_membership_refs {
                for reference in membership_refs {
                    if !self.boundary_membership_refs.contains(&reference) {
                        self.reserve(
                            reference.len()
                                + std::mem::size_of::<String>()
                                + 4 * std::mem::size_of::<usize>(),
                        )?;
                        self.boundary_membership_refs.insert(reference);
                    }
                }
            }
            drop(page_by_id);
            drop(local_ids);
            drop(anchor_path);
            if candidate_local_indexes {
                self.release_temporary_since(local_indexes_baseline);
            }
            drop(loaded);
            self.release_loaded_rows(loaded_state_bytes)?;
        }

        let non_boundary_anchor_paths = self.collect_current_paths(
            |path| {
                if !path.ends_with("/anchors.jsonl") {
                    return false;
                }
                let map_path = path
                    .rsplit_once('/')
                    .map(|(parent, _)| format!("{parent}/work-boundary-map.json"));
                !map_path.is_some_and(|candidate| map_paths.contains(&candidate))
            },
            "source-foundation closure non-boundary anchor path index",
        )?;
        let mut evidence_anchor_ids = boundary_anchor_ids;
        for anchor_path in non_boundary_anchor_paths {
            if let Some(loaded) = self.json_rows(&anchor_path, ANCHOR_SCHEMA, false)? {
                let loaded_state_bytes = loaded.temporary_state_bytes;
                for (line, anchor) in loaded.rows {
                    let location = format!("{anchor_path}:{line}");
                    if let Some(id) = text(&anchor, "anchor_id") {
                        let duplicate = if self.schema_request_store.is_some() {
                            !self.remember_candidate_anchor_id(id)?
                        } else {
                            self.reserve(
                                2 * (id.len()
                                    + std::mem::size_of::<String>()
                                    + 4 * std::mem::size_of::<usize>()),
                            )?;
                            !evidence_anchor_ids.insert(id.to_owned())
                        };
                        if duplicate {
                            self.issue(
                                &location,
                                format!("duplicate source evidence anchor_id: {id}"),
                            )?;
                        }
                        if self.schema_request_store.is_none() {
                            self.anchors.insert(id.to_owned());
                        }
                    }
                    self.expect_ref(&location, text(&anchor, "item_id"), "item")?;
                    if let Some(event_ref) = text(&anchor, "provenance_event_ref") {
                        if !self.event_exists(event_ref)? {
                            self.issue(
                                &location,
                                format!("unresolved source-anchor provenance event: {event_ref}"),
                            )?;
                        }
                    }
                }
                self.release_loaded_rows(loaded_state_bytes)?;
            }
        }
        if self.schema_request_store.is_some() {
            self.finish_candidate_anchor_ids()?;
        } else {
            for id in evidence_anchor_ids {
                if !self.anchors.contains(&id) {
                    self.reserve(
                        id.len() + std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>(),
                    )?;
                    self.anchors.insert(id);
                }
            }
        }
        Ok(())
    }

    fn register_anchor(
        &mut self,
        location: &str,
        anchor: &Value,
        all_ids: &mut BTreeSet<String>,
        local_ids: &mut BTreeSet<String>,
        page_by_id: &mut BTreeMap<String, u64>,
    ) -> Result<(), ItemRefusal> {
        let Some(id) = text(anchor, "anchor_id") else {
            return Ok(());
        };
        let per_id_state = id
            .len()
            .checked_add(std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>())
            .ok_or(ItemRefusal::Budget)?;
        let candidate_local_indexes = self.schema_request_store.is_some();
        let page_selectors = anchor
            .get("selectors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|selector| text(selector, "type") == Some("page_region"));
        let mut page_selector_count = 0usize;
        let mut page = None;
        for selector in page_selectors {
            page_selector_count = page_selector_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            if page_selector_count == 1 {
                page = selector.get("page").and_then(Value::as_u64);
            }
        }
        let page_is_inserted = page_selector_count == 1 && page.is_some();
        let local_key_exists = local_ids.contains(id);
        let page_key_exists = page_by_id.contains_key(id);
        let page_entry_state = per_id_state
            .checked_add(std::mem::size_of::<u64>())
            .ok_or(ItemRefusal::Budget)?;
        let candidate_precharge = per_id_state
            .checked_add(page_entry_state)
            .ok_or(ItemRefusal::Budget)?;
        if candidate_local_indexes {
            self.reserve_temporary(candidate_precharge)?;
        } else {
            self.reserve(per_id_state.checked_mul(3).ok_or(ItemRefusal::Budget)?)?;
        }
        let duplicate = if self.schema_request_store.is_some() {
            !self.remember_candidate_anchor_id(id)?
        } else {
            !all_ids.insert(id.to_owned())
        };
        if duplicate {
            self.issue(location, format!("duplicate boundary anchor_id: {id}"))?;
        }
        local_ids.insert(id.to_owned());
        if page_is_inserted {
            page_by_id.insert(id.to_owned(), page.ok_or(ItemRefusal::Budget)?);
        }
        if page_selector_count != 1 {
            self.issue(
                location,
                "boundary anchor must have exactly one page selector",
            )?;
        }
        if candidate_local_indexes {
            let retained_state = if local_key_exists { 0 } else { per_id_state }
                .checked_add(if page_is_inserted && !page_key_exists {
                    page_entry_state
                } else {
                    0
                })
                .ok_or(ItemRefusal::Budget)?;
            self.release_temporary_state(
                candidate_precharge
                    .checked_sub(retained_state)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
        }
        Ok(())
    }

    fn check_boundary_member(
        &mut self,
        location: &str,
        member: &Value,
        local_ids: &BTreeSet<String>,
        page_by_id: &BTreeMap<String, u64>,
        previous_end: &mut Option<u64>,
        explicit_coverage: bool,
        represented_ranges: &mut Vec<(u64, u64, String)>,
    ) -> Result<(), ItemRefusal> {
        self.expect_ref(location, text(member, "work_ref"), "work")?;
        self.expect_ref(location, text(member, "expression_ref"), "expression")?;
        if let (Some(work_ref), Some(expression_ref)) =
            (text(member, "work_ref"), text(member, "expression_ref"))
        {
            let (expression, expression_workspace) =
                self.current_record_with_state_budget(expression_ref)?;
            let belongs_to_work = expression
                .as_ref()
                .is_some_and(|record| text(&record.value, "work_ref") == Some(work_ref));
            drop(expression);
            self.release_temporary_state(expression_workspace)?;
            if !belongs_to_work {
                self.issue(
                    location,
                    format!("work-boundary expression belongs to another work: {expression_ref}"),
                )?;
            }
        }
        let start = member.get("start_page").and_then(Value::as_u64);
        let end = member.get("end_page").and_then(Value::as_u64);
        if let (Some(start), Some(end)) = (start, end) {
            if start > end {
                self.issue(
                    location,
                    format!(
                        "work-boundary start exceeds end for sequence {}",
                        member.get("sequence").unwrap_or(&Value::Null)
                    ),
                )?;
            }
            if !explicit_coverage
                && previous_end.is_some_and(|previous| previous.checked_add(1) != Some(start))
            {
                self.issue(
                    location,
                    format!(
                        "work-boundary members are not contiguous at sequence {}",
                        member.get("sequence").unwrap_or(&Value::Null)
                    ),
                )?;
            }
            *previous_end = Some(end);
            represented_ranges.push((
                start,
                end,
                format!(
                    "member sequence {}",
                    member
                        .get("sequence")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                ),
            ));
        }
        self.check_boundary_anchor_ref(
            location,
            text(member, "title_page_anchor_ref"),
            start,
            local_ids,
            page_by_id,
            "title-page",
        )?;
        for anchor_ref in value_strings(member, "boundary_evidence_anchor_refs") {
            if !local_ids.contains(&anchor_ref) {
                self.issue(
                    location,
                    format!("unresolved member boundary anchor: {anchor_ref}"),
                )?;
            }
        }
        if let Some(reference) = text(member, "membership_claim_ref") {
            self.remember_boundary_membership_ref(reference)?;
        }
        if let Some(reference) = text(member, "responsibility_claim_ref")
            .or_else(|| text(member, "translation_responsibility_claim_ref"))
        {
            self.remember_boundary_responsibility_ref(reference, false)?;
        }
        Ok(())
    }

    fn check_boundary_section(
        &mut self,
        location: &str,
        section: &Value,
        local_ids: &BTreeSet<String>,
        page_by_id: &BTreeMap<String, u64>,
        label: &str,
        previous_end: &mut Option<u64>,
        represented_ranges: &mut Vec<(u64, u64, String)>,
    ) -> Result<(), ItemRefusal> {
        let start = section.get("start_page").and_then(Value::as_u64);
        let end = section.get("end_page").and_then(Value::as_u64);
        if label != "unrepresented" {
            if let Some(start) = start {
                if previous_end.is_some_and(|previous| previous.checked_add(1) != Some(start)) {
                    self.issue(
                        location,
                        format!(
                            "{label} section is not contiguous: {}",
                            text(section, "label").unwrap_or("")
                        ),
                    )?;
                }
            }
        }
        if let (Some(start), Some(end)) = (start, end) {
            if start > end {
                self.issue(
                    location,
                    format!(
                        "{label} section start exceeds end: {}",
                        text(section, "label").unwrap_or("")
                    ),
                )?;
            }
            *previous_end = Some(end);
            represented_ranges.push((
                start,
                end,
                format!("{label} section {}", text(section, "label").unwrap_or("")),
            ));
        }
        self.check_boundary_anchor_ref(
            location,
            text(section, "boundary_anchor_ref"),
            start,
            local_ids,
            page_by_id,
            label,
        )?;
        Ok(())
    }

    fn check_boundary_anchor_ref(
        &mut self,
        location: &str,
        anchor_ref: Option<&str>,
        start_page: Option<u64>,
        local_ids: &BTreeSet<String>,
        page_by_id: &BTreeMap<String, u64>,
        label: &str,
    ) -> Result<(), ItemRefusal> {
        let Some(anchor_ref) = anchor_ref else {
            self.issue(location, format!("missing {label} anchor reference"))?;
            return Ok(());
        };
        if !local_ids.contains(anchor_ref) {
            self.issue(location, format!("unresolved {label} anchor: {anchor_ref}"))?;
        } else if page_by_id.get(anchor_ref).copied() != start_page {
            self.issue(
                location,
                format!("{label} anchor does not match start_page: {anchor_ref}"),
            )?;
        }
        Ok(())
    }
}

fn loaded_clone_cost(rows: &LoadedRows) -> Result<usize, ItemRefusal> {
    let state = rows.rows.iter().try_fold(
        rows.digest
            .len()
            .checked_add(std::mem::size_of::<LoadedRows>())
            .ok_or(ItemRefusal::Budget)?,
        |used, (line, value)| {
            used.checked_add(std::mem::size_of::<(usize, Value)>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(line)))
                .and_then(|bytes| {
                    crate::record_biblio_cut::decoded_state(value)
                        .ok()
                        .and_then(|size| bytes.checked_add(size))
                })
                .ok_or(ItemRefusal::Budget)
        },
    )?;
    let spare_rows = rows
        .rows
        .capacity()
        .checked_sub(rows.rows.len())
        .ok_or(ItemRefusal::Budget)?;
    state
        .checked_add(
            spare_rows
                .checked_mul(std::mem::size_of::<(usize, Value)>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)
}

fn check(deadline: Instant, cancelled: &std::sync::atomic::AtomicBool) -> Result<(), ItemRefusal> {
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "source-foundation closure cancelled".into(),
        ));
    }
    Ok(())
}

fn assessment_refusal(error: crate::assessment::AssessmentRefusal) -> ItemRefusal {
    use crate::assessment::AssessmentRefusal;
    match error {
        AssessmentRefusal::Budget => ItemRefusal::Budget,
        AssessmentRefusal::Deadline => ItemRefusal::Deadline,
        AssessmentRefusal::Cancelled => {
            ItemRefusal::Source("source-foundation closure cancelled during Python equality".into())
        }
        AssessmentRefusal::Schema(ItemRefusal::Budget) => ItemRefusal::Budget,
        AssessmentRefusal::Schema(ItemRefusal::BudgetCheck { check, used, limit }) => {
            ItemRefusal::BudgetCheck { check, used, limit }
        }
        AssessmentRefusal::Schema(ItemRefusal::Deadline) => ItemRefusal::Deadline,
        AssessmentRefusal::Schema(error @ ItemRefusal::Executor(_)) => error,
        AssessmentRefusal::Schema(ItemRefusal::Source(_)) => {
            ItemRefusal::Source("source-foundation equality source check failed".into())
        }
        AssessmentRefusal::Schema(ItemRefusal::Unsupported(_))
        | AssessmentRefusal::Unsupported(_) => ItemRefusal::Unsupported(
            "source-foundation Python equality profile is unsupported".into(),
        ),
        AssessmentRefusal::InvalidInput(_) => ItemRefusal::Unsupported(
            "source-foundation Python equality input is outside its maintained profile".into(),
        ),
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn value_strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn value_strings_workspace(value: &Value, key: &str) -> Result<usize, ItemRefusal> {
    let references = value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut state = std::mem::size_of::<Vec<String>>()
        .checked_add(
            references
                .len()
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    for reference in references.iter().filter_map(Value::as_str) {
        state = state
            .checked_add(estimate_string_storage(reference)?)
            .ok_or(ItemRefusal::Budget)?;
    }
    Ok(state)
}

fn link_validation_workspace(
    value: &Value,
    targets: &BTreeMap<String, String>,
) -> Result<usize, ItemRefusal> {
    let mut reference_bytes = std::mem::size_of::<Vec<String>>();
    if let Some(references) = value
        .get("association_claim_refs")
        .and_then(Value::as_array)
    {
        for reference in references.iter().filter_map(Value::as_str) {
            reference_bytes = reference_bytes
                .checked_add(reference.len())
                .and_then(|bytes| {
                    bytes.checked_add(
                        std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>() + 96,
                    )
                })
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    let target_bytes =
        targets
            .iter()
            .try_fold(std::mem::size_of::<Vec<String>>(), |state, (id, _)| {
                state
                    .checked_add(id.len())
                    .and_then(|bytes| {
                        bytes.checked_add(
                            std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>() + 96,
                        )
                    })
                    .ok_or(ItemRefusal::Budget)
            })?;
    reference_bytes
        .checked_mul(5)
        .and_then(|bytes| bytes.checked_add(target_bytes.checked_mul(3)?))
        .and_then(|bytes| bytes.checked_add(512))
        .ok_or(ItemRefusal::Budget)
}

fn link_validation_candidate_workspace(value: &Value) -> Result<usize, ItemRefusal> {
    let mut reference_bytes = std::mem::size_of::<Vec<String>>();
    if let Some(references) = value
        .get("association_claim_refs")
        .and_then(Value::as_array)
    {
        for reference in references.iter().filter_map(Value::as_str) {
            reference_bytes = reference_bytes
                .checked_add(reference.len())
                .and_then(|bytes| {
                    bytes.checked_add(
                        std::mem::size_of::<String>() + 4 * std::mem::size_of::<usize>() + 96,
                    )
                })
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    reference_bytes
        .checked_mul(5)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<String>>() + 512))
        .ok_or(ItemRefusal::Budget)
}

fn python_string_list(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| python_string_repr(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
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

fn push_python_string_repr(rendered: &mut String, value: &str) {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
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
            value if value.is_control() => {
                rendered.push_str("\\u");
                let codepoint = value as u32;
                let digits = if codepoint > 0xffff {
                    (32 - codepoint.leading_zeros()).div_ceil(4) as usize
                } else {
                    4
                };
                for shift in (0..digits).rev() {
                    let nibble = ((codepoint >> (shift * 4)) & 0xf) as u8;
                    rendered.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
                }
            }
            value => rendered.push(value),
        }
    }
    rendered.push(quote);
}

fn json_parse_reason(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Io => "I/O error",
        serde_json::error::Category::Syntax => "syntax error",
        serde_json::error::Category::Data => "data error",
        serde_json::error::Category::Eof => "incomplete input",
    }
}

fn output_binds(event: &Value, reference: &str, role: &str, digest: &str) -> bool {
    event
        .get("outputs")
        .and_then(Value::as_array)
        .is_some_and(|outputs| {
            outputs.iter().any(|output| {
                text(output, "ref") == Some(reference)
                    && text(output, "role") == Some(role)
                    && text(output, "sha256") == Some(digest)
            })
        })
}
