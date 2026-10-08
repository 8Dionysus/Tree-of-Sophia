//! Native transport adapters over owner-selected Rust query sessions.
//!
//! The operation descriptor is an API-owned candidate. Presence in the
//! descriptor never advertises a capability: a trusted selected session and
//! current-rights fence must be installed by the source owner first.

pub mod cli;
pub mod concept_search;
#[cfg(not(target_arch = "wasm32"))]
pub mod coverage;
pub mod doctor;
#[cfg(target_os = "linux")]
pub mod edge_local_verify;
pub mod edge_offline_capture;
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
#[cfg(target_os = "linux")]
pub mod private_stage_run;
pub mod public_d1_build;
#[cfg(not(target_arch = "wasm32"))]
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
mod selected_source;
#[cfg(not(target_arch = "wasm32"))]
mod selected_source_owner_cli;
pub mod site;
pub mod software_archive;
#[cfg(not(target_arch = "wasm32"))]
mod source_owner_provider_phase;
pub mod source_read;

pub use common::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence,
    IndexedSearchParams, NoOwner, Params, PreparedHealth, PreparedPacket, QuerySession,
    RegisteredOperation, SEARCH_OPERATION_ID, ScopedAccessExecutor, checked_execute, descriptor,
    registered_operations,
};

pub mod native_prepare;

pub mod capture_restore;
pub mod source_cut_restore;

pub mod evidence_projection;
#[cfg(not(target_arch = "wasm32"))]
pub mod lexical_index_command;
pub mod structural_paragraph_command;
#[cfg(not(target_arch = "wasm32"))]
pub mod technical_markup_command;

#[cfg(not(target_arch = "wasm32"))]
pub mod research_builders_command;
#[cfg(not(target_arch = "wasm32"))]
pub mod source_text_foundation_command;

#[cfg(not(target_arch = "wasm32"))]
pub mod core_snapshot;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_cold_resources;
#[cfg(not(target_arch = "wasm32"))]
pub mod reference_query_executor;
#[cfg(not(target_arch = "wasm32"))]
pub mod reference_root_query;

#[cfg(not(target_arch = "wasm32"))]
mod core_http_admission;

#[cfg(not(target_arch = "wasm32"))]
pub mod source_projection_catalog_capture;
#[cfg(not(target_arch = "wasm32"))]
pub mod source_projection_coverage;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod controlled_reference_health;

#[cfg(not(target_arch = "wasm32"))]
pub mod transfer_metadata_command;

#[cfg(not(target_arch = "wasm32"))]
pub mod dta_technical_markup_command;
#[cfg(not(target_arch = "wasm32"))]
pub mod transfer_target_passages_command;
#[cfg(not(target_arch = "wasm32"))]
pub mod transfer_source_passages_command;

#[cfg(not(target_arch = "wasm32"))]
pub mod transfer_candidates_command;

pub mod transfer_source_visible_command;

#[cfg(not(target_arch = "wasm32"))]
pub mod german_triangulation_command;

pub mod bounded_translation_input_command;

pub mod antonovsky_collation_command;

pub mod synthetic_foundation_lab_command;

pub mod authored_canon_bridge_command;

pub mod provenance_event_lab_command;

pub mod jenseits_label_command;

pub mod nietzsche_transfer_routes_command;
