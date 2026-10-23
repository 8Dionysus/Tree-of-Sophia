//! Bounded compilation of derived knowledge, catalog and publication models.
//! This crate writes private candidates; admission, rights and selection remain owner routes.

#[cfg(feature = "source-tools")]
pub mod catalog;
#[cfg(feature = "source-tools")]
pub mod d1;
#[cfg(feature = "source-tools")]
pub mod d1_prepared_pair;
#[cfg(feature = "source-tools")]
pub mod d1_projection_snapshot;
#[cfg(feature = "source-tools")]
mod d1_public_baseline;
#[cfg(feature = "source-tools")]
mod d1_public_build;
mod d1_public_capture;
#[cfg(feature = "source-tools")]
mod d1_public_delta;
#[cfg(feature = "source-tools")]
mod d1_public_graph;
#[cfg(feature = "source-tools")]
mod d1_public_header;
#[cfg(feature = "source-tools")]
mod d1_public_knowledge;
#[cfg(feature = "source-tools")]
mod d1_public_lens;
#[cfg(feature = "source-tools")]
mod d1_public_lens_specs;
#[cfg(feature = "source-tools")]
mod d1_public_metadata;
#[cfg(feature = "source-tools")]
mod d1_public_publication;
mod d1_public_rows;
#[cfg(feature = "source-tools")]
mod d1_public_schema;
#[cfg(feature = "source-tools")]
mod d1_public_semantics;
#[cfg(feature = "source-tools")]
mod d1_public_sql;
mod d1_public_static;
#[cfg(feature = "source-tools")]
pub mod epistemic_evidence;
#[cfg(feature = "source-tools")]
pub mod pwe_native;
pub mod local_prepared;
pub mod local_prepared_aux;
#[cfg(feature = "source-tools")]
pub mod local_prepared_bulk;
mod local_prepared_read;
#[cfg(feature = "source-tools")]
pub mod local_prepared_reuse;
pub mod local_prepared_search;
#[cfg(feature = "source-tools")]
pub mod prepared_catalog_index;
pub mod prepared_catalog_semantics;
#[cfg(feature = "source-tools")]
pub mod prepared_maintenance;
#[cfg(feature = "source-tools")]
pub mod prepared_maintenance_file;
#[cfg(feature = "source-tools")]
pub mod prepared_semantic_index;
#[cfg(feature = "source-tools")]
mod prepared_semantic_kernel;
pub mod prepared_source_binding;
#[cfg(feature = "source-tools")]
pub mod private_tmpfs_stage;
#[cfg(feature = "source-tools")]
pub use d1_public_build::{
    PublicD1Build, PublicD1BuildLimits, build_public_d1, portable_public_d1_limits,
    public_d1_limits,
};
pub use d1_public_capture::{
    PublicCapture, PublicCaptureInputPaths, PublicCaptureLimits, RuntimeCaptureCreationUsage,
    RuntimeCaptureOwnedBudget, RuntimeCaptureProfile, RuntimeCaptureRole,
};
#[cfg(feature = "source-tools")]
pub use d1_public_knowledge::project_private_knowledge_row;
#[cfg(feature = "source-tools")]
pub use d1_public_lens::project_private_lens_auxiliary_rows;
#[cfg(feature = "source-tools")]
pub use d1_public_rows::project_private_navigation_row;
pub use d1_public_rows::project_private_philosophy_view_row;
#[cfg(feature = "source-tools")]
mod knowledge_base;
#[cfg(feature = "source-tools")]
pub mod knowledge_candidates;
#[cfg(feature = "source-tools")]
pub mod knowledge_canon_materialize;
#[cfg(feature = "source-tools")]
pub mod knowledge_canon_prepare;
#[cfg(feature = "source-tools")]
pub mod knowledge_canon_source;
#[cfg(feature = "source-tools")]
mod knowledge_catalog_index;
mod knowledge_corpus_original;
#[cfg(feature = "source-tools")]
mod knowledge_corpus_source;
#[cfg(feature = "source-tools")]
mod native_normalization_binding;
#[cfg(feature = "source-tools")]
mod native_snapshot_originals;
pub use knowledge_corpus_original::{
    CORPUS_ORIGINAL_PROFILE, CapturedCorpusOrigin, CapturedCorpusOriginalPlan,
    CorpusOriginalCollection, CorpusOriginalCollectionReceipt, CorpusOriginalMember,
    CorpusOriginalOrigin, CorpusOriginalPage, CorpusOriginalPlan, CorpusOriginalReceipt,
    CorpusOriginalRow, CorpusOriginalSelector, CorpusOriginalViewIdentity,
    CorpusOriginalViewIdentityPage, KNOWLEDGE_CORPUS_MODEL_ABI, NATIVE_CORPUS_ORIGINAL_PROFILE,
    captured_runtime_input_manifest_digest,
};
#[cfg(feature = "source-tools")]
pub use knowledge_corpus_original::{retain_captured_corpus_original, retain_corpus_original};
#[cfg(feature = "source-tools")]
pub use knowledge_corpus_source::{
    CorpusOriginalSourceLimits, prepare_captured_corpus_original, prepare_native_corpus_original,
    retain_captured_corpus_original_from_capture,
};
#[cfg(feature = "source-tools")]
pub mod managed_agent_producer;
pub mod managed_source;
pub use managed_source::{
    KNOWLEDGE_MANAGED_MODEL_ABI, KnowledgeSourceBasis, ManagedProducerProof, ManagedSourceDeltaV1,
    ManagedSourceGenerationV1, ManagedSourceGenerationV2, ManagedSourceProofV1,
    ManagedSourceProofV2,
};
#[cfg(feature = "source-tools")]
mod knowledge_full;
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "source-tools",
    any(test, feature = "test-fixture")
))]
pub mod knowledge_full_fixture;
#[cfg(feature = "source-tools")]
mod knowledge_global_titles;
#[cfg(feature = "source-tools")]
mod knowledge_indexed;
#[cfg(feature = "source-tools")]
mod knowledge_inherited_views;
#[cfg(feature = "source-tools")]
mod knowledge_native;
#[cfg(feature = "source-tools")]
mod knowledge_native_finalize;
mod knowledge_original_rows;
mod knowledge_philosophy_original;
mod knowledge_posting_codec;
pub mod native_cold_resources;
#[cfg(feature = "source-tools")]
pub mod native_snapshot;
#[cfg(feature = "source-tools")]
pub mod native_snapshot_carriers;
#[cfg(feature = "source-tools")]
pub mod native_snapshot_manifest;
#[cfg(feature = "source-tools")]
pub mod processing_cache;
#[cfg(feature = "source-tools")]
pub use knowledge_philosophy_original::retain_philosophy_original;
pub use knowledge_philosophy_original::{
    KNOWLEDGE_PHILOSOPHY_MODEL_ABI, PHILOSOPHY_ORIGINAL_PROFILE, PhilosophyOriginalCollection,
    PhilosophyOriginalInput, PhilosophyOriginalPage, PhilosophyOriginalReceipt,
    PhilosophyOriginalRow, philosophy_original_rows_root,
};
pub use knowledge_posting_codec::{
    MAX_POSTING_DELTA_BYTES, MAX_POSTINGS_PER_BLOCK, decode_posting_block,
    decode_posting_block_for_abi,
};
mod knowledge_navigation_original;
#[cfg(feature = "source-tools")]
pub use knowledge_navigation_original::retain_navigation_original;
pub use knowledge_navigation_original::{
    KNOWLEDGE_NAVIGATION_MODEL_ABI, NAVIGATION_ORIGINAL_PROFILE, NavigationOriginalInput,
    NavigationOriginalLimits, NavigationOriginalMember, NavigationOriginalMemberPage,
    NavigationOriginalPage, NavigationOriginalReceipt, NavigationOriginalRightsHasher,
    navigation_original_rights_root,
};
mod controlled_cold_model;
mod knowledge_byte_codec;
mod knowledge_byte_dictionary;
#[cfg(feature = "source-tools")]
mod knowledge_navigation_materialize;
pub mod knowledge_normalization;
mod knowledge_payload_codec;
pub mod knowledge_payload_read;
pub mod knowledge_philosophy_display;
#[cfg(feature = "source-tools")]
mod knowledge_philosophy_materialize;
#[cfg(feature = "source-tools")]
mod knowledge_philosophy_prepare;
mod knowledge_readable_context;
mod knowledge_registry;
#[cfg(feature = "source-tools")]
mod knowledge_repository;
#[cfg(feature = "source-tools")]
pub mod knowledge_repository_source;
#[cfg(feature = "source-tools")]
mod knowledge_scope;
#[cfg(feature = "source-tools")]
mod knowledge_seal;
#[cfg(feature = "source-tools")]
mod knowledge_search;
mod knowledge_search_rank;
pub use knowledge_search_rank::{
    decode_search_rank_field, search_rank_decode_work_limit, search_rank_decode_workspace,
    search_rank_field_size, search_rank_fields_packed,
};
mod knowledge_selected;
pub use controlled_cold_model::{
    ControlledCarrierSelection, ControlledGramStat, ControlledKnowledgeModel,
    ControlledPostingPage, ControlledQueryHeap, ControlledSearchCandidate, ControlledSearchKind,
};
pub use controlled_cold_model::{
    ControlledLensKeyRow, ControlledLensKeySelection, ControlledOriginalCollection,
    ControlledOriginalReceipt, ControlledOriginalRowRead, ControlledSidecarModel,
    SearchSidecarAdmissionError,
};
#[cfg(feature = "source-tools")]
mod knowledge_semantic_join;
#[cfg(feature = "source-tools")]
pub mod knowledge_source_claims;
#[cfg(feature = "source-tools")]
mod knowledge_source_claims_prepare;
#[cfg(feature = "source-tools")]
mod knowledge_source_navigation_node;
#[cfg(feature = "source-tools")]
mod knowledge_source_navigation_prepare;
#[cfg(feature = "source-tools")]
mod knowledge_source_navigation_relation;
pub mod knowledge_stage;
mod legacy;
#[cfg(feature = "source-tools")]
mod managed_model_overlay;
#[cfg(feature = "source-tools")]
pub use managed_model_overlay::{
    CompletedManagedOverlayBaseV2, CompletedManagedOverlayRecoveryV2,
    CompletedManagedOverlaySuccessorV2, ManagedOverlayChangedMemberV2, ManagedOverlayLookupWorkV2,
    ManagedOverlayReaderV2, prepare_managed_overlay_base_v2,
    prepare_managed_overlay_initial_agent_v2, prepare_managed_overlay_recovery_rebind_v2,
};
#[cfg(feature = "source-tools")]
mod managed_model_manifest;
/// Bounded verified part decoding; namespace and source selection stay with callers.
pub use legacy::decode_partition_part;
mod native_knowledge_selection;
mod safe_open;
#[cfg(feature = "source-tools")]
pub mod source_bibliographic;
#[cfg(feature = "source-tools")]
mod source_bibliographic_native_text;
#[cfg(feature = "source-tools")]
mod source_bibliographic_navigation;
#[cfg(feature = "source-tools")]
mod source_bibliographic_render;
#[cfg(feature = "source-tools")]
mod source_bibliographic_selected_files;
#[cfg(feature = "source-tools")]
mod source_bibliographic_source;
#[cfg(feature = "source-tools")]
mod source_bibliographic_unicode;
#[cfg(feature = "source-tools")]
mod source_bibliographic_values;
#[cfg(feature = "source-tools")]
mod source_bibliographic_versions;
#[cfg(feature = "source-tools")]
pub mod source_navigation_claim_event;
#[cfg(feature = "source-tools")]
pub mod source_navigation_event;
#[cfg(feature = "source-tools")]
pub mod source_navigation_record_range;
#[cfg(feature = "source-tools")]
pub use managed_model_manifest::{
    ManagedManifestDeltaV2, ManagedManifestLimitsV2, ManagedManifestRecoveryRebindV2,
    ManagedManifestV2, ManagedOverlaySourceBindingV2,
};
#[cfg(feature = "source-tools")]
mod versions_catalog_epoch;
#[cfg(feature = "source-tools")]
pub use versions_catalog_epoch::VersionsCatalogEpochV2;
#[cfg(feature = "source-tools")]
pub mod source_corpus;
#[cfg(feature = "source-tools")]
mod source_navigation_packets;
#[cfg(feature = "source-tools")]
pub mod source_navigation_source;
#[cfg(feature = "source-tools")]
pub mod source_philosophy_atlas;
#[cfg(feature = "source-tools")]
pub mod source_philosophy_dossier_docx;
#[cfg(feature = "source-tools")]
pub mod source_philosophy_dossier_extract;
#[cfg(feature = "source-tools")]
pub mod source_philosophy_graph;
#[cfg(feature = "source-tools")]
pub mod source_philosophy_multilingual;
#[cfg(feature = "source-tools")]
pub mod source_philosophy_post_planting;
mod source_philosophy_support;
#[cfg(feature = "source-tools")]
pub mod source_registry;
pub use source_philosophy_support::PhilosophySourceReadProfile;
#[cfg(feature = "source-tools")]
pub mod source_philosophy_views;
#[cfg(feature = "source-tools")]
pub mod source_witness_catalog;
mod sqlite_budget;
pub use sqlite_budget::{DedicatedSessionSqliteHeap, dedicated_session_heap_bytes};
mod vocabulary;
#[cfg(feature = "source-tools")]
pub use knowledge_base::{BaseNodeOverrides, BaseNormalizationLimits, KnowledgeBaseNormalizer};
#[cfg(feature = "source-tools")]
pub use knowledge_catalog_index::{CatalogIndexLimits, CatalogIndexReceipt, materialize_catalog};
#[cfg(feature = "source-tools")]
pub use knowledge_full::{
    FullKnowledgeLimits, FullKnowledgeReceipt, compile_full_knowledge_components,
    managed_navigation_header_with_binding,
};
#[cfg(feature = "source-tools")]
pub use knowledge_global_titles::{
    CompleteBaseNodes, GlobalTitleLimits, GlobalTitleReceipt, clear_global_titles, endpoint_title,
    order_native_graph_rows, prepare_global_titles, verify_global_titles,
};
#[cfg(feature = "source-tools")]
pub use knowledge_indexed::{
    IndexedLimits, IndexedReceipt, materialize_indexed_sources,
    materialize_registered_indexed_nodes, materialize_registered_indexed_relations,
};
#[cfg(feature = "source-tools")]
pub use knowledge_inherited_views::{
    CompleteRelationSeal, InheritedViewLimits, InheritedViewReceipt, clear_inherited_views,
    endpoint_inherited_views, prepare_global_inherited_views,
};
#[cfg(feature = "source-tools")]
pub use knowledge_native::{
    NativeFamilyInputs, NativeProducerLimits, NativeProducerReceipt, materialize_native_sources,
    materialize_native_sources_with_inputs,
};
#[cfg(feature = "source-tools")]
pub use knowledge_native_finalize::{
    NativeFinalizeLimits, NativeFinalizeReceipt, finalize_native_graph_rows,
    finalize_native_graph_rows_with_witnesses,
};
#[cfg(feature = "source-tools")]
pub use knowledge_navigation_materialize::{
    NavigationMaterializeLimits, NavigationNodeMaterializeReceipt, NavigationPlaceholderReceipt,
    NavigationRelationMaterializeReceipt, materialize_navigation_nodes,
    materialize_navigation_placeholders, materialize_navigation_relations,
};
#[cfg(feature = "source-tools")]
pub use knowledge_philosophy_materialize::{
    PhilosophyBase, PhilosophyMaterializeLimits, PhilosophyMaterializeReceipt,
    PhilosophyNormalizer, PhilosophyRelationGlobalInputs, materialize_philosophy_nodes,
    materialize_philosophy_relations,
};
#[cfg(feature = "source-tools")]
pub use knowledge_philosophy_prepare::{
    PhilosophyExternalDependency, PhilosophyPrepareLimits, PhilosophyPrepareReceipt,
    clear_philosophy_prepare, prepare_philosophy,
};
#[cfg(feature = "source-tools")]
pub use knowledge_readable_context::{
    ReadableContextCarrier, ReadableContextCompiler, ReadableContextLimits,
    ordered_readable_witness, ordered_readable_witness_bounded,
};
pub use knowledge_registry::{KnowledgeRegistry, ResolvedType};
#[cfg(feature = "source-tools")]
pub use knowledge_repository::{
    RepositoryPrepareReceipt, RepositoryRootInput, TopologyLimits, clear_repository_topology,
    materialize_repository_nodes, materialize_repository_relations, prepare_repository_topology,
    repository_material_witness, scan_repository_relation_sources,
};
#[cfg(feature = "source-tools")]
pub use knowledge_scope::{ScopeLimits, ScopeReceipt, write_source_scope};
#[cfg(feature = "source-tools")]
pub use knowledge_seal::{SealLimits, seal_knowledge_model};
#[cfg(feature = "source-tools")]
pub use knowledge_search::{SearchBuildLimits, SearchIndexReceipt, build_search_index};
pub use knowledge_selected::{
    ColdOpenLimits, ExpectedSourceScope, ImmutableKnowledgeCustody, KNOWLEDGE_MODEL_ABI,
    KnowledgeSealReceipt, KnowledgeSelectedExpectation, VerifiedKnowledgeModel,
    open_selected_knowledge_model, open_selected_knowledge_model_owned,
};
#[cfg(feature = "source-tools")]
pub use knowledge_semantic_join::{
    SemanticJoinReceipt, clear_semantic_joins, materialize_semantic_relations,
    prepare_semantic_joins, render_supplied_shared_identity_pair,
};
#[cfg(feature = "source-tools")]
pub use knowledge_source_claims_prepare::{
    ClaimExternalDependency, ClaimPrepareLimits, ClaimPrepareReceipt, prepare_source_claims,
};
#[cfg(feature = "source-tools")]
pub use knowledge_source_navigation_node::{
    NavigationBaseNode, NavigationEndpoint, NavigationNodeLimits, NavigationNodeNormalizer,
    NavigationPlaceholderBase,
};
#[cfg(feature = "source-tools")]
pub use knowledge_source_navigation_prepare::{
    NavigationExternalDependency, NavigationHeaderClaim, NavigationPrepareLimits,
    NavigationPrepareReceipt, prepare_source_navigation,
};
#[cfg(feature = "source-tools")]
pub use knowledge_source_navigation_relation::{
    NavigationBaseRelation, NavigationRelationDependencyReceipt, NavigationRelationGlobalInputs,
    NavigationRelationLimits, NavigationRelationNormalizeLimits, NavigationRelationNormalizer,
    direct_assertion_context, prepare_navigation_relation_dependencies,
};
pub use legacy::partitioned_projection_closure;
#[cfg(feature = "source-tools")]
pub use native_knowledge_selection::prepare_native_knowledge_artifact;
pub use native_knowledge_selection::{
    LinuxFsVerityCustody, NativeFsVerityMeasurement, NativeKnowledgeSelection, NativeProcessLimits,
    NativeSelectionPaths, NativeSelectionProducer,
    measure_existing_native_knowledge_artifact_custody,
};
#[cfg(feature = "source-tools")]
pub use source_bibliographic_selected_files::SelectedBibliographicFilesV1;
#[cfg(feature = "source-tools")]
pub use source_bibliographic_source::{
    CandidateSourceCatalogInputPlan, ColdSourceCatalogInputPlan, ColdSourceCatalogSpoolLimits,
    SourceBibliographicCandidate, SourceCatalogInputLimits, SourceCatalogInputPlan,
    SourceCatalogRenderWorkV1, plan_candidate_source_catalog_inputs_spooled,
    plan_candidate_source_catalog_inputs_with_workspace, plan_cold_source_catalog_inputs,
    plan_cold_source_catalog_inputs_with_workspace, plan_source_catalog_inputs,
    prepare_candidate_source_catalog_plan_observed, prepare_cold_source_catalog_plan,
    prepare_cold_source_catalog_plan_observed, prepare_source_catalog_plan,
    render_source_bibliographic_plan, render_source_bibliographic_plan_with_work,
};
#[cfg(feature = "source-tools")]
pub use source_bibliographic_versions::StreamedBibliographicReadLedger;
#[cfg(feature = "source-tools")]
pub use source_bibliographic_versions::{
    SelectedClaimVersionV1, SelectedMetadataHistoryV1, SelectedMetadataVersionSourceV1,
    resolve_selected_claim_version_v1, resolve_selected_metadata_archive_forms_v1,
    resolve_selected_metadata_history_v1, selected_claim_archive_blob_paths_v1,
    selected_claim_archive_manifest_paths_v1, selected_metadata_archive_forms_blob_path_v1,
    selected_metadata_archive_manifest_paths_v1, selected_metadata_archive_record_blob_path_v1,
    selected_metadata_record_schema_path_v1,
};
pub use vocabulary::{
    NATIVE_KNOWLEDGE_ADAPTER_PROFILES, QueryVocabulary, RegisteredSource, VocabularyBinding,
};

