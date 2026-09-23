//! Source-owned query vocabulary selection. Parsing is not source admission:
//! the owner supplies exact bytes and registry roots from one sealed cut.

use crate::{Error, Result, SourceBinding};
use serde_json::Value;
use std::collections::BTreeSet;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

const MAX_DESCRIPTOR_BYTES: usize = 1024 * 1024;
const MAX_SOURCES: usize = 4096;
const MAX_ROUTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredSource {
    pub source_graph_id: String,
    pub owner_ref: String,
    pub input_role: String,
    pub adapter_profile: String,
    pub representative_priority: u64,
}

/// The exact authored descriptor bytes are owner data; no graph/catalog or
/// runtime generation is stored in that authored document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryVocabulary {
    pub descriptor_sha256: String,
    pub descriptor_version: u64,
    pub sources: Vec<RegisteredSource>,
    /// Sorted exact IDs from the authored registration, including sources
    /// with zero selected rows. Kept separate from derived source_scope.
    pub registered_source_ids: Vec<String>,
    pub extension_adapter_profile: String,
    pub entity_registry_id: String,
    pub relation_registry_id: String,
    pub semantic_primitive_profile: String,
    pub shared_entity_id_grammars: Vec<String>,
    pub overview_route_ids: Vec<String>,
}

/// Derived selection envelope: all roots and cut evidence are outside the
/// authored descriptor, so the descriptor->graph/catalog DAG is acyclic.
#[derive(Clone, Debug)]
pub struct VocabularyBinding {
    pub descriptor_sha256: String,
    pub entity_registry_sha256: String,
    pub relation_registry_sha256: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub membership_root: String,
    pub graph_root_sha256: String,
    pub catalog_root_sha256: String,
    pub generation: String,
}

fn field<'a>(v: &'a Value, key: &str) -> Result<&'a Value> {
    v.get(key)
        .ok_or(Error::Invalid("query vocabulary field absent"))
}
fn exact_keys(v: &Value, allowed: &[&str]) -> Result<()> {
    let obj = v
        .as_object()
        .ok_or(Error::Invalid("query vocabulary object"))?;
    if obj.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(Error::Invalid("unknown query vocabulary field"));
    }
    Ok(())
}
fn object_field<'a>(v: &'a Value, key: &str) -> Result<&'a Value> {
    let item = field(v, key)?;
    if !item.is_object() {
        return Err(Error::Invalid("query vocabulary object field"));
    }
    Ok(item)
}
fn string(v: &Value, key: &str) -> Result<String> {
    let s = field(v, key)?
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 1024)
        .ok_or(Error::Invalid("query vocabulary string field"))?;
    Ok(s.to_owned())
}
fn strings(v: &Value, key: &str, cap: usize) -> Result<Vec<String>> {
    let list = field(v, key)?
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= cap)
        .ok_or(Error::Invalid("query vocabulary list field"))?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let s = item
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 1024)
            .ok_or(Error::Invalid("query vocabulary list value"))?;
        if !seen.insert(s) {
            return Err(Error::Invalid("duplicate query vocabulary value"));
        }
        out.push(s.to_owned());
    }
    Ok(out)
}

impl QueryVocabulary {
    pub fn registered_source_ids(&self) -> &[String] {
        &self.registered_source_ids
    }

    /// Rebind a parsed descriptor to its exact owner-supplied bytes before a
    /// selected full build. The public fields permit fixture construction and
    /// composition, so their cached registration must not be taken as
    /// authority merely because the byte digest still matches.
    pub fn verify_authored_bytes(&self, authored_bytes: &[u8]) -> Result<()> {
        if Digest256::of_bytes(authored_bytes).to_hex() != self.descriptor_sha256 {
            return Err(Error::Invalid("query vocabulary descriptor bytes"));
        }
        let mut adapters = BTreeSet::new();
        adapters.insert(self.extension_adapter_profile.as_str());
        for source in &self.sources {
            adapters.insert(source.adapter_profile.as_str());
        }
        let parsed = Self::parse(authored_bytes, &adapters.into_iter().collect::<Vec<_>>())?;
        if &parsed != self {
            return Err(Error::Invalid(
                "parsed query vocabulary differs from authored bytes",
            ));
        }
        Ok(())
    }

