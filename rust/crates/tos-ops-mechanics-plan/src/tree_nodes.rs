//! Source-owned node shape and consistency, without canon admission.
use crate::route_cards::RouteSources;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_compiler::knowledge_canon_source::validate_authored_node_mechanics;
use tos_foundation::{JsonLimits, JsonMode, parse_json};

pub type Issue = (String, String);
const SCHEMA: &str = "ToS/contracts/tos-node-contract.schema.json";
const MAX_FILE: usize = 8 * 1024 * 1024;
const MAX_TOTAL: usize = 64 * 1024 * 1024;
fn bad(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
fn check(sources: &RouteSources, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(bad("tree node validation cancelled"));
    }
    sources.check()
}
fn parse(raw: &[u8]) -> io::Result<Value> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(MAX_FILE, 96, 1_000_000, 4096)
            .map_err(|e| bad(format!("node JSON limits: {e:?}")))?,
    )
    .map_err(|e| bad(format!("invalid exact node JSON: {e:?}")))?;
    serde_json::from_slice(raw).map_err(io::Error::other)
}
fn validator(raw: &[u8]) -> io::Result<jsonschema::Validator> {
    let schema = parse(raw)?;
    if !schema.is_object() {
        return Err(bad("node schema root must be an object"));
    }
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(false)
        .offline()
        .build(&schema)
        .map_err(|e| bad(format!("node schema load/check failed: {e}")))
}
fn issue(
    issues: &mut Vec<Issue>,
    total: &mut usize,
    path: &str,
    message: String,
) -> io::Result<()> {
    *total = total
        .checked_add(path.len())
        .and_then(|n| n.checked_add(message.len()))
        .filter(|n| *n <= 1024 * 1024)
        .ok_or_else(|| bad("node diagnostic byte bound"))?;
    if issues.len() >= 4096 {
        return Err(bad("node diagnostic count bound"));
    }
    issues.push((path.to_owned(), message));
    Ok(())
}
pub fn validate(sources: &mut RouteSources, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    check(sources, cancel)?;
    let mut input_bytes = 0;
    let schema = sources.bounded_bytes(SCHEMA, MAX_FILE, &mut input_bytes, MAX_TOTAL)?;
    let validator = validator(&schema)?;
    let paths = sources.selected_files_with_limits(
        "ToS/canon",
        &|p, directory| directory || p.ends_with("/node.json"),
        100_000,
        4096,
        16_384,
    )?;
    let mut issues = Vec::new();
    let mut output_bytes = 0;
    let mut identities = BTreeMap::new();
    if paths.is_empty() {
        issue(
            &mut issues,
            &mut output_bytes,
            "ToS/canon/",
            "no canonical tree node.json files found".into(),
        )?;
    }
    for path in paths {
        check(sources, cancel)?;
        let raw = sources.bounded_bytes(&path, MAX_FILE, &mut input_bytes, MAX_TOTAL)?;
        let value = match parse(&raw) {
            Ok(value) => value,
            Err(error) => {
                issue(&mut issues, &mut output_bytes, &path, error.to_string())?;
                continue;
            }
        };
        let mut schema_failed = false;
        for error in validator.iter_errors(&value) {
            check(sources, cancel)?;
            schema_failed = true;
            issue(
                &mut issues,
                &mut output_bytes,
                &path,
                format!("{}: {error}", error.instance_path()),
            )?;
        }
        if schema_failed {
            continue;
        }
        match validate_authored_node_mechanics(&raw, MAX_FILE) {
            Ok(identity) => {
                if let Some(prior) = identities.insert(identity.clone(), path.clone()) {
                    issue(
                        &mut issues,
                        &mut output_bytes,
                        &path,
                        format!("duplicate canonical node_id {identity}; already owned by {prior}"),
                    )?;
                }
            }
            Err(error) => issue(&mut issues, &mut output_bytes, &path, format!("{error}"))?,
        }
        check(sources, cancel)?;
    }
    sources.verify_root()?;
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn synthetic() -> Value {
        json!({"schema_version":"tos_canonical_node_v1","record_version":1,
            "node_id":"tos.source.synthetic","node_type":"source","source_anchor":"synthetic:source",
            "key_terms":["question"],"distilled_thesis":"A synthetic source-linked question.",
            "relations":[{"relation":"commentary-on","target_ref":"synthetic:related"}],
            "interpretation_layers":["source_linked"],"language_witnesses":[
                {"language":"el","role":"canonical_source","segments":[{"segment_id":"opening","text":"A question."},{"segment_id":"return","text":"An open answer."}]},
                {"language":"ja","role":"working_translation","segments":[{"segment_id":"opening","text":"問い"},{"segment_id":"return","text":"答え"}]}],
            "translation_tensions":[{"segment_id":"return","note":"Synthetic qualified reading."}]})
    }
    #[test]
    fn node_shared_contract_preserves_extensions_and_rejects_cross_field_and_number_defects() {
        let schema = include_bytes!("../../../../ToS/contracts/tos-node-contract.schema.json");
        let v = validator(schema).unwrap();
        let source = synthetic();
        assert!(v.is_valid(&source));
        let raw = serde_json::to_vec(&source).unwrap();
        assert_eq!(
            validate_authored_node_mechanics(&raw, MAX_FILE).unwrap(),
            "tos.source.synthetic"
        );
        for defect in 0..7 {
            let mut value = source.clone();
            match defect {
                0 => value["node_id"] = json!("tos.concept.synthetic"),
                1 => {
                    let duplicate = value["language_witnesses"][0]["segments"][0].clone();
                    value["language_witnesses"][0]["segments"]
                        .as_array_mut()
                        .unwrap()
                        .push(duplicate);
                }
                2 => {
                    value["language_witnesses"][1]["segments"]
                        .as_array_mut()
                        .unwrap()
                        .pop();
                }
                3 => value["language_witnesses"][1]["segments"]
                    .as_array_mut()
                    .unwrap()
                    .reverse(),
                4 => value["translation_tensions"][0]["segment_id"] = json!("absent"),
                5 => value["lineage_relations"] = json!([]),
                _ => value["language_witnesses"][1]["language"] = json!("el"),
            }
            assert!(
                validate_authored_node_mechanics(&serde_json::to_vec(&value).unwrap(), MAX_FILE)
                    .is_err(),
                "defect {defect}"
            );
        }
        let mut source = source;
        source["field_languages"] = json!({"distilled_thesis":{"language":null,"script":null,"qualification":{"measure":"NUMBER"}}});
        let raw = serde_json::to_string(&source).unwrap();
        for token in [
            "1e-9999",
            "0.10000000000000001",
            "9007199254740993.0",
            "1e9999",
            "NaN",
            "Infinity",
        ] {
            assert!(
                validate_authored_node_mechanics(
                    raw.replace("\"NUMBER\"", token).as_bytes(),
                    MAX_FILE
                )
                .is_err(),
                "{token}"
            );
        }
        for token in ["0.1", "-0.0", "1.2500e-20", "1.000", "5e-324"] {
            assert!(
                validate_authored_node_mechanics(
                    raw.replace("\"NUMBER\"", token).as_bytes(),
                    MAX_FILE
                )
                .is_ok(),
                "{token}"
            );
        }
        let duplicate =
            raw.strip_suffix('}').unwrap().to_owned() + ",\"node_id\":\"tos.source.other\"}";
        assert!(parse(duplicate.as_bytes()).is_err());
    }
}