use std::{fmt, io::Read, path::Path};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue};
#[cfg(test)]
use tos_foundation::{JsonLimits, JsonMode, parse_json};

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
    FoundationJson {
        code: tos_foundation::FoundationErrorCode,
        message: &'static str,
        byte_offset: Option<usize>,
    },
    SqliteVmBudget {
        phase: knowledge_stage::WritePhase,
        used_steps: u64,
        max_steps: u64,
    },
    ControlledColdClose {
        operation: Option<ColdOperationFailure>,
        close: rusqlite::Error,
    },
}

/// Non-recursive evidence retained when an opened controlled cold database
/// reports both an operation failure and an explicit SQLite close failure.
#[derive(Debug)]
pub enum ColdOperationFailure {
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
    FoundationJson {
        code: tos_foundation::FoundationErrorCode,
        message: &'static str,
        byte_offset: Option<usize>,
    },
    SqliteVmBudget {
        phase: knowledge_stage::WritePhase,
        used_steps: u64,
        max_steps: u64,
    },
    NestedControlledColdClose,
}
impl fmt::Display for ColdOperationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O: {error}"),
            Self::Sql(error) => write!(f, "SQLite: {error}"),
            Self::SqlitePhase { phase, error } => write!(f, "SQLite in {phase:?}: {error}"),
            Self::Invalid(message) => write!(f, "invalid compiler input: {message}"),
            Self::PreparedUnsupported(message) => {
                write!(f, "unsupported local prepared carrier/profile: {message}")
            }
            Self::ManagedSourceUnsupported(message) => {
                write!(f, "unsupported managed selected source: {message}")
            }
            Self::Source(message) => write!(f, "source carrier: {message}"),
            Self::Budget(message) => write!(f, "compiler budget exceeded: {message}"),
            Self::FoundationJson {
                code,
                message,
                byte_offset,
            } => write_foundation_json_failure(f, *code, message, *byte_offset),
            Self::SqliteVmBudget {
                phase,
                used_steps,
                max_steps,
            } => write!(
                f,
                "compiler budget exceeded: SQLite VM steps in {phase:?} (used {used_steps}, max {max_steps})"
            ),
            Self::NestedControlledColdClose => {
                f.write_str("nested controlled cold operation and close failure")
            }
        }
    }
}
impl std::error::Error for ColdOperationFailure {}
impl From<Error> for ColdOperationFailure {
    fn from(error: Error) -> Self {
        match error {
            Error::Io(error) => Self::Io(error),
            Error::Sql(error) => Self::Sql(error),
            Error::SqlitePhase { phase, error } => Self::SqlitePhase { phase, error },
            Error::Invalid(message) => Self::Invalid(message),
            Error::PreparedUnsupported(message) => Self::PreparedUnsupported(message),
            Error::ManagedSourceUnsupported(message) => Self::ManagedSourceUnsupported(message),
            Error::Source(message) => Self::Source(message),
            Error::Budget(message) => Self::Budget(message),
            Error::FoundationJson {
                code,
                message,
                byte_offset,
            } => Self::FoundationJson {
                code,
                message,
                byte_offset,
            },
            Error::SqliteVmBudget {
                phase,
                used_steps,
                max_steps,
            } => Self::SqliteVmBudget {
                phase,
                used_steps,
                max_steps,
            },
            Error::ControlledColdClose { .. } => Self::NestedControlledColdClose,
        }
    }
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
            Self::FoundationJson {
                code,
                message,
                byte_offset,
            } => write_foundation_json_failure(f, *code, message, *byte_offset),
            Self::SqliteVmBudget {
                phase,
                used_steps,
                max_steps,
            } => write!(
                f,
                "compiler budget exceeded: SQLite VM steps in {phase:?} (used {used_steps}, max {max_steps})"
            ),
            Self::ControlledColdClose { operation, close } => match operation {
                Some(operation) => write!(
                    f,
                    "controlled cold operation failed ({operation}); explicit SQLite close failed ({close})"
                ),
                None => write!(f, "explicit controlled cold SQLite close failed ({close})"),
            },
        }
    }
}
impl std::error::Error for Error {}

