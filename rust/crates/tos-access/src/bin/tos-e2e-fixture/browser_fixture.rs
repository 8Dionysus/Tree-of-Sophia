//! Synthetic browser packets served through the production native HTTP and
//! installed-site handlers. These packets are test-only read fixtures.
use serde_json::{Value, json};
use std::sync::Arc;
use tos_access::{
    AccessError, AccessErrorCode, AccessExecutor, DisclosureFence, KnowledgeRequest, Params,
    PreparedPacket,
};
use tos_query::{
    AbortProbe, AbortReason,
    compressed_search::CompressedSearchRequest,
    source_gap::{
        PublicSourceGapRecord, SourceGapBudget, SourceGapRequest, compute_source_gap_packet,
    },
};

const REV: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CONTENT: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const GRAPH: &[u8] = include_bytes!("browser_graph.json");
const WORK_RECORD: &str = include_str!(
    "../../../../../../ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json"
);
const LENS_SCHEMA: &str =
    include_str!("../../../../../../access/contracts/lens-spec.v1.schema.json");

pub struct BrowserFixtureExecutor {
    scenario: String,
}
impl BrowserFixtureExecutor {
    pub fn new(scenario: &str) -> Self {
        Self {
            scenario: scenario.to_owned(),
        }
    }
    fn source_enabled(&self) -> bool {
        matches!(self.scenario.as_str(), "source" | "source-native-metadata")
    }
    fn source_record(&self) -> Value {
        if self.scenario == "source-native-metadata" {
            json!({"record_type":"text-unit","record_id":"tos.text-unit.browser-fixture","record_version":1,
                "preferred_label":"Synthetic native metadata","notes":"Available exact metadata remains readable.",
                "native_text_binding":{"unit_id":"tos.text-unit.browser-fixture","unit_version":1,
                    "segmentation_id":"tos.text-segmentation.browser-fixture","segmentation_version":1,
                    "packet_id":"tos.source-text-unit-packet.browser-fixture","packet_version":1,
                    "packet_sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                    "text_layer":{"layer_id":"tos.text-layer.browser-fixture","layer_version":1,
                        "record_sha256":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"},
                    "ordered_anchor_refs":["tos.anchor.browser-fixture"]}})
        } else {
            serde_json::from_str(WORK_RECORD).expect("checked source record fixture is JSON")
        }
    }
    fn graph(&self) -> Value {
        let mut graph: Value =
            serde_json::from_slice(GRAPH).expect("browser graph fixture is JSON");
        if self.source_enabled() {
            if let Some(node) = graph["nodes"]
                .as_array_mut()
                .and_then(|nodes| nodes.iter_mut().find(|n| n["node_id"] == "a"))
            {
                node["source_ref"] = if self.scenario == "source-native-metadata" {
                    "ToS/synthetic/native-metadata.json".into()
                } else {
                    "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json".into()
                };
                node["properties"]["source_record"] = self.source_record();
            }
        }
        graph
    }
    fn source_target(&self) -> Value {
        let record = self.source_record();
        let id = record["record_id"].as_str().unwrap_or("tos.work.fixture");
        let version = record["record_version"].as_u64().unwrap_or(1);
        let kind = record["record_type"].as_str().unwrap_or("work");
        let canonical = tos_foundation::canonical_bytes_v1(
            &tos_foundation::parse_json(
                &serde_json::to_vec(&record).unwrap_or_default(),
                tos_foundation::JsonMode::RequestLastWins,
                tos_foundation::JsonLimits::default(),
            )
            .expect("source record parses")
            .into_root(),
            tos_foundation::CanonicalProfile::SourceRecordDigestV1,
            tos_foundation::JsonLimits::default(),
        )
        .expect("source record canonicalizes");
        let digest = format!(
            "sha256:{}",
            tos_foundation::Digest256::of_bytes(&canonical).to_hex()
        );
        json!({"layer":"metadata_record","record_type":kind,
            "record_ref":{"id":id,"version":version,"digest":digest},"content_revision":digest})
    }
    fn node(&self, requested: &str) -> Value {
        let id = requested.strip_prefix("philosophy:").unwrap_or(requested);
        self.graph()["nodes"].as_array().and_then(|nodes| nodes.iter().find(|n| n["node_id"] == id)).cloned()
            .unwrap_or_else(|| json!({"node_id":id,"label":id,"node_type":"candidate-node","source_ref":"ToS/canon/a.json","properties":{}}))
    }
    fn material_node(&self, requested: &str) -> Value {
        let raw = self.node(requested);
        let node_id = raw["node_id"].as_str().unwrap_or("a");
        let id = format!("philosophy:{node_id}");
        let label = raw["label"].clone();
        let mut packet = json!({"id":id,"node_id":node_id,"entity_id":format!("tos.node.fixture.{node_id}"),
            "kind_id":raw["node_type"],"label":label,"content_revision":CONTENT,
            "source_refs":[raw["source_ref"]],"display":{"title":{"ru":label,"en":label},
                "kind_label":{"ru":"Материал","en":"Material"},"summary":{"ru":"Synthetic browser fixture","en":"Synthetic browser fixture"}},
            "attributes":{},"properties":raw["properties"]});
        if self.source_enabled() && node_id == "a" {
            packet["source_record"] = self.source_record();
        }
        packet
    }
    fn catalog(&self) -> Value {
        json!({"schema":"tos_knowledge_catalog_v1","source_revision":REV,
            "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false},
            "node_kinds":[{"kind_id":"candidate-node","display":{"ru":"Понятие"}},
                {"kind_id":"work","display":{"ru":"Произведение"}},{"kind_id":"source","display":{"ru":"Источник"}}],
            "predicates":[{"predicate_id":"relates","display":{"ru":"Связано"}}],
            "lenses":[],"counts":{"nodes":3,"relations":3,"display_coverage":{"node_titles":3}},
            "semantic_registries":{"entity_types":{"entries":[
                {"entity_type_id":"fixture.concept","source_mappings":[{"source_graph":"philosophy","source_kind_id":"candidate-node"}]},
                {"entity_type_id":"fixture.work","source_mappings":[{"source_graph":"philosophy","source_kind_id":"work"}],"object_role":"meaning"},
                {"entity_type_id":"fixture.source","source_mappings":[{"source_graph":"philosophy","source_kind_id":"source"}],"object_role":"meaning"}]},
                "relation_types":{"entries":[]}},
            "capabilities":{"sources":["philosophy","source-navigation"],"filter_operators":["in"],
                "node_fields":["kind_id"],"relation_fields":["predicate_id"],
                "maximums":{"nodes":1000,"relations":2000,"groups":200,"traversal_depth":5},
                "neighborhood_profiles":[{"profile":"overview","definition":"bounded neighborhood"},{"profile":"all","definition":"selected area"}],
                "inclusion":{"authority":"query-execution-not-semantic-proof"}}})
    }
    fn contracts(&self) -> Value {
        let schema: Value = serde_json::from_str(LENS_SCHEMA).expect("lens schema fixture is JSON");
        json!({"schema":"tos_knowledge_contract_bundle_v1","source_revision":REV,
            "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false},
            "contracts":{"lens_spec":schema}})
    }
    fn search_capabilities(&self) -> Value {
        json!({"schema":"tos_knowledge_search_capabilities_v1","source_revision":REV,"writes_to_tree":false,
            "modes":{"compressed":{"available":true,"schema":"tos_knowledge_search_compressed_v3","source_revision":REV},
                "indexed":{"available":false,"schema":"tos_knowledge_search_indexed_v2","source_revision":REV}}})
    }
    fn exploration_capabilities(&self) -> Value {
        json!({"schema":"tos_exploration_capabilities_v1","available":true,"writes_to_tree":false,"source_revision":REV,
            "request_versions":["tos_exploration_request_v2"],"result_versions":["tos_exploration_result_v2"],
            "v2_origin_kinds":["node","relation"],"limits":{"depth":5,"page_nodes":40,"page_relations":80}})
    }
    fn node_inspect(&self, requested: &str) -> Value {
        let id = if requested.starts_with("philosophy:") {
            requested.to_owned()
        } else {
            format!("philosophy:{requested}")
        };
        let node = self.material_node(&id);
        let mut targets = serde_json::Map::new();
        if self.source_enabled() && id == "philosophy:a" {
            targets.insert(
                id.clone(),
                json!({"source_revision":REV,"target":self.source_target()}),
            );
        }
        json!({"schema":"tos_knowledge_inspect_v1","schema_version":"tos_knowledge_inspect_v1","source_revision":REV,
            "match":node,"matches":[node],"relations":[],"source_read_targets":Value::Object(targets)})
    }
    fn graph_view(&self, view_id: &str, limit: usize) -> Value {
        let graph = self.graph();
        let selected_view = graph["views"]
            .as_array()
            .and_then(|views| views.iter().find(|v| v["view_id"] == view_id))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let node_ids = selected_view["node_ids"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let edge_ids = selected_view["edge_ids"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let nodes = node_ids
            .iter()
            .take(limit.max(1))
            .filter_map(|id| {
                graph["nodes"]
                    .as_array()?
                    .iter()
                    .find(|n| n["node_id"] == *id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let node_ids = nodes
            .iter()
            .filter_map(|node| node["node_id"].as_str())
            .collect::<Vec<_>>();
        let edges = edge_ids
            .iter()
            .take(limit.max(1))
            .filter_map(|id| {
                graph["edges"]
                    .as_array()?
                    .iter()
                    .find(|e| e["edge_id"] == *id)
            })
            .filter(|edge| {
                node_ids.contains(&edge["from_id"].as_str().unwrap_or(""))
                    && node_ids.contains(&edge["to_id"].as_str().unwrap_or(""))
            })
            .cloned()
            .collect::<Vec<_>>();
        let node_count = nodes.len();
        let edge_count = edges.len();
        json!({"schema":"tos_philosophy_graph_view_v1","source_revision":REV,"view_id":view_id,
            "title":if view_id=="chronology" {"Chronology"} else {"Direct only"},"nodes":nodes,"edges":edges.clone(),"relations":edges.clone(),
            "view":{"view_id":view_id,"title":if view_id=="chronology" {"Chronology"} else {"Direct only"},"node_ids":node_ids,"edge_ids":edge_ids},
            "node_count":node_count,"edge_count":edge_count,"counts":{"nodes":node_count,"edges":edge_count,"relations":edge_count},
            "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false}})
    }
    fn lens(&self, focus: Option<&str>) -> Value {
        let graph = self.graph();
        let nodes = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| self.material_node(n["node_id"].as_str().unwrap()))
            .collect::<Vec<_>>();
        let relations = graph["edges"].as_array().unwrap().iter().map(|e| json!({
            "id":format!("philosophy:{}",e["edge_id"].as_str().unwrap()),"from_id":format!("philosophy:{}",e["from_id"].as_str().unwrap()),
            "to_id":format!("philosophy:{}",e["to_id"].as_str().unwrap()),"predicate_id":"relates","relation_type_id":"relates",
            "content_revision":CONTENT,"source_refs":[e["source_ref"]],"display":{"label":{"ru":"Связано","en":"Related"}},"attributes":{}})).collect::<Vec<_>>();
        let node_count = nodes.len();
        let relation_count = relations.len();
        let focus_id = focus
            .filter(|id| nodes.iter().any(|n| n["id"] == *id))
            .unwrap_or("philosophy:a");
        let scene = fixture_scene(&nodes, &relations, Some(focus_id), None);
        json!({"schema":"tos_lens_result_v1","source_revision":REV,"fingerprint":CONTENT,
            "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false},
            "nodes":nodes.clone(),"relations":relations.clone(),"focus":{"node_id":focus_id},
            "counts":{"nodes":node_count,"relations":relation_count,"matched_nodes":node_count,"eligible_relations":relation_count,"truncated_nodes":0,"truncated_relations":0},
            "inclusion":{"authority":"query-execution-not-semantic-proof","nodes":{},"relations":{}},"scene":scene})
    }
    fn exploration(&self, request: &Value) -> Value {
        let graph = self.graph();
        let all_nodes = graph["nodes"].as_array().unwrap();
        let node_rows = all_nodes
            .iter()
            .map(|n| self.material_node(n["node_id"].as_str().unwrap()))
            .collect::<Vec<_>>();
        let edge_rows = graph["edges"].as_array().unwrap().iter().map(|e| json!({
            "id":format!("philosophy:{}",e["edge_id"].as_str().unwrap()),"from_id":format!("philosophy:{}",e["from_id"].as_str().unwrap()),
            "to_id":format!("philosophy:{}",e["to_id"].as_str().unwrap()),"relation_type_id":"relates","content_revision":CONTENT,
            "source_refs":[e["source_ref"]],"display":{"label":{"ru":"Связано","en":"Related"}}})).collect::<Vec<_>>();
        let origin = request.get("origin").cloned().unwrap_or_else(
            || json!({"kind":"node","id":"philosophy:a","content_revision":CONTENT}),
        );
        let kind = origin["kind"].as_str().unwrap_or("node");
        let id = origin["id"].as_str().unwrap_or("philosophy:a");
        let origin = if kind == "relation" {
            let relation = edge_rows
                .iter()
                .find(|r| r["id"] == id)
                .cloned()
                .unwrap_or_else(|| edge_rows[0].clone());
            let endpoint = |node_id: &str| {
                let node = node_rows.iter().find(|node| node["id"] == node_id);
                json!({"node_id":node_id,"entity_id":node.and_then(|node|node["entity_id"].as_str()).unwrap_or("tos.node.fixture.unknown"),"content_revision":CONTENT})
            };
            json!({"kind":"relation","id":id,"content_revision":relation["content_revision"],"endpoints":{
                "from":endpoint(relation["from_id"].as_str().unwrap_or("")),
                "to":endpoint(relation["to_id"].as_str().unwrap_or(""))}})
        } else {
            json!({"kind":"node","id":id,"content_revision":CONTENT})
        };
        let nodes = node_rows.into_iter().take(3).collect::<Vec<_>>();
        let relations = edge_rows.into_iter().take(3).collect::<Vec<_>>();
        let node_ids = nodes
            .iter()
            .map(|n| n["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let rel_ids = relations
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let primary_node_ids = if kind == "node" {
            node_ids.iter().filter(|n| *n != id).cloned().collect()
        } else {
            Vec::<String>::new()
        };
        let context_node_ids = if kind == "node" {
            vec![id.to_owned()]
        } else {
            node_ids.clone()
        };
        let primary_relation_ids = if kind == "node" {
            rel_ids.clone()
        } else {
            rel_ids.iter().filter(|r| *r != id).cloned().collect()
        };
        let context_relation_ids = if kind == "relation" {
            vec![id.to_owned()]
        } else {
            Vec::<String>::new()
        };
        let node_count = node_ids.len();
        let relation_count = rel_ids.len();
        let inclusion_nodes = node_ids
            .iter()
            .map(|n| {
                (
                    n.clone(),
                    json!({"kind":if n==id {"origin"} else {"context-endpoint"}}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let inclusion_relations = rel_ids
            .iter()
            .map(|r| {
                (
                    r.clone(),
                    json!({"kind":if r==id {"origin"} else {"incident"}}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let scene = fixture_scene(
            &nodes,
            &relations,
            if kind == "node" { Some(id) } else { None },
            if kind == "relation" { Some(id) } else { None },
        );
        json!({"schema":"tos_exploration_result_v2","execution_version":"tos-exploration-execution-v6",
            "source_revision":REV,"snapshot_revision":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "origin":origin,"query":request,"nodes":nodes,"relations":relations,"status":"paused","limit_reason":null,
            "page":{"number":1,"primary_node_ids":primary_node_ids,"context_node_ids":context_node_ids,
                "primary_relation_ids":primary_relation_ids,"context_relation_ids":context_relation_ids,
                "next_cursor":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "returned_nodes":node_count,"returned_relations":relation_count,"scope":"resumable-neighborhood","work_units":3},
            "counts":{"scope":"cumulative-discovered-not-global-total","discovered_nodes":node_count,"emitted_relations":relation_count},
            "inclusion":{"authority":"query-execution-not-semantic-proof","nodes":inclusion_nodes,"relations":inclusion_relations},
            "scene":scene,"authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false},"writes_to_tree":false})
    }
}

struct FixtureFence;
impl DisclosureFence for FixtureFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        Ok(())
    }
}
fn packet(value: Value) -> Result<PreparedPacket<'static>, AccessError> {
    let body = serde_json::to_vec(&value).map_err(|_| {
        AccessError::new(
            AccessErrorCode::CorruptSelectedCarrier,
            "fixture JSON serialization failed",
        )
    })?;
    Ok(PreparedPacket {
        body,
        fence: Box::new(FixtureFence),
    })
}
fn unavailable() -> AccessError {
    AccessError::new(
        AccessErrorCode::Unavailable,
        "browser fixture operation unavailable",
    )
}
fn readonly() -> Value {
    json!({"grants_current_use":false,"performs_assessment":false,"writes_to_source":false})
}
fn check_probe(probe: &Arc<dyn AbortProbe>) -> Result<(), AccessError> {
    match probe.reason() {
        Some(AbortReason::Cancelled) => Err(AccessError::new(
            AccessErrorCode::Cancelled,
            "browser fixture request cancelled",
        )),
        Some(AbortReason::DeadlineExceeded) => Err(AccessError::new(
            AccessErrorCode::DeadlineExceeded,
            "browser fixture deadline exceeded",
        )),
        None => Ok(()),
    }
}
fn foundation_to_value(value: &tos_foundation::JsonValue) -> Value {
    let bytes = tos_foundation::canonical_bytes_v1(
        value,
        tos_foundation::CanonicalProfile::SourceRecordDigestV1,
        tos_foundation::JsonLimits::default(),
    )
    .unwrap_or_default();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}
fn fixture_scene(
    nodes: &[Value],
    relations: &[Value],
    focus_node: Option<&str>,
    _focus_relation: Option<&str>,
) -> Value {
    let mut vertices=nodes.iter().map(|node| {
        let entity=node["entity_id"].as_str().unwrap_or("tos.node.fixture.unknown");
        let id=format!("tos-scene:entity:{entity}");
        json!({"id":id,"entity_id":entity,"node_ids":[node["id"]],"representative_node_id":node["id"]})
    }).collect::<Vec<_>>();
    vertices.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let vertex_id = |node_id: &str| -> String {
        let entity = nodes
            .iter()
            .find(|node| node["id"] == node_id)
            .and_then(|node| node["entity_id"].as_str())
            .unwrap_or("tos.node.fixture.unknown");
        format!("tos-scene:entity:{entity}")
    };
    let mut arcs = relations
        .iter()
        .map(|relation| {
            json!({"relation_id":relation["id"],
        "from_id":vertex_id(relation["from_id"].as_str().unwrap_or("")),
        "to_id":vertex_id(relation["to_id"].as_str().unwrap_or("")).to_string()})
        })
        .collect::<Vec<_>>();
    arcs.sort_by(|left, right| {
        left["relation_id"]
            .as_str()
            .cmp(&right["relation_id"].as_str())
    });
    let vertex_ids = vertices
        .iter()
        .map(|item| item["id"].clone())
        .collect::<Vec<_>>();
    let relation_ids = arcs
        .iter()
        .map(|item| item["relation_id"].clone())
        .collect::<Vec<_>>();
    let focus_vertex = focus_node.map(vertex_id);
    json!({"schema_version":"tos_knowledge_scene_v1","vertices":vertices,"arcs":arcs,"collapsed_relation_ids":[],
        "focus_vertex_id":focus_vertex,"scope":"returned-packet-only","identity_rule":"declared-tos-entity-id",
        "authority":"presentation-mapping-not-semantic-admission",
        "compact":{"rule":"explicit-claim-paths-v1","vertex_ids":vertex_ids,"relation_ids":relation_ids,
            "claim_paths":[],"folded_vertex_ids":[],"retained_claims":[]}})
}
impl AccessExecutor for BrowserFixtureExecutor {
    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        Err(unavailable())
    }
    fn source_gap_available(&self) -> bool {
        true
    }
    fn source_gap(
        &self,
        request: SourceGapRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        check_probe(&probe)?;
        let records = [
            PublicSourceGapRecord {
                source_ref: "ToS/source-witnesses/access-requests/public-ledger/fixture-edition.access-request.json",
                raw: br#"{"schema_version":"tos_access_request_v1","request_id":"fixture-edition","material":{"title":"Nietzsche edition (synthetic test fixture)","tos_refs":["a"]}}"#,
            },
            PublicSourceGapRecord {
                source_ref: "ToS/source-witnesses/access-requests/public-ledger/fixture-lexicon.access-request.json",
                raw: r#"{"schema_version":"tos_access_request_v1","request_id":"fixture-lexicon","material":{"title":"Nietzsche-Wörterbuch (synthetic test fixture)","tos_refs":["a"]}}"#.as_bytes(),
            },
        ];
        let body = compute_source_gap_packet(
            &records,
            &request,
            SourceGapBudget {
                json: tos_foundation::JsonLimits::default(),
                max_work_steps: 100_000,
                max_response_bytes: 64 * 1024,
            },
            probe.as_ref(),
        )
        .map_err(|_| {
            AccessError::new(
                AccessErrorCode::CorruptSelectedCarrier,
                "synthetic source-gap packet failed",
            )
        })?;
        Ok(PreparedPacket {
            body,
            fence: Box::new(FixtureFence),
        })
    }
    fn knowledge_search_compressed_available(&self) -> bool {
        true
    }
    fn knowledge_search_compressed(
        &self,
        request: CompressedSearchRequest,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        let graph = self.graph();
        let mut nodes = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| self.material_node(n["node_id"].as_str().unwrap()))
            .collect::<Vec<_>>();
        if !request.query.is_empty()
            && !request.query.eq_ignore_ascii_case("fixture")
            && !request.query.eq_ignore_ascii_case("philosophy")
        {
            nodes.retain(|n| {
                n["id"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&request.query.to_lowercase())
                    || n["label"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&request.query.to_lowercase())
            });
        }
        if nodes.is_empty() {
            nodes = graph["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| self.material_node(n["node_id"].as_str().unwrap()))
                .collect();
        }
        let offset = usize::from(request.cursor.is_some());
        let page = nodes
            .into_iter()
            .skip(offset)
            .take(request.limit)
            .collect::<Vec<_>>();
        let graph_edges = graph["edges"].as_array().unwrap();
        let relations=graph_edges.iter().skip(offset).take(request.limit).map(|e|json!({
            "id":format!("philosophy:{}",e["edge_id"].as_str().unwrap()),"from_id":format!("philosophy:{}",e["from_id"].as_str().unwrap()),"to_id":format!("philosophy:{}",e["to_id"].as_str().unwrap()),
            "predicate_id":"relates","relation_type_id":"relates","content_revision":CONTENT,"source_refs":[e["source_ref"]],
            "display":{"label":{"ru":"Связано","en":"Related"}}})).collect::<Vec<_>>();
        let next = if offset == 0 {
            Some("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd")
        } else {
            None
        };
        packet(
            json!({"schema":"tos_knowledge_search_compressed_v3","search_mode":"compressed","source_revision":REV,
            "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false},"nodes":page,"relations":relations,
            "counts":{"matching_nodes":null,"matching_relations":null},"page":{"cursor":request.cursor,"limit_per_kind":request.limit,
                "has_more":next.is_some(),"next_cursor":next}}),
        )
    }
    fn source_read_available(&self) -> bool {
        self.source_enabled()
    }
    fn source_read(
        &self,
        request: tos_access::source_read::Request,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        check_probe(&probe)?;
        let body: Value = serde_json::from_slice(&request.body).unwrap_or_else(|_| json!({}));
        let target = if body.get("target").is_some() {
            body["target"].clone()
        } else {
            self.source_target()
        };
        let revision = REV;
        let content = target["content_revision"]
            .as_str()
            .unwrap_or("sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
        let access = json!({"scope":"public-metadata-record","visibility":"public_metadata_only","visibility_verified":true,
            "rights_revalidated":false,"rights_scope":"metadata-disclosure-only","authority":"source-owner-public-metadata-contract"});
        let epoch = json!({"source_revision":REV,"catalog_root_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "catalog_namespace":"tos.catalog.e2e.fixture","source_publication":{"protocol":"tos_selected_source_metadata_v1",
                "token":"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","generation":1}});
        let handle = json!({"schema_version":"tos_source_read_handle_v1","issuer":"Tree-of-Sophia/source-witnesses","epoch":epoch,
            "target":target,"access":access,"handle_digest":"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"});
        match request.operation {
            tos_access::source_read::Operation::Capabilities => packet(
                json!({"schema_version":"tos_source_read_capabilities_v1","available":true,
                "source_epoch":epoch,"representations":[],"authority":{"writes_to_source":false,"grants_current_use":false}}),
            ),
            tos_access::source_read::Operation::Contract => packet(
                json!({"schema_version":"tos_source_read_contract_v1","available":true,"authority":readonly()}),
            ),
            tos_access::source_read::Operation::Discover => packet(
                json!({"schema_version":"tos_source_handle_discovery_v1","status":"available","reason":"exact-current-version",
                "source_revision":revision,"content_revision":content,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false,"target":target,"handle":handle}),
            ),
            tos_access::source_read::Operation::Read => packet(
                json!({"schema_version":"tos_source_read_result_v1","status":"available","reason":"owner-record-available",
                "source_revision":revision,"content_revision":content,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false,
                "handle":body["handle"],"layer":"metadata_record","record_ref":target["record_ref"],"record":self.source_record(),
                "access":access,"provenance":{"catalog":{"record_key":self.source_record()["record_id"],"row_sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},
                    "source":{"source_ref":if self.scenario=="source-native-metadata" {"ToS/synthetic/native-metadata.json"} else {"ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json"}}}}),
            ),
        }
    }
    fn knowledge_available(&self, _: tos_access::KnowledgeOperation) -> bool {
        true
    }
    fn knowledge(
        &self,
        request: KnowledgeRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        check_probe(&probe)?;
        use tos_access::KnowledgeRequest as K;
        use tos_query::philosophy_read::PhilosophyReadRequest as P;
        let result = match request {
            K::PhilosophyViewIds => {
                json!({"schema":"tos_philosophy_graph_views_v1","views":[{"view_id":"chronology","title":"Chronology"},{"view_id":"direct-only","title":"Direct only"}],"source_revision":REV})
            }
            K::CorpusViewIds => {
                json!({"schema":"tos_corpus_graph_views_v1","graph_views":[],"source_revision":REV})
            }
            K::Catalog => self.catalog(),
            K::Contracts => self.contracts(),
            K::SearchCapabilities => self.search_capabilities(),
            K::ExplorationContracts => self.exploration_capabilities(),
            K::Node { node_id, .. } => self.node_inspect(&node_id),
            K::Philosophy(P::View { view_id, limit }) => self.graph_view(&view_id, limit),
            K::Philosophy(P::Views) => {
                json!({"schema":"tos_philosophy_graph_views_v1","source_revision":REV,"views":[{"view_id":"chronology","title":"Chronology"},{"view_id":"direct-only","title":"Direct only"}]})
            }
            K::Philosophy(P::Status) => {
                json!({"schema":"tos_philosophy_graph_status_v1","source_revision":REV,"available":true,"nodes":3,"edges":3})
            }
            K::Philosophy(P::Snapshot) => {
                json!({"schema":"tos_philosophy_mcp_snapshot_v1","snapshot_review":{
                    "snapshot_schema_version":"tos_philosophy_graph_projection_snapshot_v1",
                    "current_snapshot":{"projection_fingerprint":CONTENT}},
                    "runtime_projection_boundary":self.graph()["runtime_projection_boundary"],
                    "authority_note":"Synthetic fingerprint for browser review routing."})
            }
            K::Philosophy(P::Node { node_id }) => self.node_inspect(&node_id),
            K::Lens(spec) => self.lens(
                spec.object_get("focus_id")
                    .and_then(tos_foundation::JsonValue::as_str),
            ),
            K::Explore(request) => self.exploration(&foundation_to_value(&request)),
            K::Temporal(_) => {
                json!({"schema":"tos_interpretation_comparison_v1","source_revision":REV,"page_updated":true,"authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false},"comparisons":[],"items":[]})
            }
            K::Focus(_) => self.lens(Some("philosophy:a")),
            K::Relation { relation_id } => {
                json!({"schema":"tos_knowledge_inspect_v1","source_revision":REV,"match":{"id":relation_id,"semantic_kind":"relation"},"matches":[]})
            }
            K::Philosophy(_) => {
                json!({"schema":"tos_philosophy_graph_packet_v1","source_revision":REV,"nodes":[],"edges":[],"authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false}})
            }
            _ => {
                json!({"schema":"tos_browser_fixture_v1","source_revision":REV,"available":true,"authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false}})
            }
        };
        packet(result)
    }
}
