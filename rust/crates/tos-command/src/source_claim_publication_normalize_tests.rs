//! Supplied cohort regression fixtures for the live publication normalizer.
use super::*;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::OnceLock;
use tos_compiler::knowledge_full_fixture::{
    FullKnowledgeFixture, build_native_fixture_with_claim_source,
};

const MAX_CANDIDATE_ROW_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Serialize)]
struct FixtureNode {
    source_graph: String,
    record: Value,
}
#[derive(Clone, Serialize)]
struct FixtureRelation {
    source_graph: String,
    record: Value,
    identity_id: Option<String>,
}
#[derive(Clone, Serialize)]
struct FixtureInput {
    node_records: Vec<FixtureNode>,
    relation_records: Vec<FixtureRelation>,
    retained_nodes: Vec<Value>,
    claim_traces: Vec<Value>,
    source_dossier_refs: Vec<String>,
    context_node_order: Vec<String>,
    normalization_binding: Value,
}
fn limits() -> ClaimCandidateLimits {
    ClaimCandidateLimits {
        max_nodes: 1024,
        max_retained_nodes: 2048,
        max_relations: 2048,
        max_traces: 512,
        max_contexts: 4096,
        max_row_bytes: MAX_CANDIDATE_ROW_BYTES,
        max_input_bytes: 32 * 1024 * 1024,
        max_output_bytes: 32 * 1024 * 1024,
    }
}

