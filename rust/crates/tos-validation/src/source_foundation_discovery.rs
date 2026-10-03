//! Maintained discovery, Artifact, scholarly-composite, access-request and
//! server-plan checks over an exact current source membership.
//!
//! This district produces ordered diagnostics and schema requests. Schema
//! execution belongs to the caller's diagnostic-v2 route; this module never
//! runs the older boolean schema path in parallel.

use crate::executor::schema_diagnostics::{
    Failure as DiagnosticFailure, Status as DiagnosticStatus,
};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::layer_family_rules::LayerFamilySource;
use crate::native_compound::{NativeRecordHistoryReadObservation, NativeTransportState};
use crate::record_biblio_cut::{
    BiblioCurrentRecord, SourceCutInput, SourceCutInputCoverage, SourceCutInputWithIdentity,
    SourceCutRecordReport,
};
use crate::source_foundation_default_rules::{
    SourceFoundationDefaultEventLookup, SourceFoundationDefaultPaths,
    SourceFoundationDefaultRecordsLookup,
};
use crate::source_foundation_records::{
    SourceFoundationArtifactRecordPathSummary, SourceFoundationRecordsCollection,
    SourceFoundationRecordsCursor, SourceFoundationRecordsPageBudget,
    SourceFoundationRecordsStoredFact, SourceFoundationRecordsStreamedReport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonEmissionProfile, JsonLimits, JsonMode,
    JsonValue, RelativePath, SourceRevision, canonical_bytes_v1, emit_json_profile,
    emit_value_preserved_json, parse_json,
};
use tos_source_store::{CorpusCutReader, SourceMembershipV1, SourcePresenceV1};

const SOURCE_HOME: &str = "ToS/source-witnesses/";
const DISCOVERY_RUNS: &str = "ToS/source-witnesses/discovery/runs/";
const DISCOVERY_EVENTS: &str = "ToS/source-witnesses/discovery/provenance.jsonl";
const ACCESS_EVENTS: &str = "ToS/source-witnesses/access-requests/provenance.jsonl";
const SERVER_EVENTS: &str = "ToS/source-witnesses/server-import/provenance.jsonl";
const ARTIFACTS: &str = "ToS/source-witnesses/artifacts/";
const COMPOSITES: &str = "ToS/source-witnesses/scholarly-composites/";
const ACCESS_LEDGER: &str = "ToS/source-witnesses/access-requests/public-ledger/";
const SERVER_PLANS: &str = "ToS/source-witnesses/server-import/plans/";
const ITEM_MANIFEST_SUFFIX: &str = "/item.manifest.json";
const V2_ARTIFACT_SCHEMA: &str =
    "https://tree-of-sophia.local/ToS/contracts/artifact-source-witness-v2.schema.json";
const FORBIDDEN_CONTENT_KEYS: &[&str] = &[
    "text",
    "source_text",
    "transliteration",
    "translation",
    "image_data",
    "line_art_data",
    "payload",
];
const PROVENANCE_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const PROVENANCE_V2_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const RIGHTS_SCHEMA: &str = "ToS/contracts/rights-record.schema.json";
const DISCOVERY_SCHEMA: &str = "ToS/contracts/material-discovery-record.schema.json";
const ARTIFACT_SCHEMA: &str = "ToS/contracts/artifact-source-witness.schema.json";
const ARTIFACT_V2_SCHEMA: &str = "ToS/contracts/artifact-source-witness-v2.schema.json";
const HUMAN_FORM_SET_SCHEMA: &str = "ToS/contracts/human-form-set.schema.json";
const ARTIFACT_REPRESENTATION_SCHEMA: &str =
    "ToS/contracts/artifact-visual-representation.schema.json";
const COMPOSITE_SCHEMA: &str = "ToS/contracts/scholarly-composite-witness.schema.json";
const COMPOSITE_REPRESENTATION_SCHEMA: &str =
    "ToS/contracts/scholarly-composite-file-representation.schema.json";
const ACCESS_REQUEST_SCHEMA: &str = "ToS/contracts/access-request.schema.json";
const SERVER_PLAN_SCHEMA: &str = "ToS/contracts/server-import-contract.schema.json";
const PUBLIC_REPRESENTATION_POSTURES: &[&str] = &["authorized", "authorized_with_conditions"];
const LOCAL_REPRESENTATION_POSTURES: &[&str] = &[
    "not_authorized",
    "unknown",
    "authorized_with_conditions",
    "authorized",
];
const PRIVATE_HANDOFF_PATH: &str =
    "ToS/research-packets/foundation-laboratory-2026-07/private-evidence-handoff.v1.json";
const MANUAL_LEDGER_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/manual-error-ledger.jsonl";
const MANUAL_LEDGER_PROVENANCE_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/provenance.manual-error-ledger.ocr-candidate-review-foundation-v1.jsonl";
const PRIOR_EMPTY_MANUAL_LEDGER_SHA256: &str =
    "b33a8b535e65f1e431c6324607c395a69c90e56c22be6b2f402f88bcc54bf761";
const TRANSFER_CROSSWALK_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/transfer-candidate-page-crosswalk.v1.json";
const HIERARCHICAL_TARGET_ROOTS: &[&str] = &[
    "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/structure/mysl-1996-volume-2-operator-pdf",
    "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/expressions/ru-flerova-mysl-1996/structure/mysl-1996-volume-2-operator-pdf",
];
const PRIVATE_HANDOFF_REQUIRED_FORBIDDEN_CLASSES: &[&str] = &[
    "source_page_bytes",
    "source_text_or_transcription",
    "restricted_translation_text",
    "unit_level_judgments",
    "screen_capture",
    "personal_interpretation",
    "operator_identity",
    "reviewer_identity",
    "absolute_or_home_path",
    "hostname_or_network_topology",
    "token_or_credential",
    "browser_url_with_token",
];
const PRIVATE_HANDOFF_SCHEMA: &str =
    "ToS/contracts/private-laboratory-evidence-handoff.schema.json";
const PUBLIC_DERIVATIVE_SCHEMA: &str =
    "ToS/contracts/public-laboratory-evidence-derivative.schema.json";
const MANUAL_LEDGER_SCHEMA: &str = "ToS/contracts/manual-error-ledger-record.schema.json";
const TRANSFER_CROSSWALK_SCHEMA: &str =
    "ToS/contracts/transfer-candidate-structural-crosswalk.schema.json";
const HIERARCHICAL_TARGET_MAP_SCHEMA: &str =
    "ToS/contracts/hierarchical-target-numbered-unit-page-map.schema.json";
const TARGET_STRUCTURAL_CROSSWALK_SCHEMA: &str =
    "ToS/contracts/transfer-candidate-target-structural-crosswalk.schema.json";
const NATIVE_ARTIFACT_MODULE_REF: &str =
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_artifact_commands.py";
const NATIVE_ARTIFACT_COMPANIONS: &[&str] = &[
    "source-create-request.json",
    "source-create-receipt.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeStatus {
    Complete,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub location: String,
    pub code: &'static str,
    pub detail: String,
}

/// Expose the existing record-local discovery predicate to bounded consumers.
/// Schema validation and cross-record provenance binding remain separate.
pub fn material_discovery_semantic_issues(value: &Value) -> Vec<String> {
    discovery_semantic_issues(value)
}

/// Scheduled for the caller's diagnostic-v2 schema worker. `before_issue`
/// points to the insertion position for diagnostics belonging to this document.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaRequest {
    pub before_issue: usize,
    pub location: String,
    pub contract: String,
    pub document: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedScope {
    pub location: String,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPathFacts {
    pub tracked: Option<bool>,
    pub ignored: Option<bool>,
}

/// Facts from the CMD-selected physical payload/Git owner. Payload bytes are
/// streamed once by that owner and are never reopened by this validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalPayloadFacts {
    pub exists: bool,
    pub regular_file: bool,
    pub symlink: bool,
    pub git_tracked: Option<bool>,
    pub git_ignored: Option<bool>,
    pub byte_size: Option<u64>,
    pub sha256: Option<String>,
    pub sha1: Option<String>,
    pub jpeg_dimensions: Option<(u64, u64)>,
}

/// Bounded follow-up facts for a selected pathname's resolved target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalResolvedTargetFacts {
    /// Target resolution was not requested or could not be observed.
    Unknown,
    /// Resolution escaped the exact selected directory/root capability.
    OutsideSelectedRoot,
    /// A target resolved to this path inside the selected held root. The path
    /// is relative to that root and never contains a host-absolute location.
    InsideSelectedRoot {
        relative_target: String,
        topology_stamp: String,
        exists: bool,
        regular_file: bool,
        directory: bool,
        byte_size: Option<u64>,
        sha256: Option<String>,
        git_tracked: Option<bool>,
        git_ignored: Option<bool>,
    },
}

/// Physical snapshot facts for private or local owner files whose bytes must
/// remain outside the authored source cut. The outer fields describe the
/// selected pathname's no-follow entry; a resolved target has separate facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalPathFacts {
    pub exists: bool,
    pub regular_file: bool,
    pub directory: bool,
    pub symlink: bool,
    pub resolved_target: Option<PhysicalResolvedTargetFacts>,
    pub git_tracked: Option<bool>,
    pub git_ignored: Option<bool>,
    pub file_mode: Option<u32>,
    pub byte_size: Option<u64>,
    pub sha256: Option<String>,
}

/// Borrowed transaction binding returned by the CMD correction-replay owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactCorrectionReplayTransactionRef<'a> {
    pub transaction_id: &'a str,
    pub manifest_sha256: &'a str,
    pub receipt_sha256: &'a str,
}

/// Successful-only correction replay evidence from CMD's existing revision
/// reconstruction kernel. Implementations are evidence providers, not an
/// admission decision; Discovery rechecks every binding against its exact cut,
/// native history observation, and current correction receipt.
pub trait ArtifactCorrectionReplayEvidence {
    fn source_revision(&self) -> SourceRevision;
    fn current_membership(&self) -> SourceMembershipV1;
    fn source_path(&self) -> &str;
    fn record_id(&self) -> &str;
    fn origin_record_sha256(&self) -> &str;
    fn origin_record_byte_size(&self) -> usize;
    fn history_sha256(&self) -> Option<&str>;
    fn transaction_count(&self) -> usize;
    fn transaction_at(&self, index: usize) -> Option<ArtifactCorrectionReplayTransactionRef<'_>>;
    fn publication_state_bytes(&self) -> usize;
    fn returned_state_bytes(&self) -> usize;
}

/// Caller-owned map of exact Artifact record paths to borrowed CMD replay
/// observations. The observation constructors remain with their owner.
pub type ArtifactCorrectionReplayMap<'a> =
    BTreeMap<String, &'a dyn ArtifactCorrectionReplayEvidence>;

/// Candidate-fenced correction replay evidence. Its opaque source identity
/// remains the candidate's own type and never borrows `SourceRevision`.
pub trait CandidateArtifactCorrectionReplayEvidence<I: Copy + Eq> {
    fn input_identity(&self) -> &I;
    fn current_membership(&self) -> SourceMembershipV1;
    fn source_path(&self) -> &str;
    fn record_id(&self) -> &str;
    fn origin_record_sha256(&self) -> &str;
    fn origin_record_byte_size(&self) -> usize;
    fn history_sha256(&self) -> Option<&str>;
    fn transaction_count(&self) -> usize;
    fn transaction_at(&self, index: usize) -> Option<ArtifactCorrectionReplayTransactionRef<'_>>;
    fn publication_state_bytes(&self) -> usize;
    fn returned_state_bytes(&self) -> usize;
}

/// Caller-owned map of candidate Artifact paths to their typed replay proof.
pub type CandidateArtifactCorrectionReplayMap<'a, I> =
    BTreeMap<String, &'a dyn CandidateArtifactCorrectionReplayEvidence<I>>;

/// One authentic candidate-fenced Artifact evidence packet held only while
/// Discovery checks its current record. The history comes from the maintained
/// native-history reader and the optional replay from CMD's maintained replay
/// kernel; this boundary carries no serialized proof or source-admission
/// authority.
pub struct CandidateArtifactEvidence<'evidence, I: Copy + Eq> {
    history: crate::native_compound::CandidateNativeRecordHistoryReadObservation<I>,
    replay: Option<Box<dyn CandidateArtifactCorrectionReplayEvidence<I> + 'evidence>>,
    direct_source_read_bytes: u64,
    peak_state_bytes: usize,
}

/// Result of one exact-path provider lookup. The optional schema result avoids
/// repeating the candidate Records point probe inside native Artifact
/// validation; `None` means that the native schema branch did not apply.
pub struct CandidateArtifactEvidenceResponse<'evidence, I: Copy + Eq> {
    evidence: Option<CandidateArtifactEvidence<'evidence, I>>,
    candidate_schema_invalid: Option<bool>,
}

impl<'evidence, I: Copy + Eq> CandidateArtifactEvidenceResponse<'evidence, I> {
    pub fn new(
        evidence: Option<CandidateArtifactEvidence<'evidence, I>>,
        candidate_schema_invalid: Option<bool>,
    ) -> Self {
        Self {
            evidence,
            candidate_schema_invalid,
        }
    }

    pub fn evidence(&self) -> Option<&CandidateArtifactEvidence<'evidence, I>> {
        self.evidence.as_ref()
    }

    pub fn candidate_schema_invalid(&self) -> Option<bool> {
        self.candidate_schema_invalid
    }
}

impl<'evidence, I: Copy + Eq> CandidateArtifactEvidence<'evidence, I> {
    /// Assemble evidence from the maintained private-constructor native
    /// history observation and CMD's correction-replay implementation. The
    /// identity-bearing history type prevents callers from substituting a
    /// serialized or digest-only proof packet.
    pub fn from_observations(
        history: crate::native_compound::CandidateNativeRecordHistoryReadObservation<I>,
        replay: Option<Box<dyn CandidateArtifactCorrectionReplayEvidence<I> + 'evidence>>,
        direct_source_read_bytes: u64,
        peak_state_bytes: usize,
    ) -> Self {
        Self {
            history,
            replay,
            direct_source_read_bytes,
            peak_state_bytes,
        }
    }
}

impl<I: Copy + Eq> CandidateArtifactEvidence<'_, I> {
    pub fn history(
        &self,
    ) -> &crate::native_compound::CandidateNativeRecordHistoryReadObservation<I> {
        &self.history
    }

    pub fn replay(&self) -> Option<&dyn CandidateArtifactCorrectionReplayEvidence<I>> {
        self.replay.as_deref()
    }

    pub fn direct_source_read_bytes(&self) -> u64 {
        self.direct_source_read_bytes
    }

    pub fn peak_state_bytes(&self) -> usize {
        self.peak_state_bytes
    }
}

/// CMD-owned lazy provider for exact candidate Artifact paths. `begin` marks
/// the current path in its held Records index exactly once, `evidence` returns
/// at most one authentic packet, and `finish` proves EOF coverage. Portable VAL
/// depends only on this source-first contract.
pub trait CandidateArtifactEvidenceProvider<I: Copy + Eq> {
    fn validate_binding(
        &mut self,
        input: &dyn SourceCutInputWithIdentity<I>,
        coverage: &SourceCutInputCoverage,
        records: &SourceFoundationRecordsStreamedReport<'_, I>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal>;

    fn begin_artifact_path(
        &mut self,
        path: &str,
        remaining_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationArtifactRecordPathSummary>, ItemRefusal>;

    fn evidence_for_artifact<'evidence>(
        &'evidence mut self,
        path: &str,
        indexed_record: Option<SourceFoundationArtifactRecordPathSummary>,
        current_record: &Value,
        current_member_sha256: Digest256,
        current_member_size_bytes: u64,
        remaining_source_bytes: u64,
        remaining_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CandidateArtifactEvidenceResponse<'evidence, I>, ItemRefusal>;

    fn abandon_artifact_path(
        &mut self,
        path: &str,
        remaining_state_bytes: usize,
    ) -> Result<(), ItemRefusal>;

    fn finish(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal>;
}

/// Bounded observations supplied by the host/CMD owner. Authored metadata
/// posture is keyed by exact selected path. `private_files` is the physical
/// file inventory under the private correspondence root, including ignored
/// files; `None` means that inventory was unavailable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourcePhysicalFacts {
    pub git_available: Option<bool>,
    pub payloads: BTreeMap<String, PhysicalPayloadFacts>,
    pub authored_git: BTreeMap<String, GitPathFacts>,
    /// Exact no-follow observations for authored paths selected by the host.
    /// This is separate from cut membership: captured bytes do not prove Git
    /// tracking, physical file type, or the selected checkout's path shape.
    pub authored_paths: BTreeMap<String, PhysicalPathFacts>,
    /// Exact observations under the separately selected storage-artifact
    /// root, keyed relative to that root rather than the ToS repository.
    pub artifact_paths: BTreeMap<String, PhysicalPathFacts>,
    pub private_files: Option<Vec<String>>,
    /// Exact held private inventories keyed by their selected repository
    /// prefix. An absent key is unknown; an empty vector is a known-empty cut.
    pub private_inventories: BTreeMap<String, Vec<String>>,
    pub private_paths: BTreeMap<String, PhysicalPathFacts>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    pub source_bytes_read: u64,
    pub observed_payload_bytes: u64,
    pub aggregate_document_copies: usize,
    pub state_bytes: usize,
    /// Bytes measured by the shared selected-history kernel and referenced by
    /// this district. The caller owns and charges the kernel execution once.
    pub native_history_referenced_bytes: u64,
    /// Returned state retained by the shared selected-history kernel and
    /// referenced by this district; it is not a second source read or copy.
    pub native_history_referenced_state_bytes: usize,
    /// State already measured by CMD's exact Artifact correction-replay
    /// kernel and referenced here rather than read or charged a second time.
    pub artifact_replay_referenced_publication_state_bytes: usize,
    pub artifact_replay_referenced_state_bytes: usize,
    /// High-water state for the single authentic candidate evidence packet
    /// reconstructed inside Discovery. Retained proof/index state is charged
    /// by the CMD provider's separate cost report.
    pub candidate_artifact_evidence_peak_state_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub status: ScopeStatus,
    pub issues: Vec<Issue>,
    pub schema_requests: Vec<SchemaRequest>,
    /// Parsed owner event records in maintained insertion/overwrite order.
    /// CMD folds these over its Records→Gold event map before Closure.
    pub source_event_insertions: Vec<(String, Value)>,
    pub unsupported: Vec<UnsupportedScope>,
    pub cost: Cost,
}

/// Completed Discovery result bound to one opaque candidate input identity
/// and the membership already authenticated by its Records EOF fence.
/// Candidate identity is never represented as `SourceRevision`.
pub struct SourceFoundationCandidateDiscoveryReport<I> {
    input_identity: I,
    source_membership: SourceMembershipV1,
    candidate_direct_source_bytes: u64,
    report: Report,
}

impl<I> SourceFoundationCandidateDiscoveryReport<I> {
    pub fn input_identity(&self) -> &I {
        &self.input_identity
    }

    pub fn source_membership(&self) -> SourceMembershipV1 {
        self.source_membership
    }

    pub fn candidate_direct_source_bytes(&self) -> u64 {
        self.candidate_direct_source_bytes
    }

    pub fn into_report(self) -> Report {
        self.report
    }
}

struct DiscoveryKernelOutput {
    report: Report,
    candidate_direct_source_bytes: u64,
}

impl Report {
    /// A missing owner observation or a pending whole-source subcheck can
    /// never be represented as a valid empty report.
    pub fn is_valid(&self) -> bool {
        self.status == ScopeStatus::Complete
            && self.issues.is_empty()
            && self.unsupported.is_empty()
            && self.schema_requests.is_empty()
    }
}

#[derive(Debug, Clone)]
struct EventInfo {
    location: String,
    outputs: BTreeMap<String, Option<String>>,
    inputs: BTreeSet<(String, String)>,
}

#[derive(Debug, Clone)]
struct DiscoveryInfo {
    target_kind: String,
    known_refs: BTreeSet<String>,
    /// Exact captured snapshot/acquisition tuples usable by native Artifact
    /// source-binding verification. Only completed downloads with matching
    /// owner fields are retained here.
    captured_acquisitions: BTreeSet<(String, String, String, u64)>,
}

trait DiscoveryCurrentPaths: SourceFoundationDefaultPaths {
    fn for_each_discovery_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;
}

struct ResidentDiscoveryPaths<'a> {
    ordered: Vec<&'a str>,
    members: BTreeSet<&'a str>,
}

impl<'a> ResidentDiscoveryPaths<'a> {
    fn new(paths: &'a [String]) -> Self {
        let mut ordered: Vec<&str> = paths.iter().map(String::as_str).collect();
        ordered.sort_unstable();
        Self {
            ordered,
            members: paths.iter().map(String::as_str).collect(),
        }
    }
}

impl SourceFoundationDefaultPaths for ResidentDiscoveryPaths<'_> {
    fn contains(&self, path: &str) -> Result<bool, ItemRefusal> {
        Ok(self.members.contains(path))
    }

    fn for_each_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        for path in &self.ordered {
            visit(path)?;
        }
        Ok(())
    }
}

impl DiscoveryCurrentPaths for ResidentDiscoveryPaths<'_> {
    fn for_each_discovery_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.for_each_path(visit)
    }
}

struct BorrowedDiscoveryPaths<'a>(&'a dyn SourceFoundationDefaultPaths);

impl SourceFoundationDefaultPaths for BorrowedDiscoveryPaths<'_> {
    fn contains(&self, path: &str) -> Result<bool, ItemRefusal> {
        self.0.contains(path)
    }

    fn for_each_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.0.for_each_path(visit)
    }
}

impl DiscoveryCurrentPaths for BorrowedDiscoveryPaths<'_> {
    fn for_each_discovery_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.0.for_each_path(visit)
    }
}

struct Inspector<'s, 'p, S: LayerFamilySource + ?Sized, I: Copy + Eq = ()> {
    source: &'s mut S,
    paths: &'p dyn DiscoveryCurrentPaths,
    candidate_input: Option<&'p dyn SourceCutInput>,
    records_lookup: Option<&'p dyn SourceFoundationDefaultRecordsLookup>,
    candidate_identity: Option<&'p I>,
    candidate_membership: Option<SourceMembershipV1>,
    native_histories: NativeHistorySet<'p, I>,
    artifact_replays: ArtifactReplaySet<'p, I>,
    candidate_invalid_schema_proofs: Option<&'p dyn CandidateInvalidArtifactSchemaProof<I>>,
    limits: ItemLimits,
    physical: Option<&'s SourcePhysicalFacts>,
    issues: Vec<Issue>,
    schema_requests: Vec<SchemaRequest>,
    schema_locations: BTreeSet<String>,
    unsupported: Vec<UnsupportedScope>,
    digests: BTreeMap<String, String>,
    payload_observation_paths: BTreeSet<String>,
    read_bytes: u64,
    candidate_direct_source_bytes: u64,
    candidate_provider_source_bytes: u64,
    payload_bytes: u64,
    state_bytes: usize,
    document_copies: usize,
    native_history_referenced_bytes: u64,
    native_history_referenced_state_bytes: usize,
    artifact_replay_referenced_paths: Option<BTreeSet<String>>,
    artifact_replay_referenced_publication_state_bytes: usize,
    artifact_replay_referenced_state_bytes: usize,
    candidate_artifact_evidence_peak_state_bytes: usize,
}

impl<S: LayerFamilySource + ?Sized, I: Copy + Eq> Inspector<'_, '_, S, I> {
    fn checkpoint(&self) -> Result<(), ItemRefusal> {
        if self.source.cancellation().load(Ordering::Relaxed) {
            return Err(ItemRefusal::Source(
                "source-foundation discovery cancelled".into(),
            ));
        }
        self.source.checkpoint(self.limits.deadline)
    }

    fn has_current_member(&self, path: &str) -> Result<bool, ItemRefusal> {
        self.paths.contains(path)
    }

