//! Native transport adapters over owner-selected Rust query sessions.
//!
//! The operation descriptor is an API-owned candidate. Presence in the
//! descriptor never advertises a capability: a trusted selected session and
//! current-rights fence must be installed by the source owner first.

pub mod cli;
pub mod concept_search;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod coverage;
pub mod doctor;
#[cfg(target_os = "linux")]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod edge_local_verify;
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod edge_offline_capture;
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod edge_sql;
pub mod exploration_checkpoints;
pub mod exploration_contracts;
mod indexed_cursor;
pub mod knowledge;
pub mod managed_local;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-owner", feature = "research-tools"))]
pub mod managed_source_query;
pub mod persistent_exploration_checkpoints;
pub mod prepared_local;
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod prepared_maintenance;
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod prepared_publication;
#[cfg(all(
    target_os = "linux",
    any(feature = "source-owner", feature = "research-tools")
))]
mod private_stage_invocation;
#[cfg(target_os = "linux")]
pub mod private_stage_run;
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod public_corpus_build_command;
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod public_d1_build;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod public_packet_compare;
pub mod reading;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod reference_cursor;
pub mod release_state;
pub mod word_analysis;
pub use knowledge::{KnowledgeOperation, KnowledgeRequest};
mod common;
pub mod http;
#[cfg(not(target_arch = "wasm32"))]
pub mod http_observation;
pub mod mcp;
pub mod mcp_http;
mod mcp_prompts;
mod mcp_resources;
pub mod search;
#[cfg(any(feature = "source-owner", feature = "research-tools"))]
mod selected_source;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-owner", feature = "research-tools"))]
mod selected_source_owner_cli;
pub mod site;
pub mod software_archive;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-owner", feature = "research-tools"))]
pub mod source_navigation;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-owner", feature = "research-tools"))]
mod source_owner_provider_phase;
pub mod source_read;
#[cfg(all(
    not(target_arch = "wasm32"),
    any(feature = "source-owner", feature = "research-tools")
))]
pub mod source_text_search;
#[cfg(any(
    target_arch = "wasm32",
    not(any(feature = "source-owner", feature = "research-tools"))
))]
#[path = "source_text_search_unavailable.rs"]
pub mod source_text_search;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod standalone_validation;

pub use common::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence,
    IndexedSearchParams, NoOwner, Params, PreparedHealth, PreparedPacket, QuerySession,
    RegisteredOperation, SEARCH_OPERATION_ID, SOURCE_TEXT_SEARCH_HTTP_PATH,
    SOURCE_TEXT_SEARCH_MCP_TOOL, SOURCE_TEXT_SEARCH_OPERATION_ID, ScopedAccessExecutor,
    checked_execute, descriptor, registered_operations,
};

#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod native_prepare;

#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod capture_restore;
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod source_cut_restore;

#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod evidence_projection;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod core_snapshot;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_cold_resources;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod reference_query_executor;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod reference_root_query;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
mod core_http_admission;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod source_projection_catalog_capture;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub mod source_projection_coverage;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "source-tools", feature = "research-tools"))]
pub(crate) mod controlled_reference_health;