struct Fixture {
    native: FullKnowledgeFixture,
    graph: Value,
    traces: Vec<Value>,
}
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let mut source: Value = serde_json::from_slice(include_bytes!(
            "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
        ))
        .unwrap();
        // The frozen capture predates the current predicate registry. Build
        // an explicit synthetic current pair through the same producer used
        // by source admission, without rewriting the historical capture.
        let entities: Value = serde_json::from_slice(include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json"
        ))
        .unwrap();
        let relations: Value = serde_json::from_slice(include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json"
        ))
        .unwrap();
        let originals = source["nodes"].as_array().unwrap().clone();
        let mut rebound = 0;
        for node in source["nodes"].as_array_mut().unwrap() {
            if node["node_kind"] != "claim" {
                continue;
            }
            let claim = &node["properties"]["source_claim"];
            let subject = originals
                .iter()
                .find(|candidate| {
                    candidate["node_kind"] == "identity"
                        && candidate["properties"]["identity_ref"] == claim["subject_ref"]
                })
                .unwrap();
            let object = originals
                .iter()
                .find(|candidate| {
                    candidate["node_kind"] == "literal"
                        && candidate["properties"]["claim_ref"] == claim["claim_id"]
                })
                .unwrap();
            let descriptor =
                tos_compiler::source_bibliographic::supplied_claim_navigation_descriptor(
                    claim,
                    subject,
                    object,
                    &relations,
                    &entities,
                    tos_compiler::source_bibliographic::BibliographicDocumentLimits {
                        input_document_bytes: MAX_CANDIDATE_ROW_BYTES,
                        output_row_bytes: MAX_CANDIDATE_ROW_BYTES,
                    },
                )
                .unwrap()
                .unwrap();
            node["properties"]["navigation_descriptor"] = descriptor;
            rebound += 1;
        }
        assert!(rebound > 0);
        let native = build_native_fixture_with_claim_source(&serde_json::to_vec(&source).unwrap());
        let graph: Value = serde_json::from_slice(&native.graph_input_bytes).unwrap();
        Fixture {
            native,
            graph,
            traces: source["claim_traces"].as_array().unwrap().clone(),
        }
    })
}
fn all_claim_ids(graph: &Value) -> BTreeSet<String> {
    graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["source_graph"] == "source-claims")
        .map(|node| node["id"].as_str().unwrap().to_owned())
        .collect()
}
fn input_for(selected_ids: &BTreeSet<String>) -> FixtureInput {
    let fixture = fixture();
    let graph_nodes = fixture.graph["nodes"].as_array().unwrap();
    let selected = graph_nodes
        .iter()
        .filter(|node| selected_ids.contains(node["id"].as_str().unwrap()))
        .collect::<Vec<_>>();
    let node_records = selected
        .iter()
        .map(|node| FixtureNode {
            source_graph: node["source_graph"].as_str().unwrap().to_owned(),
            record: node["source_record"]["payload"].clone(),
        })
        .collect();
    let retained_nodes = graph_nodes
        .iter()
        .filter(|node| !selected_ids.contains(node["id"].as_str().unwrap()))
        .cloned()
        .collect::<Vec<_>>();
    let relations = fixture.graph["relations"].as_array().unwrap();
    let relation_records = relations
        .iter()
        .filter(|edge| {
            selected_ids.contains(edge["from_id"].as_str().unwrap())
                || selected_ids.contains(edge["to_id"].as_str().unwrap())
        })
        .map(|edge| {
            let native_id = edge["native_id"].as_str().unwrap();
            let identity = edge["id"]
                .as_str()
                .unwrap()
                .strip_prefix(&format!("{}:", edge["source_graph"].as_str().unwrap()))
                .unwrap();
            FixtureRelation {
                source_graph: edge["source_graph"].as_str().unwrap().to_owned(),
                record: edge["source_record"]["payload"].clone(),
                identity_id: (identity != native_id).then(|| identity.to_owned()),
            }
        })
        .collect();
    let source_dossier_refs = graph_nodes
        .iter()
        .filter_map(|node| node.get("source_dossier_ref").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let context_node_order = graph_nodes
        .iter()
        .filter(|node| matches!(node["kind_id"].as_str(), Some("claim" | "annotation-claim")))
        .map(|node| node["id"].as_str().unwrap().to_owned())
        .collect();
    FixtureInput {
        node_records,
        relation_records,
        retained_nodes,
        claim_traces: fixture.traces.clone(),
        source_dossier_refs,
        context_node_order,
        normalization_binding: fixture.graph["normalization_binding"].clone(),
    }
}
fn normalize(input: &FixtureInput, limits: ClaimCandidateLimits) -> Result<Value> {
    fn row(graph: &str, value: &Value, id_field: &str) -> SeekRow {
        let payload = serde_json::to_vec(value).unwrap();
        SeekRow {
            id: value[id_field].as_str().unwrap().to_owned(),
            source_graph: graph.to_owned(),
            source_order: None,
            payload_sha256: Digest256::of_bytes(&payload).to_hex(),
            payload,
        }
    }
    let candidate = ClaimCandidateInput {
        nodes: input
            .node_records
            .iter()
            .map(|node| CandidateNode {
                raw: row(&node.source_graph, &node.record, "node_id"),
                dossier_ref: declared_dossier(&node.record, &node.source_graph),
            })
            .collect(),
        relations: input
            .relation_records
            .iter()
            .map(|edge| CandidateRelation {
                raw: row(&edge.source_graph, &edge.record, "edge_id"),
                identity_id: edge.identity_id.clone(),
            })
            .collect(),
        retained_nodes: input
            .retained_nodes
            .iter()
            .map(|node| RetainedCandidateNode {
                normalized: node.clone(),
                owner_material: serde_json::to_vec(&node["source_record"]["payload"]).unwrap(),
            })
            .collect(),
        traces: input
            .claim_traces
            .iter()
            .map(|trace| row("source-claims", trace, "claim_ref"))
            .collect(),
        dossier_refs: input.source_dossier_refs.clone(),
        context_node_order: input.context_node_order.clone(),
        normalization_binding: input.normalization_binding.clone(),
    };
    let fixture = fixture();
    let normalized = normalize_claim_candidate(
        &candidate,
        ClaimCandidateRegistries {
            entity_bytes: fixture.native.entity_registry_bytes(),
            relation_bytes: fixture.native.relation_registry_bytes(),
            descriptor_bytes: &fixture.native.descriptor_bytes,
            vocabulary: &fixture.native.vocabulary,
            expected_normalization_binding: &fixture.graph["normalization_binding"],
        },
        limits,
    )?;
    Ok(json!({
        "nodes": normalized.nodes,
        "relations": normalized.relations,
        "accounting": {
            "input_bytes": normalized.input_bytes,
            "output_bytes": normalized.output_bytes,
        },
    }))
}
fn expected_nodes(graph: &Value, selected: &BTreeSet<String>) -> Vec<Value> {
    let mut rows = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| selected.contains(row["id"].as_str().unwrap()))
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    rows
}
fn expected_relations(input: &FixtureInput) -> BTreeSet<String> {
    input
        .relation_records
        .iter()
        .map(|edge| {
            format!(
                "{}:{}",
                edge.source_graph,
                edge.identity_id
                    .as_deref()
                    .unwrap_or_else(|| edge.record["edge_id"].as_str().unwrap())
            )
        })
        .collect()
}
fn assert_matches_fixture(result: &Value, input: &FixtureInput, selected: &BTreeSet<String>) {
    let mut nodes = result["nodes"].as_array().unwrap().clone();
    nodes.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    assert_eq!(nodes, expected_nodes(&fixture().graph, selected));
    let expected_ids = expected_relations(input);
    let mut expected = fixture().graph["relations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| expected_ids.contains(row["id"].as_str().unwrap()))
        .cloned()
        .collect::<Vec<_>>();
    let mut actual = result["relations"].as_array().unwrap().clone();
    expected.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    actual.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    assert_eq!(actual, expected);
}

#[test]
fn full_source_claim_cohort_matches_shared_native_projection_without_mutation() {
    let selected = all_claim_ids(&fixture().graph);
    let input = input_for(&selected);
    let before = serde_json::to_vec(&input).unwrap();
    let result = normalize(&input, limits()).unwrap();
    assert_matches_fixture(&result, &input, &selected);
    assert_eq!(serde_json::to_vec(&input).unwrap(), before);
    let literal = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["source_record"]["payload"]["node_kind"] == "literal")
        .unwrap();
    let raw_value = literal["source_record"]["payload"]["properties"]["value"].clone();
    assert_eq!(raw_value["value"], "1886-06-03");
    assert_eq!(raw_value["source_wording"]["text"], "03. 06.1886");
    assert!(
        !literal["semantics"]["assertion_contexts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn changed_claim_context_reaches_literal_without_losing_source_fields() {
    use tos_compiler::source_bibliographic::{
        BibliographicDocumentLimits, supplied_claim_navigation_descriptor,
    };

    let selected = all_claim_ids(&fixture().graph);
    let mut input = input_for(&selected);
    let before = normalize(&input, limits()).unwrap();
    let claim_position = input
        .node_records
        .iter()
        .position(|node| node.record["node_kind"] == "claim")
        .unwrap();
    let mut source_claim =
        input.node_records[claim_position].record["properties"]["source_claim"].clone();
    let claim_id = source_claim["claim_id"].as_str().unwrap().to_owned();
    let subject_ref = source_claim["subject_ref"].as_str().unwrap().to_owned();
    let subject = input
        .node_records
        .iter()
        .find(|node| {
            node.record["node_kind"] == "identity"
                && node.record["properties"]["identity_ref"] == subject_ref
        })
        .unwrap()
        .record
        .clone();
    let object = input
        .node_records
        .iter()
        .find(|node| {
            node.record["node_kind"] == "literal"
                && node.record["properties"]["claim_ref"] == claim_id
        })
        .unwrap()
        .record
        .clone();

    source_claim["qualifiers"]["synthetic_context_marker"] = json!("updated");
    let node_record = &mut input.node_records[claim_position].record;
    node_record["properties"]["qualifiers"] = source_claim["qualifiers"].clone();
    node_record["properties"]["source_claim"] = source_claim.clone();
    let source_claim_bytes = tos_foundation::canonical_raw_bytes_v1(
        &serde_json::to_vec(&source_claim).unwrap(),
        tos_foundation::CanonicalProfile::SourceRecordDigestV1,
        tos_foundation::JsonLimits::new(MAX_CANDIDATE_ROW_BYTES, 96, 1_000_000, 4096).unwrap(),
    )
    .unwrap();
    node_record["source_sha256"] = json!(Digest256::of_bytes(&source_claim_bytes).to_hex());

    let fixture = fixture();
    let entity_registry: Value =
        serde_json::from_slice(fixture.native.entity_registry_bytes()).unwrap();
    let relation_registry: Value =
        serde_json::from_slice(fixture.native.relation_registry_bytes()).unwrap();
    node_record["properties"]["navigation_descriptor"] = supplied_claim_navigation_descriptor(
        &source_claim,
        &subject,
        &object,
        &relation_registry,
        &entity_registry,
        BibliographicDocumentLimits {
            input_document_bytes: MAX_CANDIDATE_ROW_BYTES,
            output_row_bytes: MAX_CANDIDATE_ROW_BYTES,
        },
    )
    .unwrap()
    .unwrap();

    let after = normalize(&input, limits()).unwrap();
    fn find_literal(result: &Value) -> &Value {
        result["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["source_record"]["payload"]["node_kind"] == "literal")
            .unwrap()
    }
    let old_literal = find_literal(&before);
    let updated_literal = find_literal(&after);
    assert_ne!(
        old_literal["semantics"]["assertion_contexts"],
        updated_literal["semantics"]["assertion_contexts"]
    );
    assert!(
        updated_literal["semantics"]["assertion_contexts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|context| {
                context["fields"]["qualifiers"]["value"]["synthetic_context_marker"] == "updated"
            })
    );
    assert_eq!(
        updated_literal["source_record"]["payload"]["properties"]["value"]["source_wording"]["text"],
        "03. 06.1886"
    );
}

#[test]
fn incremental_agent_correction_recomputes_claim_and_non_agent_relations() {
    let graph = &fixture().graph;
    let claim = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["source_graph"] == "source-claims" && node["kind_id"] == "claim")
        .unwrap();
    let selected = BTreeSet::from([claim["id"].as_str().unwrap().to_owned()]);
    let mut input = input_for(&selected);
    let agent_id = "tos.agent.synthetic";
    input.node_records.push(FixtureNode {
        source_graph: "source-navigation".into(),
        record: json!({
            "node_id":agent_id,
            "node_kind":"agent",
            "label":"Synthetic prior agent",
            "source_ref":"test:synthetic-agent",
            "identity_status":"provisional",
            "properties":{
                "record_id":agent_id,
                "record_type":"agent",
                "record_version":1,
                "preferred_label":"Synthetic prior agent",
                "identity_status":"provisional",
                "source_refs":["test:synthetic-agent"],
                "external_identifiers":[],
                "visibility":"public_metadata_only"
            }
        }),
    });
    let nav_edge = input
        .relation_records
        .iter_mut()
        .find(|row| row.source_graph == "source-navigation")
        .unwrap();
    nav_edge.record["from_id"] = json!(agent_id);
    let prior = normalize(&input, limits()).unwrap();

    let agent = input
        .node_records
        .iter_mut()
        .find(|row| row.source_graph == "source-navigation")
        .unwrap();
    agent.record["label"] = json!("Synthetic corrected agent");
    agent.record["properties"]["preferred_label"] = json!("Synthetic corrected agent");
    agent.record["properties"]["record_version"] = json!(2);
    let result = normalize(&input, limits()).unwrap();
    let qualified_agent_id = format!("source-navigation:{agent_id}");
    let corrected = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"].as_str() == Some(qualified_agent_id.as_str()))
        .unwrap();
    assert_eq!(
        corrected["display"]["title"]["default"],
        "Synthetic corrected agent"
    );
    assert_eq!(
        corrected["source_record"]["payload"]["properties"]["record_version"],
        2
    );
    assert_ne!(prior["nodes"], result["nodes"]);
    assert!(result["relations"].as_array().unwrap().iter().any(|row| {
        row["source_graph"] == "source-navigation"
            && row["display"]["statement"]["default"]
                .as_str()
                .is_some_and(|text| text.contains("Synthetic corrected agent"))
    }));
    assert!(result["relations"].as_array().unwrap().iter().any(|row| {
        row["from_id"].as_str() != Some(qualified_agent_id.as_str())
            && row["to_id"].as_str() != Some(qualified_agent_id.as_str())
    }));
    assert!(
        result["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind_id"] == "claim")
    );
}

