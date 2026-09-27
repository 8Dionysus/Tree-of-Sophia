//! Bounded, snapshot-bound query rules over a selected read model.
//!
//! The first complete family is source-navigation descent. The adapter must
//! expose a sealed, visibility-filtered local read model; this crate never
//! treats projection presence as source authority or current rights.

#[cfg(not(target_arch = "wasm32"))]
mod knowledge_binding;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_catalog;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_inspect;
pub mod knowledge_lens_spec;
pub mod knowledge_contracts;
pub mod knowledge_focus;
pub mod knowledge_lens;
pub mod lens_plan;
pub mod knowledge_exploration;
pub mod exploration_plan;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_packet;
pub mod knowledge_presentation;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_sqlite;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_temporal;
#[cfg(not(target_arch = "wasm32"))]
pub mod knowledge_legacy_search;
pub mod search_candidate;
mod search_document;
mod search_execute;
pub mod search_index;
pub mod search_v2;
mod source_descend;
mod source_read_projection;
#[cfg(not(target_arch = "wasm32"))]
pub mod source_dossier;
#[cfg(not(target_arch = "wasm32"))]
pub mod philosophy_read;
#[cfg(not(target_arch = "wasm32"))]
pub mod corpus_read;
#[cfg(not(target_arch = "wasm32"))]
pub mod source_gap;
#[cfg(not(target_arch = "wasm32"))]
mod sqlite;
mod temporal_comparison;
mod inspect_plan;
pub use inspect_plan::{AbortProbe, AbortReason, InspectBudget, InspectRequest, InspectNeed, InspectPlan, validate_inspect_request};

#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_binding::{BoundCmpKnowledge, bind_verified_knowledge, bind_managed_verified_knowledge};
#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_catalog::{
    CATALOG_CARRIER_LAYER, CATALOG_INTENDED_USE, CATALOG_OPERATION_ID,
    CatalogBudget, CatalogCurrentAuthority, CatalogDisclosureLease, CatalogDisclosureScope,
    CatalogError, CatalogErrorCode, DisclosableCatalog, execute_selected_catalog,
};
#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_inspect::{
    DisclosableInspect, INSPECT_INTENDED_USE, InspectCurrentAuthority,
    InspectDisclosureLease, InspectedCarrier, NODE_INSPECT_OPERATION, ObservedInspectCarrier,
    RELATION_INSPECT_OPERATION, execute_selected_inspect,
};
#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_packet::{
    DisclosableIndexedSearch, IndexedDisclosureLease, IndexedDisclosureScope,
    IndexedKnowledgeAuthority, IndexedPageBudget, IndexedWireCursorCodec,
    execute_indexed_search_page,
};
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
    AdapterAdmissionBudget, AdapterAdmissionCharge, CmpPinnedModel,
    CmpSqliteReadModel, CurrentPolicy, DisclosureScope, PinnedLocalModel, SourcePin,
    SqliteReadModel,
};
pub use temporal_comparison::{
    TEMPORAL_INTENDED_USE, TEMPORAL_OPERATION, compare_temporal_operands, validate_temporal_request,
};
