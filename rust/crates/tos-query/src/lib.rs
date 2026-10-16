//! Bounded, snapshot-bound query rules over a selected read model.
//!
//! The first complete family is source-navigation descent. The adapter must
//! expose a sealed, visibility-filtered local read model; this crate never
//! treats projection presence as source authority or current rights.

#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod compressed_search;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod compressed_search_sqlite;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod compressed_search_state;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_inspect_adapter;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_lens_adapter;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_lens_budget;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_query_adapter;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod corpus_read;
pub mod exploration_plan;
mod inspect_plan;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod knowledge_binding;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod knowledge_catalog;
pub mod knowledge_contracts;
pub mod knowledge_exploration;
pub mod knowledge_focus;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod knowledge_inspect;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod knowledge_legacy_search;
pub mod knowledge_lens;
pub mod knowledge_lens_spec;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod knowledge_packet;
pub mod knowledge_presentation;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod knowledge_sqlite;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod knowledge_temporal;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_lens_adapter::{
    ControlledLensRequest, execute_controlled_lens_request_response,
    execute_controlled_lens_response,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_original_reader;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_philosophy_adapter;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_philosophy_adapter::{
    execute_controlled_philosophy_metadata_response_render,
    execute_controlled_philosophy_status_response,
    execute_controlled_philosophy_status_response_render,
};
pub mod lens_plan;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod philosophy_read;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod prepared_exploration;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod prepared_exploration_rows;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod prepared_inspect;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod prepared_lens;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod prepared_operations;
pub mod search_candidate;
mod search_document;
mod search_execute;
pub mod search_index;
pub mod search_v2;
mod source_descend;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod source_diagnostic;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod source_dossier;
pub mod source_gap;
mod source_read_projection;
pub mod source_text_search;
pub mod source_value_search;
mod temporal_comparison;
pub mod worker_search_controls;
pub use inspect_plan::{
    AbortProbe, AbortReason, InspectBudget, InspectNeed, InspectPlan, InspectRequest,
    validate_inspect_request,
};

#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_query_adapter::indexed_limit_from_foundation;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_query_adapter::{
    execute_controlled_catalog_response, execute_controlled_knowledge_header_response,
    execute_scoped_controlled_indexed_search_response,
    execute_scoped_controlled_sidecar_indexed_search_response, with_controlled_knowledge_binding,
    with_controlled_sidecar_knowledge_binding,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use knowledge_binding::{
    BoundCmpKnowledge, bind_managed_proof_verified_knowledge, bind_managed_verified_knowledge,
    bind_verified_knowledge,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use knowledge_catalog::{
    CATALOG_CARRIER_LAYER, CATALOG_INTENDED_USE, CATALOG_OPERATION_ID, CatalogBudget,
    CatalogCurrentAuthority, CatalogDisclosureLease, CatalogDisclosureScope, CatalogError,
    CatalogErrorCode, DisclosableCatalog, execute_selected_catalog,
    execute_selected_knowledge_health_metadata,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use knowledge_inspect::{
    DisclosableInspect, INSPECT_INTENDED_USE, InspectCurrentAuthority, InspectDisclosureLease,
    InspectVisitMeter, InspectedCarrier, NODE_INSPECT_OPERATION, ObservedInspectCarrier,
    RELATION_INSPECT_OPERATION, execute_selected_inspect, execute_selected_knowledge_header,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use knowledge_packet::{
    DisclosableIndexedSearch, DisclosableScopedIndexedSearch, INDEXED_SEARCH_INTENDED_USE,
    INDEXED_SEARCH_OPERATION_ID, IndexedDisclosureLease, IndexedDisclosureScope,
    IndexedKnowledgeAuthority, IndexedPageBudget, IndexedWireCursorCodec,
    ScopedIndexedKnowledgeAuthority, execute_indexed_search_page,
    execute_scoped_indexed_search_page,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use knowledge_temporal::execute_selected_temporal;
pub use search_document::{
    SearchDocumentBudget, VerifiedSearchDocument, verify_indexed_search_document,
};
pub use search_execute::{ObservedSearchCandidate, SearchKindBudget};
pub use source_descend::{
    AdjacencyPage, Binding, Budget, Charged, CpuStepsMeter, CurrentPolicy,
    DisclosableSourceDescend, DisclosureLease, DisclosureScope, ExactNode, QueryError,
    QueryErrorCode, RawRecord, ReadModel, SOURCE_DESCEND_D1_METER_V1,
    SOURCE_DESCEND_POSTGRES_METER_V1, SOURCE_DESCEND_SESSION_V1, SOURCE_DESCEND_SESSION_V2,
    SOURCE_NAVIGATION_MODEL_ABI, SOURCE_NAVIGATION_SELECTION_PROFILE, SessionAdvance, SessionCaps,
    SessionNeed, SessionNeedKind, SessionResponse, SessionResponseKind, SourceDescendRequest,
    SourceDescendSession, SourceNavigationProofV1, source_descend,
};
pub use temporal_comparison::{
    TEMPORAL_INTENDED_USE, TEMPORAL_OPERATION, compare_temporal_operands, validate_temporal_request,
};

#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub mod reading_search;

#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_inspect_adapter::execute_controlled_inspect_response;

#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_corpus_adapter;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_corpus_adapter::{
    execute_controlled_corpus_metadata_response, execute_controlled_corpus_response,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_corpus_reader;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_philosophy_domain;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_philosophy_domain::execute_controlled_philosophy_domain_response;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
mod controlled_health_children;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
pub use controlled_health_children::{
    ControlledHealthSeed, execute_controlled_health_seed_response,
    execute_controlled_knowledge_health_response,
};
