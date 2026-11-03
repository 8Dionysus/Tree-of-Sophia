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
