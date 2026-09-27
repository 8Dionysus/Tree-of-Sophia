//! Descriptive managed-source identity for the existing selected producer.
//! These packets never issue current authority. The command owner derives and
//! checks them against its retained generation, committed chain and fences.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, canonical_bytes_v1, parse_json,
};

pub const KNOWLEDGE_MANAGED_MODEL_ABI: &str = "tos_knowledge_read_model_v6";
pub const MANAGED_SOURCE_SCHEMA: &str = "tos_managed_agent_selected_source_v1";
pub const MANAGED_GRAPH_SCHEMA: &str = "tos_knowledge_graph_v2";
pub const MANAGED_CATALOG_SCHEMA: &str = "tos_knowledge_catalog_v2";
pub const MANAGED_SELECTION_SCHEMA: &str = "tos_access_native_knowledge_selection_v4";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceGenerationV1 {
    pub domain: String,
    pub store_id: [u8; 16],
    pub installed_generation_sha256: String,
    pub through_commit_seq: u64,
    pub selected_audit_generation: u64,
    pub epoch: u64,
    pub definition_sha256: String,
    pub bootstrap_source_revision: String,
    pub bootstrap_membership_sha256: String,
    pub bootstrap_members: u64,
    pub domain_sha256: String,
    pub database_oid: u64,
    pub schema_profile_sha256: String,
    pub state_sha256: String,
    pub log_sha256: String,
    pub current_membership_sha256: String,
    pub current_members: u64,
    pub history_membership_sha256: String,
    pub history_members: u64,
    pub inventory_projection_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceDeltaV1 {
    pub parent_model_sha256: String,
    pub parent_model_size_bytes: u64,
    pub parent_source_proof_sha256: String,
    pub parent_through_commit_seq: u64,
    pub committed_delta_sha256: String,
    pub committed_member_root_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceProofV1 {
    pub schema: String,
    pub generation: ManagedSourceGenerationV1,
    /// Genuine full-producer export correspondence, not the current source
    /// revision of a successor and not its installed generation digest.
    pub initial_export_source_revision: String,
    pub initial_export_membership_sha256: String,
    pub initial_export_members: u64,
    pub delta: Option<ManagedSourceDeltaV1>,
}

impl ManagedSourceProofV1 {
    pub fn validate(&self) -> Result<()> {
        if self.schema != MANAGED_SOURCE_SCHEMA || self.generation.domain.is_empty() {
            return Err(Error::Invalid("managed source proof schema/domain"));
        }
        let g = &self.generation;
        for digest in [
            &g.installed_generation_sha256,
            &g.definition_sha256,
            &g.bootstrap_source_revision,
            &g.bootstrap_membership_sha256,
            &g.domain_sha256,
            &g.schema_profile_sha256,
            &g.state_sha256,
            &g.log_sha256,
            &g.current_membership_sha256,
            &g.history_membership_sha256,
            &g.inventory_projection_sha256,
            &self.initial_export_source_revision,
            &self.initial_export_membership_sha256,
        ] {
            Digest256::from_hex(digest)
                .map_err(|_| Error::Invalid("managed source proof digest"))?;
        }
        if let Some(d) = &self.delta {
            for digest in [
                &d.parent_model_sha256,
                &d.parent_source_proof_sha256,
                &d.committed_delta_sha256,
                &d.committed_member_root_sha256,
            ] {
                Digest256::from_hex(digest)
                    .map_err(|_| Error::Invalid("managed source delta digest"))?;
            }
            if d.parent_model_size_bytes == 0
                || d.parent_through_commit_seq.checked_add(1) != Some(g.through_commit_seq)
            {
                return Err(Error::Invalid("managed source delta parent sequence"));
            }
        }
        Ok(())
    }

    /// Mechanical canonical root. Validation does not certify a generation or
    /// accept public receipts as an owner-issued committed delta capability.
    pub fn root_sha256(&self) -> Result<String> {
        self.validate()?;
        let raw = serde_json::to_vec(self).map_err(|e| Error::Source(e.to_string()))?;
        let limits = JsonLimits::default();
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits)
            .map_err(|e| Error::Source(e.to_string()))?;
        let bytes = canonical_bytes_v1(
            parsed.root(),
            CanonicalProfile::SourceRecordDigestV1,
            limits,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        Ok(Digest256::of_bytes(&bytes).to_hex())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum KnowledgeSourceBasis {
    #[serde(rename = "v1_cut")]
    V1Cut { source_revision: String },
    #[serde(rename = "managed_current")]
    ManagedCurrent { proof: ManagedSourceProofV1 },
}
impl KnowledgeSourceBasis {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::V1Cut { source_revision } => Digest256::from_hex(source_revision)
                .map(|_| ())
                .map_err(|_| Error::Invalid("knowledge cut source revision")),
            Self::ManagedCurrent { proof } => proof.validate(),
        }
    }
    pub fn source_revision(&self) -> Option<&str> {
        match self {
            Self::V1Cut { source_revision } => Some(source_revision),
            Self::ManagedCurrent { .. } => None,
        }
    }
    pub fn managed_source(&self) -> Option<&ManagedSourceProofV1> {
        match self {
            Self::V1Cut { .. } => None,
            Self::ManagedCurrent { proof } => Some(proof),
        }
    }
}
