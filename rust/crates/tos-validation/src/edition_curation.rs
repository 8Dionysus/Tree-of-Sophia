//! Structural checks for edition-curation records before selecting source bytes.
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use num_bigint::BigInt;

use serde_json::Value;

use crate::{SchemaBackendProbe, SchemaProbeError, published_value};

pub const SCHEMA_URI: &str =
    "https://tree-of-sophia.local/ToS/contracts/edition-curation.v1.schema.json";

#[derive(Debug, PartialEq, Eq)]
pub enum EditionCurationError {
    Schema(SchemaProbeError),
    InvalidSchema,
    Duplicate(&'static str),
    InvalidRange(&'static str),
    UnsafePath,
    BindingMismatch,
    DigestMismatch,
    MissingReference,
    AmbiguousReference,
}

/// Exact caller-selected bytes and their normalized repository-relative path.
#[derive(Clone, Copy)]
pub struct SelectedDocument<'a> {
    pub reference: &'a str,
    pub raw: &'a [u8],
}

/// A referenced curation and the item and edition records selected alongside it.
#[derive(Clone, Copy)]
pub struct SelectedCuration<'a> {
    pub curation: SelectedDocument<'a>,
    pub manifest: SelectedDocument<'a>,
    pub edition: SelectedDocument<'a>,
}

/// Validate exact curation JSON using the caller's selected schema resources.
pub fn validate(raw: &[u8], schema: &SchemaBackendProbe) -> Result<(), EditionCurationError> {
    document(raw, schema).map(|_| ())
}

