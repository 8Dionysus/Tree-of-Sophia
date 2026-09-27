//! Source command mechanics and durable PostgreSQL/STO coordination.
//!
//! Mechanical preparation and private execution do not grant ToS source,
//! semantic, rights, publication or canon admission.

mod durable_adapter;
pub mod source_claims;
pub mod source_command;
pub mod source_creation;
pub mod source_creation_store;
pub mod source_current_cut;
pub use durable_adapter::source_cohort;
mod source_assessment_journal;
pub mod source_forms;
pub mod source_forms_compiler;
pub mod source_operation;
pub mod source_revisions;
mod source_serialization;
mod source_sign;
mod source_sign_native;

use tos_foundation::Digest256;

pub use durable_adapter::{
    AttemptResolution, CancelOutcome, ColdCut, ColdRecoveredMember, CommitShadowAttempt,
    CompleteGeneration, DurableCommitReceipt, DurableError, DurablePgCoordinator, DurableResult,
    DurableShadowMember, DurableTiming, RegisterShadowAttempt, ShadowWriteIdentity,
    VerifiedSelectedGeneration, durable_shadow_delta, durable_shadow_delta_prepared,
    lab_record_bytes,
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