    fn for_each_current_path(
        &mut self,
        visit: &mut dyn FnMut(&mut Self, &str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let paths = self.paths;
        paths.for_each_discovery_path(&mut |path| {
            self.checkpoint()?;
            visit(self, path)
        })
    }

    fn collect_current_paths_matching(
        &mut self,
        mut matches: impl FnMut(&str) -> bool,
    ) -> Result<Vec<String>, ItemRefusal> {
        let mut paths = Vec::new();
        self.for_each_current_path(&mut |inspector, path| {
            if matches(path) {
                inspector.reserve_state(path.len().checked_add(24).ok_or(ItemRefusal::Budget)?)?;
                paths.push(path.to_owned());
            }
            Ok(())
        })?;
        Ok(paths)
    }

    fn reserve_state(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        self.state_bytes = self
            .state_bytes
            .checked_add(bytes)
            .filter(|used| *used <= self.limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn issue(
        &mut self,
        location: impl Into<String>,
        code: &'static str,
        detail: impl Into<String>,
    ) -> Result<(), ItemRefusal> {
        if self.issues.len() + self.unsupported.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        let location = location.into();
        let detail = detail.into();
        self.reserve_state(location.len() + detail.len() + code.len() + 96)?;
        self.issues.push(Issue {
            location,
            code,
            detail,
        });
        Ok(())
    }

    fn unsupported(
        &mut self,
        location: impl Into<String>,
        reason: &'static str,
    ) -> Result<(), ItemRefusal> {
        if self.issues.len() + self.unsupported.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        let location = location.into();
        self.reserve_state(location.len() + reason.len() + 64)?;
        if !self
            .unsupported
            .iter()
            .any(|row| row.location == location && row.reason == reason)
        {
            self.unsupported.push(UnsupportedScope { location, reason });
        }
        Ok(())
    }

    fn reference_native_history(
        &mut self,
        observation: &NativeHistoryRef<'_, I>,
    ) -> Result<(), ItemRefusal> {
        self.native_history_referenced_bytes = self
            .native_history_referenced_bytes
            .checked_add(observation.bytes_read())
            .ok_or(ItemRefusal::Budget)?;
        self.native_history_referenced_state_bytes = self
            .native_history_referenced_state_bytes
            .checked_add(observation.returned_state_bytes())
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn reference_artifact_replay(
        &mut self,
        path: &str,
        evidence: &ArtifactReplayRef<'_, I>,
    ) -> Result<(), ItemRefusal> {
        if let Some(paths) = &mut self.artifact_replay_referenced_paths {
            if !paths.insert(path.to_owned()) {
                return Ok(());
            }
            self.reserve_state(path.len().checked_add(64).ok_or(ItemRefusal::Budget)?)?;
        } else {
            self.check_temporary_state(path.len().checked_add(64).ok_or(ItemRefusal::Budget)?)?;
        }
        self.artifact_replay_referenced_publication_state_bytes = self
            .artifact_replay_referenced_publication_state_bytes
            .checked_add(evidence.publication_state_bytes())
            .ok_or(ItemRefusal::Budget)?;
        self.artifact_replay_referenced_state_bytes = self
            .artifact_replay_referenced_state_bytes
            .checked_add(evidence.returned_state_bytes())
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn check_temporary_state(&self, bytes: usize) -> Result<(), ItemRefusal> {
        self.state_bytes
            .checked_add(bytes)
            .filter(|used| *used <= self.limits.max_state_bytes)
            .map(|_| ())
            .ok_or(ItemRefusal::Budget)
    }

    fn canonical_record_sha256(&mut self, value: &Value) -> Result<String, ItemRefusal> {
        let raw = serde_json::to_vec(value)
            .map_err(|_| ItemRefusal::Unsupported("Artifact correction receipt encoding".into()))?;
        if raw.len() > self.limits.max_member_bytes {
            return Err(ItemRefusal::Budget);
        }
        let limits = JsonLimits::new(
            self.limits.max_member_bytes.min(8_388_608),
            64,
            300_000,
            4_300,
        )
        .map_err(|_| ItemRefusal::Budget)?;
        self.reserve_state(raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
        let ordered = parse_json(&raw, JsonMode::PublishedStrict, limits)
            .map_err(|error| {
                if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                    ItemRefusal::Budget
                } else {
                    ItemRefusal::Unsupported(
                        "Artifact correction receipt is not strict published JSON".into(),
                    )
                }
            })?
            .into_root();
        let canonical =
            canonical_bytes_v1(&ordered, CanonicalProfile::SourceCommandInputV1, limits).map_err(
                |error| {
                    if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                        ItemRefusal::Budget
                    } else {
                        ItemRefusal::Unsupported("Artifact correction receipt digest".into())
                    }
                },
            )?;
        self.reserve_state(canonical.len().checked_add(96).ok_or(ItemRefusal::Budget)?)?;
        Ok(Digest256::of_bytes(&canonical).to_prefixed())
    }

    fn current_bytes(&mut self, path: &str) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint()?;
        if !self.has_current_member(path)? {
            return Ok(None);
        }
        let mut candidate_cost_precharged = false;
        let raw = if let Some(input) = self.candidate_input {
            let mut current = None;
            let max_member_bytes = self.limits.max_member_bytes;
            let max_total_bytes = self.limits.max_total_bytes;
            let max_state_bytes = self.limits.max_state_bytes;
            // The input must receive the remaining allowance before its raw
            // or SQL read. Preserve the same eight-times-copy charge used by
            // this kernel below; a post-read check cannot bound that read.
            let remaining_read_bytes = max_total_bytes
                .checked_sub(self.read_bytes)
                .ok_or(ItemRefusal::Budget)?;
            let remaining_copy_bytes = max_state_bytes
                .checked_sub(self.state_bytes)
                .ok_or(ItemRefusal::Budget)?
                / 8;
            let max_request_bytes = max_member_bytes
                .min(usize::try_from(remaining_read_bytes).unwrap_or(usize::MAX))
                .min(remaining_copy_bytes);
            let deadline = self.limits.deadline;
            let cancellation = self.source.cancellation();
            let read_bytes = &mut self.read_bytes;
            let candidate_direct_source_bytes = &mut self.candidate_direct_source_bytes;
            let state_bytes = &mut self.state_bytes;
            input.with_current_member(
                path,
                max_request_bytes,
                deadline,
                cancellation,
                &mut |meta, bytes| {
                    let byte_count = u64::try_from(bytes.len()).map_err(|_| ItemRefusal::Budget)?;
                    if meta.path != path || current.is_some() || meta.size_bytes != byte_count {
                        return Err(ItemRefusal::Source(
                            "candidate Discovery current member metadata differs from its bytes"
                                .into(),
                        ));
                    }
                    if bytes.len() > max_request_bytes {
                        return Err(ItemRefusal::Budget);
                    }
                    let next_read_bytes = read_bytes
                        .checked_add(byte_count)
                        .filter(|used| *used <= max_total_bytes)
                        .ok_or(ItemRefusal::Budget)?;
                    let next_candidate_source_bytes = candidate_direct_source_bytes
                        .checked_add(byte_count)
                        .ok_or(ItemRefusal::Budget)?;
                    let copy_state_bytes = bytes.len().checked_mul(8).ok_or(ItemRefusal::Budget)?;
                    let next_state_bytes = state_bytes
                        .checked_add(copy_state_bytes)
                        .filter(|used| *used <= max_state_bytes)
                        .ok_or(ItemRefusal::Budget)?;
                    *read_bytes = next_read_bytes;
                    *candidate_direct_source_bytes = next_candidate_source_bytes;
                    *state_bytes = next_state_bytes;
                    candidate_cost_precharged = true;
                    current = Some(bytes.to_vec());
                    Ok(())
                },
            )?;
            current
        } else {
            self.source
                .current(path, self.limits.max_member_bytes, self.limits.deadline)?
        }
        .ok_or_else(|| {
            ItemRefusal::Source(format!(
                "captured source member disappeared through exact current reader: {path}"
            ))
        })?;
        self.checkpoint()?;
        if raw.len() > self.limits.max_member_bytes {
            return Err(ItemRefusal::Budget);
        }
        if !candidate_cost_precharged {
            let raw_bytes = u64::try_from(raw.len()).map_err(|_| ItemRefusal::Budget)?;
            self.read_bytes = self
                .read_bytes
                .checked_add(raw_bytes)
                .filter(|used| *used <= self.limits.max_total_bytes)
                .ok_or(ItemRefusal::Budget)?;
            self.reserve_state(raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
        }
        self.digests
            .entry(path.to_owned())
            .or_insert_with(|| Digest256::of_bytes(&raw).to_hex());
        Ok(Some(raw))
    }

    fn request_schema(
        &mut self,
        location: &str,
        contract: &str,
        document: &Value,
        raw_size: usize,
    ) -> Result<(), ItemRefusal> {
        if !self.schema_locations.insert(location.to_owned()) {
            return Ok(());
        }
        if !self.has_current_member(contract)? {
            self.issue(location, "missing-current-schema", contract)?;
            return Ok(());
        }
        if self.schema_requests.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        // The scheduled decoded document is the one bounded aggregate copy
        // retained for schema diagnostics; no second boolean execution runs.
        self.reserve_state(raw_size.checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
        self.document_copies = self
            .document_copies
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.schema_requests.push(SchemaRequest {
            before_issue: self.issues.len(),
            location: location.to_owned(),
            contract: contract.to_owned(),
            document: document.clone(),
        });
        Ok(())
    }

    fn json(
        &mut self,
        path: &str,
        location: &str,
        contract: &str,
    ) -> Result<Option<(Value, String, usize)>, ItemRefusal> {
        let Some((value, digest, size)) = self.json_unchecked(path, location)? else {
            return Ok(None);
        };
        self.request_schema(location, contract, &value, size)?;
        Ok(Some((value, digest, size)))
    }

    fn json_unchecked(
        &mut self,
        path: &str,
        location: &str,
    ) -> Result<Option<(Value, String, usize)>, ItemRefusal> {
        let Some(raw) = self.current_bytes(path)? else {
            return Ok(None);
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        let value = match serde_json::from_slice::<Value>(&raw) {
            Ok(value) => value,
            Err(_) => {
                self.issue(
                    location,
                    "invalid-json",
                    "current source document is invalid JSON",
                )?;
                return Ok(None);
            }
        };
        let size = raw.len();
        Ok(Some((value, digest, size)))
    }

    fn digest(&mut self, path: &str) -> Result<Option<String>, ItemRefusal> {
        if let Some(value) = self.digests.get(path) {
            return Ok(Some(value.clone()));
        }
        let Some(raw) = self.current_bytes(path)? else {
            return Ok(None);
        };
        let value = Digest256::of_bytes(&raw).to_hex();
        self.digests.insert(path.to_owned(), value.clone());
        Ok(Some(value))
    }

    fn cached_digest(&self, path: &str) -> Option<Digest256> {
        self.digests
            .get(path)
            .and_then(|value| Digest256::from_hex(value).ok())
    }

    fn exists_source_ref(&mut self, path: &str) -> Result<bool, ItemRefusal> {
        if !path.starts_with("ToS/") {
            return Ok(true);
        }
        self.checkpoint()?;
        if !self.has_current_member(path)? {
            return Ok(false);
        }
        self.source
            .exists(path, self.limits.max_member_bytes, self.limits.deadline)
    }

    fn source_refs(&mut self, value: &Value, location: &str) -> Result<(), ItemRefusal> {
        let mut refs: Vec<&Value> = Vec::new();
        for field in ["source_refs", "source_record_refs", "receipt_refs"] {
            if let Some(items) = value.get(field).and_then(Value::as_array) {
                refs.extend(items);
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
            if let Some(item) = value.get(field) {
                refs.push(item);
            }
        }
        for reference in refs {
            self.checkpoint()?;
            let Some(path) = reference.as_str() else {
                self.issue(
                    location,
                    "invalid-source-reference",
                    python_string_value(Some(reference)),
                )?;
                continue;
            };
            if !self.exists_source_ref(path)? {
                self.issue(location, "unresolved-source-reference", path)?;
            }
        }
        Ok(())
    }

    fn jsonl(&mut self, path: &str, contract: &str) -> Result<Vec<(String, Value)>, ItemRefusal> {
        let Some(raw) = self.current_bytes(path)? else {
            return Ok(Vec::new());
        };
        let text = match std::str::from_utf8(&raw) {
            Ok(text) => text,
            Err(_) => {
                self.issue(
                    path,
                    "invalid-jsonl-utf8",
                    "current provenance stream is not UTF-8",
                )?;
                return Ok(Vec::new());
            }
        };
        let mut parsed = Vec::new();
        for (index, line) in text.lines().enumerate() {
            self.checkpoint()?;
            if line.trim().is_empty() {
                self.issue(
                    format!("{path}:{}", index + 1),
                    "blank-jsonl-record",
                    "blank JSONL line is not allowed",
                )?;
                continue;
            }
            let location = format!("{path}:{}", index + 1);
            let value = match serde_json::from_str::<Value>(line) {
                Ok(value) => value,
                Err(_) => {
                    self.issue(
                        &location,
                        "invalid-jsonl-record",
                        "provenance line is invalid JSON",
                    )?;
                    continue;
                }
            };
            if !value.is_object() {
                self.issue(
                    &location,
                    "jsonl-record-not-object",
                    "JSONL record must be an object",
                )?;
                continue;
            }
            let raw_size = line.len();
            self.reserve_state(raw_size.checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
            self.request_schema(&location, contract, &value, raw_size)?;
            parsed.push((location, value));
        }
        Ok(parsed)
    }

    fn event_info(&mut self, value: &Value, location: &str) -> Result<EventInfo, ItemRefusal> {
        self.source_refs(value, location)?;
        let mut outputs = BTreeMap::new();
        for row in array(value, "outputs") {
            if let Some(reference) = string(row, "ref") {
                outputs.insert(
                    reference.to_owned(),
                    string(row, "sha256").map(str::to_owned),
                );
            }
        }
        let mut inputs = BTreeSet::new();
        for row in array(value, "inputs") {
            if let (Some(reference), Some(sha256)) = (string(row, "ref"), string(row, "sha256")) {
                inputs.insert((reference.to_owned(), sha256.to_owned()));
            }
        }
        Ok(EventInfo {
            location: location.to_owned(),
            outputs,
            inputs,
        })
    }

    fn metadata_git(&mut self, path: &str, location: &str) -> Result<(), ItemRefusal> {
        let Some(physical) = self.physical else {
            self.unsupported(
                location,
                "Git tracking facts for selected authored metadata are unavailable",
            )?;
            return Ok(());
        };
        match physical.git_available {
            Some(false) => Ok(()),
            Some(true) => match self.git_path_facts(path) {
                Some(facts) if facts.tracked == Some(true) => Ok(()),
                Some(_) => self.issue(location, "metadata-not-git-tracked", path),
                None => self.unsupported(
                    path,
                    "selected authored path has no exact Git tracking observation",
                ),
            },
            None => self.unsupported(path, "repository Git availability is unobserved"),
        }
    }

    fn git_path_facts(&self, path: &str) -> Option<GitPathFacts> {
        let physical = self.physical?;
        physical
            .authored_git
            .get(path)
            .cloned()
            .or_else(|| {
                physical.private_paths.get(path).map(|facts| GitPathFacts {
                    tracked: facts.git_tracked,
                    ignored: facts.git_ignored,
                })
            })
            .or_else(|| {
                physical.payloads.get(path).map(|facts| GitPathFacts {
                    tracked: facts.git_tracked,
                    ignored: facts.git_ignored,
                })
            })
    }

    fn physical_path_facts(&self, path: &str) -> Option<PhysicalPathFacts> {
        let physical = self.physical?;
        physical
            .authored_paths
            .get(path)
            .cloned()
            .or_else(|| physical.private_paths.get(path).cloned())
    }

    fn exact_file_posture(
        &mut self,
        path: &str,
        owner: &str,
    ) -> Result<Option<PhysicalPathFacts>, ItemRefusal> {
        let Some(facts) = self.physical_path_facts(path) else {
            self.unsupported(
                owner,
                "exact authored-path physical type and existence are unobserved",
            )?;
            return Ok(None);
        };
        if !facts.exists {
            self.issue(owner, "physical-source-file-missing", path)?;
        } else if facts.symlink || !facts.regular_file {
            self.issue(owner, "physical-source-file-not-regular", path)?;
        }
        Ok(Some(facts))
    }

    fn observe_current_path(
        &mut self,
        path: &str,
    ) -> Result<Option<PhysicalPathFacts>, ItemRefusal> {
        let Some(facts) = self.physical_path_facts(path) else {
            self.unsupported(path, "exact authored-path physical existence is unobserved")?;
            return Ok(None);
        };
        if !facts.exists {
            self.issue(path, "physical-source-file-missing", "file is missing")?;
            return Ok(Some(facts));
        }
        if facts.symlink || !facts.regular_file {
            self.issue(
                path,
                "physical-source-file-not-regular",
                "selected path is not a regular file",
            )?;
        }
        Ok(Some(facts))
    }

    fn physical_exists(&mut self, path: &str, owner: &str) -> Result<Option<bool>, ItemRefusal> {
        if let Some(facts) = self.physical_path_facts(path) {
            return Ok(Some(facts.exists));
        }
        if self.has_current_member(path)? {
            self.unsupported(
                owner,
                "physical existence for a selected authored path is unobserved",
            )?;
            return Ok(Some(true));
        }
        self.unsupported(owner, "physical existence for the exact path is unobserved")?;
        Ok(None)
    }

    fn physical_sha256(&mut self, path: &str, owner: &str) -> Result<Option<String>, ItemRefusal> {
        let Some(facts) = self.exact_file_posture(path, owner)? else {
            return Ok(None);
        };
        if let Some(digest) = facts.sha256 {
            return Ok(Some(digest));
        }
        self.unsupported(owner, "exact authored-path SHA-256 is unobserved")?;
        Ok(None)
    }

    fn required_object(
        &mut self,
        path: &str,
        contract: &str,
    ) -> Result<Option<(Value, String)>, ItemRefusal> {
        let Some((value, digest, raw_size)) = self.current_object_unvalidated(path)? else {
            return Ok(None);
        };
        self.request_schema(path, contract, &value, raw_size)?;
        Ok(Some((value, digest)))
    }

    fn current_object_unvalidated(
        &mut self,
        path: &str,
    ) -> Result<Option<(Value, String, usize)>, ItemRefusal> {
        if !self.has_current_member(path)? {
            self.issue(path, "missing-current-document", "file is missing")?;
            return Ok(None);
        }
        if self
            .observe_current_path(path)?
            .is_some_and(|facts| !facts.exists)
        {
            return Ok(None);
        }
        let Some((value, digest, raw_size)) = self.json_unchecked(path, path)? else {
            return Ok(None);
        };
        if !value.is_object() {
            self.issue(path, "json-root-not-object", "JSON root must be an object")?;
            return Ok(None);
        }
        Ok(Some((value, digest, raw_size)))
    }

    fn current_object_with_ordered_value(
        &mut self,
        path: &str,
    ) -> Result<Option<(Value, JsonValue, String, usize)>, ItemRefusal> {
        if !self.has_current_member(path)? {
            self.issue(path, "missing-current-document", "file is missing")?;
            return Ok(None);
        }
        if self
            .observe_current_path(path)?
            .is_some_and(|facts| !facts.exists)
        {
            return Ok(None);
        }
        let Some(raw) = self.current_bytes(path)? else {
            return Ok(None);
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        let value = match serde_json::from_slice::<Value>(&raw) {
            Ok(value) => value,
            Err(_) => {
                self.issue(
                    path,
                    "invalid-json",
                    "current source document is invalid JSON",
                )?;
                return Ok(None);
            }
        };
        if !value.is_object() {
            self.issue(path, "json-root-not-object", "JSON root must be an object")?;
            return Ok(None);
        }
        let limits = JsonLimits::new(
            self.limits.max_member_bytes.min(8_388_608),
            64,
            300_000,
            4_300,
        )
        .map_err(|_| ItemRefusal::Budget)?;
        self.reserve_state(raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
        let ordered = match parse_json(&raw, JsonMode::PublishedStrict, limits) {
            Ok(document) => document.into_root(),
            Err(_) => {
                self.issue(
                    path,
                    "invalid-published-json",
                    "document is not accepted by the strict published JSON profile",
                )?;
                return Ok(None);
            }
        };
        if ordered.as_object().is_none() {
            self.issue(path, "json-root-not-object", "JSON root must be an object")?;
            return Ok(None);
        }
        Ok(Some((value, ordered, digest, raw.len())))
    }

    fn digest_bound_object(
        &mut self,
        owner: &str,
        field: &str,
        binding: &Value,
        load_document: bool,
    ) -> Result<Option<Value>, ItemRefusal> {
        let Some(object) = binding.as_object() else {
            self.issue(
                owner,
                "digest-bound-reference-invalid",
                format!("{field} is not a digest-bound reference"),
            )?;
            return Ok(None);
        };
        let Some(reference) = object.get("ref").and_then(Value::as_str) else {
            self.issue(
                owner,
                "digest-bound-reference-invalid",
                format!("{field} has no string ref"),
            )?;
            return Ok(None);
        };
        if !safe_relative_path(reference) || !reference.starts_with("ToS/") {
            self.issue(
                owner,
                "digest-bound-reference-invalid",
                format!("{field} does not cite a safe current ToS path: {reference}"),
            )?;
            return Ok(None);
        }
        let physical = self.exact_file_posture(reference, owner)?;
        if !self.has_current_member(reference)? {
            self.issue(
                owner,
                "digest-bound-reference-not-current",
                format!("{field} referenced owner artifact is not in exact current membership: {reference}"),
            )?;
            return Ok(None);
        }
        if physical.as_ref().is_some_and(|facts| !facts.exists) {
            return Ok(None);
        }
        let (current_digest, value) = if load_document {
            let Some((value, digest, _)) = self.json_unchecked(reference, reference)? else {
                return Ok(None);
            };
            (digest, Some(value))
        } else {
            let Some(digest) = self.digest(reference)? else {
                return Ok(None);
            };
            (digest, None)
        };
        let declared_digest = object.get("sha256").and_then(Value::as_str);
        let actual_digest = match physical.and_then(|facts| facts.sha256) {
            Some(digest) => {
                if digest != current_digest {
                    self.issue(
                        owner,
                        "physical-source-digest-drift",
                        format!("{field} physical bytes differ from the exact current source: {reference}"),
                    )?;
                }
                digest
            }
            None => {
                self.unsupported(owner, "digest-bound physical SHA-256 is unobserved")?;
                current_digest
            }
        };
        if declared_digest != Some(actual_digest.as_str()) {
            self.issue(
                owner,
                "digest-bound-reference-digest-drift",
                format!("{field} digest drifted: {reference}"),
            )?;
        }
        if !load_document {
            return Ok(None);
        }
        let Some(value) = value else { return Ok(None) };
        if !value.is_object() {
            self.issue(
                owner,
                "digest-bound-document-not-object",
                format!("{field} referenced owner document is not an object: {reference}"),
            )?;
            return Ok(None);
        }
        Ok(Some(value))
    }

    fn required_jsonl_objects(
        &mut self,
        path: &str,
        contract: &str,
    ) -> Result<Vec<Value>, ItemRefusal> {
        if !self.has_current_member(path)? {
            self.issue(path, "missing-current-jsonl", "file is missing")?;
            return Ok(Vec::new());
        }
        if self
            .observe_current_path(path)?
            .is_some_and(|facts| !facts.exists)
        {
            return Ok(Vec::new());
        }
        let Some(raw) = self.current_bytes(path)? else {
            self.issue(path, "missing-current-jsonl", "file is missing")?;
            return Ok(Vec::new());
        };
        let text = match std::str::from_utf8(&raw) {
            Ok(text) => text,
            Err(_) => {
                self.issue(
                    path,
                    "invalid-jsonl-utf8",
                    "current provenance stream is not UTF-8",
                )?;
                return Ok(Vec::new());
            }
        };
        let mut records = Vec::new();
        for (index, line) in text.lines().enumerate() {
            self.checkpoint()?;
            let location = format!("{path}:{}", index + 1);
            if line.trim().is_empty() {
                self.issue(
                    &location,
                    "blank-jsonl-record",
                    "blank JSONL line is not allowed",
                )?;
                continue;
            }
            let value = match serde_json::from_str::<Value>(line) {
                Ok(value) => value,
                Err(_) => {
                    self.issue(
                        &location,
                        "invalid-jsonl-record",
                        "provenance line is invalid JSON",
                    )?;
                    continue;
                }
            };
            if !value.is_object() {
                self.issue(
                    &location,
                    "jsonl-record-not-object",
                    "JSONL record must be an object",
                )?;
                continue;
            }
            self.reserve_state(line.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
            self.request_schema(&location, contract, &value, line.len())?;
            records.push(value);
        }
        Ok(records)
    }

    fn payload_git_values(&self, path: &str) -> (Option<bool>, Option<bool>) {
        if let Some(facts) = self
            .physical
            .and_then(|physical| physical.payloads.get(path))
        {
            if facts.git_tracked.is_some() || facts.git_ignored.is_some() {
                return (facts.git_tracked, facts.git_ignored);
            }
        }
        self.git_path_facts(path)
            .map(|facts| (facts.tracked, facts.ignored))
            .unwrap_or((None, None))
    }

    fn public_payload_git(&mut self, path: &str, location: &str) -> Result<(), ItemRefusal> {
        let available = self.physical.and_then(|facts| facts.git_available);
        match available {
            Some(true) => {
                let (tracked, ignored) = self.payload_git_values(path);
                match tracked {
                    Some(true) => {}
                    Some(false) => self.issue(location, "public-payload-not-git-tracked", path)?,
                    None => self.unsupported(path, "public payload Git tracking is unobserved")?,
                }
                match ignored {
                    Some(false) => {}
                    Some(true) => self.issue(location, "public-payload-git-ignored", path)?,
                    None => {
                        self.unsupported(path, "public payload Git ignore posture is unobserved")?
                    }
                }
            }
            Some(false) => self.issue(location, "public-payload-not-git-tracked", path)?,
            None => self.unsupported(path, "repository Git availability is unobserved")?,
        }
        Ok(())
    }

    fn tracked_composite_payload_git(
        &mut self,
        path: &str,
        location: &str,
    ) -> Result<(), ItemRefusal> {
        match self.physical.and_then(|facts| facts.git_available) {
            Some(false) => {}
            Some(true) => {
                let (tracked, ignored) = self.payload_git_values(path);
                match tracked {
                    Some(true) => {}
                    Some(false) => self.issue(
                        location,
                        "composite-payload-not-git-tracked",
                        "local scholarly-composite payload must be tracked",
                    )?,
                    None => self.issue(
                        location,
                        "composite-payload-git-tracking-unobserved",
                        "could not determine scholarly-composite payload tracking posture",
                    )?,
                }
                match ignored {
                    Some(true) => self.issue(
                        location,
                        "composite-payload-git-ignored",
                        "tracked scholarly-composite payload must not be gitignored",
                    )?,
                    Some(false) => {}
                    None => self.unsupported(
                        path,
                        "scholarly-composite payload Git ignore posture is unobserved",
                    )?,
                }
            }
            None => self.unsupported(path, "repository Git availability is unobserved")?,
        }
        Ok(())
    }

    fn checked_payload_facts(
        &mut self,
        path: &str,
        location: &str,
    ) -> Result<Option<PhysicalPayloadFacts>, ItemRefusal> {
        let Some(physical) = self.physical else {
            self.unsupported(location, "selected physical payload facts are unavailable")?;
            return Ok(None);
        };
        let Some(facts) = physical.payloads.get(path).cloned() else {
            self.unsupported(
                path,
                "CMD payload stream did not provide this exact path observation",
            )?;
            return Ok(None);
        };
        self.reserve_state(
            path.len()
                .checked_add(facts.sha256.as_ref().map_or(0, String::len))
                .and_then(|used| used.checked_add(facts.sha1.as_ref().map_or(0, String::len)))
                .and_then(|used| used.checked_add(128))
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let first_observation = self.payload_observation_paths.insert(path.to_owned());
        if first_observation {
            self.reserve_state(path.len().checked_add(32).ok_or(ItemRefusal::Budget)?)?;
            if let Some(size) = facts.byte_size {
                self.payload_bytes = self
                    .payload_bytes
                    .checked_add(size)
                    .filter(|used| used <= &self.limits.max_total_bytes)
                    .ok_or(ItemRefusal::Budget)?;
            }
        }
        Ok(Some(facts))
    }

    fn check_required_output_digests(
        &mut self,
        event: &EventInfo,
        required: &BTreeSet<String>,
        location: &str,
    ) -> Result<(), ItemRefusal> {
        for reference in required {
            self.checkpoint()?;
            if !reference.starts_with("ToS/") {
                continue;
            }
            if self
                .physical
                .and_then(|facts| facts.payloads.get(reference))
                .is_some_and(|facts| !facts.exists)
            {
                continue;
            }
            let Some(expected) = event.outputs.get(reference) else {
                continue;
            };
            let actual = if let Some(facts) = self.physical.and_then(|f| f.payloads.get(reference))
            {
                facts.sha256.clone()
            } else if self.has_current_member(reference.as_str())? {
                self.digest(reference)?
            } else {
                None
            };
            match actual {
                Some(actual) if expected.as_deref() == Some(actual.as_str()) => {}
                Some(_) => self.issue(location, "provenance-output-digest-drift", reference)?,
                None => self.unsupported(
                    reference,
                    "required provenance output digest cannot be observed",
                )?,
            }
        }
        Ok(())
    }
}

impl<S: LayerFamilySource + ?Sized, I: Copy + Eq> Inspector<'_, '_, S, I> {
    fn referenced_json(
        &mut self,
        reference: &str,
        owner: &str,
        contract: &str,
    ) -> Result<Option<Value>, ItemRefusal> {
        if !safe_relative_path(reference) {
            self.issue(owner, "unsafe-source-reference", reference)?;
            return Ok(None);
        }
        if !self.exists_source_ref(reference)? {
            self.issue(owner, "unresolved-source-reference", reference)?;
            return Ok(None);
        }
        let Some((value, _, _)) = self.json(reference, reference, contract)? else {
            self.issue(owner, "referenced-current-document-unavailable", reference)?;
            return Ok(None);
        };
        Ok(Some(value))
    }

    fn check_rights(
        &mut self,
        owner: &str,
        rights_ref: &str,
        required_scope: &[&str],
        visibility: &str,
        postures: &[&str],
    ) -> Result<Option<Value>, ItemRefusal> {
        let Some(rights) = self.referenced_json(rights_ref, owner, RIGHTS_SCHEMA)? else {
            return Ok(None);
        };
        if rights.get("scope_refs").is_some_and(Value::is_array)
            && !rights_scope_contains(&rights, required_scope)
        {
            self.issue(rights_ref, "rights-scope-does-not-cover-owner", owner)?;
        }
        if string(&rights, "visibility") != Some(visibility)
            || !postures.contains(&string(&rights, "redistribution_posture").unwrap_or(""))
        {
            self.issue(rights_ref, "rights-posture-drift", owner)?;
        }
        Ok(Some(rights))
    }

    fn private_route(&mut self) -> Result<(), ItemRefusal> {
        let root = "ToS/source-witnesses/access-requests/private";
        let route = format!("{root}/README.md");
        if !self.has_current_member(route.as_str())? {
            self.issue(
                &route,
                "private-route-card-missing",
                "private correspondence route card is missing",
            )?;
        } else {
            self.metadata_git(&route, &route)?;
            if let Some(git) = self.git_path_facts(&route) {
                if git.ignored == Some(true) {
                    self.issue(
                        &route,
                        "private-route-card-ignored",
                        "private correspondence route card must remain tracked",
                    )?;
                }
            }
        }
        let inventory_cost = match self
            .physical
            .and_then(|facts| facts.private_inventories.get(root))
            .or_else(|| self.physical.and_then(|facts| facts.private_files.as_ref()))
        {
            Some(files) => files
                .iter()
                .try_fold(0usize, |used, path| used.checked_add(path.len() + 32))
                .ok_or(ItemRefusal::Budget)?,
            None => {
                self.unsupported(
                    root,
                    "physical private-correspondence file inventory is unavailable",
                )?;
                return Ok(());
            }
        };
        self.reserve_state(inventory_cost)?;
        let files = self
            .physical
            .and_then(|facts| facts.private_inventories.get(root).cloned())
            .or_else(|| self.physical.and_then(|facts| facts.private_files.clone()))
            .ok_or(ItemRefusal::Budget)?;
        for file in files {
            self.checkpoint()?;
            let path = if file.starts_with("ToS/") {
                file
            } else {
                format!("{root}/{file}")
            };
            if path == route {
                continue;
            }
            match self.git_path_facts(&path) {
                Some(facts) if facts.ignored == Some(true) => {}
                Some(facts) if facts.ignored == Some(false) => {
                    self.issue(
                        &path,
                        "private-file-not-git-ignored",
                        "private correspondence file is not ignored",
                    )?;
                }
                Some(_) => self.unsupported(
                    &path,
                    "private correspondence Git ignore posture is unobserved",
                )?,
                None => self.unsupported(
                    &path,
                    "private correspondence file has no exact Git ignore observation",
                )?,
            }
        }
        Ok(())
    }
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Python's maintained owner messages interpolate lists of strings with
/// `repr`, which uses quoted strings rather than Rust's `Debug` spelling.
/// These values are paths and IDs from bounded source documents.
fn python_repr_string(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut rendered = String::with_capacity(value.len() + 2);
    rendered.push(quote);
    for ch in value.chars() {
        match ch {
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            ch if ch == quote => {
                rendered.push('\\');
                rendered.push(ch);
            }
            ch if ch.is_control() => {
                let code = ch as u32;
                if code <= 0xff {
                    rendered.push_str(&format!("\\x{code:02x}"));
                } else if code <= 0xffff {
                    rendered.push_str(&format!("\\u{code:04x}"));
                } else {
                    rendered.push_str(&format!("\\U{code:08x}"));
                }
            }
            ch => rendered.push(ch),
        }
    }
    rendered.push(quote);
    rendered
}

fn python_repr_string_list<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let values: Vec<String> = values.into_iter().map(python_repr_string).collect();
    format!("[{}]", values.join(", "))
}

fn python_repr_string_pair(left: &str, right: &str) -> String {
    format!(
        "({}, {})",
        python_repr_string(left),
        python_repr_string(right)
    )
}

fn python_repr_value(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(value) => if *value { "True" } else { "False" }.to_owned(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => python_repr_string(value),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(python_repr_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| {
                    format!("{}: {}", python_repr_string(key), python_repr_value(value))
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn python_string_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(value) => python_repr_value(value),
        None => "None".to_owned(),
    }
}

fn python_json_equal(left: &Value, right: &Value) -> bool {
    crate::assessment::py_equal(left, right).unwrap_or(false)
}
fn python_optional_json_equal(left: Option<&Value>, right: Option<&Value>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => python_json_equal(left, right),
        _ => false,
    }
}
fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn string_set(value: &Value, key: &str) -> BTreeSet<String> {
    array(value, key)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn rows<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> + use<'a> {
    array(value, key).iter().filter(|row| row.is_object())
}
fn local_only_absolute(value: &Value) -> bool {
    match value {
        Value::String(text) => ["/srv/", "/home/", "/tmp/", "/var/tmp/"]
            .iter()
            .any(|prefix| text.starts_with(prefix)),
        Value::Array(values) => values.iter().any(local_only_absolute),
        Value::Object(values) => values.values().any(local_only_absolute),
        _ => false,
    }
}
fn first_forbidden_content_fields(value: &Value) -> Option<Vec<String>> {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(values) => {
                let mut leaked: Vec<String> = values
                    .keys()
                    .filter(|key| FORBIDDEN_CONTENT_KEYS.contains(&key.as_str()))
                    .cloned()
                    .collect();
                leaked.sort_unstable();
                if !leaked.is_empty() {
                    return Some(leaked);
                }
                pending.extend(values.values());
            }
            Value::Array(values) => pending.extend(values),
            _ => {}
        }
    }
    None
}
fn path_parts<'a>(path: &'a str, root: &str) -> Vec<&'a str> {
    path.strip_prefix(root).unwrap_or("").split('/').collect()
}
fn safe_relative_path(path: &str) -> bool {
    RelativePath::parse(path).is_ok()
}

fn lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn native_slug_id(value: &str, prefix: &str) -> bool {
    let Some(suffix) = value.strip_prefix(prefix) else {
        return false;
    };
    !suffix.is_empty()
        && suffix
            .split(|character| character == '.' || character == '-')
            .all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            })
}

fn native_form_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("tos.form.") else {
        return false;
    };
    let mut bytes = suffix.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn rights_scope_contains(value: &Value, ids: &[&str]) -> bool {
    let Some(scopes) = value.get("scope_refs").and_then(Value::as_array) else {
        return true;
    };
    ids.iter()
        .all(|id| scopes.iter().any(|scope| scope.as_str() == Some(id)))
}

fn json_integer(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn discovery_semantic_issues(value: &Value) -> Vec<String> {
    let Some(object) = value.as_object() else {
        return vec!["discovery record is not an object".into()];
    };
    let mut issues = Vec::new();
    let raw_channels = object.get("channels");
    let channels = raw_channels
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if raw_channels.is_some_and(|raw| !raw.is_array()) {
        issues.push("discovery channels are not an array".into());
    }
    let channel_rows: Vec<&Value> = channels.iter().filter(|row| row.is_object()).collect();
    let channel_ids: Vec<&str> = channel_rows
        .iter()
        .filter_map(|row| string(row, "channel_id"))
        .collect();
    if channel_ids.iter().collect::<BTreeSet<_>>().len() != channel_ids.len() {
        issues.push("discovery channel IDs are not unique".into());
    }
    let sequences: Vec<i64> = channel_rows
        .iter()
        .filter_map(|row| row.get("sequence").and_then(json_integer))
        .collect();
    if sequences.iter().collect::<BTreeSet<_>>().len() != sequences.len() {
        issues.push("discovery channel sequence values are not unique".into());
    }
    let general_web_sequences: Vec<i64> = channel_rows
        .iter()
        .filter(|row| string(row, "channel_type") == Some("general-web-search"))
        .filter_map(|row| row.get("sequence").and_then(json_integer))
        .collect();
    if let (Some(max_sequence), Some(max_general_web)) =
        (sequences.iter().max(), general_web_sequences.iter().max())
    {
        if max_sequence != max_general_web {
            issues.push("general web search is not the final discovery channel".into());
        }
    }

    let mut result_ids = BTreeSet::new();
    let mut expected_selected = BTreeSet::new();
    let mut expected_rejected = BTreeSet::new();
    for channel in channel_rows {
        let channel_id = python_string_value(channel.get("channel_id"));
        let results = array(channel, "results");
        let ranks: Vec<Value> = results
            .iter()
            .filter(|row| row.is_object())
            .map(|row| row.get("rank").cloned().unwrap_or(Value::Null))
            .collect();
        let expected_ranks: Vec<Value> = (1..=ranks.len()).map(|rank| json!(rank)).collect();
        if ranks != expected_ranks {
            issues.push(format!(
                "discovery result order for {channel_id} is not contiguous from rank 1"
            ));
        }
        for result in results.iter().filter(|row| row.is_object()) {
            let Some(id) = string(result, "result_id") else {
                continue;
            };
            if !result_ids.insert(id.to_owned()) {
                issues.push(format!("duplicate discovery result_id: {id}"));
            }
            match string(result, "decision") {
                Some("select") => {
                    expected_selected.insert(id.to_owned());
                }
                Some("reject") => {
                    expected_rejected.insert(id.to_owned());
                }
                _ => {}
            }
        }
    }
    let selected: BTreeSet<String> = array(value, "selected_result_ids")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let rejected: BTreeSet<String> = array(value, "rejected_result_ids")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if selected != expected_selected {
        issues.push("selected_result_ids do not match results whose decision is select".into());
    }
    if rejected != expected_rejected {
        issues.push("rejected_result_ids do not match results whose decision is reject".into());
    }
    if selected.intersection(&rejected).next().is_some() {
        issues.push("discovery result is both selected and rejected".into());
    }
    let unresolved: Vec<String> = selected
        .union(&rejected)
        .filter(|id| !result_ids.contains(*id))
        .cloned()
        .collect();
    if !unresolved.is_empty() {
        issues.push(format!(
            "discovery decision references unknown results: {}",
            python_repr_string_list(unresolved.iter().map(String::as_str))
        ));
    }
    let comparison_ids: BTreeSet<&str> = array(value, "channel_comparison")
        .iter()
        .filter(|row| row.is_object())
        .filter_map(|row| string(row, "channel_id"))
        .collect();
    let expected: BTreeSet<&str> = channel_ids.into_iter().collect();
    if comparison_ids != expected {
        issues.push("discovery channel comparison does not cover the exact channel set".into());
    }
    issues
}

fn string_values<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => out.push(text),
        Value::Array(values) => values.iter().for_each(|value| string_values(value, out)),
        Value::Object(values) => values.values().for_each(|value| string_values(value, out)),
        _ => {}
    }
}

fn leaks_local_path(value: &Value) -> bool {
    let mut strings = Vec::new();
    string_values(value, &mut strings);
    strings.iter().any(|text| {
        text.starts_with('/')
            || text.starts_with("~/")
            || text.contains("/home/")
            || text.contains("/srv/")
    })
}

fn string_values_set(value: &Value) -> BTreeSet<String> {
    let mut strings = Vec::new();
    string_values(value, &mut strings);
    strings.into_iter().map(str::to_owned).collect()
}

fn private_handoff_semantic_issues(value: &Value, destination_exists: bool) -> Vec<String> {
    let Some(object) = value.as_object() else {
        return vec!["private-evidence handoff is not an object".into()];
    };
    let mut issues = Vec::new();
    let forbidden = string_set(
        object.get("disclosure_policy").unwrap_or(&Value::Null),
        "forbidden_classes",
    );
    let mut missing: Vec<&str> = PRIVATE_HANDOFF_REQUIRED_FORBIDDEN_CLASSES
        .iter()
        .copied()
        .filter(|class| !forbidden.contains(*class))
        .collect();
    missing.sort_unstable();
    if !missing.is_empty() {
        issues.push(format!(
            "private-evidence handoff omits forbidden classes: {}",
            python_repr_string_list(missing.iter().copied())
        ));
    }
    if leaks_local_path(value) {
        issues.push("private-evidence handoff contains an absolute local path".into());
    }
    let status = string(value, "status").unwrap_or("");
    let destination = value.get("destination").unwrap_or(&Value::Null);
    if status == "contract_frozen_raw_unopened"
        && string(destination, "artifact_path").is_some()
        && destination_exists
    {
        issues.push(
            "contract-only private-evidence handoff already has a materialized derivative".into(),
        );
    }
    issues
}

fn public_derivative_semantic_issues(derivative: &Value, handoff: &Value) -> Vec<String> {
    if !derivative.is_object() {
        return vec!["public evidence derivative is not an object".into()];
    }
    if !handoff.is_object() {
        return vec!["public evidence derivative has no handoff object".into()];
    }
    let mut issues = Vec::new();
    if leaks_local_path(derivative) {
        issues.push("public evidence derivative contains an absolute local path".into());
    }
    let handoff_boundary = handoff.get("source_boundary").unwrap_or(&Value::Null);
    let derivative_boundary = derivative.get("source_boundary").unwrap_or(&Value::Null);
    for (label, actual, expected) in [
        (
            "handoff_id",
            derivative.get("handoff_id").unwrap_or(&Value::Null),
            handoff.get("handoff_id").unwrap_or(&Value::Null),
        ),
        (
            "evidence_set_id",
            derivative_boundary
                .get("evidence_set_id")
                .unwrap_or(&Value::Null),
            handoff_boundary
                .get("evidence_set_id")
                .unwrap_or(&Value::Null),
        ),
        (
            "public_return_handle",
            derivative_boundary
                .get("public_return_handle")
                .unwrap_or(&Value::Null),
            handoff_boundary
                .get("public_return_handle")
                .unwrap_or(&Value::Null),
        ),
    ] {
        if !python_json_equal(actual, expected) {
            issues.push(format!(
                "public evidence derivative {label} does not match its handoff"
            ));
        }
    }
    let policy = handoff.get("disclosure_policy").unwrap_or(&Value::Null);
    let disclosed = string_set(derivative, "disclosed_classes");
    let allowed = string_set(policy, "allowed_classes");
    let unexpected: Vec<String> = disclosed.difference(&allowed).cloned().collect();
    if !unexpected.is_empty() {
        issues.push(format!(
            "public evidence derivative discloses non-allowed classes: {}",
            python_repr_string_list(unexpected.iter().map(String::as_str))
        ));
    }
    let aggregation = derivative.get("aggregation").unwrap_or(&Value::Null);
    let derivative_floor = aggregation
        .get("minimum_group_size_applied")
        .and_then(python_int);
    let handoff_floor = policy
        .get("minimum_aggregation_group_size")
        .and_then(python_int);
    if derivative_floor
        .zip(handoff_floor)
        .is_some_and(|(actual, expected)| actual < expected)
    {
        issues.push("public evidence derivative aggregation floor is below its handoff".into());
    }
    let forbidden = string_set(policy, "forbidden_classes");
    let mut strings = Vec::new();
    string_values(derivative, &mut strings);
    let mut leaked: Vec<&str> = strings
        .into_iter()
        .filter(|text| forbidden.contains(*text))
        .collect();
    leaked.sort_unstable();
    if !leaked.is_empty() {
        issues.push(format!(
            "public evidence derivative names forbidden disclosure classes: {}",
            python_repr_string_list(leaked.iter().copied())
        ));
    }
    issues
}

fn manual_error_ledger_semantic_issues(
    records: &[Value],
    handoff: &Value,
    derivative: &Value,
    provenance_events: &[Value],
    handoff_sha256: Option<&str>,
    derivative_sha256: Option<&str>,
    ledger_sha256: Option<&str>,
) -> Vec<String> {
    if records.len() != 2 {
        return vec!["manual error ledger must preserve one state and one review episode".into()];
    }
    let mut issues = Vec::new();
    if string(&records[0], "record_type") != Some("ledger_state") {
        issues.push("manual error ledger first record is not the historical state".into());
    }
    if string(&records[1], "record_type") != Some("review_episode") {
        issues.push("manual error ledger second record is not the review episode".into());
        return issues;
    }
    let episode = &records[1];
    if leaks_local_path(&json!([episode, provenance_events])) {
        issues.push("manual error ledger evidence contains an absolute local path".into());
    }
    let derivative_ref = string(
        handoff.get("destination").unwrap_or(&Value::Null),
        "artifact_path",
    );
    let evidence_boundary = episode.get("evidence_boundary").unwrap_or(&Value::Null);
    for (label, value, expected_ref, expected_sha) in [
        (
            "handoff",
            evidence_boundary.get("handoff").unwrap_or(&Value::Null),
            Some(PRIVATE_HANDOFF_PATH),
            handoff_sha256,
        ),
        (
            "aggregate derivative",
            evidence_boundary
                .get("aggregate_derivative")
                .unwrap_or(&Value::Null),
            derivative_ref,
            derivative_sha256,
        ),
    ] {
        let Some(object) = value.as_object() else {
            issues.push(format!("manual error ledger {label} reference is missing"));
            continue;
        };
        let expected_ref_value = expected_ref
            .map(|value| Value::String(value.to_owned()))
            .unwrap_or(Value::Null);
        let expected_sha_value = expected_sha
            .map(|value| Value::String(value.to_owned()))
            .unwrap_or(Value::Null);
        if !python_json_equal(
            object.get("ref").unwrap_or(&Value::Null),
            &expected_ref_value,
        ) || !python_json_equal(
            object.get("sha256").unwrap_or(&Value::Null),
            &expected_sha_value,
        ) {
            issues.push(format!(
                "manual error ledger {label} reference or digest drifted"
            ));
        }
    }
    let source_boundary = handoff.get("source_boundary").unwrap_or(&Value::Null);
    for (label, actual, expected) in [
        (
            "private evidence set",
            evidence_boundary
                .get("private_evidence_set_id")
                .unwrap_or(&Value::Null),
            source_boundary
                .get("evidence_set_id")
                .unwrap_or(&Value::Null),
        ),
        (
            "private return handle",
            evidence_boundary
                .get("private_return_handle")
                .unwrap_or(&Value::Null),
            source_boundary
                .get("public_return_handle")
                .unwrap_or(&Value::Null),
        ),
    ] {
        if !python_json_equal(actual, expected) {
            issues.push(format!(
                "manual error ledger {label} does not match handoff"
            ));
        }
    }
    let review_scope = episode.get("review_scope").unwrap_or(&Value::Null);
    let aggregation = derivative.get("aggregation").unwrap_or(&Value::Null);
    for (label, actual, expected) in [
        (
            "source-unit count",
            review_scope
                .get("source_unit_count")
                .unwrap_or(&Value::Null),
            aggregation.get("source_unit_count").unwrap_or(&Value::Null),
        ),
        (
            "candidate-observation count",
            review_scope
                .get("candidate_observation_count")
                .unwrap_or(&Value::Null),
            aggregation
                .get("candidate_observation_count")
                .unwrap_or(&Value::Null),
        ),
        (
            "source-visible posture",
            review_scope.get("source_visible").unwrap_or(&Value::Null),
            derivative
                .pointer("/evidence_posture/human_source_visible_review_observed")
                .unwrap_or(&Value::Null),
        ),
        (
            "experiment",
            episode.get("experiment_id").unwrap_or(&Value::Null),
            derivative
                .get("public_experiment_id")
                .unwrap_or(&Value::Null),
        ),
        (
            "aggregate outcomes",
            episode
                .get("aggregate_outcome_counts")
                .unwrap_or(&Value::Null),
            derivative
                .get("aggregate_outcome_counts")
                .unwrap_or(&Value::Null),
        ),
        (
            "error taxonomy",
            episode
                .get("aggregate_error_taxonomy")
                .unwrap_or(&Value::Null),
            derivative
                .get("aggregate_error_taxonomy")
                .unwrap_or(&Value::Null),
        ),
        (
            "human time",
            episode.get("human_time").unwrap_or(&Value::Null),
            derivative
                .get("aggregate_human_time_with_confounds")
                .unwrap_or(&Value::Null),
        ),
    ] {
        if !python_json_equal(actual, expected) {
            issues.push(format!(
                "manual error ledger {label} does not match derivative"
            ));
        }
    }
    let decision_total: i128 = array(episode, "aggregate_outcome_counts")
        .iter()
        .filter(|row| string(row, "code").is_some_and(|code| code.starts_with("decision-")))
        .filter_map(|row| row.get("count").and_then(python_int))
        .sum();
    if Some(decision_total)
        != review_scope
            .get("candidate_observation_count")
            .and_then(python_int)
    {
        issues.push("manual error ledger decision counts do not close to observations".into());
    }
    let adjudication = episode.get("adjudication").unwrap_or(&Value::Null);
    if !python_json_equal(
        adjudication
            .get("candidate_dispositions_recorded")
            .unwrap_or(&Value::Null),
        review_scope
            .get("candidate_observation_count")
            .unwrap_or(&Value::Null),
    ) {
        issues.push("manual error ledger dispositions do not close to observations".into());
    }
    for (field, expected) in [
        ("source_transcriptions_accepted", json!(0)),
        ("independent_gold_units", json!(0)),
        ("general_method_winner", json!(false)),
        ("content_authority", json!(false)),
        ("routine_human_backlog_created", json!(false)),
    ] {
        if !python_optional_json_equal(adjudication.get(field), Some(&expected)) {
            issues.push(format!("manual error ledger improperly opens {field}"));
        }
    }
    if provenance_events.len() != 1 {
        issues.push("manual error ledger must have exactly one provenance event".into());
        return issues;
    }
    let event = &provenance_events[0];
    if !python_optional_json_equal(event.get("event_id"), episode.get("provenance_event_ref")) {
        issues.push("manual error ledger provenance event reference drifted".into());
    }
    if string(event, "event_type") != Some("export") {
        issues.push("manual error ledger provenance event is not an export".into());
    }
    if string(event, "status") != Some("completed_with_warnings") {
        issues.push("manual error ledger provenance must retain warnings".into());
    }
    if !event
        .get("rights_basis_ref")
        .unwrap_or(&Value::Null)
        .is_null()
    {
        issues.push("manual error ledger provenance claims a rights basis".into());
    }
    let actual_inputs: BTreeSet<(Option<String>, Option<String>)> = array(event, "inputs")
        .iter()
        .filter(|row| row.is_object())
        .map(|row| {
            (
                string(row, "ref").map(str::to_owned),
                string(row, "sha256").map(str::to_owned),
            )
        })
        .collect();
    let expected_inputs = BTreeSet::from([
        (
            Some(PRIVATE_HANDOFF_PATH.to_owned()),
            handoff_sha256.map(str::to_owned),
        ),
        (
            derivative_ref.map(str::to_owned),
            derivative_sha256.map(str::to_owned),
        ),
        (
            Some(MANUAL_LEDGER_PATH.to_owned()),
            Some(PRIOR_EMPTY_MANUAL_LEDGER_SHA256.to_owned()),
        ),
    ]);
    if actual_inputs != expected_inputs {
        issues.push("manual error ledger provenance inputs or digests drifted".into());
    }
    let actual_outputs: BTreeSet<(Option<String>, Option<String>)> = array(event, "outputs")
        .iter()
        .filter(|row| row.is_object())
        .map(|row| {
            (
                string(row, "ref").map(str::to_owned),
                string(row, "sha256").map(str::to_owned),
            )
        })
        .collect();
    let expected_outputs = BTreeSet::from([(
        Some(MANUAL_LEDGER_PATH.to_owned()),
        ledger_sha256.map(str::to_owned),
    )]);
    if actual_outputs != expected_outputs {
        issues.push("manual error ledger provenance output digest drifted".into());
    }
    let expected_configuration = json!({
        "aggregate_only": true,
        "candidate_observation_count": 30,
        "human_review_pass_count": 1,
        "private_raw_reopened": false,
        "source_unit_count": 10,
        "unit_level_judgments_embedded": false,
    });
    if !python_optional_json_equal(
        event.pointer("/method/configuration"),
        Some(&expected_configuration),
    ) {
        issues.push("manual error ledger provenance configuration drifted".into());
    }
    issues
}

fn python_int(value: &Value) -> Option<i128> {
    value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
        .or_else(|| value.as_bool().map(|value| i128::from(value as i64)))
}

fn transfer_candidate_crosswalk_issues(
    crosswalk: &Value,
    transfer_plan: &Value,
    target_map: &Value,
    label_map: &Value,
) -> Vec<String> {
    if !crosswalk.is_object()
        || !transfer_plan.is_object()
        || !target_map.is_object()
        || !label_map.is_object()
    {
        return vec!["transfer candidate crosswalk inputs are not objects".into()];
    }
    let mut issues = Vec::new();
    let work_ref = crosswalk.get("work_ref").unwrap_or(&Value::Null);
    let mut expected = BTreeMap::new();
    for candidate in array(transfer_plan, "candidate_target_units") {
        if !candidate.is_object()
            || !python_optional_json_equal(candidate.get("work_ref"), Some(work_ref))
        {
            continue;
        }
        if let Some(id) = string(candidate, "unit_id") {
            expected.insert(id.to_owned(), candidate);
        }
    }
    let pairings: BTreeMap<&str, &Value> = array(label_map, "pairings")
        .iter()
        .filter(|pairing| pairing.is_object())
        .filter_map(|pairing| Some((string(pairing, "unit_key")?, pairing)))
        .collect();
    let starts: Vec<&Value> = array(target_map, "unit_starts")
        .iter()
        .filter(|start| {
            start.is_object()
                && start.get("pdf_page").and_then(python_int).is_some()
                && string(start, "unit_key").is_some()
        })
        .collect();
    let actual = crosswalk.get("candidates").and_then(Value::as_array);
    let Some(actual) = actual else {
        return vec!["transfer candidate crosswalk candidates are not a list".into()];
    };
    let actual_ids: Vec<&str> = actual
        .iter()
        .filter(|row| row.is_object())
        .filter_map(|row| string(row, "candidate_unit_id"))
        .collect();
    if actual_ids.iter().collect::<BTreeSet<_>>().len() != actual_ids.len() {
        issues.push("transfer candidate crosswalk repeats a candidate unit".into());
    }
    let expected_ids: BTreeSet<&str> = expected.keys().map(String::as_str).collect();
    let actual_id_set: BTreeSet<&str> = actual_ids.iter().copied().collect();
    if actual_id_set != expected_ids {
        issues.push("transfer candidate crosswalk does not close the work quota".into());
    }

    let mut possible_pairing_count = 0usize;
    let mut pages_with_starts = 0usize;
    for candidate in actual {
        if !candidate.is_object() {
            issues.push("transfer candidate crosswalk contains a non-object row".into());
            continue;
        }
        let unit_id = string(candidate, "candidate_unit_id").unwrap_or("");
        let Some(expected) = expected.get(unit_id) else {
            continue;
        };
        let page = expected.get("page").and_then(python_int);
        if !python_optional_json_equal(
            candidate.get("candidate_anchor_ref"),
            expected.get("anchor_ref"),
        ) {
            issues.push(format!("{unit_id} candidate anchor drifted"));
        }
        if !python_optional_json_equal(candidate.get("target_pdf_page"), expected.get("page")) {
            issues.push(format!("{unit_id} candidate page drifted"));
        }
        if !python_optional_json_equal(candidate.get("stratum"), expected.get("stratum")) {
            issues.push(format!("{unit_id} candidate stratum drifted"));
        }
        let Some(page) = page else {
            issues.push(format!("{unit_id} has no integer target page"));
            continue;
        };
        let prior: Vec<&Value> = starts
            .iter()
            .copied()
            .filter(|start| start.get("pdf_page").and_then(python_int).unwrap_or(0) < page)
            .collect();
        let on_page: Vec<&Value> = starts
            .iter()
            .copied()
            .filter(|start| start.get("pdf_page").and_then(python_int) == Some(page))
            .collect();
        let following: Vec<&Value> = starts
            .iter()
            .copied()
            .filter(|start| start.get("pdf_page").and_then(python_int).unwrap_or(0) > page)
            .collect();
        let mut expected_keys: Vec<Value> = Vec::new();
        if let Some(last) = prior.last() {
            expected_keys.push(last.get("unit_key").cloned().unwrap_or(Value::Null));
        }
        expected_keys.extend(
            on_page
                .iter()
                .map(|start| start.get("unit_key").cloned().unwrap_or(Value::Null)),
        );
        let expected_start_keys: Vec<Value> = on_page
            .iter()
            .map(|start| start.get("unit_key").cloned().unwrap_or(Value::Null))
            .collect();
        let expected_keys_value = json!(expected_keys);
        if !python_optional_json_equal(
            candidate.get("possible_unit_keys"),
            Some(&expected_keys_value),
        ) {
            issues.push(format!("{unit_id} possible unit keys drifted"));
        }
        let expected_start_keys_value = json!(expected_start_keys);
        if !python_optional_json_equal(
            candidate.get("starts_on_page_unit_keys"),
            Some(&expected_start_keys_value),
        ) {
            issues.push(format!("{unit_id} on-page unit starts drifted"));
        }
        let relation = if on_page.is_empty() {
            "within-one-proposed-numbered-unit"
        } else {
            "prior-unit-spill-plus-unit-starts"
        };
        if string(candidate, "page_relation") != Some(relation) {
            issues.push(format!("{unit_id} page relation drifted"));
        }
        if let Some(next) = following.first() {
            let expected_next = json!({
                "unit_key": next.get("unit_key").cloned().unwrap_or(Value::Null),
                "target_pdf_page": next.get("pdf_page").cloned().unwrap_or(Value::Null),
            });
            if !python_optional_json_equal(
                candidate.get("next_proposed_start"),
                Some(&expected_next),
            ) {
                issues.push(format!("{unit_id} following proposed start drifted"));
            }
        } else {
            issues.push(format!("{unit_id} has no following proposed start"));
        }
        for key in &expected_keys {
            let Some(key) = key.as_str() else { continue };
            match pairings.get(key) {
                None => issues.push(format!("{unit_id} unit {key} has no shared-label pairing")),
                Some(pairing)
                    if pairing
                        .get("translation_alignment_claimed")
                        .and_then(Value::as_bool)
                        != Some(false) =>
                {
                    issues.push(format!("{unit_id} unit {key} claims translation alignment"));
                }
                Some(_) => {}
            }
        }
        possible_pairing_count += expected_keys.len();
        pages_with_starts += usize::from(!on_page.is_empty());
    }
    let summary = crosswalk.get("summary").unwrap_or(&Value::Null);
    if !summary.is_object() {
        issues.push("transfer candidate crosswalk summary is not an object".into());
        return issues;
    }
    let random_count = expected
        .values()
        .filter(|candidate| string(candidate, "stratum") == Some("random"))
        .count();
    let hard_count = expected
        .values()
        .filter(|candidate| string(candidate, "stratum") == Some("hard"))
        .count();
    let expected_summary = json!({
        "candidate_page_count": expected.len(),
        "random_page_count": random_count,
        "hard_page_count": hard_count,
        "page_with_unit_start_count": pages_with_starts,
        "page_without_unit_start_count": expected.len() as i128 - pages_with_starts as i128,
        "possible_pairing_count": possible_pairing_count,
    });
    for (field, expected_value) in expected_summary.as_object().into_iter().flatten() {
        if !python_optional_json_equal(summary.get(field), Some(expected_value)) {
            issues.push(format!(
                "transfer candidate crosswalk summary {field} drifted"
            ));
        }
    }
    issues
}

fn target_structural_crosswalk_issues(
    crosswalk: &Value,
    transfer_plan: &Value,
    target_map: &Value,
) -> Vec<String> {
    if !crosswalk.is_object() || !transfer_plan.is_object() || !target_map.is_object() {
        return vec!["target structural crosswalk inputs are not objects".into()];
    }
    let Some(series) = target_map.get("series").and_then(Value::as_array) else {
        return vec!["target structural crosswalk target series are not a list".into()];
    };
    let Some(transfer_candidates) = transfer_plan
        .get("candidate_target_units")
        .and_then(Value::as_array)
    else {
        return vec!["target structural crosswalk transfer candidates are not a list".into()];
    };
    let work_ref = crosswalk.get("work_ref").unwrap_or(&Value::Null);
    let expected: BTreeMap<&str, &Value> = transfer_candidates
        .iter()
        .filter(|candidate| {
            candidate.is_object()
                && python_optional_json_equal(candidate.get("work_ref"), Some(work_ref))
        })
        .filter_map(|candidate| Some((string(candidate, "unit_id")?, candidate)))
        .collect();
    let mut issues = Vec::new();
    let mut starts = Vec::<(String, i128)>::new();
    for row in series {
        if !row.is_object() {
            issues.push("target structural crosswalk map contains a non-object series".into());
            continue;
        }
        let Some(series_key) = string(row, "series_key") else {
            issues.push("target structural crosswalk map series is malformed".into());
            continue;
        };
        let Some(unit_starts) = row.get("unit_starts").and_then(Value::as_array) else {
            issues.push("target structural crosswalk map series is malformed".into());
            continue;
        };
        for start in unit_starts {
            if !start.is_object() {
                issues.push("target structural crosswalk map has a non-object start".into());
                continue;
            }
            let (Some(unit_key), Some(page)) = (
                string(start, "unit_key"),
                start.get("pdf_page").and_then(python_int),
            ) else {
                issues.push("target structural crosswalk map start is malformed".into());
                continue;
            };
            starts.push((format!("{series_key}:{unit_key}"), page));
        }
    }
    let Some(actual) = crosswalk.get("candidates").and_then(Value::as_array) else {
        return vec!["target structural crosswalk candidates are not a list".into()];
    };
    let actual_ids: Vec<&str> = actual
        .iter()
        .filter(|row| row.is_object())
        .filter_map(|row| string(row, "candidate_unit_id"))
        .collect();
    if actual_ids.iter().collect::<BTreeSet<_>>().len() != actual_ids.len() {
        issues.push("target structural crosswalk repeats a candidate unit".into());
    }
    if actual_ids.iter().copied().collect::<BTreeSet<_>>() != expected.keys().copied().collect() {
        issues.push("target structural crosswalk does not close the work quota".into());
    }
    if !python_optional_json_equal(
        crosswalk.get("target_expression_ref"),
        target_map.get("expression_ref"),
    ) {
        issues.push("target structural crosswalk expression drifted".into());
    }
    if !python_optional_json_equal(crosswalk.get("target_item_ref"), target_map.get("item_ref")) {
        issues.push("target structural crosswalk item drifted".into());
    }
    let mut possible_route_count = 0usize;
    let mut pages_with_starts = 0usize;
    for candidate in actual {
        if !candidate.is_object() {
            issues.push("target structural crosswalk contains a non-object row".into());
            continue;
        }
        let unit_id = string(candidate, "candidate_unit_id").unwrap_or("");
        let Some(expected) = expected.get(unit_id) else {
            continue;
        };
        let page = expected.get("page").and_then(python_int);
        if !python_optional_json_equal(
            candidate.get("candidate_anchor_ref"),
            expected.get("anchor_ref"),
        ) {
            issues.push(format!("{unit_id} target candidate anchor drifted"));
        }
        if !python_optional_json_equal(candidate.get("target_pdf_page"), expected.get("page")) {
            issues.push(format!("{unit_id} target candidate page drifted"));
        }
        if !python_optional_json_equal(candidate.get("stratum"), expected.get("stratum")) {
            issues.push(format!("{unit_id} target candidate stratum drifted"));
        }
        let Some(page) = page else {
            issues.push(format!("{unit_id} has no integer target page"));
            continue;
        };
        let prior: Vec<&(String, i128)> = starts
            .iter()
            .filter(|(_, start_page)| *start_page < page)
            .collect();
        let on_page: Vec<&(String, i128)> = starts
            .iter()
            .filter(|(_, start_page)| *start_page == page)
            .collect();
        let following: Vec<&(String, i128)> = starts
            .iter()
            .filter(|(_, start_page)| *start_page > page)
            .collect();
        if prior.is_empty() || following.is_empty() {
            issues.push(format!("{unit_id} lacks surrounding proposed starts"));
            continue;
        }
        let expected_refs: Vec<Value> =
            std::iter::once(json!(prior.last().map(|row| row.0.as_str()).unwrap_or("")))
                .chain(on_page.iter().map(|row| json!(row.0)))
                .collect();
        let expected_start_refs: Vec<Value> = on_page.iter().map(|row| json!(row.0)).collect();
        let expected_refs_value = json!(expected_refs);
        if !python_optional_json_equal(
            candidate.get("possible_target_unit_refs"),
            Some(&expected_refs_value),
        ) {
            issues.push(format!("{unit_id} possible target unit refs drifted"));
        }
        let expected_start_refs_value = json!(expected_start_refs);
        if !python_optional_json_equal(
            candidate.get("starts_on_page_target_unit_refs"),
            Some(&expected_start_refs_value),
        ) {
            issues.push(format!("{unit_id} on-page target unit starts drifted"));
        }
        let relation = if on_page.is_empty() {
            "within-one-proposed-target-numbered-unit"
        } else {
            "prior-target-unit-spill-plus-unit-starts"
        };
        if string(candidate, "page_relation") != Some(relation) {
            issues.push(format!("{unit_id} target page relation drifted"));
        }
        let next = following[0];
        let expected_next = json!({ "target_unit_ref": next.0, "target_pdf_page": next.1 });
        if !python_optional_json_equal(candidate.get("next_proposed_start"), Some(&expected_next)) {
            issues.push(format!("{unit_id} following target start drifted"));
        }
        possible_route_count += expected_refs.len();
        pages_with_starts += usize::from(!on_page.is_empty());
    }
    let summary = crosswalk.get("summary").unwrap_or(&Value::Null);
    if !summary.is_object() {
        issues.push("target structural crosswalk summary is not an object".into());
        return issues;
    }
    let random_count = expected
        .values()
        .filter(|candidate| string(candidate, "stratum") == Some("random"))
        .count();
    let hard_count = expected
        .values()
        .filter(|candidate| string(candidate, "stratum") == Some("hard"))
        .count();
    let expected_summary = json!({
        "candidate_page_count": expected.len(),
        "random_page_count": random_count,
        "hard_page_count": hard_count,
        "page_with_unit_start_count": pages_with_starts,
        "page_without_unit_start_count": expected.len() as i128 - pages_with_starts as i128,
        "possible_target_unit_route_count": possible_route_count,
    });
    for (field, expected_value) in expected_summary.as_object().into_iter().flatten() {
        if !python_optional_json_equal(summary.get(field), Some(expected_value)) {
            issues.push(format!(
                "target structural crosswalk summary {field} drifted"
            ));
        }
    }
    issues
}

fn hierarchical_target_map_issues(payload: &Value) -> Vec<String> {
    if !payload.is_object() {
        return vec!["hierarchical target numbered-unit map is not an object".into()];
    }
    let Some(series_rows) = payload.get("series").and_then(Value::as_array) else {
        return vec!["hierarchical target numbered-unit series are not a list".into()];
    };
    let mut issues = Vec::new();
    let method = payload.get("method").unwrap_or(&Value::Null);
    if !method.is_object() {
        issues.push("hierarchical target map method is not an object".into());
    }
    let trials_value = method.get("series_trials").unwrap_or(&Value::Null);
    if !trials_value.is_array() {
        issues.push("hierarchical target map method trials are not a list".into());
    }
    let mut trial_by_key: BTreeMap<String, &Value> = BTreeMap::new();
    for trial in array(method, "series_trials") {
        if !trial.is_object() {
            issues.push("hierarchical target map contains a non-object method trial".into());
            continue;
        }
        let Some(key) = string(trial, "series_key") else {
            issues.push("hierarchical target map method trial has no string series key".into());
            continue;
        };
        if trial_by_key.insert(key.to_owned(), trial).is_some() {
            issues.push(format!(
                "hierarchical target map repeats method trial {key}"
            ));
        }
    }
    let mut series_keys = Vec::new();
    let mut series_sequences = Vec::new();
    let mut all_pages: Vec<i128> = Vec::new();
    let mut all_anchor_refs = Vec::new();
    let mut expected_override_refs = Vec::new();
    let mut expected_override_pages = BTreeSet::new();
    let mut expected_machine_count: i128 = 0;
    for (index, series) in series_rows.iter().enumerate() {
        if !series.is_object() {
            issues.push("hierarchical target map contains a non-object series".into());
            continue;
        }
        let Some(series_key) = string(series, "series_key") else {
            issues.push("hierarchical target series key is not a string".into());
            continue;
        };
        series_keys.push(series_key.to_owned());
        series_sequences.push(
            series
                .get("series_sequence")
                .cloned()
                .unwrap_or(Value::Null),
        );
        let expected_series_sequence = json!(index + 1);
        if !python_optional_json_equal(
            series.get("series_sequence"),
            Some(&expected_series_sequence),
        ) {
            issues.push(format!(
                "hierarchical target series {series_key} sequence drifted"
            ));
        }
        let expected_count = series.get("expected_unit_count").and_then(python_int);
        let Some(starts) = series.get("unit_starts").and_then(Value::as_array) else {
            issues.push(format!(
                "hierarchical target series {series_key} starts are not a list"
            ));
            continue;
        };
        let Some(expected_count) = expected_count.and_then(|value| usize::try_from(value).ok())
        else {
            issues.push(format!(
                "hierarchical target series {series_key} count drifted"
            ));
            continue;
        };
        if starts.len() != expected_count {
            issues.push(format!(
                "hierarchical target series {series_key} count drifted"
            ));
            continue;
        }
        let expected_keys: Vec<Value> =
            (1..=expected_count).map(|n| json!(n.to_string())).collect();
        let actual_keys: Vec<Value> = starts
            .iter()
            .map(|start| start.get("unit_key").cloned().unwrap_or(Value::Null))
            .collect();
        let actual_sequences: Vec<Value> = starts
            .iter()
            .map(|start| start.get("sequence").cloned().unwrap_or(Value::Null))
            .collect();
        let expected_sequences: Vec<Value> = (1..=expected_count).map(|n| json!(n)).collect();
        if !python_json_equal(&json!(actual_keys), &json!(expected_keys)) {
            issues.push(format!(
                "hierarchical target series {series_key} unit keys drifted"
            ));
        }
        if !python_json_equal(&json!(actual_sequences), &json!(expected_sequences)) {
            issues.push(format!(
                "hierarchical target series {series_key} unit sequences drifted"
            ));
        }
        let start_page = series.get("start_page").and_then(python_int);
        let end_page = series.get("end_page").and_then(python_int);
        let mut series_pages = Vec::new();
        let mut series_override_keys = Vec::new();
        for start in starts {
            if !start.is_object() {
                continue;
            }
            let unit_key = start
                .get("unit_key")
                .map(|value| python_string_value(Some(value)))
                .unwrap_or_else(|| "None".to_owned());
            let Some(page) = start.get("pdf_page").and_then(python_int) else {
                continue;
            };
            series_pages.push(page);
            all_pages.push(page);
            if let Some(anchor_ref) = string(start, "anchor_ref") {
                all_anchor_refs.push(anchor_ref.to_owned());
            } else {
                issues.push(format!(
                    "hierarchical target unit {series_key}:{unit_key} has no anchor ref"
                ));
            }
            if !start_page
                .zip(end_page)
                .is_some_and(|(first, last)| first <= page && page <= last)
            {
                issues.push(format!(
                    "hierarchical target unit {series_key}:{unit_key} leaves its series"
                ));
            }
            let expected_resource = format!("pdf-page-{page:04}");
            if string(start, "resource_id") != Some(expected_resource.as_str()) {
                issues.push(format!(
                    "hierarchical target unit {series_key}:{unit_key} resource drifted"
                ));
            }
            if string(start, "basis") == Some("source_visible_gap_review") {
                series_override_keys.push(unit_key.to_owned());
                expected_override_refs.push(format!("{series_key}:{unit_key}"));
                expected_override_pages.insert(page);
            }
        }
        let mut sorted_pages = series_pages.clone();
        sorted_pages.sort_unstable();
        if series_pages != sorted_pages {
            issues.push(format!(
                "hierarchical target series {series_key} pages are not monotonic"
            ));
        }
        match trial_by_key.get(series_key) {
            None => issues.push(format!(
                "hierarchical target series {series_key} has no method trial"
            )),
            Some(trial) => {
                let expected_machine = expected_count.saturating_sub(series_override_keys.len());
                let expected_count_value = json!(expected_count);
                if !python_optional_json_equal(
                    trial.get("expected_unit_count"),
                    Some(&expected_count_value),
                ) {
                    issues.push(format!(
                        "hierarchical target series {series_key} trial count drifted"
                    ));
                }
                let expected_machine_value = json!(expected_machine);
                if !python_optional_json_equal(
                    trial.get("ordered_bbox_candidate_match_count"),
                    Some(&expected_machine_value),
                ) {
                    issues.push(format!(
                        "hierarchical target series {series_key} machine count drifted"
                    ));
                }
                let expected_overrides: Vec<Value> =
                    series_override_keys.iter().map(|key| json!(key)).collect();
                let expected_overrides_value = json!(expected_overrides);
                if !python_optional_json_equal(
                    trial.get("source_visible_override_unit_keys"),
                    Some(&expected_overrides_value),
                ) {
                    issues.push(format!(
                        "hierarchical target series {series_key} override keys drifted"
                    ));
                }
                expected_machine_count += expected_machine as i128;
            }
        }
    }
    let unique_series: BTreeSet<&str> = series_keys.iter().map(String::as_str).collect();
    if unique_series.len() != series_keys.len() {
        issues.push("hierarchical target map repeats a series key".into());
    }
    let expected_series_sequences: Vec<Value> = (1..=series_rows.len()).map(|n| json!(n)).collect();
    if !python_json_equal(&json!(series_sequences), &json!(expected_series_sequences)) {
        issues.push("hierarchical target map series sequence is not contiguous".into());
    }
    let mut sorted_all_pages = all_pages.clone();
    sorted_all_pages.sort_unstable();
    if all_pages != sorted_all_pages {
        issues.push("hierarchical target map pages are not globally monotonic".into());
    }
    let unique_anchors: BTreeSet<&str> = all_anchor_refs.iter().map(String::as_str).collect();
    if unique_anchors.len() != all_anchor_refs.len() {
        issues.push("hierarchical target map repeats an anchor ref".into());
    }
    if trial_by_key
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != unique_series
    {
        issues.push("hierarchical target map method trials do not close the series".into());
    }
    let source_visible_review = method.get("source_visible_review").unwrap_or(&Value::Null);
    if !source_visible_review.is_object() {
        issues.push("hierarchical target source-visible review is not an object".into());
    } else {
        let expected_refs: Vec<Value> = expected_override_refs
            .iter()
            .map(|reference| json!(reference))
            .collect();
        let expected_refs_value = json!(expected_refs);
        if !python_optional_json_equal(
            source_visible_review.get("override_unit_refs"),
            Some(&expected_refs_value),
        ) {
            issues.push("hierarchical target override refs drifted".into());
        }
        let expected_pages: Vec<Value> = expected_override_pages
            .iter()
            .map(|page| json!(page))
            .collect();
        let expected_pages_value = json!(expected_pages);
        if !python_optional_json_equal(
            source_visible_review.get("reviewed_target_pdf_pages"),
            Some(&expected_pages_value),
        ) {
            issues.push("hierarchical target reviewed pages drifted".into());
        }
    }
    let summary = payload.get("summary").unwrap_or(&Value::Null);
    if !summary.is_object() {
        issues.push("hierarchical target map summary is not an object".into());
        return issues;
    }
    let expected_summary = json!({
        "series_count": series_rows.len(),
        "numbered_unit_count": all_pages.len(),
        "ordered_bbox_candidate_match_count": expected_machine_count,
        "source_visible_override_unit_count": expected_override_refs.len(),
        "source_visible_reviewed_page_count": expected_override_pages.len(),
        "exact_start_page_candidates_materialized": all_pages.len(),
    });
    for (field, expected_value) in expected_summary.as_object().into_iter().flatten() {
        if !python_optional_json_equal(summary.get(field), Some(expected_value)) {
            issues.push(format!("hierarchical target map summary {field} drifted"));
        }
    }
    issues
}

fn inspect_companion_district<S: LayerFamilySource + ?Sized, I: Copy + Eq>(
    inspector: &mut Inspector<'_, '_, S, I>,
) -> Result<(), ItemRefusal> {
    let handoff = inspector.required_object(PRIVATE_HANDOFF_PATH, PRIVATE_HANDOFF_SCHEMA)?;
    let mut handoff_value = None;
    let mut handoff_digest = None;
    if let Some((value, source_digest)) = handoff {
        inspector.physical_sha256(PRIVATE_HANDOFF_PATH, PRIVATE_HANDOFF_PATH)?;
        if let Some(physical_digest) = inspector
            .physical_path_facts(PRIVATE_HANDOFF_PATH)
            .and_then(|facts| facts.sha256)
        {
            if physical_digest != source_digest {
                inspector.issue(
                    PRIVATE_HANDOFF_PATH,
                    "physical-source-digest-drift",
                    "physical handoff bytes differ from exact current source bytes",
                )?;
            }
            handoff_digest = Some(physical_digest);
        }
        handoff_value = Some(value);
    }
    if let Some(handoff) = handoff_value.as_ref() {
        let destination = string(
            handoff.get("destination").unwrap_or(&Value::Null),
            "artifact_path",
        );
        let frozen = string(handoff, "status") == Some("contract_frozen_raw_unopened");
        let destination_exists = if frozen {
            match destination {
                Some(path) if safe_relative_path(path) => inspector
                    .physical_exists(path, PRIVATE_HANDOFF_PATH)?
                    .unwrap_or(false),
                Some(path) => {
                    inspector.issue(PRIVATE_HANDOFF_PATH, "unsafe-handoff-destination", path)?;
                    false
                }
                None => false,
            }
        } else {
            false
        };
        for message in private_handoff_semantic_issues(handoff, destination_exists) {
            inspector.issue(PRIVATE_HANDOFF_PATH, "private-handoff-semantic", message)?;
        }

        if let Some(destination) = destination {
            if !safe_relative_path(destination) {
                inspector.issue(
                    PRIVATE_HANDOFF_PATH,
                    "unsafe-handoff-destination",
                    destination,
                )?;
            } else if inspector
                .physical_exists(destination, PRIVATE_HANDOFF_PATH)?
                .unwrap_or(false)
            {
                if let Some((derivative, derivative_source_digest)) =
                    inspector.required_object(destination, PUBLIC_DERIVATIVE_SCHEMA)?
                {
                    inspector.physical_sha256(destination, destination)?;
                    let derivative_digest = inspector
                        .physical_path_facts(destination)
                        .and_then(|facts| facts.sha256);
                    if derivative_digest
                        .as_deref()
                        .is_some_and(|digest| digest != derivative_source_digest)
                    {
                        inspector.issue(
                            destination,
                            "physical-source-digest-drift",
                            "physical derivative bytes differ from exact current source bytes",
                        )?;
                    }
                    for message in public_derivative_semantic_issues(&derivative, handoff) {
                        inspector.issue(destination, "public-derivative-semantic", message)?;
                    }

                    let ledger = inspector
                        .required_jsonl_objects(MANUAL_LEDGER_PATH, MANUAL_LEDGER_SCHEMA)?;
                    let provenance = inspector
                        .required_jsonl_objects(MANUAL_LEDGER_PROVENANCE_PATH, PROVENANCE_SCHEMA)?;
                    let ledger_source_digest = inspector.digest(MANUAL_LEDGER_PATH)?;
                    let ledger_digest =
                        inspector.physical_sha256(MANUAL_LEDGER_PATH, MANUAL_LEDGER_PATH)?;
                    if ledger_digest
                        .as_deref()
                        .is_some_and(|digest| Some(digest) != ledger_source_digest.as_deref())
                    {
                        inspector.issue(
                            MANUAL_LEDGER_PATH,
                            "physical-source-digest-drift",
                            "physical manual ledger bytes differ from exact current source bytes",
                        )?;
                    }
                    for message in manual_error_ledger_semantic_issues(
                        &ledger,
                        handoff,
                        &derivative,
                        &provenance,
                        handoff_digest.as_deref(),
                        derivative_digest.as_deref(),
                        ledger_digest.as_deref(),
                    ) {
                        inspector.issue(MANUAL_LEDGER_PATH, "manual-ledger-semantic", message)?;
                    }
                }
            }
        }
    }

    if let Some((crosswalk, _)) =
        inspector.required_object(TRANSFER_CROSSWALK_PATH, TRANSFER_CROSSWALK_SCHEMA)?
    {
        let mut loaded = BTreeMap::<String, Value>::new();
        if let Some(inputs) = crosswalk.get("inputs").and_then(Value::as_object) {
            for (name, binding) in inputs {
                let Some(_) = binding.get("ref").and_then(Value::as_str) else {
                    continue;
                };
                let needs_document = matches!(
                    name.as_str(),
                    "transfer_plan" | "target_numbered_unit_map" | "shared_label_correspondence"
                );
                if let Some(document) = inspector.digest_bound_object(
                    TRANSFER_CROSSWALK_PATH,
                    &format!("inputs.{name}"),
                    binding,
                    needs_document,
                )? {
                    loaded.insert(name.clone(), document);
                }
            }
        }
        if let (Some(transfer_plan), Some(target_map), Some(label_map)) = (
            loaded.get("transfer_plan"),
            loaded.get("target_numbered_unit_map"),
            loaded.get("shared_label_correspondence"),
        ) {
            for message in transfer_candidate_crosswalk_issues(
                &crosswalk,
                transfer_plan,
                target_map,
                label_map,
            ) {
                inspector.issue(
                    TRANSFER_CROSSWALK_PATH,
                    "transfer-crosswalk-semantic",
                    message,
                )?;
            }
        }
    }

    for root in HIERARCHICAL_TARGET_ROOTS {
        let map_path = format!("{root}/hierarchical-numbered-unit-page-map.json");
        let Some((target_map, _)) =
            inspector.required_object(&map_path, HIERARCHICAL_TARGET_MAP_SCHEMA)?
        else {
            continue;
        };
        for binding_name in ["inventory", "work_boundary"] {
            if let Some(binding) = target_map.get(binding_name) {
                if !binding.is_object() || string(binding, "ref").is_none() {
                    continue;
                }
                inspector.digest_bound_object(&map_path, binding_name, binding, false)?;
            }
        }
        for message in hierarchical_target_map_issues(&target_map) {
            inspector.issue(&map_path, "hierarchical-target-map-semantic", message)?;
        }

        let crosswalk_path = format!("{root}/transfer-candidate-page-crosswalk.v1.json");
        let Some((crosswalk, _)) =
            inspector.required_object(&crosswalk_path, TARGET_STRUCTURAL_CROSSWALK_SCHEMA)?
        else {
            continue;
        };
        let mut transfer_plan = None;
        if let Some(inputs) = crosswalk.get("inputs").and_then(Value::as_object) {
            for (name, binding) in inputs {
                let Some(_) = binding.get("ref").and_then(Value::as_str) else {
                    continue;
                };
                let is_transfer_plan = name == "transfer_plan";
                if let Some(document) = inspector.digest_bound_object(
                    &crosswalk_path,
                    &format!("inputs.{name}"),
                    binding,
                    is_transfer_plan,
                )? {
                    if is_transfer_plan {
                        transfer_plan = Some(document);
                    }
                }
            }
        }
        if let Some(transfer_plan) = transfer_plan.as_ref() {
            for message in
                target_structural_crosswalk_issues(&crosswalk, transfer_plan, &target_map)
            {
                inspector.issue(
                    &crosswalk_path,
                    "target-structural-crosswalk-semantic",
                    message,
                )?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeArtifactCapture {
    Legacy,
    Unobserved,
    Partial,
    Invalid,
    Pending,
    Complete,
}

#[derive(Debug)]
struct NativeArtifactFormReplay {
    initial_source: Value,
    retained_form_sha256: String,
    retained_form_size: usize,
    forms_have_history: bool,
}

fn ordered_value_as_serde(value: &JsonValue, max_bytes: usize) -> Result<Value, ItemRefusal> {
    let limits = JsonLimits::new(max_bytes.min(8_388_608), 64, 300_000, 4_300)
        .map_err(|_| ItemRefusal::Budget)?;
    let raw = emit_value_preserved_json(value, limits).map_err(|error| {
        if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
            ItemRefusal::Budget
        } else {
            ItemRefusal::Unsupported("native Artifact ordered JSON conversion".into())
        }
    })?;
    serde_json::from_slice(&raw)
        .map_err(|_| ItemRefusal::Unsupported("native Artifact ordered JSON conversion".into()))
}

fn serde_value_as_ordered(value: &Value, max_bytes: usize) -> Result<JsonValue, ItemRefusal> {
    let raw = serde_json::to_vec(value)
        .map_err(|_| ItemRefusal::Unsupported("native Artifact ordered JSON conversion".into()))?;
    let limits = JsonLimits::new(max_bytes.min(8_388_608), 64, 300_000, 4_300)
        .map_err(|_| ItemRefusal::Budget)?;
    parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map(|document| document.into_root())
        .map_err(|error| {
            if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                ItemRefusal::Budget
            } else {
                ItemRefusal::Unsupported("native Artifact ordered JSON conversion".into())
            }
        })
}

fn replay_native_artifact_initial_forms<S: LayerFamilySource + ?Sized, I: Copy + Eq>(
    inspector: &mut Inspector<'_, '_, S, I>,
    request_path: &str,
    forms_path: &str,
    request_ordered: &JsonValue,
    forms_ordered: &JsonValue,
    principal_id: &str,
) -> Result<Option<NativeArtifactFormReplay>, ItemRefusal> {
    let Some(original) = request_ordered.object_get("record") else {
        inspector.issue(
            request_path,
            "native-artifact-original-record-missing",
            "retained creation request has no native record",
        )?;
        return Ok(None);
    };
    use crate::source_forms::source_copy_kernel as kernel;
    let subject = match kernel::metadata_subject(original) {
        Ok(subject) => subject,
        Err(_) => {
            inspector.issue(
                request_path,
                "native-artifact-form-source-invalid",
                "retained Artifact record cannot produce its maintained source-copy subject",
            )?;
            return Ok(None);
        }
    };
    let Some(selections) = request_ordered
        .object_get("forms")
        .and_then(JsonValue::as_array)
    else {
        inspector.issue(
            request_path,
            "native-artifact-form-selection-invalid",
            "retained source-copy form selection list is missing",
        )?;
        return Ok(None);
    };
    let mut changes = Vec::with_capacity(selections.len());
    inspector.reserve_state(
        selections
            .len()
            .checked_mul(std::mem::size_of::<JsonValue>() * 2 + 96)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    for selection in selections {
        let (Some(form_id), Some(field_id)) = (
            selection.object_get("form_id").and_then(JsonValue::as_str),
            selection.object_get("field_id").and_then(JsonValue::as_str),
        ) else {
            inspector.issue(
                request_path,
                "native-artifact-form-selection-invalid",
                "retained source-copy form selection lacks form_id or field_id",
            )?;
            return Ok(None);
        };
        let change =
            match kernel::prepare_form_change(original, None, principal_id, form_id, field_id) {
                Ok(change) => change,
                Err(_) => {
                    inspector.issue(
                        request_path,
                        "native-artifact-form-selection-invalid",
                        "retained source-copy form selection does not resolve to an owned field",
                    )?;
                    return Ok(None);
                }
            };
        changes.push(change);
    }
    let expected_set = match kernel::apply_form_changes(None, &subject, &changes) {
        Ok(set) => set,
        Err(_) => {
            inspector.issue(
                request_path,
                "native-artifact-form-replay-invalid",
                "retained request does not reconstruct its initial source-copy form set",
            )?;
            return Ok(None);
        }
    };
    if let Err(_) = kernel::validate_history(forms_ordered, &subject) {
        inspector.issue(
            forms_path,
            "native-artifact-form-history-invalid",
            "retained HumanForm set does not satisfy the maintained source-copy history law",
        )?;
        return Ok(None);
    }

    let mut selected_initial_forms = Vec::with_capacity(selections.len());
    for selection in selections {
        let form_id = selection
            .object_get("form_id")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        let initial = ["forms", "prior_forms"].into_iter().find_map(|collection| {
            forms_ordered
                .object_get(collection)
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
                .find(|form| {
                    form.object_get("form_id").and_then(JsonValue::as_str) == Some(form_id)
                        && form.object_get("form_version").and_then(JsonValue::as_u64) == Some(1)
                })
        });
        let Some(initial) = initial else {
            inspector.issue(
                forms_path,
                "native-artifact-initial-form-missing",
                "a requested initial source-copy form is not retained at version one",
            )?;
            return Ok(None);
        };
        let (Some(actual_subject), expected_subject) = (
            initial.object_get("subject"),
            ordered_value_as_serde(&subject, inspector.limits.max_member_bytes)?,
        ) else {
            inspector.issue(
                forms_path,
                "native-artifact-initial-form-subject-drift",
                "a requested initial source-copy form lacks its subject binding",
            )?;
            return Ok(None);
        };
        let actual_subject =
            ordered_value_as_serde(actual_subject, inspector.limits.max_member_bytes)?;
        if !python_json_equal(&actual_subject, &expected_subject) {
            inspector.issue(
                forms_path,
                "native-artifact-initial-form-subject-drift",
                "a requested initial source-copy form is not bound to the original Artifact subject",
            )?;
            return Ok(None);
        }
        selected_initial_forms.push(initial.clone());
    }
    let expected_forms = expected_set
        .object_get("forms")
        .cloned()
        .unwrap_or_else(|| JsonValue::Array(Vec::new()));
    let selected_json = ordered_value_as_serde(
        &JsonValue::Array(selected_initial_forms),
        inspector.limits.max_member_bytes,
    )?;
    let expected_json = ordered_value_as_serde(&expected_forms, inspector.limits.max_member_bytes)?;
    if !python_json_equal(&selected_json, &expected_json) {
        inspector.issue(
            forms_path,
            "native-artifact-initial-form-replay-drift",
            "retained initial forms differ from the exact source-copy request replay",
        )?;
    }

    let forms_have_history = forms_ordered
        .object_get("prior_forms")
        .and_then(JsonValue::as_array)
        .is_some_and(|rows| !rows.is_empty())
        || forms_ordered
            .object_get("growth_history")
            .is_some_and(|history| match history {
                JsonValue::Array(rows) => !rows.is_empty(),
                JsonValue::Null | JsonValue::Bool(false) => false,
                _ => true,
            });
    let limits = JsonLimits::new(
        inspector.limits.max_member_bytes.min(8_388_608),
        64,
        300_000,
        4_300,
    )
    .map_err(|_| ItemRefusal::Budget)?;
    let encoded = emit_json_profile(
        &expected_set,
        JsonEmissionProfile::SourceFormSetPublishedV1,
        limits,
    )
    .map_err(|error| {
        if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
            ItemRefusal::Budget
        } else {
            ItemRefusal::Unsupported("native Artifact initial form-set emission".into())
        }
    })?;
    inspector.reserve_state(
        encoded
            .bytes
            .len()
            .checked_add(std::mem::size_of::<NativeArtifactFormReplay>())
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let initial_source = ordered_value_as_serde(&subject, inspector.limits.max_member_bytes)?;
    let retained_form_sha256 = Digest256::of_bytes(&encoded.bytes).to_hex();
    let retained_form_size = encoded.bytes.len();
    Ok(Some(NativeArtifactFormReplay {
        initial_source,
        retained_form_sha256,
        retained_form_size,
        forms_have_history,
    }))
}

#[derive(Debug, Clone, Copy)]
struct NativeCutBinding {
    revision: SourceRevision,
    membership: SourceMembershipV1,
}

enum NativeHistoryRef<'a, I: Copy + Eq> {
    Cut(&'a NativeRecordHistoryReadObservation),
    Candidate(&'a crate::native_compound::CandidateNativeRecordHistoryReadObservation<I>),
}

impl<I: Copy + Eq> Copy for NativeHistoryRef<'_, I> {}

impl<I: Copy + Eq> Clone for NativeHistoryRef<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Clone, Copy)]
enum NativeArtifactBinding<'a, I: Copy + Eq> {
    Cut(NativeCutBinding),
    Candidate {
        identity: &'a I,
        membership: SourceMembershipV1,
    },
}

impl<I: Copy + Eq> NativeHistoryRef<'_, I> {
    fn belongs_to_cut(&self, binding: NativeCutBinding) -> bool {
        matches!(self, Self::Cut(observation)
            if observation.source_revision() == binding.revision
                && observation.current_membership() == binding.membership)
    }

    fn belongs_to_candidate(&self, identity: &I, membership: SourceMembershipV1) -> bool {
        matches!(self, Self::Candidate(observation)
            if observation.input_identity() == identity
                && observation.current_membership() == membership)
    }

    fn matches_binding(&self, binding: NativeArtifactBinding<'_, I>) -> bool {
        match binding {
            NativeArtifactBinding::Cut(cut) => self.belongs_to_cut(cut),
            NativeArtifactBinding::Candidate {
                identity,
                membership,
            } => self.belongs_to_candidate(identity, membership),
        }
    }

    fn record_path(&self) -> &str {
        match self {
            Self::Cut(observation) => observation.record_path(),
            Self::Candidate(observation) => observation.record_path(),
        }
    }

    fn identity_field(&self) -> &str {
        match self {
            Self::Cut(observation) => observation.identity_field(),
            Self::Candidate(observation) => observation.identity_field(),
        }
    }

    fn identity(&self) -> &str {
        match self {
            Self::Cut(observation) => observation.identity(),
            Self::Candidate(observation) => observation.identity(),
        }
    }

    fn selected_package(&self) -> &BTreeMap<String, Vec<u8>> {
        match self {
            Self::Cut(observation) => observation.selected_package(),
            Self::Candidate(observation) => observation.selected_package(),
        }
    }

    fn current_record(&self) -> &Value {
        match self {
            Self::Cut(observation) => observation.current_record(),
            Self::Candidate(observation) => observation.current_record(),
        }
    }

    fn origin_record_sha256(&self) -> &str {
        match self {
            Self::Cut(observation) => observation.origin_record_sha256(),
            Self::Candidate(observation) => observation.origin_record_sha256(),
        }
    }

    fn origin_record_byte_size(&self) -> usize {
        match self {
            Self::Cut(observation) => observation.origin_record_byte_size(),
            Self::Candidate(observation) => observation.origin_record_byte_size(),
        }
    }

    fn history_sha256(&self) -> Option<&str> {
        match self {
            Self::Cut(observation) => observation.history_sha256(),
            Self::Candidate(observation) => observation.history_sha256(),
        }
    }

    fn history(&self) -> &Value {
        match self {
            Self::Cut(observation) => observation.history(),
            Self::Candidate(observation) => observation.history(),
        }
    }

    fn history_receipt_count(&self) -> usize {
        match self {
            Self::Cut(observation) => observation.history_receipt_count(),
            Self::Candidate(observation) => observation.history_receipt_count(),
        }
    }

    fn transaction_count(&self) -> usize {
        match self {
            Self::Cut(observation) => observation.transactions().len(),
            Self::Candidate(observation) => observation.transactions().len(),
        }
    }

    fn transaction_is_committed(&self, id: &str) -> bool {
        match self {
            Self::Cut(observation) => observation.transactions().iter().any(|transaction| {
                transaction.transaction_id() == id
                    && transaction.transport() == NativeTransportState::Committed
            }),
            Self::Candidate(observation) => observation.transactions().iter().any(|transaction| {
                transaction.transaction_id() == id
                    && transaction.transport() == NativeTransportState::Committed
            }),
        }
    }

    fn has_transaction(&self, id: &str) -> bool {
        match self {
            Self::Cut(observation) => observation
                .transactions()
                .iter()
                .any(|transaction| transaction.transaction_id() == id),
            Self::Candidate(observation) => observation
                .transactions()
                .iter()
                .any(|transaction| transaction.transaction_id() == id),
        }
    }

    fn transaction_manifest_sha256(&self, id: &str) -> Option<&str> {
        match self {
            Self::Cut(observation) => observation
                .transactions()
                .iter()
                .find(|transaction| transaction.transaction_id() == id)
                .map(|transaction| transaction.manifest_sha256()),
            Self::Candidate(observation) => observation
                .transactions()
                .iter()
                .find(|transaction| transaction.transaction_id() == id)
                .map(|transaction| transaction.manifest_sha256()),
        }
    }

    fn bytes_read(&self) -> u64 {
        match self {
            Self::Cut(observation) => observation.bytes_read(),
            Self::Candidate(observation) => observation.bytes_read(),
        }
    }

    fn returned_state_bytes(&self) -> usize {
        match self {
            Self::Cut(observation) => observation.returned_state_bytes(),
            Self::Candidate(observation) => observation.returned_state_bytes(),
        }
    }
}

enum ArtifactReplayRef<'a, I: Copy + Eq> {
    Cut(&'a dyn ArtifactCorrectionReplayEvidence),
    Candidate(&'a dyn CandidateArtifactCorrectionReplayEvidence<I>),
}

impl<I: Copy + Eq> Copy for ArtifactReplayRef<'_, I> {}

impl<I: Copy + Eq> Clone for ArtifactReplayRef<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: Copy + Eq> ArtifactReplayRef<'_, I> {
    fn belongs_to_cut(&self, binding: NativeCutBinding) -> bool {
        matches!(self, Self::Cut(replay)
            if replay.source_revision() == binding.revision
                && replay.current_membership() == binding.membership)
    }

    fn belongs_to_candidate(&self, identity: &I, membership: SourceMembershipV1) -> bool {
        matches!(self, Self::Candidate(replay)
            if replay.input_identity() == identity
                && replay.current_membership() == membership)
    }

    fn matches_binding(&self, binding: NativeArtifactBinding<'_, I>) -> bool {
        match binding {
            NativeArtifactBinding::Cut(cut) => self.belongs_to_cut(cut),
            NativeArtifactBinding::Candidate {
                identity,
                membership,
            } => self.belongs_to_candidate(identity, membership),
        }
    }

    fn source_path(&self) -> &str {
        match self {
            Self::Cut(replay) => replay.source_path(),
            Self::Candidate(replay) => replay.source_path(),
        }
    }

    fn record_id(&self) -> &str {
        match self {
            Self::Cut(replay) => replay.record_id(),
            Self::Candidate(replay) => replay.record_id(),
        }
    }

    fn origin_record_sha256(&self) -> &str {
        match self {
            Self::Cut(replay) => replay.origin_record_sha256(),
            Self::Candidate(replay) => replay.origin_record_sha256(),
        }
    }

    fn origin_record_byte_size(&self) -> usize {
        match self {
            Self::Cut(replay) => replay.origin_record_byte_size(),
            Self::Candidate(replay) => replay.origin_record_byte_size(),
        }
    }

    fn history_sha256(&self) -> Option<&str> {
        match self {
            Self::Cut(replay) => replay.history_sha256(),
            Self::Candidate(replay) => replay.history_sha256(),
        }
    }

    fn transaction_count(&self) -> usize {
        match self {
            Self::Cut(replay) => replay.transaction_count(),
            Self::Candidate(replay) => replay.transaction_count(),
        }
    }

    fn transaction_at(&self, index: usize) -> Option<ArtifactCorrectionReplayTransactionRef<'_>> {
        match self {
            Self::Cut(replay) => replay.transaction_at(index),
            Self::Candidate(replay) => replay.transaction_at(index),
        }
    }

    fn publication_state_bytes(&self) -> usize {
        match self {
            Self::Cut(replay) => replay.publication_state_bytes(),
            Self::Candidate(replay) => replay.publication_state_bytes(),
        }
    }

    fn returned_state_bytes(&self) -> usize {
        match self {
            Self::Cut(replay) => replay.returned_state_bytes(),
            Self::Candidate(replay) => replay.returned_state_bytes(),
        }
    }
}

enum NativeHistorySet<'a, I: Copy + Eq> {
    Empty,
    Cut(&'a BTreeMap<String, NativeRecordHistoryReadObservation>),
    Candidate(
        &'a BTreeMap<
            String,
            crate::native_compound::CandidateNativeRecordHistoryReadObservation<I>,
        >,
    ),
}

impl<I: Copy + Eq> Copy for NativeHistorySet<'_, I> {}

impl<I: Copy + Eq> Clone for NativeHistorySet<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: Copy + Eq> NativeHistorySet<'_, I> {
    fn get(&self, path: &str) -> Option<NativeHistoryRef<'_, I>> {
        match self {
            Self::Empty => None,
            Self::Cut(rows) => rows.get(path).map(NativeHistoryRef::Cut),
            Self::Candidate(rows) => rows.get(path).map(NativeHistoryRef::Candidate),
        }
    }
}

enum ArtifactReplaySet<'a, I: Copy + Eq> {
    Empty,
    Cut(&'a ArtifactCorrectionReplayMap<'a>),
    Candidate(&'a CandidateArtifactCorrectionReplayMap<'a, I>),
}

impl<I: Copy + Eq> Copy for ArtifactReplaySet<'_, I> {}

impl<I: Copy + Eq> Clone for ArtifactReplaySet<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: Copy + Eq> ArtifactReplaySet<'_, I> {
    fn get(&self, path: &str) -> Option<ArtifactReplayRef<'_, I>> {
        match self {
            Self::Empty => None,
            Self::Cut(rows) => rows
                .get(path)
                .map(|evidence| ArtifactReplayRef::Cut(*evidence)),
            Self::Candidate(rows) => rows
                .get(path)
                .map(|evidence| ArtifactReplayRef::Candidate(*evidence)),
        }
    }