/// Bind a numbered recipe to its selected historical source map and required keys.
pub fn validate_numbered_source_map(
    raw: &[u8],
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, "numbered-structure")?;
    let locator = recipe["locators"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
        .iter()
        .find(|locator| locator["role"] == "source_map_path")
        .ok_or(EditionCurationError::MissingReference)?;
    let reference = locator["repository_path"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let digest = recipe["source_map_sha256"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    verify_selected_bytes(selected, reference, digest)?;
    let map = published_value(selected.raw, selected.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    let mut keys = BTreeSet::new();
    for unit in map["unit_starts"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
    {
        let key = unit["unit_key"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if !keys.insert(key) {
            return Err(EditionCurationError::Duplicate("source numbered key"));
        }
    }
    for required in recipe["source_map_required_keys"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
    {
        if !required.as_str().is_some_and(|key| keys.contains(key)) {
            return Err(EditionCurationError::MissingReference);
        }
    }
    Ok(())
}

/// Bind a stage to the exact caller-selected plan bytes before source payloads are opened.
pub fn validate_recipe_plan(
    raw: &[u8],
    stage: &str,
    plan: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    tos_foundation::RelativePath::parse(plan.reference)
        .map_err(|_| EditionCurationError::UnsafePath)?;
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let reference = if let Some(reference) = recipe["plan_ref"].as_str() {
        reference
    } else {
        let mut matches = recipe["locators"]
            .as_array()
            .ok_or(EditionCurationError::MissingReference)?
            .iter()
            .filter(|locator| locator["role"] == "index_plan" || locator["role"] == "plan");
        let locator = matches
            .next()
            .ok_or(EditionCurationError::MissingReference)?;
        if matches.next().is_some() {
            return Err(EditionCurationError::AmbiguousReference);
        }
        locator["repository_path"]
            .as_str()
            .ok_or(EditionCurationError::MissingReference)?
    };
    let current = recipe["plan_sha256"].as_str();
    let historical = recipe["historical_plan_sha256"].as_str();
    if matches!((current, historical), (Some(a), Some(b)) if a != b) {
        return Err(EditionCurationError::BindingMismatch);
    }
    let expected = current
        .or(historical)
        .ok_or(EditionCurationError::MissingReference)?;
    verify_selected_bytes(plan, reference, expected)?;
    if let Some(profile) = recipe.get("tei_text_profile") {
        let selected = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let issuance = &recipe["identity_issuance"];
        for name in ["created_at_plan_pointer", "identity_policy_plan_pointer"] {
            let pointer = issuance[name]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)?;
            if selected.pointer(pointer).and_then(Value::as_str).is_none() {
                return Err(EditionCurationError::MissingReference);
            }
        }
        if selected["identity_policy"]["automatic_remint_on_locator_drift"]
            != issuance["automatic_remint_on_locator_drift"]
        {
            return Err(EditionCurationError::BindingMismatch);
        }
        let excluded_pointer = profile["excluded_elements_plan_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let excluded = selected
            .pointer(excluded_pointer)
            .and_then(Value::as_array)
            .ok_or(EditionCurationError::MissingReference)?;
        let mut seen = BTreeSet::new();
        for element in excluded {
            let name = element
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(EditionCurationError::InvalidSchema)?;
            if !seen.insert(name) {
                return Err(EditionCurationError::Duplicate("excluded element"));
            }
        }
        let normalization_pointer = profile["normalization_plan_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if selected
            .pointer(normalization_pointer)
            .and_then(Value::as_str)
            != Some("none")
        {
            return Err(EditionCurationError::BindingMismatch);
        }
    }
    if let Some(rules) = recipe.get("selection_rules") {
        let selected = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let pointer = |key: &str| -> Result<&str, EditionCurationError> {
            rules[key]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)
        };
        let extraction = selected
            .pointer(pointer("extraction_pointer")?)
            .and_then(Value::as_object)
            .ok_or(EditionCurationError::MissingReference)?;
        for field in ["tool", "version", "mode"] {
            if extraction
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(EditionCurationError::MissingReference);
            }
        }
        let plan_id = selected
            .pointer(pointer("plan_id_pointer")?)
            .and_then(Value::as_str)
            .ok_or(EditionCurationError::MissingReference)?;
        let expected_seed = selected
            .pointer(pointer("seed_digest_pointer")?)
            .and_then(Value::as_str)
            .ok_or(EditionCurationError::MissingReference)?;
        let pdf_sha = recipe["expected_file_sha256"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let domain = rules["seed_domain"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let seed =
            tos_foundation::Digest256::of_bytes(format!("{pdf_sha}|{plan_id}|{domain}").as_bytes())
                .to_hex();
        if seed != expected_seed {
            return Err(EditionCurationError::BindingMismatch);
        }
        let filter_pointer = rules["content_filter_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let filter = selected
            .pointer(filter_pointer)
            .and_then(Value::as_object)
            .ok_or(EditionCurationError::MissingReference)?;
        for key in [
            "boundary_trim_pages",
            "minimum_alphabetic_characters",
            "minimum_nonblank_lines",
        ] {
            if filter.get(key).and_then(Value::as_u64).is_none() {
                return Err(EditionCurationError::MissingReference);
            }
        }
        let quota_pointer = rules["work_quotas_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let quotas = selected
            .pointer(quota_pointer)
            .and_then(Value::as_array)
            .ok_or(EditionCurationError::MissingReference)?;
        let works = recipe["works"]
            .as_array()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if quotas.len() != works.len()
            || works.iter().zip(quotas).any(|(work, quota)| {
                work["work_ref"] != quota["work_ref"]
                    || quota["random"].as_u64().is_none()
                    || quota["hard"].as_u64().is_none()
            })
        {
            return Err(EditionCurationError::BindingMismatch);
        }
    }

    if let Some(binding) = recipe.get("source_index_binding") {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let pointer = binding["consumer_input"]["input_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let input = selected_plan
            .pointer(pointer)
            .ok_or(EditionCurationError::MissingReference)?;
        let reference = input["ref"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        tos_foundation::RelativePath::parse(reference)
            .map_err(|_| EditionCurationError::UnsafePath)?;
        let digest = input["sha256"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let parsed = tos_foundation::Digest256::from_hex(digest)
            .map_err(|_| EditionCurationError::InvalidSchema)?;
        if parsed.to_hex() != digest {
            return Err(EditionCurationError::InvalidSchema);
        }
    }
    if let Some(settings) = recipe.get("evidence_settings") {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let status = settings["positive_status_plan_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let minimum = settings["minimum_universe_support_plan_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if selected_plan
            .pointer(status)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
            || selected_plan
                .pointer(minimum)
                .and_then(Value::as_u64)
                .is_none()
        {
            return Err(EditionCurationError::MissingReference);
        }
    }
    if let Some(settings) = recipe.get("keyword_settings") {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let pointer = settings["minimum_frequency_plan_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if selected_plan
            .pointer(pointer)
            .and_then(Value::as_u64)
            .is_none()
        {
            return Err(EditionCurationError::MissingReference);
        }
    }
    if let Some(settings) = recipe.get("sequence_settings") {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let pointer = settings["minimum_count_plan_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if selected_plan
            .pointer(pointer)
            .and_then(Value::as_u64)
            .is_none()
        {
            return Err(EditionCurationError::MissingReference);
        }
    }
    if let Some(profile) = recipe.get("association_classification") {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        for group in ["proposed", "ambiguous"] {
            let pointers = profile[group]
                .as_object()
                .ok_or(EditionCurationError::InvalidSchema)?;
            for pointer in pointers.values() {
                let pointer = pointer
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if selected_plan
                    .pointer(pointer)
                    .and_then(Value::as_u64)
                    .is_none()
                {
                    return Err(EditionCurationError::MissingReference);
                }
            }
        }
    }
    if let Some(entries) = recipe["retained_artifacts"].as_array() {
        if entries.iter().any(|entry| {
            entry["plan_pointer"].is_string() || entry["plan_value_pointer"].is_string()
        }) {
            let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
                .map_err(EditionCurationError::Schema)?;
            for entry in entries {
                if let Some(pointer) = entry["plan_pointer"].as_str() {
                    let reference = selected_plan
                        .pointer(pointer)
                        .and_then(Value::as_str)
                        .ok_or(EditionCurationError::MissingReference)?;
                    tos_foundation::RelativePath::parse(reference)
                        .map_err(|_| EditionCurationError::UnsafePath)?;
                }
                if let Some(pointer) = entry["plan_value_pointer"].as_str() {
                    if selected_plan.pointer(pointer) != Some(&entry["sha256"]) {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                }
            }
        }
    }
    if let Some(profile) = recipe
        .get("identity_profile")
        .or_else(|| recipe.get("opaque_identity_profile"))
    {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let pointer = profile["issuance_time_plan_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if !pointer.starts_with('/')
            || selected_plan
                .pointer(pointer)
                .and_then(Value::as_str)
                .is_none()
        {
            return Err(EditionCurationError::MissingReference);
        }
    }
    if matches!(stage, "sentence-alignment" | "text-collation") {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let outputs = selected_plan["outputs"]
            .as_object()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let frozen = recipe["frozen_outputs"]
            .as_array()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if outputs
            .keys()
            .filter(|role| !role.starts_with("private_"))
            .count()
            != frozen.len()
        {
            return Err(EditionCurationError::BindingMismatch);
        }
        for output in frozen {
            let role = output["role"]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)?;
            if role.starts_with("private_") || outputs.get(role) != Some(&output["repository_path"])
            {
                return Err(EditionCurationError::BindingMismatch);
            }
        }
        if stage == "text-collation" {
            let roles = recipe["witness_roles"]
                .as_array()
                .ok_or(EditionCurationError::InvalidSchema)?;
            for role in roles {
                let plan_key = role["plan_key"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let identity_key = role["identity_key"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if selected_plan
                    .get(plan_key)
                    .and_then(|witness| witness.get("item_ref"))
                    != Some(&role["item_ref"])
                    || selected_plan["opaque_ids"]
                        .get(identity_key)
                        .and_then(Value::as_str)
                        .is_none()
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
            }
        }
    }
    if matches!(stage, "tei-source-foundation" | "authored-canon-evidence") {
        let selected_plan = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let selector = if stage == "tei-source-foundation" {
            &selected_plan["selector"]
        } else {
            &selected_plan["source_selector"]
        };
        let steps = recipe["tei_selector"]["steps"]
            .as_array()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let mut expression = String::new();
        for step in steps {
            let element = step["element"]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)?;
            expression.push_str(&format!("/*[local-name()='{element}']"));
            if let Some(ordinal) = step["ordinal"].as_u64() {
                expression.push_str(&format!("[{ordinal}]"));
            }
        }
        if selector["scheme"] != "xpath-1.0-local-name" || selector["value"] != expression {
            return Err(EditionCurationError::BindingMismatch);
        }
    }
    Ok(())
}

/// Check a selected stage template against its curation-owned path and digest.
pub fn validate_recipe_template(
    raw: &[u8],
    stage: &str,
    role: &str,
    template: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let entry = recipe["historical_templates"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["role"] == role));
    let (reference, digest) = if let Some(entry) = entry {
        (entry["ref"].as_str(), entry["sha256"].as_str())
    } else if role == "historical-template" {
        (
            recipe["historical_template_ref"].as_str(),
            recipe["historical_template_sha256"].as_str(),
        )
    } else {
        return Err(EditionCurationError::MissingReference);
    };
    let reference = reference.ok_or(EditionCurationError::MissingReference)?;
    let digest = digest.ok_or(EditionCurationError::MissingReference)?;
    verify_selected_bytes(template, reference, digest)?;
    if stage == "morphology-theme-candidates" && role == "historical-template" {
        let selected = published_value(template.raw, template.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        for language in recipe["signature_settings"]["languages"]
            .as_array()
            .ok_or(EditionCurationError::InvalidSchema)?
        {
            let key = language["table_key"]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)?;
            let rows = selected[key]
                .as_array()
                .ok_or(EditionCurationError::MissingReference)?;
            if rows.iter().any(|row| {
                !row.as_array().is_some_and(|pair| {
                    pair.len() == 2
                        && pair
                            .iter()
                            .all(|entry| entry.as_str().is_some_and(|text| !text.is_empty()))
                })
            }) {
                return Err(EditionCurationError::InvalidSchema);
            }
        }
    }
    if stage == "witness-triangulation" && role == "historical-template" {
        let selected = published_value(template.raw, template.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let inputs = selected["packet"]["inputs"]
            .as_object()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let keys = recipe["source_selection"]["ordered_input_keys"]
            .as_array()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if inputs.len() != keys.len()
            || keys
                .iter()
                .any(|key| !key.as_str().is_some_and(|key| inputs.contains_key(key)))
        {
            return Err(EditionCurationError::MissingReference);
        }
    }
    if stage == "concept-candidate" && role == "legacy-output-template" {
        let selected_template = published_value(template.raw, template.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let claims = selected_template["claim_specs"]
            .as_object()
            .ok_or(EditionCurationError::InvalidSchema)?;
        for rule in recipe["candidate_signals"]["claim_rules"]
            .as_array()
            .ok_or(EditionCurationError::InvalidSchema)?
        {
            for claim in rule["claims"]
                .as_array()
                .ok_or(EditionCurationError::InvalidSchema)?
            {
                if !claim
                    .as_str()
                    .is_some_and(|claim| claims.contains_key(claim))
                {
                    return Err(EditionCurationError::MissingReference);
                }
            }
        }
    }
    Ok(())
}

/// Bind a consumer and its index owner to the same selected index plan.
pub fn validate_source_index_plans(
    raw: &[u8],
    stage: &str,
    consumer_plan: SelectedDocument<'_>,
    index_plan: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<String, EditionCurationError> {
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let binding = &recipe["source_index_binding"];
    let owner_stage = binding["index_recipe_stage"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    validate_recipe_plan(raw, stage, consumer_plan, schema)?;
    validate_recipe_plan(raw, owner_stage, index_plan, schema)?;
    let owner = published_value(index_plan.raw, index_plan.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    let pointer = binding["index_path_pointer"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let path = owner
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or(EditionCurationError::MissingReference)?;
    tos_foundation::RelativePath::parse(path).map_err(|_| EditionCurationError::UnsafePath)?;
    if binding["consumer_input"]["kind"] == "direct-plan" {
        let consumer = published_value(consumer_plan.raw, consumer_plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let pointer = binding["consumer_input"]["input_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if consumer
            .pointer(pointer)
            .and_then(|input| input["ref"].as_str())
            != Some(path)
        {
            return Err(EditionCurationError::BindingMismatch);
        }
    }
    Ok(path.to_owned())
}

/// Bind the selected review scope to its plan and historical candidate template.
pub fn validate_review_sources(
    raw: &[u8],
    plan: SelectedDocument<'_>,
    template: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    let stage = "concept-review";
    validate_recipe_plan(raw, stage, plan, schema)?;
    validate_recipe_template(raw, stage, "legacy-output-template", template, schema)?;
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let scope = &recipe["review_scope"];
    let plan =
        published_value(plan.raw, plan.raw.len().max(1)).map_err(EditionCurationError::Schema)?;
    let template = published_value(template.raw, template.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    let array = |document: &Value, key: &str| -> Result<Vec<Value>, EditionCurationError> {
        let pointer = scope[key]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        document
            .pointer(pointer)
            .and_then(Value::as_array)
            .cloned()
            .ok_or(EditionCurationError::MissingReference)
    };
    let gaps = array(&plan, "gap_refs_plan_pointer")?;
    let count_pointer = scope["gap_count_plan_pointer"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    if plan.pointer(count_pointer).and_then(Value::as_u64) != Some(gaps.len() as u64) {
        return Err(EditionCurationError::BindingMismatch);
    }
    let template_gaps = array(&template, "gaps_template_pointer")?;
    let template_gap_refs: Vec<_> = template_gaps
        .iter()
        .map(|row| row["alignment_ref"].clone())
        .collect();
    let readings = array(&plan, "reading_refs_plan_pointer")?;
    let selected_readings: Vec<_> = recipe["speaker_selection"]["readings"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
        .iter()
        .map(|row| row["reading_ref"].clone())
        .collect();
    let mut axes = array(&plan, "primary_axes_plan_pointer")?;
    let controls = array(&plan, "control_axes_plan_pointer")?;
    let blocked_pointer = scope["blocked_axis_plan_pointer"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let blocked = plan
        .pointer(blocked_pointer)
        .ok_or(EditionCurationError::MissingReference)?;
    if blocked.as_str().is_none() || !controls.contains(blocked) {
        return Err(EditionCurationError::BindingMismatch);
    }
    axes.extend(controls);
    if gaps != template_gap_refs
        || readings != selected_readings
        || readings != array(&template, "readings_template_pointer")?
        || axes != array(&template, "axes_template_pointer")?
    {
        return Err(EditionCurationError::BindingMismatch);
    }
    for entries in [&gaps, &readings, &axes] {
        let mut seen = BTreeSet::new();
        for entry in entries {
            let id = entry.as_str().ok_or(EditionCurationError::InvalidSchema)?;
            if !seen.insert(id) {
                return Err(EditionCurationError::Duplicate("review scope reference"));
            }
        }
    }
    Ok(())
}

/// Verify a retained artifact selected from its exact plan or authored path.
pub fn validate_retained_artifact(
    raw: &[u8],
    stage: &str,
    role: &str,
    plan: SelectedDocument<'_>,
    artifact: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    validate_recipe_plan(raw, stage, plan, schema)?;
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let entry = recipe["retained_artifacts"]
        .as_array()
        .ok_or(EditionCurationError::MissingReference)?
        .iter()
        .find(|entry| entry["role"] == role)
        .ok_or(EditionCurationError::MissingReference)?;
    let digest = entry["sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    if let Some(reference) = entry["repository_path"].as_str() {
        return verify_selected_bytes(artifact, reference, digest);
    }
    let pointer = entry["plan_pointer"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    let selected_plan =
        published_value(plan.raw, plan.raw.len().max(1)).map_err(EditionCurationError::Schema)?;
    let reference = selected_plan
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or(EditionCurationError::MissingReference)?;
    verify_selected_bytes(artifact, reference, digest)
}

/// Select a digest value from the exact plan or a verified retained artifact.
pub fn select_retained_digest(
    raw: &[u8],
    stage: &str,
    role: &str,
    plan: SelectedDocument<'_>,
    source: Option<SelectedDocument<'_>>,
    schema: &SchemaBackendProbe,
) -> Result<String, EditionCurationError> {
    validate_recipe_plan(raw, stage, plan, schema)?;
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let entry = recipe["retained_artifacts"]
        .as_array()
        .ok_or(EditionCurationError::MissingReference)?
        .iter()
        .find(|entry| entry["role"] == role)
        .ok_or(EditionCurationError::MissingReference)?;
    let expected = entry["sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let selected = if let Some(pointer) = entry["plan_value_pointer"].as_str() {
        let plan_value = published_value(plan.raw, plan.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        plan_value.pointer(pointer).cloned()
    } else if let Some(locator) = entry.get("artifact_value") {
        let source = source.ok_or(EditionCurationError::MissingReference)?;
        let source_role = locator["source_role"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        validate_retained_artifact(raw, stage, source_role, plan, source, schema)?;
        let parent = published_value(source.raw, source.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let pointer = locator["json_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        parent.pointer(pointer).cloned()
    } else {
        return Err(EditionCurationError::MissingReference);
    };
    if selected.as_ref().and_then(Value::as_str) != Some(expected) {
        return Err(EditionCurationError::BindingMismatch);
    }
    Ok(expected.to_owned())
}

/// Select the exact frozen manifest and check its member metadata.
pub fn select_frozen_manifest(
    raw: &[u8],
    stage: &str,
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<Value, EditionCurationError> {
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let descriptor = &recipe["frozen_manifest"];
    let reference = descriptor["ref"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    let digest = descriptor["sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    verify_selected_bytes(selected, reference, digest)?;
    let manifest = published_value(selected.raw, selected.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    if manifest["schema_version"] != descriptor["schema_version"] {
        return Err(EditionCurationError::BindingMismatch);
    }
    let root = descriptor["output_root"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let prefix = format!("{root}/");
    let members = manifest["generated_outputs"]
        .as_object()
        .ok_or(EditionCurationError::InvalidSchema)?;
    for (path, member) in members {
        tos_foundation::RelativePath::parse(path).map_err(|_| EditionCurationError::UnsafePath)?;
        if !path.starts_with(&prefix) || path == reference {
            return Err(EditionCurationError::BindingMismatch);
        }
        let digest = member["sha256"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let parsed = tos_foundation::Digest256::from_hex(digest)
            .map_err(|_| EditionCurationError::InvalidSchema)?;
        if parsed.to_hex() != digest || member["byte_size"].as_u64().is_none() {
            return Err(EditionCurationError::InvalidSchema);
        }
    }
    Ok(manifest)
}

/// Verify one selected member against a selected frozen manifest.
pub fn validate_frozen_manifest_member(
    raw: &[u8],
    stage: &str,
    manifest: SelectedDocument<'_>,
    member: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    tos_foundation::RelativePath::parse(member.reference)
        .map_err(|_| EditionCurationError::UnsafePath)?;
    let selected = select_frozen_manifest(raw, stage, manifest, schema)?;
    let entry = selected["generated_outputs"]
        .get(member.reference)
        .ok_or(EditionCurationError::MissingReference)?;
    if entry["byte_size"].as_u64() != Some(member.raw.len() as u64) {
        return Err(EditionCurationError::BindingMismatch);
    }
    let digest = entry["sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    verify_selected_bytes(member, member.reference, digest)
}

/// Select one named field from a manifest-bound historical coverage record.
pub fn select_frozen_coverage_field(
    raw: &[u8],
    stage: &str,
    role: &str,
    manifest: SelectedDocument<'_>,
    coverage: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<Value, EditionCurationError> {
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let descriptor = &recipe["retained_coverage"];
    let root = recipe["frozen_manifest"]["output_root"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    let filename = descriptor["filename"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    if coverage.reference != format!("{root}/{filename}") {
        return Err(EditionCurationError::BindingMismatch);
    }
    let field = descriptor["coverage_fields"]
        .as_array()
        .ok_or(EditionCurationError::MissingReference)?
        .iter()
        .find(|field| field["role"] == role)
        .ok_or(EditionCurationError::MissingReference)?;
    validate_frozen_manifest_member(raw, stage, manifest, coverage, schema)?;
    let data = published_value(coverage.raw, coverage.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    let pointer = field["json_pointer"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    data.pointer(pointer)
        .cloned()
        .ok_or(EditionCurationError::MissingReference)
}

/// Bind the default request to its selected bytes and request key.
pub fn validate_default_request(
    raw: &[u8],
    stage: &str,
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let layout = &recipe["output_layout"];
    let reference = layout["default_request_ref"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    let digest = layout["default_request_sha256"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    verify_selected_bytes(selected, reference, digest)?;
    let request = published_value(selected.raw, selected.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    if request["request_key"] != layout["default_request_key"] {
        return Err(EditionCurationError::BindingMismatch);
    }
    if let Some(rules) = recipe["acceptance_probe_rules"].as_array() {
        for rule in rules {
            let pointer = rule["request_pointer"]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)?;
            if request.pointer(pointer).and_then(Value::as_str).is_none() {
                return Err(EditionCurationError::MissingReference);
            }
        }
    }
    Ok(())
}

/// Verify exact selected bytes against a frozen output's path and digest.
pub fn validate_frozen_output_fixity(
    raw: &[u8],
    stage: &str,
    role: &str,
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let output = recipe["frozen_outputs"]
        .as_array()
        .ok_or(EditionCurationError::MissingReference)?
        .iter()
        .find(|output| output["role"] == role)
        .ok_or(EditionCurationError::MissingReference)?;
    let reference = output["repository_path"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let digest = output["expected_sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    verify_selected_bytes(selected, reference, digest)
}

/// Select the private payload digest named by a caller-selected predecessor manifest.
pub fn select_recipe_input_digest(
    raw: &[u8],
    stage: &str,
    role: &str,
    manifest: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<String, EditionCurationError> {
    tos_foundation::RelativePath::parse(manifest.reference)
        .map_err(|_| EditionCurationError::UnsafePath)?;
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let input = recipe["input_sources"]
        .as_array()
        .ok_or(EditionCurationError::MissingReference)?
        .iter()
        .find(|input| input["role"] == role)
        .ok_or(EditionCurationError::MissingReference)?;
    if input["manifest_ref"] != manifest.reference {
        return Err(EditionCurationError::BindingMismatch);
    }
    let data =
        published_value(manifest.raw, 8 * 1024 * 1024).map_err(EditionCurationError::Schema)?;
    let mut matches = data["private_artifacts"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
        .iter()
        .filter(|item| item["ref"] == input["private_payload_ref"]);
    let item = matches
        .next()
        .ok_or(EditionCurationError::MissingReference)?;
    if matches.next().is_some() {
        return Err(EditionCurationError::AmbiguousReference);
    }
    let digest = item["sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let parsed = tos_foundation::Digest256::from_hex(digest)
        .map_err(|_| EditionCurationError::InvalidSchema)?;
    if parsed.to_hex() != digest {
        return Err(EditionCurationError::InvalidSchema);
    }
    Ok(digest.to_owned())
}

/// Validate the selected historical receipt and its frozen identity.
pub fn validate_selected_historical_receipt(
    raw: &[u8],
    stage: &str,
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    selected_historical_receipt(raw, stage, selected, schema).map(|_| ())
}

fn selected_historical_receipt(
    raw: &[u8],
    stage: &str,
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<(Value, Value), EditionCurationError> {
    tos_foundation::RelativePath::parse(selected.reference)
        .map_err(|_| EditionCurationError::UnsafePath)?;
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let reference = recipe["receipt_ref"]
        .as_str()
        .ok_or(EditionCurationError::MissingReference)?;
    let digest = recipe["historical_receipt_sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    verify_selected_bytes(selected, reference, digest)?;
    let receipt = published_value(selected.raw, selected.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    if receipt["schema_version"] != recipe["receipt_identity"]["schema_version"]
        || receipt["receipt_id"] != recipe["receipt_identity"]["historical_id"]
    {
        return Err(EditionCurationError::BindingMismatch);
    }
    for field in recipe["historical_receipt_fields"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
    {
        let pointer = field["json_pointer"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if receipt.pointer(pointer).is_none() {
            return Err(EditionCurationError::MissingReference);
        }
    }
    Ok((value, receipt))
}

/// Select one field from the exact historical receipt named by the recipe.
pub fn select_historical_receipt_field(
    raw: &[u8],
    stage: &str,
    role: &str,
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<Value, EditionCurationError> {
    let (value, receipt) = selected_historical_receipt(raw, stage, selected, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let pointer = recipe["historical_receipt_fields"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
        .iter()
        .find(|field| field["role"] == role)
        .and_then(|field| field["json_pointer"].as_str())
        .ok_or(EditionCurationError::MissingReference)?;
    receipt
        .pointer(pointer)
        .cloned()
        .ok_or(EditionCurationError::MissingReference)
}

/// Select a historical JSON field from exact, curation-bound output bytes.
pub fn select_historical_field(
    raw: &[u8],
    stage: &str,
    role: &str,
    selected: SelectedDocument<'_>,
    schema: &SchemaBackendProbe,
) -> Result<Value, EditionCurationError> {
    tos_foundation::RelativePath::parse(selected.reference)
        .map_err(|_| EditionCurationError::UnsafePath)?;
    let value = document(raw, schema)?;
    let recipe = selected_recipe(&value, stage)?;
    let field = recipe["historical_fields"]
        .as_array()
        .ok_or(EditionCurationError::MissingReference)?
        .iter()
        .find(|field| field["role"] == role)
        .ok_or(EditionCurationError::MissingReference)?;
    let output = recipe["frozen_outputs"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
        .iter()
        .find(|output| output["role"] == field["output_role"])
        .ok_or(EditionCurationError::MissingReference)?;
    let reference = output["repository_path"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let digest = output["expected_sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    verify_selected_bytes(selected, reference, digest)?;
    let historical = published_value(selected.raw, selected.raw.len().max(1))
        .map_err(EditionCurationError::Schema)?;
    if historical["schema_version"] != output["schema_version"] {
        return Err(EditionCurationError::BindingMismatch);
    }
    let pointer = field["json_pointer"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    historical
        .pointer(pointer)
        .cloned()
        .ok_or(EditionCurationError::MissingReference)
}

fn verify_selected_bytes(
    selected: SelectedDocument<'_>,
    reference: &str,
    digest: &str,
) -> Result<(), EditionCurationError> {
    tos_foundation::RelativePath::parse(selected.reference)
        .map_err(|_| EditionCurationError::UnsafePath)?;
    if selected.reference != reference {
        return Err(EditionCurationError::BindingMismatch);
    }
    if tos_foundation::Digest256::of_bytes(selected.raw).to_hex() != digest {
        return Err(EditionCurationError::DigestMismatch);
    }
    Ok(())
}

fn selected_recipe<'a>(value: &'a Value, stage: &str) -> Result<&'a Value, EditionCurationError> {
    let mut matching = value["recipes"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
        .iter()
        .filter(|recipe| recipe["stage"] == stage);
    let recipe = matching
        .next()
        .ok_or(EditionCurationError::MissingReference)?;
    if matching.next().is_some() {
        return Err(EditionCurationError::AmbiguousReference);
    }
    Ok(recipe)
}

fn document(raw: &[u8], schema: &SchemaBackendProbe) -> Result<Value, EditionCurationError> {
    let value = published_value(raw, raw.len().max(1)).map_err(EditionCurationError::Schema)?;
    if !schema
        .is_valid_value(SCHEMA_URI, &value)
        .map_err(EditionCurationError::Schema)?
    {
        return Err(EditionCurationError::InvalidSchema);
    }
    check_rules(&value)?;
    Ok(value)
}

/// Bind a curation to its selected manifest and explicit cross-item recipes.
/// Callers select the exact bytes through their source-custody interface.
pub fn validate_bindings(
    current: SelectedDocument<'_>,
    manifest: SelectedDocument<'_>,
    edition: SelectedDocument<'_>,
    references: &[SelectedCuration<'_>],
    schema: &SchemaBackendProbe,
) -> Result<(), EditionCurationError> {
    fn path(value: &str) -> Result<(), EditionCurationError> {
        tos_foundation::RelativePath::parse(value)
            .map(|_| ())
            .map_err(|_| EditionCurationError::UnsafePath)
    }
    fn bind_manifest(
        current: SelectedDocument<'_>,
        selected: &Value,
        manifest: SelectedDocument<'_>,
        edition: SelectedDocument<'_>,
    ) -> Result<(), EditionCurationError> {
        path(current.reference)?;
        path(manifest.reference)?;
        path(edition.reference)?;
        let manifest_relative = selected["manifest"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        path(manifest_relative)?;
        let (parent, name) = current
            .reference
            .rsplit_once('/')
            .ok_or(EditionCurationError::UnsafePath)?;
        if name != "edition-curation.v1.json"
            || manifest.reference != format!("{parent}/{manifest_relative}")
        {
            return Err(EditionCurationError::BindingMismatch);
        }
        let (items, _) = parent
            .rsplit_once('/')
            .ok_or(EditionCurationError::UnsafePath)?;
        let (edition_dir, items_name) = items
            .rsplit_once('/')
            .ok_or(EditionCurationError::UnsafePath)?;
        if items_name != "items" || edition.reference != format!("{edition_dir}/edition.json") {
            return Err(EditionCurationError::BindingMismatch);
        }
        let item = published_value(manifest.raw, manifest.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        let edition = published_value(edition.raw, edition.raw.len().max(1))
            .map_err(EditionCurationError::Schema)?;
        if item["item_id"] != selected["owner_item_ref"]
            || item["embodiment_ref"] != selected["edition_ref"]
            || edition["record_type"] != "edition"
            || edition["record_id"] != selected["edition_ref"]
        {
            return Err(EditionCurationError::BindingMismatch);
        }
        Ok(())
    }
    let selected = document(current.raw, schema)?;
    bind_manifest(current, &selected, manifest, edition)?;
    let mut ids = BTreeSet::new();
    ids.insert(
        selected["curation_id"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?
            .to_owned(),
    );
    let mut chosen = BTreeMap::new();
    for binding in references {
        let reference = binding.curation;
        path(reference.reference)?;
        if reference.reference == current.reference || chosen.contains_key(reference.reference) {
            return Err(EditionCurationError::Duplicate("curation reference"));
        }
        let value = document(reference.raw, schema)?;
        bind_manifest(reference, &value, binding.manifest, binding.edition)?;
        let id = value["curation_id"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if !ids.insert(id.to_owned()) {
            return Err(EditionCurationError::Duplicate("curation id"));
        }
        chosen.insert(reference.reference, value);
    }
    let owner = selected["owner_item_ref"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    for recipe in selected["recipes"]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
    {
        if recipe["stage"] == "source-passages" {
            let locators = recipe["locators"]
                .as_array()
                .ok_or(EditionCurationError::InvalidSchema)?;
            for route in recipe["work_routes"]
                .as_array()
                .ok_or(EditionCurationError::InvalidSchema)?
            {
                let reference = route["marker_curation_ref"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                path(reference)?;
                let target = if reference == current.reference {
                    &selected
                } else {
                    chosen
                        .get(reference)
                        .ok_or(EditionCurationError::MissingReference)?
                };
                selected_recipe(target, "source-passage-marker-returns")?;
                let role = route["default_content_witness"]["item_role"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let directory = locators
                    .iter()
                    .find(|locator| locator["role"] == role)
                    .and_then(|locator| locator["repository_path"].as_str())
                    .ok_or(EditionCurationError::MissingReference)?;
                if reference != format!("{directory}/edition-curation.v1.json") {
                    return Err(EditionCurationError::BindingMismatch);
                }
            }
            let profile = &recipe["marker_profile"];
            let reference = profile["normalization_curation_ref"]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)?;
            let target = if reference == current.reference {
                &selected
            } else {
                chosen
                    .get(reference)
                    .ok_or(EditionCurationError::MissingReference)?
            };
            let stage = profile["normalization_stage"]
                .as_str()
                .ok_or(EditionCurationError::InvalidSchema)?;
            if !selected_recipe(target, stage)?["ocr_substitutions"].is_array() {
                return Err(EditionCurationError::MissingReference);
            }
            continue;
        }
        if recipe["stage"] != "recipe-reference" {
            continue;
        }
        let reference = recipe["curation_ref"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        path(reference)?;
        let target = chosen
            .get(reference)
            .ok_or(EditionCurationError::MissingReference)?;
        let target_owner = target["owner_item_ref"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        let matching = target["recipes"]
            .as_array()
            .ok_or(EditionCurationError::InvalidSchema)?
            .iter()
            .filter(|candidate| {
                candidate["stage"] == recipe["recipe_stage"]
                    && candidate["source_item_refs"]
                        .as_array()
                        .is_some_and(|items| {
                            items.iter().any(|item| item.as_str() == Some(owner))
                                && items.iter().any(|item| item.as_str() == Some(target_owner))
                        })
            })
            .count();
        match matching {
            1 => {}
            0 => return Err(EditionCurationError::BindingMismatch),
            _ => return Err(EditionCurationError::AmbiguousReference),
        }
    }
    Ok(())
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum MarkerPart {
    Text(String),
    Number(bool, String, BigInt),
}

fn check_replacements(value: &Value) -> Result<(), EditionCurationError> {
    let mut seen = BTreeSet::new();
    for replacement in value
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
    {
        let from = replacement["from"]
            .as_str()
            .ok_or(EditionCurationError::InvalidSchema)?;
        if !seen.insert(from) {
            return Err(EditionCurationError::Duplicate("character replacement"));
        }
    }
    Ok(())
}

fn character_ranges(
    profile: &Value,
    field: &'static str,
) -> Result<Vec<(char, char)>, EditionCurationError> {
    let mut result = Vec::new();
    for range in profile[field]
        .as_array()
        .ok_or(EditionCurationError::InvalidSchema)?
    {
        let start = range["start"]
            .as_str()
            .and_then(|s| s.chars().next())
            .ok_or(EditionCurationError::InvalidSchema)?;
        let end = range["end"]
            .as_str()
            .and_then(|s| s.chars().next())
            .ok_or(EditionCurationError::InvalidSchema)?;
        if start > end
            || result
                .last()
                .is_some_and(|(_, previous)| *previous >= start)
        {
            return Err(EditionCurationError::InvalidRange(field));
        }
        result.push((start, end));
    }
    Ok(result)
}

fn check_rules(value: &Value) -> Result<(), EditionCurationError> {
    match value {
        Value::Object(object) => {
            for (field, key, label) in [
                ("private_example_forms", "name", "private example name"),
                ("gap_proposals", "gap_code", "gap proposal code"),
                (
                    "acceptance_probe_rules",
                    "output_field",
                    "acceptance probe output field",
                ),
            ] {
                if let Some(entries) = object.get(field) {
                    let mut seen = BTreeSet::new();
                    for entry in entries
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?
                    {
                        let name = entry[key]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if !seen.insert(name) {
                            return Err(EditionCurationError::Duplicate(label));
                        }
                    }
                }
            }
            if object.get("stage").and_then(Value::as_str) == Some("reading-workbench") {
                let profile = object
                    .get("unicode_profile")
                    .and_then(Value::as_str)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                tos_foundation::UnicodeProfile::from_profile(profile)
                    .map_err(|_| EditionCurationError::InvalidSchema)?;
            }
            if let Some(profile) = object.get("provider_profile") {
                character_ranges(profile, "form_character_ranges")?;
                check_replacements(&profile["stem_replacements"])?;
                let role = profile["coverage_field_role"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let fields = object["retained_coverage"]["coverage_fields"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if !fields.iter().any(|field| field["role"] == role) {
                    return Err(EditionCurationError::MissingReference);
                }
            }
            if let Some(replacements) = object.get("search_replacements") {
                check_replacements(replacements)?;
            }
            if object.get("stage").and_then(Value::as_str)
                == Some("witness-structure-correspondence")
            {
                let matching = object
                    .get("matching")
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let casefold = matching["casefold_unicode_profile"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                tos_foundation::UnicodeProfile::from_profile(casefold)
                    .map_err(|_| EditionCurationError::InvalidSchema)?;
                let (major, minor, patch) = unicode_normalization::UNICODE_VERSION;
                if matching["nfkc_unicode_version"] != format!("{major}.{minor}.{patch}") {
                    return Err(EditionCurationError::BindingMismatch);
                }
                let alphabet = matching["token_alphabet"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut seen = BTreeSet::new();
                if alphabet.chars().any(|ch| !seen.insert(ch)) {
                    return Err(EditionCurationError::Duplicate("token character"));
                }
            }
            if let Some(profile) = object.get("tei_text_profile") {
                let unicode = profile["whitespace_profile"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                tos_foundation::UnicodeProfile::from_profile(unicode)
                    .map_err(|_| EditionCurationError::InvalidSchema)?;
                let markers = profile["formatting_whitespace_markers"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut seen = BTreeSet::new();
                if markers.chars().any(|ch| !seen.insert(ch)) {
                    return Err(EditionCurationError::Duplicate(
                        "formatting whitespace marker",
                    ));
                }
            }
            if let Some(profile) = object.get("line_join_profile") {
                let whitespace = profile["whitespace_profile"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                tos_foundation::UnicodeProfile::from_profile(whitespace)
                    .map_err(|_| EditionCurationError::InvalidSchema)?;
                for text in [
                    profile["markers"].as_str(),
                    object
                        .get("token_apostrophe_characters")
                        .and_then(Value::as_str),
                    object
                        .get("join_allowed_characters")
                        .and_then(Value::as_str),
                ] {
                    let text = text.ok_or(EditionCurationError::InvalidSchema)?;
                    let mut seen = BTreeSet::new();
                    if text.chars().any(|ch| !seen.insert(ch)) {
                        return Err(EditionCurationError::Duplicate("token character"));
                    }
                }
            }
            if let Some(profile) = object.get("text_classification") {
                let whitespace = profile["whitespace_profile"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                tos_foundation::UnicodeProfile::from_profile(whitespace)
                    .map_err(|_| EditionCurationError::InvalidSchema)?;
                let ranges = character_ranges(profile, "letter_ranges")?;
                for field in ["additional_letters", "dash_characters"] {
                    let text = profile[field]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let mut seen = BTreeSet::new();
                    if text.chars().any(|ch| !seen.insert(ch)) {
                        return Err(EditionCurationError::Duplicate(
                            "text classification character",
                        ));
                    }
                }
                let extra = profile["additional_letters"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if extra.chars().any(|ch| {
                    ranges
                        .iter()
                        .any(|(start, end)| (*start..=*end).contains(&ch))
                }) {
                    return Err(EditionCurationError::Duplicate(
                        "text classification character",
                    ));
                }
            }
            if let Some(profile) = object.get("guillemet_ocr_profile") {
                let letters = character_ranges(profile, "letter_ranges")?;
                let after = character_ranges(profile, "after_whitespace_ranges")?;
                let extra = profile["additional_letters"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let contains = |ch: char| {
                    extra.contains(ch)
                        || letters
                            .iter()
                            .any(|(start, end)| (*start..=*end).contains(&ch))
                };
                for field in [
                    "additional_letters",
                    "before_whitespace_characters",
                    "after_whitespace_additional_letters",
                ] {
                    let text = profile[field]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let mut seen = BTreeSet::new();
                    if text.chars().any(|ch| !seen.insert(ch)) {
                        return Err(EditionCurationError::Duplicate("OCR character"));
                    }
                }
                let after_extra = profile["after_whitespace_additional_letters"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if extra.chars().any(|ch| {
                    letters
                        .iter()
                        .any(|(start, end)| (*start..=*end).contains(&ch))
                }) || after_extra.chars().any(|ch| {
                    after
                        .iter()
                        .any(|(start, end)| (*start..=*end).contains(&ch))
                }) {
                    return Err(EditionCurationError::Duplicate("OCR character"));
                }
                if profile["before_whitespace_characters"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?
                    .chars()
                    .any(|ch| !contains(ch))
                    || profile["after_whitespace_additional_letters"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?
                        .chars()
                        .any(|ch| !contains(ch))
                    || after.iter().any(|(start, end)| {
                        !letters
                            .iter()
                            .any(|(left, right)| left <= start && end <= right)
                    })
                    || !object
                        .get("quote_pairs")
                        .and_then(Value::as_array)
                        .is_some_and(|pairs| {
                            pairs.iter().any(|pair| pair["close"] == profile["marker"])
                        })
                {
                    return Err(EditionCurationError::MissingReference);
                }
            }
            if let Some(context) = object.get("quote_context") {
                if context["continuation_uses_ocr_marker"] == true
                    && !object.contains_key("guillemet_ocr_profile")
                {
                    return Err(EditionCurationError::MissingReference);
                }
                let pairs = object
                    .get("quote_pairs")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::MissingReference)?;
                for field in ["unmatched_close_marker", "literal_apostrophe_marker"] {
                    if !pairs.iter().any(|pair| {
                        pair["open"] == context[field] || pair["close"] == context[field]
                    }) {
                        return Err(EditionCurationError::MissingReference);
                    }
                }
                for field in ["leading_prefix_characters", "closing_following_characters"] {
                    let text = context[field]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let mut seen = BTreeSet::new();
                    if text.chars().any(|ch| !seen.insert(ch)) {
                        return Err(EditionCurationError::Duplicate("quote context character"));
                    }
                }
            }
            if let Some(profile) = object.get("residue_profile") {
                for field in ["frequency_buckets", "length_buckets"] {
                    let buckets = profile[field]
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let mut next = Some(1_u64);
                    for (index, bucket) in buckets.iter().enumerate() {
                        let minimum = bucket["minimum"]
                            .as_u64()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        let maximum = bucket["maximum"].as_u64();
                        if next != Some(minimum) || maximum.is_some_and(|maximum| maximum < minimum)
                        {
                            return Err(EditionCurationError::InvalidRange(field));
                        }
                        next = maximum.and_then(|maximum| maximum.checked_add(1));
                        if index + 1 < buckets.len() && next.is_none() {
                            return Err(EditionCurationError::InvalidRange(field));
                        }
                        if index + 1 == buckets.len() && maximum.is_some() {
                            return Err(EditionCurationError::InvalidRange(field));
                        }
                    }
                }
            }
            if let Some(recipes) = object.get("recipes").and_then(Value::as_array) {
                for recipe in recipes {
                    if let Some(binding) = recipe.get("source_index_binding") {
                        let stage = binding["index_recipe_stage"]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        let matches = recipes
                            .iter()
                            .filter(|candidate| {
                                candidate["stage"] == stage
                                    && candidate["locators"].as_array().is_some_and(|locators| {
                                        locators
                                            .iter()
                                            .any(|locator| locator["role"] == "index_plan")
                                    })
                            })
                            .count();
                        match matches {
                            1 => {}
                            0 => return Err(EditionCurationError::MissingReference),
                            _ => return Err(EditionCurationError::AmbiguousReference),
                        }
                    }
                }
                for recipe in recipes
                    .iter()
                    .filter(|recipe| recipe["stage"] == "target-passages")
                {
                    if let Some(profile) = recipe.get("marker_profile") {
                        let stage = profile["normalization_stage"]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        let matches = recipes
                            .iter()
                            .filter(|candidate| {
                                candidate["stage"] == stage
                                    && candidate["candidate_profile"]["kind"] == "pdf-bbox"
                            })
                            .count();
                        match matches {
                            1 => {}
                            0 => return Err(EditionCurationError::MissingReference),
                            _ => return Err(EditionCurationError::AmbiguousReference),
                        }
                    }
                }
            }
            if let Some(profile) = object
                .get("marker_profile")
                .filter(|_| object.get("stage").and_then(Value::as_str) == Some("target-passages"))
            {
                if !matches!(
                    compare_numbers(&profile["minimum_x"], &profile["maximum_x"]),
                    Some(Ordering::Less | Ordering::Equal)
                ) {
                    return Err(EditionCurationError::InvalidRange("marker_profile"));
                }
                let mut seen = BTreeSet::new();
                for item in profile["overrides"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let page = item["page"]
                        .as_u64()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let unit = item["unit_key"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !seen.insert((page, unit)) {
                        return Err(EditionCurationError::Duplicate("marker override"));
                    }
                }
            }
            if let Some(profile) = object
                .get("marker_profile")
                .filter(|_| object.get("stage").and_then(Value::as_str) == Some("source-passages"))
            {
                let refs = object["marker_curation_refs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if !refs.contains(&profile["normalization_curation_ref"]) {
                    return Err(EditionCurationError::MissingReference);
                }
                for (lower, upper) in [
                    ("pdf_body_y_minimum", "pdf_body_y_maximum_exclusive"),
                    ("abbyy_body_y_minimum", "abbyy_body_y_maximum_exclusive"),
                    ("pdf_marker_x_minimum", "pdf_marker_x_maximum"),
                    ("djvu_marker_x_minimum", "djvu_marker_x_maximum"),
                ] {
                    if !matches!(
                        compare_numbers(&profile[lower], &profile[upper]),
                        Some(Ordering::Less)
                    ) {
                        return Err(EditionCurationError::InvalidRange("source marker profile"));
                    }
                }
                let routes = object["work_routes"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut seen = BTreeSet::new();
                for entry in profile["abbyy_overrides"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let work = entry["work_ref"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let unit = entry["unit_key"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !routes.iter().any(|route| route["work_ref"] == work) {
                        return Err(EditionCurationError::MissingReference);
                    }
                    if !seen.insert((work, unit)) {
                        return Err(EditionCurationError::Duplicate("source marker override"));
                    }
                }
            }
            if let Some(sequence) = object.get("unit_sequence") {
                let first = sequence["first_number"]
                    .as_u64()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let last = sequence["last_number"]
                    .as_u64()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let repeats = sequence["repeat_after"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if first > last
                    || repeats
                        .windows(2)
                        .any(|pair| pair[0].as_u64() >= pair[1].as_u64())
                    || repeats
                        .iter()
                        .any(|number| number.as_u64().is_none_or(|n| n < first || n > last))
                    || last
                        .checked_sub(first)
                        .and_then(|n| n.checked_add(1))
                        .and_then(|n| n.checked_add(repeats.len() as u64))
                        != sequence["expected_count"].as_u64()
                {
                    return Err(EditionCurationError::InvalidRange("unit_sequence"));
                }
                let required = object["required_ocr_keys"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let overrides = object["page_overrides"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let profile = &object["candidate_profile"];
                for evidence in object["ocr_evidence"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    if !required.contains(&evidence["unit_key"])
                        || !matches!(
                            compare_numbers(&profile["first_page"], &evidence["pdf_page"]),
                            Some(Ordering::Less | Ordering::Equal)
                        )
                        || !matches!(
                            compare_numbers(&evidence["pdf_page"], &profile["last_page"]),
                            Some(Ordering::Less | Ordering::Equal)
                        )
                    {
                        return Err(EditionCurationError::MissingReference);
                    }
                }
                if required
                    .iter()
                    .any(|key| overrides.iter().any(|entry| entry["key"] == *key))
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
                if object["source_map_required_keys"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                    .len()
                    > 0
                    && (object
                        .get("source_map_sha256")
                        .and_then(Value::as_str)
                        .is_none()
                        || !object["locators"]
                            .as_array()
                            .ok_or(EditionCurationError::InvalidSchema)?
                            .iter()
                            .any(|locator| {
                                locator["role"] == "source_map_path"
                                    && locator["repository_path"].is_string()
                            }))
                {
                    return Err(EditionCurationError::MissingReference);
                }
                let suffix = sequence["repeat_suffix"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for field in [
                    "page_overrides",
                    "gap_review_keys",
                    "ocr_disambiguation_keys",
                    "repeated_number_review_keys",
                    "required_ocr_keys",
                    "ocr_evidence",
                ] {
                    if let Some(entries) = object.get(field).and_then(Value::as_array) {
                        for entry in entries {
                            let key = if field == "page_overrides" {
                                entry["key"].as_str()
                            } else if field == "ocr_evidence" {
                                entry["unit_key"].as_str()
                            } else {
                                entry.as_str()
                            }
                            .ok_or(EditionCurationError::InvalidSchema)?;
                            let (digits, repeated) =
                                if key.bytes().all(|byte| byte.is_ascii_digit()) {
                                    (key, false)
                                } else {
                                    (
                                        key.strip_suffix(suffix)
                                            .ok_or(EditionCurationError::MissingReference)?,
                                        true,
                                    )
                                };
                            let number = digits
                                .parse::<u64>()
                                .map_err(|_| EditionCurationError::MissingReference)?;
                            if number.to_string() != digits
                                || number < first
                                || number > last
                                || (repeated
                                    && !repeats.iter().any(|value| value.as_u64() == Some(number)))
                            {
                                return Err(EditionCurationError::MissingReference);
                            }
                        }
                    }
                }
            }
            if let Some(profile) = object
                .get("candidate_profile")
                .filter(|value| value.is_object())
            {
                if let Some(normalization) = profile["normalization"].as_array() {
                    let mut characters = BTreeSet::new();
                    for substitution in normalization {
                        let from = substitution["from"]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if !characters.insert(from) {
                            return Err(EditionCurationError::Duplicate("normalization character"));
                        }
                    }
                }
                for (lower, upper) in [
                    ("first_page", "last_page"),
                    ("minimum_top", "maximum_top"),
                    ("minimum_x", "maximum_x"),
                    ("minimum_y", "maximum_y"),
                    ("minimum_characters", "maximum_characters"),
                    ("minimum_number", "maximum_number"),
                ] {
                    if let (Some(a), Some(b)) = (profile.get(lower), profile.get(upper)) {
                        if !matches!(
                            compare_numbers(a, b),
                            Some(Ordering::Less | Ordering::Equal)
                        ) {
                            return Err(EditionCurationError::InvalidRange("candidate_profile"));
                        }
                    }
                }
                if profile.get("source_page_count").is_some()
                    && !matches!(
                        compare_numbers(&profile["last_page"], &profile["source_page_count"]),
                        Some(Ordering::Less | Ordering::Equal)
                    )
                {
                    return Err(EditionCurationError::InvalidRange("candidate_profile"));
                }
            }
            if let Some(routes) = object.get("nonintersecting_routes") {
                let mut seen = BTreeSet::new();
                for route in routes
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let candidate = route["frozen_page_candidate_id"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let unit = route["qualified_unit_key"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !seen.insert((candidate, unit)) {
                        return Err(EditionCurationError::Duplicate("nonintersecting route"));
                    }
                }
            }
            if let Some(rules) = object.get("posture_rules") {
                let rules = rules
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if rules.last().and_then(|rule| rule["condition"].as_str()) != Some("always") {
                    return Err(EditionCurationError::InvalidRange("posture_rules"));
                }
                let mut seen = BTreeSet::new();
                for rule in rules {
                    let condition = rule["condition"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !seen.insert(condition) {
                        return Err(EditionCurationError::Duplicate("posture condition"));
                    }
                }
            }
            if let Some(risks) = object.get("cross_kind_risks") {
                let mut sides = BTreeSet::new();
                for exclusion in object["scope_census"]["exclusions"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let side = exclusion["side"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !sides.insert(side) {
                        return Err(EditionCurationError::Duplicate("exclusion side"));
                    }
                }
                let parts = object["parts"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut seen = BTreeSet::new();
                for risk in risks
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let part = risk["part_order"]
                        .as_u64()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let reading = risk["reading_ordinal"]
                        .as_u64()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let selected = usize::try_from(part)
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .and_then(|n| parts.get(n))
                        .ok_or(EditionCurationError::InvalidRange("cross_kind_risks"))?;
                    if selected["expected_readings"]
                        .as_u64()
                        .is_none_or(|count| reading > count)
                    {
                        return Err(EditionCurationError::InvalidRange("cross_kind_risks"));
                    }
                    if !seen.insert((part, reading)) {
                        return Err(EditionCurationError::Duplicate("cross kind risk"));
                    }
                }
            }
            if let Some(scope) = object.get("legacy_work_scope") {
                let sources = object["source_item_refs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if !sources.contains(&scope["outside_item_ref"])
                    || scope["primary_work_ref"] == scope["outside_work_ref"]
                {
                    return Err(EditionCurationError::MissingReference);
                }
                let census = &scope["expected_census"];
                for (subset, total) in [
                    ("de_work_scope_occurrences", "de_exact_occurrences"),
                    ("de_work_scope_exact_forms", "de_exact_forms"),
                    ("de_work_scope_analysis_forms", "de_analysis_forms"),
                ] {
                    if !matches!(
                        compare_numbers(&census[subset], &census[total]),
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        return Err(EditionCurationError::InvalidRange("expected_census"));
                    }
                }
            }
            if let Some(routes) = object.get("work_routes") {
                let routes = routes
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let locators = object["locators"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut works = BTreeSet::new();
                let mut slugs = BTreeSet::new();
                for route in routes {
                    let work = route["work_ref"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let slug = route["legacy_slug"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let role = route["source_map_role"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !works.insert(work) || !slugs.insert(slug) {
                        return Err(EditionCurationError::Duplicate("source work route"));
                    }
                    if !locators.iter().any(|locator| locator["role"] == role) {
                        return Err(EditionCurationError::MissingReference);
                    }
                    if !object["marker_curation_refs"]
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?
                        .contains(&route["marker_curation_ref"])
                    {
                        return Err(EditionCurationError::MissingReference);
                    }
                    if let Some(nav) = route.get("xml_navigation") {
                        if !locators
                            .iter()
                            .any(|locator| locator["role"] == nav["inventory_item_role"])
                        {
                            return Err(EditionCurationError::MissingReference);
                        }
                    }
                    let profile = &route["boundary_evidence_profile"];
                    for witness in [
                        profile.get("image_witness"),
                        profile.get("scandata_witness"),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if !locators
                            .iter()
                            .any(|locator| locator["role"] == witness["item_role"])
                        {
                            return Err(EditionCurationError::MissingReference);
                        }
                    }
                    let mut layers = BTreeSet::new();
                    for witness in std::iter::once(&route["address_witness"])
                        .chain(std::iter::once(&route["default_content_witness"]))
                        .chain(
                            route["layer_content_witnesses"]
                                .as_array()
                                .ok_or(EditionCurationError::InvalidSchema)?,
                        )
                    {
                        if !locators
                            .iter()
                            .any(|locator| locator["role"] == witness["item_role"])
                        {
                            return Err(EditionCurationError::MissingReference);
                        }
                        if let Some(layer) = witness.get("layer").and_then(Value::as_str) {
                            if !layers.insert(layer) {
                                return Err(EditionCurationError::Duplicate(
                                    "content witness layer",
                                ));
                            }
                        }
                    }
                    let mut keys = BTreeSet::new();
                    for entry in route["overrides"]
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?
                    {
                        let key = entry["qualified_unit_key"]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if !keys.insert(key) {
                            return Err(EditionCurationError::Duplicate("source layer key"));
                        }
                    }
                }
            }
            if let Some(entries) = object.get("retained_artifacts").and_then(Value::as_array) {
                for entry in entries {
                    if let Some(value) = entry.get("artifact_value") {
                        let source_role = value["source_role"]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if entry["role"] == source_role
                            || !entries.iter().any(|source| {
                                source["role"] == source_role
                                    && (source["repository_path"].is_string()
                                        || source["plan_pointer"].is_string())
                            })
                        {
                            return Err(EditionCurationError::MissingReference);
                        }
                    }
                }
            }
            if object.get("stage").and_then(Value::as_str) == Some("transfer-readiness") {
                let base = object["base_ref"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let frozen = object["frozen_outputs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if frozen.len() != 2 {
                    return Err(EditionCurationError::BindingMismatch);
                }
                for (role, field) in [
                    ("projection", "output_filename"),
                    ("provenance", "provenance_filename"),
                ] {
                    let filename = object[field]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let expected = format!("{base}/{filename}");
                    if !frozen
                        .iter()
                        .any(|entry| entry["role"] == role && entry["repository_path"] == expected)
                    {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                }
            }
            if object.get("stage").and_then(Value::as_str) == Some("source-review-receipt") {
                let base = object["base_ref"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let prefix = object["receipt_filename_prefix"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let generation = object["historical_generation"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let expected = format!("{base}/{prefix}{generation}.json");
                let frozen = object["frozen_outputs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if frozen.len() != 1
                    || frozen[0]["role"] != "receipt"
                    || frozen[0]["repository_path"] != expected
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
            }
            if object.get("stage").and_then(Value::as_str) == Some("structural-source-routes") {
                let root = object["alignment_root"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let directory = object["output_subdirectory"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let configurations = object["configurations"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let files = object["output_files"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let frozen = object["frozen_outputs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if configurations.len().checked_mul(files.len()) != Some(frozen.len()) {
                    return Err(EditionCurationError::BindingMismatch);
                }
                let mut slugs = BTreeSet::new();
                for configuration in configurations {
                    let slug = configuration["slug"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let pair = configuration["pair_slug"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    for component in [slug, pair] {
                        tos_foundation::RelativePath::parse(component)
                            .map_err(|_| EditionCurationError::UnsafePath)?;
                        if component.contains('/') {
                            return Err(EditionCurationError::UnsafePath);
                        }
                    }
                    if !slugs.insert(slug) {
                        return Err(EditionCurationError::Duplicate("structural route slug"));
                    }
                    for file in files {
                        let role = file["role"]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        let filename = file["filename"]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        let expected_role = format!("{slug}:{role}");
                        let expected_path = format!("{root}/{slug}/{directory}/{pair}/{filename}");
                        if !frozen.iter().any(|entry| {
                            entry["role"] == expected_role
                                && entry["repository_path"] == expected_path
                        }) {
                            return Err(EditionCurationError::BindingMismatch);
                        }
                    }
                }
            }
            if object
                .get("stage")
                .and_then(Value::as_str)
                .is_some_and(|stage| {
                    matches!(
                        stage,
                        "source-passages"
                            | "target-passages"
                            | "transfer-candidates"
                            | "numbered-label-correspondence"
                    )
                })
            {
                let locators = object["locators"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let frozen = object["frozen_outputs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let expected_roles: &[&str] = match object["stage"].as_str() {
                    Some("transfer-candidates") => &["anchors", "provenance"],
                    Some("numbered-label-correspondence") => &["output", "provenance"],
                    _ => &["candidates", "anchors", "provenance"],
                };
                if frozen.len() != expected_roles.len()
                    || expected_roles.iter().any(|role| {
                        let locator = locators.iter().find(|entry| entry["role"] == *role);
                        let output = frozen.iter().find(|entry| entry["role"] == *role);
                        !matches!((locator, output), (Some(a), Some(b)) if a["repository_path"] == b["repository_path"])
                    })
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
            }
            if let Some(naming) = object.get("output_naming") {
                let base = object["base_ref"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let generation = naming["initial_generation"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let packet = naming["packet_stem"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let provenance = naming["provenance_stem"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let include_generation = !naming["initial_provenance_omits_generation"]
                    .as_bool()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let suffix = if include_generation {
                    format!(".{generation}")
                } else {
                    String::new()
                };
                let expected = [
                    ("packet", format!("{base}/{packet}.{generation}.json")),
                    ("provenance", format!("{base}/{provenance}{suffix}.jsonl")),
                ];
                let frozen = object["frozen_outputs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if frozen.len() != expected.len()
                    || expected.iter().any(|(role, path)| {
                        !frozen.iter().any(|entry| {
                            entry["role"] == *role && entry["repository_path"] == *path
                        })
                    })
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
            }
            if let Some(manifest) = object.get("frozen_manifest") {
                let route = object["route"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let root = if object.get("route_base").and_then(Value::as_str) == Some("repository")
                {
                    route.to_owned()
                } else {
                    let work = object["work_path"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    format!("{work}/{route}")
                };
                let reference = manifest["ref"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if manifest["output_root"] != root
                    || reference.rsplit_once('/').map(|(parent, _)| parent) != Some(root.as_str())
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
            }
            if object.contains_key("output_files")
                && object.contains_key("frozen_outputs")
                && !object.contains_key("output_layout")
                && object.get("stage").and_then(Value::as_str) != Some("structural-source-routes")
            {
                let files = object["output_files"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let frozen = object["frozen_outputs"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let route = object["route"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let root = match object.get("output_subdirectory") {
                    Some(directory) => format!(
                        "{route}/{}",
                        directory
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?
                    ),
                    None => route.to_owned(),
                };
                if files.len() != frozen.len() {
                    return Err(EditionCurationError::BindingMismatch);
                }
                for file in files {
                    let filename = file["filename"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let expected = format!("{root}/{filename}");
                    if !frozen.iter().any(|output| {
                        output["role"] == file["role"] && output["repository_path"] == expected
                    }) {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                }
            }
            if let Some(layout) = object.get("output_layout") {
                let files = object
                    .get("output_files")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let frozen = object
                    .get("frozen_outputs")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut scopes = BTreeMap::new();
                for (group, scope) in [
                    ("root_roles", false),
                    ("request_roles", true),
                    ("default_or_request_roles", false),
                ] {
                    for role in layout[group]
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?
                    {
                        let role = role.as_str().ok_or(EditionCurationError::InvalidSchema)?;
                        if scopes.insert(role, scope).is_some() {
                            return Err(EditionCurationError::Duplicate("output role scope"));
                        }
                    }
                }
                if files.len() != scopes.len() || files.len() != frozen.len() {
                    return Err(EditionCurationError::BindingMismatch);
                }
                let route = object["route"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let directory = layout["scope_directory"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let default_scope = layout["default_scope"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let issuance = layout["issuance_filename"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let issuance_role = object["identity_profile"]["historical_issuance_role"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let issuance_ref = format!("{route}/{issuance}");
                if !object["historical_templates"]
                    .as_array()
                    .is_some_and(|entries| {
                        entries.iter().any(|entry| {
                            entry["role"] == issuance_role && entry["ref"] == issuance_ref
                        })
                    })
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
                for file in files {
                    let role = file["role"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let filename = file["filename"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let request_scope = scopes
                        .get(role)
                        .ok_or(EditionCurationError::MissingReference)?;
                    let expected = if *request_scope {
                        format!("{route}/{directory}/{default_scope}/{filename}")
                    } else {
                        format!("{route}/{filename}")
                    };
                    if !frozen.iter().any(|output| {
                        output["role"] == role && output["repository_path"] == expected
                    }) {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                }
            }
            if let Some(profiles) = object.get("identity_profiles") {
                let mut roles = BTreeSet::new();
                let mut kinds = BTreeSet::new();
                for profile in profiles
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let role = profile["role"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !roles.insert(role) {
                        return Err(EditionCurationError::Duplicate("identity role"));
                    }
                    let expected = match role {
                        "discourse" => ("method_id", "method-newline-separated-parts"),
                        "formula" => (
                            "formula_method_id",
                            "compact-sorted-utf8-json-method-and-binding",
                        ),
                        _ => return Err(EditionCurationError::InvalidSchema),
                    };
                    if profile["method_field"] != expected.0 || profile["hash_input"] != expected.1
                    {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                    for kind in profile["kinds"]
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?
                    {
                        let kind = kind.as_str().ok_or(EditionCurationError::InvalidSchema)?;
                        if !kinds.insert(kind) {
                            return Err(EditionCurationError::Duplicate("identity kind"));
                        }
                    }
                }
            }
            if let Some(profile) = object.get("signature_settings") {
                let mut languages = BTreeSet::new();
                for language in profile["languages"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let code = language["language"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !languages.insert(code) {
                        return Err(EditionCurationError::Duplicate("signature language"));
                    }
                    check_replacements(&language["character_replacements"])?;
                }
                check_replacements(&profile["unknown_language_character_replacements"])?;
                let fallback = profile["unknown_language_table"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let case = object
                    .get("case_settings")
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if !languages.contains(fallback)
                    || !case["language"]
                        .as_str()
                        .is_some_and(|code| languages.contains(code))
                {
                    return Err(EditionCurationError::MissingReference);
                }
                if !matches!(
                    compare_numbers(&case["lower_ratio"], &case["upper_ratio"]),
                    Some(Ordering::Less | Ordering::Equal)
                ) {
                    return Err(EditionCurationError::InvalidRange("case ratios"));
                }
            }
            if object.contains_key("identity_profile")
                && object.contains_key("opaque_identity_profile")
            {
                return Err(EditionCurationError::BindingMismatch);
            }
            if let Some(profile) = object
                .get("identity_profile")
                .or_else(|| object.get("opaque_identity_profile"))
            {
                let mut kinds = BTreeSet::new();
                for prefix in profile["identity_prefixes"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    let kind = prefix["kind"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !kinds.insert(kind) {
                        return Err(EditionCurationError::Duplicate("identity kind"));
                    }
                }
                let role = profile["historical_issuance_role"]
                    .as_str()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if !object
                    .get("historical_templates")
                    .and_then(Value::as_array)
                    .is_some_and(|entries| entries.iter().any(|entry| entry["role"] == role))
                {
                    return Err(EditionCurationError::MissingReference);
                }
            }
            if let Some(orthography) = object.get("orthography") {
                let profiles = orthography["language_profiles"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut languages = BTreeSet::new();
                for profile in profiles {
                    let language = profile["language"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !languages.insert(language) {
                        return Err(EditionCurationError::Duplicate("orthography language"));
                    }
                    check_replacements(&profile["character_replacements"])?;
                }
            }
            if let Some(selection) = object.get("stopword_selection") {
                let profiles = selection["profiles"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut languages = BTreeSet::new();
                for profile in profiles {
                    let language = profile["language"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !languages.insert(language) {
                        return Err(EditionCurationError::Duplicate("stopword language"));
                    }
                }
                if !selection["unknown_language_profile"]
                    .as_str()
                    .is_some_and(|language| languages.contains(language))
                {
                    return Err(EditionCurationError::MissingReference);
                }
            }
            if let Some(profile) = object.get("form_selection") {
                let languages = profile["languages"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut seen = BTreeSet::new();
                for language in languages {
                    let code = language["language"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !seen.insert(code) {
                        return Err(EditionCurationError::Duplicate("form selection language"));
                    }
                    check_replacements(&language["character_replacements"])?;
                }
                check_replacements(&profile["unknown_language_character_replacements"])?;
                for code in [
                    &profile["unknown_language_suffix_profile"],
                    &profile["historical_prefix"]["language"],
                ] {
                    if !code.as_str().is_some_and(|code| seen.contains(code)) {
                        return Err(EditionCurationError::MissingReference);
                    }
                }
            }
            if let Some(profile) = object.get("speaker_cues") {
                let cues = profile["cues"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut seen = BTreeSet::new();
                for cue in cues {
                    let language = cue["language"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let role = cue["role"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !seen.insert((language, role)) {
                        return Err(EditionCurationError::Duplicate("speaker cue role"));
                    }
                }
                let outcomes = profile["outcomes"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for (outcome, expected_case) in
                    outcomes
                        .iter()
                        .zip(["verse", "transition", "continuation", "unresolved"])
                {
                    if outcome["case"] != expected_case {
                        return Err(EditionCurationError::InvalidRange("speaker outcome order"));
                    }
                    let allowed: BTreeSet<_> = ["language", "reading", "role", "state"]
                        .into_iter()
                        .collect();
                    for value in [outcome.get("primary"), outcome.get("exception_group")]
                        .into_iter()
                        .flatten()
                        .chain(outcome["alternatives"].as_array().into_iter().flatten())
                    {
                        let text = value.as_str().ok_or(EditionCurationError::InvalidSchema)?;
                        if !template_fields(text).is_some_and(|fields| fields.is_subset(&allowed)) {
                            return Err(EditionCurationError::InvalidRange(
                                "speaker outcome template",
                            ));
                        }
                    }
                }
            }
            if let Some(profile) = object.get("candidate_signals") {
                let rules = profile["signals"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut codes = BTreeSet::new();
                for rule in rules {
                    let code = rule["code"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !codes.insert(code) {
                        return Err(EditionCurationError::Duplicate("signal code"));
                    }
                }
                for group in [
                    "recurrence_signals",
                    "eternity_signals",
                    "ring_signals",
                    "universal_signals",
                    "direct_core_signals",
                ] {
                    for code in profile["classification"][group]
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?
                    {
                        if !code.as_str().is_some_and(|code| codes.contains(code)) {
                            return Err(EditionCurationError::MissingReference);
                        }
                    }
                }
                let exclusions: BTreeSet<_> = profile["exclusions"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                    .iter()
                    .filter_map(|entry| entry["code"].as_str())
                    .collect();
                for rule in profile["claim_rules"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?
                {
                    if rule["kind"] == "class-exclusion"
                        && !rule["exclusion_code"]
                            .as_str()
                            .is_some_and(|code| exclusions.contains(code))
                    {
                        return Err(EditionCurationError::MissingReference);
                    }
                }
            }
            if let Some(selection) = object.get("speaker_selection") {
                let readings = selection["readings"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let mut seen = BTreeSet::new();
                for reading in readings {
                    let reference = reading["reading_ref"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !seen.insert(reference) {
                        return Err(EditionCurationError::Duplicate("reading_ref"));
                    }
                    let count = reading["expected_selected_count"]
                        .as_u64()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let mut previous_end = None;
                    for range in reading["overrides"]
                        .as_array()
                        .ok_or(EditionCurationError::InvalidSchema)?
                    {
                        let start = range["start"]
                            .as_u64()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        let end = range["end"]
                            .as_u64()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if start > end
                            || end > count
                            || previous_end.is_some_and(|previous| start <= previous)
                        {
                            return Err(EditionCurationError::InvalidRange("speaker overrides"));
                        }
                        previous_end = Some(end);
                    }
                }
            }
            if let Some(Value::Array(keys)) = object.get("output_keys") {
                let outputs = object
                    .get("frozen_outputs")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                if keys.len() != outputs.len()
                    || keys
                        .iter()
                        .zip(outputs)
                        .any(|(key, output)| output.get("role") != Some(key))
                {
                    return Err(EditionCurationError::BindingMismatch);
                }
                if let Some(provenance_ref) = object.get("historical_provenance_ref") {
                    let provenance = outputs
                        .iter()
                        .find(|output| output["role"] == "provenance_ref")
                        .ok_or(EditionCurationError::MissingReference)?;
                    if provenance.get("repository_path") != Some(provenance_ref)
                        || provenance.get("expected_sha256")
                            != object.get("historical_provenance_sha256")
                    {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                }
            }
            if object.get("stage").and_then(Value::as_str) == Some("text-collation") {
                let roles = object
                    .get("witness_roles")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let items = object
                    .get("source_item_refs")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for (index, expected_role) in ["source", "target"].iter().enumerate() {
                    if roles[index]["role"] != *expected_role
                        || items.get(index) != Some(&roles[index]["item_ref"])
                    {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                }
            }
            if object.get("stage").and_then(Value::as_str)
                == Some("witness-structure-correspondence")
            {
                let parts = object
                    .get("parts")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let items = object
                    .get("source_item_refs")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let matching = object
                    .get("matching")
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let base = matching["epub_page_index_base"]
                    .as_u64()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let limit = base
                    .checked_add(
                        matching["target_page_count"]
                            .as_u64()
                            .ok_or(EditionCurationError::InvalidSchema)?,
                    )
                    .ok_or(EditionCurationError::InvalidRange("target_page_count"))?;
                let mut previous_start = None;
                for (index, part) in parts.iter().enumerate() {
                    if items.get(index) != Some(&part["source_item_ref"]) {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                    let first = part["first_target_page"]
                        .as_u64()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    let last = part["last_target_page"]
                        .as_u64()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if first < base
                        || first > last
                        || last >= limit
                        || previous_start.is_some_and(|previous| first < previous)
                    {
                        return Err(EditionCurationError::InvalidRange("parts"));
                    }
                    previous_start = Some(first);
                }
            }
            if object.get("stage").and_then(Value::as_str) == Some("constructor-library") {
                let parts = object
                    .get("parts")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                let items = object
                    .get("source_item_refs")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for (index, part) in parts.iter().enumerate() {
                    if part["number"].as_u64() != u64::try_from(index + 1).ok() {
                        return Err(EditionCurationError::InvalidRange("parts"));
                    }
                    if items.get(index) != Some(&part["item_ref"]) {
                        return Err(EditionCurationError::BindingMismatch);
                    }
                }
                let presentation = object
                    .get("presentation")
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for (name, expected) in [
                    (
                        "part_description_template",
                        &["chapter_count", "fragment_count"][..],
                    ),
                    ("part_source_note_template", &["fragment_count", "year"][..]),
                    (
                        "chapter_body_template",
                        &["chapter_label", "part_label"][..],
                    ),
                    ("part_title_template", &["part_label"][..]),
                    ("chapter_description_template", &["part_label", "count"][..]),
                    ("chapter_source_note_template", &["heading", "count"][..]),
                    ("provisional_speaker_template", &["speaker"][..]),
                ] {
                    let expected: BTreeSet<_> = expected.iter().copied().collect();
                    for language in ["ru", "en"] {
                        let text = presentation[name][language]
                            .as_str()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if template_fields(text) != Some(expected.clone()) {
                            return Err(EditionCurationError::InvalidRange(
                                "presentation template",
                            ));
                        }
                    }
                }
            }
            if let Some(Value::Array(regions)) = object.get("regions") {
                let mut seen = BTreeSet::new();
                for region in regions {
                    if let (Some(page), Some(panel)) = (
                        region.get("page").and_then(Value::as_u64),
                        region.get("panel").and_then(Value::as_str),
                    ) {
                        if !seen.insert((page, panel)) {
                            return Err(EditionCurationError::Duplicate("page region"));
                        }
                    }
                }
            }
            if object.get("stage").and_then(Value::as_str) == Some("numbered-label-correspondence")
            {
                let count = |key: &str| object.get(key).and_then(Value::as_u64);
                let size = |key: &str| {
                    object
                        .get(key)
                        .and_then(Value::as_array)
                        .and_then(|a| u64::try_from(a.len()).ok())
                };
                let shared =
                    count("expected_shared_count").ok_or(EditionCurationError::InvalidSchema)?;
                for (total, exclusive) in [
                    ("expected_source_count", "source_only_keys"),
                    ("expected_target_count", "target_only_keys"),
                ] {
                    if !matches!((size(exclusive).and_then(|n| shared.checked_add(n)), count(total)), (Some(expected), Some(actual)) if expected == actual)
                    {
                        return Err(EditionCurationError::InvalidRange(total));
                    }
                }
            }
            for (lower, upper) in [
                ("start_page", "end_page"),
                ("represented_start_page", "represented_end_page"),
                ("page_min", "page_max"),
                ("x_min", "x_max"),
                ("y_min", "y_max"),
                ("min_words", "max_words"),
                ("bilingual", "evidence_units"),
                ("demo_translations", "evidence_units"),
                ("evidence_units", "total_nodes"),
            ] {
                if let (Some(a), Some(b)) = (object.get(lower), object.get(upper)) {
                    if !matches!(
                        compare_numbers(a, b),
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        return Err(EditionCurationError::InvalidRange(lower));
                    }
                }
            }
            if let Some(Value::Array(order)) = object.get("basis_rule_order") {
                if order.last().and_then(Value::as_str) != Some("default") {
                    return Err(EditionCurationError::InvalidRange("basis_rule_order"));
                }
                for rule in order {
                    let field = match rule.as_str() {
                        Some("repeated-number-review") => Some("repeated_number_review_keys"),
                        Some("gap-review") => Some("gap_review_keys"),
                        Some("page-override") => Some("page_overrides"),
                        Some("ocr-disambiguation") => Some("ocr_disambiguation_keys"),
                        _ => None,
                    };
                    if field.is_some_and(|field| !object.contains_key(field)) {
                        return Err(EditionCurationError::InvalidSchema);
                    }
                }
            }
            for (array, field) in [
                ("page_overrides", "key"),
                ("locators", "role"),
                ("input_sources", "role"),
                ("payload_roles", "role"),
                ("ocr_evidence", "unit_key"),
                ("derived_reference_formats", "role"),
                ("input_sources", "private_payload_ref"),
                ("outputs", "kind"),
                ("outputs", "filename"),
                ("output_files", "role"),
                ("private_output_files", "role"),
                ("private_output_files", "filename"),
                ("difficulty_signals", "metric"),
                ("difficulty_signals", "label"),
                ("frequency_buckets", "label"),
                ("length_buckets", "label"),
                ("output_files", "filename"),
                ("frozen_outputs", "role"),
                ("frozen_outputs", "repository_path"),
                ("historical_fields", "role"),
                ("coverage_fields", "role"),
                ("quote_pairs", "open"),
                ("historical_receipt_fields", "role"),
                ("historical_templates", "role"),
                ("retained_artifacts", "role"),
                ("passage_id_substitutions", "from"),
                ("reading_substitutions", "from"),
                ("structure_parent_defaults", "record_kind"),
                ("kinds", "kind"),
                ("record_identities", "role"),
                ("role_precedence", "role"),
                ("visual_markers", "citation_contains"),
                ("visual_markers", "technical_structure_key"),
                ("visual_markers", "contradiction_ref"),
                ("witness_roles", "plan_key"),
                ("witness_roles", "identity_key"),
                ("anchor_prefixes", "role"),
                ("families", "family"),
                ("families", "repository_path"),
                ("parts", "part_label"),
                ("parts", "manifest_ref"),
                ("chapter_labels", "key"),
                ("class_notes", "key"),
                ("speaker_labels", "key"),
                ("exclusions", "code"),
                ("demo_ids", "annotation_ref"),
                ("demo_ids", "slug"),
                ("work_mappings", "work_ref"),
                ("work_mappings", "candidate_token"),
                ("work_mappings", "work_path_segment"),
                ("works", "work_ref"),
                ("series", "series_key"),
            ] {
                if let Some(Value::Array(items)) = object.get(array) {
                    let mut seen = BTreeSet::new();
                    for item in items {
                        if let Some(key) = item.get(field).and_then(Value::as_str) {
                            if !seen.insert(key) {
                                return Err(EditionCurationError::Duplicate(field));
                            }
                        }
                    }
                }
            }
            if let Some(Value::Array(fields)) = object.get("historical_fields") {
                let outputs = object
                    .get("frozen_outputs")
                    .and_then(Value::as_array)
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for field in fields {
                    let role = field["output_role"]
                        .as_str()
                        .ok_or(EditionCurationError::InvalidSchema)?;
                    if !outputs.iter().any(|output| output["role"] == role) {
                        return Err(EditionCurationError::MissingReference);
                    }
                }
            }
            if object.get("page_order").and_then(Value::as_str) == Some("nondecreasing") {
                let overrides = object["page_overrides"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for pair in overrides.windows(2) {
                    if !matches!(
                        compare_numbers(&pair[0]["page"], &pair[1]["page"]),
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        return Err(EditionCurationError::InvalidRange("page_overrides"));
                    }
                }
            }
            if let Some(Value::Array(items)) = object.get("marker_returns") {
                let mut seen = BTreeSet::new();
                for item in items {
                    if let Some(key) = item.get("key").and_then(Value::as_array) {
                        let key = key
                            .iter()
                            .map(|part| match part {
                                Value::String(text) => Some(MarkerPart::Text(text.clone())),
                                Value::Number(_) => {
                                    number_parts(part).map(|(sign, digits, magnitude)| {
                                        MarkerPart::Number(sign, digits, magnitude)
                                    })
                                }
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if !seen.insert(key) {
                            return Err(EditionCurationError::Duplicate("marker key"));
                        }
                    }
                }
            }
            if let Some(Value::Array(bounds)) = object.get("line_bbox") {
                for (lower, upper) in [(0, 2), (1, 3)] {
                    if !matches!(
                        compare_numbers(&bounds[lower], &bounds[upper]),
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        return Err(EditionCurationError::InvalidRange("line_bbox"));
                    }
                }
            }
            for child in object.values() {
                check_rules(child)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                check_rules(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn template_fields(mut text: &str) -> Option<BTreeSet<&str>> {
    let mut fields = BTreeSet::new();
    loop {
        let Some(start) = text.find('{') else {
            return (!text.contains('}')).then_some(fields);
        };
        if text[..start].contains('}') {
            return None;
        }
        let tail = &text[start + 1..];
        let end = tail.find('}')?;
        let field = &tail[..end];
        if field.is_empty()
            || !field
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return None;
        }
        fields.insert(field);
        text = &tail[end + 1..];
    }
}

fn number_parts(value: &Value) -> Option<(bool, String, BigInt)> {
    let text = value.as_number()?.to_string();
    let (negative, text) = match text.strip_prefix('-') {
        Some(text) => (true, text),
        None => (false, text.as_str()),
    };
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], BigInt::from_str(&text[index + 1..]).ok()?),
        None => (text, BigInt::from(0)),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, String::new(), BigInt::from(0)));
    }
    let magnitude = exponent + BigInt::from(digits.len()) - BigInt::from(fraction.len());
    Some((negative, digits.trim_end_matches('0').to_owned(), magnitude))
}

// Compare JSON decimals without rounding coordinates or allocating by exponent.
fn compare_numbers(a: &Value, b: &Value) -> Option<Ordering> {
    let (an, a, am) = number_parts(a)?;
    let (bn, b, bm) = number_parts(b)?;
    if a.is_empty() || b.is_empty() {
        return Some(match (a.is_empty(), b.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => {
                if bn {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (false, true) => {
                if an {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            _ => unreachable!(),
        });
    }
    if an != bn {
        return Some(if an {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let magnitude_order = am.cmp(&bm).then_with(|| {
        let len = a.len().max(b.len());
        a.bytes()
            .chain(std::iter::repeat(b'0'))
            .take(len)
            .cmp(b.bytes().chain(std::iter::repeat(b'0')).take(len))
    });
    Some(if an {
        magnitude_order.reverse()
    } else {
        magnitude_order
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_range_comparison_keeps_exact_precision() {
        fn number(raw: &str) -> Value {
            serde_json::from_str(raw).unwrap()
        }
        for (a, b, expected) in [
            ("9007199254740993", "9007199254740992", Ordering::Greater),
            ("1.00000000000000001", "1.0", Ordering::Greater),
            ("-1.01", "-1.001", Ordering::Less),
            ("1e1000", "9e999", Ordering::Greater),
            ("-0.0", "0", Ordering::Equal),
        ] {
            assert_eq!(compare_numbers(&number(a), &number(b)), Some(expected));
        }
    }
}