#[test]
fn required_trace_endpoints_duplicates_and_binding_fail_closed() {
    let selected = all_claim_ids(&fixture().graph);
    let input = input_for(&selected);
    let mut missing_trace = input.clone();
    missing_trace.claim_traces.clear();
    assert!(normalize(&missing_trace, limits()).is_err());
    let mut missing_endpoint = input.clone();
    missing_endpoint.retained_nodes.clear();
    assert!(normalize(&missing_endpoint, limits()).is_err());
    let mut duplicate_node = input.clone();
    duplicate_node
        .node_records
        .push(duplicate_node.node_records[0].clone());
    assert!(normalize(&duplicate_node, limits()).is_err());
    let mut duplicate_relation = input.clone();
    duplicate_relation
        .relation_records
        .push(duplicate_relation.relation_records[0].clone());
    assert!(normalize(&duplicate_relation, limits()).is_err());
    let mut duplicate_trace = input.clone();
    duplicate_trace
        .claim_traces
        .push(duplicate_trace.claim_traces[0].clone());
    assert!(normalize(&duplicate_trace, limits()).is_err());
    let mut wrong_binding = input;
    wrong_binding.normalization_binding["processor_digest"] = json!("f".repeat(64));
    assert!(normalize(&wrong_binding, limits()).is_err());
}

