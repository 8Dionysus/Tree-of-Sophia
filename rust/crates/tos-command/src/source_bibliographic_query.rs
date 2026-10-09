//! Bounded exact-selector query over a source-verified bibliographic graph.
//!
//! Source capture, projection composition, and tracked-file parity belong to
//! `source_corpus_index_projection` and its native caller. This module only
//! selects complete claim bundles from the already verified projection.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256;

const GRAPH_REF: &str =
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json";
const GRAPH_SCHEMA: &str = "tos_source_witness_bibliographic_graph_v1";
const QUERY_SCHEMA: &str = "tos_source_witness_bibliographic_query_result_v1";
const MAX_QUERY_LIMIT: u64 = 100;

fn canonical(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|error| format!("query JSON encoding: {error}"))
}

fn digest(bytes: &[u8]) -> String {
    Digest256::of_bytes(bytes).to_hex()
}

fn required_text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("bibliographic graph lacks text field {key}"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("bibliographic graph lacks array {key}"))
}

fn node<'a>(nodes: &'a BTreeMap<String, &'a Value>, id: &str) -> Result<&'a Value, String> {
    nodes
        .get(id)
        .copied()
        .ok_or_else(|| format!("bibliographic trace names missing node {id}"))
}

fn node_list<'a>(
    nodes: &'a BTreeMap<String, &'a Value>,
    trace: &Value,
    key: &str,
) -> Result<Vec<&'a Value>, String> {
    let Some(values) = trace.get(key) else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| format!("bibliographic trace {key} must be a list"))?;
    values
        .iter()
        .map(|value| {
            let id = value
                .as_str()
                .ok_or_else(|| format!("bibliographic trace {key} has a non-text id"))?;
            node(nodes, id)
        })
        .collect()
}