    fn for_each_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        match self {
            Self::Empty => Ok(()),
            Self::Cut(rows) => {
                for path in rows.keys() {
                    visit(path)?;
                }
                Ok(())
            }
            Self::Candidate(rows) => {
                for path in rows.keys() {
                    visit(path)?;
                }
                Ok(())
            }
        }
    }
}

trait CandidateInvalidArtifactSchemaProof<I: Copy + Eq> {
    fn binding_matches(&self, identity: &I, membership: SourceMembershipV1) -> bool;
    fn proves_invalid(
        &self,
        path: &str,
        member_sha256: Digest256,
        member_size_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal>;
}

#[derive(Debug, Clone, Copy)]
struct CurrentArtifactSchemaDiagnosticProof {
    record_id_sha256: Digest256,
    member_sha256: Digest256,
    member_size_bytes: u64,
    record_count: usize,
    target_diagnostic_count: usize,
    diagnostic_unit_sha256: Option<Digest256>,
    diagnostic_report_sha256: Option<Digest256>,
    invalid: bool,
}

/// Conservative logical cost for producing and validating the narrow current
/// Artifact schema-rejection proof. Scan work units include report rows and a
/// cap-weighted upper bound for issue/path traversals performed by
/// `Report::is_well_formed`; they are not wall-time or source-I/O claims.
/// Hash-input bytes count proof transcript bytes for both passes, not
/// schema-worker input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CurrentArtifactInvalidSchemaProofCost {
    pub retained_state_bytes: usize,
    pub preparation_scan_work_upper_bound: usize,
    pub validation_scan_work_upper_bound: usize,
    pub hash_input_bytes_upper_bound: usize,
}