#[test]
fn source_descriptor_and_retained_digest_corruption_fail_closed() {
    let selected = all_claim_ids(&fixture().graph);
    let input = input_for(&selected);
    let mut bad_descriptor = input.clone();
    let claim = bad_descriptor
        .node_records
        .iter_mut()
        .find(|node| node.record["node_kind"] == "claim")
        .unwrap();
    claim.record["properties"]["navigation_descriptor"]["invented"] = json!(false);
    assert!(normalize(&bad_descriptor, limits()).is_err());
    let mut bad_retained = input;
    bad_retained.retained_nodes[0]["source_record"]["payload"]["corrupt"] = json!(true);
    assert!(normalize(&bad_retained, limits()).is_err());
}

#[test]
fn explicit_budgets_and_stateless_retry_hold_after_refusal() {
    let selected = all_claim_ids(&fixture().graph);
    let input = input_for(&selected);
    let expected = normalize(&input, limits()).unwrap();
    let input_bytes = expected["accounting"]["input_bytes"].as_u64().unwrap() as usize;
    let output_bytes = expected["accounting"]["output_bytes"].as_u64().unwrap() as usize;
    for restricted in [
        ClaimCandidateLimits {
            max_nodes: 0,
            ..limits()
        },
        ClaimCandidateLimits {
            max_relations: 0,
            ..limits()
        },
        ClaimCandidateLimits {
            max_traces: 0,
            ..limits()
        },
        ClaimCandidateLimits {
            max_input_bytes: input_bytes - 1,
            ..limits()
        },
        ClaimCandidateLimits {
            max_output_bytes: output_bytes - 1,
            ..limits()
        },
    ] {
        assert!(normalize(&input, restricted).is_err());
        assert_eq!(normalize(&input, limits()).unwrap(), expected);
    }
    let exact = ClaimCandidateLimits {
        max_input_bytes: input_bytes,
        max_output_bytes: output_bytes,
        ..limits()
    };
    assert_eq!(normalize(&input, exact).unwrap(), expected);
}

