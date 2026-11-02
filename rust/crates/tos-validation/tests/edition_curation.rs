use std::{fs, path::Path};

use serde_json::{Value, json};
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    edition_curation::{EditionCurationError, SCHEMA_URI, validate},
};

#[test]
fn edition_curation_checks_schema_and_cross_field_rules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let schema = SchemaBackendProbe::new(
        [SchemaResource {
            uri: SCHEMA_URI.into(),
            raw: fs::read(root.join("ToS/contracts/edition-curation.v1.schema.json")).unwrap(),
        }],
        FormatProfile::AssertedSourceCandidateV1,
    )
    .unwrap();
    let path = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/edition-curation.v1.json";
    let raw = fs::read(root.join(path)).unwrap();
    assert_eq!(validate(&raw, &schema), Ok(()));
    let original: Value = serde_json::from_slice(&raw).unwrap();
    let mut invalid = original.clone();
    invalid["owner_item_ref"] = json!("not-an-id");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
    let mut invalid = original.clone();
    let overrides = invalid["recipes"][0]["page_overrides"]
        .as_array_mut()
        .unwrap();
    overrides.push(overrides[0].clone());
    assert_eq!(
        validate(&serde_json::to_vec(&invalid).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("key"))
    );
    let mut unordered = original.clone();
    unordered["recipes"][0]["page_overrides"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert_eq!(
        validate(&serde_json::to_vec(&unordered).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("page_overrides"))
    );
    unordered["recipes"][0]["page_order"] = json!("unconstrained");
    assert_eq!(
        validate(&serde_json::to_vec(&unordered).unwrap(), &schema),
        Ok(())
    );
    let mut duplicate_marker = original.clone();
    let markers = duplicate_marker["recipes"][1]["marker_returns"]
        .as_array_mut()
        .unwrap();
    let mut repeated = markers[0].clone();
    repeated["key"][1] = serde_json::from_str("52.0").unwrap();
    markers.push(repeated);
    assert_eq!(
        validate(&serde_json::to_vec(&duplicate_marker).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("marker key"))
    );
    let mut invalid_box = original.clone();
    invalid_box["recipes"][1]["marker_returns"][0]["line_bbox"][0] = json!(999999);
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_box).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("line_bbox"))
    );
    let dta_path = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json";
    let dta_raw = fs::read(root.join(dta_path)).unwrap();
    assert_eq!(validate(&dta_raw, &schema), Ok(()));
    let mut invalid_cues: Value = serde_json::from_slice(&dta_raw).unwrap();
    invalid_cues["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["stage"] == "reading-workbench")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("cue_matching");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_cues).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
    let target_path = "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/edition-curation.v1.json";
    let target_raw = fs::read(root.join(target_path)).unwrap();
    assert_eq!(validate(&target_raw, &schema), Ok(()));
    let mut target: Value = serde_json::from_slice(&target_raw).unwrap();
    target["recipes"][1]["candidate_geometry"]["x_min"] = json!(999999);
    assert_eq!(
        validate(&serde_json::to_vec(&target).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("x_min"))
    );
    let hierarchical_path = "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/edition-curation.v1.json";
    let hierarchical_raw = fs::read(root.join(hierarchical_path)).unwrap();
    assert_eq!(validate(&hierarchical_raw, &schema), Ok(()));
    let mut invalid_identity: Value = serde_json::from_slice(&hierarchical_raw).unwrap();
    let hierarchical = invalid_identity["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["stage"] == "hierarchical-source-structure")
        .unwrap();
    hierarchical["identity"]["anchor_prefix"] = json!("not-an-anchor");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_identity).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
    let mut invalid_counts = original.clone();
    let labels = invalid_counts["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["stage"] == "numbered-label-correspondence")
        .unwrap();
    labels["expected_shared_count"] = json!(297);
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_counts).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("expected_source_count"))
    );
    let mut invalid = original;
    invalid["recipes"][0]["locators"][0]["item_relative_path"] = json!("../outside");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
}
