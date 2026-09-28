//! Header for the disposable public graph. Counts describe exactly the
//! path-adapted rows used by Catalog, Search and the D1 SQL transport.

use crate::{
    Error, KnowledgeRegistry, Result,
    d1_public_capture::PublicCapture,
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tos_foundation::Digest256;

pub(crate) fn build_public_header(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    source_revision: &str,
    processor_digest: Digest256,
    configuration_digest: Digest256,
    semantic_report: &Value,
) -> Result<Value> {
    if !stage.public_build()
        || source_revision.len() != 64
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
    let (sources, states, mapped, missing, cross_layer) =
        stage.with_connection(WritePhase::Finalize, |db| {
            let mut sources = BTreeMap::<String, u64>::new();
            let mut states = [BTreeMap::<String, u64>::new(), BTreeMap::new()];
            let mut mapped = [0u64; 2];
            let mut missing = [0u64; 2];
            let mut cross_layer = 0u64;
            for (kind, table) in ["knowledge_nodes", "knowledge_relations"]
                .into_iter()
                .enumerate()
            {
                let mut statement = db.prepare(&format!(
                    "SELECT source_graph,payload_len,payload_sha256,\
                     CASE WHEN payload_len<=8000000 AND length(payload)=payload_len THEN payload ELSE NULL END \
                     FROM {table} ORDER BY source_order"
                ))?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    let source: String = row.get(0)?;
                    let length: i64 = row.get(1)?;
                    let sha: Vec<u8> = row.get(2)?;
                    let raw: Option<Vec<u8>> = row.get(3)?;
                    let raw = raw.ok_or(Error::Budget("public D1 header row bytes"))?;
                    if length < 0
                        || length as usize != raw.len()
                        || sha.as_slice() != Digest256::of_bytes(&raw).as_bytes()
                    {
                        return Err(Error::Invalid("public D1 header row digest"));
                    }
                    capture.charge_work(raw.len() as u64)?;
                    let value: Value = serde_json::from_slice(&raw)
                        .map_err(|error| Error::Source(error.to_string()))?;
                    if kind == 0 {
                        *sources.entry(source.clone()).or_default() += 1;
                    } else if source == "semantic-interchange" {
                        cross_layer += 1;
                    }
                    let state = if kind == 0 {
                        "summary_state"
                    } else {
                        "explanation_state"
                    };
                    let state = value["display"][state]
                        .as_str()
                        .ok_or(Error::Invalid("public D1 display state"))?;
                    *states[kind].entry(state.to_owned()).or_default() += 1;
                    let mapping = if kind == 0 {
                        "type_mapping"
                    } else {
                        "predicate_mapping"
                    };
                    if value[mapping]["status"] == "mapped" {
                        mapped[kind] += 1;
                    }
                    let availability = if kind == 0 {
                        "source_summary_available"
                    } else {
                        "source_explanation_available"
                    };
                    if value["display"]["provenance"][availability] == false {
                        missing[kind] += 1;
                    }
                }
            }
            Ok((sources, states, mapped, missing, cross_layer))
        })?;
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
    Ok(json!({
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
            "semantic_validation":semantic_report
        },
        "authority_boundary":{
            "is_source":false,"is_canon":false,"writes_to_tree":false,
            "source_owner":"Tree-of-Sophia",
            "note":"This normalized graph is a consumer read model. Every item returns to its ToS source_refs."
        }
    }))
}
