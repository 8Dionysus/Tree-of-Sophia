//! Bounded compilation of a derived source-navigation read model.
//! This crate writes private candidates; admission, rights and selection remain owner routes.

pub mod catalog;
pub mod d1;
pub mod d1_prepared_pair;
pub mod d1_projection_snapshot;
mod d1_public_baseline;
mod d1_public_build;
mod d1_public_capture;
mod d1_public_delta;
mod d1_public_graph;
mod d1_public_header;
mod d1_public_knowledge;
mod d1_public_lens;
mod d1_public_lens_specs;
mod d1_public_metadata;
mod d1_public_publication;
mod d1_public_rows;
mod d1_public_schema;
mod d1_public_semantics;
mod d1_public_sql;
mod d1_public_static;
pub mod epistemic_evidence;
pub mod local_prepared;
pub mod local_prepared_aux;
pub mod local_prepared_bulk;
mod local_prepared_read;
pub mod local_prepared_reuse;
pub mod local_prepared_search;
pub mod prepared_catalog_index;
pub mod prepared_catalog_semantics;
pub mod prepared_maintenance;
pub mod prepared_maintenance_file;
pub mod prepared_semantic_index;
mod prepared_semantic_kernel;
pub mod prepared_source_binding;
pub mod private_tmpfs_stage;
pub use d1_public_build::{
    PublicD1Build, PublicD1BuildLimits, build_public_d1, portable_public_d1_limits,
};
pub use d1_public_capture::{
    PublicCapture, PublicCaptureInputPaths, PublicCaptureLimits, RuntimeCaptureCreationUsage,
    RuntimeCaptureOwnedBudget, RuntimeCaptureProfile, RuntimeCaptureRole,
};
pub use d1_public_knowledge::project_private_knowledge_row;
pub use d1_public_lens::project_private_lens_auxiliary_rows;
pub use d1_public_rows::{project_private_navigation_row, project_private_philosophy_view_row};
mod knowledge_base;
pub mod knowledge_candidates;
pub mod knowledge_canon_materialize;
pub mod knowledge_canon_prepare;
pub mod knowledge_canon_source;
mod knowledge_catalog_index;
mod knowledge_corpus_original;
mod knowledge_corpus_source;
mod native_snapshot_originals;
pub use knowledge_corpus_original::{
    CORPUS_ORIGINAL_PROFILE, CapturedCorpusOrigin, CapturedCorpusOriginalPlan,
    CorpusOriginalCollection, CorpusOriginalCollectionReceipt, CorpusOriginalMember,
    CorpusOriginalOrigin, CorpusOriginalPage, CorpusOriginalPlan, CorpusOriginalReceipt,
    CorpusOriginalRow, CorpusOriginalSelector, CorpusOriginalViewIdentity,
    CorpusOriginalViewIdentityPage, KNOWLEDGE_CORPUS_MODEL_ABI, NATIVE_CORPUS_ORIGINAL_PROFILE,
    captured_runtime_input_manifest_digest, retain_captured_corpus_original,
    retain_corpus_original,
};
pub use knowledge_corpus_source::{
    CorpusOriginalSourceLimits, prepare_captured_corpus_original, prepare_native_corpus_original,
    retain_captured_corpus_original_from_capture,
};
pub mod managed_agent_producer;
pub mod managed_source;
pub use managed_source::{
    KNOWLEDGE_MANAGED_MODEL_ABI, KnowledgeSourceBasis, ManagedProducerProof, ManagedSourceDeltaV1,
    ManagedSourceGenerationV1, ManagedSourceGenerationV2, ManagedSourceProofV1,
    ManagedSourceProofV2,
};
mod knowledge_full;
#[cfg(all(not(target_arch = "wasm32"), any(test, feature = "test-fixture")))]
pub mod knowledge_full_fixture;
mod knowledge_global_titles;
mod knowledge_indexed;
mod knowledge_inherited_views;
mod knowledge_native;
mod knowledge_native_finalize;
mod knowledge_navigation_finalize;
mod knowledge_original_rows;
mod knowledge_philosophy_original;
mod knowledge_posting_codec;
pub mod native_snapshot;
pub mod native_snapshot_carriers;
pub mod native_snapshot_manifest;
pub use knowledge_philosophy_original::{
    KNOWLEDGE_PHILOSOPHY_MODEL_ABI, PHILOSOPHY_ORIGINAL_PROFILE, PhilosophyOriginalCollection,
    PhilosophyOriginalInput, PhilosophyOriginalPage, PhilosophyOriginalReceipt,
    PhilosophyOriginalRow, philosophy_original_rows_root, retain_philosophy_original,
};
pub use knowledge_posting_codec::{
    MAX_POSTING_DELTA_BYTES, MAX_POSTINGS_PER_BLOCK, decode_posting_block,
};
mod knowledge_navigation_original;
pub use knowledge_navigation_original::{
    KNOWLEDGE_NAVIGATION_MODEL_ABI, NAVIGATION_ORIGINAL_PROFILE, NavigationOriginalInput,
    NavigationOriginalLimits, NavigationOriginalMember, NavigationOriginalMemberPage,
    NavigationOriginalPage, NavigationOriginalReceipt, navigation_original_rights_root,
    retain_navigation_original,
};
mod knowledge_navigation_materialize;
pub mod knowledge_normalization;
mod knowledge_ordered;
mod knowledge_payload_codec;
pub mod knowledge_payload_read;
pub mod knowledge_philosophy_display;
mod knowledge_philosophy_materialize;
mod knowledge_philosophy_prepare;
mod knowledge_readable_context;
mod knowledge_registry;
mod knowledge_repository;
pub mod knowledge_repository_source;
mod knowledge_scope;
mod knowledge_seal;
mod knowledge_search;
mod knowledge_selected;
mod knowledge_semantic_join;
pub mod knowledge_source_claims;
mod knowledge_source_claims_prepare;
mod knowledge_source_navigation_node;
mod knowledge_source_navigation_prepare;
mod knowledge_source_navigation_relation;
pub mod knowledge_stage;
mod legacy;
mod managed_model_overlay;
pub use managed_model_overlay::{
    CompletedManagedOverlayBaseV2, CompletedManagedOverlayRecoveryV2,
    CompletedManagedOverlaySuccessorV2, ManagedOverlayChangedMemberV2, ManagedOverlayLookupWorkV2,
    ManagedOverlayReaderV2, prepare_managed_overlay_base_v2,
    prepare_managed_overlay_initial_agent_v2, prepare_managed_overlay_recovery_rebind_v2,
};
mod managed_model_manifest;
/// Bounded verified part decoding; namespace and source selection stay with callers.
pub use legacy::decode_partition_part;
mod native_knowledge_selection;
mod publication;
mod safe_open;
mod selected;
pub mod source_bibliographic;
mod source_bibliographic_native_text;
mod source_bibliographic_navigation;
mod source_bibliographic_render;
mod source_bibliographic_source;
mod source_bibliographic_unicode;
mod source_bibliographic_values;
mod source_bibliographic_versions;
pub use managed_model_manifest::{
    ManagedManifestDeltaV2, ManagedManifestLimitsV2, ManagedManifestRecoveryRebindV2,
    ManagedManifestV2, ManagedOverlaySourceBindingV2,
};
mod versions_catalog_epoch;
pub use versions_catalog_epoch::VersionsCatalogEpochV2;
pub mod source_corpus;
mod source_navigation_packets;
pub mod source_navigation_source;
mod source_navigation_storage;
pub mod source_philosophy;
pub mod source_philosophy_atlas;
pub mod source_philosophy_dossier_docx;
pub mod source_philosophy_dossier_extract;
pub mod source_philosophy_graph;
pub mod source_philosophy_multilingual;
pub mod source_philosophy_post_planting;
mod source_philosophy_support;
pub use source_philosophy_support::PhilosophySourceReadProfile;
pub mod source_philosophy_views;
pub mod source_witness_catalog;
mod sqlite_budget;
pub use sqlite_budget::{DedicatedSessionSqliteHeap, dedicated_session_heap_bytes};
mod vocabulary;
pub use knowledge_base::{BaseNodeOverrides, BaseNormalizationLimits, KnowledgeBaseNormalizer};
pub use knowledge_catalog_index::{CatalogIndexLimits, CatalogIndexReceipt, materialize_catalog};
pub use knowledge_full::{
    FullKnowledgeLimits, FullKnowledgeReceipt, compile_full_knowledge_components,
};
pub use knowledge_global_titles::{
    CompleteBaseNodes, GlobalTitleLimits, GlobalTitleReceipt, clear_global_titles, endpoint_title,
    order_native_graph_rows, prepare_global_titles, verify_global_titles,
};
pub use knowledge_indexed::{
    IndexedLimits, IndexedReceipt, materialize_indexed_sources,
    materialize_registered_indexed_nodes, materialize_registered_indexed_relations,
};
pub use knowledge_inherited_views::{
    CompleteRelationSeal, InheritedViewLimits, InheritedViewReceipt, clear_inherited_views,
    endpoint_inherited_views, prepare_global_inherited_views,
};
pub use knowledge_native::{
    NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeFamilyInputs, NativeProducerLimits,
    NativeProducerReceipt, materialize_native_sources, materialize_native_sources_with_inputs,
};
pub use knowledge_native_finalize::{
    NativeFinalizeLimits, NativeFinalizeReceipt, finalize_native_graph_rows,
    finalize_native_graph_rows_with_witnesses,
};
pub use knowledge_navigation_finalize::{
    NavigationFinalizeLimits, NavigationInheritedReceipt, apply_navigation_inherited_views,
};
pub use knowledge_navigation_materialize::{
    NavigationMaterializeLimits, NavigationNodeMaterializeReceipt, NavigationPlaceholderReceipt,
    NavigationRelationMaterializeReceipt, materialize_navigation_nodes,
    materialize_navigation_placeholders, materialize_navigation_relations,
};
pub use knowledge_ordered::{
    NormalizedNodeCandidate, NormalizedRelationCandidate, OrderedCandidateLimits,
    OrderedCandidateReceipt, OrderedKnowledgeSink,
};
pub use knowledge_philosophy_materialize::{
    PhilosophyBase, PhilosophyMaterializeLimits, PhilosophyMaterializeReceipt,
    PhilosophyNormalizer, PhilosophyRelationGlobalInputs, materialize_philosophy_nodes,
    materialize_philosophy_relations,
};
pub use knowledge_philosophy_prepare::{
    PhilosophyExternalDependency, PhilosophyPrepareLimits, PhilosophyPrepareReceipt,
    clear_philosophy_prepare, prepare_philosophy,
};
pub use knowledge_readable_context::{
    ReadableContextCarrier, ReadableContextCompiler, ReadableContextLimits,
    ordered_readable_witness, ordered_readable_witness_bounded,
};
pub use knowledge_registry::{KnowledgeRegistry, ResolvedType};
pub use knowledge_repository::{
    RepositoryPrepareReceipt, RepositoryRootInput, TopologyLimits, clear_repository_topology,
    materialize_repository_nodes, materialize_repository_relations, prepare_repository_topology,
    repository_material_witness, scan_repository_relation_sources,
};
pub use knowledge_scope::{ScopeLimits, ScopeReceipt, write_source_scope};
pub use knowledge_seal::{
    KNOWLEDGE_MODEL_ABI, KnowledgeSealReceipt, SealLimits, seal_knowledge_model,
};
pub use knowledge_search::{SearchBuildLimits, SearchIndexReceipt, build_search_index};
pub use knowledge_selected::{
    ColdOpenLimits, ExpectedSourceScope, ImmutableKnowledgeCustody, KnowledgeSelectedExpectation,
    VerifiedKnowledgeModel, open_selected_knowledge_model, open_selected_knowledge_model_owned,
};
pub use knowledge_semantic_join::{
    SemanticJoinReceipt, clear_semantic_joins, materialize_semantic_relations,
    prepare_semantic_joins, render_supplied_shared_identity_pair,
};
pub use knowledge_source_claims_prepare::{
    ClaimExternalDependency, ClaimPrepareLimits, ClaimPrepareReceipt, prepare_source_claims,
};
pub use knowledge_source_navigation_node::{
    NavigationBaseNode, NavigationEndpoint, NavigationNodeLimits, NavigationNodeNormalizer,
    NavigationPlaceholderBase,
};
pub use knowledge_source_navigation_prepare::{
    NavigationExternalDependency, NavigationHeaderClaim, NavigationPrepareLimits,
    NavigationPrepareReceipt, prepare_source_navigation,
};
pub use knowledge_source_navigation_relation::{
    NavigationBaseRelation, NavigationRelationDependencyReceipt, NavigationRelationGlobalInputs,
    NavigationRelationLimits, NavigationRelationNormalizeLimits, NavigationRelationNormalizer,
    direct_assertion_context, prepare_navigation_relation_dependencies,
};
pub use legacy::LegacyPartitionedNavigation;
pub use native_knowledge_selection::{
    LinuxFsVerityCustody, NativeFsVerityMeasurement, NativeKnowledgeSelection, NativeProcessLimits,
    NativeSelectionPaths, NativeSelectionProducer, prepare_native_knowledge_artifact,
};
pub use publication::{PublicationAuthority, PublishedReceipt, SelectionFence, publish_candidate};
pub use selected::{
    ImmutableModelCustody, SelectedExpectation, VerifiedSelectedModel, VerifiedSelection,
    open_selected_model,
};
pub use source_bibliographic_source::{
    CandidateSourceCatalogInputPlan, ColdSourceCatalogInputPlan, ColdSourceCatalogInputSpool,
    ColdSourceCatalogSpoolIsolation, ColdSourceCatalogSpoolLimits, ColdSourceCatalogSpoolPhase,
    SourceBibliographicCandidate, SourceCatalogInputLimits, SourceCatalogInputPlan,
    SourceCatalogRenderWorkV1, plan_candidate_source_catalog_inputs_with_workspace,
    plan_cold_source_catalog_inputs, plan_cold_source_catalog_inputs_with_workspace,
    plan_source_catalog_inputs, plan_streamed_cold_source_catalog_inputs,
    prepare_candidate_source_catalog_plan_observed, prepare_cold_source_catalog_plan,
    prepare_cold_source_catalog_plan_observed, prepare_source_catalog_plan,
    prepare_streamed_cold_source_catalog_spool, render_source_bibliographic_plan,
    render_source_bibliographic_plan_with_work, render_streamed_source_bibliographic_spool,
};
pub use source_bibliographic_versions::StreamedBibliographicReadLedger;
pub use vocabulary::{QueryVocabulary, RegisteredSource, VocabularyBinding};

