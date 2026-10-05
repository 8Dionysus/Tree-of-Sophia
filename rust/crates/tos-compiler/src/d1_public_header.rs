//! Header for the disposable public graph. Counts describe exactly the
//! path-adapted rows used by Catalog, Search and the D1 SQL transport.

use crate::{
    Error, KnowledgeRegistry, Result,
    d1_public_capture::{CreationState, PublicCapture},
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
        semantic_report,
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
    semantic_report: &Value,
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
        semantic_report,
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
    semantic_report: &Value,
) -> Result<Value> {
    if let Some(state) = stage.owned_creation_state() {
        return build_owned_header(
            stage,
            capture,
            registry,
            entity_bytes,
            source_revision,
            processor_digest,
            configuration_digest,
            semantic_report,
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
    semantics: &Value,
    state: &CreationState<'_>,
) -> Result<Value> {
    if source_revision.len() != 64
        || !source_revision.bytes().all(|b| b.is_ascii_hexdigit())
        || semantics.get("valid") != Some(&Value::Bool(true))
        || !semantics
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
    let (sources,states,mapped,missing,cross_layer)=stage.with_connection(WritePhase::Finalize,|db| {
        let mut sources=BTreeMap::<String,u64>::new();
        let mut states=[BTreeMap::<String,u64>::new(),BTreeMap::new()];
        let mut mapped=[0u64;2]; let mut missing=[0u64;2]; let mut cross_layer=0u64;
        let _query_hold=state.hold(512+tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())?;
        for (kind,table) in ["knowledge_nodes","knowledge_relations"].into_iter().enumerate() {
            let mut stmt=db.prepare(&format!("SELECT source_graph,payload_len,payload_sha256,CASE WHEN payload_len<=8000000 AND length(payload)=payload_len THEN payload ELSE NULL END FROM {table} ORDER BY source_order"))?;
            let mut rows=stmt.query([])?;
            while let Some(row)=rows.next()? {
                state.active()?;
                let source=row.get_ref(0)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?; let length:i64=row.get(1)?;
                let sha=row.get_ref(2)?.as_blob().map_err(|_| Error::Invalid("public D1 SQL blob"))?;
                let raw=row.get_ref(3)?.as_blob().map_err(|_|Error::Budget("public D1 header row bytes"))?;
                if length<0 || length as usize !=raw.len() || raw.len()>8_000_000 || sha.len()!=32 {
                    return Err(Error::Invalid("public D1 header row digest"));
                }
                capture.charge_work(raw.len() as u64)?;
                if sha!=Digest256::of_bytes(raw).as_bytes() { return Err(Error::Invalid("public D1 header row digest")); }
                let limits=tos_foundation::JsonLimits::new(8_000_000,96,1_000_000,4096)
                    .map_err(|_|Error::Budget("owned header row JSON"))?;
                state.with_serde_owned_with_limits(raw,limits,|value| {
                    if kind==0 { increment_count(&mut sources,source,state)?; }
                    else if source=="semantic-interchange" { cross_layer=cross_layer.checked_add(1).ok_or(Error::Budget("owned header cross-layer count"))?; }
                    let display_key=if kind==0 {"summary_state"} else {"explanation_state"};
                    let display=value["display"][display_key].as_str().ok_or(Error::Invalid("public D1 display state"))?;
                    increment_count(&mut states[kind],display,state)?;
                    let mapping=if kind==0 {"type_mapping"} else {"predicate_mapping"};
                    if value[mapping]["status"]=="mapped" { mapped[kind]=mapped[kind].checked_add(1).ok_or(Error::Budget("owned header mapped count"))?; }
                    let availability=if kind==0 {"source_summary_available"} else {"source_explanation_available"};
                    if value["display"]["provenance"][availability]==false { missing[kind]=missing[kind].checked_add(1).ok_or(Error::Budget("owned header missing count"))?; }
                    Ok(())
                })?;
            }
        }
        Ok((sources,states,mapped,missing,cross_layer))
    })?;
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
        ("cross_layer_relations", cross_layer),
    ] {
        mapping[key] = Value::from(value);
    }
    output["counts"]["semantic_validation"] = state.clone_value(semantics)?;
    state.active()?;
    Ok(output)
}
