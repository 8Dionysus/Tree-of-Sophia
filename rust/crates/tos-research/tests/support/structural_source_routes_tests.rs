use super::*;

#[test]
fn selected_routes_refuse_traversal_and_duplicate_configurations() {
    let curation: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/edition-curation.v1.json")).unwrap();
    let mut recipe = curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["stage"] == "structural-source-routes")
        .unwrap()
        .clone();
    assert_eq!(selected_configs(&recipe).unwrap().len(), 2);
    recipe["configurations"][0]["slug"] = json!("../outside");
    assert!(selected_configs(&recipe).is_err());
    let first = recipe["configurations"][1].clone();
    recipe["configurations"][0] = first;
    assert!(selected_configs(&recipe).is_err());
}

#[test]
fn structural_profile_refuses_mismatched_schema_and_output_traversal() {
    let curation: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/edition-curation.v1.json")).unwrap();
    let mut recipe = curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["stage"] == "structural-source-routes")
        .unwrap()
        .clone();
    assert!(Profile::selected(&recipe).is_ok());
    recipe["correspondence_schema_uri"] =
        json!("https://tree-of-sophia.local/ToS/contracts/other.json");
    assert!(Profile::selected(&recipe).is_err());
    recipe["correspondence_schema_uri"] = json!(format!(
        "https://tree-of-sophia.local/{}",
        recipe["correspondence_contract_ref"].as_str().unwrap()
    ));
    recipe["output_files"][0]["filename"] = json!("../outside.json");
    assert!(Profile::selected(&recipe).is_err());
}

#[test]
fn selected_route_input_fixity_refuses_changed_map() {
    let root =
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
            .unwrap();
    let curation: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/edition-curation.v1.json")).unwrap();
    let mut recipe = curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["stage"] == "structural-source-routes")
        .unwrap()
        .clone();
    recipe["configurations"][0]["source_sha256"] = json!("0".repeat(64));
    let ctx = ResearchExecution::new(&root, 60).unwrap();
    let error = run(&ctx, &recipe, false, None, None).unwrap_err();
    assert!(error.contains("selected structural input fixity differs"));
}
