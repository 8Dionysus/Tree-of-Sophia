//! Source command mechanics and durable PostgreSQL/STO coordination.
//!
//! Mechanical preparation and private execution do not grant ToS source,
//! semantic, rights, publication or canon admission.

pub mod backup_recovery;
pub mod backup_recovery_cli;
mod durable_adapter;
pub mod source_agent_publication;
mod source_agent_publication_apply;
mod source_agent_publication_assembly;
mod source_agent_publication_closure;
mod source_agent_publication_commit;
mod source_agent_publication_profile;
mod source_agent_publication_recovery;
pub mod source_artifact_native;
pub mod source_claim_publication;
mod source_claim_publication_assembly;
mod source_claim_publication_bytes;
mod source_claim_publication_closure;
mod source_claim_publication_context;
mod source_claim_publication_dependencies;
mod source_claim_publication_graph;
mod source_claim_publication_normalize;
mod source_claim_publication_roots;
pub mod source_claims;
pub mod source_command;
pub mod source_creation;
pub mod source_creation_store;
pub mod source_current_cut;
mod source_managed_query;
pub mod source_managed_selection;
pub mod source_metadata_publication;
mod source_metadata_publication_assembly;
pub mod source_native_cli;
pub use durable_adapter::source_cohort;
mod source_assessment_journal;
pub mod source_forms;
pub mod source_forms_compiler;
mod source_item_deposit;
mod source_item_inventory;
pub mod source_operation;
pub mod source_revisions;
mod source_serialization;
mod source_sign;
mod source_sign_native;
pub mod source_text_alignment_entry;
mod source_text_identity;
pub mod source_text_layer_derived_entry;
mod source_text_layer_derived_proposal;
pub mod source_text_layer_entry;
mod source_text_layer_native;
mod source_text_layer_normalize;
mod source_text_layer_payload;
mod source_text_layer_proposal;
mod source_text_layer_xml;
mod source_text_layer_zip;
mod source_text_owner;
mod source_text_owner_ocr;
mod source_text_private_store;
pub mod source_text_unit_entry;
mod source_text_unit_native;
mod source_text_unit_proposal;

use tos_foundation::Digest256;

pub use durable_adapter::{
    AttemptResolution, CancelOutcome, ColdCut, ColdRecoveredMember, ColdWorkspaceLimits,
    CommitShadowAttempt, CompleteGeneration, DurableCommitReceipt, DurableError,
    DurablePgCoordinator, DurableResult, DurableShadowMember, DurableTiming,
    PrivateGenerationWorkspace, RegisterShadowAttempt, ShadowWriteIdentity,
    StreamedGenerationProfile, VerifiedSelectedGeneration, durable_shadow_delta,
    durable_shadow_delta_prepared, lab_record_bytes,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateToken {
    pub kind: PredicateKind,
    pub owner: String,
    pub scope: String,
    pub token: String,
    pub definition_version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicateKind {
    Unique,
    Range,
}

impl PredicateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unique => "unique",
            Self::Range => "range",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PredicateRead {
    Exact {
        namespace: String,
        key: String,
        expected_version: Option<u64>,
        expected_digest: Option<Digest256>,
    },
    Absent {
        namespace: String,
        key: String,
    },
    Generation {
        predicate: PredicateToken,
        observed_generation: u64,
    },
}

pub mod source_public_text_entry;
pub mod source_public_text_owner;
pub mod source_public_text_proposal;

// Descriptive work, without source authority.
pub use durable_adapter::audit_delta::AuditDeltaWork;
pub use durable_adapter::source_cohort::ManagedSourceWorkV1;
