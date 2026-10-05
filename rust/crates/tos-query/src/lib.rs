//! Bounded, snapshot-bound query rules over a selected read model.
//!
//! The first complete family is source-navigation descent. The adapter must
//! expose a sealed, visibility-filtered local read model; this crate never
//! treats projection presence as source authority or current rights.

#[cfg(not(target_arch = "wasm32"))]
pub mod compressed_search;
#[cfg(not(target_arch = "wasm32"))]
mod compressed_search_sqlite;
#[cfg(not(target_arch = "wasm32"))]
mod compressed_search_state;
#[cfg(not(target_arch = "wasm32"))]
pub mod corpus_read;
pub mod exploration_plan;
mod inspect_plan;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_binding;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_catalog;
pub mod knowledge_contracts;
pub mod knowledge_exploration;
pub mod knowledge_focus;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_inspect;
#[cfg(not(target_arch = "wasm32"))]
pub mod knowledge_legacy_search;
pub mod knowledge_lens;
pub mod knowledge_lens_spec;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_packet;
pub mod knowledge_presentation;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_sqlite;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_temporal;
#[cfg(not(target_arch = "wasm32"))]
mod controlled_query_adapter;
#[cfg(not(target_arch = "wasm32"))]
mod controlled_inspect_adapter;
#[cfg(not(target_arch = "wasm32"))]
mod controlled_lens_budget;
#[cfg(not(target_arch = "wasm32"))]
mod controlled_lens_adapter;
#[cfg(not(target_arch = "wasm32"))]
pub use controlled_lens_adapter::{execute_controlled_lens_response,
    execute_controlled_lens_request_response, ControlledLensRequest};
#[cfg(not(target_arch = "wasm32"))]
mod controlled_original_reader;
#[cfg(not(target_arch = "wasm32"))]
mod controlled_philosophy_adapter;
#[cfg(not(target_arch = "wasm32"))]
pub use controlled_philosophy_adapter::{execute_controlled_philosophy_status_response, execute_controlled_philosophy_status_response_render, execute_controlled_philosophy_metadata_response_render};
pub mod lens_plan;
#[cfg(not(target_arch = "wasm32"))]
pub mod philosophy_read;
#[cfg(not(target_arch = "wasm32"))]
pub mod prepared_exploration;
#[cfg(not(target_arch = "wasm32"))]
mod prepared_exploration_rows;
#[cfg(not(target_arch = "wasm32"))]
mod prepared_inspect;
#[cfg(not(target_arch = "wasm32"))]
mod prepared_lens;
#[cfg(not(target_arch = "wasm32"))]
mod prepared_operations;
pub mod search_candidate;
mod search_document;
mod search_execute;
pub mod search_index;
pub mod search_v2;
mod source_descend;
#[cfg(not(target_arch = "wasm32"))]
pub mod source_diagnostic;
#[cfg(not(target_arch = "wasm32"))]
pub mod source_dossier;
#[cfg(not(target_arch = "wasm32"))]
pub mod source_gap;
mod source_read_projection;
#[cfg(not(target_arch = "wasm32"))]
mod sqlite;
mod temporal_comparison;
pub use inspect_plan::{
    AbortProbe, AbortReason, InspectBudget, InspectNeed, InspectPlan, InspectRequest,
    validate_inspect_request,
};

#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_binding::{
    BoundCmpKnowledge, bind_managed_proof_verified_knowledge, bind_managed_verified_knowledge,
    bind_verified_knowledge,
};
#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_catalog::{
    CATALOG_CARRIER_LAYER, CATALOG_INTENDED_USE, CATALOG_OPERATION_ID, CatalogBudget,
    CatalogCurrentAuthority, CatalogDisclosureLease, CatalogDisclosureScope, CatalogError,
    CatalogErrorCode, DisclosableCatalog, execute_selected_catalog,
    execute_selected_knowledge_health_metadata,
};
#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_inspect::{
    DisclosableInspect, INSPECT_INTENDED_USE, InspectCurrentAuthority, InspectDisclosureLease,
    InspectVisitMeter, InspectedCarrier, NODE_INSPECT_OPERATION, ObservedInspectCarrier,
    RELATION_INSPECT_OPERATION, execute_selected_inspect, execute_selected_knowledge_header,
};
#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_packet::{
    DisclosableIndexedSearch, DisclosableScopedIndexedSearch, INDEXED_SEARCH_INTENDED_USE, INDEXED_SEARCH_OPERATION_ID,
    IndexedDisclosureLease, IndexedDisclosureScope, IndexedKnowledgeAuthority, IndexedPageBudget,
    IndexedWireCursorCodec, ScopedIndexedKnowledgeAuthority, execute_indexed_search_page, execute_scoped_indexed_search_page,
};
#[cfg(not(target_arch = "wasm32"))]
pub use controlled_query_adapter::{
    execute_controlled_catalog_response,
    execute_controlled_knowledge_header_response,
    execute_scoped_controlled_indexed_search_response,
    execute_scoped_controlled_sidecar_indexed_search_response,
    with_controlled_knowledge_binding, with_controlled_sidecar_knowledge_binding,
};
#[cfg(not(target_arch = "wasm32"))]
pub use controlled_query_adapter::indexed_limit_from_foundation;
#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_temporal::execute_selected_temporal;
pub use search_document::{
    SearchDocumentBudget, VerifiedSearchDocument, verify_indexed_search_document,
};
pub use search_execute::{ObservedSearchCandidate, SearchKindBudget};
pub use source_descend::{
    AdjacencyPage, Binding, Budget, Charged, DisclosableSourceDescend, DisclosureLease, ExactNode,
    QueryError, QueryErrorCode, RawRecord, ReadModel, SOURCE_DESCEND_D1_METER_V1,
    SOURCE_DESCEND_SESSION_V1, SessionAdvance, SessionCaps, SessionNeed, SessionNeedKind,
    SessionResponse, SessionResponseKind, SourceDescendRequest, SourceDescendSession,
    source_descend,
};
#[cfg(not(target_arch = "wasm32"))]
pub use sqlite::{
    AdapterAdmissionBudget, AdapterAdmissionCharge, CmpPinnedModel, CmpSqliteReadModel,
    CurrentPolicy, DisclosureScope, PinnedLocalModel, SourcePin, SqliteReadModel,
};
pub use temporal_comparison::{
    TEMPORAL_INTENDED_USE, TEMPORAL_OPERATION, compare_temporal_operands, validate_temporal_request,
};

#[cfg(not(target_arch = "wasm32"))]
pub mod reading_search;

#[cfg(not(target_arch = "wasm32"))]
pub use controlled_inspect_adapter::execute_controlled_inspect_response;

#[cfg(not(target_arch = "wasm32"))]
mod controlled_corpus_adapter;
#[cfg(not(target_arch = "wasm32"))]
pub use controlled_corpus_adapter::{execute_controlled_corpus_metadata_response, execute_controlled_corpus_response};
#[cfg(not(target_arch = "wasm32"))]
mod controlled_corpus_reader;
#[cfg(not(target_arch = "wasm32"))]
mod controlled_philosophy_domain;
#[cfg(not(target_arch = "wasm32"))]
pub use controlled_philosophy_domain::execute_controlled_philosophy_domain_response;
#[cfg(not(target_arch = "wasm32"))]
mod controlled_health_children;
#[cfg(not(target_arch = "wasm32"))]
pub use controlled_health_children::{ControlledHealthSeed, execute_controlled_health_seed_response,
    execute_controlled_knowledge_health_response};