/// Bounded logical accounting for a candidate-bound Artifact schema proof.
/// Index page reads and typed diagnostic work are source evidence; the cost is
/// not a source verdict or an admission receipt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CandidateArtifactInvalidSchemaProofCost {
    pub retained_state_bytes: usize,
    pub current_record_rows_scanned: usize,
    pub schema_diagnostic_rows_scanned: usize,
    pub diagnostic_validation_work_upper_bound: usize,
    pub hash_input_bytes_upper_bound: usize,
    pub index_page_peak_state_bytes: usize,
    pub candidate_record_count: usize,
    pub target_diagnostic_count: usize,
    /// Exact input path probes performed while preparing this proof. A later
    /// binding check performs one more probe per unique retained Artifact path.
    pub current_path_probe_count: usize,
}

/// Opaque proof over one exact candidate Records report. It borrows that
/// report so the evidence cannot be rebound to another report with the same
/// membership summary. Candidate identity remains its own typed value and is
/// never represented as `SourceRevision`.
pub struct CandidateArtifactInvalidSchemaProofs<'report, 'store, I: Copy + Eq> {
    records: &'report SourceFoundationRecordsStreamedReport<'store, I>,
    input_identity: I,
    current_membership: SourceMembershipV1,
    schema_identity: crate::source_foundation_records::SourceFoundationCandidateSchemaIdentity,
    candidate_record_count: usize,
    candidate_records_sha256: Digest256,
    target_diagnostic_count: usize,
    target_diagnostics_sha256: Digest256,
    max_state_bytes: usize,
    page_budget: SourceFoundationRecordsPageBudget,
    cost: CandidateArtifactInvalidSchemaProofCost,
}

impl<I: Copy + Eq> CandidateArtifactInvalidSchemaProofs<'_, '_, I> {
    pub fn input_identity(&self) -> I {
        self.input_identity
    }

    pub fn current_membership(&self) -> SourceMembershipV1 {
        self.current_membership
    }

    pub fn cost(&self) -> CandidateArtifactInvalidSchemaProofCost {
        self.cost
    }

    pub fn candidate_record_count(&self) -> usize {
        self.candidate_record_count
    }

    pub fn candidate_records_sha256(&self) -> Digest256 {
        self.candidate_records_sha256
    }

    pub fn target_diagnostic_count(&self) -> usize {
        self.target_diagnostic_count
    }

    pub fn target_diagnostics_sha256(&self) -> Digest256 {
        self.target_diagnostics_sha256
    }

    /// Return true only for one current v2 Artifact record and one complete
    /// invalid diagnostics-v2 result from this report's prepared schema set.
    pub fn proves_invalid(
        &self,
        path: &str,
        input_identity: &I,
        current_membership: SourceMembershipV1,
        member_sha256: Digest256,
        member_size_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        if &self.input_identity != input_identity || self.current_membership != current_membership {
            return Ok(false);
        }
        self.records
            .index()
            .candidate_artifact_schema_proves_invalid(
                path,
                &member_sha256.to_hex(),
                member_size_bytes,
                std::num::NonZeroUsize::new(self.max_state_bytes).ok_or(ItemRefusal::Budget)?,
                deadline,
                cancelled,
            )
    }

    /// Check the exact borrowed report and candidate identity before a caller
    /// uses the proof. Point probes exercise the candidate's live fence for
    /// every retained Artifact path without rebuilding the full member map.
    pub fn validate_report_binding(
        &self,
        input: &dyn SourceCutInputWithIdentity<I>,
        records: &SourceFoundationRecordsStreamedReport<'_, I>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if !std::ptr::eq(self.records, records)
            || self.input_identity != *input.input_identity()
            || self.input_identity != *records.input_identity()
            || self.current_membership != *records.source_membership()
            || records.candidate_schema_identity() != Some(&self.schema_identity)
        {
            return Err(ItemRefusal::Source(
                "source-foundation candidate Artifact proof binding differs".into(),
            ));
        }
        let mut after_path: Option<String> = None;
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Err(ItemRefusal::Source(
                    "source-foundation candidate Artifact proof validation cancelled".into(),
                ));
            }
            if Instant::now() >= deadline {
                return Err(ItemRefusal::Deadline);
            }
            let page = records.index().candidate_artifact_schema_proof_paths_page(
                after_path.as_deref(),
                self.page_budget,
                deadline,
                cancelled,
            )?;
            let has_more = page.has_more;
            let mut paths = page.paths;
            if paths.is_empty() {
                if has_more {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate Artifact proof path page stalled".into(),
                    ));
                }
                break;
            }
            for path in &paths {
                let lookup_state = path
                    .len()
                    .checked_mul(32)
                    .and_then(|bytes| bytes.checked_add(8_192))
                    .ok_or(ItemRefusal::Budget)?;
                if self
                    .cost
                    .retained_state_bytes
                    .checked_add(page.charged_state_bytes)
                    .and_then(|used| used.checked_add(lookup_state))
                    .is_none_or(|used| used > self.max_state_bytes)
                {
                    return Err(ItemRefusal::Budget);
                }
                if input.path_presence(path, deadline, cancelled)? != Some(SourcePresenceV1::File) {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate Artifact proof path is not a current file"
                            .into(),
                    ));
                }
            }
            if has_more {
                after_path = paths.pop();
                if after_path.is_none() {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate Artifact proof path cursor is missing".into(),
                    ));
                }
            } else {
                break;
            }
        }
        Ok(())
    }
}

