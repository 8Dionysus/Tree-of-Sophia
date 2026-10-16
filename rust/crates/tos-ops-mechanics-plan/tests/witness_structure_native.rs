//! The explicit source-data integration route. Software-only CI has no witness
//! corpus; run this target with --ignored when the selected metadata is present.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tos_ops_mechanics_plan::witness_structure as w;
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf()
}
fn load(path: &str) -> Value {
    serde_json::from_slice(&fs::read(root().join(path)).unwrap()).unwrap()
}
fn lines(path: &str) -> Vec<Value> {
    fs::read_to_string(root().join(path))
        .unwrap()
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
#[test]
#[ignore = "explicit tracked witness metadata integration; source_foundation data route"]
fn tracked_five_families_and_actual_native_entry_preserve_addresses() {
    let output = Command::new(env!("CARGO_BIN_EXE_tos-ops-mechanics-plan"))
        .args([
            "--repo-root",
            root().to_str().unwrap(),
            "--witness-structure-validate",
        ])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("validated text-free witness-structure correspondences")
    );
    let set = load(w::ANCHOR_SET_PATH);
    let a = lines(w::ANCHOR_RECORDS_PATH);
    assert_eq!(set["bindings"].as_array().unwrap().len(), 82);
    assert_eq!(a.len(), 246);
    assert!(a.iter().all(|a| a["status"] == "proposed"));
    assert_eq!(set["source_text_included"], false);
    let p = load(w::PARALLEL_MAP_PATH);
    assert_eq!(p["divisions"].as_array().unwrap().len(), 11);
    assert_eq!(lines(w::PARALLEL_ANCHOR_RECORDS_PATH).len(), 22);
    assert_eq!(
        p["summary"]["supplemental_numbered_units"],
        json!(["65a", "73a", "237a"])
    );
    assert_eq!(
        p["summary"]["exact_numbered_unit_start_pages_materialized"],
        0
    );
    assert_eq!(p["summary"]["human_review_performed"], false);
    let source = load(w::NUMBERED_UNIT_MAP_PATH);
    assert_eq!(source["unit_starts"].as_array().unwrap().len(), 299);
    assert_eq!(lines(w::NUMBERED_UNIT_ANCHOR_RECORDS_PATH).len(), 299);
    let u = source["unit_starts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["unit_key"] == "237a")
        .unwrap();
    assert_eq!(u["pdf_page"], 189);
    assert_eq!(u["basis"], "source_visible_repeated_number_review");
    let target = load(w::TARGET_NUMBERED_UNIT_MAP_PATH);
    let units = target["unit_starts"].as_array().unwrap();
    assert_eq!(units.len(), 298);
    assert_eq!(
        lines(w::TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH).len(),
        298
    );
    assert!(!units.iter().any(|u| u["unit_key"] == "237a"));
    for (key, page) in [("6", 244), ("65a", 291), ("73a", 292), ("285", 399)] {
        let u = units.iter().find(|u| u["unit_key"] == key).unwrap();
        assert_eq!(u["pdf_page"], page);
        if key == "6" {
            assert_eq!(u["basis"], "source_visible_ocr_disambiguation");
        }
    }
    let labels = load(w::NUMBERED_UNIT_LABEL_MAP_PATH);
    assert_eq!(labels["pairings"].as_array().unwrap().len(), 298);
    assert!(
        !labels["pairings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["unit_key"] == "237a")
    );
    assert_eq!(labels["summary"]["source_only_unit_keys"], json!(["237a"]));
    assert_eq!(labels["method"]["source_to_target_text_compared"], false);
    assert_eq!(labels["method"]["translation_alignment_inferred"], false);
    for path in [
        w::ANCHOR_RECORDS_PATH,
        w::PARALLEL_ANCHOR_RECORDS_PATH,
        w::NUMBERED_UNIT_ANCHOR_RECORDS_PATH,
        w::TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH,
    ] {
        for a in lines(path) {
            assert_eq!(a["status"], "proposed");
            for s in a["selectors"].as_array().unwrap() {
                assert!(!matches!(
                    s["type"].as_str(),
                    Some("text_quote" | "text_position")
                ));
            }
        }
    }
}
#[test]
#[ignore = "explicit tracked witness metadata integration; source_foundation data route"]
fn source_only_number_and_translation_claims_are_rejected_by_owner_schemas() {
    for (path, schema, pointer) in [
        (
            w::TARGET_NUMBERED_UNIT_MAP_PATH,
            w::TARGET_NUMBERED_UNIT_SCHEMA_PATH,
            "/numbering_asymmetries/0/target_numbered_unit_materialized",
        ),
        (
            w::NUMBERED_UNIT_LABEL_MAP_PATH,
            w::NUMBERED_UNIT_LABEL_SCHEMA_PATH,
            "/pairings/0/translation_alignment_claimed",
        ),
        (
            w::PARALLEL_MAP_PATH,
            w::PARALLEL_SCHEMA_PATH,
            "/divisions/0/translation_equivalence_claimed",
        ),
    ] {
        let schema = load(schema);
        let validator = jsonschema::options()
            .offline()
            .should_validate_formats(true)
            .build(&schema)
            .unwrap();
        let mut v = load(path);
        assert!(validator.is_valid(&v));
        *v.pointer_mut(pointer).unwrap() = json!(true);
        assert!(!validator.is_valid(&v));
    }
}
