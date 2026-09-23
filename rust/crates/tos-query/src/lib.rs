//! Bounded, snapshot-bound query rules over a selected read model.
//!
//! The first complete family is source-navigation descent. The adapter must
//! expose a sealed, visibility-filtered local read model; this crate never
//! treats projection presence as source authority or current rights.

mod source_descend;
#[cfg(not(target_arch = "wasm32"))]
mod sqlite;

pub use source_descend::{
    AdjacencyPage, Binding, Budget, Charged, DisclosableSourceDescend, DisclosureLease, ExactNode,
    QueryError, QueryErrorCode, RawRecord, ReadModel, SourceDescendRequest, source_descend,
};
#[cfg(not(target_arch = "wasm32"))]
pub use sqlite::{
    AdapterAdmissionBudget, AdapterAdmissionCharge, CmpPinnedModel, CmpSqliteReadModel,
    CurrentPolicy, PinnedLocalModel, SourcePin, SqliteReadModel,
};