impl<'report, 'store, I: Copy + Eq> CandidateArtifactInvalidSchemaProofs<'report, 'store, I> {
    /// Prepare the shared proof from complete, bounded pages in the same
    /// candidate Records report consumed by Discovery. The record scan cap and
    /// diagnostic-work cap are explicit caller limits; neither is inferred
    /// from available memory or replaced by a resident report.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_candidate(
        input: &dyn SourceCutInputWithIdentity<I>,
        records: &'report SourceFoundationRecordsStreamedReport<'store, I>,
        page_budget: SourceFoundationRecordsPageBudget,
        max_state_bytes: usize,
        max_scan_rows: usize,
        max_diagnostic_validation_work: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let check_active = || -> Result<(), ItemRefusal> {
            if cancelled.load(Ordering::Relaxed) {
                Err(ItemRefusal::Source(
                    "source-foundation candidate Artifact proof preparation cancelled".into(),
                ))
            } else if Instant::now() >= deadline {
                Err(ItemRefusal::Deadline)
            } else {
                Ok(())
            }
        };
        check_active()?;
        let input_identity = *input.input_identity();
        let current_membership = *records.source_membership();
        let schema_identity = *records.candidate_schema_identity().ok_or_else(|| {
            ItemRefusal::Source(
                "source-foundation candidate Artifact proof requires the prepared schema binding"
                    .into(),
            )
        })?;
        if input_identity != *records.input_identity()
            || max_state_bytes == 0
            || max_scan_rows == 0
            || max_diagnostic_validation_work == 0
            || page_budget.max_state_bytes.get() > max_state_bytes
        {
            return Err(ItemRefusal::Source(
                "source-foundation candidate Artifact proof input profile differs".into(),
            ));
        }

        let mut rows_scanned = 0usize;
        let mut page_peak_state_bytes = 0usize;
        let mut candidate_record_count = 0usize;
        let mut current_record_rows_scanned = 0usize;
        let current_rows_before = rows_scanned;
        visit_candidate_record_pages(
            records,
            SourceFoundationRecordsCollection::CurrentRecords,
            page_budget,
            max_scan_rows,
            &mut rows_scanned,
            &mut page_peak_state_bytes,
            deadline,
            cancelled,
            &mut |row, page_state_bytes| {
                let SourceFoundationRecordsStoredFact::CurrentRecord { record_id, record } = row
                else {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate CurrentRecords page kind differs".into(),
                    ));
                };
                if !is_current_v2_artifact_record(record) {
                    return Ok(());
                }
                let lookup_state = record
                    .path
                    .len()
                    .checked_mul(32)
                    .and_then(|bytes| bytes.checked_add(8_192))
                    .ok_or(ItemRefusal::Budget)?;
                if page_state_bytes
                    .checked_add(lookup_state)
                    .is_none_or(|used| used > max_state_bytes)
                {
                    return Err(ItemRefusal::Budget);
                }
                if input.path_presence(&record.path, deadline, cancelled)?
                    != Some(SourcePresenceV1::File)
                {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate Artifact record is not a current file".into(),
                    ));
                }
                let _ = record_id;
                candidate_record_count = candidate_record_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
                Ok(())
            },
        )?;
        current_record_rows_scanned = current_record_rows_scanned
            .checked_add(
                rows_scanned
                    .checked_sub(current_rows_before)
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;

        let retained_state_bytes = std::mem::size_of::<Self>()
            .checked_add(512)
            .ok_or(ItemRefusal::Budget)?;
        if retained_state_bytes
            .checked_add(page_budget.max_state_bytes.get())
            .is_none_or(|used| used > max_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }

        records
            .index()
            .reset_candidate_artifact_schema_proofs(deadline, cancelled)?;

        let mut candidate_hasher = Digest256Hasher::new();
        hash_text(
            &mut candidate_hasher,
            "tos-candidate-artifact-schema-proof-records-v1",
        )?;
        let mut hash_input_bytes_upper_bound = 0usize;
        let current_rows_before = rows_scanned;
        visit_candidate_record_pages(
            records,
            SourceFoundationRecordsCollection::CurrentRecords,
            page_budget,
            max_scan_rows,
            &mut rows_scanned,
            &mut page_peak_state_bytes,
            deadline,
            cancelled,
            &mut |row, page_state_bytes| {
                let SourceFoundationRecordsStoredFact::CurrentRecord { record_id, record } = row
                else {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate CurrentRecords page kind differs".into(),
                    ));
                };
                if !is_current_v2_artifact_record(record) {
                    return Ok(());
                }
                update_candidate_artifact_record_fingerprint(
                    &mut candidate_hasher,
                    record_id,
                    record,
                )?;
                hash_input_bytes_upper_bound = hash_input_bytes_upper_bound
                    .checked_add(candidate_artifact_record_fingerprint_bytes(
                        record_id, record,
                    )?)
                    .ok_or(ItemRefusal::Budget)?;
                if retained_state_bytes
                    .checked_add(page_state_bytes)
                    .is_none_or(|used| used > max_state_bytes)
                {
                    return Err(ItemRefusal::Budget);
                }
                let _ = record_id;
                records.index().retain_candidate_artifact_schema_record(
                    &record.path,
                    std::num::NonZeroUsize::new(max_state_bytes).ok_or(ItemRefusal::Budget)?,
                    deadline,
                    cancelled,
                )?;
                Ok(())
            },
        )?;
        current_record_rows_scanned = current_record_rows_scanned
            .checked_add(
                rows_scanned
                    .checked_sub(current_rows_before)
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;

        let mut target_diagnostic_count = 0usize;
        let mut diagnostic_validation_work_upper_bound = 0usize;
        let mut diagnostic_hasher = Digest256Hasher::new();
        hash_text(
            &mut diagnostic_hasher,
            "tos-candidate-artifact-schema-proof-diagnostics-v1",
        )?;
        let mut schema_diagnostic_rows_scanned = 0usize;
        visit_candidate_record_pages(
            records,
            SourceFoundationRecordsCollection::RecordSchemaDiagnostics,
            page_budget,
            max_scan_rows,
            &mut rows_scanned,
            &mut page_peak_state_bytes,
            deadline,
            cancelled,
            &mut |row, page_state_bytes| {
                let SourceFoundationRecordsStoredFact::RecordSchemaDiagnostic(row) = row else {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate schema-diagnostic page kind differs".into(),
                    ));
                };
                schema_diagnostic_rows_scanned = schema_diagnostic_rows_scanned
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
                let diagnostic = &row.diagnostic;
                if !diagnostic_targets_artifact_schema(diagnostic) {
                    return Ok(());
                }
                target_diagnostic_count = target_diagnostic_count
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?;
                diagnostic_validation_work_upper_bound = diagnostic_validation_work_upper_bound
                    .checked_add(diagnostic_issue_validation_work_upper_bound(diagnostic)?)
                    .filter(|work| *work <= max_diagnostic_validation_work)
                    .ok_or(ItemRefusal::Budget)?;
                hash_input_bytes_upper_bound = hash_input_bytes_upper_bound
                    .checked_add(diagnostic_fingerprint_input_upper_bound(diagnostic)?)
                    .ok_or(ItemRefusal::Budget)?;
                if page_state_bytes
                    .checked_add(retained_state_bytes)
                    .is_none_or(|used| used > max_state_bytes)
                {
                    return Err(ItemRefusal::Budget);
                }
                update_diagnostic_fingerprint(&mut diagnostic_hasher, diagnostic)?;
                let member_sha256 = diagnostic.unit.raw_sha256;
                let member_size_bytes = u64::try_from(diagnostic.input_instance_bytes)
                    .map_err(|_| ItemRefusal::Budget)?;
                let exact_schema_set = diagnostic.verdict.format_profile
                    == schema_identity.profile()
                    && diagnostic.verdict.schema_set_digest == schema_identity.schema_set_digest()
                    && diagnostic.verdict.worker_binary_digest == schema_identity.worker_digest();
                let complete_invalid = exact_schema_set
                    && complete_invalid_artifact_schema_diagnostic(
                        diagnostic,
                        diagnostic.path.as_str(),
                        member_sha256,
                        member_size_bytes,
                    );
                let diagnostic_state = page_state_bytes
                    .checked_add(retained_state_bytes)
                    .and_then(|bytes| bytes.checked_add(diagnostic.path.len().saturating_mul(2)))
                    .and_then(|bytes| bytes.checked_add(8_192))
                    .ok_or(ItemRefusal::Budget)?;
                if diagnostic_state > max_state_bytes {
                    return Err(ItemRefusal::Budget);
                }
                records
                    .index()
                    .update_candidate_artifact_schema_diagnostic(
                        &diagnostic.path,
                        &member_sha256.to_hex(),
                        member_size_bytes,
                        &diagnostic.unit.unit_sha256.to_hex(),
                        &diagnostic.unit.report.report_sha256.to_hex(),
                        exact_schema_set,
                        complete_invalid,
                        std::num::NonZeroUsize::new(max_state_bytes).ok_or(ItemRefusal::Budget)?,
                        deadline,
                        cancelled,
                    )?;
                Ok(())
            },
        )?;
        check_active()?;
        if input.input_identity() != records.input_identity()
            || *records.source_membership() != current_membership
        {
            return Err(ItemRefusal::Source(
                "source-foundation candidate Artifact proof input changed during preparation"
                    .into(),
            ));
        }
        hash_input_bytes_upper_bound = hash_input_bytes_upper_bound
            .checked_add(512)
            .ok_or(ItemRefusal::Budget)?;
        let cost = CandidateArtifactInvalidSchemaProofCost {
            retained_state_bytes,
            current_record_rows_scanned,
            schema_diagnostic_rows_scanned,
            diagnostic_validation_work_upper_bound,
            hash_input_bytes_upper_bound,
            index_page_peak_state_bytes: page_peak_state_bytes,
            candidate_record_count,
            target_diagnostic_count,
            current_path_probe_count: candidate_record_count,
        };
        Ok(Self {
            records,
            input_identity,
            current_membership,
            schema_identity,
            candidate_record_count,
            candidate_records_sha256: candidate_hasher.finalize(),
            target_diagnostic_count,
            target_diagnostics_sha256: diagnostic_hasher.finalize(),
            max_state_bytes,
            page_budget,
            cost,
        })
    }
}

fn visit_candidate_record_pages<I, F>(
    records: &SourceFoundationRecordsStreamedReport<'_, I>,
    collection: SourceFoundationRecordsCollection,
    budget: SourceFoundationRecordsPageBudget,
    max_scan_rows: usize,
    rows_scanned: &mut usize,
    page_peak_state_bytes: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    visit: &mut F,
) -> Result<(), ItemRefusal>
where
    F: FnMut(&SourceFoundationRecordsStoredFact, usize) -> Result<(), ItemRefusal>,
{
    let mut after: Option<SourceFoundationRecordsCursor> = None;
    loop {
        let page = records
            .index()
            .page(collection, after.as_ref(), budget, deadline, cancelled)?;
        *page_peak_state_bytes = (*page_peak_state_bytes).max(page.charged_state_bytes);
        *rows_scanned = (*rows_scanned)
            .checked_add(page.rows.len())
            .filter(|rows| *rows <= max_scan_rows)
            .ok_or(ItemRefusal::Budget)?;
        for (index, row) in page.rows.iter().enumerate() {
            if index % 128 == 0 {
                if cancelled.load(Ordering::Relaxed) {
                    return Err(ItemRefusal::Source(
                        "source-foundation candidate Artifact proof page cancelled".into(),
                    ));
                }
                if Instant::now() >= deadline {
                    return Err(ItemRefusal::Deadline);
                }
            }
            visit(row, page.charged_state_bytes)?;
        }
        let next = page.next_cursor.clone();
        drop(page);
        let Some(next) = next else {
            return Ok(());
        };
        after = Some(next);
    }
}

/// Opaque, exact-cut evidence that a current v2 Artifact schema unit already
/// produced one complete Invalid owner diagnostic. Map keys borrow only the
/// exact cut's paths; no Records report is retained or cloned.
pub struct CurrentArtifactInvalidSchemaProofs<'cut> {
    source_revision: SourceRevision,
    current_membership: SourceMembershipV1,
    entries: BTreeMap<&'cut str, CurrentArtifactSchemaDiagnosticProof>,
    candidate_record_count: usize,
    candidate_records_sha256: Digest256,
    target_diagnostic_count: usize,
    target_diagnostics_sha256: Digest256,
    cost: CurrentArtifactInvalidSchemaProofCost,
}

impl CurrentArtifactInvalidSchemaProofs<'_> {
    pub fn source_revision(&self) -> SourceRevision {
        self.source_revision
    }

    pub fn current_membership(&self) -> SourceMembershipV1 {
        self.current_membership
    }

    pub fn cost(&self) -> CurrentArtifactInvalidSchemaProofCost {
        self.cost
    }

    /// Return true only for a unique complete invalid v2 diagnostic bound to
    /// this exact current member and source revision.
    pub fn proves_invalid(
        &self,
        path: &str,
        revision: SourceRevision,
        membership: SourceMembershipV1,
        member_sha256: Digest256,
        member_size_bytes: u64,
    ) -> bool {
        if self.source_revision != revision || self.current_membership != membership {
            return false;
        }
        self.entries.get(path).is_some_and(|proof| {
            proof.record_count == 1
                && proof.target_diagnostic_count == 1
                && proof.invalid
                && proof.member_sha256 == member_sha256
                && proof.member_size_bytes == member_size_bytes
                && proof.diagnostic_unit_sha256.is_some()
                && proof.diagnostic_report_sha256.is_some()
        })
    }
}

