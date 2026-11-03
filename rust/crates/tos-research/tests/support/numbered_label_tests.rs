use super::*;
#[test]
fn intersection_preserves_target_order_and_refuses_duplicate_or_materialized_237a() {
    let curation: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/edition-curation.v1.json")).unwrap();
    let selected = curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["stage"] == "numbered-label-correspondence")
        .unwrap();
    let recipe = Recipe::selected(selected).unwrap();
    let rows=(1..=298).map(|i|json!({"unit_key":i.to_string(),"anchor_ref":format!("tos.anchor.test.unit-{i}"),"pdf_page":i})).collect::<Vec<_>>();
    let mut source = json!({"work_ref":"tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese","unit_starts":rows});
    let mut target = source.clone();
    source["unit_starts"]
        .as_array_mut()
        .unwrap()
        .push(json!({"unit_key":"237a","anchor_ref":"tos.anchor.test.extra","pdf_page":237}));
    target["unit_starts"].as_array_mut().unwrap().reverse();
    let pairs = pairings(&source, &target, &recipe).unwrap();
    let value = serde_json::to_value(Out::Array(pairs)).unwrap();
    assert_eq!(value[0]["unit_key"], "298");
    assert_eq!(value[297]["unit_key"], "1");
    assert_eq!(value[0]["translation_alignment_claimed"], false);
    assert!(
        !value
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["unit_key"] == "237a")
    );
    let first = target["unit_starts"][0].clone();
    target["unit_starts"][1] = first;
    assert!(pairings(&source, &target, &recipe).is_err());
    target["unit_starts"][1] = source["unit_starts"][298].clone();
    assert!(pairings(&source, &target, &recipe).is_err());
}

#[test]
fn selected_recipe_supports_unpaired_keys_on_both_sides() {
    let curation: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/edition-curation.v1.json")).unwrap();
    let mut selected = curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["stage"] == "numbered-label-correspondence")
        .unwrap()
        .clone();
    selected["expected_source_count"] = json!(2);
    selected["expected_target_count"] = json!(2);
    selected["expected_shared_count"] = json!(1);
    selected["source_only_keys"] = json!(["1"]);
    selected["target_only_keys"] = json!(["3"]);
    selected["unpaired_units"] = json!([
        {"side":"source","unit_key":"1","reason":"source-only","target_unit_materialized":false},
        {"side":"target","unit_key":"3","reason":"target-only","target_unit_materialized":true}
    ]);
    let recipe = Recipe::selected(&selected).unwrap();
    let entry =
        |key| json!({"unit_key":key,"anchor_ref":format!("tos.anchor.test.{key}"),"pdf_page":1});
    let source = json!({"work_ref":recipe.work,"unit_starts":[entry("1"),entry("2")]});
    let target = json!({"work_ref":recipe.work,"unit_starts":[entry("3"),entry("2")]});
    let pairs = pairings(&source, &target, &recipe).unwrap();
    let value = serde_json::to_value(Out::Array(pairs)).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 1);
    assert_eq!(value[0]["unit_key"], "2");
    selected["unpaired_units"][1]["unit_key"] = json!("2");
    assert!(Recipe::selected(&selected).is_err());
}

#[test]
fn selected_input_fixity_rejects_a_mismatched_map() {
    let root =
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
            .unwrap();
    let curation: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/edition-curation.v1.json")).unwrap();
    let mut selected = curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["stage"] == "numbered-label-correspondence")
        .unwrap()
        .clone();
    selected["input_fixity"][0]["sha256"] = json!("0".repeat(64));
    let ctx = ResearchExecution::new(&root, 60).unwrap();
    let error = run(&ctx, &selected, false, None, None).unwrap_err();
    assert!(error.contains("selected label input fixity differs"));
}
