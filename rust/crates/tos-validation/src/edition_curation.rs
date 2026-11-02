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
    if stage == "text-collation" {
        let selected_plan = published_value(plan.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
            .map_err(EditionCurationError::Schema)?;
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
        .ok_or(EditionCurationError::MissingReference)?
        .iter()
        .find(|entry| entry["role"] == role)
        .ok_or(EditionCurationError::MissingReference)?;
    let reference = entry["ref"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    let digest = entry["sha256"]
        .as_str()
        .ok_or(EditionCurationError::InvalidSchema)?;
    verify_selected_bytes(template, reference, digest)
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

fn check_rules(value: &Value) -> Result<(), EditionCurationError> {
    match value {
        Value::Object(object) => {
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