#[test]
fn current_record_identity_and_inherited_version_views_are_preserved() {
    use std::time::{Duration, Instant};
    use tos_compiler::source_bibliographic::{
        BibliographicLimits, SuppliedNavigationRecordInput, render_supplied_navigation_record,
    };
    use tos_compiler::source_witness_catalog::SourceCatalogLimits;

    fn exact_digest(record: &Value) -> String {
        let mut canonical = record.clone();
        canonical.sort_all_objects();
        format!(
            "sha256:{}",
            Digest256::of_bytes(&serde_json::to_vec(&canonical).unwrap()).to_hex()
        )
    }

    let identity = "tos.agent.synthetic-history";
    let old_record = json!({
        "record_id":identity,
        "record_type":"agent",
        "record_version":1,
        "preferred_label":"Synthetic historical Agent",
        "identity_status":"provisional",
        "field_languages":{"preferred_label":{"language":"en"}},
        "external_identifiers":[],
        "source_refs":["test:synthetic-history"],
        "visibility":"public_metadata_only"
    });
    let current_record = json!({
        "record_id":identity,
        "record_type":"agent",
        "record_version":2,
        "preferred_label":"Synthetic current Agent",
        "identity_status":"provisional",
        "field_languages":{"preferred_label":{"language":"en"}},
        "external_identifiers":[],
        "source_refs":["test:synthetic-history"],
        "visibility":"public_metadata_only"
    });
    let old_ref = json!({
        "id":identity,
        "version":1,
        "digest":exact_digest(&old_record)
    });
    let current_ref = json!({
        "id":identity,
        "version":2,
        "digest":exact_digest(&current_record)
    });
    let provenance = json!({
        "source":{"archive_blob_ref":null,"source_ref":"test:synthetic-history"}
    });
    let history = json!({
        "status":"available",
        "reason":"synthetic-current-and-retained",
        "record_id":identity,
        "current_ref":current_ref.clone(),
        "refs":[old_ref.clone(),current_ref.clone()],
        "provenance":provenance.clone(),
        "grants_current_use":false,
        "performs_assessment":false,
        "writes_to_source":false
    });
    let versions = vec![
        (
            old_ref.clone(),
            json!({
                "status":"available",
                "reason":"synthetic-exact-version",
                "version_status":"historical",
                "record":old_record.clone(),
                "provenance":provenance.clone()
            }),
        ),
        (
            current_ref.clone(),
            json!({
                "status":"available",
                "reason":"synthetic-exact-version",
                "version_status":"current",
                "record":current_record.clone(),
                "provenance":provenance.clone()
            }),
        ),
    ];
    let entry = json!({
        "record_id":identity,
        "record_type":"agent",
        "record_sha256":exact_digest(&current_record),
        "source_record_ref":"test:synthetic-history",
        "preferred_label":"Synthetic current Agent",
        "identity_status":"provisional"
    });
    let projection = render_supplied_navigation_record(
        SuppliedNavigationRecordInput {
            entry: &entry,
            source_record: &current_record,
            forms: None,
            history: Some(&history),
            versions: &versions,
            native_composite: false,
        },
        BibliographicLimits {
            catalog: SourceCatalogLimits {
                max_files: 128,
                max_rows: 4096,
                max_file_bytes: 2 * 1024 * 1024,
                max_row_bytes: 1024 * 1024,
                max_contract_bytes: 2 * 1024 * 1024,
                max_output_row_bytes: 256 * 1024,
            },
            max_claim_cohort_rows: 256,
            max_claim_cohort_bytes: 64 * 1024 * 1024,
            max_output_rows: 1000,
            max_output_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(5),
        },
    )
    .unwrap();

    let graph = &fixture().graph;
    let claim = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["source_graph"] == "source-claims" && node["kind_id"] == "claim")
        .unwrap();
    let selected = BTreeSet::from([claim["id"].as_str().unwrap().to_owned()]);
    let mut input = input_for(&selected);
    for node in &projection.nodes {
        input.node_records.push(FixtureNode {
            source_graph: "source-navigation".into(),
            record: node.clone(),
        });
    }
    for edge in &projection.edges {
        let mut edge = edge.clone();
        edge["view_ids"] = json!(["fixture-inherited-view"]);
        input.relation_records.push(FixtureRelation {
            source_graph: "source-navigation".into(),
            record: edge,
            identity_id: None,
        });
    }

    let result = normalize(&input, limits()).unwrap();
    let current = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| {
            node["entity_id"].as_str() == Some(identity) && node["kind_id"] != "record-version"
        })
        .unwrap();
    assert_eq!(
        current["source_record"]["payload"]["properties"]["record_version"],
        2
    );
    assert_eq!(
        current["display"]["title"]["default"],
        "Synthetic current Agent"
    );
    assert!(
        current["view_ids"]
            .as_array()
            .unwrap()
            .contains(&json!("fixture-inherited-view"))
    );
    let version_nodes = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["kind_id"] == "record-version")
        .collect::<Vec<_>>();
    assert_eq!(version_nodes.len(), 2);
    assert!(
        version_nodes
            .iter()
            .all(|node| node["entity_id"].as_str() != Some(identity))
    );
    assert!(version_nodes.iter().all(|node| {
        node["semantics"]["record_version"]["grants_current_use"] == false
            && node["view_ids"]
                .as_array()
                .unwrap()
                .contains(&json!("fixture-inherited-view"))
    }));
    assert_eq!(
        version_nodes
            .iter()
            .map(|node| node["semantics"]["record_version"]["version_status"]
                .as_str()
                .unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["current", "historical"])
    );
}
