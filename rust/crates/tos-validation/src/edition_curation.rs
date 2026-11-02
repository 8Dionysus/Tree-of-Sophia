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
    let reference = recipe["plan_ref"]
        .as_str()
        .or_else(|| {
            recipe["locators"].as_array()?.iter().find_map(|locator| {
                (locator["role"] == "index_plan")
                    .then(|| locator["repository_path"].as_str())
                    .flatten()
            })
        })
        .ok_or(EditionCurationError::MissingReference)?;
    let current = recipe["plan_sha256"].as_str();
    let historical = recipe["historical_plan_sha256"].as_str();
    if matches!((current, historical), (Some(a), Some(b)) if a != b) {
        return Err(EditionCurationError::BindingMismatch);
    }
    let expected = current
        .or(historical)
        .ok_or(EditionCurationError::MissingReference)?;
    verify_selected_bytes(plan, reference, expected)?;
    if let Some(profile) = recipe
        .get("identity_profile")
        .or_else(|| recipe.get("opaque_identity_profile"))
    {
        let selected_plan = published_value(plan.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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
        let selected_plan = published_value(plan.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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
        let selected_plan = published_value(plan.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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
        let selected = published_value(template.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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
    if stage == "concept-candidate" && role == "legacy-output-template" {
        let selected_template =
            published_value(template.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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
    let plan = published_value(plan.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
        .map_err(EditionCurationError::Schema)?;
    let template = published_value(template.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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
    axes.extend(array(&plan, "control_axes_plan_pointer")?);
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
    let request = published_value(selected.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
        .map_err(EditionCurationError::Schema)?;
    if request["request_key"] != layout["default_request_key"] {
        return Err(EditionCurationError::BindingMismatch);
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
    let historical = published_value(selected.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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
    let value = published_value(raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
        .map_err(EditionCurationError::Schema)?;
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
        let item = published_value(manifest.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
            .map_err(EditionCurationError::Schema)?;
        let edition = published_value(edition.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
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

fn check_rules(value: &Value) -> Result<(), EditionCurationError> {
    match value {
        Value::Object(object) => {
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
                ("outputs", "kind"),
                ("outputs", "filename"),
                ("output_files", "role"),
                ("output_files", "filename"),
                ("frozen_outputs", "role"),
                ("frozen_outputs", "repository_path"),
                ("historical_fields", "role"),
                ("historical_templates", "role"),
                ("record_identities", "role"),
                ("role_precedence", "role"),
                ("visual_markers", "citation_contains"),
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
