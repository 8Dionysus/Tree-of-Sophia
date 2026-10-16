//! Native graph/view regression for exact prepared-dossier backlog carriage.
use serde_json::{Value, json};
use tos_foundation::JsonLimits;
use tos_query::{
    AbortProbe, AbortReason, InspectBudget,
    philosophy_read::{PhilosophyReadBudget, compute_source_philosophy_view_diagnostic},
};

struct NeverAbort;
impl AbortProbe for NeverAbort {
    fn reason(&self) -> Option<AbortReason> {
        None
    }
}

fn dossier_graph() -> (Value, u64) {
    let families = [
        ("source_anchor_backlog", "ToS/fixture/source-anchor.jsonl"),
        ("term_index", "ToS/fixture/term-index.jsonl"),
        ("transmission_backlog", "ToS/fixture/transmission.jsonl"),
    ];
    let mut next_row = [0u64; 3];
    let mut expected_total = 0;
    let nodes = (0..190)
        .map(|index| {
            let dossier_id = if index == 0 {
                "A01".to_owned()
            } else if index == 187 {
                "T3-51".to_owned()
            } else if index == 188 {
                "T3-43".to_owned()
            } else if index == 189 {
                "T3-57".to_owned()
            } else {
                format!("fixture-{index:03}")
            };
            let node_id = format!("atlas-dossier:{dossier_id}");
            let total = 92 + if index < 119 { 1 } else { 0 };
            expected_total += total;
            let counts = [total / 3, total / 3, total - 2 * (total / 3)];
            let mut source_backlogs = serde_json::Map::new();
            for (family_index, (family, source_ref)) in families.iter().enumerate() {
                let count = counts[family_index];
                let rows = (0..count)
                    .map(|_| {
                        next_row[family_index] += 1;
                        let row = next_row[family_index];
                        json!({
                            "source_record": {"dossier_id": dossier_id, "source_row": row},
                            "source_record_ref": source_ref,
                            "source_file_sha256": "a".repeat(64),
                            "source_record_sha256": "b".repeat(64),
                            "source_format": "jsonl",
                            "source_row": row,
                            "source_line": row
                        })
                    })
                    .collect::<Vec<_>>();
                source_backlogs.insert(
                    (*family).to_owned(),
                    json!({
                        "source_ref": source_ref,
                        "source_file_sha256": "a".repeat(64),
                        "record_count": rows.len(),
                        "records": rows
                    }),
                );
            }
            json!({
                "node_id": node_id,
                "node_type": "prepared-dossier",
                "label": dossier_id,
                "source_ref": "ToS/philosophy/atlas/dossiers/index.jsonl",
                "view_ids": ["source-evidence"],
                "properties": {
                    "dossier_id": dossier_id,
                    "source_backlogs": source_backlogs
                }
            })
        })
        .collect::<Vec<_>>();
    let node_ids = nodes
        .iter()
        .map(|node| node["node_id"].clone())
        .collect::<Vec<_>>();
    (
        json!({
            "schema_version": "tos_philosophy_graph_projection_v2",
            "views": [{
                "view_id": "source-evidence",
                "layout_hint": "evidence-dag",
                "node_ids": node_ids,
                "edge_ids": [],
                "source_refs": ["ToS/philosophy/atlas/dossiers/index.jsonl"],
                "review_intent": "Preserve source evidence context."
            }],
            "nodes": nodes,
            "edges": [],
            "clusters": [],
            "review_packets": [{"view_id": "source-evidence", "packet_id": "review:source-evidence"}]
        }),
        expected_total,
    )
}

fn backlog_total(node: &Value) -> u64 {
    node["properties"]["source_backlogs"]
        .as_object()
        .unwrap()
        .values()
        .map(|family| family["record_count"].as_u64().unwrap())
        .sum()
}

fn backlog_rows(node: &Value) -> usize {
    node["properties"]["source_backlogs"]
        .as_object()
        .unwrap()
        .values()
        .map(|family| family["records"].as_array().unwrap().len())
        .sum()
}

#[test]
fn all_prepared_dossier_backlogs_survive_native_source_evidence_view_without_authority_upgrade() {
    let (graph, expected_total) = dossier_graph();
    assert_eq!(expected_total, 17_599);
    let dossiers = graph["nodes"].as_array().unwrap();
    assert_eq!(dossiers.iter().map(backlog_total).sum::<u64>(), 17_599);
    assert_eq!(dossiers.iter().map(backlog_rows).sum::<usize>(), 17_599);

    let raw_graph = serde_json::to_vec(&graph).unwrap();
    let budget = InspectBudget {
        max_open_vm_steps: 1,
        max_read_vm_steps: 1,
        max_matches: 1000,
        max_rows: 1_000,
        max_field_bytes: 32_768,
        max_payload_bytes: 8 * 1024 * 1024,
        max_decoded_bytes: 8 * 1024 * 1024,
        max_response_bytes: 8 * 1024 * 1024,
        json: JsonLimits::new(8 * 1024 * 1024, 64, 1_000_000, 4096).unwrap(),
    };
    let result = compute_source_philosophy_view_diagnostic(
        &raw_graph,
        "source-evidence",
        PhilosophyReadBudget {
            inspect: budget,
            max_work_steps: 1_000_000,
        },
        &NeverAbort,
    )
    .unwrap();
    let page: Value = serde_json::from_slice(&result).unwrap();
    assert_eq!(page["view"]["view_id"], "source-evidence");
    let queried = page["nodes"].as_array().unwrap();
    assert_eq!(queried.len(), 190);
    assert_eq!(queried.iter().map(backlog_total).sum::<u64>(), 17_599);
    assert_eq!(queried.iter().map(backlog_rows).sum::<usize>(), 17_599);
    for (source, projected) in dossiers.iter().zip(queried) {
        assert_eq!(source["node_id"], projected["node_id"]);
        assert_eq!(
            source["properties"]["source_backlogs"],
            projected["properties"]["source_backlogs"]
        );
        assert_eq!(projected["node_type"], "prepared-dossier");
        assert!(projected["properties"].get("canon_status").is_none());
        assert!(projected["properties"].get("authority_posture").is_none());
    }
}