    /// `supported_adapters` comes from actual installed compiler capabilities,
    /// never from the descriptor itself. An eighth registered source needs no
    /// source-ID code branch when it uses an installed adapter.
    pub fn parse(authored_bytes: &[u8], supported_adapters: &[&str]) -> Result<Self> {
        if authored_bytes.is_empty() || authored_bytes.len() > MAX_DESCRIPTOR_BYTES {
            return Err(Error::Budget("query vocabulary bytes"));
        }
        let limits = JsonLimits::new(MAX_DESCRIPTOR_BYTES, 64, 100_000, 4096)
            .map_err(|_| Error::Budget("query vocabulary JSON limits"))?;
        parse_json(authored_bytes, JsonMode::PublishedStrict, limits)
            .map_err(|e| Error::Source(e.to_string()))?;
        let doc: Value = serde_json::from_slice(authored_bytes)
            .map_err(|_| Error::Invalid("query vocabulary JSON"))?;
        exact_keys(
            &doc,
            &[
                "$schema",
                "schema_version",
                "descriptor_id",
                "descriptor_version",
                "owner_ref",
                "purpose",
                "semantic_registry_refs",
                "query_profile",
                "sources",
                "extension_adapter_profile",
                "source_coverage",
                "mapping_policy",
                "identity",
                "overview",
                "filters",
                "catalog",
            ],
        )?;
        if string(&doc, "schema_version")? != "tos_knowledge_query_vocabulary_v1" {
            return Err(Error::Invalid("query vocabulary schema version"));
        }
        // Reject a forged authored document that tries to supply derived
        // currentness or output roots. The public schema is stricter still.
        for forbidden in [
            "source_cut",
            "through_commit_seq",
            "membership_root",
            "graph_root_sha256",
            "catalog_root_sha256",
            "generation",
            "descriptor_root",
        ] {
            if doc.get(forbidden).is_some() {
                return Err(Error::Invalid("derived field in authored vocabulary"));
            }
        }
        let descriptor_version = field(&doc, "descriptor_version")?
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or(Error::Invalid("query vocabulary version"))?;
        string(&doc, "descriptor_id")?;
        string(&doc, "owner_ref")?;
        string(&doc, "purpose")?;
        if string(&doc, "source_coverage")?
            != "all-registered-sources-exactly-once; unknown-or-unsupported-adapter-refuses"
            || string(&doc, "mapping_policy")?
                != "exact-source-graph-and-native-id; unknown-retains-native-id-with-registry-fallback; no-name-inference"
        {
            return Err(Error::Invalid("query vocabulary coverage policy"));
        }
        let registry = object_field(&doc, "semantic_registry_refs")?;
        exact_keys(registry, &["entity", "relation"])?;
        for key in ["entity", "relation"] {
            let reference = object_field(registry, key)?;
            exact_keys(
                reference,
                &[
                    "registry_id",
                    "source_ref",
                    "fallback_id_field",
                    "property_field",
                ],
            )?;
            string(reference, "source_ref")?;
            string(reference, "fallback_id_field")?;
        }
        let entity_registry_id = string(object_field(registry, "entity")?, "registry_id")?;
        let relation_registry_id = string(object_field(registry, "relation")?, "registry_id")?;
        let profile = object_field(&doc, "query_profile")?;
        exact_keys(
            profile,
            &[
                "profile_id",
                "profile_version",
                "semantic_primitive_profile",
                "normalized_carrier_id_profile",
                "source_dossier_profile",
            ],
        )?;
        string(profile, "profile_id")?;
        field(profile, "profile_version")?
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or(Error::Invalid("query profile version"))?;
        string(profile, "normalized_carrier_id_profile")?;
        string(profile, "source_dossier_profile")?;
        let semantic_primitive_profile = string(profile, "semantic_primitive_profile")?;
        let identity = object_field(&doc, "identity")?;
        exact_keys(
            identity,
            &[
                "shared_entity_id_grammars",
                "carrier_group_profile",
                "focus_resolution_order",
                "native_id_collision",
                "source_dossier_graph_id",
                "source_dossier_kinds",
                "source_dossier_ref_profile",
                "same_as_relation_effect",
            ],
        )?;
        let shared_entity_id_grammars = strings(identity, "shared_entity_id_grammars", 128)?;
        string(identity, "carrier_group_profile")?;
        if strings(identity, "focus_resolution_order", 3)?
            != ["id", "entity_id", "unique_native_id"]
            || string(identity, "native_id_collision")? != "ambiguous-refuse"
            || string(identity, "source_dossier_ref_profile")? != "exact-owner-issued-handle-only"
            || string(identity, "same_as_relation_effect")? != "retained-relation-only"
        {
            return Err(Error::Invalid("query identity profile"));
        }
        let dossier_graph = string(identity, "source_dossier_graph_id")?;
        strings(identity, "source_dossier_kinds", 256)?;
        let extension_adapter_profile = string(&doc, "extension_adapter_profile")?;
        let supported: BTreeSet<&str> = supported_adapters.iter().copied().collect();
        if !supported.contains(extension_adapter_profile.as_str()) {
            return Err(Error::Invalid("unsupported extension adapter"));
        }
        let rows = field(&doc, "sources")?
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= MAX_SOURCES)
            .ok_or(Error::Invalid("source registration count"))?;
        let mut ids = BTreeSet::new();
        let mut priorities = BTreeSet::new();
        let mut sources = Vec::with_capacity(rows.len());
        for row in rows {
            exact_keys(
                row,
                &[
                    "source_graph_id",
                    "owner_ref",
                    "input_role",
                    "adapter_profile",
                    "representative_priority",
                ],
            )?;
            let source_graph_id = string(row, "source_graph_id")?;
            let owner_ref = string(row, "owner_ref")?;
            let input_role = string(row, "input_role")?;
            let adapter_profile = string(row, "adapter_profile")?;
            let representative_priority = field(row, "representative_priority")?
                .as_u64()
                .ok_or(Error::Invalid("source representative priority"))?;
            if !ids.insert(source_graph_id.clone()) || !priorities.insert(representative_priority) {
                return Err(Error::Invalid("duplicate source or priority"));
            }
            if !supported.contains(adapter_profile.as_str()) {
                return Err(Error::Invalid("unsupported source adapter"));
            }
            sources.push(RegisteredSource {
                source_graph_id,
                owner_ref,
                input_role,
                adapter_profile,
                representative_priority,
            });
        }
        if !ids.contains(&dossier_graph) {
            return Err(Error::Invalid("dossier source not registered"));
        }
        let overview = object_field(&doc, "overview")?;
        exact_keys(
            overview,
            &[
                "excluded_predicate_ids",
                "excluded_relation_type_ids",
                "routes",
            ],
        )?;
        strings(overview, "excluded_predicate_ids", MAX_ROUTES)?;
        strings(overview, "excluded_relation_type_ids", MAX_ROUTES)?;
        let routes = field(overview, "routes")?
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= MAX_ROUTES)
            .ok_or(Error::Invalid("overview route count"))?;
        let mut route_ids = BTreeSet::new();
        let mut overview_route_ids = Vec::with_capacity(routes.len());
        for route in routes {
            exact_keys(
                route,
                &[
                    "route_id",
                    "candidate_kind_ids",
                    "confirming_predicate_ids",
                    "candidate_type_ids",
                    "confirming_relation_type_ids",
                ],
            )?;
            let route_id = string(route, "route_id")?;
            for key in [
                "candidate_kind_ids",
                "confirming_predicate_ids",
                "candidate_type_ids",
                "confirming_relation_type_ids",
            ] {
                let values = field(route, key)?
                    .as_array()
                    .filter(|v| v.len() <= MAX_ROUTES)
                    .ok_or(Error::Invalid("overview route set"))?;
                let mut seen = BTreeSet::new();
                for value in values {
                    let text = value
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 1024)
                        .ok_or(Error::Invalid("overview route value"))?;
                    if !seen.insert(text) {
                        return Err(Error::Invalid("duplicate overview route value"));
                    }
                }
            }
            if !route_ids.insert(route_id.clone()) {
                return Err(Error::Invalid("duplicate overview route"));
            }
            overview_route_ids.push(route_id);
        }
        let filters = object_field(&doc, "filters")?;
        exact_keys(
            filters,
            &[
                "operators",
                "dotted_field_profile",
                "node_fields",
                "relation_fields",
                "property_definitions",
                "unknown_field",
                "unknown_property_id",
            ],
        )?;
        for key in ["operators", "node_fields", "relation_fields"] {
            strings(filters, key, MAX_ROUTES)?;
        }
        string(filters, "dotted_field_profile")?;
        if string(filters, "property_definitions")?
            != "selected-entity-registry-property-definitions; preserve-applies-to-inherited-value-type-operators"
            || string(filters, "unknown_field")? != "reject"
            || string(filters, "unknown_property_id")? != "reject"
        {
            return Err(Error::Invalid("filter registry policy"));
        }
        let catalog = object_field(&doc, "catalog")?;
        exact_keys(catalog, &["canonical_order", "facets"])?;
        string(catalog, "canonical_order")?;
        strings(catalog, "facets", MAX_ROUTES)?;
        let registered_source_ids = ids.into_iter().collect();
        Ok(Self {
            descriptor_sha256: Digest256::of_bytes(authored_bytes).to_hex(),
            descriptor_version,
            sources,
            registered_source_ids,
            extension_adapter_profile,
            entity_registry_id,
            relation_registry_id,
            semantic_primitive_profile,
            shared_entity_id_grammars,
            overview_route_ids,
        })
    }

    /// The independent source/selection owner must supply exact registry
    /// digests and graph/catalog roots. A caller cannot bind arbitrary bytes
    /// to a different descriptor digest or incomplete sealed source cut.
    pub fn bind(
        &self,
        source: &SourceBinding,
        descriptor_sha256: &str,
        entity_registry_sha256: &str,
        relation_registry_sha256: &str,
        graph_root_sha256: &str,
        catalog_root_sha256: &str,
        generation: &str,
    ) -> Result<VocabularyBinding> {
        source.validate()?;
        if self.descriptor_sha256 != descriptor_sha256 || generation.is_empty() {
            return Err(Error::Invalid("vocabulary selection mismatch"));
        }
        for digest in [
            entity_registry_sha256,
            relation_registry_sha256,
            graph_root_sha256,
            catalog_root_sha256,
        ] {
            Digest256::from_hex(digest).map_err(|_| Error::Invalid("vocabulary binding digest"))?;
        }
        Ok(VocabularyBinding {
            descriptor_sha256: self.descriptor_sha256.clone(),
            entity_registry_sha256: entity_registry_sha256.to_owned(),
            relation_registry_sha256: relation_registry_sha256.to_owned(),
            source_cut: source.source_cut.clone(),
            through_commit_seq: source.through_commit_seq,
            membership_root: source.membership_root.clone(),
            graph_root_sha256: graph_root_sha256.to_owned(),
            catalog_root_sha256: catalog_root_sha256.to_owned(),
            generation: generation.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASELINE: &[u8] = include_bytes!("../tests/fixtures/query-vocabulary.v1.json");
    const ADAPTERS: &[&str] = &[
        "philosophy-node-edge-v1",
        "canon-node-relation-v1",
        "candidate-relation-v1",
        "source-navigation-node-edge-v1",
        "reified-bibliographic-claims-v1",
        "declared-identity-and-source-ref-joins-v1",
        "repository-topology-v1",
        "indexed-node-edge-v1",
    ];

    #[test]
    fn baseline_source_owned_descriptor_is_exact_and_separate_from_derived_roots() {
        let vocab = QueryVocabulary::parse(BASELINE, ADAPTERS).unwrap();
        assert_eq!(
            vocab.descriptor_sha256,
            "60353a59a4dad50c3f711d7e8f08e9400866841ea4d169f43824be7dec1864ea"
        );
        assert_eq!(vocab.sources.len(), 7);
        assert_eq!(vocab.overview_route_ids.len(), 7);
        assert_eq!(vocab.entity_registry_id, "tos.semantic.entity-types");
        let source = SourceBinding {
            owner_profile: "sealed-test".into(),
            source_cut: "cut-1".into(),
            through_commit_seq: 9,
            membership_root: "a".repeat(64),
            index_generation: "i1".into(),
            route_map_version: "r1".into(),
            reader_abi: "reader1".into(),
            projection_root_sha256: "b".repeat(64),
            complete: true,
        };
        let binding = vocab
            .bind(
                &source,
                &vocab.descriptor_sha256,
                &"c".repeat(64),
                &"d".repeat(64),
                &"e".repeat(64),
                &"f".repeat(64),
                "g1",
            )
            .unwrap();
        assert_eq!(binding.source_cut, "cut-1");
        assert_eq!(binding.descriptor_sha256, vocab.descriptor_sha256);
        assert!(
            vocab
                .bind(
                    &source,
                    &"0".repeat(64),
                    &"c".repeat(64),
                    &"d".repeat(64),
                    &"e".repeat(64),
                    &"f".repeat(64),
                    "g1"
                )
                .is_err()
        );
    }

    #[test]
    fn eighth_owner_registered_source_uses_installed_adapter_without_id_branch() {
        let mut doc: Value = serde_json::from_slice(BASELINE).unwrap();
        let eighth = serde_json::json!({
            "source_graph_id": "another-owner-source", "owner_ref": "owner:another",
            "input_role": "source-graph", "adapter_profile": "indexed-node-edge-v1",
            "representative_priority": 7
        });
        doc["sources"].as_array_mut().unwrap().push(eighth);
        let bytes = serde_json::to_vec(&doc).unwrap();
        let selected = QueryVocabulary::parse(&bytes, ADAPTERS).unwrap();
        assert_eq!(selected.sources.len(), 8);
        assert_eq!(selected.sources[7].source_graph_id, "another-owner-source");
        assert_eq!(selected.registered_source_ids().len(), 8);
        assert_eq!(selected.registered_source_ids()[0], "another-owner-source");
        assert!(
            selected
                .registered_source_ids()
                .windows(2)
                .all(|ids| ids[0] < ids[1])
        );
        doc["sources"][7]["adapter_profile"] = Value::String("uninstalled-v7".into());
        assert!(QueryVocabulary::parse(&serde_json::to_vec(&doc).unwrap(), ADAPTERS).is_err());
        doc["sources"][7]["adapter_profile"] = Value::String("indexed-node-edge-v1".into());
        doc["sources"][7]["source_graph_id"] = Value::String("canon".into());
        assert!(QueryVocabulary::parse(&serde_json::to_vec(&doc).unwrap(), ADAPTERS).is_err());
    }

    #[test]
    fn forged_currentness_and_duplicate_json_keys_refuse() {
        let mut doc: Value = serde_json::from_slice(BASELINE).unwrap();
        doc["graph_root_sha256"] = Value::String("0".repeat(64));
        assert!(QueryVocabulary::parse(&serde_json::to_vec(&doc).unwrap(), ADAPTERS).is_err());
        let duplicate = br#"{"schema_version":"tos_knowledge_query_vocabulary_v1","schema_version":"tos_knowledge_query_vocabulary_v1"}"#;
        assert!(QueryVocabulary::parse(duplicate, ADAPTERS).is_err());
    }
}
