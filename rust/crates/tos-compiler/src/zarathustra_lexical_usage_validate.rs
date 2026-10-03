//! Validation for the private Zarathustra usage-context receipt.
//!
//! This checks source bindings and withholding boundaries recorded by the
//! usage-context route. It does not read the ignored local bundle or make a
//! semantic or rights judgment.

use crate::zarathustra_lexical::{LexicalCapture, LexicalSchema};
use crate::zarathustra_lexical_validate::{
    check_active, load_provenance, nested_keys, recorded_generator_digest,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;

const PLAN_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/index-plan.v1.json";
const PROJECTION_REF: &str =
    "ToS/derived-exports/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.min.json";
const USAGE_CONTEXT_PLAN_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/usage-context-plan.v1.json";
const USAGE_CONTEXT_RECEIPT_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/usage-context-receipt.v1.json";
const USAGE_CONTEXT_PROVENANCE_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/usage-context-provenance.jsonl";
const USAGE_CONTEXT_GENERATOR_REF: &str = "scripts/build_zarathustra_usage_context_bundle.py";

const PROHIBITED_TRACKED_ROW_KEYS: &[&str] = &[
    "target_exact_form",
    "left_exact_tokens",
    "right_exact_tokens",
    "occurrence_id",
    "token_ordinal",
    "text_node_path",
    "start_offset",
    "end_offset",
];

fn object_member<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .ok_or_else(|| format!("missing object field: {key}"))
}

fn string_value<'a>(value: &'a Value, label: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{label} must be a string"))
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    string_value(object_member(value, key)?, key)
}

fn bool_field(value: &Value, key: &str) -> Result<bool, String> {
    object_member(value, key)?
        .as_bool()
        .ok_or_else(|| format!("{key} must be a boolean"))
}

fn count_field(value: &Value, key: &str) -> Result<u64, String> {
    object_member(value, key)?
        .as_u64()
        .ok_or_else(|| format!("{key} must be a non-negative integer"))
}

fn array_field<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    object_member(value, key)?
        .as_array()
        .ok_or_else(|| format!("{key} must be an array"))
}

fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn capture_read(
    capture: &mut LexicalCapture<'_>,
    reference: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, String> {
    check_active(deadline, cancelled)?;
    let raw = capture
        .read(reference)
        .map_err(|error| format!("cannot read {reference}: {error}"))?;
    check_active(deadline, cancelled)?;
    Ok(raw)
}

fn parse_json_object(raw: &[u8], path: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(raw).map_err(|error| format!("cannot read {path}: {error}"))?;
    if !value.is_object() {
        return Err(format!("{path} must contain a JSON object"));
    }
    Ok(value)
}

fn sha256(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

fn digest_for(
    capture: &mut LexicalCapture<'_>,
    reference: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    Ok(sha256(&capture_read(
        capture, reference, deadline, cancelled,
    )?))
}

fn provenance_pairs(value: &Value, key: &str) -> Result<BTreeSet<(String, String)>, String> {
    let mut pairs = BTreeSet::new();
    for entry in array_field(value, key)? {
        pairs.insert((
            string_field(entry, "ref")?.to_owned(),
            string_field(entry, "sha256")?.to_owned(),
        ));
    }
    Ok(pairs)
}

/// Validate the source bindings, receipt, provenance and privacy boundaries
/// for the tracked usage-context layer.
pub(super) fn validate_usage_context(
    capture: &mut LexicalCapture<'_>,
    schema: &mut dyn LexicalSchema,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value, String> {
    check_active(deadline, cancelled)?;
    let plan_raw = capture_read(capture, USAGE_CONTEXT_PLAN_REF, deadline, cancelled)?;
    let receipt_raw = capture_read(capture, USAGE_CONTEXT_RECEIPT_REF, deadline, cancelled)?;
    let plan = parse_json_object(&plan_raw, USAGE_CONTEXT_PLAN_REF)?;
    let receipt = parse_json_object(&receipt_raw, USAGE_CONTEXT_RECEIPT_REF)?;
    schema.check(
        "ToS/contracts/lexical-usage-context-plan.schema.json",
        &plan_raw,
    )?;
    schema.check(
        "ToS/contracts/lexical-usage-context-receipt.schema.json",
        &receipt_raw,
    )?;

    let provenance_raw = capture_read(capture, USAGE_CONTEXT_PROVENANCE_REF, deadline, cancelled)?;
    let provenance_text = std::str::from_utf8(&provenance_raw).map_err(|error| {
        format!(
            "cannot read provenance {}: {error}",
            USAGE_CONTEXT_PROVENANCE_REF
        )
    })?;
    check_active(deadline, cancelled)?;
    let provenance_events = load_provenance(capture, USAGE_CONTEXT_PROVENANCE_REF)?;
    if provenance_events.is_empty() {
        return Err("lexical provenance must contain at least one event".into());
    }
    if provenance_events.len() != 1 {
        return Err("usage-context provenance must contain exactly one event".into());
    }
    let provenance = &provenance_events[0];
    let provenance_line = provenance_text
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "lexical provenance must contain at least one event".to_owned())?;
    capture_read(
        capture,
        "ToS/contracts/provenance-event.schema.json",
        deadline,
        cancelled,
    )?;
    schema.check(
        "ToS/contracts/provenance-event.schema.json",
        provenance_line.as_bytes(),
    )?;

    let prohibited_keys = nested_keys(&receipt)
        .into_iter()
        .chain(nested_keys(provenance))
        .collect::<BTreeSet<_>>();
    let leaked_keys = PROHIBITED_TRACKED_ROW_KEYS
        .iter()
        .filter(|key| prohibited_keys.contains(**key))
        .copied()
        .collect::<Vec<_>>();
    if !leaked_keys.is_empty() {
        return Err(format!(
            "usage-context tracked row data leaked through keys: {}",
            leaked_keys.join(", ")
        ));
    }

    if string_field(&plan, "status")? != "frozen-before-output"
        || !bool_field(&plan, "frozen_before_output")?
    {
        return Err("usage-context selection was not frozen before output".into());
    }
    if string_field(&receipt, "generated_or_authored")? != "generated_from_local_lexical_projection"
    {
        return Err("usage-context generation posture drift".into());
    }
    let authority_boundary = string_field(&plan, "authority_boundary")?;
    if string_field(&receipt, "authority_boundary")? != authority_boundary {
        return Err("usage-context authority boundary drift".into());
    }

    let plan_digest = sha256(&plan_raw);
    check_active(deadline, cancelled)?;
    let generator_digest = recorded_generator_digest(
        capture,
        USAGE_CONTEXT_GENERATOR_REF,
        string_field(object_member(&receipt, "generator")?, "sha256")?,
    )?;
    check_active(deadline, cancelled)?;
    if object_member(&receipt, "plan")?
        != &json!({"ref": USAGE_CONTEXT_PLAN_REF, "sha256": plan_digest})
    {
        return Err("usage-context plan receipt drift".into());
    }
    if object_member(&receipt, "generator")?
        != &json!({"ref": USAGE_CONTEXT_GENERATOR_REF, "sha256": generator_digest})
    {
        return Err("usage-context generator receipt drift".into());
    }
    if string_field(&plan, "tracked_receipt_ref")? != USAGE_CONTEXT_RECEIPT_REF {
        return Err("usage-context tracked receipt route drift".into());
    }
    if string_field(&plan, "provenance_ref")? != USAGE_CONTEXT_PROVENANCE_REF {
        return Err("usage-context provenance route drift".into());
    }

    let source = object_member(&plan, "source_lexical_index")?;
    let control = object_member(&plan, "recurrence_control")?;
    let source_refs = [
        (
            "index_plan",
            string_field(source, "index_plan_ref")?,
            string_field(source, "index_plan_sha256")?,
        ),
        (
            "lexical_projection",
            string_field(source, "tracked_projection_ref")?,
            string_field(source, "tracked_projection_sha256")?,
        ),
        (
            "recurrence_plan",
            string_field(control, "plan_ref")?,
            string_field(control, "plan_sha256")?,
        ),
        (
            "recurrence_projection",
            string_field(control, "projection_ref")?,
            string_field(control, "projection_sha256")?,
        ),
    ];
    if source_refs[0].1 != PLAN_REF {
        return Err("usage-context index-plan route drift".into());
    }
    if source_refs[1].1 != PROJECTION_REF {
        return Err("usage-context lexical-projection route drift".into());
    }
    for (label, reference, expected_digest) in source_refs {
        check_active(deadline, cancelled)?;
        if digest_for(capture, reference, deadline, cancelled)? != expected_digest {
            return Err(format!("usage-context {label} digest drift"));
        }
        let expected_receipt = json!({"ref": reference, "sha256": expected_digest});
        if object_member(object_member(&receipt, "source_projections")?, label)?
            != &expected_receipt
        {
            return Err(format!("usage-context {label} receipt drift"));
        }
    }

    let database = object_member(&receipt, "source_database")?;
    if string_field(database, "relative_path")?
        != string_field(source, "local_database_relative_path")?
    {
        return Err("usage-context database route drift".into());
    }
    if string_field(database, "sha256")? != string_field(source, "local_database_sha256")? {
        return Err("usage-context database digest drift".into());
    }
    if count_field(database, "bytes")? != count_field(source, "local_database_bytes")? {
        return Err("usage-context database byte drift".into());
    }
    if string_field(database, "quick_check")? != "ok" {
        return Err("usage-context database check drift".into());
    }

    let observed_control = object_member(&receipt, "recurrence_control")?;
    if observed_control
        != &json!({
            "form_key": object_member(control, "form_key")?,
            "exact_form_sha256": object_member(control, "exact_form_sha256")?,
            "selection_basis": object_member(control, "selection_basis")?,
            "observed_tuple": object_member(control, "expected_tuple")?,
        })
    {
        return Err("usage-context recurrence control drift".into());
    }
    if !bool_field(control, "selection_frozen_before_context_output")? {
        return Err("usage-context control selection drift".into());
    }
    if bool_field(control, "tracked_source_surface")? {
        return Err("usage-context control unexpectedly exposes source surface".into());
    }

    let policy = object_member(&plan, "context_policy")?;
    let expected_policy = json!({
        "policy_id": object_member(policy, "policy_id")?,
        "window_tokens_each_side": object_member(policy, "window_tokens_each_side")?,
        "boundary": object_member(policy, "boundary")?,
        "sampling": object_member(policy, "sampling")?,
        "row_order": object_member(policy, "row_order")?,
        "sentence_boundary_claimed": object_member(policy, "sentence_boundary_claimed")?,
    });
    if object_member(&receipt, "context_policy")? != &expected_policy {
        return Err("usage-context policy receipt drift".into());
    }
    if string_field(policy, "sampling")? != "none-complete-occurrence-census" {
        return Err("usage-context census posture drift".into());
    }
    if bool_field(policy, "sentence_boundary_claimed")? {
        return Err("usage-context baseline claims a sentence boundary".into());
    }
    if bool_field(policy, "future_challengers_scheduled")? {
        return Err("usage-context plan schedules unadmitted challengers".into());
    }

    let local_plan = object_member(&plan, "local_bundle")?;
    let local_receipt = object_member(&receipt, "local_bundle")?;
    for (plan_field, receipt_field) in [
        ("relative_path", "relative_path"),
        ("format", "format"),
        ("schema_ref", "schema_ref"),
        ("schema_version", "schema_version"),
        ("mode", "mode"),
        ("expected_row_count", "row_count"),
        ("required_fields", "required_fields"),
    ] {
        if object_member(local_receipt, receipt_field)? != object_member(local_plan, plan_field)? {
            return Err(format!("usage-context local bundle {receipt_field} drift"));
        }
    }
    if string_field(local_plan, "storage_posture")? != "gitignored-local-only" {
        return Err("usage-context storage posture drift".into());
    }
    if !bool_field(local_plan, "source_bearing")? {
        return Err("usage-context local bundle source posture drift".into());
    }

    let expected_tuple = object_member(control, "expected_tuple")?;
    let summary = object_member(&receipt, "summary")?;
    let summary_expectations = [
        (
            "row_count",
            count_field(expected_tuple, "occurrence_count")?,
        ),
        (
            "target_occurrence_count",
            count_field(expected_tuple, "occurrence_count")?,
        ),
        (
            "source_item_count",
            count_field(expected_tuple, "part_range")?,
        ),
        ("page_count", count_field(expected_tuple, "page_range")?),
        (
            "section_count",
            count_field(expected_tuple, "section_range")?,
        ),
        (
            "unsectioned_occurrence_count",
            count_field(expected_tuple, "unsectioned_occurrence_count")?,
        ),
        (
            "source_editorial_occurrence_count",
            count_field(expected_tuple, "source_editorial_occurrence_count")?,
        ),
        ("semantic_fields_populated", 0),
    ];
    for (field, expected) in summary_expectations {
        if count_field(summary, field)? != expected {
            return Err(format!("usage-context summary {field} drift"));
        }
    }
    check_active(deadline, cancelled)?;
    let parts = array_field(&receipt, "parts")?;
    let source_item_count = count_field(summary, "source_item_count")?;
    if parts.len() as u64 != source_item_count
        || parts
            .iter()
            .enumerate()
            .any(|(index, part)| count_field(part, "part_order").ok() != Some(index as u64 + 1))
    {
        return Err("usage-context part order drift".into());
    }
    let unique_items = parts
        .iter()
        .map(|part| string_field(part, "item_ref").map(str::to_owned))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if unique_items.len() as u64 != source_item_count {
        return Err("usage-context source-item closure drift".into());
    }
    for (field, summary_field) in [
        ("occurrence_count", "target_occurrence_count"),
        ("page_count", "page_count"),
        ("section_count", "section_count"),
        (
            "unsectioned_occurrence_count",
            "unsectioned_occurrence_count",
        ),
    ] {
        let part_sum = parts.iter().try_fold(0_u64, |total, part| {
            check_active(deadline, cancelled)?;
            total
                .checked_add(count_field(part, field)?)
                .ok_or_else(|| format!("usage-context part {field} count overflow"))
        })?;
        if part_sum != count_field(summary, summary_field)? {
            return Err(format!("usage-context part {field} closure drift"));
        }
    }

    let identity = object_member(&receipt, "identity_closure")?;
    let target_count = count_field(summary, "target_occurrence_count")?;
    for field in [
        "unique_context_id_count",
        "unique_occurrence_id_count",
        "target_digest_match_count",
        "page_selector_resolution_count",
        "source_file_digest_resolution_count",
    ] {
        if count_field(identity, field)? != target_count {
            return Err(format!("usage-context identity {field} drift"));
        }
    }
    let resolved_sections = target_count
        .checked_sub(count_field(summary, "unsectioned_occurrence_count")?)
        .ok_or_else(|| "usage-context section-selector closure drift".to_owned())?;
    if count_field(identity, "section_selector_resolution_count")? != resolved_sections {
        return Err("usage-context section-selector closure drift".into());
    }
    if !bool_field(identity, "complete_occurrence_census")? {
        return Err("usage-context census is incomplete".into());
    }

    if object_member(&receipt, "content_exposure")? != object_member(&plan, "content_exposure")? {
        return Err("usage-context exposure posture drift".into());
    }
    let exposure = object_member(&receipt, "content_exposure")?;
    for field in [
        "tracked_exact_strings",
        "tracked_sequence",
        "tracked_context",
        "tracked_occurrence_positions",
        "confidentiality_claimed",
    ] {
        if bool_field(exposure, field)? {
            return Err(format!("usage-context tracked exposure opened: {field}"));
        }
    }
    if object_member(&receipt, "rights_and_visibility")?
        != object_member(&plan, "rights_and_visibility")?
    {
        return Err("usage-context rights posture drift".into());
    }
    let rights = object_member(&receipt, "rights_and_visibility")?;
    if string_field(rights, "future_site_route")? != "blocked"
        || !bool_field(rights, "fresh_public_acquisition_and_rights_gate_required")?
    {
        return Err("usage-context public route opened".into());
    }
    if object_member(&receipt, "semantic_boundary")? != object_member(&plan, "semantic_boundary")? {
        return Err("usage-context semantic boundary drift".into());
    }
    let semantic_boundary = object_member(&receipt, "semantic_boundary")?;
    if semantic_boundary
        .as_object()
        .ok_or_else(|| "usage-context semantic boundary must be an object".to_owned())?
        .values()
        .any(python_truthy)
    {
        return Err("usage-context semantic authority opened".into());
    }

    if string_field(provenance, "event_id")? != string_field(&plan, "provenance_event_ref")? {
        return Err("usage-context provenance identity drift".into());
    }
    if string_field(provenance, "event_type")? != "export" {
        return Err("usage-context provenance type drift".into());
    }
    if string_field(provenance, "status")? != "completed_with_warnings" {
        return Err("usage-context provenance status drift".into());
    }
    let method = object_member(provenance, "method")?;
    if string_field(method, "artifact_digest")? != generator_digest {
        return Err("usage-context provenance method drift".into());
    }
    let configuration = object_member(method, "configuration")?;
    if count_field(configuration, "target_occurrences")? != target_count
        || bool_field(configuration, "tracked_source_strings")?
        || !bool_field(configuration, "source_strings_local_only")?
        || bool_field(configuration, "sentence_boundary_claimed")?
        || bool_field(configuration, "future_challengers_scheduled")?
        || bool_field(configuration, "human_work_scheduled")?
    {
        return Err("usage-context provenance configuration drift".into());
    }

    let mut expected_inputs = BTreeSet::from([
        (USAGE_CONTEXT_PLAN_REF.to_owned(), plan_digest.clone()),
        (
            string_field(source, "local_database_relative_path")?.to_owned(),
            string_field(source, "local_database_sha256")?.to_owned(),
        ),
        (source_refs[1].1.to_owned(), source_refs[1].2.to_owned()),
        (source_refs[3].1.to_owned(), source_refs[3].2.to_owned()),
    ]);
    let research_ref = string_field(&plan, "research_ref")?;
    expected_inputs.insert((
        research_ref.to_owned(),
        digest_for(capture, research_ref, deadline, cancelled)?,
    ));
    if provenance_pairs(provenance, "inputs")? != expected_inputs {
        return Err("usage-context provenance input drift".into());
    }

    let receipt_digest = sha256(&receipt_raw);
    let expected_outputs = BTreeSet::from([
        (
            string_field(local_plan, "relative_path")?.to_owned(),
            string_field(local_receipt, "sha256")?.to_owned(),
        ),
        (USAGE_CONTEXT_RECEIPT_REF.to_owned(), receipt_digest.clone()),
    ]);
    if provenance_pairs(provenance, "outputs")? != expected_outputs {
        return Err("usage-context provenance output drift".into());
    }
    if !object_member(provenance, "rights_basis_ref")?.is_null() {
        return Err("usage-context provenance unexpectedly claims a rights basis".into());
    }

    check_active(deadline, cancelled)?;

    Ok(json!({
        "plan_ref": USAGE_CONTEXT_PLAN_REF,
        "plan_sha256": plan_digest,
        "receipt_ref": USAGE_CONTEXT_RECEIPT_REF,
        "receipt_sha256": receipt_digest,
        "local_bundle_sha256": string_field(local_receipt, "sha256")?,
        "local_bundle_verified": false,
        "summary": summary,
        "authority_boundary": authority_boundary,
    }))
}