impl<'cut> CurrentArtifactInvalidSchemaProofs<'cut> {
    /// Build one bounded proof set from the complete Records report and the
    /// exact captured cut. The returned entries borrow cut paths only.
    pub fn prepare(
        cut: &'cut CorpusCutReader,
        report: &SourceCutRecordReport,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let check_active = || -> Result<(), ItemRefusal> {
            if cancelled.load(Ordering::Relaxed) {
                Err(ItemRefusal::Source(
                    "source-foundation Artifact proof preparation cancelled".into(),
                ))
            } else if Instant::now() >= deadline {
                Err(ItemRefusal::Deadline)
            } else {
                Ok(())
            }
        };
        check_active()?;
        let source_revision = cut.current().revision();
        let current_membership = cut
            .stream(source_revision)
            .map_err(|_| {
                ItemRefusal::Source("source-foundation exact cut membership unavailable".into())
            })?
            .expectation();
        if report.source_revision != source_revision
            || report.current_membership != current_membership
        {
            return Err(ItemRefusal::Source(
                "source-foundation Records report differs from the exact source cut".into(),
            ));
        }

        let mut candidate_record_count = 0usize;
        let mut max_candidate_path_bytes = 0usize;
        for (index, record) in report.records.values().enumerate() {
            if index % 128 == 0 {
                check_active()?;
            }
            if !is_current_v2_artifact_record(record) {
                continue;
            }
            let relative = RelativePath::parse(&record.path).map_err(|_| {
                ItemRefusal::Source(
                    "source-foundation Records Artifact path is not a normalized current member"
                        .into(),
                )
            })?;
            if cut.current().member(&relative).is_none() {
                return Err(ItemRefusal::Source(
                    "source-foundation Records Artifact path is outside the exact current cut"
                        .into(),
                ));
            }
            candidate_record_count = candidate_record_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            max_candidate_path_bytes = max_candidate_path_bytes.max(record.path.len());
        }

        // Reserve the maximum one-node-per-candidate tree before allocating.
        // Keys point into the cut and fingerprints are fixed-size; the path
        // allowance covers one transient RelativePath parse.
        let entry_state = std::mem::size_of::<CurrentArtifactSchemaDiagnosticProof>()
            .checked_add(std::mem::size_of::<&str>())
            .and_then(|bytes| bytes.checked_add(128))
            .ok_or(ItemRefusal::Budget)?;
        let retained_state_bytes = candidate_record_count
            .checked_mul(entry_state)
            .and_then(|bytes| {
                bytes.checked_add(std::mem::size_of::<CurrentArtifactInvalidSchemaProofs<'cut>>())
            })
            .and_then(|bytes| {
                max_candidate_path_bytes
                    .checked_mul(2)
                    .and_then(|scratch| scratch.checked_add(64))
                    .and_then(|scratch| bytes.checked_add(scratch))
            })
            .ok_or(ItemRefusal::Budget)?;
        if retained_state_bytes > max_state_bytes {
            return Err(ItemRefusal::Budget);
        }

        let mut entries = BTreeMap::new();
        let mut candidate_hasher = Digest256Hasher::new();
        hash_text(
            &mut candidate_hasher,
            "tos-current-artifact-schema-proof-records-v1",
        )?;
        let mut preparation_scan_work_upper_bound = 0usize;
        let mut hash_input_bytes_upper_bound = 0usize;
        for (index, (record_id, record)) in report.records.iter().enumerate() {
            if index % 128 == 0 {
                check_active()?;
            }
            preparation_scan_work_upper_bound = preparation_scan_work_upper_bound
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            if !is_current_v2_artifact_record(record) {
                continue;
            }
            check_active()?;
            let relative = RelativePath::parse(&record.path).map_err(|_| {
                ItemRefusal::Source(
                    "source-foundation Records Artifact path is not a normalized current member"
                        .into(),
                )
            })?;
            let member = cut.current().member(&relative).ok_or_else(|| {
                ItemRefusal::Source(
                    "source-foundation Records Artifact path is outside the exact current cut"
                        .into(),
                )
            })?;
            let candidate_hash_bytes = record_id
                .len()
                .checked_add(record.path.len())
                .and_then(|bytes| {
                    bytes.checked_add(string(&record.value, "schema_version").unwrap_or("").len())
                })
                .and_then(|bytes| {
                    bytes.checked_add(string(&record.value, "$schema").unwrap_or("").len())
                })
                .and_then(|bytes| bytes.checked_add(256))
                .ok_or(ItemRefusal::Budget)?;
            hash_input_bytes_upper_bound = hash_input_bytes_upper_bound
                .checked_add(candidate_hash_bytes)
                .ok_or(ItemRefusal::Budget)?;
            update_candidate_fingerprint(&mut candidate_hasher, record_id, record, member)?;
            let entry = entries.entry(member.path.as_str()).or_insert(
                CurrentArtifactSchemaDiagnosticProof {
                    record_id_sha256: Digest256::of_bytes(record_id.as_bytes()),
                    member_sha256: member.sha256,
                    member_size_bytes: member.size_bytes,
                    record_count: 0,
                    target_diagnostic_count: 0,
                    diagnostic_unit_sha256: None,
                    diagnostic_report_sha256: None,
                    invalid: false,
                },
            );
            entry.record_count = entry
                .record_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            if entry.member_sha256 != member.sha256 || entry.member_size_bytes != member.size_bytes
            {
                return Err(ItemRefusal::Source(
                    "source-foundation Records Artifact path has inconsistent exact-cut metadata"
                        .into(),
                ));
            }
        }
        let candidate_records_sha256 = candidate_hasher.finalize();

        let mut target_diagnostic_count = 0usize;
        let mut target_schema_validation_work = 0usize;
        let mut diagnostic_hasher = Digest256Hasher::new();
        hash_text(
            &mut diagnostic_hasher,
            "tos-current-artifact-schema-proof-diagnostics-v1",
        )?;
        for (index, diagnostic) in report.schema_diagnostics.iter().enumerate() {
            if index % 128 == 0 {
                check_active()?;
            }
            preparation_scan_work_upper_bound = preparation_scan_work_upper_bound
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            if !diagnostic_targets_artifact_schema(diagnostic) {
                continue;
            }
            let Some(proof) = entries.get_mut(diagnostic.path.as_str()) else {
                continue;
            };
            check_active()?;
            let issue_work = diagnostic_issue_validation_work_upper_bound(diagnostic)?;
            target_schema_validation_work = target_schema_validation_work
                .checked_add(issue_work)
                .ok_or(ItemRefusal::Budget)?;
            preparation_scan_work_upper_bound = preparation_scan_work_upper_bound
                .checked_add(issue_work)
                .ok_or(ItemRefusal::Budget)?;
            let diagnostic_hash_bytes = diagnostic_fingerprint_input_upper_bound(diagnostic)?;
            hash_input_bytes_upper_bound = hash_input_bytes_upper_bound
                .checked_add(diagnostic_hash_bytes)
                .ok_or(ItemRefusal::Budget)?;
            check_active()?;
            proof.target_diagnostic_count = proof
                .target_diagnostic_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            let invalid = complete_invalid_artifact_schema_diagnostic(
                diagnostic,
                diagnostic.path.as_str(),
                proof.member_sha256,
                proof.member_size_bytes,
            );
            proof.invalid = proof.target_diagnostic_count == 1 && invalid;
            if proof.target_diagnostic_count == 1 {
                proof.diagnostic_unit_sha256 = Some(diagnostic.unit.unit_sha256);
                proof.diagnostic_report_sha256 = Some(diagnostic.unit.report.report_sha256);
            }
            check_active()?;
            update_diagnostic_fingerprint(&mut diagnostic_hasher, diagnostic)?;
            target_diagnostic_count = target_diagnostic_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        let validation_scan_work_upper_bound = report
            .records
            .len()
            .checked_add(report.schema_diagnostics.len())
            .and_then(|rows| rows.checked_add(target_schema_validation_work))
            .ok_or(ItemRefusal::Budget)?;
        // The preparation pass above scans all Records rows twice.
        preparation_scan_work_upper_bound = preparation_scan_work_upper_bound
            .checked_add(report.records.len())
            .ok_or(ItemRefusal::Budget)?;
        hash_input_bytes_upper_bound = hash_input_bytes_upper_bound
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(512))
            .ok_or(ItemRefusal::Budget)?;
        Ok(Self {
            source_revision,
            current_membership,
            entries,
            candidate_record_count,
            candidate_records_sha256,
            target_diagnostic_count,
            target_diagnostics_sha256: diagnostic_hasher.finalize(),
            cost: CurrentArtifactInvalidSchemaProofCost {
                retained_state_bytes,
                preparation_scan_work_upper_bound,
                validation_scan_work_upper_bound,
                hash_input_bytes_upper_bound,
            },
        })
    }

    fn validate_report_binding(
        &self,
        cut: &CorpusCutReader,
        report: &SourceCutRecordReport,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        let check_active = || -> Result<(), ItemRefusal> {
            if cancelled.load(Ordering::Relaxed) {
                Err(ItemRefusal::Source(
                    "source-foundation Artifact proof validation cancelled".into(),
                ))
            } else if Instant::now() >= deadline {
                Err(ItemRefusal::Deadline)
            } else {
                Ok(())
            }
        };
        check_active()?;
        let revision = cut.current().revision();
        let membership = cut
            .stream(revision)
            .map_err(|_| {
                ItemRefusal::Source("source-foundation exact cut membership unavailable".into())
            })?
            .expectation();
        if self.source_revision != revision
            || self.current_membership != membership
            || report.source_revision != revision
            || report.current_membership != membership
        {
            return Err(ItemRefusal::Source(
                "source-foundation Artifact proof differs from the exact source cut".into(),
            ));
        }

        let mut candidate_count = 0usize;
        let mut candidate_hasher = Digest256Hasher::new();
        hash_text(
            &mut candidate_hasher,
            "tos-current-artifact-schema-proof-records-v1",
        )?;
        for (index, (record_id, record)) in report.records.iter().enumerate() {
            if index % 128 == 0 {
                check_active()?;
            }
            if !is_current_v2_artifact_record(record) {
                continue;
            }
            check_active()?;
            let relative = RelativePath::parse(&record.path).map_err(|_| {
                ItemRefusal::Source(
                    "source-foundation Records Artifact path is not a normalized current member"
                        .into(),
                )
            })?;
            let member = cut.current().member(&relative).ok_or_else(|| {
                ItemRefusal::Source(
                    "source-foundation Records Artifact path is outside the exact current cut"
                        .into(),
                )
            })?;
            let proof = self.entries.get(member.path.as_str()).ok_or_else(|| {
                ItemRefusal::Source(
                    "source-foundation Artifact proof does not bind the Records report".into(),
                )
            })?;
            if (proof.record_count == 1
                && proof.record_id_sha256 != Digest256::of_bytes(record_id.as_bytes()))
                || proof.member_sha256 != member.sha256
                || proof.member_size_bytes != member.size_bytes
            {
                return Err(ItemRefusal::Source(
                    "source-foundation Artifact proof does not bind the Records report".into(),
                ));
            }
            update_candidate_fingerprint(&mut candidate_hasher, record_id, record, member)?;
            candidate_count = candidate_count.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
        if candidate_count != self.candidate_record_count
            || candidate_hasher.finalize() != self.candidate_records_sha256
        {
            return Err(ItemRefusal::Source(
                "source-foundation Artifact proof does not bind the Records report".into(),
            ));
        }

        let mut target_diagnostic_count = 0usize;
        let mut diagnostic_hasher = Digest256Hasher::new();
        hash_text(
            &mut diagnostic_hasher,
            "tos-current-artifact-schema-proof-diagnostics-v1",
        )?;
        for (index, diagnostic) in report.schema_diagnostics.iter().enumerate() {
            if index % 128 == 0 {
                check_active()?;
            }
            if !diagnostic_targets_artifact_schema(diagnostic) {
                continue;
            }
            let Some(proof) = self.entries.get(diagnostic.path.as_str()) else {
                continue;
            };
            check_active()?;
            if proof.target_diagnostic_count == 1 {
                if proof.diagnostic_unit_sha256 != Some(diagnostic.unit.unit_sha256)
                    || proof.diagnostic_report_sha256 != Some(diagnostic.unit.report.report_sha256)
                {
                    return Err(ItemRefusal::Source(
                        "source-foundation Artifact proof does not bind the Records report".into(),
                    ));
                }
                check_active()?;
                let invalid = complete_invalid_artifact_schema_diagnostic(
                    diagnostic,
                    diagnostic.path.as_str(),
                    proof.member_sha256,
                    proof.member_size_bytes,
                );
                if proof.invalid != invalid {
                    return Err(ItemRefusal::Source(
                        "source-foundation Artifact proof does not bind the Records report".into(),
                    ));
                }
            }
            check_active()?;
            update_diagnostic_fingerprint(&mut diagnostic_hasher, diagnostic)?;
            target_diagnostic_count = target_diagnostic_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        if target_diagnostic_count != self.target_diagnostic_count
            || diagnostic_hasher.finalize() != self.target_diagnostics_sha256
        {
            return Err(ItemRefusal::Source(
                "source-foundation Artifact proof does not bind the Records report".into(),
            ));
        }
        Ok(())
    }
}

impl<I: Copy + Eq> CandidateInvalidArtifactSchemaProof<I>
    for CandidateArtifactInvalidSchemaProofs<'_, '_, I>
{
    fn binding_matches(&self, identity: &I, membership: SourceMembershipV1) -> bool {
        &self.input_identity == identity && self.current_membership == membership
    }

    fn proves_invalid(
        &self,
        path: &str,
        member_sha256: Digest256,
        member_size_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        CandidateArtifactInvalidSchemaProofs::proves_invalid(
            self,
            path,
            &self.input_identity,
            self.current_membership,
            member_sha256,
            member_size_bytes,
            deadline,
            cancelled,
        )
    }
}

fn is_current_v2_artifact_record(record: &BiblioCurrentRecord) -> bool {
    record.path.starts_with(ARTIFACTS)
        && record.path.ends_with("/artifact-witness.json")
        && string(&record.value, "schema_version") == Some("tos_artifact_source_witness_v2")
        && string(&record.value, "$schema") == Some(V2_ARTIFACT_SCHEMA)
}

fn diagnostic_targets_artifact_schema(
    diagnostic: &crate::record_biblio_cut::SourceCutSchemaDiagnostic,
) -> bool {
    diagnostic.verdict.root_uri == V2_ARTIFACT_SCHEMA
        || diagnostic.unit.root_uri == V2_ARTIFACT_SCHEMA
}

fn diagnostic_issue_validation_work_upper_bound(
    diagnostic: &crate::record_biblio_cut::SourceCutSchemaDiagnostic,
) -> Result<usize, ItemRefusal> {
    let report = &diagnostic.unit.report;
    let issue_rows = report.issues.len();
    let path_bytes =
        usize::try_from(report.caps.max_path_bytes).map_err(|_| ItemRefusal::Budget)?;
    let path_segments = usize::from(report.caps.max_path_segments);
    // `is_well_formed` checks adjacent ordering, scans issue shape and both
    // paths, serializes every issue once for the size check, and serializes it
    // again for the issue digest. This deliberately overbounds both string
    // byte comparisons and bounded path-segment visits by the declared caps.
    let bounded_path_work = issue_rows
        .checked_mul(4)
        .and_then(|work| {
            issue_rows
                .checked_mul(path_bytes.checked_mul(10)?)
                .and_then(|bytes| work.checked_add(bytes))
        })
        .and_then(|work| {
            issue_rows
                .checked_mul(path_segments.checked_mul(8)?)
                .and_then(|segments| work.checked_add(segments))
        })
        .ok_or(ItemRefusal::Budget)?;
    // Adjacent-order comparisons can inspect both neighboring serialized
    // issues before the later per-issue path and report-size guards run.
    bounded_path_work
        .checked_add(
            diagnostic
                .response_bytes
                .checked_mul(2)
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)
}

fn diagnostic_fingerprint_input_upper_bound(
    diagnostic: &crate::record_biblio_cut::SourceCutSchemaDiagnostic,
) -> Result<usize, ItemRefusal> {
    let text_bytes = [
        diagnostic.path.len(),
        diagnostic.verdict.format_profile.id().len(),
        diagnostic.verdict.root_uri.len(),
        diagnostic.verdict.worker_protocol_id.len(),
        diagnostic.unit.member_id.len(),
        diagnostic.unit.relative_path.len(),
        diagnostic.unit.root_uri.len(),
        diagnostic.checkpoint.profile.id().len(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| total.checked_add(bytes))
    .ok_or(ItemRefusal::Budget)?;
    // Fixed digests, counters, booleans, hash-text length prefixes, and the
    // fixed transcript labels are all covered by this fixed allowance.
    text_bytes.checked_add(2_048).ok_or(ItemRefusal::Budget)
}

fn complete_invalid_artifact_schema_diagnostic(
    diagnostic: &crate::record_biblio_cut::SourceCutSchemaDiagnostic,
    path: &str,
    member_sha256: Digest256,
    member_size_bytes: u64,
) -> bool {
    let unit = &diagnostic.unit;
    let report = &unit.report;
    let Some(issue_count) = u64::try_from(report.issues.len()).ok() else {
        return false;
    };
    !diagnostic.verdict.valid
        && diagnostic.path == path
        && diagnostic.verdict.root_uri == V2_ARTIFACT_SCHEMA
        && unit.relative_path == path
        && unit.root_uri == V2_ARTIFACT_SCHEMA
        && unit.ordinal == 0
        && unit.member_id == "biblio-record-schema-unit"
        && diagnostic.verdict.worker_protocol_id == "tos_schema_diagnostics_v2"
        && diagnostic.verdict.instance_sha256 == member_sha256
        && unit.raw_sha256 == member_sha256
        && u64::try_from(diagnostic.input_instance_bytes).ok() == Some(member_size_bytes)
        && diagnostic.checkpoint.worker_sha256 == report.worker_sha256
        && diagnostic.checkpoint.request_sha256 == report.request_sha256
        && diagnostic.checkpoint.schema_set_sha256 == report.schema_set_sha256
        && diagnostic.checkpoint.caps_sha256 == report.caps_sha256()
        && diagnostic.checkpoint.profile == diagnostic.verdict.format_profile
        && diagnostic.verdict.worker_binary_digest == report.worker_sha256
        && diagnostic.verdict.schema_set_digest == report.schema_set_sha256
        && report.unit_sha256 == unit.unit_sha256
        && report.is_well_formed()
        && report.status == DiagnosticStatus::Invalid
        && report.failure == DiagnosticFailure::None
        && !report.truncated
        && report.total_issue_count == issue_count
}

fn hash_text(hasher: &mut Digest256Hasher, value: &str) -> Result<(), ItemRefusal> {
    let length = u64::try_from(value.len()).map_err(|_| ItemRefusal::Budget)?;
    hasher.update(&length.to_be_bytes());
    hasher.update(value.as_bytes());
    Ok(())
}

fn hash_digest(hasher: &mut Digest256Hasher, digest: Digest256) {
    hasher.update(digest.as_bytes());
}

fn hash_u64(hasher: &mut Digest256Hasher, value: u64) {
    hasher.update(&value.to_be_bytes());
}

fn hash_usize(hasher: &mut Digest256Hasher, value: usize) -> Result<(), ItemRefusal> {
    hash_u64(
        hasher,
        u64::try_from(value).map_err(|_| ItemRefusal::Budget)?,
    );
    Ok(())
}

fn hash_optional_u64(hasher: &mut Digest256Hasher, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_u64(hasher, value);
        }
        None => hasher.update(&[0]),
    }
}

fn hash_exceptional_usage(
    hasher: &mut Digest256Hasher,
    value: Option<crate::executor::ExceptionalSchemaUsage>,
) {
    let Some(value) = value else {
        hasher.update(&[0]);
        return;
    };
    hasher.update(&[1]);
    for field in [
        value.schema_scan_work,
        value.schema_scan_bytes,
        value.pattern_compile_count,
        value.pattern_bytes,
        value.evaluation_work,
        value.evaluation_bytes,
        value.reference_steps,
        value.regex_checks,
        value.regex_bytes,
    ] {
        hash_u64(hasher, field);
    }
}

fn update_candidate_fingerprint(
    hasher: &mut Digest256Hasher,
    record_id: &str,
    record: &BiblioCurrentRecord,
    member: &tos_source_store::MemberMetadata,
) -> Result<(), ItemRefusal> {
    hash_text(hasher, "candidate")?;
    hash_text(hasher, record_id)?;
    hash_text(hasher, &record.path)?;
    hash_digest(hasher, member.sha256);
    hash_u64(hasher, member.size_bytes);
    hash_text(
        hasher,
        string(&record.value, "schema_version").unwrap_or(""),
    )?;
    hash_text(hasher, string(&record.value, "$schema").unwrap_or(""))?;
    Ok(())
}

fn candidate_artifact_record_fingerprint_bytes(
    record_id: &str,
    record: &BiblioCurrentRecord,
) -> Result<usize, ItemRefusal> {
    [
        record_id.len(),
        record.path.len(),
        string(&record.value, "schema_version").unwrap_or("").len(),
        string(&record.value, "$schema").unwrap_or("").len(),
    ]
    .into_iter()
    .try_fold(256usize, |bytes, field| {
        bytes
            .checked_add(field)
            .and_then(|sum| sum.checked_add(8))
            .ok_or(ItemRefusal::Budget)
    })
}

fn update_candidate_artifact_record_fingerprint(
    hasher: &mut Digest256Hasher,
    record_id: &str,
    record: &BiblioCurrentRecord,
) -> Result<(), ItemRefusal> {
    hash_text(hasher, "candidate-artifact-record")?;
    hash_text(hasher, record_id)?;
    hash_text(hasher, &record.path)?;
    hash_text(
        hasher,
        string(&record.value, "schema_version").unwrap_or(""),
    )?;
    hash_text(hasher, string(&record.value, "$schema").unwrap_or(""))?;
    Ok(())
}

fn update_diagnostic_fingerprint(
    hasher: &mut Digest256Hasher,
    diagnostic: &crate::record_biblio_cut::SourceCutSchemaDiagnostic,
) -> Result<(), ItemRefusal> {
    use crate::executor::schema_diagnostics::{Failure, Status};

    hash_text(hasher, "diagnostic")?;
    hash_text(hasher, &diagnostic.path)?;
    hash_u64(hasher, diagnostic.evaluation_ordinal);
    hash_usize(hasher, diagnostic.before_issue)?;

    hash_digest(hasher, diagnostic.verdict.instance_sha256);
    hash_digest(hasher, diagnostic.verdict.schema_set_digest);
    hash_text(hasher, diagnostic.verdict.format_profile.id())?;
    hash_text(hasher, &diagnostic.verdict.root_uri)?;
    hash_text(hasher, &diagnostic.verdict.worker_protocol_id)?;
    hash_digest(hasher, diagnostic.verdict.worker_binary_digest);
    hasher.update(&[u8::from(diagnostic.verdict.valid)]);

    hash_u64(hasher, diagnostic.unit.ordinal);
    hash_text(hasher, &diagnostic.unit.member_id)?;
    hash_text(hasher, &diagnostic.unit.relative_path)?;
    hash_text(hasher, &diagnostic.unit.root_uri)?;
    hash_digest(hasher, diagnostic.unit.raw_sha256);
    hash_digest(hasher, diagnostic.unit.unit_sha256);

    let checkpoint = diagnostic.checkpoint;
    hash_digest(hasher, checkpoint.worker_sha256);
    hash_digest(hasher, checkpoint.request_sha256);
    hash_text(hasher, checkpoint.profile.id())?;
    hash_digest(hasher, checkpoint.schema_set_sha256);
    hash_digest(hasher, checkpoint.ordered_manifest_sha256);
    hash_digest(hasher, checkpoint.caps_sha256);
    hash_u64(hasher, checkpoint.completed_count);
    hash_digest(hasher, checkpoint.result_stream_sha256);
    hash_u64(hasher, checkpoint.worker_request_bytes);
    hash_u64(hasher, checkpoint.worker_response_bytes);
    hash_optional_u64(hasher, checkpoint.worker_cpu_micros);
    hash_exceptional_usage(hasher, checkpoint.exceptional_remaining);
    hash_exceptional_usage(hasher, checkpoint.exceptional_usage);

    hash_u64(hasher, u64::from(diagnostic.unit.report.protocol_version));
    hash_digest(hasher, diagnostic.unit.report.worker_sha256);
    hash_digest(hasher, diagnostic.unit.report.request_sha256);
    hash_digest(hasher, diagnostic.unit.report.unit_sha256);
    hash_digest(hasher, diagnostic.unit.report.schema_set_sha256);
    let caps = diagnostic.unit.report.caps;
    hash_u64(hasher, u64::from(caps.max_issues_per_unit));
    hash_u64(hasher, u64::from(caps.max_report_bytes_per_unit));
    hash_u64(hasher, u64::from(caps.max_path_segments));
    hash_u64(hasher, u64::from(caps.max_path_bytes));
    hasher.update(&[diagnostic.unit.report.status as u8]);
    hasher.update(&[diagnostic.unit.report.failure as u8]);
    hash_u64(hasher, diagnostic.unit.report.total_issue_count);
    hasher.update(&[u8::from(diagnostic.unit.report.truncated)]);
    hash_digest(hasher, diagnostic.unit.report.issues_sha256);
    hash_digest(hasher, diagnostic.unit.report.report_sha256);
    hash_usize(hasher, diagnostic.unit.report.issues.len())?;

    for field in [
        diagnostic.schema_resource_bytes,
        diagnostic.schema_resource_buffer_bytes,
        diagnostic.input_instance_bytes,
        diagnostic.input_instance_buffer_bytes,
        diagnostic.input_metadata_bytes,
        diagnostic.request_bytes,
        diagnostic.request_buffer_bytes,
        diagnostic.response_bytes,
        diagnostic.response_buffer_bytes,
        diagnostic.retained_state_bytes,
        diagnostic.accounted_state_bytes,
    ] {
        hash_usize(hasher, field)?;
    }
    hash_u64(hasher, diagnostic.worker_cpu_micros);

    // Keep the statuses used by the narrow proof in the transcript as well as
    // the report's full digest, so a replacement row cannot reuse a digest
    // field while changing its owner-facing verdict flags.
    hasher.update(&[u8::from(diagnostic.verdict.valid)]);
    hasher.update(&[u8::from(diagnostic.unit.report.status == Status::Invalid)]);
    hasher.update(&[u8::from(diagnostic.unit.report.failure == Failure::None)]);
    Ok(())
}
fn check_native_artifact_history<S: LayerFamilySource + ?Sized, I: Copy + Eq>(
    inspector: &mut Inspector<'_, '_, S, I>,
    artifact_path: &str,
    current_record: &Value,
    binding: NativeArtifactBinding<'_, I>,
    observation: &NativeHistoryRef<'_, I>,
    replay: Option<&ArtifactReplayRef<'_, I>>,
) -> Result<(bool, bool), ItemRefusal> {
    inspector.reference_native_history(observation)?;
    let mut valid = true;
    if !observation.matches_binding(binding) {
        inspector.issue(
            artifact_path,
            "native-artifact-history-input-drift",
            "selected native Artifact history belongs to a different exact source input",
        )?;
        valid = false;
    }
    if observation.record_path() != artifact_path
        || observation.identity_field() != "artifact_id"
        || string(current_record, "artifact_id") != Some(observation.identity())
        || !python_json_equal(current_record, observation.current_record())
    {
        inspector.issue(
            artifact_path,
            "native-artifact-history-current-record-drift",
            "selected native Artifact history does not bind the exact current artifact record",
        )?;
        valid = false;
    }
    let basename = artifact_path.rsplit('/').next().unwrap_or("");
    let current_digest = inspector.digest(artifact_path)?;
    let selected_digest = observation
        .selected_package()
        .get(basename)
        .map(|raw| Digest256::of_bytes(raw).to_hex());
    if current_digest.as_deref() != selected_digest.as_deref() {
        inspector.issue(
            artifact_path,
            "native-artifact-history-package-drift",
            "selected native Artifact package bytes differ from exact current source bytes",
        )?;
        valid = false;
    }
    if observation.history_receipt_count()
        != observation
            .history()
            .get("receipts")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    {
        inspector.issue(
            artifact_path,
            "native-artifact-history-observation-drift",
            "selected native Artifact history receipt count differs from its measured observation",
        )?;
        valid = false;
    }
    let history_receipts = observation
        .history()
        .get("receipts")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut replay_valid = true;
    if let Some(replay) = replay {
        inspector.reference_artifact_replay(artifact_path, replay)?;
        if !replay.matches_binding(binding)
            || replay.source_path() != artifact_path
            || replay.record_id() != observation.identity()
            || replay.record_id() != string(current_record, "artifact_id").unwrap_or("")
            || replay.origin_record_sha256() != observation.origin_record_sha256()
            || replay.origin_record_byte_size() != observation.origin_record_byte_size()
            || replay.history_sha256() != observation.history_sha256()
            || replay.transaction_count() != history_receipts.len()
            || observation.transaction_count() != history_receipts.len()
        {
            inspector.issue(
                artifact_path,
                "native-artifact-correction-replay-binding-drift",
                "CMD correction replay does not bind the exact selected cut, Artifact origin, history, and ordered receipt count",
            )?;
            replay_valid = false;
        }
    }
    let mut transaction_ids = BTreeSet::new();
    for (index, receipt) in history_receipts.iter().enumerate() {
        inspector.checkpoint()?;
        let Some(publication) = receipt.get("publication") else {
            inspector.issue(
                artifact_path,
                "native-artifact-correction-publication-missing",
                "retained native Artifact correction lacks selected publication evidence",
            )?;
            valid = false;
            continue;
        };
        let Some(transaction_id) = string(publication, "transaction_id") else {
            inspector.issue(
                artifact_path,
                "native-artifact-correction-publication-invalid",
                "retained native Artifact correction has no selected transaction identity",
            )?;
            valid = false;
            continue;
        };
        if !transaction_ids.insert(transaction_id) {
            inspector.issue(
                artifact_path,
                "native-artifact-correction-transaction-duplicate",
                "native Artifact correction history repeats a publication transaction",
            )?;
            valid = false;
        }
        if !observation.has_transaction(transaction_id) {
            inspector.issue(
                artifact_path,
                "native-artifact-correction-transaction-missing",
                "selected native Artifact correction transaction is absent from the measured history",
            )?;
            valid = false;
            continue;
        }
        if !observation.transaction_is_committed(transaction_id) {
            inspector.issue(
                artifact_path,
                "native-artifact-correction-transaction-uncommitted",
                "selected native Artifact correction transaction is not committed",
            )?;
            valid = false;
        }

        if let Some(replay) = replay {
            let current_receipt_sha256 = inspector.canonical_record_sha256(receipt)?;
            match replay.transaction_at(index) {
                Some(replay_transaction)
                    if replay_transaction.transaction_id == transaction_id
                        && Some(replay_transaction.manifest_sha256.as_str())
                            == observation.transaction_manifest_sha256(transaction_id)
                        && replay_transaction.receipt_sha256 == current_receipt_sha256 => {}
                _ => {
                    inspector.issue(
                        artifact_path,
                        "native-artifact-correction-replay-receipt-drift",
                        "CMD correction replay transaction does not bind the exact ordered current receipt and selected publication",
                    )?;
                    replay_valid = false;
                }
            }
        }
    }
    if replay.is_some() && !replay_valid {
        valid = false;
    }
    let correction_replay_pending = !history_receipts.is_empty() && replay.is_none();
    Ok((valid, correction_replay_pending))
}

fn native_artifact_capture<S: LayerFamilySource + ?Sized, I: Copy + Eq>(
    inspector: &mut Inspector<'_, '_, S, I>,
    artifact_path: &str,
    current_record: &Value,
    current_member_size_bytes: u64,
    discoveries: &BTreeMap<String, DiscoveryInfo>,
    binding: Option<NativeArtifactBinding<'_, I>>,
    native_history: Option<NativeHistoryRef<'_, I>>,
    artifact_replay: Option<ArtifactReplayRef<'_, I>>,
    invalid_current_schema_proofs: Option<&CurrentArtifactInvalidSchemaProofs<'_>>,
    candidate_schema_invalid_override: Option<bool>,
) -> Result<NativeArtifactCapture, ItemRefusal> {
    let issue_start = inspector.issues.len();
    let unsupported_start = inspector.unsupported.len();
    let parent = artifact_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let companion_paths: Vec<String> = NATIVE_ARTIFACT_COMPANIONS
        .iter()
        .map(|name| format!("{parent}/{name}"))
        .collect();
    let mut present = Vec::with_capacity(companion_paths.len());
    let mut unobserved = false;
    for path in &companion_paths {
        match inspector.physical_path_facts(path) {
            Some(facts) => present.push(facts.exists),
            None => {
                present.push(false);
                unobserved = true;
            }
        }
    }
    if unobserved {
        inspector.unsupported(
            artifact_path,
            "native Artifact companion existence is not fully observed in the selected checkout",
        )?;
        return Ok(NativeArtifactCapture::Unobserved);
    }
    let present_count = present.iter().filter(|present| **present).count();
    if present_count == 0 {
        return Ok(NativeArtifactCapture::Legacy);
    }
    if present_count != companion_paths.len() {
        inspector.issue(
            artifact_path,
            "native-artifact-capture-incomplete",
            "Artifact native creation capture is incomplete",
        )?;
        return Ok(NativeArtifactCapture::Partial);
    }
    if string(current_record, "$schema") != Some(V2_ARTIFACT_SCHEMA) {
        inspector.issue(
            artifact_path,
            "native-artifact-current-schema-drift",
            "retained native Artifact creation evidence requires the current v2 record schema",
        )?;
        return Ok(NativeArtifactCapture::Invalid);
    }
    let exact_invalid_current_schema = match binding {
        Some(NativeArtifactBinding::Cut(binding)) => {
            invalid_current_schema_proofs.is_some_and(|proofs| {
                inspector
                    .cached_digest(artifact_path)
                    .is_some_and(|digest| {
                        proofs.proves_invalid(
                            artifact_path,
                            binding.revision,
                            binding.membership,
                            digest,
                            current_member_size_bytes,
                        )
                    })
            })
        }
        Some(NativeArtifactBinding::Candidate {
            identity,
            membership,
        }) => {
            if let Some(invalid) = candidate_schema_invalid_override {
                invalid
            } else if let Some(proofs) = inspector.candidate_invalid_schema_proofs {
                if !proofs.binding_matches(identity, membership) {
                    false
                } else if let Some(digest) = inspector.cached_digest(artifact_path) {
                    proofs.proves_invalid(
                        artifact_path,
                        digest,
                        current_member_size_bytes,
                        inspector.limits.deadline,
                        inspector.source.cancellation(),
                    )?
                } else {
                    false
                }
            } else {
                false
            }
        }
        None => false,
    };

    let request_path = &companion_paths[0];
    let receipt_path = &companion_paths[1];
    let environment_path = &companion_paths[2];
    let provenance_path = &companion_paths[3];
    let forms_path = format!("{parent}/artifact-witness.human-forms.json");
    let request = inspector.current_object_with_ordered_value(request_path)?;
    let receipt = inspector.current_object_unvalidated(receipt_path)?;
    let human_forms = inspector.current_object_with_ordered_value(&forms_path)?;
    if let Some((forms_value, _, _, forms_size)) = human_forms.as_ref() {
        inspector.request_schema(&forms_path, HUMAN_FORM_SET_SCHEMA, forms_value, *forms_size)?;
    }
    let _environment_digest = if inspector.has_current_member(environment_path.as_str())? {
        inspector.digest(environment_path)?
    } else {
        inspector.issue(
            artifact_path,
            "native-artifact-capture-not-current",
            environment_path,
        )?;
        None
    };
    let provenance = inspector.required_jsonl_objects(provenance_path, PROVENANCE_V2_SCHEMA)?;
    if provenance.len() != 1 {
        inspector.issue(
            provenance_path,
            "native-artifact-creation-event-count",
            "Artifact creation requires one exact serialization event",
        )?;
    }

    if let (Some((request, request_ordered, _, request_size)), Some((receipt, _, _))) =
        (request, receipt)
    {
        let expected_request_keys = BTreeSet::from([
            "schema_version",
            "operation",
            "record",
            "forms",
            "source_bindings",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]);
        let actual_request_keys: BTreeSet<&str> = request
            .as_object()
            .map(|object| object.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if actual_request_keys != expected_request_keys {
            inspector.issue(
                request_path,
                "native-artifact-request-shape-drift",
                "retained source.create request fields differ from the maintained command contract",
            )?;
        }
        if string(&request, "schema_version") != Some("tos_local_source_command_v1")
            || string(&request, "operation") != Some("source.create")
            || !request.get("expected_source").is_some_and(Value::is_null)
            || !request.get("expected_revision").is_some_and(Value::is_null)
        {
            inspector.issue(
                request_path,
                "native-artifact-request-profile-drift",
                "retained request is not an unconditioned source.create request",
            )?;
        }
        let expected_receipt_keys = BTreeSet::from([
            "schema_version",
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "source_path",
            "source",
            "dependencies",
            "files",
            "grants_admission",
        ]);
        let actual_receipt_keys: BTreeSet<&str> = receipt
            .as_object()
            .map(|object| object.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if actual_receipt_keys != expected_receipt_keys
            || string(&receipt, "schema_version") != Some("tos_local_source_create_receipt_v1")
        {
            inspector.issue(
                receipt_path,
                "native-artifact-receipt-shape-drift",
                "retained receipt fields differ from the maintained source.create receipt contract",
            )?;
        }
        let command_id = string(&request, "command_id");
        if !command_id.is_some_and(|value| (1..=256).contains(&value.len())) {
            inspector.issue(
                request_path,
                "native-artifact-command-id-invalid",
                "retained request command_id must contain 1 to 256 characters",
            )?;
        }
        let form_selections = array(&request, "forms");
        let mut selected_form_ids = BTreeSet::new();
        if !(1..=32).contains(&form_selections.len()) {
            inspector.issue(
                request_path,
                "native-artifact-form-selection-count",
                "retained request must select between 1 and 32 source-copy forms",
            )?;
        }
        for selection in form_selections {
            let selection_keys: BTreeSet<&str> = selection
                .as_object()
                .map(|object| object.keys().map(String::as_str).collect())
                .unwrap_or_default();
            let form_id = string(selection, "form_id");
            let field_id = string(selection, "field_id");
            if selection_keys != BTreeSet::from(["form_id", "field_id"])
                || !form_id.is_some_and(native_form_id)
                || !field_id.is_some_and(|value| !value.trim().is_empty())
            {
                inspector.issue(
                    request_path,
                    "native-artifact-form-selection-shape-drift",
                    "each retained form selection must contain exact nonempty form_id and field_id strings",
                )?;
            }
            if let Some(form_id) = form_id {
                if !selected_form_ids.insert(form_id.to_owned()) {
                    inspector.issue(
                        request_path,
                        "native-artifact-form-selection-duplicate",
                        form_id,
                    )?;
                }
            }
        }
        let canonical_request = serde_json::to_vec(&request)
            .map_err(|_| ItemRefusal::Source("source request encoding failed".into()))?;
        let request_digest = format!(
            "sha256:{}",
            Digest256::of_bytes(&canonical_request).to_hex()
        );
        if !python_optional_json_equal(
            receipt.get("source_path"),
            Some(&Value::String(artifact_path.to_owned())),
        ) {
            inspector.issue(
                receipt_path,
                "native-artifact-source-path-drift",
                artifact_path,
            )?;
        }
        if !python_optional_json_equal(receipt.get("command_id"), request.get("command_id"))
            || string(&receipt, "request_digest") != Some(request_digest.as_str())
            || !python_optional_json_equal(
                receipt.get("owner_configuration"),
                request.get("expected_configuration"),
            )
            || !python_optional_json_equal(
                receipt.get("dependencies"),
                request.get("expected_dependencies"),
            )
        {
            inspector.issue(
                receipt_path,
                "native-artifact-request-binding-drift",
                "creation receipt does not bind its retained request",
            )?;
        }
        if receipt.get("grants_admission") != Some(&Value::Bool(false)) {
            inspector.issue(
                receipt_path,
                "native-artifact-receipt-admission-drift",
                "creation receipt must not grant admission",
            )?;
        }

        let original = request.get("record").unwrap_or(&Value::Null);
        if !original.is_object() {
            inspector.issue(
                request_path,
                "native-artifact-original-record-missing",
                "retained creation request has no native record",
            )?;
        } else {
            if first_forbidden_content_fields(original).is_some() || local_only_absolute(original) {
                inspector.issue(
                    request_path,
                    "native-artifact-initial-content-exposure",
                    "retained initial Artifact record exposes forbidden content or an owner-local absolute path",
                )?;
            }
            inspector.request_schema(
                &format!("{request_path}#/record"),
                ARTIFACT_V2_SCHEMA,
                original,
                request_size,
            )?;
            let artifact_path_parts: Vec<&str> = artifact_path.split('/').collect();
            let maker = original.get("maker").unwrap_or(&Value::Null);
            let expected_maker = json!({
                "maker_type": string(maker, "maker_type").unwrap_or(""),
                "agent_ref": string(&receipt, "principal_id").unwrap_or(""),
                "human_review_performed": false,
            });
            let maker_type = string(maker, "maker_type").unwrap_or("");
            let principal_id = string(&receipt, "principal_id").unwrap_or("");
            let original_id = string(original, "artifact_id").unwrap_or("");
            let original_event_id = string(original, "provenance_event_ref").unwrap_or("");
            if artifact_path_parts.len() != 7
                || artifact_path_parts[..3] != ["ToS", "source-witnesses", "artifacts"]
                || artifact_path_parts[6] != "artifact-witness.json"
                || artifact_path_parts[3..6]
                    .iter()
                    .any(|part| part.eq_ignore_ascii_case("cdli"))
                || string(original, "schema_version") != Some("tos_artifact_source_witness_v2")
                || json_integer(original.get("record_version").unwrap_or(&Value::Null)) != Some(1)
                || !native_slug_id(original_id, "tos.artifact.")
                || !native_slug_id(original_event_id, "tos.event.")
                || !["human", "software", "model"].contains(&maker_type)
                || principal_id.trim().is_empty()
                || string(
                    original.pointer("/authority").unwrap_or(&Value::Null),
                    "review_status",
                ) != Some("unreviewed")
                || !python_optional_json_equal(original.get("maker"), Some(&expected_maker))
                || !python_optional_json_equal(
                    original.get("philosophy_planting_refs"),
                    Some(&Value::Array(Vec::new())),
                )
            {
                inspector.issue(
                    request_path,
                    "native-artifact-initial-record-profile-drift",
                    "retained creation record is not the exact unreviewed native Artifact profile",
                )?;
            }
            if string(original, "schema_version") != Some("tos_artifact_source_witness_v2")
                || string(original, "artifact_id") != string(current_record, "artifact_id")
                || string(original, "provenance_event_ref")
                    != string(current_record, "provenance_event_ref")
            {
                inspector.issue(
                    request_path,
                    "native-artifact-initial-identity-drift",
                    "retained creation record does not preserve the current artifact identity and creation event",
                )?;
            }
            let canonical_record = serde_json::to_vec(original)
                .map_err(|_| ItemRefusal::Source("source record encoding failed".into()))?;
            let expected_record_digest =
                format!("sha256:{}", Digest256::of_bytes(&canonical_record).to_hex());
            let receipt_source = receipt.get("source").unwrap_or(&Value::Null);
            let expected_source_keys = BTreeSet::from(["id", "version", "digest"]);
            let actual_source_keys: BTreeSet<&str> = receipt_source
                .as_object()
                .map(|object| object.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if actual_source_keys != expected_source_keys
                || string(receipt_source, "id") != string(original, "artifact_id")
                || !python_optional_json_equal(
                    receipt_source.get("version"),
                    original.get("record_version"),
                )
                || string(receipt_source, "digest") != Some(expected_record_digest.as_str())
            {
                inspector.issue(
                    receipt_path,
                    "native-artifact-initial-record-binding-drift",
                    "creation receipt does not bind the exact initial artifact record",
                )?;
            }

            let source_bindings = request.get("source_bindings").unwrap_or(&Value::Null);
            let expected_binding_names =
                BTreeSet::from(["rights_ref", "discovery_ref", "research_ref"]);
            let actual_binding_names: BTreeSet<&str> = source_bindings
                .as_object()
                .map(|rows| rows.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if actual_binding_names != expected_binding_names {
                inspector.issue(
                    request_path,
                    "native-artifact-source-bindings-drift",
                    "retained request must bind distinct rights, discovery, and research inputs",
                )?;
            }
            let mut input_refs = BTreeSet::new();
            for field in ["rights_ref", "discovery_ref", "research_ref"] {
                let binding = source_bindings.get(field).unwrap_or(&Value::Null);
                let expected_binding_keys = BTreeSet::from(["ref", "sha256"]);
                let actual_binding_keys: BTreeSet<&str> = binding
                    .as_object()
                    .map(|object| object.keys().map(String::as_str).collect())
                    .unwrap_or_default();
                let reference = string(binding, "ref").unwrap_or("");
                let parts: Vec<&str> = reference.split('/').collect();
                if actual_binding_keys != expected_binding_keys
                    || !string(binding, "sha256").is_some_and(lowercase_sha256)
                {
                    inspector.issue(
                        request_path,
                        "native-artifact-source-binding-shape-drift",
                        field,
                    )?;
                }
                if !safe_relative_path(reference)
                    || !reference.starts_with("ToS/")
                    || parts.iter().any(|part| {
                        part.starts_with('.')
                            || ["payload", "local-content", "owner-local", "catalog"].contains(part)
                    })
                    || reference.split('/').count() < 3
                {
                    inspector.issue(
                        request_path,
                        "native-artifact-source-input-path-invalid",
                        format!("{field} is not a public ToS input path"),
                    )?;
                    continue;
                }
                if !input_refs.insert(reference.to_owned()) {
                    inspector.issue(
                        request_path,
                        "native-artifact-source-input-duplicate",
                        reference,
                    )?;
                }
                if field == "discovery_ref" && !reference.starts_with(DISCOVERY_RUNS) {
                    inspector.issue(
                        request_path,
                        "native-artifact-discovery-route-drift",
                        reference,
                    )?;
                }
                if !python_optional_json_equal(
                    original.get(field),
                    Some(&Value::String(reference.to_owned())),
                ) {
                    inspector.issue(
                        request_path,
                        "native-artifact-source-reference-drift",
                        field,
                    )?;
                }
                let digest = string(binding, "sha256");
                let current_digest = inspector.digest(reference)?;
                if !current_digest.is_some_and(|current| digest == Some(current.as_str())) {
                    inspector.issue(
                        request_path,
                        "native-artifact-source-input-digest-drift",
                        reference,
                    )?;
                }
                if let Some(facts) = inspector.physical_path_facts(reference) {
                    if facts.symlink || !facts.regular_file || !facts.exists {
                        inspector.issue(
                            request_path,
                            "native-artifact-source-input-not-regular",
                            reference,
                        )?;
                    }
                    if facts
                        .sha256
                        .as_deref()
                        .is_some_and(|sha| digest != Some(sha))
                    {
                        inspector.issue(
                            request_path,
                            "native-artifact-source-input-physical-digest-drift",
                            reference,
                        )?;
                    } else if facts.sha256.is_none() {
                        inspector.unsupported(
                            request_path,
                            "native Artifact source-input physical SHA-256 is unobserved",
                        )?;
                    }
                } else {
                    inspector.unsupported(
                        request_path,
                        "native Artifact source-input physical posture is unobserved",
                    )?;
                }
            }
            let discovery_ref = string(
                source_bindings.get("discovery_ref").unwrap_or(&Value::Null),
                "ref",
            )
            .unwrap_or("");
            if let Some(discovery) = discoveries.get(discovery_ref) {
                for fingerprint in original
                    .pointer("/digital_catalog_record/response_fingerprints")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if fingerprint.get("captured") != Some(&Value::Bool(true)) {
                        continue;
                    }
                    let surface = string(fingerprint, "surface");
                    let sha256 = string(fingerprint, "sha256");
                    let byte_size = fingerprint.get("byte_size").and_then(Value::as_u64);
                    let matched = match (surface, sha256, byte_size) {
                        (Some(surface), Some(sha256), Some(byte_size)) => discovery
                            .captured_acquisitions
                            .iter()
                            .any(|(candidate_surface, snapshot_sha, acquisition_sha, size)| {
                                candidate_surface == surface
                                    && snapshot_sha == sha256
                                    && acquisition_sha == sha256
                                    && *size == byte_size
                            }),
                        _ => false,
                    };
                    if !matched {
                        inspector.issue(
                            request_path,
                            "native-artifact-response-fingerprint-unresolved",
                            "captured response fingerprint has no exact discovery snapshot and acquisition account",
                        )?;
                    }
                }
            } else {
                inspector.issue(
                    request_path,
                    "native-artifact-discovery-input-unresolved",
                    discovery_ref,
                )?;
            }
        }

        let (history_valid, correction_replay_pending) = if exact_invalid_current_schema {
            (false, false)
        } else {
            match (binding, native_history) {
                (Some(binding), Some(observation)) => check_native_artifact_history(
                    inspector,
                    artifact_path,
                    current_record,
                    binding,
                    &observation,
                    artifact_replay.as_ref(),
                )?,
                (Some(_), None) => {
                    inspector.unsupported(
                        artifact_path,
                        "same-cut selected native Artifact history observation is unavailable",
                    )?;
                    (false, false)
                }
                (None, Some(observation)) => {
                    inspector.reference_native_history(&observation)?;
                    if let Some(replay) = artifact_replay {
                        inspector.reference_artifact_replay(artifact_path, &replay)?;
                        inspector.issue(
                        artifact_path,
                        "native-artifact-correction-replay-cut-unavailable",
                        "CMD correction replay cannot be joined without the caller's exact source cut",
                    )?;
                    }
                    inspector.unsupported(
                    artifact_path,
                    "native Artifact history cannot be joined without the caller's exact source cut",
                )?;
                    (false, false)
                }
                (None, None) => {
                    if let Some(replay) = artifact_replay {
                        inspector.reference_artifact_replay(artifact_path, &replay)?;
                        inspector.issue(
                        artifact_path,
                        "native-artifact-correction-replay-history-unavailable",
                        "CMD correction replay has no exact selected native Artifact history to join",
                    )?;
                    }
                    inspector.unsupported(
                    artifact_path,
                    "selected native Artifact history requires the caller's exact source cut and observation",
                )?;
                    (false, false)
                }
            }
        };

        let form_replay = if let Some((_, forms_ordered, _, _)) = human_forms.as_ref() {
            replay_native_artifact_initial_forms(
                inspector,
                request_path,
                &forms_path,
                &request_ordered,
                forms_ordered,
                string(&receipt, "principal_id").unwrap_or(""),
            )?
        } else {
            None
        };

        if let Some(replay) = form_replay.as_ref() {
            if !exact_invalid_current_schema {
                if let Some(observation) = native_history {
                    let history_receipts = observation
                        .history()
                        .get("receipts")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    if let Some(first) = history_receipts.first() {
                        if !python_optional_json_equal(
                            first.get("previous_source"),
                            Some(&replay.initial_source),
                        ) {
                            inspector.issue(
                                artifact_path,
                                "native-artifact-history-origin-drift",
                                "selected native Artifact history does not begin at its retained creation subject",
                            )?;
                        }
                    } else {
                        let current_ordered = serde_value_as_ordered(
                            observation.current_record(),
                            inspector.limits.max_member_bytes,
                        )?;
                        let current_subject =
                            crate::source_forms::source_copy_kernel::metadata_subject(
                                &current_ordered,
                            );
                        match current_subject {
                            Ok(subject) => {
                                let current_subject = ordered_value_as_serde(
                                    &subject,
                                    inspector.limits.max_member_bytes,
                                )?;
                                if !python_json_equal(&current_subject, &replay.initial_source) {
                                    inspector.issue(
                                        artifact_path,
                                        "native-artifact-history-origin-drift",
                                        "current native Artifact differs from its retained creation subject without revision history",
                                    )?;
                                }
                            }
                            Err(_) => {
                                inspector.issue(
                                    artifact_path,
                                    "native-artifact-history-current-subject-invalid",
                                    "current native Artifact cannot produce its maintained source-copy subject",
                                )?;
                            }
                        }
                    }
                }
            }
            if !python_optional_json_equal(receipt.get("source"), Some(&replay.initial_source)) {
                inspector.issue(
                    receipt_path,
                    "native-artifact-initial-record-binding-drift",
                    "creation receipt source differs from the exact retained initial Artifact subject",
                )?;
            }
        }

        if correction_replay_pending {
            inspector.unsupported(
                artifact_path,
                "selected native Artifact corrections lack exact pending-plan replay equality",
            )?;
        }

        let expected_file_paths = [
            ("artifact-witness.json", artifact_path.to_owned()),
            (
                "artifact-witness.human-forms.json",
                format!("{parent}/artifact-witness.human-forms.json"),
            ),
            ("source-create-environment.json", environment_path.clone()),
            ("source-create-provenance.jsonl", provenance_path.clone()),
            ("source-create-request.json", request_path.clone()),
        ];
        let receipt_files = receipt.get("files").unwrap_or(&Value::Null);
        let expected_names: BTreeSet<&str> =
            expected_file_paths.iter().map(|(name, _)| *name).collect();
        let actual_names: BTreeSet<&str> = receipt_files
            .as_object()
            .map(|rows| rows.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if actual_names != expected_names {
            inspector.issue(
                receipt_path,
                "native-artifact-receipt-file-set-drift",
                "creation receipt file inventory is incomplete or contains extra entries",
            )?;
        }
        if history_valid {
            if let Some(observation) = native_history {
                let receipt_current_digest = inspector.digest(receipt_path)?;
                let selected_receipt_digest = observation
                    .selected_package()
                    .get("source-create-receipt.json")
                    .map(|raw| Digest256::of_bytes(raw).to_hex());
                if receipt_current_digest.as_deref() != selected_receipt_digest.as_deref() {
                    inspector.issue(
                        receipt_path,
                        "native-artifact-history-package-drift",
                        "selected native Artifact receipt bytes differ from exact current source bytes",
                    )?;
                }
            }
        }
        for (name, file_path) in &expected_file_paths {
            if !inspector.has_current_member(file_path.as_str())? {
                inspector.issue(
                    receipt_path,
                    "native-artifact-capture-not-current",
                    file_path,
                )?;
                continue;
            }
            let current_digest = inspector.digest(file_path)?;
            let facts = inspector.exact_file_posture(file_path, receipt_path)?;
            let physical_digest = facts.as_ref().and_then(|facts| facts.sha256.as_deref());
            if physical_digest.is_some_and(|digest| current_digest.as_deref() != Some(digest)) {
                inspector.issue(
                    receipt_path,
                    "native-artifact-file-physical-digest-drift",
                    file_path,
                )?;
            }
            if history_valid {
                if let Some(observation) = native_history {
                    let selected_digest = observation
                        .selected_package()
                        .get(*name)
                        .map(|raw| Digest256::of_bytes(raw).to_hex());
                    if current_digest.as_deref() != selected_digest.as_deref() {
                        inspector.issue(
                            receipt_path,
                            "native-artifact-history-package-drift",
                            file_path,
                        )?;
                    }
                }
            }
            let binding = receipt_files.get(*name).unwrap_or(&Value::Null);
            let (expected_sha_raw, expected_bytes) = match *name {
                "artifact-witness.json" => match native_history.filter(|_| history_valid) {
                    Some(observation) => (
                        Some(observation.origin_record_sha256().to_owned()),
                        u64::try_from(observation.origin_record_byte_size()).ok(),
                    ),
                    None if json_integer(
                        current_record.get("record_version").unwrap_or(&Value::Null),
                    ) == Some(1) =>
                    {
                        (
                            physical_digest.map(str::to_owned),
                            facts.as_ref().and_then(|f| f.byte_size),
                        )
                    }
                    None => (None, None),
                },
                "artifact-witness.human-forms.json" => match form_replay.as_ref() {
                    Some(replay) if replay.forms_have_history => (
                        Some(replay.retained_form_sha256.clone()),
                        u64::try_from(replay.retained_form_size).ok(),
                    ),
                    _ => (
                        physical_digest.map(str::to_owned),
                        facts.as_ref().and_then(|f| f.byte_size),
                    ),
                },
                _ => (
                    physical_digest.map(str::to_owned),
                    facts.as_ref().and_then(|f| f.byte_size),
                ),
            };
            let expected_sha = expected_sha_raw.map(|digest| format!("sha256:{digest}"));
            let expected_binding_keys = BTreeSet::from(["sha256", "bytes"]);
            let actual_binding_keys: BTreeSet<&str> = binding
                .as_object()
                .map(|object| object.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if actual_binding_keys != expected_binding_keys
                || !expected_sha.as_deref().is_some_and(|sha| {
                    python_optional_json_equal(
                        binding.get("sha256"),
                        Some(&Value::String(sha.to_owned())),
                    )
                })
            {
                inspector.issue(
                    receipt_path,
                    "native-artifact-receipt-file-digest-drift",
                    *name,
                )?;
            }
            if let Some(byte_size) = expected_bytes {
                if !python_optional_json_equal(
                    binding.get("bytes"),
                    Some(&Value::Number(byte_size.into())),
                ) {
                    inspector.issue(
                        receipt_path,
                        "native-artifact-receipt-file-size-drift",
                        *name,
                    )?;
                }
            } else {
                inspector.unsupported(
                    receipt_path,
                    "native Artifact original file byte size is unobserved",
                )?;
            }
            if expected_sha.is_none() {
                inspector.unsupported(
                    receipt_path,
                    "native Artifact original file SHA-256 is unobserved",
                )?;
            }
        }

        if provenance.len() == 1 {
            let event = &provenance[0];
            let event_id = string(current_record, "provenance_event_ref");
            if string(event, "event_id") != event_id
                || !event.get("activity").is_some_and(Value::is_object)
                || !event.get("record_binding").is_some_and(Value::is_object)
            {
                inspector.issue(
                    provenance_path,
                    "native-artifact-creation-event-identity-drift",
                    "retained serialization event does not bind the current artifact identity",
                )?;
            }
            if string(
                event.get("record_binding").unwrap_or(&Value::Null),
                "manifest_ref",
            ) != Some(receipt_path.as_str())
                || string(
                    event.pointer("/method/procedure").unwrap_or(&Value::Null),
                    "name",
                ) != Some("native-artifact-metadata-serialization")
                || string(event.get("activity").unwrap_or(&Value::Null), "event_type")
                    != Some("annotation")
            {
                inspector.issue(
                    provenance_path,
                    "native-artifact-serialization-event-profile-drift",
                    "retained event is not the exact metadata-only serialization event",
                )?;
            }
            let expected_outputs = BTreeSet::from([
                artifact_path.to_owned(),
                format!("{parent}/artifact-witness.human-forms.json"),
            ]);
            let outputs = array(event.get("entities").unwrap_or(&Value::Null), "outputs");
            let actual_outputs: BTreeSet<String> = outputs
                .iter()
                .filter_map(|row| string(row, "entity_ref").map(str::to_owned))
                .collect();
            if outputs.len() != expected_outputs.len() || actual_outputs != expected_outputs {
                inspector.issue(
                    provenance_path,
                    "native-artifact-serialization-output-closure",
                    "creation event outputs are not the artifact and its human-form packet",
                )?;
            }
            for output in outputs {
                let Some(reference) = string(output, "entity_ref") else {
                    continue;
                };
                let name = reference.rsplit('/').next().unwrap_or("");
                let file_binding = receipt_files.get(name).unwrap_or(&Value::Null);
                let expected_sha = string(file_binding, "sha256")
                    .and_then(|digest| digest.strip_prefix("sha256:"));
                if string(output, "sha256") != expected_sha
                    || !python_optional_json_equal(
                        output.get("size_bytes"),
                        file_binding.get("bytes"),
                    )
                {
                    inspector.issue(
                        provenance_path,
                        "native-artifact-serialization-output-digest-drift",
                        reference,
                    )?;
                }
            }
            let software_refs: BTreeSet<&str> = event
                .pointer("/method/software_components")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|row| string(row, "artifact_ref"))
                .collect();
            if !software_refs.contains(NATIVE_ARTIFACT_MODULE_REF)
                || event.pointer("/rights_and_visibility/publication_authorized")
                    != Some(&Value::Bool(false))
                || event.pointer("/review_and_authority/accepted_uses")
                    != Some(&Value::Array(Vec::new()))
                || event.pointer("/review_and_authority/promotion_authorized")
                    != Some(&Value::Bool(false))
            {
                inspector.issue(
                    provenance_path,
                    "native-artifact-serialization-event-boundary-drift",
                    "creation event does not retain its non-admitting authority boundary",
                )?;
            }
        }
    }

    if exact_invalid_current_schema {
        return Ok(NativeArtifactCapture::Invalid);
    }
    if inspector.issues.len() > issue_start {
        return Ok(NativeArtifactCapture::Invalid);
    }
    if inspector.unsupported.len() > unsupported_start {
        return Ok(NativeArtifactCapture::Pending);
    }
    Ok(NativeArtifactCapture::Complete)
}

/// Inspect the complete exact-current authored membership without claiming
/// physical payload or Git posture that this source interface does not own.
pub fn inspect<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
) -> Result<Report, ItemRefusal> {
    inspect_internal(
        source,
        current_paths,
        prior_events,
        limits,
        None,
        false,
        None,
        None,
        None,
        None,
    )
}

/// Inspect authored metadata plus the bounded physical/Git facts captured by
/// the selected host adapter. Payload bytes are never reopened here.
pub fn inspect_with_physical_facts<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
) -> Result<Report, ItemRefusal> {
    inspect_internal(
        source,
        current_paths,
        prior_events,
        limits,
        Some(physical),
        false,
        None,
        None,
        None,
        None,
    )
}

/// Inspect discovery and artifact source under the same exact corpus cut used
/// by the native-history kernel and Closure. CMD supplies the complete current
/// member list and the genuine measured observations; this function never
/// replays selected history itself.
pub fn inspect_with_cut<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
    cut: &CorpusCutReader,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    require_local_payloads: bool,
) -> Result<Report, ItemRefusal> {
    let artifact_replays = ArtifactCorrectionReplayMap::new();
    inspect_with_cut_and_artifact_replays(
        source,
        current_paths,
        prior_events,
        limits,
        physical,
        cut,
        native_histories,
        &artifact_replays,
        require_local_payloads,
    )
}

/// Inspect the exact source cut while consuming successful-only Artifact
/// correction replay observations from CMD. The borrowed proof map is an
/// evidence input; current receipts, native history and cut identity are
/// independently joined here before replay-pending can be cleared.
pub fn inspect_with_cut_and_artifact_replays<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
    cut: &CorpusCutReader,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    require_local_payloads: bool,
) -> Result<Report, ItemRefusal> {
    inspect_with_cut_inputs(
        source,
        current_paths,
        prior_events,
        limits,
        physical,
        cut,
        native_histories,
        artifact_replays,
        None,
        None,
        require_local_payloads,
    )
}

/// Inspect the exact source cut with the completed Records report. This entry
/// preserves the strict native-history requirement; callers that have prepared
/// the shared invalid-schema proof use the additive proof-aware entry below.
pub fn inspect_with_cut_and_artifact_replays_and_records<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
    cut: &CorpusCutReader,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    record_report: &SourceCutRecordReport,
    require_local_payloads: bool,
) -> Result<Report, ItemRefusal> {
    inspect_with_cut_inputs(
        source,
        current_paths,
        prior_events,
        limits,
        physical,
        cut,
        native_histories,
        artifact_replays,
        Some(record_report),
        None,
        require_local_payloads,
    )
}

/// Records-aware Discovery entry using one prebuilt opaque proof set. The
/// proofset constructor is shared with CMD's retained-history selector; this
/// consumer validates its exact cut/report binding in one linear scan before
/// allowing the already-invalid schema branch to skip native history.
#[allow(clippy::too_many_arguments)]
pub fn inspect_with_cut_and_artifact_replays_and_records_with_proofs<
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
    cut: &CorpusCutReader,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    record_report: &SourceCutRecordReport,
    invalid_schema_proofs: &CurrentArtifactInvalidSchemaProofs<'_>,
    require_local_payloads: bool,
) -> Result<Report, ItemRefusal> {
    inspect_with_cut_inputs(
        source,
        current_paths,
        prior_events,
        limits,
        physical,
        cut,
        native_histories,
        artifact_replays,
        Some(record_report),
        Some(invalid_schema_proofs),
        require_local_payloads,
    )
}

