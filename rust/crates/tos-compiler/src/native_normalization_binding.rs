//! Shared binding for the native normalized-knowledge producer.
//!
//! The processor identity is supplied by the current producer owner. The
//! configuration digest binds the exact descriptor and registry bytes used
//! by that producer; registry digests bind their parsed semantic registries.

use crate::KnowledgeRegistry;
use serde_json::{Value, json};
use tos_foundation::{Digest256, Digest256Hasher};

/// Exact native normalization binding for one current producer invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeNormalizationBinding {
    pub(crate) processor_digest: Digest256,
    pub(crate) entity_registry_digest: String,
    pub(crate) relation_registry_digest: String,
    pub(crate) configuration_digest: Digest256,
}

impl NativeNormalizationBinding {
    /// Bind an owner-supplied current processor to the exact inputs it used.
    /// `registry` must have been parsed from `entity_registry` and
    /// `relation_registry`.
    pub(crate) fn from_current_inputs(
        processor_digest: Digest256,
        registry: &KnowledgeRegistry,
        descriptor: &[u8],
        entity_registry: &[u8],
        relation_registry: &[u8],
    ) -> Self {
        Self {
            processor_digest,
            entity_registry_digest: registry.entity_semantic_digest.clone(),
            relation_registry_digest: registry.relation_semantic_digest.clone(),
            configuration_digest: configuration_digest(
                descriptor,
                entity_registry,
                relation_registry,
            ),
        }
    }

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "schema":"tos_knowledge_graph_normalization_binding_v1",
            "processor_digest":self.processor_digest.to_hex(),
            "entity_registry_digest":self.entity_registry_digest,
            "relation_registry_digest":self.relation_registry_digest,
            "configuration_digest":self.configuration_digest.to_hex()
        })
    }
}

/// Hash the exact producer inputs with the established length-prefixed frame.
pub(crate) fn configuration_digest(
    descriptor: &[u8],
    entity_registry: &[u8],
    relation_registry: &[u8],
) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    for raw in [descriptor, entity_registry, relation_registry] {
        hasher.update(&(raw.len() as u64).to_be_bytes());
        hasher.update(raw);
    }
    hasher.finalize()
}
