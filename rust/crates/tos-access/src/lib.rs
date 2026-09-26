//! Native transport adapters over owner-selected Rust query sessions.
//!
//! The operation descriptor is an API-owned candidate. Presence in the
//! descriptor never advertises a capability: a trusted selected session and
//! current-rights fence must be installed by the source owner first.

pub mod cli;
pub mod knowledge;
pub use knowledge::{KnowledgeOperation, KnowledgeRequest};
mod common;
pub mod http;
pub mod mcp;

pub use common::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence,
    IndexedSearchParams, Params, PreparedPacket, QuerySession, descriptor,
};