fn inspect_with_cut_inputs<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
    cut: &CorpusCutReader,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    record_report: Option<&SourceCutRecordReport>,
    invalid_schema_proofs: Option<&CurrentArtifactInvalidSchemaProofs<'_>>,
    require_local_payloads: bool,
) -> Result<Report, ItemRefusal> {
    let revision = cut.current().revision();
    let membership = cut
        .stream(revision)
        .map_err(|_| {
            ItemRefusal::Source("source-foundation exact cut membership unavailable".into())
        })?
        .expectation();
    if record_report.is_some_and(|report| {
        report.source_revision != revision || report.current_membership != membership
    }) {
        return Err(ItemRefusal::Source(
            "source-foundation Records report differs from the exact source cut".into(),
        ));
    }
    let mut exact_members = cut.current().members();
    for supplied in current_paths {
        let Some(member) = exact_members.next() else {
            return Err(ItemRefusal::Source(
                "source-foundation discovery current paths differ from the exact source cut".into(),
            ));
        };
        if member.path.as_str() != supplied {
            return Err(ItemRefusal::Source(
                "source-foundation discovery current paths differ from the exact source cut".into(),
            ));
        }
    }
    if exact_members.next().is_some() {
        return Err(ItemRefusal::Source(
            "source-foundation discovery current paths differ from the exact source cut".into(),
        ));
    }
    if let Some(proofs) = invalid_schema_proofs {
        let Some(report) = record_report else {
            return Err(ItemRefusal::Source(
                "source-foundation Artifact proof requires the completed Records report".into(),
            ));
        };
        proofs.validate_report_binding(cut, report, limits.deadline, source.cancellation())?;
    }
    inspect_internal(
        source,
        current_paths,
        prior_events,
        limits,
        Some(physical),
        require_local_payloads,
        Some(NativeCutBinding {
            revision,
            membership,
        }),
        Some(native_histories),
        Some(artifact_replays),
        invalid_schema_proofs,
    )
}

fn inspect_internal<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    current_paths: &[String],
    prior_events: &BTreeMap<String, Value>,
    limits: ItemLimits,
    physical: Option<&SourcePhysicalFacts>,
    require_local_payloads: bool,
    native_cut: Option<NativeCutBinding>,
    native_histories: Option<&BTreeMap<String, NativeRecordHistoryReadObservation>>,
    artifact_replays: Option<&ArtifactCorrectionReplayMap<'_>>,
    invalid_current_artifact_schema_proofs: Option<&CurrentArtifactInvalidSchemaProofs<'_>>,
) -> Result<Report, ItemRefusal> {
    let paths = ResidentDiscoveryPaths::new(current_paths);
    let native_history_set =
        native_histories.map_or(NativeHistorySet::Empty, NativeHistorySet::Cut);
    let artifact_replay_set =
        artifact_replays.map_or(ArtifactReplaySet::Empty, ArtifactReplaySet::Cut);
    inspect_kernel::<S, ()>(
        source,
        &paths,
        None,
        None,
        None,
        None,
        native_history_set,
        artifact_replay_set,
        None,
        prior_events,
        limits,
        physical,
        require_local_payloads,
        native_cut,
        invalid_current_artifact_schema_proofs,
        None,
    )
    .map(|output| output.report)
}

/// Inspect one already-completed candidate Records scope through the shared
/// Discovery rules. Current paths are traversed through the bounded provider;
/// point membership comes from that same provider and bytes are read from the
/// exact identity-bearing input. The verified Records coverage is reused as
/// the source fence, never translated into a `SourceRevision`.
#[allow(clippy::too_many_arguments)]
pub fn inspect_candidate_with_artifact_replays_and_records_with_proofs<
    S: LayerFamilySource + ?Sized,
    I: Copy + Eq,
>(
    source: &mut S,
    input: &dyn SourceCutInputWithIdentity<I>,
    coverage: &SourceCutInputCoverage,
    paths: &dyn SourceFoundationDefaultPaths,
    prior_events: &dyn SourceFoundationDefaultEventLookup,
    records_lookup: &dyn SourceFoundationDefaultRecordsLookup,
    records: &SourceFoundationRecordsStreamedReport<'_, I>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
    native_histories: &BTreeMap<
        String,
        crate::native_compound::CandidateNativeRecordHistoryReadObservation<I>,
    >,
    artifact_replays: &CandidateArtifactCorrectionReplayMap<'_, I>,
    invalid_schema_proofs: &CandidateArtifactInvalidSchemaProofs<'_, '_, I>,
    require_local_payloads: bool,
) -> Result<SourceFoundationCandidateDiscoveryReport<I>, ItemRefusal> {
    source.checkpoint(limits.deadline)?;
    let input_identity = *input.input_identity();
    let membership = *records.source_membership();
    if input_identity != *records.input_identity()
        || coverage.membership() != membership
        || invalid_schema_proofs.input_identity() != input_identity
        || invalid_schema_proofs.current_membership() != membership
    {
        return Err(ItemRefusal::Source(
            "source-foundation candidate Discovery input/report binding differs".into(),
        ));
    }
    input.verify_current_fence(coverage, limits.deadline, source.cancellation())?;
    invalid_schema_proofs.validate_report_binding(
        input,
        records,
        limits.deadline,
        source.cancellation(),
    )?;
    for (path, observation) in native_histories {
        if observation.input_identity() != &input_identity
            || observation.current_membership() != membership
            || observation.record_path() != path
            || !paths.contains(path)?
            || input.path_presence(path, limits.deadline, source.cancellation())?
                != Some(SourcePresenceV1::File)
        {
            return Err(ItemRefusal::Source(
                "source-foundation candidate native Artifact history binding differs".into(),
            ));
        }
    }
    for (path, replay) in artifact_replays {
        if replay.input_identity() != &input_identity
            || replay.current_membership() != membership
            || replay.source_path() != path
            || !paths.contains(path)?
            || input.path_presence(path, limits.deadline, source.cancellation())?
                != Some(SourcePresenceV1::File)
        {
            return Err(ItemRefusal::Source(
                "source-foundation candidate Artifact replay binding differs".into(),
            ));
        }
    }
    let path_view = BorrowedDiscoveryPaths(paths);
    let output = inspect_kernel::<S, I>(
        source,
        &path_view,
        Some(input.source_input()),
        Some(records_lookup),
        Some(&input_identity),
        Some(membership),
        NativeHistorySet::Candidate(native_histories),
        ArtifactReplaySet::Candidate(artifact_replays),
        Some(invalid_schema_proofs),
        prior_events,
        limits,
        Some(physical),
        require_local_payloads,
        None,
        None,
        None,
    )?;
    input.verify_current_fence(coverage, limits.deadline, source.cancellation())?;
    Ok(SourceFoundationCandidateDiscoveryReport {
        input_identity,
        source_membership: membership,
        candidate_direct_source_bytes: output.candidate_direct_source_bytes,
        report: output.report,
    })
}

/// Inspect a candidate source input while reconstructing one authentic
/// Artifact history/replay packet at a time through CMD's provider. The held
/// Records path index is consumed in the same Artifact traversal and checked
/// for orphan rows at EOF.
#[allow(clippy::too_many_arguments)]
pub fn inspect_candidate_with_artifact_evidence_provider<
    S: LayerFamilySource + ?Sized,
    I: Copy + Eq,
>(
    source: &mut S,
    input: &dyn SourceCutInputWithIdentity<I>,
    coverage: &SourceCutInputCoverage,
    paths: &dyn SourceFoundationDefaultPaths,
    prior_events: &dyn SourceFoundationDefaultEventLookup,
    records_lookup: &dyn SourceFoundationDefaultRecordsLookup,
    records: &SourceFoundationRecordsStreamedReport<'_, I>,
    limits: ItemLimits,
    physical: &SourcePhysicalFacts,
    evidence_provider: &mut dyn CandidateArtifactEvidenceProvider<I>,
    require_local_payloads: bool,
) -> Result<SourceFoundationCandidateDiscoveryReport<I>, ItemRefusal> {
    source.checkpoint(limits.deadline)?;
    let input_identity = *input.input_identity();
    let membership = *records.source_membership();
    if input_identity != *records.input_identity() || coverage.membership() != membership {
        return Err(ItemRefusal::Source(
            "source-foundation candidate Discovery input/report binding differs".into(),
        ));
    }
    input.verify_current_fence(coverage, limits.deadline, source.cancellation())?;
    evidence_provider.validate_binding(
        input,
        coverage,
        records,
        limits.deadline,
        source.cancellation(),
    )?;
    let path_view = BorrowedDiscoveryPaths(paths);
    let output = inspect_kernel::<S, I>(
        source,
        &path_view,
        Some(input.source_input()),
        Some(records_lookup),
        Some(&input_identity),
        Some(membership),
        NativeHistorySet::Empty,
        ArtifactReplaySet::Empty,
        None,
        prior_events,
        limits,
        Some(physical),
        require_local_payloads,
        None,
        None,
        Some(evidence_provider),
    )?;
    input.verify_current_fence(coverage, limits.deadline, source.cancellation())?;
    Ok(SourceFoundationCandidateDiscoveryReport {
        input_identity,
        source_membership: membership,
        candidate_direct_source_bytes: output.candidate_direct_source_bytes,
        report: output.report,
    })
}