use rusqlite::{Connection, params};
use std::{fmt, fs, io::Read, path::Path};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue,
    canonical_bytes_v1, parse_json,
};

pub const MODEL_ABI: &str = "tos_source_navigation_read_model_v1";
pub const COMPILER_VERSION: &str = "tos_compiler_navigation_v1";
pub const SELECTION_PROFILE: &str = "tos_source_navigation_visible_v1";

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Sql(rusqlite::Error),
    SqlitePhase {
        phase: knowledge_stage::WritePhase,
        error: rusqlite::Error,
    },
    Invalid(&'static str),
    PreparedUnsupported(&'static str),
    ManagedSourceUnsupported(&'static str),
    Source(String),
    Budget(&'static str),
    SqliteVmBudget {
        phase: knowledge_stage::WritePhase,
        used_steps: u64,
        max_steps: u64,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Sql(e) => write!(f, "SQLite: {e}"),
            Self::SqlitePhase { phase, error } => write!(f, "SQLite in {phase:?}: {error}"),
            Self::Invalid(s) => write!(f, "invalid compiler input: {s}"),
            Self::PreparedUnsupported(s) => {
                write!(f, "unsupported local prepared carrier/profile: {s}")
            }
            Self::ManagedSourceUnsupported(s) => {
                write!(f, "unsupported managed selected source: {s}")
            }
            Self::Source(s) => write!(f, "source carrier: {s}"),
            Self::Budget(s) => write!(f, "compiler budget exceeded: {s}"),
            Self::SqliteVmBudget {
                phase,
                used_steps,
                max_steps,
            } => write!(
                f,
                "compiler budget exceeded: SQLite VM steps in {phase:?} (used {used_steps}, max {max_steps})"
            ),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        if matches!(
            &e,
            rusqlite::Error::SqliteFailure(failure, _)
                if failure.code == rusqlite::ffi::ErrorCode::OperationInterrupted
        ) {
            return Self::Budget("SQLite VM steps");
        }
        Self::Sql(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// The trusted source owner supplies this exact, pinned cut. A projection digest
/// binds transport bytes but never attests source admission by itself.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    pub owner_profile: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub membership_root: String,
    pub index_generation: String,
    pub route_map_version: String,
    pub reader_abi: String,
    pub projection_root_sha256: String,
    pub complete: bool,
}
impl SourceBinding {
    fn validate(&self) -> Result<()> {
        if !self.complete {
            return Err(Error::Invalid("source index incomplete"));
        }
        for s in [
            &self.owner_profile,
            &self.source_cut,
            &self.membership_root,
            &self.index_generation,
            &self.route_map_version,
            &self.reader_abi,
        ] {
            if s.is_empty() {
                return Err(Error::Invalid("missing source binding"));
            }
        }
        Digest256::from_hex(&self.projection_root_sha256)
            .map_err(|_| Error::Invalid("invalid projection root digest"))?;
        Digest256::from_hex(&self.membership_root)
            .map_err(|_| Error::Invalid("invalid sealed membership root digest"))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_rows: u64,
    pub max_row_bytes: usize,
    pub max_output_bytes: u64,
    pub max_work_bytes: u64,
    pub sqlite_cache_kib: u32,
    /// Cumulative SQLite virtual-machine instructions, checked at most every 1,000 steps.
    pub max_sql_vm_steps: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_rows: 10_000_000,
            max_row_bytes: 8 * 1024 * 1024,
            max_output_bytes: 8 * 1024 * 1024 * 1024,
            max_work_bytes: 16 * 1024 * 1024 * 1024,
            sqlite_cache_kib: 8192,
            max_sql_vm_steps: 1_000_000_000_000,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<()> {
        if self.max_rows == 0
            || self.max_row_bytes == 0
            || self.max_output_bytes == 0
            || self.max_work_bytes == 0
            || self.sqlite_cache_kib == 0
            || self.max_sql_vm_steps == 0
        {
            return Err(Error::Budget("limits must be positive"));
        }
        if self.max_row_bytes > 8 * 1024 * 1024 || self.sqlite_cache_kib > 64 * 1024 {
            return Err(Error::Budget("row or cache exceeds format cap"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Collection {
    Nodes,
    Edges,
    Rights,
}
impl Collection {
    pub const ALL: [Self; 3] = [Self::Nodes, Self::Edges, Self::Rights];
    pub const fn legacy_name(self) -> &'static str {
        match self {
            Self::Nodes => "source_navigation/nodes",
            Self::Edges => "source_navigation/edges",
            Self::Rights => "source_navigation/rights",
        }
    }
    fn id_field(self) -> &'static str {
        match self {
            Self::Nodes => "node_id",
            Self::Edges => "edge_id",
            Self::Rights => "rights_id",
        }
    }
}

/// Each visit is a complete, bounded pass over one collection from the same cut.
pub trait NavigationInput {
    fn verify_binding(&self, binding: &SourceBinding) -> Result<()>;
    /// Exact source-owned navigation header selected by the pinned root.
    fn authority_boundary(&self) -> Result<String>;
    fn visit(
        &mut self,
        collection: Collection,
        sink: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<()>;
    /// Recheck the same sealed cut and complete input bytes; do not require
    /// that this cut is still the owner's newest selected pointer.
    fn verify_sealed_cut(&mut self) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct CandidateReceipt {
    pub source_cut: String,
    pub projection_root_sha256: String,
    pub node_count: u64,
    pub edge_count: u64,
    pub rights_count: u64,
    pub visible_node_count: u64,
    pub visible_edge_count: u64,
    pub sqlite_sha256: String,
    pub sqlite_size_bytes: u64,
}

fn required<'a>(row: &'a JsonValue, field: &str) -> Result<&'a str> {
    let s = row
        .object_get(field)
        .and_then(JsonValue::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("missing nonempty row identity or endpoint"))?;
    if s.len() > 4096 {
        return Err(Error::Budget("identity bytes"));
    }
    Ok(s)
}
fn has_provenance(row: &JsonValue) -> bool {
    row.object_get("source_ref")
        .and_then(JsonValue::as_str)
        .is_some_and(|s| !s.is_empty())
        || row
            .object_get("source_refs")
            .and_then(JsonValue::as_array)
            .is_some_and(|v| {
                !v.is_empty() && v.iter().all(|x| x.as_str().is_some_and(|s| !s.is_empty()))
            })
}
fn emitted_row(raw: &[u8], limits: Limits) -> Result<(JsonValue, Vec<u8>)> {
    if raw.len() > limits.max_row_bytes {
        return Err(Error::Budget("row bytes"));
    }
    let json_limits = JsonLimits::new(limits.max_row_bytes, 96, 1_000_000, 4300)
        .map_err(|e| Error::Source(e.to_string()))?;
    let doc = parse_json(raw, JsonMode::PublishedStrict, json_limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let row = doc.into_root();
    if row.as_object().is_none() || !has_provenance(&row) {
        return Err(Error::Invalid("row object/provenance absent"));
    }
    let carrier = canonical_bytes_v1(&row, CanonicalProfile::SourceRecordDigestV1, json_limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    Ok((row, carrier))
}
fn truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null | JsonValue::Bool(false) => false,
        JsonValue::String(s) => !s.units().is_empty(),
        JsonValue::Array(v) => !v.is_empty(),
        JsonValue::Object(v) => !v.is_empty(),
        // A finite host float loses Python truthiness for very large JSON
        // integers and exponents. Only mantissa digits determine zero.
        JsonValue::Number(n) => n
            .lexeme
            .split(['e', 'E'])
            .next()
            .is_some_and(|mantissa| mantissa.bytes().any(|b| (b'1'..=b'9').contains(&b))),
        JsonValue::Bool(true) => true,
    }
}
fn visible_node(row: &JsonValue) -> bool {
    !row.object_get("properties")
        .and_then(|p| p.object_get("packet_id"))
        .is_some_and(truthy)
}

#[cfg(test)]
mod truthiness_tests {
    use super::*;

    #[test]
    fn numeric_packet_selection_does_not_round_through_host_float() {
        let limits = JsonLimits::new(4096, 16, 128, 128).unwrap();
        let nonzero = parse_json(b"1e308", JsonMode::PublishedStrict, limits)
            .unwrap()
            .into_root();
        let zero = parse_json(b"0e308", JsonMode::PublishedStrict, limits)
            .unwrap()
            .into_root();
        let large_integer = format!("1{}", "0".repeat(100));
        let large_integer = parse_json(large_integer.as_bytes(), JsonMode::PublishedStrict, limits)
            .unwrap()
            .into_root();
        assert!(truthy(&nonzero));
        assert!(!truthy(&zero));
        assert!(truthy(&large_integer));
    }
}
fn root_item(hasher: &mut Digest256Hasher, id: &str, digest: &str) {
    hasher.update(&(id.len() as u64).to_be_bytes());
    hasher.update(id.as_bytes());
    hasher.update(
        Digest256::from_hex(digest)
            .expect("stored digest")
            .as_bytes(),
    );
}
fn stream_digest(file: &mut impl Read) -> Result<(String, u64)> {
    stream_digest_with_check(file, |_| Ok(()))
}

/// Existing digest primitive with the serial owner's original work/cutoff hook.
/// The callback sees each actually read chunk before hashing; refusal does not
/// refund observed bytes. This helper creates no grant or source authority.
fn stream_digest_with_check(
    file: &mut impl Read,
    mut check_and_charge: impl FnMut(usize) -> Result<()>,
) -> Result<(String, u64)> {
    let mut hash = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buf = [0u8; 65536];
    loop {
        check_and_charge(0)?;
        let n = file.read(&mut buf)?;
        check_and_charge(n)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .ok_or(Error::Budget("file length"))?;
        hash.update(&buf[..n]);
    }
    Ok((hash.finalize().to_hex(), total))
}
fn file_digest(path: &Path) -> Result<(String, u64)> {
    let mut file = safe_open::open_regular(path, u64::MAX)?;
    stream_digest(&mut file)
}
fn meta(db: &Connection, key: &str, value: &str) -> Result<()> {
    db.execute("INSERT INTO metadata VALUES (?1,?2)", params![key, value])?;
    Ok(())
}

/// Writes one private unselected SQLite candidate; failure removes only its new file.
pub fn compile_navigation<I: NavigationInput>(
    input: &mut I,
    binding: &SourceBinding,
    candidate: &Path,
    limits: Limits,
) -> Result<CandidateReceipt> {
    binding.validate()?;
    input.verify_binding(binding)?;
    let boundary = input.authority_boundary()?;
    if boundary.is_empty() || boundary.len() > 256 * 1024 {
        return Err(Error::Invalid("source navigation authority boundary"));
    }
    limits.validate()?;
    if candidate.exists() || candidate.is_symlink() {
        return Err(Error::Invalid("candidate exists"));
    }
    let parent = candidate
        .parent()
        .ok_or(Error::Invalid("candidate needs parent"))?;
    if !parent.is_dir() || parent.is_symlink() {
        return Err(Error::Invalid("candidate parent"));
    }
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    drop(opts.open(candidate)?);
    let result = compile_created(input, binding, candidate, limits, &boundary);
    if result.is_err() {
        let _ = fs::remove_file(candidate);
    }
    result
}

fn compile_created<I: NavigationInput>(
    input: &mut I,
    binding: &SourceBinding,
    candidate: &Path,
    limits: Limits,
    authority_boundary: &str,
) -> Result<CandidateReceipt> {
    let mut db = Connection::open(candidate)?;
    sqlite_budget::configure(&db, limits)?;
    db.execute_batch(
        "CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
         CREATE TABLE nodes(node_id TEXT PRIMARY KEY, visible INTEGER NOT NULL,
             carrier BLOB NOT NULL, carrier_sha256 TEXT NOT NULL, carrier_size INTEGER NOT NULL) WITHOUT ROWID;
         CREATE TABLE edges(edge_id TEXT PRIMARY KEY,from_id TEXT NOT NULL,to_id TEXT NOT NULL,
             visible INTEGER NOT NULL,carrier BLOB NOT NULL,carrier_sha256 TEXT NOT NULL,
             carrier_size INTEGER NOT NULL) WITHOUT ROWID;
         CREATE INDEX outgoing ON edges(visible,from_id,edge_id);
         CREATE TABLE rights(rights_id TEXT PRIMARY KEY,carrier BLOB NOT NULL,
             carrier_sha256 TEXT NOT NULL,carrier_size INTEGER NOT NULL) WITHOUT ROWID;
         CREATE TABLE rights_scopes(rights_id TEXT NOT NULL,scope_id TEXT NOT NULL,
             PRIMARY KEY(rights_id,scope_id)) WITHOUT ROWID;
         CREATE INDEX rights_by_scope ON rights_scopes(scope_id,rights_id);
         CREATE TABLE adjacency(from_id TEXT PRIMARY KEY,edge_count INTEGER NOT NULL,
             edges_sha256 TEXT NOT NULL) WITHOUT ROWID;"
    )?;
    let tx = db.transaction()?;
    let mut total = 0u64;
    let mut emitted_input_bytes = 0u64;
    let mut counts = [0u64; 3];
    for (index, collection) in Collection::ALL.into_iter().enumerate() {
        input.visit(collection, &mut |raw| {
            total = total.checked_add(1).ok_or(Error::Budget("row count"))?;
            if total > limits.max_rows {
                return Err(Error::Budget("row count"));
            }
            emitted_input_bytes = emitted_input_bytes
                .checked_add(raw.len() as u64)
                .ok_or(Error::Budget("emitted input bytes"))?;
            if emitted_input_bytes > limits.max_work_bytes {
                return Err(Error::Budget("emitted input bytes"));
            }
            let (row, carrier) = emitted_row(raw, limits)?;
            let id = required(&row, collection.id_field())?;
            let sha = Digest256::of_bytes(&carrier).to_hex();
            let size = carrier.len() as i64;
            match collection {
                Collection::Nodes => {
                    tx.execute(
                        "INSERT INTO nodes VALUES (?1,?2,?3,?4,?5)",
                        params![id, visible_node(&row), carrier, sha, size],
                    )?;
                }
                Collection::Edges => {
                    let from = required(&row, "from_id")?;
                    let to = required(&row, "to_id")?;
                    tx.execute(
                        "INSERT INTO edges VALUES (?1,?2,?3,?4,?5,?6,?7)",
                        params![id, from, to, false, carrier, sha, size],
                    )?;
                }
                Collection::Rights => {
                    tx.execute(
                        "INSERT INTO rights VALUES (?1,?2,?3,?4)",
                        params![id, carrier, sha, size],
                    )?;
                    if let Some(scopes) = row.object_get("scope_refs").and_then(JsonValue::as_array)
                    {
                        for scope in scopes {
                            let scope = scope
                                .as_str()
                                .filter(|s| !s.is_empty())
                                .ok_or(Error::Invalid("invalid rights scope"))?;
                            tx.execute(
                                "INSERT OR IGNORE INTO rights_scopes VALUES (?1,?2)",
                                params![id, scope],
                            )?;
                        }
                    }
                }
            }
            counts[index] += 1;
            if total % 1024 == 0 {
                let pages: u64 = tx.query_row("PRAGMA page_count", [], |r| r.get(0))?;
                let size: u64 = tx.query_row("PRAGMA page_size", [], |r| r.get(0))?;
                if pages.saturating_mul(size) > limits.max_output_bytes {
                    return Err(Error::Budget("output bytes"));
                }
            }
            Ok(())
        })?;
    }
    input.verify_sealed_cut()?;
    tx.execute(
        "UPDATE edges SET visible=1 WHERE EXISTS
           (SELECT 1 FROM nodes a WHERE a.node_id=edges.from_id AND a.visible=1)
           AND EXISTS (SELECT 1 FROM nodes b WHERE b.node_id=edges.to_id AND b.visible=1)",
        [],
    )?;
    let visible_nodes: u64 =
        tx.query_row("SELECT count(*) FROM nodes WHERE visible=1", [], |r| {
            r.get(0)
        })?;
    let visible_edges: u64 =
        tx.query_row("SELECT count(*) FROM edges WHERE visible=1", [], |r| {
            r.get(0)
        })?;
    let mut roots = Vec::new();
    for sql in [
        "SELECT node_id,carrier_sha256 FROM nodes ORDER BY node_id",
        "SELECT edge_id,carrier_sha256 FROM edges ORDER BY edge_id",
        "SELECT rights_id,carrier_sha256 FROM rights ORDER BY rights_id",
    ] {
        let mut hash = Digest256Hasher::new();
        let mut statement = tx.prepare(sql)?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, digest) = row?;
            root_item(&mut hash, &id, &digest);
        }
        roots.push(hash.finalize().to_hex());
    }
    {
        let mut statement=tx.prepare(
            "SELECT from_id,edge_id,carrier_sha256 FROM edges WHERE visible=1 ORDER BY from_id,edge_id")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut current = String::new();
        let mut count = 0u64;
        let mut hash = Digest256Hasher::new();
        for row in rows {
            let (from, edge, digest) = row?;
            if current != from {
                if count > 0 {
                    tx.execute(
                        "INSERT INTO adjacency VALUES (?1,?2,?3)",
                        params![current, count, hash.finalize().to_hex()],
                    )?;
                }
                current = from;
                count = 0;
                hash = Digest256Hasher::new();
            }
            root_item(&mut hash, &edge, &digest);
            count += 1;
        }
        if count > 0 {
            tx.execute(
                "INSERT INTO adjacency VALUES (?1,?2,?3)",
                params![current, count, hash.finalize().to_hex()],
            )?;
        }
    }
    let empty_sha = Digest256::of_bytes(b"").to_hex();
    tx.execute(
        "INSERT INTO adjacency(from_id,edge_count,edges_sha256)
         SELECT node_id,0,?1 FROM nodes WHERE visible=1
         AND NOT EXISTS (SELECT 1 FROM adjacency a WHERE a.from_id=nodes.node_id)",
        params![empty_sha],
    )?;
    for (k, v) in [
        ("model_abi", MODEL_ABI.to_owned()),
        ("compiler_version", COMPILER_VERSION.to_owned()),
        ("selection_profile", SELECTION_PROFILE.to_owned()),
        (
            "json_profile",
            JsonMode::PublishedStrict.as_str().to_owned(),
        ),
        ("owner_profile", binding.owner_profile.clone()),
        ("source_cut", binding.source_cut.clone()),
        ("through_commit_seq", binding.through_commit_seq.to_string()),
        ("membership_root", binding.membership_root.clone()),
        ("index_generation", binding.index_generation.clone()),
        ("route_map_version", binding.route_map_version.clone()),
        ("reader_abi", binding.reader_abi.clone()),
        (
            "projection_root_sha256",
            binding.projection_root_sha256.clone(),
        ),
        ("node_count", counts[0].to_string()),
        ("edge_count", counts[1].to_string()),
        ("rights_count", counts[2].to_string()),
        ("visible_node_count", visible_nodes.to_string()),
        ("visible_edge_count", visible_edges.to_string()),
        ("node_root_sha256", roots[0].clone()),
        ("edge_root_sha256", roots[1].clone()),
        ("rights_root_sha256", roots[2].clone()),
        ("complete", "true".to_owned()),
        ("authority_boundary", authority_boundary.to_owned()),
        (
            "derived_authority",
            "candidate_only_no_admission".to_owned(),
        ),
    ] {
        meta(&tx, k, &v)?;
    }
    tx.commit()?;
    if db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))? != "ok" {
        return Err(Error::Invalid("SQLite integrity"));
    }
    db.close().map_err(|(_, e)| Error::Sql(e))?;
    let (sha, size) = file_digest(candidate)?;
    if size > limits.max_output_bytes {
        return Err(Error::Budget("final output bytes"));
    }
    fs::File::open(candidate)?.sync_all()?;
    fs::File::open(candidate.parent().unwrap())?.sync_all()?;
    Ok(CandidateReceipt {
        source_cut: binding.source_cut.clone(),
        projection_root_sha256: binding.projection_root_sha256.clone(),
        node_count: counts[0],
        edge_count: counts[1],
        rights_count: counts[2],
        visible_node_count: visible_nodes,
        visible_edge_count: visible_edges,
        sqlite_sha256: sha,
        sqlite_size_bytes: size,
    })
}

/// Exact private-source structural/paragraph producer; no semantic admission.
pub mod antonovsky_structural;

pub mod native_prepare;

/// Finite owned external-tool I/O for exact native producers.
pub mod owned_native_child;

pub mod zarathustra_lexical;
pub mod zarathustra_lexical_schema;
pub mod zarathustra_lexical_validate;

mod zarathustra_lexical_morphology_validate;
mod zarathustra_lexical_usage_validate;

pub mod research_concept_workbench;
pub mod research_eternal_return;
pub mod research_eternal_return_concept;
pub mod research_execution;
pub mod research_morphology_theme;
pub mod research_paragraph_alignment;
pub mod research_parallel_lexical;

mod research_reading_discourse;
mod research_reading_formulas;
pub mod research_reading_workbench;

mod retained_state;

// Explicit layout is admitted only by the original owned whole producer.
pub use native_snapshot::{
    NativeSnapshotCreationUsage, NativeSnapshotOwnedBudget, NativeSnapshotOwnedReadLoan,
    build_native_knowledge_snapshot_from_capture_with_owned_budget_and_layout,
    with_native_knowledge_snapshot_from_capture_with_owned_budget_and_layout,
};
