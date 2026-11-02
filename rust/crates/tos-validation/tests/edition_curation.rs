use std::{fs, path::Path};

use serde_json::{Value, json};
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    edition_curation::{
        EditionCurationError, SCHEMA_URI, SelectedCuration, SelectedDocument,
        select_historical_field, validate, validate_bindings, validate_recipe_plan,
        validate_recipe_template,
    },
};

fn edition_reference(curation: &str) -> String {
    format!(
        "{}/edition.json",
        curation.rsplit_once("/items/").unwrap().0
    )
}

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
    let dta_value: Value = serde_json::from_slice(&dta_raw).unwrap();
    let mut overlapping_speakers = dta_value.clone();
    let review = overlapping_speakers["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-review")
        .unwrap();
    let ranges = review["speaker_selection"]["readings"][0]["overrides"]
        .as_array_mut()
        .unwrap();
    ranges.insert(1, ranges[0].clone());
    assert_eq!(
        validate(&serde_json::to_vec(&overlapping_speakers).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("speaker overrides"))
    );
    let review = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "concept-review")
        .unwrap();
    let template_ref = review["historical_templates"][0]["ref"].as_str().unwrap();
    let template_raw = fs::read(root.join(template_ref)).unwrap();
    let template = SelectedDocument {
        reference: template_ref,
        raw: &template_raw,
    };
    assert_eq!(
        validate_recipe_template(
            &dta_raw,
            "concept-review",
            "legacy-output-template",
            template,
            &schema
        ),
        Ok(())
    );
    assert_eq!(
        validate_recipe_template(
            &dta_raw,
            "concept-review",
            "legacy-output-template",
            SelectedDocument {
                reference: template_ref,
                raw: b"changed"
            },
            &schema
        ),
        Err(EditionCurationError::DigestMismatch)
    );
    let mut invalid_correspondence = dta_value.clone();
    let correspondence = invalid_correspondence["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "witness-structure-correspondence")
        .unwrap();
    correspondence["parts"][0]["last_target_page"] = json!(529);
    assert_eq!(
        validate(
            &serde_json::to_vec(&invalid_correspondence).unwrap(),
            &schema
        ),
        Err(EditionCurationError::InvalidRange("parts"))
    );
    let foundation_recipe = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "tei-source-foundation")
        .unwrap();
    let foundation_plan_ref = foundation_recipe["plan_ref"].as_str().unwrap();
    let foundation_plan_raw = fs::read(root.join(foundation_plan_ref)).unwrap();
    let foundation_plan = SelectedDocument {
        reference: foundation_plan_ref,
        raw: &foundation_plan_raw,
    };
    assert_eq!(
        validate_recipe_plan(&dta_raw, "tei-source-foundation", foundation_plan, &schema),
        Ok(())
    );
    let bridge_recipe = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "authored-canon-evidence")
        .unwrap();
    let bridge_plan_ref = bridge_recipe["plan_ref"].as_str().unwrap();
    let bridge_plan_raw = fs::read(root.join(bridge_plan_ref)).unwrap();
    assert_eq!(
        validate_recipe_plan(
            &dta_raw,
            "authored-canon-evidence",
            SelectedDocument {
                reference: bridge_plan_ref,
                raw: &bridge_plan_raw
            },
            &schema
        ),
        Ok(())
    );
    let mut wrong_selector = dta_value.clone();
    let foundation = wrong_selector["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "tei-source-foundation")
        .unwrap();
    foundation["tei_selector"]["steps"][3]["ordinal"] = json!(2);
    assert_eq!(
        validate_recipe_plan(
            &serde_json::to_vec(&wrong_selector).unwrap(),
            "tei-source-foundation",
            foundation_plan,
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let historical_ref = foundation_recipe["frozen_outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|output| output["role"] == "text_layer_ref")
        .unwrap()["repository_path"]
        .as_str()
        .unwrap();
    let historical_raw = fs::read(root.join(historical_ref)).unwrap();
    let historical = SelectedDocument {
        reference: historical_ref,
        raw: &historical_raw,
    };
    let expected_historical: Value = serde_json::from_slice(&historical_raw).unwrap();
    assert_eq!(
        select_historical_field(
            &dta_raw,
            "tei-source-foundation",
            "layer-authority",
            historical,
            &schema
        ),
        Ok(expected_historical["authority_boundary"].clone())
    );
    let mut wrong_historical_schema = dta_value.clone();
    let foundation = wrong_historical_schema["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "tei-source-foundation")
        .unwrap();
    let output = foundation["frozen_outputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|output| output["role"] == "text_layer_ref")
        .unwrap();
    output["schema_version"] = json!("unexpected_version");
    assert_eq!(
        select_historical_field(
            &serde_json::to_vec(&wrong_historical_schema).unwrap(),
            "tei-source-foundation",
            "layer-authority",
            historical,
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    assert_eq!(
        select_historical_field(
            &dta_raw,
            "tei-source-foundation",
            "layer-authority",
            SelectedDocument {
                reference: historical_ref,
                raw: b"{}"
            },
            &schema
        ),
        Err(EditionCurationError::DigestMismatch)
    );
    assert_eq!(
        select_historical_field(
            &dta_raw,
            "tei-source-foundation",
            "layer-authority",
            SelectedDocument {
                reference: "ToS/wrong.json",
                raw: &historical_raw
            },
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut missing_historical_field = dta_value.clone();
    let foundation = missing_historical_field["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "tei-source-foundation")
        .unwrap();
    foundation["historical_fields"][0]["json_pointer"] = json!("/missing");
    assert_eq!(
        select_historical_field(
            &serde_json::to_vec(&missing_historical_field).unwrap(),
            "tei-source-foundation",
            "layer-authority",
            historical,
            &schema
        ),
        Err(EditionCurationError::MissingReference)
    );
    let mut out_of_order = dta_value.clone();
    let constructor = out_of_order["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "constructor-library")
        .unwrap();
    constructor["parts"].as_array_mut().unwrap().swap(0, 1);
    assert_eq!(
        validate(&serde_json::to_vec(&out_of_order).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("parts"))
    );
    let mut invalid_template = dta_value.clone();
    let constructor = invalid_template["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "constructor-library")
        .unwrap();
    constructor["presentation"]["part_description_template"]["ru"] = json!("{unknown}");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_template).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("presentation template"))
    );
    let mut missing_historical_output = dta_value.clone();
    let foundation = missing_historical_output["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "tei-source-foundation")
        .unwrap();
    foundation["historical_fields"][0]["output_role"] = json!("missing-output");
    assert_eq!(
        validate(
            &serde_json::to_vec(&missing_historical_output).unwrap(),
            &schema
        ),
        Err(EditionCurationError::MissingReference)
    );
    let plan_ref = dta_value["recipes"][0]["plan_ref"].as_str().unwrap();
    let plan_raw = fs::read(root.join(plan_ref)).unwrap();
    let plan = SelectedDocument {
        reference: plan_ref,
        raw: &plan_raw,
    };
    assert_eq!(
        validate_recipe_plan(&dta_raw, "tei-technical-markup", plan, &schema),
        Ok(())
    );
    assert_eq!(
        validate_recipe_plan(
            &dta_raw,
            "tei-technical-markup",
            SelectedDocument {
                reference: plan_ref,
                raw: b"changed"
            },
            &schema
        ),
        Err(EditionCurationError::DigestMismatch)
    );
    assert_eq!(
        validate_recipe_plan(
            &dta_raw,
            "tei-technical-markup",
            SelectedDocument {
                reference: "../plan.json",
                raw: &plan_raw
            },
            &schema
        ),
        Err(EditionCurationError::UnsafePath)
    );
    assert_eq!(
        validate_recipe_plan(&dta_raw, "unknown-stage", plan, &schema),
        Err(EditionCurationError::MissingReference)
    );
    let mut ambiguous_plan = dta_value.clone();
    let recipes = ambiguous_plan["recipes"].as_array_mut().unwrap();
    recipes.push(recipes[0].clone());
    assert_eq!(
        validate_recipe_plan(
            &serde_json::to_vec(&ambiguous_plan).unwrap(),
            "tei-technical-markup",
            plan,
            &schema
        ),
        Err(EditionCurationError::AmbiguousReference)
    );
    let lexical = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "lexical-index")
        .unwrap();
    let lexical_plan_ref = lexical["locators"]
        .as_array()
        .unwrap()
        .iter()
        .find(|locator| locator["role"] == "index_plan")
        .unwrap()["repository_path"]
        .as_str()
        .unwrap();
    let lexical_plan_raw = fs::read(root.join(lexical_plan_ref)).unwrap();
    assert_eq!(
        validate_recipe_plan(
            &dta_raw,
            "lexical-index",
            SelectedDocument {
                reference: lexical_plan_ref,
                raw: &lexical_plan_raw
            },
            &schema
        ),
        Ok(())
    );
    let mut duplicate_dta_output: Value = serde_json::from_slice(&dta_raw).unwrap();
    duplicate_dta_output["recipes"][0]["frozen_outputs"][1]["repository_path"] =
        duplicate_dta_output["recipes"][0]["frozen_outputs"][0]["repository_path"].clone();
    assert_eq!(
        validate(&serde_json::to_vec(&duplicate_dta_output).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("repository_path"))
    );
    let antonovsky_path = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/editions/saint-petersburg-prometey-1911-fourth/items/rsl-neb-scan-pdf/edition-curation.v1.json";
    let antonovsky_raw = fs::read(root.join(antonovsky_path)).unwrap();
    assert_eq!(validate(&antonovsky_raw, &schema), Ok(()));
    let mut reordered_legacy_outputs: Value = serde_json::from_slice(&antonovsky_raw).unwrap();
    reordered_legacy_outputs["recipes"][0]["legacy_layout"]["frozen_outputs"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert_eq!(
        validate(
            &serde_json::to_vec(&reordered_legacy_outputs).unwrap(),
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut inconsistent_provenance: Value = serde_json::from_slice(&antonovsky_raw).unwrap();
    inconsistent_provenance["recipes"][0]["legacy_layout"]["historical_provenance_sha256"] =
        json!("0".repeat(64));
    assert_eq!(
        validate(
            &serde_json::to_vec(&inconsistent_provenance).unwrap(),
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let collation: Value = serde_json::from_slice(&antonovsky_raw).unwrap();
    let mut swapped_witnesses = collation.clone();
    swapped_witnesses["recipes"][1]["witness_roles"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert_eq!(
        validate(&serde_json::to_vec(&swapped_witnesses).unwrap(), &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let collation_plan_ref = collation["recipes"][1]["plan_ref"].as_str().unwrap();
    let collation_plan_raw = fs::read(root.join(collation_plan_ref)).unwrap();
    let mut wrong_role_key = collation.clone();
    wrong_role_key["recipes"][1]["witness_roles"][0]["plan_key"] = json!("missing_witness");
    assert_eq!(
        validate_recipe_plan(
            &serde_json::to_vec(&wrong_role_key).unwrap(),
            "text-collation",
            SelectedDocument {
                reference: collation_plan_ref,
                raw: &collation_plan_raw
            },
            &schema,
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    assert_eq!(
        validate_recipe_plan(
            &antonovsky_raw,
            "text-collation",
            SelectedDocument {
                reference: collation_plan_ref,
                raw: &collation_plan_raw
            },
            &schema
        ),
        Ok(())
    );
    let mut duplicate_region: Value = serde_json::from_slice(&antonovsky_raw).unwrap();
    let regions = duplicate_region["recipes"][0]["layout_rules"]["regions"]
        .as_array_mut()
        .unwrap();
    regions.push(regions[0].clone());
    assert_eq!(
        validate(&serde_json::to_vec(&duplicate_region).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("page region"))
    );
    let mut duplicate_output: Value = serde_json::from_slice(&antonovsky_raw).unwrap();
    duplicate_output["recipes"][0]["outputs"][1]["filename"] =
        duplicate_output["recipes"][0]["outputs"][0]["filename"].clone();
    assert_eq!(
        validate(&serde_json::to_vec(&duplicate_output).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("filename"))
    );
    let mut missing_provenance: Value = serde_json::from_slice(&antonovsky_raw).unwrap();
    missing_provenance["recipes"][0]["legacy_layout"]
        .as_object_mut()
        .unwrap()
        .remove("historical_provenance_sha256");
    assert_eq!(
        validate(&serde_json::to_vec(&missing_provenance).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
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
    let part2_path = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-2/editions/chemnitz-schmeitzner-1883-part-2/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json";
    let manifest_path = part2_path
        .strip_suffix("edition-curation.v1.json")
        .unwrap()
        .to_owned()
        + "item.manifest.json";
    let part2 = fs::read(root.join(part2_path)).unwrap();
    let part2_manifest = fs::read(root.join(&manifest_path)).unwrap();
    let current = SelectedDocument {
        reference: part2_path,
        raw: &part2,
    };
    let manifest = SelectedDocument {
        reference: &manifest_path,
        raw: &part2_manifest,
    };
    let edition_path = edition_reference(part2_path);
    let edition_raw = fs::read(root.join(&edition_path)).unwrap();
    let edition = SelectedDocument {
        reference: &edition_path,
        raw: &edition_raw,
    };
    let owner_manifest_path = dta_path
        .strip_suffix("edition-curation.v1.json")
        .unwrap()
        .to_owned()
        + "item.manifest.json";
    let owner_manifest_raw = fs::read(root.join(&owner_manifest_path)).unwrap();
    let owner_edition_path = edition_reference(dta_path);
    let owner_edition_raw = fs::read(root.join(&owner_edition_path)).unwrap();
    let owner = SelectedCuration {
        curation: SelectedDocument {
            reference: dta_path,
            raw: &dta_raw,
        },
        manifest: SelectedDocument {
            reference: &owner_manifest_path,
            raw: &owner_manifest_raw,
        },
        edition: SelectedDocument {
            reference: &owner_edition_path,
            raw: &owner_edition_raw,
        },
    };
    assert_eq!(
        validate_bindings(current, manifest, edition, &[owner], &schema),
        Ok(())
    );
    assert_eq!(
        validate_bindings(current, manifest, edition, &[], &schema),
        Err(EditionCurationError::MissingReference)
    );
    assert_eq!(
        validate_bindings(current, manifest, edition, &[owner, owner], &schema),
        Err(EditionCurationError::Duplicate("curation reference"))
    );
    let wrong_manifest = SelectedDocument {
        reference: &manifest_path,
        raw: &raw,
    };
    assert_eq!(
        validate_bindings(current, wrong_manifest, edition, &[owner], &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let wrong_edition = SelectedDocument {
        reference: &edition_path,
        raw: &owner_edition_raw,
    };
    assert_eq!(
        validate_bindings(current, manifest, wrong_edition, &[owner], &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let other_name = part2_path.replace("edition-curation.v1.json", "other.json");
    assert_eq!(
        validate_bindings(
            SelectedDocument {
                reference: &other_name,
                raw: &part2
            },
            manifest,
            edition,
            &[owner],
            &schema,
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut duplicate_owner: Value = serde_json::from_slice(&dta_raw).unwrap();
    let recipes = duplicate_owner["recipes"].as_array_mut().unwrap();
    recipes.push(recipes[0].clone());
    let duplicate_bytes = serde_json::to_vec(&duplicate_owner).unwrap();
    let duplicate_recipe_owner = SelectedCuration {
        curation: SelectedDocument {
            reference: dta_path,
            raw: &duplicate_bytes,
        },
        manifest: owner.manifest,
        edition: owner.edition,
    };
    assert_eq!(
        validate_bindings(
            current,
            manifest,
            edition,
            &[duplicate_recipe_owner],
            &schema
        ),
        Err(EditionCurationError::AmbiguousReference)
    );
    let mut unrelated_owner: Value = serde_json::from_slice(&dta_raw).unwrap();
    unrelated_owner["recipes"][0]["source_item_refs"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    let unrelated_bytes = serde_json::to_vec(&unrelated_owner).unwrap();
    let unrelated = SelectedCuration {
        curation: SelectedDocument {
            reference: dta_path,
            raw: &unrelated_bytes,
        },
        manifest: owner.manifest,
        edition: owner.edition,
    };
    assert_eq!(
        validate_bindings(current, manifest, edition, &[unrelated], &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let invalid_owner = SelectedCuration {
        curation: owner.curation,
        manifest: SelectedDocument {
            reference: &owner_manifest_path,
            raw: &part2_manifest,
        },
        edition: owner.edition,
    };
    assert_eq!(
        validate_bindings(current, manifest, edition, &[invalid_owner], &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let unsafe_manifest = SelectedDocument {
        reference: "../item.manifest.json",
        raw: &part2_manifest,
    };
    assert_eq!(
        validate_bindings(current, unsafe_manifest, edition, &[owner], &schema),
        Err(EditionCurationError::UnsafePath)
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
    for path in [
        "../outside",
        "a/./b",
        "a//b",
        "a/.git/config",
        "a/b/",
        "a\\b",
        "a/\nb",
    ] {
        let mut invalid = original.clone();
        invalid["recipes"][0]["locators"][0]
            .as_object_mut()
            .unwrap()
            .remove("item_root");
        invalid["recipes"][0]["locators"][0]["item_relative_path"] = json!(path);
        assert_eq!(
            validate(&serde_json::to_vec(&invalid).unwrap(), &schema),
            Err(EditionCurationError::InvalidSchema),
            "path {path:?}"
        );
    }
}

#[test]
fn authored_edition_curations_bind_to_their_selected_items() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let schema = SchemaBackendProbe::new(
        [SchemaResource {
            uri: SCHEMA_URI.into(),
            raw: fs::read(root.join("ToS/contracts/edition-curation.v1.schema.json")).unwrap(),
        }],
        FormatProfile::AssertedSourceCandidateV1,
    )
    .unwrap();
    let curations = [
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml/edition-curation.v1.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/edition-curation.v1.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1891-part-4/editions/leipzig-naumann-1891-part-4/items/dta-sub-goettingen-corrected-tei-p5/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1893/editions/leipzig-c-g-naumann-1893/items/internet-archive-cornell-auto-epub/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1893/editions/leipzig-c-g-naumann-1893/items/internet-archive-image-container-pdf/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-2/editions/chemnitz-schmeitzner-1883-part-2/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/editions/saint-petersburg-prometey-1911-fourth/items/rsl-neb-scan-pdf/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-cultural-revolution/editions/moscow-cultural-revolution-2007-volume-4/items/operator-pdf/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/edition-curation.v1.json",
    ];
    for curation in curations {
        let path = root.join(curation);
        let reference = path.strip_prefix(&root).unwrap().to_str().unwrap();
        let raw = fs::read(&path).unwrap();
        let value: Value = serde_json::from_slice(&raw).unwrap();
        let manifest_ref = format!(
            "{}/{}",
            reference.rsplit_once('/').unwrap().0,
            value["manifest"].as_str().unwrap()
        );
        let manifest_raw = fs::read(root.join(&manifest_ref)).unwrap();
        let edition_ref = edition_reference(reference);
        let edition_raw = fs::read(root.join(&edition_ref)).unwrap();
        let mut references = std::collections::BTreeMap::new();
        for recipe in value["recipes"].as_array().unwrap() {
            if recipe["stage"] == "recipe-reference" {
                let target = recipe["curation_ref"].as_str().unwrap();
                let target_raw = fs::read(root.join(target)).unwrap();
                let target_value: Value = serde_json::from_slice(&target_raw).unwrap();
                let target_manifest = format!(
                    "{}/{}",
                    target.rsplit_once('/').unwrap().0,
                    target_value["manifest"].as_str().unwrap()
                );
                let target_manifest_raw = fs::read(root.join(&target_manifest)).unwrap();
                let target_edition = edition_reference(target);
                let target_edition_raw = fs::read(root.join(&target_edition)).unwrap();
                references.insert(
                    target.to_owned(),
                    (
                        target_raw,
                        target_manifest,
                        target_manifest_raw,
                        target_edition,
                        target_edition_raw,
                    ),
                );
            }
        }
        let selected: Vec<_> = references
            .iter()
            .map(
                |(reference, (raw, manifest, manifest_raw, edition, edition_raw))| {
                    SelectedCuration {
                        curation: SelectedDocument { reference, raw },
                        manifest: SelectedDocument {
                            reference: manifest,
                            raw: manifest_raw,
                        },
                        edition: SelectedDocument {
                            reference: edition,
                            raw: edition_raw,
                        },
                    }
                },
            )
            .collect();
        let result = validate_bindings(
            SelectedDocument {
                reference,
                raw: &raw,
            },
            SelectedDocument {
                reference: &manifest_ref,
                raw: &manifest_raw,
            },
            SelectedDocument {
                reference: &edition_ref,
                raw: &edition_raw,
            },
            &selected,
            &schema,
        );
        assert_eq!(result, Ok(()), "curation {reference}");
    }
}
