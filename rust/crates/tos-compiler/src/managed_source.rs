//! Descriptive managed-source identity for the existing selected producer.
//! These packets never issue current authority. The command owner derives and
//! checks them against its retained generation, committed chain and fences.

pub use crate::managed_agent_producer::{
    CompletedManagedAgentProducer, prepare_managed_agent_selected_model,
    prepare_managed_agent_selected_successor,
};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, canonical_bytes_v1, parse_json,
};

pub const KNOWLEDGE_MANAGED_MODEL_ABI: &str = tos_foundation::KNOWLEDGE_MODEL_ABI_V6_POSTINGS_V1;
pub const MANAGED_SOURCE_V2_SCHEMA: &str = "tos_managed_agent_selected_source_v2";
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
pub struct ManagedSourceGenerationV2 {
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
    pub addressed_metadata_tree_sha256: String,
    pub addressed_metadata_members: u64,
    pub state_profile_sha256: String,
    pub addressed_inventory_root_sha256: String,
    pub log_sha256: String,
    pub addressed_current_tree_sha256: String,
    pub current_members: u64,
    pub addressed_history_tree_sha256: String,
    pub history_members: u64,
    pub inventory_projection_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceProofV2 {
    pub schema: String,
    pub generation: ManagedSourceGenerationV2,
    /// Genuine full-producer export correspondence, not the current source
    /// revision of a successor and not its installed generation digest.
    pub initial_export_source_revision: String,
    pub initial_export_membership_sha256: String,
    pub initial_export_members: u64,
    pub delta: Option<ManagedSourceDeltaV1>,
}

impl ManagedSourceProofV2 {
    pub fn validate(&self) -> Result<()> {
        if self.schema != MANAGED_SOURCE_V2_SCHEMA || self.generation.domain.is_empty() {
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
            &g.addressed_metadata_tree_sha256,
            &g.state_profile_sha256,
            &g.addressed_inventory_root_sha256,
            &g.log_sha256,
            &g.addressed_current_tree_sha256,
            &g.addressed_history_tree_sha256,
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

    /// Explicit installed-current identity in the existing stage source-cut
    /// string field. This is never a v1 SourceRevision.
    pub fn stage_source_cut(&self) -> Result<String> {
        Ok(format!("managed-agent-current-v2:{}", self.root_sha256()?))
    }
    pub(crate) fn check_binding(
        &self,
        source_cut: &str,
        membership: &str,
        through_seq: u64,
    ) -> Result<()> {
        if source_cut != self.stage_source_cut()?
            || membership != self.generation.addressed_current_tree_sha256
            || through_seq != self.generation.through_commit_seq
        {
            return Err(Error::Invalid("managed source exact stage binding"));
        }
        Ok(())
    }

    /// Mechanical canonical root. Validation does not certify a generation or
    /// accept public receipts as an owner-issued committed delta capability.
    pub fn root_sha256(&self) -> Result<String> {
        self.validate()?;
        proof_root(self)
    }
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

    /// Explicit installed-current identity in the existing stage source-cut
    /// string field. This is never a v1 SourceRevision.
    pub fn stage_source_cut(&self) -> Result<String> {
        Ok(format!("managed-agent-current:{}", self.root_sha256()?))
    }
    pub(crate) fn check_binding(
        &self,
        source_cut: &str,
        membership: &str,
        through_seq: u64,
    ) -> Result<()> {
        if source_cut != self.stage_source_cut()?
            || membership != self.generation.current_membership_sha256
            || through_seq != self.generation.through_commit_seq
        {
            return Err(Error::Invalid("managed source exact stage binding"));
        }
        Ok(())
    }

    /// Mechanical canonical root. Validation does not certify a generation or
    /// accept public receipts as an owner-issued committed delta capability.
    pub fn root_sha256(&self) -> Result<String> {
        self.validate()?;
        proof_root(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum KnowledgeSourceBasis {
    #[serde(rename = "v1_cut")]
    V1Cut { source_revision: String },
    #[serde(rename = "managed_current")]
    ManagedCurrent { proof: ManagedSourceProofV1 },
    #[serde(rename = "managed_current_v2")]
    ManagedCurrentV2 { proof: ManagedSourceProofV2 },
}
impl KnowledgeSourceBasis {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::V1Cut { source_revision } => Digest256::from_hex(source_revision)
                .map(|_| ())
                .map_err(|_| Error::Invalid("knowledge cut source revision")),
            Self::ManagedCurrent { proof } => proof.validate(),
            Self::ManagedCurrentV2 { proof } => proof.validate(),
        }
    }
    pub fn source_revision(&self) -> Option<&str> {
        match self {
            Self::V1Cut { source_revision } => Some(source_revision),
            Self::ManagedCurrent { .. } | Self::ManagedCurrentV2 { .. } => None,
        }
    }
    pub fn managed_source(&self) -> Option<&ManagedSourceProofV1> {
        match self {
            Self::V1Cut { .. } | Self::ManagedCurrentV2 { .. } => None,
            Self::ManagedCurrent { proof } => Some(proof),
        }
    }
}

impl KnowledgeSourceBasis {
    pub fn managed_source_v2(&self) -> Option<&ManagedSourceProofV2> {
        match self {
            Self::ManagedCurrentV2 { proof } => Some(proof),
            _ => None,
        }
    }
    pub(crate) fn managed_proof(&self) -> Option<ManagedProofRef<'_>> {
        match self {
            Self::ManagedCurrent { proof } => Some(ManagedProofRef::V1(proof)),
            Self::ManagedCurrentV2 { proof } => Some(ManagedProofRef::V2(proof)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ManagedProofRef<'a> {
    V1(&'a ManagedSourceProofV1),
    V2(&'a ManagedSourceProofV2),
}
impl ManagedProofRef<'_> {
    pub fn root_sha256(&self) -> Result<String> {
        match self {
            Self::V1(p) => p.root_sha256(),
            Self::V2(p) => p.root_sha256(),
        }
    }
    pub fn check_binding(&self, cut: &str, membership: &str, seq: u64) -> Result<()> {
        match self {
            Self::V1(p) => p.check_binding(cut, membership, seq),
            Self::V2(p) => p.check_binding(cut, membership, seq),
        }
    }
}
mod sealed {
    pub trait Proof {}
}
/// Exactly the two descriptive source proof versions. This interface issues no authority.
pub trait ManagedProducerProof: sealed::Proof + Clone + std::fmt::Debug + PartialEq + Sync {
    fn validate(&self) -> Result<()>;
    fn root_sha256(&self) -> Result<String>;
    fn stage_source_cut(&self) -> Result<String>;
    fn basis(&self) -> KnowledgeSourceBasis;
    fn current_root(&self) -> &str;
    fn through_commit_seq(&self) -> u64;
    fn inventory_projection(&self) -> &str;
    fn initial_export(&self) -> (&str, &str, u64);
    fn delta(&self) -> Option<&ManagedSourceDeltaV1>;
    fn same_immutable_context(&self, other: &Self) -> bool;
}

fn proof_root(proof: &impl Serialize) -> Result<String> {
    let limits = JsonLimits::default();
    let mut out = ProofWriter {
        bytes: Vec::new(),
        max: limits.max_bytes,
    };
    serde_json::to_writer(&mut out, proof)
        .map_err(|_| Error::Budget("managed source proof bytes"))?;
    let parsed = parse_json(&out.bytes, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let bytes = canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    Ok(Digest256::of_bytes(&bytes).to_hex())
}

struct ProofWriter {
    bytes: Vec<u8>,
    max: usize,
}
impl std::io::Write for ProofWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.max)
        {
            return Err(std::io::Error::other("managed source proof bytes"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn header_basis(header: &serde_json::Value) -> Result<KnowledgeSourceBasis> {
    let basis = match header.get("schema").and_then(serde_json::Value::as_str) {
        Some("tos_knowledge_graph_v1") => KnowledgeSourceBasis::V1Cut {
            source_revision: header
                .get("source_revision")
                .and_then(serde_json::Value::as_str)
                .ok_or(Error::Invalid("knowledge graph cut revision"))?
                .into(),
        },
        Some(MANAGED_GRAPH_SCHEMA) => {
            if header.get("source_revision").is_some() {
                return Err(Error::Invalid("managed graph cannot declare v1 revision"));
            }
            let basis: KnowledgeSourceBasis = serde_json::from_value(
                header
                    .get("source_basis")
                    .ok_or(Error::Invalid("managed graph source basis"))?
                    .clone(),
            )
            .map_err(|_| Error::Invalid("managed graph source basis shape"))?;
            if !matches!(
                basis,
                KnowledgeSourceBasis::ManagedCurrent { .. }
                    | KnowledgeSourceBasis::ManagedCurrentV2 { .. }
            ) {
                return Err(Error::Invalid("managed graph basis profile"));
            }
            basis
        }
        _ => return Err(Error::Invalid("knowledge graph source schema")),
    };
    basis.validate()?;
    Ok(basis)
}

impl sealed::Proof for ManagedSourceProofV1 {}
impl ManagedProducerProof for ManagedSourceProofV1 {
    fn validate(&self) -> Result<()> {
        ManagedSourceProofV1::validate(self)
    }
    fn root_sha256(&self) -> Result<String> {
        ManagedSourceProofV1::root_sha256(self)
    }
    fn stage_source_cut(&self) -> Result<String> {
        ManagedSourceProofV1::stage_source_cut(self)
    }
    fn basis(&self) -> KnowledgeSourceBasis {
        KnowledgeSourceBasis::ManagedCurrent {
            proof: self.clone(),
        }
    }
    fn current_root(&self) -> &str {
        &self.generation.current_membership_sha256
    }
    fn through_commit_seq(&self) -> u64 {
        self.generation.through_commit_seq
    }
    fn inventory_projection(&self) -> &str {
        &self.generation.inventory_projection_sha256
    }
    fn initial_export(&self) -> (&str, &str, u64) {
        (
            &self.initial_export_source_revision,
            &self.initial_export_membership_sha256,
            self.initial_export_members,
        )
    }
    fn delta(&self) -> Option<&ManagedSourceDeltaV1> {
        self.delta.as_ref()
    }
    fn same_immutable_context(&self, other: &Self) -> bool {
        self.generation.domain == other.generation.domain
            && self.generation.store_id == other.generation.store_id
            && self.generation.epoch == other.generation.epoch
            && self.generation.definition_sha256 == other.generation.definition_sha256
            && self.generation.database_oid == other.generation.database_oid
            && self.generation.schema_profile_sha256 == other.generation.schema_profile_sha256
            && self.generation.bootstrap_source_revision
                == other.generation.bootstrap_source_revision
            && self.generation.bootstrap_membership_sha256
                == other.generation.bootstrap_membership_sha256
            && self.generation.bootstrap_members == other.generation.bootstrap_members
    }
}

impl sealed::Proof for ManagedSourceProofV2 {}
impl ManagedProducerProof for ManagedSourceProofV2 {
    fn validate(&self) -> Result<()> {
        ManagedSourceProofV2::validate(self)
    }
    fn root_sha256(&self) -> Result<String> {
        ManagedSourceProofV2::root_sha256(self)
    }
    fn stage_source_cut(&self) -> Result<String> {
        ManagedSourceProofV2::stage_source_cut(self)
    }
    fn basis(&self) -> KnowledgeSourceBasis {
        KnowledgeSourceBasis::ManagedCurrentV2 {
            proof: self.clone(),
        }
    }
    fn current_root(&self) -> &str {
        &self.generation.addressed_current_tree_sha256
    }
    fn through_commit_seq(&self) -> u64 {
        self.generation.through_commit_seq
    }
    fn inventory_projection(&self) -> &str {
        &self.generation.inventory_projection_sha256
    }
    fn initial_export(&self) -> (&str, &str, u64) {
        (
            &self.initial_export_source_revision,
            &self.initial_export_membership_sha256,
            self.initial_export_members,
        )
    }
    fn delta(&self) -> Option<&ManagedSourceDeltaV1> {
        self.delta.as_ref()
    }
    fn same_immutable_context(&self, other: &Self) -> bool {
        self.generation.domain == other.generation.domain
            && self.generation.store_id == other.generation.store_id
            && self.generation.epoch == other.generation.epoch
            && self.generation.definition_sha256 == other.generation.definition_sha256
            && self.generation.database_oid == other.generation.database_oid
            && self.generation.schema_profile_sha256 == other.generation.schema_profile_sha256
            && self.generation.bootstrap_source_revision
                == other.generation.bootstrap_source_revision
            && self.generation.bootstrap_membership_sha256
                == other.generation.bootstrap_membership_sha256
            && self.generation.bootstrap_members == other.generation.bootstrap_members
            && self.generation.state_profile_sha256 == other.generation.state_profile_sha256
    }
}

#[cfg(test)]
mod versioned_tests {
    use super::*;

    // Codec/binding fixtures issue no generation, membership or current authority.
    fn v1() -> ManagedSourceProofV1 {
        let digest = "0".repeat(64);
        let raw = serde_json::json!({
            "schema": MANAGED_SOURCE_SCHEMA,
            "generation": {
                "domain":"codec-only", "store_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
                "installed_generation_sha256":digest, "through_commit_seq":0,
                "selected_audit_generation":0, "epoch":0, "definition_sha256":digest,
                "bootstrap_source_revision":digest, "bootstrap_membership_sha256":digest,
                "bootstrap_members":0, "domain_sha256":digest, "database_oid":0,
                "schema_profile_sha256":digest, "state_sha256":digest, "log_sha256":digest,
                "current_membership_sha256":digest, "current_members":0,
                "history_membership_sha256":digest, "history_members":0,
                "inventory_projection_sha256":digest
            },
            "initial_export_source_revision":digest, "initial_export_membership_sha256":digest,
            "initial_export_members":0, "delta":null
        });
        serde_json::from_value(raw).unwrap()
    }

    #[test]
    fn addressed_proof_preserves_v1_wire_and_refuses_cross_version_bindings() {
        let before = v1();
        let old = serde_json::to_value(&before).unwrap();
        let mut addressed = old.clone();
        addressed["schema"] = MANAGED_SOURCE_V2_SCHEMA.into();
        let g = addressed["generation"].as_object_mut().unwrap();
        for (old, new) in [
            ("state_sha256", "addressed_metadata_tree_sha256"),
            ("current_membership_sha256", "addressed_current_tree_sha256"),
            ("history_membership_sha256", "addressed_history_tree_sha256"),
        ] {
            let value = g.remove(old).unwrap();
            g.insert(new.into(), value);
        }
        g.insert("addressed_metadata_members".into(), 0.into());
        g.insert("state_profile_sha256".into(), "1".repeat(64).into());
        g.insert(
            "addressed_inventory_root_sha256".into(),
            "2".repeat(64).into(),
        );
        let after: ManagedSourceProofV2 = serde_json::from_value(addressed.clone()).unwrap();
        assert!(serde_json::from_value::<ManagedSourceProofV1>(addressed).is_err());
        assert!(serde_json::from_value::<ManagedSourceProofV2>(old.clone()).is_err());
        assert_eq!(serde_json::to_value(&before).unwrap(), old);
        assert_ne!(before.root_sha256().unwrap(), after.root_sha256().unwrap());
        assert!(
            after
                .check_binding(&before.stage_source_cut().unwrap(), after.current_root(), 0)
                .is_err()
        );
        assert!(
            after
                .check_binding(&after.stage_source_cut().unwrap(), after.current_root(), 0)
                .is_ok()
        );
        let header =
            serde_json::json!({"schema":MANAGED_GRAPH_SCHEMA,"source_basis":after.basis()});
        assert_eq!(header_basis(&header).unwrap(), after.basis());
        let mut mislabeled = header;
        mislabeled["source_basis"]["kind"] = "managed_current".into();
        assert!(header_basis(&mislabeled).is_err());
    }
}
