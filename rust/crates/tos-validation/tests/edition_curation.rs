use std::{fs, path::Path};

use serde_json::{Value, json};
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    edition_curation::{
        EditionCurationError, SCHEMA_URI, SelectedCuration, SelectedDocument, validate,
        validate_bindings,
    },
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
    let antonovsky_path = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/editions/saint-petersburg-prometey-1911-fourth/items/rsl-neb-scan-pdf/edition-curation.v1.json";
    let antonovsky_raw = fs::read(root.join(antonovsky_path)).unwrap();
    assert_eq!(validate(&antonovsky_raw, &schema), Ok(()));
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
    let owner_manifest_path = dta_path
        .strip_suffix("edition-curation.v1.json")
        .unwrap()
        .to_owned()
        + "item.manifest.json";
    let owner_manifest_raw = fs::read(root.join(&owner_manifest_path)).unwrap();
    let owner = SelectedCuration {
        curation: SelectedDocument {
            reference: dta_path,
            raw: &dta_raw,
        },
        manifest: SelectedDocument {
            reference: &owner_manifest_path,
            raw: &owner_manifest_raw,
        },
    };
    assert_eq!(
        validate_bindings(current, manifest, &[owner], &schema),
        Ok(())
    );
    assert_eq!(
        validate_bindings(current, manifest, &[], &schema),
        Err(EditionCurationError::MissingReference)
    );
    assert_eq!(
        validate_bindings(current, manifest, &[owner, owner], &schema),
        Err(EditionCurationError::Duplicate("curation reference"))
    );
    let wrong_manifest = SelectedDocument {
        reference: &manifest_path,
        raw: &raw,
    };
    assert_eq!(
        validate_bindings(current, wrong_manifest, &[owner], &schema),
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
    };
    assert_eq!(
        validate_bindings(current, manifest, &[duplicate_recipe_owner], &schema),
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
    };
    assert_eq!(
        validate_bindings(current, manifest, &[unrelated], &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let invalid_owner = SelectedCuration {
        curation: owner.curation,
        manifest: SelectedDocument {
            reference: &owner_manifest_path,
            raw: &part2_manifest,
        },
    };
    assert_eq!(
        validate_bindings(current, manifest, &[invalid_owner], &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let unsafe_manifest = SelectedDocument {
        reference: "../item.manifest.json",
        raw: &part2_manifest,
    };
    assert_eq!(
        validate_bindings(current, unsafe_manifest, &[owner], &schema),
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
    let mut invalid = original;
    invalid["recipes"][0]["locators"][0]["item_relative_path"] = json!("../outside");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
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
    let mut dirs = vec![root.join("ToS/source-witnesses")];
    let mut curations = Vec::new();
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let ty = entry.file_type().unwrap();
            if ty.is_dir() {
                dirs.push(entry.path());
            } else if ty.is_file() && entry.file_name() == "edition-curation.v1.json" {
                curations.push(entry.path());
            }
        }
    }
    assert!(!curations.is_empty());
    for path in curations {
        let reference = path.strip_prefix(&root).unwrap().to_str().unwrap();
        let raw = fs::read(&path).unwrap();
        let value: Value = serde_json::from_slice(&raw).unwrap();
        let manifest_ref = format!(
            "{}/{}",
            reference.rsplit_once('/').unwrap().0,
            value["manifest"].as_str().unwrap()
        );
        let manifest_raw = fs::read(root.join(&manifest_ref)).unwrap();
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
                references.insert(
                    target.to_owned(),
                    (target_raw, target_manifest, target_manifest_raw),
                );
            }
        }
        let selected: Vec<_> = references
            .iter()
            .map(
                |(reference, (raw, manifest, manifest_raw))| SelectedCuration {
                    curation: SelectedDocument { reference, raw },
                    manifest: SelectedDocument {
                        reference: manifest,
                        raw: manifest_raw,
                    },
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
            &selected,
            &schema,
        );
        assert_eq!(result, Ok(()), "curation {reference}");
    }
}