fn write_foundation_json_failure(
    f: &mut fmt::Formatter<'_>,
    code: tos_foundation::FoundationErrorCode,
    message: &str,
    byte_offset: Option<usize>,
) -> fmt::Result {
    match byte_offset {
        Some(offset) => write!(
            f,
            "Foundation JSON {} at byte {offset}: {message}",
            code.as_str()
        ),
        None => write!(f, "Foundation JSON {}: {message}", code.as_str()),
    }
}

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
/// Maintained source-navigation visibility rule; this does not grant disclosure rights.
pub fn visible_node(row: &JsonValue) -> bool {
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
/// Exact private-source structural/paragraph producer; no semantic admission.
#[cfg(feature = "research")]
pub mod antonovsky_structural;

#[cfg(feature = "source-tools")]
pub mod native_prepare;

/// Finite owned external-tool I/O for exact native producers.
#[cfg(feature = "source-tools")]
pub mod owned_native_child;

#[cfg(feature = "research")]
pub mod zarathustra_lexical;
#[cfg(feature = "research")]
pub mod zarathustra_lexical_schema;
#[cfg(feature = "research")]
pub mod zarathustra_lexical_validate;

#[cfg(feature = "research")]
mod zarathustra_lexical_morphology_validate;
#[cfg(feature = "research")]
mod zarathustra_lexical_usage_validate;

#[cfg(feature = "research")]
pub mod dta_technical_markup;
#[cfg(feature = "research")]
pub mod opening_sentence_alignment;
#[cfg(feature = "research")]
pub mod research_concept_workbench;
#[cfg(feature = "research")]
pub mod research_eternal_return;
#[cfg(feature = "research")]
pub mod research_eternal_return_concept;
#[cfg(feature = "source-tools")]
pub mod research_execution;
#[cfg(feature = "research")]
pub mod research_morphology_theme;
#[cfg(feature = "research")]
pub mod research_paragraph_alignment;
#[cfg(feature = "research")]
pub mod research_parallel_lexical;
#[cfg(feature = "research")]
pub mod source_text_foundation;
#[cfg(feature = "research")]
pub mod target_text_foundation;
#[cfg(feature = "research")]
pub mod transfer_route_readiness;

#[cfg(feature = "research")]
mod research_reading_discourse;
#[cfg(feature = "research")]
mod research_reading_formulas;
#[cfg(feature = "research")]
pub mod research_reading_workbench;

mod retained_state;

// Explicit layout is admitted only by the original owned whole producer.
#[cfg(feature = "source-tools")]
pub use native_snapshot::{
    NativeSnapshotCreationUsage, NativeSnapshotOwnedBudget, NativeSnapshotOwnedReadLoan,
    build_native_knowledge_snapshot_from_capture_with_owned_budget_and_layout,
    with_native_knowledge_snapshot_from_capture_with_owned_budget_and_layout,
};

#[cfg(feature = "research")]
pub mod transfer_source_passages;
#[cfg(feature = "research")]
pub mod transfer_target_passages;

#[cfg(feature = "research")]
pub mod transfer_candidates;

#[cfg(feature = "source-tools")]
pub mod html_entities;
#[cfg(feature = "research")]
pub mod research_html;
#[cfg(feature = "research")]
mod research_text_comparison;
#[cfg(feature = "research")]
pub mod transfer_source_visible;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "research")]
pub mod german_triangulation;

#[cfg(feature = "research")]
pub mod bounded_translation_input;

#[cfg(feature = "research")]
pub mod antonovsky_collation;

#[cfg(feature = "research")]
pub mod synthetic_foundation_labs;

#[cfg(feature = "research")]
pub mod authored_canon_bridge;

#[cfg(feature = "research")]
pub mod provenance_event_lab;

#[cfg(feature = "source-tools")]
pub mod constructor_library;
#[cfg(feature = "research")]
pub mod jenseits_label_correspondence;

#[cfg(feature = "research")]
pub mod nietzsche_transfer_source_routes;

#[cfg(feature = "research")]
pub mod jenseits_numbered_structure;

#[cfg(feature = "research")]
pub mod jenseits_polilov_numbered_structure;

#[cfg(feature = "research")]
pub mod mysl_transfer_target_structure;
#[cfg(feature = "research")]
pub mod nietzsche_transfer_source_structure;

#[cfg(feature = "research")]
pub mod witness_structure_correspondence;

#[cfg(feature = "research")]
pub mod lexical_derivatives;

#[cfg(feature = "research")]
pub mod research_visual_result;
