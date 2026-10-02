//! Native transport adapters over owner-selected Rust query sessions.
//!
//! The operation descriptor is an API-owned candidate. Presence in the
//! descriptor never advertises a capability: a trusted selected session and
//! current-rights fence must be installed by the source owner first.

pub mod cli;
pub mod concept_search;
pub mod doctor;
pub mod edge_sql;
pub mod exploration_checkpoints;
pub mod exploration_contracts;
mod indexed_cursor;
pub mod knowledge;
pub mod managed_local;
#[cfg(not(target_arch = "wasm32"))]
pub mod managed_source_query;
pub mod persistent_exploration_checkpoints;
pub mod prepared_local;
pub mod prepared_maintenance;
pub mod prepared_publication;
pub mod public_d1_build;
pub mod reading;
pub mod release_state;
pub mod word_analysis;
pub use knowledge::{KnowledgeOperation, KnowledgeRequest};
mod common;
pub mod http;
pub mod mcp;
pub mod mcp_http;
mod mcp_prompts;
mod mcp_resources;
pub mod search;
mod selected_source;
pub mod site;
pub mod software_archive;
pub mod source_read;

pub use common::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence,
    IndexedSearchParams, NoOwner, Params, PreparedPacket, QuerySession, RegisteredOperation,
    SEARCH_OPERATION_ID, checked_execute, descriptor, registered_operations,
};

pub mod native_prepare;

pub mod capture_restore;

pub mod evidence_projection;
#[cfg(not(target_arch = "wasm32"))]
pub mod lexical_index_command;
pub mod structural_paragraph_command;

#[cfg(not(target_arch = "wasm32"))]
pub mod research_builders_command;
