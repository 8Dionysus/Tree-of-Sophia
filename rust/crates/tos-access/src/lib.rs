//! Native transport adapters over owner-selected Rust query sessions.
//!
//! The operation descriptor is an API-owned candidate. Presence in the
//! descriptor never advertises a capability: a trusted selected session and
//! current-rights fence must be installed by the source owner first.

pub mod cli;
pub mod doctor;
pub mod exploration_checkpoints;
pub mod exploration_contracts;
mod indexed_cursor;
pub mod knowledge;
pub mod managed_local;
pub mod prepared_publication;
pub mod public_d1_build;
pub mod prepared_local;
pub mod release_state;
pub use knowledge::{KnowledgeOperation, KnowledgeRequest};
mod common;
pub mod http;
pub mod mcp;
pub mod search;
pub mod site;
pub mod software_archive;

pub use common::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence,
    IndexedSearchParams, NoOwner, Params, PreparedPacket, QuerySession, RegisteredOperation,
    SEARCH_OPERATION_ID, checked_execute, descriptor, registered_operations,
};
