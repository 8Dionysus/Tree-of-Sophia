//! Mechanical closure checks for the retained Zarathustra morphology context packet.
//!
//! This validates source bindings, private-content boundaries, artifact admission
//! refusal, and the recorded result. It does not grant semantic, rights, or runtime
//! authority.

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde_json::{Value, json};

use crate::zarathustra_lexical::{LexicalCapture, LexicalSchema};
use crate::zarathustra_lexical_validate::{
    check_active, load_provenance, nested_keys, recorded_generator_digest,
};

const PLAN_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/morphology-contextual-episode.selected-form-b.v1.json";
const RECEIPT_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/morphology-contextual-episode.selected-form-b.receipt.v1.json";
const PROVENANCE_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/provenance.morphology-contextual-episode.selected-form-b.v1.jsonl";
const ADMISSION_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/morphology-contextual-episode.selected-form-b.artifact-admission.v1.json";
const ADMISSION_PROVENANCE_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/provenance.morphology-contextual-episode.selected-form-b.artifact-admission.v1.jsonl";
const RESULT_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/morphology-contextual-episode.selected-form-b.result.v1.json";
const RESULT_PROVENANCE_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/provenance.morphology-contextual-episode.selected-form-b.result.v1.jsonl";
const RESULT_GENERATOR_REF: &str = "scripts/record_zarathustra_morphology_contextual_result.py";
const GENERATOR_REF: &str = "scripts/build_zarathustra_morphology_context_packet.py";

fn parse_object(raw: &[u8], reference: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(raw).map_err(|error| format!("cannot read {reference}: {error}"))?;
    if !value.is_object() {
        return Err(format!("{reference} must contain a JSON object"));
    }
    Ok(value)
}

fn read_json(
    capture: &mut LexicalCapture<'_>,
    schema: &mut dyn LexicalSchema,
    reference: &str,
    contract: &str,
    label: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(Value, Vec<u8>), String> {
    check_active(deadline, cancelled)?;
    let raw = capture.read(reference).map_err(|error| error.to_string())?;
    check_active(deadline, cancelled)?;
    let value = parse_object(&raw, reference)?;
    schema
        .check(contract, &raw)
        .map_err(|error| format!("{label} schema failed: {error}"))?;
    check_active(deadline, cancelled)?;
    Ok((value, raw))
}

fn check_provenance_schema(
    capture: &mut LexicalCapture<'_>,
    schema: &mut dyn LexicalSchema,
    reference: &str,
    label: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    check_active(deadline, cancelled)?;
    let raw = capture
        .read(reference)
        .map_err(|error| format!("cannot read provenance {reference}: {error}"))?;
    check_active(deadline, cancelled)?;
    let text = std::str::from_utf8(&raw)
        .map_err(|error| format!("cannot read provenance {reference}: {error}"))?;
    capture
        .read("ToS/contracts/provenance-event.schema.json")
        .map_err(|error| format!("cannot read provenance schema: {error}"))?;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        check_active(deadline, cancelled)?;
        schema
            .check(
                "ToS/contracts/provenance-event.schema.json",
                line.as_bytes(),
            )
            .map_err(|error| format!("{label} schema failed: {error}"))?;
    }
    Ok(())
}

fn sha256(raw: &[u8]) -> String {
    tos_foundation::Digest256::of_bytes(raw).to_hex()
}

fn sha256_ref(
    capture: &mut LexicalCapture<'_>,
    reference: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    check_active(deadline, cancelled)?;
    let raw = capture.read(reference).map_err(|error| error.to_string())?;
    check_active(deadline, cancelled)?;
    Ok(sha256(&raw))
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |current, key| {
        current.get(*key).unwrap_or(&Value::Null)
    })
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    value_at(value, path)
        .as_str()
        .ok_or_else(|| format!("expected string field {}", path.join(".")))
}

fn string_pairs(value: &Value) -> BTreeSet<(String, String)> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some((
                entry.get("ref")?.as_str()?.to_owned(),
                entry.get("sha256")?.as_str()?.to_owned(),
            ))
        })
        .collect()
}

fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_none_or(|number| number != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn admission_gate_open(value: &Value) -> bool {
    if value == &Value::Bool(false) {
        return false;
    }
    if let Some(number) = value.as_number() {
        return number.as_f64().unwrap_or(f64::NAN) != 0.0;
    }
    true
}

fn exact_one(events: Vec<Value>, label: &str) -> Result<Value, String> {
    if events.len() != 1 {
        return Err(format!("{label} must contain exactly one event"));
    }
    Ok(events.into_iter().next().expect("length checked"))
}

pub(super) fn validate_morphology_context(
    capture: &mut LexicalCapture<'_>,
    schema: &mut dyn LexicalSchema,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value, String> {
    check_active(deadline, cancelled)?;

    let (plan, plan_raw) = read_json(
        capture,
        schema,
        PLAN_REF,
        "ToS/contracts/morphology-contextual-episode-plan.schema.json",
        "morphology contextual episode plan",
        deadline,
        cancelled,
    )?;
    let (receipt, receipt_raw) = read_json(
        capture,
        schema,
        RECEIPT_REF,
        "ToS/contracts/morphology-contextual-episode-receipt.schema.json",
        "morphology contextual episode receipt",
        deadline,
        cancelled,
    )?;
    let (admission, admission_raw) = read_json(
        capture,
        schema,
        ADMISSION_REF,
        "ToS/contracts/morphology-contextual-artifact-admission.schema.json",
        "morphology contextual artifact admission",
        deadline,
        cancelled,
    )?;
    let (result, result_raw) = read_json(
        capture,
        schema,
        RESULT_REF,
        "ToS/contracts/morphology-contextual-result-receipt.schema.json",
        "morphology contextual result",
        deadline,
        cancelled,
    )?;

    check_active(deadline, cancelled)?;
    let provenance = exact_one(
        load_provenance(capture, PROVENANCE_REF)?,
        "morphology context provenance",
    )?;
    check_provenance_schema(
        capture,
        schema,
        PROVENANCE_REF,
        "morphology contextual episode provenance",
        deadline,
        cancelled,
    )?;
    check_active(deadline, cancelled)?;
    let admission_provenance = exact_one(
        load_provenance(capture, ADMISSION_PROVENANCE_REF)?,
        "morphology context artifact admission provenance",
    )?;
    check_provenance_schema(
        capture,
        schema,
        ADMISSION_PROVENANCE_REF,
        "morphology contextual artifact admission provenance",
        deadline,
        cancelled,
    )?;
    check_active(deadline, cancelled)?;
    let result_provenance = exact_one(
        load_provenance(capture, RESULT_PROVENANCE_REF)?,
        "morphology contextual result provenance",
    )?;
    check_provenance_schema(
        capture,
        schema,
        RESULT_PROVENANCE_REF,
        "morphology contextual result provenance",
        deadline,
        cancelled,
    )?;
    check_active(deadline, cancelled)?;

    let prohibited_private_keys: BTreeSet<String> = [
        "context_text",
        "target_exact_form",
        "occurrence_id",
        "text_node_path",
        "context_node_path",
        "source_node_start_offset",
        "source_node_end_offset",
        "target_start_offset",
        "target_end_offset",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut receipt_provenance_keys = nested_keys(&receipt);
    receipt_provenance_keys.extend(nested_keys(&provenance));
    let leaked: Vec<_> = prohibited_private_keys
        .intersection(&receipt_provenance_keys)
        .cloned()
        .collect();
    if !leaked.is_empty() {
        return Err(format!(
            "morphology context private row data leaked through keys: {}",
            leaked.join(", ")
        ));
    }
    if plan["status"] != "ready-to-materialize-context-packet"
        || plan["frozen_before_b_output"] != true
    {
        return Err("morphology context plan was not frozen before B output".to_owned());
    }
    let authority_boundary = string_at(&plan, &["authority_boundary"])?;
    if string_at(&receipt, &["authority_boundary"])? != authority_boundary {
        return Err("morphology context authority boundary drift".to_owned());
    }

    let plan_digest = sha256(&plan_raw);
    check_active(deadline, cancelled)?;
    let generator_digest = recorded_generator_digest(
        capture,
        GENERATOR_REF,
        string_at(&receipt, &["generator", "sha256"])?,
    )?;
    check_active(deadline, cancelled)?;
    if receipt["plan"] != json!({"ref": PLAN_REF, "sha256": plan_digest}) {
        return Err("morphology context plan receipt drift".to_owned());
    }
    if receipt["generator"] != json!({"ref": GENERATOR_REF, "sha256": generator_digest}) {
        return Err("morphology context generator receipt drift".to_owned());
    }
    if plan["tracked_receipt_ref"] != RECEIPT_REF {
        return Err("morphology context receipt route drift".to_owned());
    }
    if plan["provenance_ref"] != PROVENANCE_REF {
        return Err("morphology context provenance route drift".to_owned());
    }

    let ref_bindings = [
        ("research", value_at(&plan, &["research"])),
        (
            "parent morphology plan",
            value_at(&plan, &["parent_morphology_plan"]),
        ),
        (
            "A result receipt",
            value_at(&plan, &["a_trigger", "result_receipt"]),
        ),
        (
            "recurrence plan",
            value_at(&plan, &["source_recurrence", "plan"]),
        ),
        (
            "recurrence receipt",
            value_at(&plan, &["source_recurrence", "receipt"]),
        ),
    ];
    for (label, binding) in ref_bindings {
        check_active(deadline, cancelled)?;
        let reference = binding["ref"].as_str().unwrap_or("");
        let expected_digest = binding["sha256"].as_str().unwrap_or("");
        if sha256_ref(capture, reference, deadline, cancelled)? != expected_digest {
            return Err(format!("morphology context {label} digest drift"));
        }
    }

    let trigger = &plan["a_trigger"];
    let trigger_receipt = &receipt["trigger_closure"];
    if trigger_receipt["result_receipt"] != trigger["result_receipt"] {
        return Err("morphology context A result binding drift".to_owned());
    }
    for field in [
        "selected_row_sha256",
        "input_preserved",
        "lemma_analysis_count",
        "provider_pos_categories",
        "separable_candidate_count",
    ] {
        if trigger_receipt[field] != trigger[field] {
            return Err(format!("morphology context trigger {field} drift"));
        }
    }
    if trigger_receipt["private_raw_output_sha256"] != trigger["private_raw_output"]["sha256"]
        || trigger_receipt["selected_row_match_count"] != 1
        || trigger["source_values_tracked"] != false
    {
        return Err("morphology context private A trigger closure drift".to_owned());
    }

    let recurrence = &plan["source_recurrence"];
    let observed_recurrence = &receipt["source_recurrence"];
    let expected_recurrence = json!({
        "plan": recurrence["plan"],
        "receipt": recurrence["receipt"],
        "local_bundle_sha256": recurrence["local_bundle"]["sha256"],
        "complete_occurrence_count": recurrence["occurrence_count"],
        "source_payload_fixity_match_count": 3,
    });
    if observed_recurrence != &expected_recurrence {
        return Err("morphology context recurrence closure drift".to_owned());
    }
    let selection = &receipt["selection"];
    for field in ["method", "recurrence_ranks", "rank_roles"] {
        if selection[field] != plan["selection"][field] {
            return Err(format!("morphology context selection {field} drift"));
        }
    }
    if selection["row_count"] != 3
        || selection["part_counts"] != json!({"1": 1, "3": 1, "4": 1})
        || selection["b_output_visible_during_selection"] != false
        || selection["semantic_labels_used"] != false
    {
        return Err("morphology context output-blind selection drift".to_owned());
    }

    let local_plan = &plan["local_packet"];
    let local_receipt = &receipt["local_packet"];
    for field in [
        "relative_path",
        "format",
        "schema_ref",
        "schema_version",
        "mode",
    ] {
        if local_receipt[field] != local_plan[field] {
            return Err(format!("morphology context local packet {field} drift"));
        }
    }
    if local_receipt["row_count"] != local_plan["expected_row_count"]
        || local_receipt["source_bearing"] != true
        || local_plan["visibility"] != "gitignored-local-only"
    {
        return Err("morphology context local packet posture drift".to_owned());
    }
    if receipt["content_exposure"]["local_context"] != true {
        return Err("morphology context local source-bearing posture drift".to_owned());
    }
    for field in [
        "tracked_exact_strings",
        "tracked_context",
        "tracked_occurrence_positions",
    ] {
        if receipt["content_exposure"][field] != false {
            return Err(format!(
                "morphology context tracked exposure opened: {field}"
            ));
        }
    }
    if receipt["rights_and_visibility"] != plan["rights_and_visibility"] {
        return Err("morphology context rights posture drift".to_owned());
    }
    if receipt["competence_boundary"] != plan["competence_boundary"] {
        return Err("morphology context competence posture drift".to_owned());
    }
    if receipt["semantic_boundary"] != plan["semantic_boundary"] {
        return Err("morphology context semantic boundary drift".to_owned());
    }
    if receipt["semantic_boundary"]
        .as_object()
        .is_none_or(|boundary| boundary.values().any(python_truthy))
    {
        return Err("morphology context semantic authority opened".to_owned());
    }
    if receipt["variant_state"]
        != json!({
            "a": "existing-context-free-candidate-set",
            "b": "admitted-unacquired",
            "c": "blocked-question-inapplicable",
            "b_acquisition_requires_artifact_audit": true,
            "b_execution_requires_fresh_host_preflight": true,
            "human_work_scheduled": false,
        })
    {
        return Err("morphology context variant state drift".to_owned());
    }

    if provenance["event_id"] != receipt["provenance_event_ref"] {
        return Err("morphology context provenance identity drift".to_owned());
    }
    if provenance["event_type"] != "annotation"
        || provenance["status"] != "completed_with_warnings"
        || provenance["method"]["artifact_digest"] != generator_digest
    {
        return Err("morphology context provenance method/status drift".to_owned());
    }
    let configuration = &provenance["method"]["configuration"];
    if configuration.get("selection") != Some(&plan["selection"]["method"])
        || configuration.get("recurrence_ranks") != Some(&plan["selection"]["recurrence_ranks"])
        || configuration["b_output_visible_during_selection"] != false
        || configuration["c_admitted_for_question"] != false
        || configuration["human_work_scheduled"] != false
    {
        return Err("morphology context provenance configuration drift".to_owned());
    }
    let receipt_digest = sha256(&receipt_raw);
    if string_pairs(&provenance["outputs"])
        != BTreeSet::from([
            (
                local_plan["relative_path"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned(),
                local_receipt["sha256"].as_str().unwrap_or("").to_owned(),
            ),
            (RECEIPT_REF.to_owned(), receipt_digest.clone()),
        ])
    {
        return Err("morphology context provenance output drift".to_owned());
    }
    if !provenance["rights_basis_ref"].is_null() {
        return Err("morphology context provenance unexpectedly claims rights basis".to_owned());
    }

    let admission_question = &admission["question"];
    if admission_question["plan"] != json!({"ref": PLAN_REF, "sha256": plan_digest}) {
        return Err("morphology context artifact admission plan drift".to_owned());
    }
    if admission_question["context_receipt"]
        != json!({"ref": RECEIPT_REF, "sha256": receipt_digest})
    {
        return Err("morphology context artifact admission receipt drift".to_owned());
    }
    if admission_question["private_packet_sha256"] != local_receipt["sha256"]
        || admission_question["private_packet_bytes"] != local_receipt["bytes"]
        || admission_question["private_packet_row_count"] != local_receipt["row_count"]
        || admission_question["frozen_before_b_output"] != true
    {
        return Err("morphology context artifact admission packet closure drift".to_owned());
    }
    if admission["status"] != "artifact-acquired-admission-denied-b-not-run"
        || admission["variant"] != "B"
    {
        return Err("morphology context artifact admission status/authority drift".to_owned());
    }
    let artifact = &admission["artifact_candidate"];
    if artifact["principal_wheel"]["sha256"]
        != "9d35263ac80e80e9730ee21830ffdbe96cf256b72c71e30326ae5865456ade9a"
        || artifact["principal_wheel"]["bytes"] != 627548130u64
        || artifact["principal_wheel_license_status"] != "absent"
        || artifact["model_metadata_license_status"] != "absent"
    {
        return Err("morphology context exact artifact or license posture drift".to_owned());
    }
    let acquisition = &admission["acquisition"];
    if acquisition["status"] != "private-cache-complete"
        || acquisition["wheel_count"] != 41
        || acquisition["target_runtime"] != "CPython-3.12-x86_64-linux"
    {
        return Err("morphology context private acquisition closure drift".to_owned());
    }
    let trust = &admission["trust_admission"];
    let expected_deny_reasons: BTreeSet<String> = [
        "no_latest_record",
        "verification_not_ok",
        "verification_errors_present",
        "verification_missing_required_sidecars",
        "required_controls_not_verified:abi_signature,ml_bom,sbom,sigstore_cosign,slsa_in_toto",
        "production_consumer_requires_non_local_trust_root",
        "production_consumer_requires_release_lifecycle",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let deny_reasons: BTreeSet<String> = trust["deny_reasons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if trust["trust_gate_verdict"] != "deny"
        || trust["verification_ok"] != false
        || trust["latest_eligible"] != false
        || trust["verified_controls"]
            .as_array()
            .is_none_or(|value| !value.is_empty())
        || trust["signature_status"] != "missing_backend"
        || deny_reasons != expected_deny_reasons
    {
        return Err("morphology context fail-closed artifact verdict drift".to_owned());
    }
    if admission["execution_effects"]
        .as_object()
        .is_none_or(|effects| effects.values().any(python_truthy))
    {
        return Err("morphology context B execution was claimed after denied admission".to_owned());
    }
    if admission["content_boundary"]
        .as_object()
        .is_none_or(|boundary| boundary.values().any(python_truthy))
    {
        return Err("morphology context artifact admission exposed private content".to_owned());
    }
    if admission["gate_effects"]
        .as_object()
        .is_none_or(|effects| effects.values().any(admission_gate_open))
    {
        return Err("morphology context artifact refusal opened a downstream gate".to_owned());
    }
    let serialized_admission = serde_json::to_string(&admission).unwrap_or_default();
    if serialized_admission.contains("/srv/") || serialized_admission.contains("local-content/") {
        return Err("morphology context artifact admission leaked a private path".to_owned());
    }

    let admission_digest = sha256(&admission_raw);
    if admission_provenance["event_id"] != admission["provenance_event_ref"] {
        return Err("morphology context artifact admission provenance identity drift".to_owned());
    }
    if admission_provenance["event_type"] != "rejection"
        || admission_provenance["status"] != "completed_with_warnings"
        || admission_provenance["method"]["artifact_digest"]
            != artifact["principal_wheel"]["sha256"]
        || !admission_provenance["rights_basis_ref"].is_null()
    {
        return Err("morphology context artifact admission provenance posture drift".to_owned());
    }
    let admission_input_pairs = string_pairs(&admission_provenance["inputs"]);
    let required_admission_inputs = BTreeSet::from([
        (PLAN_REF.to_owned(), plan_digest.clone()),
        (RECEIPT_REF.to_owned(), receipt_digest.clone()),
    ]);
    if !required_admission_inputs.is_subset(&admission_input_pairs) {
        return Err("morphology context artifact admission provenance input drift".to_owned());
    }
    if string_pairs(&admission_provenance["outputs"])
        != BTreeSet::from([(ADMISSION_REF.to_owned(), admission_digest.clone())])
    {
        return Err("morphology context artifact admission provenance output drift".to_owned());
    }

    let result_digest = sha256(&result_raw);
    check_active(deadline, cancelled)?;
    let result_generator_digest = recorded_generator_digest(
        capture,
        RESULT_GENERATOR_REF,
        string_at(&result, &["generator", "sha256"])?,
    )?;
    check_active(deadline, cancelled)?;
    if result["status"] != "b-executed-machine-proposal-awaiting-real-trigger"
        || result["question"]["plan"] != json!({"ref": PLAN_REF, "sha256": plan_digest})
        || result["question"]["context_receipt"]
            != json!({"ref": RECEIPT_REF, "sha256": receipt_digest})
        || result["question"]["historical_negative_admission"]
            != json!({
                "ref": ADMISSION_REF,
                "sha256": admission_digest,
                "retained": true,
                "superseded": false,
            })
        || result["generator"]
            != json!({
                "ref": RESULT_GENERATOR_REF,
                "sha256": result_generator_digest,
            })
    {
        return Err("morphology contextual B result identity/authority drift".to_owned());
    }
    if result["source_input"]["packet_sha256"] != local_receipt["sha256"]
        || result["source_input"]["packet_bytes"] != local_receipt["bytes"]
        || result["source_input"]["row_count"] != 3
        || result["source_input"]["selection_ranks"] != json!([1, 73, 145])
        || result["source_input"]["source_text_accepted"] != false
    {
        return Err("morphology contextual B result source closure drift".to_owned());
    }
    let trust_result = &result["artifact_admission"];
    if trust_result["trust_gate_verdict"] != "allow"
        || trust_result["latest_eligible"] != true
        || trust_result["lifecycle_state"] != "manually-verified"
        || trust_result["required_controls"] != trust_result["present_controls"]
        || trust_result["present_controls"] != trust_result["verified_controls"]
        || trust_result["rights_effect"] != "none"
    {
        return Err("morphology contextual B result trust closure drift".to_owned());
    }
    if result["repeat_determinism"]["pass_1_stream_sha256"]
        != result["repeat_determinism"]["pass_2_stream_sha256"]
        || result["repeat_determinism"]["deterministic"] != true
        || result["repeat_determinism"]["mismatch_count"] != 0
        || result["tokenization"]["exact_single_token_alignment_count"] != 3
        || result["tokenization"]["split_or_expanded_alignment_count"] != 0
    {
        return Err("morphology contextual B result repeat/tokenization drift".to_owned());
    }
    if result["quality"]["status"] != "unmeasured-no-german-competent-gold"
        || result["quality"]["german_competent_gold_count"] != 0
        || result["followup"]["human_work_scheduled"] != false
        || result["followup"]["automatic_review_opened"] != false
        || result["followup"]["automatic_promotion_authorized"] != false
        || result["semantic_boundary"]
            .as_object()
            .is_none_or(|boundary| boundary.values().any(python_truthy))
    {
        return Err("morphology contextual B result authority gate opened".to_owned());
    }
    let serialized_result =
        serde_json::to_string(&json!([result, result_provenance])).unwrap_or_default();
    for prohibited in [
        "/srv/",
        "local-content/",
        "context_text",
        "target_exact_form",
        "occurrence_id",
        "target_start_offset",
        "target_end_offset",
    ] {
        if serialized_result.contains(prohibited) {
            return Err(format!(
                "morphology contextual B result leaked private material: {prohibited}"
            ));
        }
    }
    if result_provenance["event_id"] != result["provenance_event_ref"]
        || result_provenance["event_type"] != "annotation"
        || result_provenance["status"] != "completed_with_warnings"
        || result_provenance["method"]["artifact_digest"] != result_generator_digest
        || !result_provenance["rights_basis_ref"].is_null()
    {
        return Err("morphology contextual B result provenance posture drift".to_owned());
    }
    if string_pairs(&result_provenance["outputs"])
        != BTreeSet::from([(RESULT_REF.to_owned(), result_digest.clone())])
    {
        return Err("morphology contextual B result provenance output drift".to_owned());
    }

    let summary = json!({
        "plan_ref": PLAN_REF,
        "plan_sha256": plan_digest,
        "receipt_ref": RECEIPT_REF,
        "receipt_sha256": receipt_digest,
        "local_packet_sha256": local_receipt["sha256"],
        "local_packet_verified": false,
        "selection": selection,
        "context_summary": receipt["context_summary"],
        "variant_state": receipt["variant_state"],
        "b_artifact_admission": {
            "ref": ADMISSION_REF,
            "sha256": admission_digest,
            "status": admission["status"],
            "authority_boundary": admission["authority_boundary"],
            "trust_gate_verdict": trust["trust_gate_verdict"],
            "runtime_built": admission["execution_effects"]["runtime_built"],
            "model_output_present": admission["execution_effects"]["model_output_present"],
        },
        "b_result": {
            "ref": RESULT_REF,
            "sha256": result_digest,
            "status": result["status"],
            "authority_boundary": result["authority_boundary"],
            "quality": result["quality"]["status"],
            "human_work_scheduled": result["followup"]["human_work_scheduled"],
            "semantic_effect": result["semantic_boundary"]
                .as_object()
                .is_some_and(|boundary| boundary.values().any(python_truthy)),
        },
        "authority_boundary": authority_boundary,
    });
    check_active(deadline, cancelled)?;
    Ok(summary)
}
