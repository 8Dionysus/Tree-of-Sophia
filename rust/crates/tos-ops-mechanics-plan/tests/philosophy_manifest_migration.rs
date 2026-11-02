use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

fn read_json(root: &Path, path: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(path)).unwrap()).unwrap()
}

fn rows(root: &Path, path: &str) -> Vec<Value> {
    fs::read_to_string(root.join(path))
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn derived_manifest_values_match_origin_main_before_migration() {
    // This fixture records the removed values at origin/main 12c5e797, before
    // the migration; it remains an independent oracle when Git history is shallow.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let baseline: Value =
        serde_json::from_str(include_str!("fixtures/philosophy-branch-origin-main.json")).unwrap();
    let schema = read_json(root, "ToS/contracts/philosophy-branch-manifest.schema.json");
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut cache: HashMap<String, Vec<Value>> = HashMap::new();
    let manifests = baseline["manifests"].as_object().unwrap();
    assert_eq!(manifests.len(), 851);
    for (path, old) in manifests {
        let manifest = read_json(root, path);
        assert!(
            validator.is_valid(&manifest),
            "{path}: {:?}",
            validator.iter_errors(&manifest).collect::<Vec<_>>()
        );
        let directory = Path::new(path).parent().unwrap().to_str().unwrap();
        assert_eq!(old["path"], directory, "{path}");
        if let Some(children) = old.get("children") {
            let mut actual: Vec<String> = if let Some(explicit) = manifest.get("children") {
                explicit
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect()
            } else {
                fs::read_dir(root.join(directory))
                    .unwrap()
                    .map(Result::unwrap)
                    .filter(|e| e.file_type().unwrap().is_dir())
                    .map(|e| e.file_name().into_string().unwrap())
                    .collect()
            };
            let mut expected: Vec<_> = children
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "{path} children");
        }
        for key in [
            "anchor_count",
            "proposed_node_count",
            "proposed_relation_count",
            "row_count",
            "packet_count",
            "dossier_count",
            "source_planting_count",
            "planting_count",
        ] {
            let Some(expected) = old.get(key) else {
                continue;
            };
            let count = match key {
                "anchor_count" => {
                    rows(root, &format!("{directory}/source-anchor-backlog.jsonl")).len()
                }
                "proposed_node_count" | "proposed_relation_count" => {
                    let summary = read_json(root, &format!("{directory}/pre-canon-summary.json"));
                    let reference = summary[if key == "proposed_node_count" {
                        "proposed_nodes_ref"
                    } else {
                        "proposed_relations_ref"
                    }]
                    .as_str()
                    .unwrap();
                    cache
                        .entry(reference.to_owned())
                        .or_insert_with(|| rows(root, reference))
                        .iter()
                        .filter(|row| row["dossier_id"] == summary["dossier_id"])
                        .count()
                }
                "row_count" => {
                    let single = format!("{directory}/rows.jsonl");
                    if root.join(&single).is_file() {
                        rows(root, &single).len()
                    } else {
                        ["table-i", "table-ii", "table-iii"]
                            .iter()
                            .map(|table| {
                                rows(root, &format!("{directory}/{table}/rows.jsonl")).len()
                            })
                            .sum()
                    }
                }
                "packet_count" => manifest["source_refs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|reference| rows(root, reference.as_str().unwrap()).len())
                    .sum(),
                "dossier_count" => rows(root, &format!("{directory}/index.jsonl")).len(),
                "source_planting_count" => {
                    manifest["source_planting_refs"].as_array().unwrap().len()
                }
                "planting_count" => manifest["planting_refs"].as_array().unwrap().len(),
                _ => unreachable!(),
            };
            assert_eq!(expected.as_u64().unwrap() as usize, count, "{path} {key}");
        }
    }
}