/// Query one graph output only after the caller has completed source capture,
/// native composition, schema validation, and tracked projection parity.
/// Selectors are exact and conjunctive. `max_output_bytes` bounds the full
/// returned source bundle; matches are never silently truncated.
pub fn query_verified_projection(
    graph_bytes: &[u8],
    selectors: &Value,
    limit: Option<u64>,
    max_output_bytes: u64,
) -> Result<Value, String> {
    if graph_bytes.len() as u64 > max_output_bytes {
        return Err("source graph exceeds query byte budget".into());
    }
    let mut graph: Value = serde_json::from_slice(graph_bytes)
        .map_err(|error| format!("cannot parse source graph: {error}"))?;
    if graph.get("schema_version").and_then(Value::as_str) != Some(GRAPH_SCHEMA) {
        return Err("source graph has an unexpected schema".into());
    }
    graph
        .as_object()
        .ok_or("source graph root must be an object")?;
    // Temporarily remove only the fingerprint field rather than cloning the
    // complete graph. The caller supplies an explicit graph/output byte cap.
    let fingerprint = graph
        .get("projection_fingerprint")
        .and_then(Value::as_str)
        .ok_or("bibliographic graph lacks text field projection_fingerprint")?
        .to_owned();
    let saved_fingerprint = graph
        .as_object_mut()
        .ok_or("source graph root must be an object")?
        .remove("projection_fingerprint")
        .ok_or("source graph lacks projection fingerprint")?;
    let expected_fingerprint = digest(&canonical(&graph)?);
    graph
        .as_object_mut()
        .ok_or("source graph root must be an object")?
        .insert("projection_fingerprint".into(), saved_fingerprint);
    if fingerprint != expected_fingerprint {
        return Err("projection fingerprint does not match graph content".into());
    }

    let requested_limit = limit.unwrap_or(20);
    if !(1..=MAX_QUERY_LIMIT).contains(&requested_limit) {
        return Err("query limit must be an integer from 1 to 100".into());
    }
    let selector_fields = [
        "claim_ref",
        "subject_ref",
        "object_ref",
        "normalized_ref",
        "predicate",
        "review_status",
        "visibility",
    ];
    let selector_object = selectors
        .as_object()
        .ok_or("query selectors must be an object")?;
    if selector_object
        .keys()
        .any(|key| !selector_fields.contains(&key.as_str()))
    {
        return Err("query contains an unsupported selector".into());
    }
    let mut selected_fields = Map::new();
    for key in selector_fields {
        if let Some(value) = selector_object.get(key) {
            if !value.is_string() {
                return Err(format!("query selector {key} must be text"));
            }
            selected_fields.insert(key.to_owned(), value.clone());
        }
    }
    if selected_fields.is_empty() {
        return Err("at least one exact query selector is required".into());
    }

    let mut nodes = BTreeMap::new();
    for value in array(&graph, "nodes")? {
        let id = required_text(value, "node_id")?;
        if nodes.insert(id.to_owned(), value).is_some() {
            return Err("source graph has duplicate node ids".into());
        }
    }

    let mut selected_traces = Vec::new();
    let mut matched_count = 0u64;
    for trace in array(&graph, "claim_traces")? {
        let subject = node(&nodes, required_text(trace, "subject_node_id")?)?;
        let object = node(&nodes, required_text(trace, "object_node_id")?)?;
        let subject_ref = subject
            .pointer("/properties/identity_ref")
            .and_then(Value::as_str);
        let object_ref = (object.get("node_kind").and_then(Value::as_str) == Some("identity"))
            .then(|| object.pointer("/properties/identity_ref").and_then(Value::as_str))
            .flatten();
        let mut normalized_refs = BTreeSet::new();
        if let Some(ids) = trace.get("normalized_identity_node_ids") {
            let ids = ids
                .as_array()
                .ok_or("normalized_identity_node_ids must be a list")?;
            for id in ids {
                let id = id
                    .as_str()
                    .ok_or("normalized identity node id must be text")?;
                let normalized = node(&nodes, id)?
                    .pointer("/properties/identity_ref")
                    .and_then(Value::as_str)
                    .ok_or("normalized identity node lacks identity_ref")?;
                normalized_refs.insert(normalized.to_owned());
            }
        }
        let mut matches = true;
        for (key, selected) in &selected_fields {
            let expected = selected.as_str().unwrap_or_default();
            let actual = match key.as_str() {
                "claim_ref" | "predicate" | "review_status" | "visibility" => {
                    trace.get(key).and_then(Value::as_str)
                }
                "subject_ref" => subject_ref,
                "object_ref" => object_ref,
                "normalized_ref" => normalized_refs
                    .contains(expected)
                    .then_some(expected),
                _ => None,
            };
            if actual != Some(expected) {
                matches = false;
                break;
            }
        }
        if matches {
            matched_count = matched_count
                .checked_add(1)
                .ok_or("bibliographic match count overflow")?;
            selected_traces.push(trace);
        }
    }
    if matched_count > requested_limit {
        return Err(format!(
            "query matched {matched_count} claims, exceeding explicit limit {requested_limit}"
        ));
    }
    selected_traces.sort_by_key(|trace| {
        trace
            .get("claim_ref")
            .and_then(Value::as_str)
            .unwrap_or_default()
    });

    let selected_claims: BTreeSet<String> = selected_traces
        .iter()
        .map(|trace| required_text(trace, "claim_ref").map(str::to_owned))
        .collect::<Result<_, _>>()?;
    let mut selected_edges = BTreeMap::<String, Vec<&Value>>::new();
    for edge in array(&graph, "edges")? {
        let claim_ref = required_text(edge, "claim_ref")?;
        if selected_claims.contains(claim_ref) {
            selected_edges
                .entry(claim_ref.to_owned())
                .or_default()
                .push(edge);
        }
    }
    for edges in selected_edges.values_mut() {
        edges.sort_by_key(|edge| edge.get("edge_id").and_then(Value::as_str).unwrap_or_default());
    }

    let mut matches = Vec::with_capacity(selected_traces.len());
    for trace in selected_traces {
        let claim_ref = required_text(trace, "claim_ref")?;
        let claim_node = node(&nodes, required_text(trace, "claim_node_id")?)?;
        let source_claim = claim_node
            .pointer("/properties/source_claim")
            .ok_or("claim node lacks exact source_claim return")?;
        if source_claim.get("claim_id").and_then(Value::as_str) != Some(claim_ref)
            || digest(&canonical(source_claim)?)
                != required_text(trace, "source_claim_sha256")?
        {
            return Err(format!("{claim_ref}: source-return identity or digest differs"));
        }
        let edges = selected_edges
            .remove(claim_ref)
            .unwrap_or_default()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        matches.push(json!({
            "claim_ref": claim_ref,
            "predicate": required_text(trace, "predicate")?,
            "claim_sha256": required_text(trace, "claim_sha256")?,
            "source_return": {
                "file_ref": required_text(trace, "source_claim_file_ref")?,
                "line": trace.get("source_claim_line").ok_or("trace lacks source_claim_line")?,
                "canonical_sha256": required_text(trace, "source_claim_sha256")?,
                "source_claim": source_claim,
            },
            "trace": trace,
            "claim_node": claim_node,
            "subject_node": node(&nodes, required_text(trace, "subject_node_id")?)?,
            "object_node": node(&nodes, required_text(trace, "object_node_id")?)?,
            "evidence_nodes": node_list(&nodes, trace, "evidence_node_ids")?,
            "counterevidence_nodes": node_list(&nodes, trace, "counterevidence_node_ids")?,
            "maker_node": node(&nodes, required_text(trace, "maker_node_id")?)?,
            "provenance_event_node": node(&nodes, required_text(trace, "provenance_event_node_id")?)?,
            "review_nodes": node_list(&nodes, trace, "review_node_ids")?,
            "normalized_identity_nodes": node_list(&nodes, trace, "normalized_identity_node_ids")?,
            "edges": edges,
        }));
    }

    let query = json!({
        "match_semantics": "all_selectors",
        "selectors": selected_fields,
        "limit": requested_limit,
    });
    let claim_refs: Vec<Value> = matches
        .iter()
        .map(|item| item["claim_ref"].clone())
        .collect();
    let query_material = json!({
        "source_graph_fingerprint": graph["projection_fingerprint"],
        "query": query,
        "claim_refs": claim_refs,
    });
    let mut graph_material = canonical(&graph)?;
    graph_material.push(b'\n');
    let result = json!({
        "schema_version": QUERY_SCHEMA,
        "status": if matches.is_empty() { "no_match" } else { "ok" },
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "ephemeral_source_witness_bibliographic_query_result",
        "source_graph_ref": GRAPH_REF,
        "source_graph_sha256": digest(&graph_material),
        "source_graph_digest_scope": "canonical-logical-json-with-trailing-newline",
        "source_graph_fingerprint": graph["projection_fingerprint"],
        "query": query,
        "query_fingerprint": digest(&canonical(&query_material)?),
        "result_count": matches.len(),
        "matches": matches,
        "authority_boundary": {
            "role": "deterministic read-only source return over the generated bibliographic graph",
            "does_not_establish": [],
        },
    });
    let output = canonical(&result)?;
    if output.len() as u64 > max_output_bytes {
        return Err("bibliographic query result exceeds output byte budget".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_bytes() -> Vec<u8> {
        let source_claim = json!({
            "claim_id": "tos.claim.query-fixture",
            "claim_type": "relation",
            "schema_version": "tos_source_claim_v1"
        });
        let source_sha = digest(&canonical(&source_claim).unwrap());
        let mut graph = json!({
            "schema_version": GRAPH_SCHEMA,
            "nodes": [
                {"node_id":"claim:fixture","node_kind":"claim","properties":{"source_claim":source_claim}},
                {"node_id":"identity:subject","node_kind":"identity","properties":{"identity_ref":"tos.work.query-subject"}},
                {"node_id":"identity:object","node_kind":"identity","properties":{"identity_ref":"tos.work.query-object"}},
                {"node_id":"maker:fixture","node_kind":"maker","properties":{}},
                {"node_id":"event:fixture","node_kind":"provenance_event","properties":{}}
            ],
            "edges": [
                {"edge_id":"edge:fixture","claim_ref":"tos.claim.query-fixture"}
            ],
            "claim_traces": [{
                "claim_ref":"tos.claim.query-fixture",
                "claim_node_id":"claim:fixture",
                "subject_node_id":"identity:subject",
                "object_node_id":"identity:object",
                "maker_node_id":"maker:fixture",
                "provenance_event_node_id":"event:fixture",
                "evidence_node_ids":[],
                "counterevidence_node_ids":[],
                "review_node_ids":[],
                "normalized_identity_node_ids":[],
                "predicate":"has_expression",
                "claim_sha256":source_sha,
                "source_claim_file_ref":"ToS/source-witnesses/relations/query-fixture/claims.jsonl",
                "source_claim_line":1,
                "source_claim_sha256":source_sha,
                "review_status":"unreviewed",
                "visibility":"public_metadata_only"
            }],
            "input_digests":{}
        });
        let fingerprint = digest(&canonical(&graph).unwrap());
        graph["projection_fingerprint"] = json!(fingerprint);
        canonical(&graph).unwrap()
    }

    #[test]
    fn exact_query_returns_one_complete_source_bundle() {
        let graph = graph_bytes();
        let query = query_verified_projection(
            &graph,
            &json!({
                "claim_ref":"tos.claim.query-fixture",
                "subject_ref":"tos.work.query-subject",
                "predicate":"has_expression"
            }),
            None,
            1024 * 1024,
        )
        .unwrap();
        assert_eq!(query["status"], "ok");
        assert_eq!(query["result_count"], 1);
        assert_eq!(query["matches"][0]["claim_ref"], "tos.claim.query-fixture");
        assert_eq!(query["matches"][0]["source_return"]["source_claim"]["claim_id"], "tos.claim.query-fixture");
        assert_eq!(query["matches"][0]["edges"][0]["edge_id"], "edge:fixture");
        assert_eq!(query["query"]["match_semantics"], "all_selectors");
        let rendered = serde_json::to_string(&query).unwrap();
        assert!(!rendered.contains("/srv/"));
        assert!(!rendered.contains("/home/"));
    }

    #[test]
    fn query_requires_a_selector_and_refuses_unbounded_match_limit() {
        let graph = graph_bytes();
        assert!(query_verified_projection(&graph, &json!({}), None, 1024 * 1024)
            .unwrap_err()
            .contains("at least one exact query selector"));
        assert!(query_verified_projection(
            &graph,
            &json!({"review_status":"unreviewed"}),
            Some(101),
            1024 * 1024,
        )
        .unwrap_err()
        .contains("1 to 100"));
    }

    #[test]
    fn normalized_identity_query_returns_full_literal_claim_bundle_and_no_match() {
        let mut graph: Value = serde_json::from_slice(&graph_bytes()).unwrap();
        graph["nodes"].as_array_mut().unwrap().push(json!({
            "node_id":"identity:normalized",
            "node_kind":"identity",
            "properties":{"identity_ref":"tos.place.query-place","identity_kind":"place"}
        }));
        graph["nodes"].as_array_mut().unwrap().push(json!({
            "node_id":"literal:fixture",
            "node_kind":"literal",
            "properties":{"value":{"temporal":{"role":"statement_date"}}}
        }));
        let trace = &mut graph["claim_traces"][0];
        trace["object_node_id"] = json!("literal:fixture");
        trace["normalized_identity_node_ids"] = json!(["identity:normalized"]);
        graph
            .as_object_mut()
            .unwrap()
            .remove("projection_fingerprint")
            .unwrap();
        let fingerprint = digest(&canonical(&graph).unwrap());
        graph.as_object_mut().unwrap().insert(
            "projection_fingerprint".into(),
            json!(fingerprint),
        );
        let raw = canonical(&graph).unwrap();

        let query = query_verified_projection(
            &raw,
            &json!({"predicate":"has_expression","normalized_ref":"tos.place.query-place"}),
            None,
            1024 * 1024,
        )
        .unwrap();
        assert_eq!(query["result_count"], 1);
        assert_eq!(query["matches"][0]["object_node"]["node_kind"], "literal");
        assert_eq!(
            query["matches"][0]["normalized_identity_nodes"][0]["properties"]["identity_ref"],
            "tos.place.query-place"
        );

        let absent = query_verified_projection(
            &raw,
            &json!({"normalized_ref":"tos.place.no-such-place"}),
            None,
            1024 * 1024,
        )
        .unwrap();
        assert_eq!(absent["status"], "no_match");
        assert_eq!(absent["matches"], json!([]));
    }

    #[test]
    fn query_refuses_a_result_larger_than_its_declared_output_budget() {
        let graph = graph_bytes();
        assert!(query_verified_projection(
            &graph,
            &json!({"claim_ref":"tos.claim.query-fixture"}),
            None,
            1,
        )
        .unwrap_err()
        .contains("exceeds output byte budget"));
    }

    #[test]
    fn query_fails_instead_of_silently_truncating_matches() {
        let mut graph: Value = serde_json::from_slice(&graph_bytes()).unwrap();
        let second_claim = json!({
            "claim_id":"tos.claim.query-fixture-second",
            "claim_type":"relation",
            "schema_version":"tos_source_claim_v1"
        });
        let second_sha = digest(&canonical(&second_claim).unwrap());
        graph["nodes"].as_array_mut().unwrap().push(json!({
            "node_id":"claim:second",
            "node_kind":"claim",
            "properties":{"source_claim":second_claim}
        }));
        graph["edges"].as_array_mut().unwrap().push(json!({
            "edge_id":"edge:second",
            "claim_ref":"tos.claim.query-fixture-second"
        }));
        let mut trace = graph["claim_traces"][0].clone();
        trace["claim_ref"] = json!("tos.claim.query-fixture-second");
        trace["claim_node_id"] = json!("claim:second");
        trace["claim_sha256"] = json!(second_sha);
        trace["source_claim_file_ref"] = json!("ToS/source-witnesses/relations/query-fixture/claims.jsonl");
        trace["source_claim_line"] = json!(2);
        trace["source_claim_sha256"] = json!(second_sha);
        graph["claim_traces"].as_array_mut().unwrap().push(trace);
        graph
            .as_object_mut()
            .unwrap()
            .remove("projection_fingerprint")
            .unwrap();
        let fingerprint = digest(&canonical(&graph).unwrap());
        graph
            .as_object_mut()
            .unwrap()
            .insert("projection_fingerprint".into(), json!(fingerprint));

        assert!(query_verified_projection(
            &canonical(&graph).unwrap(),
            &json!({"review_status":"unreviewed"}),
            Some(1),
            1024 * 1024,
        )
        .unwrap_err()
        .contains("exceeding explicit limit 1"));
    }
}
