//! Native, bounded normalization of an explicitly supplied source cohort.
//!
//! This is a mechanics-only candidate API. It does not discover missing input,
//! prove incidence or reducer completeness, admit source, or publish anything.
use super::source_claim_publication_normalize::{
    CandidateNode, CandidateRelation, ClaimCandidateInput, ClaimCandidateLimits,
    ClaimCandidateRegistries, RetainedCandidateNode, declared_dossier, normalize_claim_candidate,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::io::Write;
use tos_compiler::knowledge_stage::SeekRow;
use tos_compiler::{Error, QueryVocabulary, Result};
use tos_foundation::Digest256;

const MAX_CANDIDATE_ROW_BYTES: usize = 8 * 1024 * 1024;
const MAX_CLAIM_CONTEXTS: usize = 4096;

/// Explicit budgets for one supplied assembly candidate. The per-row and
/// readable-context ceilings are inherited from the shared native kernels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssemblyNormalizationLimits {
    pub max_nodes: usize,
    pub max_retained_nodes: usize,
    pub max_relations: usize,
    pub max_traces: usize,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
}
impl Default for AssemblyNormalizationLimits {
    fn default() -> Self {
        Self {
            max_nodes: 1024,
            max_retained_nodes: 2048,
            max_relations: 2048,
            max_traces: 512,
            max_input_bytes: 32 * 1024 * 1024,
            max_output_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AssemblyNodeRecord {
    pub source_graph: String,
    pub record: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct AssemblyRelationRecord {
    pub source_graph: String,
    pub record: Value,
    pub identity_id: Option<String>,
}
/// Caller-selected raw source frames and exact normalized endpoints from the
/// current owner snapshot. Registry/vocabulary selection is a separate input.
#[derive(Clone, Debug, Serialize)]
pub struct SourceAssemblyCandidateInput {
    pub node_records: Vec<AssemblyNodeRecord>,
    pub relation_records: Vec<AssemblyRelationRecord>,
    pub retained_nodes: Vec<Value>,
    pub claim_traces: Vec<Value>,
    pub source_dossier_refs: Vec<String>,
    pub context_node_order: Vec<String>,
    pub normalization_binding: Value,
}
/// Exact selected owner dependencies used by the native normalizers.
pub struct SourceAssemblyNormalizationSelection<'a> {
    pub entity_registry_bytes: &'a [u8],
    pub relation_registry_bytes: &'a [u8],
    pub descriptor_bytes: &'a [u8],
    pub vocabulary: &'a QueryVocabulary,
    /// Read independently from the selected owner, not from the candidate.
    pub expected_normalization_binding: &'a Value,
}

struct CountWriter {
    bytes: usize,
    cap: usize,
}
impl Write for CountWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.cap)
            .ok_or_else(|| std::io::Error::other("source assembly byte budget"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encoded_len(value: &impl Serialize, cap: usize, label: &'static str) -> Result<usize> {
    let mut writer = CountWriter { bytes: 0, cap };
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::Budget(label))?;
    Ok(writer.bytes)
}
fn encode_row(value: &Value, cap: usize, label: &'static str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    struct Capped<'a> {
        bytes: &'a mut Vec<u8>,
        cap: usize,
    }
    impl Write for Capped<'_> {
        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
            if self
                .bytes
                .len()
                .checked_add(input.len())
                .is_none_or(|n| n > self.cap)
            {
                return Err(std::io::Error::other("source assembly row budget"));
            }
            self.bytes.extend_from_slice(input);
            Ok(input.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(
        &mut Capped {
            bytes: &mut bytes,
            cap,
        },
        value,
    )
    .map_err(|_| Error::Budget(label))?;
    Ok(bytes)
}
fn row_id(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .map(str::to_owned)
        .ok_or(Error::Invalid("Source assembly exact row identifier"))
}
fn seek_row(graph: &str, id: String, value: &Value, cap: usize) -> Result<SeekRow> {
    let payload = encode_row(value, cap, "Source assembly raw row bytes")?;
    let payload_sha256 = Digest256::of_bytes(&payload).to_hex();
    Ok(SeekRow {
        id,
        source_graph: graph.to_owned(),
        source_order: None,
        payload,
        payload_sha256,
    })
}
/// Normalize only the explicitly supplied cohort through the same Rust
/// kernels used by full knowledge construction. It has no ambient cache to
/// install, clear, or restore.
pub fn normalize_source_assembly_candidate(
    input: &SourceAssemblyCandidateInput,
    selected: SourceAssemblyNormalizationSelection<'_>,
    limits: AssemblyNormalizationLimits,
) -> Result<Value> {
    if input.node_records.len() > limits.max_nodes
        || input.retained_nodes.len() > limits.max_retained_nodes
        || input.relation_records.len() > limits.max_relations
        || input.claim_traces.len() > limits.max_traces
    {
        return Err(Error::Budget("Source assembly row count"));
    }
    if limits.max_input_bytes == 0 || limits.max_output_bytes == 0 {
        return Err(Error::Budget("Source assembly byte limits"));
    }
    let dependency_bytes = selected
        .entity_registry_bytes
        .len()
        .checked_add(selected.relation_registry_bytes.len())
        .and_then(|bytes| bytes.checked_add(selected.descriptor_bytes.len()))
        .filter(|bytes| *bytes <= limits.max_input_bytes)
        .ok_or(Error::Budget("Source assembly dependency bytes"))?;
    let _registry = tos_compiler::KnowledgeRegistry::parse(
        selected.entity_registry_bytes,
        selected.relation_registry_bytes,
    )?;
    selected
        .vocabulary
        .verify_authored_bytes(selected.descriptor_bytes)?;
    let entity_registry: Value = serde_json::from_slice(selected.entity_registry_bytes)
        .map_err(|_| Error::Invalid("Source assembly entity registry JSON"))?;
    let relation_registry: Value = serde_json::from_slice(selected.relation_registry_bytes)
        .map_err(|_| Error::Invalid("Source assembly relation registry JSON"))?;
    let input_material = (
        &input.node_records,
        &input.relation_records,
        &input.retained_nodes,
        &input.claim_traces,
        &input.source_dossier_refs,
        &input.context_node_order,
        &input.normalization_binding,
        &entity_registry,
        &relation_registry,
    );
    let input_bytes = encoded_len(
        &input_material,
        limits.max_input_bytes,
        "Source assembly input frame bytes",
    )?;
    let native_input_bytes = limits
        .max_input_bytes
        .checked_add(dependency_bytes)
        .ok_or(Error::Budget("Source assembly native input bytes"))?;
    let row_cap = MAX_CANDIDATE_ROW_BYTES.min(limits.max_input_bytes);
    if row_cap == 0 {
        return Err(Error::Budget("Source assembly row bytes"));
    }

    let mut nodes = Vec::with_capacity(input.node_records.len());
    for spec in &input.node_records {
        let id = row_id(&spec.record, "node_id")?;
        let row = seek_row(&spec.source_graph, id, &spec.record, row_cap)?;
        let dossier_ref = declared_dossier(&spec.record, &spec.source_graph);
        nodes.push(CandidateNode {
            raw: row,
            dossier_ref,
        });
    }
    let mut relations = Vec::with_capacity(input.relation_records.len());
    for spec in &input.relation_records {
        let id = row_id(&spec.record, "edge_id")?;
        relations.push(CandidateRelation {
            raw: seek_row(&spec.source_graph, id, &spec.record, row_cap)?,
            identity_id: spec.identity_id.clone(),
        });
    }
    let mut retained_nodes = Vec::with_capacity(input.retained_nodes.len());
    for node in &input.retained_nodes {
        let owner = node
            .pointer("/source_record/payload")
            .filter(|payload| payload.is_object())
            .ok_or(Error::Invalid("Source assembly retained source payload"))?;
        retained_nodes.push(RetainedCandidateNode {
            normalized: node.clone(),
            owner_material: encode_row(owner, row_cap, "Source assembly retained owner bytes")?,
        });
    }
    let mut traces = Vec::with_capacity(input.claim_traces.len());
    for trace in &input.claim_traces {
        traces.push(seek_row(
            "source-claims",
            row_id(trace, "claim_ref")?,
            trace,
            row_cap,
        )?);
    }
    let candidate = ClaimCandidateInput {
        nodes,
        relations,
        retained_nodes,
        traces,
        dossier_refs: input.source_dossier_refs.clone(),
        context_node_order: input.context_node_order.clone(),
        normalization_binding: input.normalization_binding.clone(),
    };
    let normalized = normalize_claim_candidate(
        &candidate,
        ClaimCandidateRegistries {
            entity_bytes: selected.entity_registry_bytes,
            relation_bytes: selected.relation_registry_bytes,
            descriptor_bytes: selected.descriptor_bytes,
            vocabulary: selected.vocabulary,
            expected_normalization_binding: selected.expected_normalization_binding,
        },
        ClaimCandidateLimits {
            max_nodes: limits.max_nodes,
            max_retained_nodes: limits.max_retained_nodes,
            max_relations: limits.max_relations,
            max_traces: limits.max_traces,
            max_contexts: MAX_CLAIM_CONTEXTS,
            max_row_bytes: row_cap,
            max_input_bytes: native_input_bytes,
            max_output_bytes: limits.max_output_bytes,
        },
    )?;
    let output_bytes = encoded_len(
        &(&normalized.nodes, &normalized.relations),
        limits.max_output_bytes,
        "Source assembly output bytes",
    )?;
    let node_count = normalized.nodes.len();
    let relation_count = normalized.relations.len();
    let trace_count = candidate.traces.len();
    let retained_count = candidate.retained_nodes.len();
    Ok(json!({
        "schema":"tos_source_assembly_normalization_candidate_v1",
        "nodes":normalized.nodes,
        "relations":normalized.relations,
        "accounting":{
            "input_bytes":input_bytes,
            "output_bytes":output_bytes,
            "nodes":node_count,
            "relations":relation_count,
            "traces":trace_count,
            "retained_nodes":retained_count
        },
        "scope":{
            "complete_incidence_verified":false,
            "source_transition_verified":false,
            "reducer_closure_verified":false,
            "global_semantics_validated":false,
            "catalog_updated":false,
            "published":false
        },
        "is_semantic_acceptance":false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::OnceLock;
    use tos_compiler::knowledge_full_fixture::{FullKnowledgeFixture, build_native_fixture};

    struct Fixture {
        native: FullKnowledgeFixture,
        graph: Value,
        traces: Vec<Value>,
    }
    fn fixture() -> &'static Fixture {
        static FIXTURE: OnceLock<Fixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let native = build_native_fixture();
            let graph: Value = serde_json::from_slice(&native.graph_input_bytes).unwrap();
            let source: Value = serde_json::from_slice(include_bytes!(
                "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
            ))
            .unwrap();
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
    fn input_for(selected_ids: &BTreeSet<String>) -> SourceAssemblyCandidateInput {
        let fixture = fixture();
        let graph_nodes = fixture.graph["nodes"].as_array().unwrap();
        let selected = graph_nodes
            .iter()
            .filter(|node| selected_ids.contains(node["id"].as_str().unwrap()))
            .collect::<Vec<_>>();
        let node_records = selected
            .iter()
            .map(|node| AssemblyNodeRecord {
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
                AssemblyRelationRecord {
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
        SourceAssemblyCandidateInput {
            node_records,
            relation_records,
            retained_nodes,
            claim_traces: fixture.traces.clone(),
            source_dossier_refs,
            context_node_order,
            normalization_binding: fixture.graph["normalization_binding"].clone(),
        }
    }
    fn normalize(
        input: &SourceAssemblyCandidateInput,
        limits: AssemblyNormalizationLimits,
    ) -> Result<Value> {
        let fixture = fixture();
        normalize_source_assembly_candidate(
            input,
            SourceAssemblyNormalizationSelection {
                entity_registry_bytes: fixture.native.entity_registry_bytes(),
                relation_registry_bytes: fixture.native.relation_registry_bytes(),
                descriptor_bytes: &fixture.native.descriptor_bytes,
                vocabulary: &fixture.native.vocabulary,
                expected_normalization_binding: &fixture.graph["normalization_binding"],
            },
            limits,
        )
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
    fn expected_relations(input: &SourceAssemblyCandidateInput) -> BTreeSet<String> {
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
    fn assert_matches_fixture(
        result: &Value,
        input: &SourceAssemblyCandidateInput,
        selected: &BTreeSet<String>,
    ) {
        assert_eq!(
            result["schema"],
            "tos_source_assembly_normalization_candidate_v1"
        );
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
        assert_eq!(result["scope"]["complete_incidence_verified"], false);
        assert_eq!(result["scope"]["published"], false);
        assert_eq!(result["is_semantic_acceptance"], false);
    }

    #[test]
    fn full_source_claim_cohort_matches_shared_native_projection_without_mutation() {
        let selected = all_claim_ids(&fixture().graph);
        let input = input_for(&selected);
        let before = serde_json::to_vec(&input).unwrap();
        let result = normalize(&input, AssemblyNormalizationLimits::default()).unwrap();
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
        let before = normalize(&input, AssemblyNormalizationLimits::default()).unwrap();
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

        let after = normalize(&input, AssemblyNormalizationLimits::default()).unwrap();
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
                    context["fields"]["qualifiers"]["value"]["synthetic_context_marker"]
                        == "updated"
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
        input.node_records.push(AssemblyNodeRecord {
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
        let prior = normalize(&input, AssemblyNormalizationLimits::default()).unwrap();

        let agent = input
            .node_records
            .iter_mut()
            .find(|row| row.source_graph == "source-navigation")
            .unwrap();
        agent.record["label"] = json!("Synthetic corrected agent");
        agent.record["properties"]["preferred_label"] = json!("Synthetic corrected agent");
        agent.record["properties"]["record_version"] = json!(2);
        let result = normalize(&input, AssemblyNormalizationLimits::default()).unwrap();
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
        assert!(normalize(&missing_trace, AssemblyNormalizationLimits::default()).is_err());
        let mut missing_endpoint = input.clone();
        missing_endpoint.retained_nodes.clear();
        assert!(normalize(&missing_endpoint, AssemblyNormalizationLimits::default()).is_err());
        let mut duplicate_node = input.clone();
        duplicate_node
            .node_records
            .push(duplicate_node.node_records[0].clone());
        assert!(normalize(&duplicate_node, AssemblyNormalizationLimits::default()).is_err());
        let mut duplicate_relation = input.clone();
        duplicate_relation
            .relation_records
            .push(duplicate_relation.relation_records[0].clone());
        assert!(normalize(&duplicate_relation, AssemblyNormalizationLimits::default()).is_err());
        let mut duplicate_trace = input.clone();
        duplicate_trace
            .claim_traces
            .push(duplicate_trace.claim_traces[0].clone());
        assert!(normalize(&duplicate_trace, AssemblyNormalizationLimits::default()).is_err());
        let mut wrong_binding = input;
        wrong_binding.normalization_binding["processor_digest"] = json!("f".repeat(64));
        assert!(normalize(&wrong_binding, AssemblyNormalizationLimits::default()).is_err());
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
        assert!(normalize(&bad_descriptor, AssemblyNormalizationLimits::default()).is_err());
        let mut bad_retained = input;
        bad_retained.retained_nodes[0]["source_record"]["payload"]["corrupt"] = json!(true);
        assert!(normalize(&bad_retained, AssemblyNormalizationLimits::default()).is_err());
    }

    #[test]
    fn explicit_budgets_and_stateless_retry_hold_after_refusal() {
        let selected = all_claim_ids(&fixture().graph);
        let input = input_for(&selected);
        let expected = normalize(&input, AssemblyNormalizationLimits::default()).unwrap();
        let input_bytes = expected["accounting"]["input_bytes"].as_u64().unwrap() as usize;
        let output_bytes = expected["accounting"]["output_bytes"].as_u64().unwrap() as usize;
        for limits in [
            AssemblyNormalizationLimits {
                max_nodes: 0,
                ..Default::default()
            },
            AssemblyNormalizationLimits {
                max_relations: 0,
                ..Default::default()
            },
            AssemblyNormalizationLimits {
                max_traces: 0,
                ..Default::default()
            },
            AssemblyNormalizationLimits {
                max_input_bytes: input_bytes - 1,
                ..Default::default()
            },
            AssemblyNormalizationLimits {
                max_output_bytes: output_bytes - 1,
                ..Default::default()
            },
        ] {
            assert!(normalize(&input, limits).is_err());
            assert_eq!(
                normalize(&input, AssemblyNormalizationLimits::default()).unwrap(),
                expected
            );
        }
        let exact = AssemblyNormalizationLimits {
            max_input_bytes: input_bytes,
            max_output_bytes: output_bytes,
            ..Default::default()
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
            input.node_records.push(AssemblyNodeRecord {
                source_graph: "source-navigation".into(),
                record: node.clone(),
            });
        }
        for edge in &projection.edges {
            let mut edge = edge.clone();
            edge["view_ids"] = json!(["fixture-inherited-view"]);
            input.relation_records.push(AssemblyRelationRecord {
                source_graph: "source-navigation".into(),
                record: edge,
                identity_id: None,
            });
        }

        let result = normalize(&input, AssemblyNormalizationLimits::default()).unwrap();
        let current = result["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| {
                node["entity_id"].as_str() == Some(identity)
                    && node["node_kind"] != "record-version"
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
            .filter(|node| node["node_kind"] == "record-version")
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
}