fn inspect_kernel<S: LayerFamilySource + ?Sized, I: Copy + Eq>(
    source: &mut S,
    paths: &dyn DiscoveryCurrentPaths,
    candidate_input: Option<&dyn SourceCutInput>,
    records_lookup: Option<&dyn SourceFoundationDefaultRecordsLookup>,
    candidate_identity: Option<&I>,
    candidate_membership: Option<SourceMembershipV1>,
    native_histories: NativeHistorySet<'_, I>,
    artifact_replays: ArtifactReplaySet<'_, I>,
    candidate_invalid_schema_proofs: Option<&dyn CandidateInvalidArtifactSchemaProof<I>>,
    prior_events: &dyn SourceFoundationDefaultEventLookup,
    limits: ItemLimits,
    physical: Option<&SourcePhysicalFacts>,
    require_local_payloads: bool,
    native_cut: Option<NativeCutBinding>,
    invalid_current_artifact_schema_proofs: Option<&CurrentArtifactInvalidSchemaProofs<'_>>,
    mut candidate_artifact_evidence_provider: Option<&mut dyn CandidateArtifactEvidenceProvider<I>>,
) -> Result<DiscoveryKernelOutput, ItemRefusal> {
    let mut inspector = Inspector {
        source,
        paths,
        candidate_input,
        records_lookup,
        candidate_identity,
        candidate_membership,
        native_histories,
        artifact_replays,
        candidate_invalid_schema_proofs,
        limits,
        physical,
        issues: Vec::new(),
        schema_requests: Vec::new(),
        schema_locations: BTreeSet::new(),
        unsupported: Vec::new(),
        digests: BTreeMap::new(),
        payload_observation_paths: BTreeSet::new(),
        read_bytes: 0,
        candidate_direct_source_bytes: 0,
        candidate_provider_source_bytes: 0,
        payload_bytes: 0,
        state_bytes: 0,
        document_copies: 0,
        native_history_referenced_bytes: 0,
        native_history_referenced_state_bytes: 0,
        artifact_replay_referenced_paths: candidate_artifact_evidence_provider
            .is_none()
            .then(BTreeSet::new),
        artifact_replay_referenced_publication_state_bytes: 0,
        artifact_replay_referenced_state_bytes: 0,
        candidate_artifact_evidence_peak_state_bytes: 0,
    };
    let mut previous_path: Option<String> = None;
    let mut previous_path_state = 0usize;
    inspector.for_each_current_path(&mut |inspector, path| {
        if !safe_relative_path(path) {
            inspector.issue(
                path,
                "unsafe-current-member-path",
                "captured member is not a safe relative path",
            )?;
        }
        if previous_path.as_deref() == Some(path) {
            inspector.issue(
                path,
                "duplicate-current-member-path",
                "exact current membership contains a duplicate path",
            )?;
        }
        let next_state = path.len().checked_add(24).ok_or(ItemRefusal::Budget)?;
        if next_state > previous_path_state {
            inspector.reserve_state(next_state - previous_path_state)?;
            previous_path_state = next_state;
        }
        previous_path.get_or_insert_with(String::new).clear();
        previous_path.as_mut().unwrap().push_str(path);
        Ok(())
    })?;

    let mut prior_event_cost = 0usize;
    let mut event_ids: BTreeSet<String> = BTreeSet::new();
    prior_events.for_each_event(&mut |id, _| {
        prior_event_cost = prior_event_cost
            .checked_add(id.len().checked_add(64).ok_or(ItemRefusal::Budget)?)
            .ok_or(ItemRefusal::Budget)?;
        event_ids.insert(id.to_owned());
        Ok(())
    })?;
    inspector.reserve_state(prior_event_cost)?;
    let mut source_event_insertions: Vec<(String, Value)> = Vec::new();
    let mut boundary_events: BTreeMap<String, EventInfo> = BTreeMap::new();

    // The maintained Python route validates this earlier private/handoff and
    // target-map district before its later discovery and artifact pass.
    inspect_companion_district(&mut inspector)?;

    for path in [ACCESS_EVENTS, SERVER_EVENTS] {
        for (location, value) in inspector.jsonl(path, PROVENANCE_SCHEMA)? {
            let info = inspector.event_info(&value, &location)?;
            let Some(id) = string(&value, "event_id") else {
                inspector.issue(
                    &location,
                    "missing-event-id",
                    "boundary provenance event has no event_id",
                )?;
                continue;
            };
            let id = id.to_owned();
            if !event_ids.insert(id.clone()) {
                inspector.issue(&location, "duplicate-event-id", id.as_str())?;
            }
            boundary_events.insert(id.clone(), info);
            source_event_insertions.push((id, value));
        }
    }

    let mut discoveries: BTreeMap<String, DiscoveryInfo> = BTreeMap::new();
    let discovery_paths = inspector.collect_current_paths_matching(|path| {
        path.strip_prefix(DISCOVERY_RUNS)
            .is_some_and(|tail| !tail.contains('/') && tail.ends_with(".json"))
    })?;
    for path in &discovery_paths {
        let path = path.as_str();
        let Some((value, _, _)) = inspector.json(path, path, DISCOVERY_SCHEMA)? else {
            continue;
        };
        inspector.source_refs(&value, path)?;
        for message in discovery_semantic_issues(&value) {
            inspector.issue(path, "discovery-semantic", message)?;
        }
        let target_kind = value
            .pointer("/target/target_kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let known_refs = string_set(
            value.pointer("/target").unwrap_or(&Value::Null),
            "known_tos_refs",
        );
        let mut captured_acquisitions = BTreeSet::new();
        for channel in array(&value, "channels") {
            for result in array(channel, "results") {
                inspector.checkpoint()?;
                let snapshot = result.get("snapshot").unwrap_or(&Value::Null);
                let acquisition = result.get("acquisition").unwrap_or(&Value::Null);
                if string(snapshot, "state") != Some("captured")
                    || acquisition.get("downloaded") != Some(&Value::Bool(true))
                {
                    continue;
                }
                let (
                    Some(surface),
                    Some(snapshot_sha256),
                    Some(acquisition_sha256),
                    Some(byte_size),
                ) = (
                    string(result, "result_url"),
                    string(snapshot, "sha256"),
                    string(acquisition, "sha256"),
                    acquisition.get("byte_size").and_then(Value::as_u64),
                )
                else {
                    continue;
                };
                inspector.reserve_state(
                    surface
                        .len()
                        .checked_add(snapshot_sha256.len())
                        .and_then(|used| used.checked_add(acquisition_sha256.len()))
                        .and_then(|used| used.checked_add(96))
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                captured_acquisitions.insert((
                    surface.to_owned(),
                    snapshot_sha256.to_owned(),
                    acquisition_sha256.to_owned(),
                    byte_size,
                ));
            }
        }
        discoveries.insert(
            path.to_owned(),
            DiscoveryInfo {
                target_kind,
                known_refs,
                captured_acquisitions,
            },
        );
    }

    let mut discovery_events: BTreeMap<String, EventInfo> = BTreeMap::new();
    let mut discovery_event_ids = BTreeSet::new();
    for (location, value) in inspector.jsonl(DISCOVERY_EVENTS, PROVENANCE_SCHEMA)? {
        let info = inspector.event_info(&value, &location)?;
        let Some(id) = string(&value, "event_id") else {
            inspector.issue(
                &location,
                "missing-event-id",
                "discovery provenance event has no event_id",
            )?;
            continue;
        };
        let id = id.to_owned();
        if !discovery_event_ids.insert(id.clone()) {
            inspector.issue(&location, "duplicate-discovery-event-id", id.as_str())?;
        }
        if !event_ids.insert(id.clone()) {
            inspector.issue(&location, "duplicate-event-id", id.as_str())?;
        } else {
            source_event_insertions.push((id.clone(), value));
        }
        discovery_events.insert(id, info);
    }

    let mut artifact_ids = BTreeSet::new();
    let mut artifacts_by_path: BTreeMap<String, Value> = BTreeMap::new();
    let artifact_paths = inspector.collect_current_paths_matching(|path| {
        path.starts_with(ARTIFACTS) && path.ends_with("/artifact-witness.json")
    })?;
    for path in &artifact_paths {
        let path = path.as_str();
        inspector.checkpoint()?;
        let indexed_artifact_record =
            if let Some(provider) = candidate_artifact_evidence_provider.as_deref_mut() {
                let remaining_state = inspector
                    .limits
                    .max_state_bytes
                    .checked_sub(inspector.state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                let summary = provider.begin_artifact_path(
                    path,
                    remaining_state,
                    inspector.limits.deadline,
                    inspector.source.cancellation(),
                )?;
                if let Some(summary) = summary {
                    inspector.check_temporary_state(summary.charged_state_bytes)?;
                    inspector.candidate_artifact_evidence_peak_state_bytes = inspector
                        .candidate_artifact_evidence_peak_state_bytes
                        .max(summary.charged_state_bytes);
                }
                summary
            } else {
                None
            };
        let Some(raw) = inspector.current_bytes(path)? else {
            if let Some(provider) = candidate_artifact_evidence_provider.as_deref_mut() {
                let remaining_state = inspector
                    .limits
                    .max_state_bytes
                    .checked_sub(inspector.state_bytes)
                    .and_then(|state| {
                        state.checked_sub(
                            indexed_artifact_record
                                .map_or(0, |summary| summary.charged_state_bytes),
                        )
                    })
                    .ok_or(ItemRefusal::Budget)?;
                provider.abandon_artifact_path(path, remaining_state)?;
            }
            continue;
        };
        let value = match serde_json::from_slice::<Value>(&raw) {
            Ok(value) => value,
            Err(_) => {
                inspector.issue(
                    path,
                    "invalid-json",
                    "current source document is invalid JSON",
                )?;
                if let Some(provider) = candidate_artifact_evidence_provider.as_deref_mut() {
                    let remaining_state = inspector
                        .limits
                        .max_state_bytes
                        .checked_sub(inspector.state_bytes)
                        .and_then(|state| {
                            state.checked_sub(
                                indexed_artifact_record
                                    .map_or(0, |summary| summary.charged_state_bytes),
                            )
                        })
                        .ok_or(ItemRefusal::Budget)?;
                    provider.abandon_artifact_path(path, remaining_state)?;
                }
                continue;
            }
        };
        if let Some(records) = inspector.records_lookup {
            match records.record_by_path(path)? {
                Some(record) if record.path == path && python_json_equal(&record.value, &value) => {
                }
                Some(_) => inspector.issue(
                    path,
                    "candidate-discovery-current-record-drift",
                    "stored Records lookup differs from the exact candidate Artifact member",
                )?,
                None => inspector.issue(
                    path,
                    "candidate-discovery-current-record-unindexed",
                    "candidate Artifact member is absent from the completed Records lookup",
                )?,
            }
        }
        let v2 = string(&value, "$schema") == Some(V2_ARTIFACT_SCHEMA);
        inspector.request_schema(
            path,
            if v2 {
                ARTIFACT_V2_SCHEMA
            } else {
                ARTIFACT_SCHEMA
            },
            &value,
            raw.len(),
        )?;
        inspector.source_refs(&value, path)?;
        if let Some(fields) = first_forbidden_content_fields(&value) {
            inspector.issue(
                path,
                "metadata-content-exposure",
                format!(
                    "artifact metadata packet exposes content fields: {}",
                    python_repr_string_list(fields.iter().map(String::as_str))
                ),
            )?;
        } else if local_only_absolute(&value) {
            inspector.issue(
                path,
                "metadata-content-exposure",
                "artifact metadata packet exposes an absolute owner-local path",
            )?;
        }
        let id = string(&value, "artifact_id").unwrap_or("");
        if !id.is_empty() && !artifact_ids.insert(id.to_owned()) {
            inspector.issue(path, "duplicate-artifact-id", id)?;
        }
        let relative = path.strip_prefix(ARTIFACTS).unwrap_or(path);
        if path_parts(relative, "")
            .iter()
            .take(path_parts(relative, "").len().saturating_sub(1))
            .any(|part| part.eq_ignore_ascii_case("cdli"))
        {
            inspector.issue(
                path,
                "provider-keyed-artifact-path",
                "artifact path must not be keyed to mutable CDLI provider",
            )?;
        }
        let rights_ref = string(&value, "rights_ref").unwrap_or("");
        let discovery_ref = string(&value, "discovery_ref").unwrap_or("");
        let research_ref = string(&value, "research_ref").unwrap_or("");
        for (field, reference) in [
            ("rights_ref", rights_ref),
            ("discovery_ref", discovery_ref),
            ("research_ref", research_ref),
        ] {
            if !inspector.exists_source_ref(reference)? {
                inspector.issue(
                    path,
                    "unresolved-witness-reference",
                    format!("{field} does not resolve to a current file: {reference}"),
                )?;
            }
        }
        let planting_refs: Vec<&str> = array(&value, "philosophy_planting_refs")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for (index, reference) in planting_refs.iter().enumerate() {
            if !inspector.exists_source_ref(reference)? {
                inspector.issue(path, "unresolved-witness-reference", format!("philosophy_planting_refs[{index}] does not resolve to a current file: {reference}"))?;
            }
        }
        let _ = inspector.check_rights(
            path,
            rights_ref,
            &[id],
            "public_metadata_only",
            &["metadata_only"],
        )?;
        match discoveries.get(discovery_ref) {
            Some(discovery) => {
                if !discovery.known_refs.contains(id) {
                    inspector.issue(
                        path,
                        "discovery-target-omits-owner",
                        "artifact discovery target does not include artifact_id",
                    )?;
                }
                if discovery.target_kind != "artifact" {
                    inspector.issue(
                        path,
                        "discovery-target-kind-drift",
                        "artifact discovery target_kind must be artifact",
                    )?;
                }
            }
            None => inspector.issue(
                path,
                "unresolved-discovery-run",
                "artifact discovery_ref does not resolve to a validated discovery run",
            )?,
        }
        let event_ref = string(&value, "provenance_event_ref").unwrap_or("");
        let candidate_evidence_response =
            if let Some(provider) = candidate_artifact_evidence_provider.as_deref_mut() {
                let consumed_source = inspector
                    .read_bytes
                    .checked_add(inspector.candidate_provider_source_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                let remaining_source = inspector
                    .limits
                    .max_total_bytes
                    .checked_sub(consumed_source)
                    .ok_or(ItemRefusal::Budget)?;
                let indexed_record_state =
                    indexed_artifact_record.map_or(0, |summary| summary.charged_state_bytes);
                let remaining_state = inspector
                    .limits
                    .max_state_bytes
                    .checked_sub(inspector.state_bytes)
                    .and_then(|state| state.checked_sub(indexed_record_state))
                    .ok_or(ItemRefusal::Budget)?;
                Some(provider.evidence_for_artifact(
                    path,
                    indexed_artifact_record,
                    &value,
                    Digest256::of_bytes(&raw),
                    u64::try_from(raw.len()).map_err(|_| ItemRefusal::Budget)?,
                    remaining_source,
                    remaining_state,
                    inspector.limits.deadline,
                    inspector.source.cancellation(),
                )?)
            } else {
                None
            };
        let candidate_schema_invalid = candidate_evidence_response
            .as_ref()
            .and_then(|response| response.candidate_schema_invalid());
        if let Some(evidence) = candidate_evidence_response
            .as_ref()
            .and_then(|response| response.evidence())
        {
            inspector.check_temporary_state(
                indexed_artifact_record
                    .map_or(0, |summary| summary.charged_state_bytes)
                    .checked_add(evidence.peak_state_bytes())
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            inspector.candidate_artifact_evidence_peak_state_bytes =
                inspector.candidate_artifact_evidence_peak_state_bytes.max(
                    indexed_artifact_record
                        .map_or(0, |summary| summary.charged_state_bytes)
                        .checked_add(evidence.peak_state_bytes())
                        .ok_or(ItemRefusal::Budget)?,
                );
            inspector.candidate_provider_source_bytes = inspector
                .candidate_provider_source_bytes
                .checked_add(evidence.direct_source_read_bytes())
                .filter(|provider_used| {
                    inspector
                        .read_bytes
                        .checked_add(*provider_used)
                        .is_some_and(|total| total <= inspector.limits.max_total_bytes)
                })
                .ok_or(ItemRefusal::Budget)?;
            inspector.candidate_direct_source_bytes = inspector
                .candidate_direct_source_bytes
                .checked_add(evidence.direct_source_read_bytes())
                .ok_or(ItemRefusal::Budget)?;
        }
        let (native_history, artifact_replay) = match candidate_evidence_response
            .as_ref()
            .and_then(|response| response.evidence())
        {
            Some(evidence) => (
                Some(NativeHistoryRef::Candidate(evidence.history())),
                evidence.replay().map(ArtifactReplayRef::Candidate),
            ),
            None => (
                inspector.native_histories.get(path),
                inspector.artifact_replays.get(path),
            ),
        };
        let artifact_binding = native_cut.map(NativeArtifactBinding::Cut).or_else(|| {
            candidate_identity
                .zip(candidate_membership)
                .map(|(identity, membership)| NativeArtifactBinding::Candidate {
                    identity,
                    membership,
                })
        });
        let native_capture = native_artifact_capture(
            &mut inspector,
            path,
            &value,
            u64::try_from(raw.len()).map_err(|_| ItemRefusal::Budget)?,
            &discoveries,
            artifact_binding,
            native_history,
            artifact_replay,
            invalid_current_artifact_schema_proofs,
            candidate_schema_invalid,
        )?;
        if native_capture == NativeArtifactCapture::Complete {
            if !event_ref.is_empty() && !event_ids.insert(event_ref.to_owned()) {
                inspector.issue(path, "duplicate-native-artifact-event-id", event_ref)?;
            }
        } else if native_capture == NativeArtifactCapture::Legacy {
            if let Some(event) = discovery_events.get(event_ref).cloned() {
                let mut required = BTreeSet::from([
                    path.to_owned(),
                    rights_ref.to_owned(),
                    discovery_ref.to_owned(),
                    research_ref.to_owned(),
                ]);
                required.extend(
                    planting_refs
                        .iter()
                        .map(|reference| (*reference).to_owned()),
                );
                let absent: Vec<String> = required
                    .iter()
                    .filter(|reference| !event.outputs.contains_key(*reference))
                    .cloned()
                    .collect();
                if !absent.is_empty() {
                    inspector.issue(
                        &event.location,
                        "artifact-provenance-output-closure",
                        format!(
                            "artifact planting provenance lacks exact output closure: {}",
                            python_repr_string_list(absent.iter().map(String::as_str))
                        ),
                    )?;
                }
            } else {
                inspector.issue(
                    path,
                    "unresolved-discovery-provenance-event",
                    "artifact provenance_event_ref is absent from discovery provenance",
                )?;
            }
        }
        artifacts_by_path.insert(path.to_owned(), value);
    }

    if let Some(provider) = candidate_artifact_evidence_provider.as_deref_mut() {
        provider.finish(limits.deadline, inspector.source.cancellation())?;
    }

    let artifact_replays = inspector.artifact_replays;
    artifact_replays.for_each_path(&mut |path| {
        inspector.checkpoint()?;
        if inspector
            .artifact_replay_referenced_paths
            .as_ref()
            .is_some_and(|paths| paths.contains(path))
        {
            return Ok(());
        }
        let location = if safe_relative_path(path) && path.starts_with(ARTIFACTS) {
            path
        } else {
            ARTIFACTS.trim_end_matches('/')
        };
        inspector.issue(
            location,
            "unresolved-artifact-correction-replay-evidence",
            "CMD correction replay evidence does not resolve to an exact current native Artifact history",
        )
    })?;

    let mut representation_file_ids = BTreeSet::new();
    let artifact_representation_paths = inspector.collect_current_paths_matching(|path| {
        path.starts_with(ARTIFACTS) && path.ends_with("/representation.json")
    })?;
    for path in &artifact_representation_paths {
        let path = path.as_str();
        let Some((value, _, _)) = inspector.json(path, path, ARTIFACT_REPRESENTATION_SCHEMA)?
        else {
            continue;
        };
        inspector.source_refs(&value, path)?;
        let file_id = string(&value, "file_id").unwrap_or("");
        if !file_id.is_empty() && !representation_file_ids.insert(file_id.to_owned()) {
            inspector.issue(path, "duplicate-representation-file-id", file_id)?;
        }
        let artifact_id = string(&value, "artifact_id").unwrap_or("");
        let artifact_ref = string(&value, "artifact_ref").unwrap_or("");
        if !artifact_ids.contains(artifact_id) {
            inspector.issue(path, "unresolved-represented-artifact", artifact_id)?;
        }
        match artifacts_by_path.get(artifact_ref) {
            Some(artifact) if string(artifact, "artifact_id") == Some(artifact_id) => {}
            _ => inspector.issue(
                path,
                "artifact-reference-id-drift",
                "artifact_ref does not resolve the represented artifact_id",
            )?,
        }

        let payload = value.get("payload").unwrap_or(&Value::Null);
        let relative = string(payload, "relative_path").unwrap_or("");
        let parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        let payload_path = format!("{parent}/{relative}");
        let safe_payload = !relative.is_empty() && safe_relative_path(&payload_path);
        if !safe_payload {
            inspector.issue(path, "unsafe-artifact-payload-path", relative)?;
        }
        let expected_file_id = format!(
            "tos.file.sha256.{}",
            string(payload, "sha256").unwrap_or("")
        );
        if file_id != expected_file_id {
            inspector.issue(
                path,
                "representation-file-id-not-content-addressed",
                file_id,
            )?;
        }
        let rights_ref = string(&value, "rights_ref").unwrap_or("");
        let _ = inspector.check_rights(
            path,
            rights_ref,
            &[artifact_id, file_id],
            "public_payload",
            PUBLIC_REPRESENTATION_POSTURES,
        )?;
        let discovery_ref = string(&value, "discovery_ref").unwrap_or("");
        match discoveries.get(discovery_ref) {
            Some(discovery) if discovery.known_refs.contains(artifact_id) => {}
            Some(_) => inspector.issue(
                path,
                "representation-discovery-target-omits-artifact",
                discovery_ref,
            )?,
            None => inspector.issue(path, "unresolved-representation-discovery", discovery_ref)?,
        }

        if safe_payload {
            if let Some(facts) = inspector.checked_payload_facts(&payload_path, path)? {
                if !facts.exists || !facts.regular_file || facts.symlink {
                    inspector.issue(
                        path,
                        "public-artifact-payload-missing-or-nonregular",
                        &payload_path,
                    )?;
                } else {
                    inspector.public_payload_git(&payload_path, path)?;
                    match facts.byte_size {
                        Some(actual)
                            if payload.get("byte_size").is_some_and(|expected| {
                                python_json_equal(&json!(actual), expected)
                            }) => {}
                        Some(_) => inspector.issue(
                            path,
                            "artifact-representation-size-drift",
                            &payload_path,
                        )?,
                        None => inspector.unsupported(
                            &payload_path,
                            "public artifact payload size is unobserved",
                        )?,
                    }
                    match facts.sha256.as_deref() {
                        Some(actual)
                            if payload.get("sha256").is_some_and(|expected| {
                                python_json_equal(&Value::String(actual.to_owned()), expected)
                            }) => {}
                        Some(_) => inspector.issue(
                            path,
                            "artifact-representation-sha256-drift",
                            &payload_path,
                        )?,
                        None => inspector.unsupported(
                            &payload_path,
                            "public artifact payload SHA-256 is unobserved",
                        )?,
                    }
                    match facts.sha1.as_deref() {
                        Some(actual)
                            if payload.get("source_sha1").is_some_and(|expected| {
                                python_json_equal(&Value::String(actual.to_owned()), expected)
                            }) => {}
                        Some(_) => inspector.issue(
                            path,
                            "artifact-representation-sha1-drift",
                            &payload_path,
                        )?,
                        None => inspector.unsupported(
                            &payload_path,
                            "public artifact payload SHA-1 is unobserved",
                        )?,
                    }
                    match facts.jpeg_dimensions {
                        Some((width, height))
                            if python_optional_json_equal(
                                payload.get("width_pixels"),
                                Some(&json!(width)),
                            ) && python_optional_json_equal(
                                payload.get("height_pixels"),
                                Some(&json!(height)),
                            ) => {}
                        Some(_) => inspector.issue(
                            path,
                            "artifact-representation-jpeg-dimensions-drift",
                            &payload_path,
                        )?,
                        None => inspector.unsupported(
                            &payload_path,
                            "public artifact JPEG dimensions are unobserved",
                        )?,
                    }
                }
            }
        }
        let event_ref = string(&value, "provenance_event_ref").unwrap_or("");
        match discovery_events.get(event_ref).cloned() {
            Some(event) => {
                let mut required = BTreeSet::from([path.to_owned(), rights_ref.to_owned()]);
                if safe_payload {
                    required.insert(payload_path.clone());
                }
                let absent: Vec<String> = required
                    .iter()
                    .filter(|reference| !event.outputs.contains_key(*reference))
                    .cloned()
                    .collect();
                if !absent.is_empty() {
                    inspector.issue(
                        &event.location,
                        "artifact-representation-output-closure",
                        format!(
                            "artifact representation provenance lacks output closure: {}",
                            python_repr_string_list(absent.iter().map(String::as_str))
                        ),
                    )?;
                }
                inspector.check_required_output_digests(&event, &required, &event.location)?;
            }
            None => inspector.issue(
                path,
                "unresolved-representation-provenance-event",
                event_ref,
            )?,
        }
    }

    let mut composite_ids = BTreeSet::new();
    let mut composites_by_path: BTreeMap<String, Value> = BTreeMap::new();
    let composite_paths = inspector.collect_current_paths_matching(|path| {
        path.starts_with(COMPOSITES) && path.ends_with("/composite-witness.json")
    })?;
    for path in &composite_paths {
        let path = path.as_str();
        let Some((value, _, _)) = inspector.json(path, path, COMPOSITE_SCHEMA)? else {
            continue;
        };
        inspector.source_refs(&value, path)?;
        if let Some(fields) = first_forbidden_content_fields(&value) {
            inspector.issue(
                path,
                "metadata-content-exposure",
                format!(
                    "scholarly-composite packet exposes content fields: {}",
                    python_repr_string_list(fields.iter().map(String::as_str))
                ),
            )?;
        } else if local_only_absolute(&value) {
            inspector.issue(
                path,
                "metadata-content-exposure",
                "scholarly-composite packet exposes an absolute owner-local path",
            )?;
        }
        let id = string(&value, "composite_id").unwrap_or("");
        if !id.is_empty() && !composite_ids.insert(id.to_owned()) {
            inspector.issue(path, "duplicate-composite-id", id)?;
        }
        let relative = path.strip_prefix(COMPOSITES).unwrap_or(path);
        let provider_parts = ["cdli", "dcclt", "oracc"];
        let parts = path_parts(relative, "");
        if parts
            .iter()
            .take(parts.len().saturating_sub(1))
            .any(|part| {
                provider_parts
                    .iter()
                    .any(|provider| part.eq_ignore_ascii_case(provider))
            })
        {
            inspector.issue(
                path,
                "provider-keyed-composite-path",
                "scholarly-composite path must not be keyed to a mutable provider",
            )?;
        }
        let rights_ref = string(&value, "rights_ref").unwrap_or("");
        let discovery_ref = string(&value, "discovery_ref").unwrap_or("");
        let research_ref = string(&value, "research_ref").unwrap_or("");
        for (field, reference) in [
            ("rights_ref", rights_ref),
            ("discovery_ref", discovery_ref),
            ("research_ref", research_ref),
        ] {
            if !inspector.exists_source_ref(reference)? {
                inspector.issue(
                    path,
                    "unresolved-witness-reference",
                    format!("{field} does not resolve to a current file: {reference}"),
                )?;
            }
        }
        let planting_refs: Vec<&str> = array(&value, "philosophy_planting_refs")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for (index, reference) in planting_refs.iter().enumerate() {
            if !inspector.exists_source_ref(reference)? {
                inspector.issue(path, "unresolved-witness-reference", format!("philosophy_planting_refs[{index}] does not resolve to a current file: {reference}"))?;
            }
        }
        let _ = inspector.check_rights(
            path,
            rights_ref,
            &[id],
            "public_metadata_only",
            &["metadata_only"],
        )?;
        match discoveries.get(discovery_ref) {
            Some(discovery) => {
                if !discovery.known_refs.contains(id) {
                    inspector.issue(
                        path,
                        "discovery-target-omits-owner",
                        "scholarly-composite discovery target does not include composite_id",
                    )?;
                }
                if discovery.target_kind != "scholarly-composite" {
                    inspector.issue(
                        path,
                        "discovery-target-kind-drift",
                        "scholarly-composite discovery target_kind must be scholarly-composite",
                    )?;
                }
            }
            None => inspector.issue(
                path,
                "unresolved-discovery-run",
                "scholarly-composite discovery_ref does not resolve to a validated discovery run",
            )?,
        }
        for member in rows(&value, "member_observations") {
            let member_id = string(member, "member_artifact_id").unwrap_or("");
            if !artifact_ids.contains(member_id) {
                inspector.issue(path, "unresolved-composite-member-artifact", member_id)?;
            }
        }
        let mut coverage = BTreeSet::new();
        for observation in rows(&value, "coverage_observations") {
            let key = (
                python_string_value(observation.get("provider")),
                python_string_value(observation.get("surface")),
            );
            if !coverage.insert(key.clone()) {
                inspector.issue(
                    path,
                    "duplicate-composite-coverage",
                    format!(
                        "duplicate composite coverage observation: {}",
                        python_repr_string_pair(&key.0, &key.1)
                    ),
                )?;
            }
        }
        let event_ref = string(&value, "provenance_event_ref").unwrap_or("");
        match discovery_events.get(event_ref).cloned() {
            Some(event) => {
                let mut required = BTreeSet::from([
                    path.to_owned(),
                    rights_ref.to_owned(),
                    discovery_ref.to_owned(),
                    research_ref.to_owned(),
                ]);
                required.extend(
                    planting_refs
                        .iter()
                        .map(|reference| (*reference).to_owned()),
                );
                let absent: Vec<String> = required
                    .iter()
                    .filter(|reference| !event.outputs.contains_key(*reference))
                    .cloned()
                    .collect();
                if !absent.is_empty() {
                    inspector.issue(
                        &event.location,
                        "composite-provenance-output-closure",
                        format!(
                            "scholarly-composite planting provenance lacks exact output closure: {}",
                            python_repr_string_list(absent.iter().map(String::as_str))
                        ),
                    )?;
                }
            }
            None => inspector.issue(
                path,
                "unresolved-discovery-provenance-event",
                "scholarly-composite provenance_event_ref is absent from discovery provenance",
            )?,
        }
        composites_by_path.insert(path.to_owned(), value);
    }

    let mut composite_representation_ids = BTreeSet::new();
    let composite_representation_paths = inspector.collect_current_paths_matching(|path| {
        if !path.starts_with(COMPOSITES) || !path.ends_with("/representation.json") {
            return false;
        }
        let parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        let Some(relative) = parent.strip_prefix(COMPOSITES) else {
            return false;
        };
        let parts: Vec<&str> = relative.split('/').collect();
        parts.len() >= 2
            && parts[parts.len() - 2] == "representations"
            && !parts[parts.len() - 1].is_empty()
    })?;
    for path in &composite_representation_paths {
        let path = path.as_str();
        let Some((value, _, _)) = inspector.json(path, path, COMPOSITE_REPRESENTATION_SCHEMA)?
        else {
            continue;
        };
        inspector.source_refs(&value, path)?;
        let representation_id = string(&value, "representation_id").unwrap_or("");
        if !representation_id.is_empty()
            && !composite_representation_ids.insert(representation_id.to_owned())
        {
            inspector.issue(
                path,
                "duplicate-composite-representation-id",
                representation_id,
            )?;
        }
        let file_id = string(&value, "file_id").unwrap_or("");
        if !file_id.is_empty() && !representation_file_ids.insert(file_id.to_owned()) {
            inspector.issue(path, "duplicate-representation-file-id", file_id)?;
        }
        let composite_id = string(&value, "composite_id").unwrap_or("");
        let composite_ref = string(&value, "composite_ref").unwrap_or("");
        if !composite_ids.contains(composite_id) {
            inspector.issue(path, "unresolved-represented-composite", composite_id)?;
        }
        match composites_by_path.get(composite_ref) {
            Some(composite) if string(composite, "composite_id") == Some(composite_id) => {}
            _ => inspector.issue(
                path,
                "composite-reference-id-drift",
                "composite_ref does not resolve the represented composite_id",
            )?,
        }

        let payload = value.get("payload").unwrap_or(&Value::Null);
        let relative = string(payload, "relative_path").unwrap_or("");
        let payload_name = relative.strip_prefix("payload/").unwrap_or("");
        let safe_name = !payload_name.is_empty()
            && !payload_name.contains('/')
            && !payload_name.contains('\\')
            && !payload_name.contains('\0')
            && payload_name != "."
            && payload_name != "..";
        let payload_path = format!(
            "{}/{}",
            path.rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or(""),
            relative
        );
        if !safe_name || !safe_relative_path(&payload_path) {
            inspector.issue(path, "composite-payload-not-direct-owned-file", relative)?;
            continue;
        }
        if safe_name && safe_relative_path(&payload_path) {
            if let Some(facts) = inspector.checked_payload_facts(&payload_path, path)? {
                if facts.symlink || (facts.exists && !facts.regular_file) {
                    inspector.issue(
                        path,
                        "composite-payload-nonregular-or-symlink",
                        &payload_path,
                    )?;
                    continue;
                }
                if facts.exists {
                    if string(payload, "materialization_status") != Some("materialized")
                        || string(payload, "storage_posture") != Some("tracked_repository_payload")
                        || payload.get("git_tracked").and_then(Value::as_bool) != Some(true)
                    {
                        inspector.issue(
                            path,
                            "composite-payload-declared-posture-drift",
                            &payload_path,
                        )?;
                    }
                    inspector.tracked_composite_payload_git(&payload_path, path)?;
                    let expected_file_id = format!(
                        "tos.file.sha256.{}",
                        string(payload, "sha256").unwrap_or("")
                    );
                    if file_id != expected_file_id {
                        inspector.issue(
                            path,
                            "representation-file-id-not-content-addressed",
                            file_id,
                        )?;
                    }
                    match facts.byte_size {
                        Some(actual)
                            if payload.get("byte_size").is_some_and(|expected| {
                                python_json_equal(&json!(actual), expected)
                            }) => {}
                        Some(_) => inspector.issue(
                            path,
                            "composite-representation-size-drift",
                            &payload_path,
                        )?,
                        None => inspector.unsupported(
                            &payload_path,
                            "local composite payload size is unobserved",
                        )?,
                    }
                    match facts.sha256.as_deref() {
                        Some(actual)
                            if payload.get("sha256").is_some_and(|expected| {
                                python_json_equal(&Value::String(actual.to_owned()), expected)
                            }) => {}
                        Some(_) => inspector.issue(
                            path,
                            "composite-representation-sha256-drift",
                            &payload_path,
                        )?,
                        None => inspector.unsupported(
                            &payload_path,
                            "local composite payload SHA-256 is unobserved",
                        )?,
                    }
                } else {
                    if string(payload, "materialization_status") != Some("not_materialized")
                        || string(payload, "storage_posture") != Some("unmaterialized_payload")
                        || payload.get("git_tracked").and_then(Value::as_bool) != Some(false)
                    {
                        inspector.issue(
                            path,
                            "composite-payload-declared-posture-drift",
                            &payload_path,
                        )?;
                    }
                    match require_local_payloads {
                        true => inspector.issue(
                            path,
                            "composite-payload-required-but-missing",
                            "scholarly-composite representation payload is missing",
                        )?,
                        false => {}
                    }
                }
            }
        }

        let rights_ref = string(&value, "rights_ref").unwrap_or("");
        let _ = inspector.check_rights(
            path,
            rights_ref,
            &[composite_id, representation_id, file_id],
            "local_only",
            LOCAL_REPRESENTATION_POSTURES,
        )?;
        let discovery_ref = string(&value, "discovery_ref").unwrap_or("");
        match discoveries.get(discovery_ref) {
            Some(discovery) if discovery.known_refs.contains(composite_id) => {}
            Some(_) => inspector.issue(
                path,
                "representation-discovery-target-omits-composite",
                discovery_ref,
            )?,
            None => inspector.issue(path, "unresolved-representation-discovery", discovery_ref)?,
        }

        if inspector.physical.and_then(|facts| facts.git_available) == Some(true) {
            inspector.metadata_git(path, path)?;
            inspector.metadata_git(DISCOVERY_EVENTS, path)?;
            if !rights_ref.is_empty() {
                inspector.metadata_git(rights_ref, path)?;
            }
            if !discovery_ref.is_empty() {
                inspector.metadata_git(discovery_ref, path)?;
            }
        } else if inspector
            .physical
            .and_then(|facts| facts.git_available)
            .is_none()
        {
            inspector.unsupported(
                path,
                "repository Git availability is unobserved for composite metadata posture",
            )?;
        }

        let event_ref = string(&value, "provenance_event_ref").unwrap_or("");
        match discovery_events.get(event_ref).cloned() {
            Some(event) => {
                let mut required =
                    BTreeSet::from([path.to_owned(), rights_ref.to_owned(), payload_path.clone()]);
                let absent: Vec<String> = required
                    .iter()
                    .filter(|reference| !event.outputs.contains_key(*reference))
                    .cloned()
                    .collect();
                if !absent.is_empty() {
                    inspector.issue(
                        &event.location,
                        "composite-representation-output-closure",
                        format!(
                            "composite representation provenance lacks output closure: {}",
                            python_repr_string_list(absent.iter().map(String::as_str))
                        ),
                    )?;
                }
                if inspector
                    .physical
                    .and_then(|facts| facts.payloads.get(&payload_path))
                    .is_some_and(|facts| facts.exists)
                {
                    inspector.check_required_output_digests(&event, &required, &event.location)?;
                } else {
                    required.remove(&payload_path);
                    inspector.check_required_output_digests(&event, &required, &event.location)?;
                }
            }
            None => inspector.issue(
                path,
                "unresolved-representation-provenance-event",
                event_ref,
            )?,
        }
    }

    let access_request_paths = inspector.collect_current_paths_matching(|path| {
        path.strip_prefix(ACCESS_LEDGER)
            .is_some_and(|tail| !tail.contains('/') && tail.ends_with(".json"))
    })?;
    for path in &access_request_paths {
        let path = path.as_str();
        let Some((value, _, _)) = inspector.json(path, path, ACCESS_REQUEST_SCHEMA)? else {
            continue;
        };
        inspector.source_refs(&value, path)?;
        for event_ref in array(&value, "provenance_event_refs")
            .iter()
            .filter_map(Value::as_str)
        {
            if !event_ids.contains(event_ref) {
                inspector.issue(path, "unresolved-access-request-event", event_ref)?;
            }
        }
    }
    inspector.private_route()?;

    let mut expected_manifest_refs = BTreeSet::new();
    inspector.for_each_current_path(&mut |inspector, path| {
        if path.starts_with(SOURCE_HOME) && path.ends_with(ITEM_MANIFEST_SUFFIX) {
            inspector.reserve_state(path.len().checked_add(96).ok_or(ItemRefusal::Budget)?)?;
            expected_manifest_refs.insert(path.to_owned());
        }
        Ok(())
    })?;
    let mut planned_manifest_refs = BTreeSet::new();
    let server_plan_paths = inspector.collect_current_paths_matching(|path| {
        path.strip_prefix(SERVER_PLANS)
            .is_some_and(|tail| !tail.contains('/') && tail.ends_with(".json"))
    })?;
    for path in &server_plan_paths {
        let path = path.as_str();
        let Some((plan, plan_digest, _)) = inspector.json(path, path, SERVER_PLAN_SCHEMA)? else {
            continue;
        };
        inspector.source_refs(&plan, path)?;
        let manifest_evidence = plan.get("manifest").unwrap_or(&Value::Null);
        let manifest_ref = string(manifest_evidence, "ref").unwrap_or("");
        if !manifest_ref.is_empty() {
            planned_manifest_refs.insert(manifest_ref.to_owned());
            match inspector.referenced_json(
                manifest_ref,
                path,
                "ToS/contracts/source-item-manifest.schema.json",
            )? {
                Some(manifest) => {
                    let actual_digest = inspector.digest(manifest_ref)?;
                    if actual_digest.as_deref() != string(manifest_evidence, "sha256") {
                        inspector.issue(path, "server-plan-manifest-digest-drift", manifest_ref)?;
                    }
                    if !python_optional_json_equal(manifest.get("item_id"), plan.get("item_ref")) {
                        inspector.issue(path, "server-plan-item-id-drift", manifest_ref)?;
                    }
                    let expected_payloads: Vec<Value> = rows(&manifest, "payload_files")
                        .map(|entry| json!({
                            "file_ref": entry.get("file_id").cloned().unwrap_or(Value::Null),
                            "relative_path": entry.get("relative_path").cloned().unwrap_or(Value::Null),
                            "byte_size": entry.get("byte_size").cloned().unwrap_or(Value::Null),
                            "sha256": entry.get("sha256").cloned().unwrap_or(Value::Null),
                            "verified": true,
                        }))
                        .collect();
                    if !python_optional_json_equal(
                        plan.get("payload_files"),
                        Some(&json!(expected_payloads)),
                    ) {
                        inspector.issue(
                            path,
                            "server-plan-payload-inventory-drift",
                            manifest_ref,
                        )?;
                    }
                    let rights_policy = plan.get("rights_policy").unwrap_or(&Value::Null);
                    let rights_ref = rights_policy.get("rights_record_ref");
                    if !python_optional_json_equal(manifest.get("rights_ref"), rights_ref) {
                        inspector.issue(path, "server-plan-rights-ref-drift", manifest_ref)?;
                    } else if let Some(rights_ref) = rights_ref.and_then(Value::as_str) {
                        if !safe_relative_path(rights_ref)
                            || !inspector.exists_source_ref(rights_ref)?
                        {
                            inspector.issue(
                                path,
                                "server-plan-rights-record-missing",
                                format!("server plan rights record is missing: {rights_ref}"),
                            )?;
                        } else {
                            let rights_digest = inspector.digest(rights_ref)?;
                            match rights_digest {
                                Some(digest)
                                    if Some(digest.as_str())
                                        == string(rights_policy, "rights_record_sha256") => {}
                                None => inspector.issue(
                                    path,
                                    "server-plan-rights-record-missing",
                                    format!("server plan rights record is missing: {rights_ref}"),
                                )?,
                                Some(_) => inspector.issue(
                                    path,
                                    "server-plan-rights-digest-drift",
                                    rights_ref,
                                )?,
                            }
                        }
                    }
                }
                None => {}
            }
        } else {
            inspector.issue(
                path,
                "server-plan-manifest-reference-missing",
                "server plan manifest.ref is missing",
            )?;
        }
        let version = plan
            .get("contract_version")
            .and_then(json_integer)
            .unwrap_or(1);
        for event_ref in array(&plan, "provenance_event_refs")
            .iter()
            .filter_map(Value::as_str)
        {
            let Some(event) = boundary_events.get(event_ref).cloned() else {
                inspector.issue(path, "unresolved-server-plan-boundary-event", event_ref)?;
                continue;
            };
            if version < 2 {
                continue;
            }
            let expected_output = (path.to_owned(), plan_digest.clone());
            if !event
                .outputs
                .get(&expected_output.0)
                .is_some_and(|digest| digest.as_deref() == Some(expected_output.1.as_str()))
            {
                inspector.issue(
                    path,
                    "server-plan-provenance-output-digest-drift",
                    event_ref,
                )?;
            }
            let rights_policy = plan.get("rights_policy").unwrap_or(&Value::Null);
            let expected_inputs = BTreeSet::from([
                (
                    manifest_ref.to_owned(),
                    string(manifest_evidence, "sha256").unwrap_or("").to_owned(),
                ),
                (
                    string(rights_policy, "rights_record_ref")
                        .unwrap_or("")
                        .to_owned(),
                    string(rights_policy, "rights_record_sha256")
                        .unwrap_or("")
                        .to_owned(),
                ),
            ]);
            if !expected_inputs.is_subset(&event.inputs) {
                inspector.issue(path, "server-plan-provenance-input-digest-drift", event_ref)?;
            }
        }
    }
    if planned_manifest_refs != expected_manifest_refs {
        inspector.issue(
            SERVER_PLANS.trim_end_matches('/'),
            "server-plan-manifest-coverage-drift",
            "server plan coverage differs from the exact current item-manifest set",
        )?;
    }

    inspector.checkpoint()?;
    let status = if inspector.unsupported.is_empty() {
        ScopeStatus::Complete
    } else {
        ScopeStatus::Unsupported
    };
    Ok(DiscoveryKernelOutput {
        report: Report {
            status,
            issues: inspector.issues,
            schema_requests: inspector.schema_requests,
            source_event_insertions,
            unsupported: inspector.unsupported,
            cost: Cost {
                source_bytes_read: inspector.read_bytes,
                observed_payload_bytes: inspector.payload_bytes,
                aggregate_document_copies: inspector.document_copies,
                state_bytes: inspector.state_bytes,
                native_history_referenced_bytes: inspector.native_history_referenced_bytes,
                native_history_referenced_state_bytes: inspector
                    .native_history_referenced_state_bytes,
                artifact_replay_referenced_publication_state_bytes: inspector
                    .artifact_replay_referenced_publication_state_bytes,
                artifact_replay_referenced_state_bytes: inspector
                    .artifact_replay_referenced_state_bytes,
                candidate_artifact_evidence_peak_state_bytes: inspector
                    .candidate_artifact_evidence_peak_state_bytes,
            },
        },
        candidate_direct_source_bytes: inspector.candidate_direct_source_bytes,
    })
}
