//! Header for the disposable public graph. Counts describe exactly the
//! path-adapted rows used by Catalog, Search and the D1 SQL transport.

use crate::{
    Error, KnowledgeRegistry, Result,
    d1_public_capture::{CreationState, PublicCapture},
    knowledge_stage::{CoreRoots, KnowledgeStage},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tos_foundation::Digest256;

/// Folded while semantic validation already owns each authenticated final row.
/// Only this small summary crosses the phase boundary; header construction
/// authenticates the same graph roots before using it.
#[derive(Default)]
pub(crate) struct HeaderCounts {
    sources: BTreeMap<String, u64>,
    states: [BTreeMap<String, u64>; 2],
    mapped: [u64; 2],
    missing: [u64; 2],
    rows: [u64; 2],
    cross_layer: u64,
}
impl HeaderCounts {
    pub(crate) fn observe(
        &mut self,
        kind: usize,
        source: &str,
        value: &Value,
        state: Option<&CreationState<'_>>,
    ) -> Result<()> {
        let inc = |map: &mut BTreeMap<String, u64>, key: &str| -> Result<()> {
            if let Some(state) = state {
                increment_count(map, key, state)
            } else {
                let count = map.entry(key.to_owned()).or_default();
                *count = count.checked_add(1).ok_or(Error::Budget("header count"))?;
                Ok(())
            }
        };
        self.rows[kind] = self.rows[kind]
            .checked_add(1)
            .ok_or(Error::Budget("header rows"))?;
        if kind == 0 {
            inc(&mut self.sources, source)?;
        } else if source == "semantic-interchange" {
            self.cross_layer = self
                .cross_layer
                .checked_add(1)
                .ok_or(Error::Budget("header cross-layer count"))?;
        }
        let display_key = if kind == 0 {
            "summary_state"
        } else {
            "explanation_state"
        };
        let display = value["display"][display_key]
            .as_str()
            .ok_or(Error::Invalid("public D1 display state"))?;
        inc(&mut self.states[kind], display)?;
        let mapping = if kind == 0 {
            "type_mapping"
        } else {
            "predicate_mapping"
        };
        self.mapped[kind] += u64::from(value[mapping]["status"] == "mapped");
        let availability = if kind == 0 {
            "source_summary_available"
        } else {
            "source_explanation_available"
        };
        self.missing[kind] += u64::from(value["display"]["provenance"][availability] == false);
        if let Some(state) = state {
            state.charge_work(source.len() + display.len() + 32)?;
        }
        Ok(())
    }
}

// Keep repeated two-field gaps compact during the row scan. The DOM is
// constructed once at the header handoff; moving strings preserves identity.
pub(crate) struct SemanticGap {
    pub(crate) id: String,
    pub(crate) kind: String,
}
pub(crate) struct ValidatedGraphSemantics {
    report: Value,
    gaps: Vec<SemanticGap>,
    counts: HeaderCounts,
    roots: CoreRoots,
}
impl ValidatedGraphSemantics {
    pub(crate) fn new(
        report: Value,
        gaps: Vec<SemanticGap>,
        counts: HeaderCounts,
        roots: CoreRoots,
    ) -> Result<Self> {
        if counts.rows != [roots.nodes, roots.relations] {
            return Err(Error::Invalid("semantic header row coverage"));
        }
        Ok(Self {
            report,
            gaps,
            counts,
            roots,
        })
    }
    pub(crate) fn into_report(mut self, state: Option<&CreationState<'_>>) -> Result<Value> {
        if let Some(state) = state {
            state.retain(
                self.gaps
                    .len()
                    .checked_mul(std::mem::size_of::<Value>())
                    .ok_or(Error::Budget("semantic report array"))?,
            )?;
        }
        let mut gaps = Vec::with_capacity(self.gaps.len());
        for gap in self.gaps {
            if let Some(state) = state {
                state.charge_work(gap.id.len() + gap.kind.len())?;
                state.retain(
                    crate::knowledge_normalization::serde_object_slots_upper(2)?
                        + "id".len()
                        + "kind".len(),
                )?;
            }
            let mut object = serde_json::Map::new();
            object.insert("id".into(), Value::String(gap.id));
            object.insert("kind".into(), Value::String(gap.kind));
            gaps.push(Value::Object(object));
        }
        self.report["gaps"] = Value::Array(gaps);
        Ok(self.report)
    }
    fn counts_for(&self, roots: &CoreRoots) -> Result<&HeaderCounts> {
        if self.roots.nodes != roots.nodes
            || self.roots.relations != roots.relations
            || self.roots.node_sha256 != roots.node_sha256
            || self.roots.relation_sha256 != roots.relation_sha256
        {
            return Err(Error::Invalid("semantic header graph changed"));
        }
        Ok(&self.counts)
    }
}

pub(crate) fn build_public_header(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    source_revision: &str,
    processor_digest: Digest256,
    configuration_digest: Digest256,
    semantics: ValidatedGraphSemantics,
) -> Result<Value> {
    if !stage.public_build() {
        return Err(Error::Invalid("public D1 stage required"));
    }
    build_public_header_captured(
        stage,
        capture,
        registry,
        entity_bytes,
        source_revision,
        processor_digest,
        configuration_digest,
        semantics,
    )
}

pub(crate) fn build_native_snapshot_header(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    source_revision: &str,
    processor_digest: Digest256,
    configuration_digest: Digest256,
    semantics: ValidatedGraphSemantics,
) -> Result<Value> {
    if stage.public_build()
        || stage.exact_receipt()?.binding.owner_profile != "tos-native-projection-snapshot-v1"
    {
        return Err(Error::Invalid("native snapshot stage required"));
    }
    build_public_header_captured(
        stage,
        capture,
        registry,
        entity_bytes,
        source_revision,
        processor_digest,
        configuration_digest,
        semantics,
    )
}

fn build_public_header_captured(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    source_revision: &str,
    processor_digest: Digest256,
    configuration_digest: Digest256,
    semantics: ValidatedGraphSemantics,
) -> Result<Value> {
    let semantic_report = &semantics.report;
    if let Some(state) = stage.owned_creation_state() {
        return build_owned_header(
            stage,
            capture,
            registry,
            entity_bytes,
            source_revision,
            processor_digest,
            configuration_digest,
            semantics,
            state,
        );
    }
    if source_revision.len() != 64
        || !source_revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        || semantic_report.get("valid") != Some(&Value::Bool(true))
        || semantic_report.get("violations") != Some(&json!([]))
    {
        return Err(Error::Invalid("public D1 graph header authority/semantics"));
    }
    if Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256 {
        return Err(Error::Invalid("public D1 entity registry bytes"));
    }
    let roots = stage.core_roots()?;
    let HeaderCounts {
        sources,
        states,
        mapped,
        missing,
        cross_layer,
        ..
    } = semantics.counts_for(&roots)?;
    let entity: Value = serde_json::from_slice(entity_bytes)
        .map_err(|_| Error::Invalid("public D1 entity registry JSON"))?;
    let definitions = entity["property_definitions"]
        .as_array()
        .ok_or(Error::Invalid("public D1 property definitions"))?;
    let query_properties = definitions
        .iter()
        .map(|definition| {
            let mut packet = serde_json::Map::new();
            for key in [
                "property_id",
                "field",
                "value_type",
                "applies_to",
                "inherited",
                "operators",
            ] {
                packet.insert(
                    key.to_owned(),
                    definition
                        .get(key)
                        .cloned()
                        .ok_or(Error::Invalid("public D1 property definition"))?,
                );
            }
            Ok(Value::Object(packet))
        })
        .collect::<Result<Vec<_>>>()?;
    if mapped[0] > roots.nodes || mapped[1] > roots.relations {
        return Err(Error::Invalid("public D1 semantic mapping counts"));
    }
    let mut header = json!({
        "schema":"tos_knowledge_graph_v1",
        "source_revision":source_revision,
        "normalization_binding":{
            "schema":"tos_knowledge_graph_normalization_binding_v1",
            "processor_digest":processor_digest.to_hex(),
            "entity_registry_digest":registry.entity_semantic_digest,
            "relation_registry_digest":registry.relation_semantic_digest,
            "configuration_digest":configuration_digest.to_hex()
        },
        "query_properties":query_properties,
        "counts":{
            "nodes":roots.nodes,"relations":roots.relations,"sources":sources,
            "display_coverage":{
                "node_titles":roots.nodes,"node_summaries":roots.nodes,
                "node_summary_states":states[0],"nodes_without_source_summary":missing[0],
                "relation_labels":roots.relations,"relation_statements":roots.relations,
                "relation_explanations":roots.relations,
                "relation_explanation_states":states[1],
                "relations_without_source_explanation":missing[1]
            },
            "semantic_mapping":{
                "mapped_nodes":mapped[0],"unmapped_nodes":roots.nodes-mapped[0],
                "mapped_relations":mapped[1],"unmapped_relations":roots.relations-mapped[1],
                "cross_layer_relations":cross_layer
            },
            "semantic_validation":null
        },
        "authority_boundary":{
            "is_source":false,"is_canon":false,"writes_to_tree":false,
            "source_owner":"Tree-of-Sophia",
            "note":"This normalized graph is a consumer read model. Every item returns to its ToS source_refs."
        }
    });
    header["counts"]["semantic_validation"] = semantics.into_report(None)?;
    Ok(header)
}

const HEADER_TEMPLATE: &[u8] = br#"{
    "schema":"tos_knowledge_graph_v1","source_revision":"",
    "normalization_binding":{"schema":"tos_knowledge_graph_normalization_binding_v1","processor_digest":"","entity_registry_digest":"","relation_registry_digest":"","configuration_digest":""},
    "query_properties":[],
    "counts":{"nodes":0,"relations":0,"sources":{},
        "display_coverage":{"node_titles":0,"node_summaries":0,"node_summary_states":{},"nodes_without_source_summary":0,"relation_labels":0,"relation_statements":0,"relation_explanations":0,"relation_explanation_states":{},"relations_without_source_explanation":0},
        "semantic_mapping":{"mapped_nodes":0,"unmapped_nodes":0,"mapped_relations":0,"unmapped_relations":0,"cross_layer_relations":0},"semantic_validation":null},
    "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false,"source_owner":"Tree-of-Sophia","note":"This normalized graph is a consumer read model. Every item returns to its ToS source_refs."}}
"#;
fn increment_count(
    map: &mut BTreeMap<String, u64>,
    key: &str,
    state: &CreationState<'_>,
) -> Result<()> {
    if let Some(count) = map.get_mut(key) {
        *count = count
            .checked_add(1)
            .ok_or(Error::Budget("owned header count"))?;
    } else {
        let node =
            11 * std::mem::size_of::<(String, u64)>() + 12 * std::mem::size_of::<usize>() + 64;
        state.retain(
            node.checked_add(key.len())
                .ok_or(Error::Budget("owned header map state"))?,
        )?;
        map.insert(key.to_owned(), 1);
    }
    Ok(())
}
fn build_owned_header(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    source_revision: &str,
    processor: Digest256,
    configuration: Digest256,
    semantics: ValidatedGraphSemantics,
    state: &CreationState<'_>,
) -> Result<Value> {
    if source_revision.len() != 64
        || !source_revision.bytes().all(|b| b.is_ascii_hexdigit())
        || semantics.report.get("valid") != Some(&Value::Bool(true))
        || !semantics
            .report
            .get("violations")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        return Err(Error::Invalid("public D1 graph header authority/semantics"));
    }
    let _digest_hold = state.hold(64)?;
    state.charge_work(entity_bytes.len())?;
    if Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256 {
        return Err(Error::Invalid("public D1 entity registry bytes"));
    }
    let roots = stage.core_roots()?;
    let HeaderCounts {
        sources,
        states,
        mapped,
        missing,
        cross_layer,
        ..
    } = semantics.counts_for(&roots)?;
    let entity_limits = tos_foundation::JsonLimits::new(4 * 1024 * 1024, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("owned header registry JSON"))?;
    let properties = state.with_serde_owned_with_limits(entity_bytes, entity_limits, |entity| {
        let definitions = entity["property_definitions"]
            .as_array()
            .ok_or(Error::Invalid("public D1 property definitions"))?;
        state.retain(
            definitions
                .len()
                .checked_mul(std::mem::size_of::<Value>())
                .ok_or(Error::Budget("owned header property slots"))?,
        )?;
        let mut output = Vec::with_capacity(definitions.len());
        for definition in definitions {
            state.active()?;
            state.retain(
                crate::knowledge_normalization::serde_object_slots_upper(6)?
                    + [
                        "property_id",
                        "field",
                        "value_type",
                        "applies_to",
                        "inherited",
                        "operators",
                    ]
                    .into_iter()
                    .map(str::len)
                    .sum::<usize>(),
            )?;
            let mut packet = serde_json::Map::new();
            for key in [
                "property_id",
                "field",
                "value_type",
                "applies_to",
                "inherited",
                "operators",
            ] {
                let value = definition
                    .get(key)
                    .ok_or(Error::Invalid("public D1 property definition"))?;
                packet.insert(key.to_owned(), state.clone_value(value)?);
            }
            output.push(Value::Object(packet));
        }
        Ok(output)
    })?;
    if mapped[0] > roots.nodes || mapped[1] > roots.relations {
        return Err(Error::Invalid("public D1 semantic mapping counts"));
    }
    let mut output = state.serde_owned(HEADER_TEMPLATE, HEADER_TEMPLATE.len())?;
    state.retain(
        64 + 64
            + 64
            + registry.entity_semantic_digest.len()
            + registry.relation_semantic_digest.len()
            + 20 * 18,
    )?;
    output["source_revision"] = Value::String(source_revision.to_owned());
    output["normalization_binding"]["processor_digest"] = Value::String(processor.to_hex());
    output["normalization_binding"]["entity_registry_digest"] =
        Value::String(registry.entity_semantic_digest.clone());
    output["normalization_binding"]["relation_registry_digest"] =
        Value::String(registry.relation_semantic_digest.clone());
    output["normalization_binding"]["configuration_digest"] = Value::String(configuration.to_hex());
    output["query_properties"] = Value::Array(properties);
    for (key, value) in [("nodes", roots.nodes), ("relations", roots.relations)] {
        output["counts"][key] = Value::from(value);
    }
    output["counts"]["sources"] = state.with_json_encoded(&sources, 4 * 1024 * 1024, |raw| {
        state.serde_owned(raw, 4 * 1024 * 1024)
    })?;
    let display = &mut output["counts"]["display_coverage"];
    for (key, value) in [
        ("node_titles", roots.nodes),
        ("node_summaries", roots.nodes),
        ("nodes_without_source_summary", missing[0]),
        ("relation_labels", roots.relations),
        ("relation_statements", roots.relations),
        ("relation_explanations", roots.relations),
        ("relations_without_source_explanation", missing[1]),
    ] {
        display[key] = Value::from(value);
    }
    display["node_summary_states"] =
        state.with_json_encoded(&states[0], 4 * 1024 * 1024, |raw| {
            state.serde_owned(raw, 4 * 1024 * 1024)
        })?;
    display["relation_explanation_states"] =
        state.with_json_encoded(&states[1], 4 * 1024 * 1024, |raw| {
            state.serde_owned(raw, 4 * 1024 * 1024)
        })?;
    let mapping = &mut output["counts"]["semantic_mapping"];
    for (key, value) in [
        ("mapped_nodes", mapped[0]),
        ("unmapped_nodes", roots.nodes - mapped[0]),
        ("mapped_relations", mapped[1]),
        ("unmapped_relations", roots.relations - mapped[1]),
        ("cross_layer_relations", *cross_layer),
    ] {
        mapping[key] = Value::from(value);
    }
    output["counts"]["semantic_validation"] = semantics.into_report(Some(state))?;
    state.active()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folded_counts_preserve_display_and_refuse_another_graph() {
        let mut counts = HeaderCounts::default();
        for source in ["alpha", "beta", "alpha"] {
            counts.observe(0, source, &json!({
                "display":{"summary_state":"source","provenance":{"source_summary_available":true}},
                "type_mapping":{"status":"mapped"}
            }), None).unwrap();
        }
        counts.observe(1, "semantic-interchange", &json!({
            "display":{"explanation_state":"missing","provenance":{"source_explanation_available":false}},
            "predicate_mapping":{"status":"unmapped"}
        }), None).unwrap();
        assert_eq!(
            counts.sources,
            BTreeMap::from([("alpha".into(), 2), ("beta".into(), 1)])
        );
        assert_eq!(counts.states[0].get("source"), Some(&3));
        assert_eq!(counts.states[1].get("missing"), Some(&1));
        assert_eq!(counts.mapped, [3, 0]);
        assert_eq!(counts.missing, [0, 1]);
        assert_eq!(counts.cross_layer, 1);
        let mut roots = CoreRoots {
            nodes: 3,
            relations: 1,
            node_sha256: "a".repeat(64),
            relation_sha256: "b".repeat(64),
        };
        let validated = ValidatedGraphSemantics::new(
            json!({"valid":true,"gaps":[]}),
            Vec::new(),
            counts,
            roots.clone(),
        )
        .unwrap();
        assert!(validated.counts_for(&roots).is_ok());
        roots.node_sha256 = "c".repeat(64);
        assert!(validated.counts_for(&roots).is_err());
        assert!(
            ValidatedGraphSemantics::new(json!({}), Vec::new(), HeaderCounts::default(), roots)
                .is_err()
        );
        assert!(
            HeaderCounts::default()
                .observe(0, "alpha", &json!({}), None)
                .is_err()
        );
    }
}
