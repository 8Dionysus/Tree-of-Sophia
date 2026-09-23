//! Bounded, snapshot-bound query rules over a selected read model.
//!
//! The first complete family is source-navigation descent. The adapter must
//! expose a sealed, visibility-filtered local read model; this crate never
//! treats projection presence as source authority or current rights.

#[cfg(not(target_arch = "wasm32"))]
mod knowledge_binding;
#[cfg(not(target_arch = "wasm32"))]
mod knowledge_sqlite;
pub mod search_candidate;
mod search_document;
mod search_execute;
pub mod search_index;
pub mod search_v2;
mod source_descend;
#[cfg(not(target_arch = "wasm32"))]
mod sqlite;

#[cfg(not(target_arch = "wasm32"))]
pub use knowledge_binding::{BoundCmpKnowledge, bind_verified_knowledge};
pub use search_document::{
    SearchDocumentBudget, VerifiedSearchDocument, verify_indexed_search_document,
};
pub use source_descend::{
    AdjacencyPage, Binding, Budget, Charged, DisclosableSourceDescend, DisclosureLease, ExactNode,
    QueryError, QueryErrorCode, RawRecord, ReadModel, SOURCE_DESCEND_D1_METER_V1,
    SOURCE_DESCEND_SESSION_V1, SessionAdvance, SessionCaps, SessionNeed, SessionNeedKind,
    SessionResponse, SessionResponseKind, SourceDescendRequest, SourceDescendSession,
    source_descend,
};
#[cfg(not(target_arch = "wasm32"))]
pub use sqlite::{
    AbortProbe, AbortReason, AdapterAdmissionBudget, AdapterAdmissionCharge, CmpPinnedModel,
    CmpSqliteReadModel, CurrentPolicy, DisclosureScope, PinnedLocalModel, SourcePin,
    SqliteReadModel,
};
