//! Shared validation for curation-selected numbered structure recipes.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_source_store::execution::Execution;
use tos_validation::{
    SchemaBackendProbe,
    edition_curation::{self, SelectedDocument},
};

fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{name} must be a string"))
}
fn number(value: &Value, name: &str) -> Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| format!("{name} must be an unsigned integer"))
}

pub(crate) fn keys(ctx: &Execution, recipe: &Value) -> Result<Vec<String>, String> {
    let sequence = &recipe["unit_sequence"];
    let first = number(&sequence["first_number"], "first number")?;
    let last = number(&sequence["last_number"], "last number")?;
    let suffix = text(&sequence["repeat_suffix"], "repeat suffix")?;
    let repeat = sequence["repeat_after"]
        .as_array()
        .ok_or("repeat numbers required")?;
    let repeated = repeat
        .iter()
        .map(|value| number(value, "repeat number"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if repeated.len() != repeat.len()
        || first > last
        || repeated.iter().any(|value| *value < first || *value > last)
    {
        return Err("invalid numbered sequence".into());
    }
    let expected = number(&sequence["expected_count"], "expected count")?;
    let calculated = last
        .checked_sub(first)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_add(repeated.len() as u64))
        .ok_or("numbered sequence overflow")?;
    if expected != calculated {
        return Err("numbered sequence count differs".into());
    }
    let remaining = ctx.budget_report();
    let max_work = number(&remaining["work_units_max"], "work limit")?;
    let used = number(&remaining["charged_work_units"], "charged work")?;
    if expected > max_work.saturating_sub(used) {
        return Err("numbered sequence exceeds remaining work".into());
    }
    let count = usize::try_from(expected).map_err(|_| "numbered sequence exceeds address space")?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| "numbered sequence allocation failed")?;
    for n in first..=last {
        ctx.tick(1)?;
        result.push(n.to_string());
        if repeated.contains(&n) {
            result.push(format!("{n}{suffix}"));
        }
    }
    Ok(result)
}

fn includes(recipe: &Value, field: &str, key: &str) -> bool {
    recipe[field]
        .as_array()
        .is_some_and(|entries| entries.iter().any(|entry| entry.as_str() == Some(key)))
}

fn basis<'a>(recipe: &'a Value, key: &str, overridden: bool) -> Result<&'a str, String> {
    for rule in recipe["basis_rule_order"]
        .as_array()
        .ok_or("basis order required")?
    {
        match text(rule, "basis rule")? {
            "repeated-number-review" if includes(recipe, "repeated_number_review_keys", key) => {
                return Ok("source_visible_repeated_number_review");
            }
            "gap-review" if includes(recipe, "gap_review_keys", key) => {
                return Ok("source_visible_gap_review");
            }
            "page-override" if overridden => return Ok("source_visible_gap_review"),
            "ocr-disambiguation" if includes(recipe, "ocr_disambiguation_keys", key) => {
                return Ok("source_visible_ocr_disambiguation");
            }
            "default" => return text(&recipe["default_basis"], "default basis"),
            "repeated-number-review" | "gap-review" | "page-override" | "ocr-disambiguation" => (),
            _ => return Err("unknown numbered basis rule".into()),
        }
    }
    Err("numbered basis rule did not select a value".into())
}

pub fn validate_frozen_map(
    ctx: &Execution,
    curation: &[u8],
    owner: &Value,
    recipe: &Value,
    schema: &SchemaBackendProbe,
) -> Result<(), String> {
    if recipe.get("source_map_sha256").is_some() {
        let source = recipe["locators"]
            .as_array()
            .ok_or("numbered locators required")?
            .iter()
            .find(|locator| locator["role"] == "source_map_path")
            .ok_or("source map locator required")?;
        let source_ref = text(&source["repository_path"], "source map path")?;
        let source_raw = ctx.read(source_ref)?;
        edition_curation::validate_numbered_source_map(
            curation,
            SelectedDocument {
                reference: source_ref,
                raw: &source_raw,
            },
            schema,
        )
        .map_err(|error| format!("source map: {error:?}"))?;
    }
    let output = recipe["frozen_outputs"]
        .as_array()
        .ok_or("frozen outputs required")?
        .iter()
        .find(|output| output["role"] == "map")
        .ok_or("frozen map required")?;
    let reference = text(&output["repository_path"], "map path")?;
    let raw = ctx.read(reference)?;
    edition_curation::validate_frozen_output_fixity(
        curation,
        "numbered-structure",
        "map",
        SelectedDocument {
            reference,
            raw: &raw,
        },
        schema,
    )
    .map_err(|error| format!("frozen map: {error:?}"))?;
    let map: Value = serde_json::from_slice(&raw).map_err(|error| error.to_string())?;
    if map["schema_version"] != output["schema_version"]
        || map["map_id"] != recipe["map_id"]
        || map["work_ref"] != recipe["work_ref"]
        || map["expression_ref"] != recipe["expression_ref"]
        || map["edition_ref"] != recipe["edition_ref"]
        || map["item_ref"] != owner["owner_item_ref"]
        || map["provenance_event_ref"] != recipe["event_id"]
    {
        return Err("frozen map identity differs from curation".into());
    }
    let keys = keys(ctx, recipe)?;
    let units = map["unit_starts"]
        .as_array()
        .ok_or("numbered map units required")?;
    if units.len() != keys.len() {
        return Err("numbered map count differs from curation".into());
    }
    let prefix = text(&recipe["identity"]["anchor_prefix"], "anchor prefix")?;
    let suffix = text(&recipe["identity"]["anchor_suffix"], "anchor suffix")?;
    let mut overrides = BTreeMap::new();
    for item in recipe["page_overrides"]
        .as_array()
        .ok_or("page overrides required")?
    {
        let key = text(&item["key"], "override key")?;
        let page = number(&item["page"], "override page")?;
        if overrides.insert(key, page).is_some() {
            return Err("duplicate page override".into());
        }
    }
    let mut previous = 0;
    for (index, (unit, key)) in units.iter().zip(keys.iter()).enumerate() {
        ctx.tick(1)?;
        let page = number(&unit["pdf_page"], "unit page")?;
        if unit["sequence"] != (index + 1) as u64
            || unit["unit_key"] != *key
            || unit["anchor_ref"] != format!("{prefix}{key}{suffix}")
            || unit["basis"] != basis(recipe, key, overrides.contains_key(key.as_str()))?
            || (recipe["page_order"] == "nondecreasing" && page < previous)
            || overrides
                .get(key.as_str())
                .is_some_and(|expected| *expected != page)
        {
            return Err("numbered map differs from selected sequence or overrides".into());
        }
        previous = page;
    }
    Ok(())
}
