use std::{fs, path::Path};

use serde_json::{Value, json};
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    edition_curation::{
        EditionCurationError, SCHEMA_URI, SelectedCuration, SelectedDocument,
        select_frozen_manifest, select_historical_field, select_recipe_input_digest,
        select_retained_digest, validate, validate_bindings, validate_default_request,
        validate_frozen_manifest_member, validate_frozen_output_fixity, validate_recipe_plan,
        validate_recipe_template, validate_retained_artifact, validate_review_sources,
        validate_selected_historical_receipt,
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
    let mut duplicate_source_layer = original.clone();
    let source = duplicate_source_layer["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "source-passages")
        .unwrap();
    let overrides = source["work_routes"][0]["overrides"]
        .as_array_mut()
        .unwrap();
    overrides.push(overrides[0].clone());
    assert_eq!(
        validate(
            &serde_json::to_vec(&duplicate_source_layer).unwrap(),
            &schema
        ),
        Err(EditionCurationError::Duplicate("source layer key"))
    );
    let mut wrong_witness_role = original.clone();
    let source = wrong_witness_role["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "source-passages")
        .unwrap();
    source["work_routes"][0]["address_witness"]["item_role"] = json!("missing-item");
    assert_eq!(
        validate(&serde_json::to_vec(&wrong_witness_role).unwrap(), &schema),
        Err(EditionCurationError::MissingReference)
    );
    let mut wrong_marker = original.clone();
    let source = wrong_marker["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "source-passages")
        .unwrap();
    source["work_routes"][0]["marker_curation_ref"] = json!("missing-curation.json");
    assert_eq!(
        validate(&serde_json::to_vec(&wrong_marker).unwrap(), &schema),
        Err(EditionCurationError::MissingReference)
    );
    let concept_path = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json";
    let concept_raw = fs::read(root.join(concept_path)).unwrap();
    let coverage_ref = "ToS/candidate-intake/zarathustra/concept-workbench-v1/outputs/fate/coverage-receipt.v1.json";
    let coverage_raw = fs::read(root.join(coverage_ref)).unwrap();
    let probe = select_historical_field(
        &concept_raw,
        "concept-workbench",
        "default_acceptance_probe",
        SelectedDocument {
            reference: coverage_ref,
            raw: &coverage_raw,
        },
        &schema,
    )
    .unwrap();
    assert_eq!(probe["de_direct_occurrences"], 26);
    let mut bad_scope: Value =
        serde_json::from_slice(&fs::read(root.join(concept_path)).unwrap()).unwrap();
    let concept = bad_scope["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    concept["legacy_work_scope"]["expected_census"]["de_work_scope_occurrences"] = json!(999999);
    assert_eq!(
        validate(&serde_json::to_vec(&bad_scope).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("expected_census"))
    );
    let reading_raw = fs::read(root.join(concept_path)).unwrap();
    let manifest_ref = "ToS/candidate-intake/zarathustra/concept-workbench-v1/manifest.v1.json";
    let manifest_raw = fs::read(root.join(manifest_ref)).unwrap();
    let digest = select_recipe_input_digest(
        &reading_raw,
        "reading-workbench",
        "concept_workbench",
        SelectedDocument {
            reference: manifest_ref,
            raw: &manifest_raw,
        },
        &schema,
    )
    .unwrap();
    assert_eq!(
        digest,
        "2d072c2b7b07711b14426c86929ec67b807c8de7aea2b39e4cbdfb052bdf3332"
    );
    assert_eq!(
        select_recipe_input_digest(
            &reading_raw,
            "reading-workbench",
            "concept_workbench",
            SelectedDocument {
                reference: "other.json",
                raw: &manifest_raw
            },
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut bad_threshold: Value = serde_json::from_slice(&reading_raw).unwrap();
    let parallel = bad_threshold["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "parallel-lexical-candidates")
        .unwrap();
    let parallel_plan_ref = parallel["plan_ref"].as_str().unwrap().to_owned();
    parallel["association_classification"]["proposed"]["minimum_support"] = json!("/missing");
    let parallel_plan_raw = fs::read(root.join(&parallel_plan_ref)).unwrap();
    assert_eq!(
        validate_recipe_plan(
            &serde_json::to_vec(&bad_threshold).unwrap(),
            "parallel-lexical-candidates",
            SelectedDocument {
                reference: &parallel_plan_ref,
                raw: &parallel_plan_raw
            },
            &schema
        ),
        Err(EditionCurationError::MissingReference)
    );
    let mut bad_reading: Value = serde_json::from_slice(&reading_raw).unwrap();
    let alignment = bad_reading["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "paragraph-alignment")
        .unwrap();
    alignment["cross_kind_risks"][0]["reading_ordinal"] = json!(99);
    assert_eq!(
        validate(&serde_json::to_vec(&bad_reading).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("cross_kind_risks"))
    );
    let visual_path = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-cultural-revolution/editions/moscow-cultural-revolution-2007-volume-4/items/operator-pdf/edition-curation.v1.json";
    let visual_raw = fs::read(root.join(visual_path)).unwrap();
    let visual: Value = serde_json::from_slice(&visual_raw).unwrap();
    let visual_recipe = visual["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "visual-retrieval-result")
        .unwrap();
    let receipt_ref = visual_recipe["receipt_ref"].as_str().unwrap();
    let receipt_raw = fs::read(root.join(receipt_ref)).unwrap();
    assert_eq!(
        validate_selected_historical_receipt(
            &visual_raw,
            "visual-retrieval-result",
            SelectedDocument {
                reference: receipt_ref,
                raw: &receipt_raw
            },
            &schema
        ),
        Ok(())
    );
    assert!(
        validate_selected_historical_receipt(
            &visual_raw,
            "visual-retrieval-result",
            SelectedDocument {
                reference: receipt_ref,
                raw: b"{}"
            },
            &schema
        )
        .is_err()
    );
    let target_path = "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/edition-curation.v1.json";
    let mut duplicate_target: Value =
        serde_json::from_slice(&fs::read(root.join(target_path)).unwrap()).unwrap();
    let target = duplicate_target["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "target-passages")
        .unwrap();
    target["work_mappings"][1]["candidate_token"] =
        target["work_mappings"][0]["candidate_token"].clone();
    assert_eq!(
        validate(&serde_json::to_vec(&duplicate_target).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("candidate_token"))
    );
    let review = original["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "source-review-receipt")
        .unwrap();
    let receipt_ref = review["frozen_outputs"][0]["repository_path"]
        .as_str()
        .unwrap();
    let receipt_raw = fs::read(root.join(receipt_ref)).unwrap();
    assert_eq!(
        validate_frozen_output_fixity(
            &raw,
            "source-review-receipt",
            "receipt",
            SelectedDocument {
                reference: receipt_ref,
                raw: &receipt_raw,
            },
            &schema,
        ),
        Ok(())
    );
    let mut wrong_transfer_output = original.clone();
    let transfer = wrong_transfer_output["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "source-passages")
        .unwrap();
    let locator = transfer["locators"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|locator| locator["role"] == "candidates")
        .unwrap();
    locator["repository_path"] = json!("different.json");
    assert_eq!(
        validate(
            &serde_json::to_vec(&wrong_transfer_output).unwrap(),
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
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
    let recurrence = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "lexical-recurrence")
        .unwrap();
    let recurrence_plan_ref = recurrence["plan_ref"].as_str().unwrap();
    let recurrence_plan_raw = fs::read(root.join(recurrence_plan_ref)).unwrap();
    let recurrence_plan: Value = serde_json::from_slice(&recurrence_plan_raw).unwrap();
    let event_entry = recurrence["retained_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["role"] == "event")
        .unwrap();
    let event_ref = recurrence_plan
        .pointer(event_entry["plan_pointer"].as_str().unwrap())
        .unwrap()
        .as_str()
        .unwrap();
    let event_raw = fs::read(root.join(event_ref)).unwrap();
    assert_eq!(
        validate_retained_artifact(
            &dta_raw,
            "lexical-recurrence",
            "event",
            SelectedDocument {
                reference: recurrence_plan_ref,
                raw: &recurrence_plan_raw,
            },
            SelectedDocument {
                reference: event_ref,
                raw: &event_raw,
            },
            &schema,
        ),
        Ok(())
    );
    let context_result = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "morphology-context-result")
        .unwrap();
    let context_plan_ref = context_result["plan_ref"].as_str().unwrap();
    let context_plan_raw = fs::read(root.join(context_plan_ref)).unwrap();
    let selected_context_plan = SelectedDocument {
        reference: context_plan_ref,
        raw: &context_plan_raw,
    };
    let form_digest = context_result["retained_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["role"] == "form")
        .unwrap()["sha256"]
        .as_str()
        .unwrap();
    assert_eq!(
        select_retained_digest(
            &dta_raw,
            "morphology-context-result",
            "form",
            selected_context_plan,
            None,
            &schema,
        ),
        Ok(form_digest.to_owned())
    );
    let previous_ref = context_result["retained_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["role"] == "previous-receipt")
        .unwrap()["repository_path"]
        .as_str()
        .unwrap();
    let previous_raw = fs::read(root.join(previous_ref)).unwrap();
    let runner_digest = context_result["retained_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["role"] == "runner")
        .unwrap()["sha256"]
        .as_str()
        .unwrap();
    assert_eq!(
        select_retained_digest(
            &dta_raw,
            "morphology-context-result",
            "runner",
            selected_context_plan,
            Some(SelectedDocument {
                reference: previous_ref,
                raw: &previous_raw,
            }),
            &schema,
        ),
        Ok(runner_digest.to_owned())
    );
    assert_eq!(
        validate_retained_artifact(
            &dta_raw,
            "morphology-context-result",
            "form",
            SelectedDocument {
                reference: context_plan_ref,
                raw: &context_plan_raw,
            },
            SelectedDocument {
                reference: event_ref,
                raw: &event_raw,
            },
            &schema,
        ),
        Err(EditionCurationError::MissingReference)
    );
    let mut invalid_comparison_output = dta_value.clone();
    let comparison = invalid_comparison_output["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "witness-triangulation")
        .unwrap();
    comparison["output_naming"]["initial_generation"] = json!("v2");
    assert_eq!(
        validate(
            &serde_json::to_vec(&invalid_comparison_output).unwrap(),
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let alignment = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "sentence-alignment")
        .unwrap();
    let alignment_plan_ref = alignment["plan_ref"].as_str().unwrap();
    let alignment_plan_raw = fs::read(root.join(alignment_plan_ref)).unwrap();
    let alignment_plan = SelectedDocument {
        reference: alignment_plan_ref,
        raw: &alignment_plan_raw,
    };
    assert_eq!(
        validate_recipe_plan(&dta_raw, "sentence-alignment", alignment_plan, &schema),
        Ok(())
    );
    let mut wrong_alignment_output = dta_value.clone();
    let alignment = wrong_alignment_output["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "sentence-alignment")
        .unwrap();
    alignment["frozen_outputs"][0]["repository_path"] = json!("different.json");
    assert_eq!(
        validate_recipe_plan(
            &serde_json::to_vec(&wrong_alignment_output).unwrap(),
            "sentence-alignment",
            alignment_plan,
            &schema,
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    for stage in [
        "paragraph-alignment",
        "parallel-lexical-candidates",
        "morphology-theme-candidates",
    ] {
        let recipe = dta_value["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|recipe| recipe["stage"] == stage)
            .unwrap();
        let reference = recipe["frozen_manifest"]["ref"].as_str().unwrap();
        let manifest_raw = fs::read(root.join(reference)).unwrap();
        let manifest = SelectedDocument {
            reference,
            raw: &manifest_raw,
        };
        let selected = select_frozen_manifest(&dta_raw, stage, manifest, &schema).unwrap();
        if stage == "paragraph-alignment" {
            let mut large_raw = manifest_raw.clone();
            large_raw.resize(SchemaBackendProbe::MAX_INSTANCE_BYTES + 1, b' ');
            let mut larger_curation = dta_value.clone();
            let recipe = larger_curation["recipes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|recipe| recipe["stage"] == stage)
                .unwrap();
            recipe["frozen_manifest"]["sha256"] =
                json!(tos_foundation::Digest256::of_bytes(&large_raw).to_hex());
            assert!(
                select_frozen_manifest(
                    &serde_json::to_vec(&larger_curation).unwrap(),
                    stage,
                    SelectedDocument {
                        reference,
                        raw: &large_raw,
                    },
                    &schema,
                )
                .is_ok()
            );
        }
        let member_ref = selected["generated_outputs"]
            .as_object()
            .unwrap()
            .keys()
            .find(|path| path.ends_with("/summary.v1.json"))
            .unwrap();
        let mut member_raw = fs::read(root.join(member_ref)).unwrap();
        assert_eq!(
            validate_frozen_manifest_member(
                &dta_raw,
                stage,
                manifest,
                SelectedDocument {
                    reference: member_ref,
                    raw: &member_raw,
                },
                &schema,
            ),
            Ok(())
        );
        member_raw[0] ^= 1;
        assert_eq!(
            validate_frozen_manifest_member(
                &dta_raw,
                stage,
                manifest,
                SelectedDocument {
                    reference: member_ref,
                    raw: &member_raw,
                },
                &schema,
            ),
            Err(EditionCurationError::DigestMismatch)
        );
    }
    let mut wrong_review_output = dta_value.clone();
    let review = wrong_review_output["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-review")
        .unwrap();
    review["output_subdirectory"] = json!("different");
    assert_eq!(
        validate(&serde_json::to_vec(&wrong_review_output).unwrap(), &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut invalid_output_scope = dta_value.clone();
    let workbench = invalid_output_scope["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    workbench["output_layout"]["root_roles"][0] = json!("unknown");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_output_scope).unwrap(), &schema),
        Err(EditionCurationError::MissingReference)
    );
    let mut unsafe_output_scope = dta_value.clone();
    let workbench = unsafe_output_scope["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    workbench["output_layout"]["default_scope"] = json!("../other");
    assert_eq!(
        validate(&serde_json::to_vec(&unsafe_output_scope).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
    let mut invalid_identity_method = dta_value.clone();
    let reading = invalid_identity_method["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "reading-workbench")
        .unwrap();
    reading["identity_profiles"][0]["method_field"] = json!("formula_method_id");
    assert_eq!(
        validate(
            &serde_json::to_vec(&invalid_identity_method).unwrap(),
            &schema
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut invalid_voice_order = dta_value.clone();
    let reading = invalid_voice_order["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "reading-workbench")
        .unwrap();
    reading["voice_search"]["selection_order"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_voice_order).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
    let review = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "concept-review")
        .unwrap();
    let review_plan_ref = review["plan_ref"].as_str().unwrap();
    let review_template_ref = review["historical_templates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["role"] == "legacy-output-template")
        .unwrap()["ref"]
        .as_str()
        .unwrap();
    let review_plan_raw = fs::read(root.join(review_plan_ref)).unwrap();
    let review_template_raw = fs::read(root.join(review_template_ref)).unwrap();
    let selected_plan = SelectedDocument {
        reference: review_plan_ref,
        raw: &review_plan_raw,
    };
    let selected_template = SelectedDocument {
        reference: review_template_ref,
        raw: &review_template_raw,
    };
    assert_eq!(
        validate_review_sources(&dta_raw, selected_plan, selected_template, &schema),
        Ok(())
    );
    let mut reordered_review = dta_value.clone();
    let review = reordered_review["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-review")
        .unwrap();
    review["speaker_selection"]["readings"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert_eq!(
        validate_review_sources(
            &serde_json::to_vec(&reordered_review).unwrap(),
            selected_plan,
            selected_template,
            &schema,
        ),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut bad_ratio = dta_value.clone();
    let morphology = bad_ratio["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "morphology-theme-candidates")
        .unwrap();
    morphology["case_settings"]["lower_ratio"] = json!(0.9);
    assert_eq!(
        validate(&serde_json::to_vec(&bad_ratio).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange("case ratios"))
    );
    let mut missing_suffix_table = dta_value.clone();
    let morphology = missing_suffix_table["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "morphology-theme-candidates")
        .unwrap();
    morphology["signature_settings"]["languages"][0]["table_key"] = json!("missing");
    let reference = morphology["historical_template_ref"]
        .as_str()
        .unwrap()
        .to_owned();
    let template_raw = fs::read(root.join(&reference)).unwrap();
    assert_eq!(
        validate_recipe_template(
            &serde_json::to_vec(&missing_suffix_table).unwrap(),
            "morphology-theme-candidates",
            "historical-template",
            SelectedDocument {
                reference: &reference,
                raw: &template_raw,
            },
            &schema,
        ),
        Err(EditionCurationError::MissingReference)
    );
    for stage in [
        "paragraph-alignment",
        "morphology-theme-candidates",
        "witness-triangulation",
        "bounded-translation-input",
    ] {
        let recipe = dta_value["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|recipe| recipe["stage"] == stage)
            .unwrap();
        let reference = recipe["historical_template_ref"].as_str().unwrap();
        let raw = fs::read(root.join(reference)).unwrap();
        assert_eq!(
            validate_recipe_template(
                &dta_raw,
                stage,
                "historical-template",
                SelectedDocument {
                    reference,
                    raw: &raw,
                },
                &schema,
            ),
            Ok(())
        );
        assert_eq!(
            validate_recipe_template(
                &dta_raw,
                stage,
                "historical-template",
                SelectedDocument {
                    reference,
                    raw: b"modified",
                },
                &schema,
            ),
            Err(EditionCurationError::DigestMismatch)
        );
    }
    let workbench = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    let frozen_context = workbench["frozen_outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|output| output["role"] == "contexts")
        .unwrap()["repository_path"]
        .as_str()
        .unwrap();
    let request_ref = workbench["output_layout"]["default_request_ref"]
        .as_str()
        .unwrap();
    let request_raw = fs::read(root.join(request_ref)).unwrap();
    assert_eq!(
        validate_default_request(
            &dta_raw,
            "concept-workbench",
            SelectedDocument {
                reference: request_ref,
                raw: &request_raw,
            },
            &schema,
        ),
        Ok(())
    );
    let context_raw = fs::read(root.join(frozen_context)).unwrap();
    assert!(context_raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES);
    assert_eq!(
        validate_frozen_output_fixity(
            &dta_raw,
            "concept-workbench",
            "contexts",
            SelectedDocument {
                reference: frozen_context,
                raw: &context_raw,
            },
            &schema,
        ),
        Ok(())
    );
    assert_eq!(
        validate_frozen_output_fixity(
            &dta_raw,
            "concept-workbench",
            "contexts",
            SelectedDocument {
                reference: frozen_context,
                raw: b"modified",
            },
            &schema,
        ),
        Err(EditionCurationError::DigestMismatch)
    );
    let issuance_ref = workbench["historical_templates"][0]["ref"]
        .as_str()
        .unwrap();
    let issuance_raw = fs::read(root.join(issuance_ref)).unwrap();
    let workbench_plan_ref = workbench["plan_ref"].as_str().unwrap();
    let workbench_plan_raw = fs::read(root.join(workbench_plan_ref)).unwrap();
    assert_eq!(
        validate_recipe_plan(
            &dta_raw,
            "concept-workbench",
            SelectedDocument {
                reference: workbench_plan_ref,
                raw: &workbench_plan_raw,
            },
            &schema,
        ),
        Ok(())
    );
    assert_eq!(
        validate_recipe_template(
            &dta_raw,
            "concept-workbench",
            "identity-issuance",
            SelectedDocument {
                reference: issuance_ref,
                raw: &issuance_raw,
            },
            &schema,
        ),
        Ok(())
    );
    let mut duplicate_identity = dta_value.clone();
    let workbench = duplicate_identity["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    let prefixes = workbench["identity_profile"]["identity_prefixes"]
        .as_array_mut()
        .unwrap();
    prefixes.push(prefixes[0].clone());
    assert_eq!(
        validate(&serde_json::to_vec(&duplicate_identity).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("identity kind"))
    );
    for stage in ["concept-candidate", "concept-review"] {
        let recipe = dta_value["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|recipe| recipe["stage"] == stage)
            .unwrap();
        let issuance_ref = recipe["historical_templates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["role"] == "identity-issuance")
            .unwrap()["ref"]
            .as_str()
            .unwrap();
        let issuance_raw = fs::read(root.join(issuance_ref)).unwrap();
        assert_eq!(
            validate_recipe_template(
                &dta_raw,
                stage,
                "identity-issuance",
                SelectedDocument {
                    reference: issuance_ref,
                    raw: &issuance_raw,
                },
                &schema,
            ),
            Ok(())
        );
        let plan_ref = recipe["plan_ref"].as_str().unwrap();
        let plan_raw = fs::read(root.join(plan_ref)).unwrap();
        assert_eq!(
            validate_recipe_plan(
                &dta_raw,
                stage,
                SelectedDocument {
                    reference: plan_ref,
                    raw: &plan_raw,
                },
                &schema,
            ),
            Ok(())
        );
    }
    let mut duplicate_orthography = dta_value.clone();
    let parallel = duplicate_orthography["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "parallel-lexical-candidates")
        .unwrap();
    let replacements = parallel["orthography"]["language_profiles"][0]["character_replacements"]
        .as_array_mut()
        .unwrap();
    replacements.push(replacements[0].clone());
    assert_eq!(
        validate(
            &serde_json::to_vec(&duplicate_orthography).unwrap(),
            &schema
        ),
        Err(EditionCurationError::Duplicate("character replacement"))
    );
    let mut invalid_stopwords = dta_value.clone();
    let parallel = invalid_stopwords["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "parallel-lexical-candidates")
        .unwrap();
    parallel["stopword_selection"]["unknown_language_profile"] = json!("missing");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_stopwords).unwrap(), &schema),
        Err(EditionCurationError::MissingReference)
    );
    let mut invalid_language = dta_value.clone();
    let workbench = invalid_language["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    workbench["form_selection"]["unknown_language_suffix_profile"] = json!("missing");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_language).unwrap(), &schema),
        Err(EditionCurationError::MissingReference)
    );
    let mut repeated_cue = dta_value.clone();
    let workbench = repeated_cue["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    let cues = workbench["speaker_cues"]["cues"].as_array_mut().unwrap();
    cues.push(cues[0].clone());
    assert_eq!(
        validate(&serde_json::to_vec(&repeated_cue).unwrap(), &schema),
        Err(EditionCurationError::Duplicate("speaker cue role"))
    );
    let mut unsafe_term = dta_value.clone();
    let workbench = unsafe_term["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    workbench["speaker_cues"]["cues"][0]["verbs"][0] = json!("a|b");
    assert_eq!(
        validate(&serde_json::to_vec(&unsafe_term).unwrap(), &schema),
        Err(EditionCurationError::InvalidSchema)
    );
    let mut invalid_outcome = dta_value.clone();
    let workbench = invalid_outcome["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-workbench")
        .unwrap();
    workbench["speaker_cues"]["outcomes"][0]["primary"] = json!("{unknown}");
    assert_eq!(
        validate(&serde_json::to_vec(&invalid_outcome).unwrap(), &schema),
        Err(EditionCurationError::InvalidRange(
            "speaker outcome template"
        ))
    );
    let candidate = dta_value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "concept-candidate")
        .unwrap();
    let candidate_template_ref = candidate["historical_templates"][0]["ref"]
        .as_str()
        .unwrap();
    let candidate_template_raw = fs::read(root.join(candidate_template_ref)).unwrap();
    let candidate_template = SelectedDocument {
        reference: candidate_template_ref,
        raw: &candidate_template_raw,
    };
    assert_eq!(
        validate_recipe_template(
            &dta_raw,
            "concept-candidate",
            "legacy-output-template",
            candidate_template,
            &schema
        ),
        Ok(())
    );
    let mut missing_claim = dta_value.clone();
    let candidate = missing_claim["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-candidate")
        .unwrap();
    candidate["candidate_signals"]["claim_rules"][0]["claims"][0] = json!("missing_claim");
    assert_eq!(
        validate_recipe_template(
            &serde_json::to_vec(&missing_claim).unwrap(),
            "concept-candidate",
            "legacy-output-template",
            candidate_template,
            &schema
        ),
        Err(EditionCurationError::MissingReference)
    );
    let mut missing_signal = dta_value.clone();
    let candidate = missing_signal["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "concept-candidate")
        .unwrap();
    candidate["candidate_signals"]["classification"]["recurrence_signals"][0] =
        json!("missing-signal");
    assert_eq!(
        validate(&serde_json::to_vec(&missing_signal).unwrap(), &schema),
        Err(EditionCurationError::MissingReference)
    );
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
    let mut wrong_readiness: Value = serde_json::from_slice(&target_raw).unwrap();
    let readiness = wrong_readiness["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "transfer-readiness")
        .unwrap();
    readiness["output_filename"] = json!("different.json");
    assert_eq!(
        validate(&serde_json::to_vec(&wrong_readiness).unwrap(), &schema),
        Err(EditionCurationError::BindingMismatch)
    );
    let mut unsafe_routes: Value = serde_json::from_slice(&target_raw).unwrap();
    let route = unsafe_routes["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["stage"] == "structural-source-routes")
        .unwrap();
    route["configurations"][0]["slug"] = json!("../other");
    assert_eq!(
        validate(&serde_json::to_vec(&unsafe_routes).unwrap(), &schema),
        Err(EditionCurationError::UnsafePath)
    );
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
