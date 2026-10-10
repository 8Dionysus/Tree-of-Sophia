//! Native, private source-navigation base-node normalization.
//!
//! This produces Python-compatible base nodes for bounded owner envelopes,
//! including direct assertion/temporal semantics and exact record versions.
//! Global claim/context updates and inherited views may change a base row,
//! so this module never writes a final graph row. Exact historical records
//! with floating JSON numbers refuse until their canonical encoding is pinned.

use crate::knowledge_normalization::{SourceRow, stable_digest, stamp_content_revision};
use crate::knowledge_philosophy_display::source_navigation_node_display;
use crate::knowledge_source_navigation_prepare::NavigationPrepareReceipt;
use crate::knowledge_stage::SeekRow;
use crate::{Error, KnowledgeRegistry, QueryVocabulary, Result};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256;

const ADAPTER_PROFILE: &str = "source-navigation-node-edge-v1";
const SHARED_ID_GRAMMAR: &str = "^tos\\.[a-z0-9]+(?:[.-][a-z0-9]+)*$";
const RECORD_SCHEMA: &str = "tos_record_version_view_v1";
const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;
const MAX_ADDRESS: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug)]
pub struct NavigationNodeLimits {
    pub max_raw_bytes: usize,
    pub max_output_bytes: usize,
    pub max_ancestor_cache_bytes: usize,
}
impl NavigationNodeLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_raw_bytes == 0
            || self.max_raw_bytes > 8 * 1024 * 1024
            || self.max_output_bytes == 0
            || self.max_output_bytes > 8 * 1024 * 1024
            || self.max_ancestor_cache_bytes == 0
            || self.max_ancestor_cache_bytes > 4 * 1024 * 1024
        {
            return Err(Error::Budget("navigation node normalization limits"));
        }
        Ok(())
    }
}

struct TypeEntry {
    parents: Vec<String>,
    labels: Option<Value>,
    object_role: Option<String>,
    mapping_labels: BTreeMap<String, BTreeMap<String, Value>>,
}

/// The registry bytes must be the independently selected owner bytes used to
/// construct `KnowledgeRegistry`; a matching digest alone is not admission.
pub struct NavigationNodeNormalizer<'a> {
    registry: &'a KnowledgeRegistry,
    source_graph_id: String,
    dossier_kinds: BTreeSet<String>,
    entity_registry_ref: String,
    types: BTreeMap<String, TypeEntry>,
    ancestors: BTreeMap<String, Vec<String>>,
    ancestor_cache_bytes: usize,
    limits: NavigationNodeLimits,
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("navigation native node field"))
}
fn text(value: Option<&Value>) -> Option<&str> {
    value?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
fn string_list(value: Option<&Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().filter(|s| !s.is_empty()))
        .filter(|s| seen.insert((*s).to_owned()))
        .map(|s| Value::String(s.to_owned()))
        .collect()
}
fn source_dossier_id<'a>(kinds: &BTreeSet<String>, kind: &str, native: &'a str) -> Option<&'a str> {
    if kinds.contains(kind) && valid_tos_id(native) {
        Some(native)
    } else {
        None
    }
}
fn valid_tos_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos.") else {
        return false;
    };
    let mut previous_separator = true;
    for byte in rest.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else if matches!(byte, b'.' | b'-') && !previous_separator {
            previous_separator = true;
        } else {
            return false;
        }
    }
    !previous_separator
}
pub(crate) fn attributes(item: &Value) -> Result<Map<String, Value>> {
    let object = item
        .as_object()
        .ok_or(Error::Invalid("navigation native node object"))?;
    let mut attrs = item
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in object {
        if !matches!(
            key.as_str(),
            "id" | "node_id"
                | "label"
                | "canonical_label"
                | "node_type"
                | "node_kind"
                | "resource_kind"
                | "display"
                | "multilingual"
                | "properties"
                | "source_ref"
                | "source_refs"
                | "graph_layers"
                | "view_ids"
        ) && !attrs.contains_key(key)
        {
            attrs.insert(key.clone(), value.clone());
        }
    }
    Ok(attrs)
}
pub(crate) fn epistemic(item: &Value) -> Value {
    let props = item.get("properties").and_then(Value::as_object);
    let p = |key| props.and_then(|p| p.get(key));
    let authority = text(p("authority_posture"))
        .or_else(|| text(item.get("authority_layer")))
        .unwrap_or("derived-export");
    let canon = text(p("canon_status")).or_else(|| text(item.get("status")));
    let review = text(p("review_posture"))
        .or_else(|| text(p("review_status")))
        .or_else(|| text(item.get("review_status")))
        .unwrap_or("not-recorded");
    let confidence = [
        p("confidence"),
        p("master_confidence"),
        item.get("confidence"),
    ]
    .into_iter()
    .flatten()
    .find_map(|v| match v {
        Value::Number(n) if n.as_f64().is_some_and(f64::is_finite) => Some(v.clone()),
        Value::String(s) if !s.trim().is_empty() => Some(Value::String(s.trim().to_owned())),
        _ => None,
    })
    .unwrap_or(Value::Null);
    json!({"authority_layer":authority,"canon_status":canon,"review_posture":review,"confidence":confidence})
}
fn owner_envelope(item: &Value) -> Result<()> {
    let object = item
        .as_object()
        .ok_or(Error::Invalid("navigation native owner object"))?;
    // File originals may declare the complete Item-manifest membership set.
    // Preserve that exact optional value for the maintained dossier reader,
    // which owns type/completeness checks and fails closed on malformed sets.
    let file_sources = item.get("node_kind").and_then(Value::as_str) == Some("file")
        && object.contains_key("source_refs");
    if object.len() != 6 + usize::from(file_sources)
        || object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "node_id" | "node_kind" | "label" | "source_ref" | "identity_status" | "properties"
            ) && !(file_sources && key == "source_refs")
        })
        || item.get("properties").and_then(Value::as_object).is_none()
        || item.get("label").and_then(Value::as_str).is_none()
        || item
            .get("identity_status")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(Error::Invalid("navigation owner node envelope"));
    }
    Ok(())
}
pub(crate) const ASSERTION_FIELDS: &[&str] = &[
    "claim_id",
    "claim_ref",
    "claim_version",
    "claim_type",
    "assertion_layer",
    "subject_ref",
    "predicate",
    "object",
    "proposition",
    "qualifiers",
    "polarity",
    "negated",
    "condition",
    "conditions",
    "attribution",
    "scope",
    "temporal_context",
    "spatial_context",
    "epistemic_status",
    "review_status",
    "claim_status",
    "review_refs",
    "reviews",
    "assessment_refs",
    "confidence",
    "maker",
    "method_ref",
    "evidence_refs",
    "counterevidence_refs",
    "alternative_claim_refs",
    "competing_claim_refs",
    "supersedes_claim_ref",
    "provenance_event_ref",
    "visibility",
];
fn assertion_trigger(key: &str) -> bool {
    ASSERTION_FIELDS.contains(&key)
        && !matches!(
            key,
            "subject_ref"
                | "predicate"
                | "object"
                | "scope"
                | "temporal_context"
                | "spatial_context"
                | "visibility"
                | "confidence"
                | "maker"
                | "method_ref"
        )
}
pub(crate) fn assertion_context(item: &Value, refs: &[String]) -> Result<Option<Value>> {
    let props = item.get("properties").and_then(Value::as_object);
    let embedded = props
        .and_then(|props| props.get("source_claim"))
        .and_then(Value::as_object);
    let mut layers: Vec<(&Map<String, Value>, &str)> = Vec::new();
    if let Some(top) = item.as_object() {
        layers.push((top, ""));
    }
    if let Some(props) = props {
        layers.push((props, "/properties"));
    }
    if let Some(embedded) = embedded {
        layers.push((embedded, "/properties/source_claim"));
    }
    if !layers
        .iter()
        .any(|(layer, _)| layer.keys().any(|key| assertion_trigger(key)))
    {
        return Ok(None);
    }
    let mut fields = Map::new();
    let mut conflicts = Vec::new();
    for (layer, prefix) in layers {
        for key in ASSERTION_FIELDS {
            let Some(value) = layer.get(*key) else {
                continue;
            };
            let entry = json!({"value":value,"source_pointer":format!("{prefix}/{key}")});
            if let Some(previous) = fields.get(*key) {
                if stable_digest(
                    previous
                        .get("value")
                        .ok_or(Error::Invalid("assertion field"))?,
                )? != stable_digest(value)?
                {
                    conflicts.push(
                        json!({"field":key,"lower_priority":previous,"higher_priority":entry}),
                    );
                }
            }
            fields.insert((*key).into(), entry);
        }
    }
    Ok(Some(json!({"schema_version":"tos_assertion_context_v1",
        "binding_role":"carrier","source_record_digest":stable_digest(item)?,
        "source_refs":refs,"fields":fields,"conflicts":conflicts,
        "interpretation":"source-declared-not-semantic-assessment"})))
}
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Number(number)) => number.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(items)) => !items.is_empty(),
        _ => true,
    }
}
fn month_days(year: i64, month: i64) -> Option<i64> {
    let leap = year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0);
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    })
}
fn date_parts(raw: &str) -> Option<(&'static str, i64, i64, i64)> {
    let text = raw.strip_prefix('-').unwrap_or(raw);
    let negative = text.len() != raw.len();
    let pieces = text.split('-').collect::<Vec<_>>();
    if pieces.is_empty()
        || pieces.len() > 3
        || pieces[0].len() != 4
        || pieces
            .iter()
            .any(|part| !part.bytes().all(|b| b.is_ascii_digit()))
        || pieces.iter().skip(1).any(|part| part.len() != 2)
    {
        return None;
    }
    let mut year: i64 = pieces[0].parse().ok()?;
    if negative {
        year = -year;
    }
    let month: i64 = pieces
        .get(1)
        .and_then(|part| part.parse().ok())
        .unwrap_or(1);
    let day: i64 = pieces
        .get(2)
        .and_then(|part| part.parse().ok())
        .unwrap_or(1);
    if day < 1 || day > month_days(year, month)? {
        return None;
    }
    Some((
        match pieces.len() {
            1 => "year",
            2 => "month",
            _ => "day",
        },
        year,
        month,
        day,
    ))
}
fn push_unique(items: &mut Vec<String>, value: &str) {
    if !items.iter().any(|item| item == value) {
        items.push(value.into());
    }
}
fn comparison_issues(value: &Map<String, Value>) -> Vec<String> {
    let mut issues = Vec::new();
    if !matches!(
        value.get("calendar").and_then(Value::as_str),
        Some("gregorian" | "proleptic-gregorian")
    ) {
        issues.push("calendar-not-comparable".into());
    }
    if value.get("year_numbering").and_then(Value::as_str) != Some("astronomical") {
        issues.push("year-numbering-not-comparable".into());
    }
    if matches!(
        value.get("precision").and_then(Value::as_str),
        Some("approximate" | "uncertain" | "unknown")
    ) || value
        .get("certainty")
        .is_some_and(|certainty| certainty.as_str() != Some("exact"))
    {
        issues.push("non-exact-date".into());
    }
    issues
}
pub(crate) fn normalized_time(value: Option<&Value>, field: &str) -> Result<Option<Value>> {
    normalized_time_inner(value, field, 0)
}
fn normalized_time_inner(
    value: Option<&Value>,
    field: &str,
    depth: usize,
) -> Result<Option<Value>> {
    if depth > 32 {
        return Err(Error::Budget("navigation temporal nesting"));
    }
    let Some(value) = value else { return Ok(None) };
    if let Some(raw) = text(Some(value)) {
        if let Some((precision, year, month, day)) = date_parts(raw) {
            let start = year * 10000 + month * 100 + day;
            let end_month = if precision == "year" { 12 } else { month };
            let end_day = if precision == "day" {
                day
            } else {
                month_days(year, end_month).ok_or(Error::Invalid("navigation date month"))?
            };
            return Ok(Some(json!({"kind":"date-assertion","sort_start":start,
                "sort_end":year*10000+end_month*100+end_day,
                "comparison_calendar":"proleptic-gregorian","year_numbering":"astronomical",
                "value":raw,"raw":raw,"precision":precision,
                "normalization_status":"source-literal-parsed","source_field":field})));
        }
        return Ok(Some(
            json!({"kind":"unparsed-period-label","value":raw,"raw":raw,
            "precision":null,"normalization_status":"source-literal-unparsed","source_field":field}),
        ));
    }
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    for wrapper in ["temporal", "interval"] {
        let Some(inner) = object.get(wrapper).and_then(Value::as_object) else {
            continue;
        };
        let mut merged = inner.clone();
        let mut conflicts = Vec::new();
        for key in [
            "calendar",
            "year_numbering",
            "certainty",
            "precision",
            "role",
        ] {
            if let Some(outer) = object.get(key) {
                if merged.get(key).is_some_and(|own| own != outer) {
                    conflicts.push(format!("conflicting-{key}"));
                } else {
                    merged.insert(key.into(), outer.clone());
                }
            }
        }
        let Some(mut result) =
            normalized_time_inner(Some(&Value::Object(merged)), field, depth + 1)?
        else {
            return Ok(None);
        };
        let target = result
            .as_object_mut()
            .ok_or(Error::Invalid("navigation normalized temporal"))?;
        target.insert("raw".into(), value.clone());
        if let Some(wording) = object.get("source_wording") {
            target.insert("source_wording".into(), wording.clone());
        }
        let mut issues = target
            .get("issues")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for conflict in conflicts {
            push_unique(&mut issues, &conflict);
        }
        if !issues.is_empty() {
            for key in [
                "sort_start",
                "sort_end",
                "comparison_calendar",
                "year_numbering",
            ] {
                target.remove(key);
            }
        }
        target.insert("issues".into(), json!(issues));
        if wrapper == "interval" {
            target.insert(
                "interval".into(),
                object
                    .get(wrapper)
                    .ok_or(Error::Invalid("navigation interval"))?
                    .clone(),
            );
            target.insert(
                "kind".into(),
                Value::String(
                    text(object.get("chronology_kind"))
                        .unwrap_or("interval-assertion")
                        .into(),
                ),
            );
            for key in [
                "sequence_posture",
                "publication_posture",
                "scope",
                "stages",
                "ordering_warning",
            ] {
                if let Some(value) = object.get(key) {
                    target.insert(key.into(), value.clone());
                }
            }
        }
        return Ok(Some(result));
    }
    let common = json!({"calendar":object.get("calendar"),"role":object.get("role"),
        "declared_year_numbering":object.get("year_numbering"),"certainty":object.get("certainty"),
        "source_wording":object.get("source_wording"),"normalization_status":"structured-source",
        "source_field":field,"raw":value});
    let mut result = common
        .as_object()
        .ok_or(Error::Invalid("navigation temporal common"))?
        .clone();
    if object.contains_key("start") || object.contains_key("end") {
        let mut bounds = Map::new();
        for key in ["start", "end"] {
            if let Some(value) = object.get(key) {
                bounds.insert(key.into(), value.clone());
            }
        }
        let mut issues = comparison_issues(object);
        let mut parsed = Vec::new();
        for key in ["start", "end"] {
            let mut bound = bounds.get(key).cloned();
            if let Some(Value::Object(inner)) = &mut bound {
                for context in ["calendar", "year_numbering", "certainty"] {
                    if let Some(outer) = object.get(context) {
                        if inner.get(context).is_some_and(|own| own != outer) {
                            push_unique(&mut issues, &format!("conflicting-{context}"));
                        } else {
                            inner.insert(context.into(), outer.clone());
                        }
                    }
                }
            }
            parsed.push(normalized_time_inner(bound.as_ref(), field, depth + 1)?);
        }
        let left = parsed[0]
            .as_ref()
            .and_then(|v| v.get("sort_start"))
            .and_then(Value::as_i64);
        let right = parsed[1]
            .as_ref()
            .and_then(|v| v.get("sort_end"))
            .and_then(Value::as_i64);
        if let (Some(left), Some(right)) = (left, right) {
            if issues.is_empty() && left > right {
                push_unique(&mut issues, "reversed-interval");
            }
            if issues.is_empty() {
                result.insert("sort_start".into(), json!(left));
                result.insert("sort_end".into(), json!(right));
                result.insert("comparison_calendar".into(), json!("proleptic-gregorian"));
                result.insert("year_numbering".into(), json!("astronomical"));
            }
        } else {
            push_unique(&mut issues, "incomplete-or-unparsed-interval");
        }
        result.insert("kind".into(), json!("interval-assertion"));
        result.insert("interval".into(), Value::Object(bounds));
        result.insert("issues".into(), json!(issues));
        return Ok(Some(Value::Object(result)));
    }
    if matches!(
        object.get("kind").and_then(Value::as_str),
        Some("relative-order" | "unknown-date")
    ) {
        result.insert(
            "kind".into(),
            object
                .get("kind")
                .ok_or(Error::Invalid("navigation temporal kind"))?
                .clone(),
        );
        result.insert(
            "relative".into(),
            object.get("relative").cloned().unwrap_or(Value::Null),
        );
        result.insert("issues".into(), json!(["no-absolute-date-bounds"]));
        return Ok(Some(Value::Object(result)));
    }
    if ![
        "date", "value", "period", "start", "end", "year", "month", "day",
    ]
    .iter()
    .any(|key| object.contains_key(*key))
    {
        return Ok(None);
    }
    let mut issues = comparison_issues(object);
    let mut from_parts: Option<String> = None;
    if let Some(year) = object.get("year").filter(|value| !value.is_null()) {
        let parts = ["year", "month", "day"]
            .iter()
            .filter_map(|key| object.get(*key).map(|v| (*key, v)))
            .collect::<Vec<_>>();
        if parts
            .iter()
            .any(|(_, value)| value.as_i64().is_none() || value.is_boolean())
        {
            push_unique(&mut issues, "invalid-date-parts");
        } else if let Some(year) = year.as_i64() {
            let magnitude = year
                .checked_abs()
                .ok_or(Error::Budget("navigation date year"))?;
            let mut form = format!("{}{:04}", if year < 0 { "-" } else { "" }, magnitude);
            if let Some(month) = object.get("month").filter(|value| !value.is_null()) {
                form.push_str(&format!("-{:02}", month.as_i64().unwrap_or_default()));
            }
            if let Some(day) = object.get("day").filter(|value| !value.is_null()) {
                if object.get("month").is_none_or(Value::is_null) {
                    push_unique(&mut issues, "invalid-date-parts");
                } else {
                    form.push_str(&format!("-{:02}", day.as_i64().unwrap_or_default()));
                }
            }
            if !issues.iter().any(|issue| issue == "invalid-date-parts") {
                from_parts = Some(form);
            }
        }
    }
    let literal = object.get("value").or_else(|| object.get("date"));
    let literal = literal
        .cloned()
        .unwrap_or_else(|| from_parts.clone().map(Value::String).unwrap_or(Value::Null));
    let parsed = if literal.is_string() {
        normalized_time_inner(Some(&literal), field, depth + 1)?
    } else {
        None
    };
    if !literal.is_null()
        && parsed
            .as_ref()
            .and_then(|v| v.get("precision"))
            .is_none_or(Value::is_null)
    {
        push_unique(&mut issues, "unparsed-date-value");
    }
    if issues.is_empty() {
        if let Some(parsed) = &parsed {
            for key in [
                "sort_start",
                "sort_end",
                "comparison_calendar",
                "year_numbering",
            ] {
                if let Some(value) = parsed.get(key) {
                    result.insert(key.into(), value.clone());
                }
            }
        }
    }
    result.insert(
        "kind".into(),
        json!(text(object.get("kind")).unwrap_or("date-assertion")),
    );
    result.insert("calendar".into(), json!(text(object.get("calendar"))));
    result.insert("value".into(), literal);
    result.insert(
        "period".into(),
        object.get("period").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "precision".into(),
        object
            .get("precision")
            .filter(|value| truthy(Some(value)))
            .cloned()
            .or_else(|| parsed.as_ref().and_then(|v| v.get("precision")).cloned())
            .unwrap_or(Value::Null),
    );
    result.insert(
        "role".into(),
        object.get("role").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "source_posture".into(),
        object.get("source_posture").cloned().unwrap_or(Value::Null),
    );
    result.insert("issues".into(), json!(issues));
    Ok(Some(Value::Object(result)))
}
fn node_semantics(item: &Value, kind: &str, refs: &[String]) -> Result<Value> {
    let props = item.get("properties").and_then(Value::as_object);
    let p = |key: &str| props.and_then(|props| props.get(key));
    let mut semantics = Map::new();
    if let Some(multilingual) = item.get("multilingual").and_then(Value::as_object) {
        semantics.insert(
            "language_context".into(),
            Value::Object(
                multilingual
                    .iter()
                    .filter(|(key, _)| key.as_str() != "label")
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(context) = assertion_context(item, refs)? {
        semantics.insert("assertion_contexts".into(), Value::Array(vec![context]));
    }
    if truthy(p("packet_id")) {
        semantics.insert(
            "annotation".into(),
            json!({
            "packet_id":p("packet_id"),"packet_version":p("packet_version"),
            "content_available":p("content_available"),
            "publication_posture":p("publication_posture")}),
        );
        if kind == "annotation-claim" {
            semantics.insert(
                "claim".into(),
                json!({
                "claim_id":p("claim_id"),"claim_version":p("claim_version"),
                "proposition":p("proposition"),"review_status":p("claim_status"),
                "contract_ref":"ToS/contracts/semantic-annotation-packet-v2.schema.json"}),
            );
        }
    }
    let period = p("period").or_else(|| item.get("temporal_context"));
    let period_field = if props.is_some_and(|props| props.contains_key("period")) {
        "properties.period"
    } else {
        "temporal_context"
    };
    if let Some(time) = normalized_time(period, period_field)? {
        semantics.insert("time".into(), time);
    }
    if kind == "place" {
        semantics.insert(
            "space".into(),
            json!({"kind":"place-identity",
            "place_id":text(item.get("node_id")),"identity_status":item.get("identity_status"),
            "normalization_status":"source-declared"}),
        );
    }
    if kind == "region" {
        semantics.insert(
            "space".into(),
            json!({"kind":"navigation-region",
            "normalization_status":"not-a-place-identity"}),
        );
    }
    Ok(Value::Object(semantics))
}
fn record_ref_digest(value: &Value) -> Result<String> {
    // Source-owner identity sorts every object, including nested record fields.
    // Workspace feature unification may enable serde_json `preserve_order`.
    let mut canonical = value.clone();
    canonical.sort_all_objects();
    let raw = serde_json::to_vec(&canonical)
        .map_err(|_| Error::Invalid("record version reference JSON"))?;
    Ok(Digest256::of_bytes(&raw).to_hex())
}
pub(crate) fn native_metadata_identity(record: &Value) -> Result<&str> {
    let schema = record.get("schema_version").and_then(Value::as_str);
    if schema == Some("tos_canonical_node_v1") {
        let kind = required(record, "node_type")?;
        if record.get("record_id").is_some()
            || !matches!(
                kind,
                "source"
                    | "concept"
                    | "principle"
                    | "lineage"
                    | "event"
                    | "state"
                    | "support"
                    | "context"
                    | "analogy"
                    | "synthesis"
            )
            || !required(record, "node_id")?
                .strip_prefix(&format!("tos.{kind}."))
                .is_some_and(valid_tos_suffix)
            || record
                .get("record_version")
                .and_then(Value::as_u64)
                .is_none_or(|version| version == 0 || version > MAX_ADDRESS)
        {
            return Err(Error::Invalid("native canonical metadata identity"));
        }
        return Ok("node_id");
    }
    let field = match schema {
        Some("tos_scholarly_composite_witness_v1") => Some("composite_id"),
        Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => {
            Some("artifact_id")
        }
        _ => None,
    };
    if let Some(field) = field {
        if record.get("record_id").is_some()
            || !required(record, field)?
                .starts_with(&format!("tos.{}.", field.trim_end_matches("_id")))
        {
            return Err(Error::Invalid("native metadata identity"));
        }
    }
    Ok(field.unwrap_or("record_id"))
}
fn valid_tos_suffix(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
        && !value
            .as_bytes()
            .first()
            .is_some_and(|byte| matches!(*byte, b'.' | b'-'))
        && !value
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(*byte, b'.' | b'-'))
        && !value.contains("..")
        && !value.contains(".-")
        && !value.contains("-.")
}
fn language_key(value: &str) -> bool {
    if matches!(value, "default" | "original") {
        return false;
    }
    let mut parts = value.split('-');
    let first = parts.next().unwrap_or_default();
    if matches!(first, "i" | "I" | "x" | "X") {
        let parts = parts.collect::<Vec<_>>();
        return !parts.is_empty()
            && parts.iter().all(|part| {
                !part.is_empty()
                    && part.len() <= 8
                    && part.bytes().all(|b| b.is_ascii_alphanumeric())
            });
    }
    first.len() >= 2
        && first.len() <= 8
        && first.bytes().all(|b| b.is_ascii_alphabetic())
        && parts.all(|part| {
            !part.is_empty() && part.len() <= 8 && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}
fn quoted_summary(
    display: &mut Map<String, Value>,
    wording: &str,
    language: Option<&str>,
    pointer: &str,
) -> Result<()> {
    let language = language.filter(|language| language_key(language));
    let mut summary = json!({"default":wording,"ru":null,"en":null,"original":wording});
    if let Some(language) = language {
        summary[language] = Value::String(wording.into());
    }
    display.insert("summary".into(), summary);
    display.insert("summary_state".into(), json!("source-derived"));
    let provenance = display
        .get_mut("provenance")
        .and_then(Value::as_object_mut)
        .ok_or(Error::Invalid("record version provenance"))?;
    provenance.insert("summary".into(), json!("exact-record-quotation"));
    provenance.insert("source_summary_available".into(), json!(true));
    provenance.insert("summary_source_language".into(), json!(language));
    provenance.insert("summary_source_pointer".into(), json!(pointer));
    Ok(())
}
fn canonical_record_digest(value: &Value) -> Result<String> {
    fn only_integer_numbers(value: &Value) -> bool {
        match value {
            Value::Number(number) => number.as_i64().is_some() || number.as_u64().is_some(),
            Value::Array(items) => items.iter().all(only_integer_numbers),
            Value::Object(items) => items.values().all(only_integer_numbers),
            _ => true,
        }
    }
    if !only_integer_numbers(value) {
        return Err(Error::Invalid("unsupported exact record floating number"));
    }
    record_ref_digest(value)
}
fn record_view<'a>(item: &'a Value, native: &str) -> Result<&'a Value> {
    let object = item
        .as_object()
        .ok_or(Error::Invalid("record version owner object"))?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "node_id" | "node_kind" | "source_ref" | "properties" | "label" | "identity_status"
        )
    }) || required(item, "node_kind")? != "record-version"
        || item
            .get("label")
            .is_some_and(|v| v.as_str() != Some("Exact record version"))
        || item
            .get("identity_status")
            .is_some_and(|v| v.as_str() != Some("not_applicable"))
    {
        return Err(Error::Invalid("record version closed envelope"));
    }
    let props = item
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("record version properties"))?;
    if props.len() != 1 {
        return Err(Error::Invalid("record version property coverage"));
    }
    let view = props
        .get("record_version_view")
        .ok_or(Error::Invalid("record version view"))?;
    let view_obj = view
        .as_object()
        .ok_or(Error::Invalid("record version view object"))?;
    if view_obj.len() != 10
        || view_obj.keys().any(|k| {
            !matches!(
                k.as_str(),
                "schema_version"
                    | "record_ref"
                    | "record_kind"
                    | "status"
                    | "reason"
                    | "version_status"
                    | "record"
                    | "provenance"
                    | "grants_current_use"
                    | "performs_assessment"
            )
        })
        || required(view, "schema_version")? != RECORD_SCHEMA
        || !matches!(required(view, "record_kind")?, "claim" | "metadata")
        || !matches!(
            required(view, "status")?,
            "available" | "missing" | "stale" | "corrupt" | "access-restricted" | "over-budget"
        )
        || view.get("provenance").and_then(Value::as_object).is_none()
        || view.get("grants_current_use") != Some(&Value::Bool(false))
        || view.get("performs_assessment") != Some(&Value::Bool(false))
        || required(view, "reason")?.chars().count() > 256
    {
        return Err(Error::Invalid("record version view status"));
    }
    let reference = view
        .get("record_ref")
        .ok_or(Error::Invalid("record version reference"))?;
    let ref_obj = reference
        .as_object()
        .ok_or(Error::Invalid("record version reference object"))?;
    if ref_obj.len() != 3
        || ref_obj
            .keys()
            .any(|k| !matches!(k.as_str(), "id" | "version" | "digest"))
        || !valid_tos_id(required(reference, "id")?)
        || required(reference, "id")?.starts_with("tos.claim.")
            != (required(view, "record_kind")? == "claim")
        || reference
            .get("version")
            .and_then(Value::as_u64)
            .is_none_or(|v| v == 0 || v > MAX_ADDRESS)
        || Digest256::from_prefixed(required(reference, "digest")?).is_err()
        || format!("record-version:{}", record_ref_digest(reference)?) != native
    {
        return Err(Error::Invalid("record version exact reference"));
    }
    if required(view, "status")? == "available" {
        let record = view
            .get("record")
            .filter(|record| record.is_object())
            .ok_or(Error::Invalid("available exact record"))?;
        if view
            .get("provenance")
            .and_then(Value::as_object)
            .is_none_or(Map::is_empty)
            || !matches!(
                view.get("version_status").and_then(Value::as_str),
                Some("current" | "historical")
            )
        {
            return Err(Error::Invalid("available exact record provenance"));
        }
        let claim = required(view, "record_kind")? == "claim";
        let identity_field = if claim {
            "claim_id"
        } else {
            native_metadata_identity(record)?
        };
        let version_field = if claim {
            "claim_version"
        } else {
            "record_version"
        };
        if record.get(identity_field) != reference.get("id")
            || record.get(version_field).and_then(Value::as_u64)
                != reference.get("version").and_then(Value::as_u64)
            || format!("sha256:{}", canonical_record_digest(record)?)
                != required(reference, "digest")?
        {
            return Err(Error::Invalid("available exact record binding"));
        }
    } else if !view.get("record").is_some_and(Value::is_null)
        || !view.get("version_status").is_some_and(Value::is_null)
        || !view
            .get("provenance")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty)
    {
        return Err(Error::Invalid("unavailable exact record withholding"));
    }
    Ok(view)
}
fn apply_record_view(output: &mut Value, view: &Value) -> Result<()> {
    let version = view
        .get("record_ref")
        .and_then(|r| r.get("version"))
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("record version number"))?;
    let mut record = Map::new();
    for key in [
        "schema_version",
        "record_ref",
        "record_kind",
        "status",
        "reason",
        "version_status",
        "grants_current_use",
        "performs_assessment",
    ] {
        record.insert(
            key.into(),
            view.get(key)
                .ok_or(Error::Invalid("record version member"))?
                .clone(),
        );
    }
    record.insert(
        "record_pointer".into(),
        Value::String("/attributes/record_version_view/record".into()),
    );
    let record = Value::Object(record);
    let ancestors = output
        .get("semantics")
        .and_then(|s| s.get("type_ancestors"))
        .ok_or(Error::Invalid("record version ancestors"))?
        .clone();
    output["semantics"] = json!({"type_ancestors":ancestors,"record_version":record});
    let display = output
        .get_mut("display")
        .and_then(Value::as_object_mut)
        .ok_or(Error::Invalid("record version display"))?;
    display.insert("title".into(),json!({"default":format!("Exact record version {version}"),
        "ru":format!("Точная версия записи {version}"),"en":format!("Exact record version {version}"),"original":null}));
    display.insert(
        "summary".into(),
        json!({"default":"The exact source wording is not available in this packet.",
        "ru":"Точная исходная формулировка недоступна в этом пакете.",
        "en":"The exact source wording is not available in this packet.","original":null}),
    );
    display.insert("summary_state".into(), Value::String("missing".into()));
    let provenance = display
        .get_mut("provenance")
        .and_then(Value::as_object_mut)
        .ok_or(Error::Invalid("record version provenance"))?;
    provenance.insert(
        "title".into(),
        Value::String("record-version-navigation".into()),
    );
    provenance.insert("source_title_available".into(), Value::Bool(false));
    provenance.insert("record_version".into(), record);
    provenance.insert("summary".into(), Value::String("missing".into()));
    provenance.insert("source_summary_available".into(), Value::Bool(false));
    if required(view, "status")? != "available" {
        return Ok(());
    }
    let body = view
        .get("record")
        .ok_or(Error::Invalid("available record body"))?;
    let refs = output
        .get("source_refs")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("record version source refs"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or(Error::Invalid("record version source ref"))
        })
        .collect::<Result<Vec<_>>>()?;
    if required(view, "record_kind")? == "metadata" {
        output["semantics"]["assertion_contexts"] = json!([{
            "schema_version":"tos_assertion_context_v1","binding_role":"carrier",
            "source_record_digest":stable_digest(body)?,"source_refs":refs,
            "fields":{"record":{"value":body,
                "source_pointer":"/properties/record_version_view/record"}},
            "conflicts":[],"interpretation":"source-declared-not-semantic-assessment"}]);
        if let Some(wording) = text(body.get("notes")) {
            let language = body
                .get("field_languages")
                .and_then(|value| value.get("notes"))
                .and_then(|value| text(value.get("language")));
            let display = output
                .get_mut("display")
                .and_then(Value::as_object_mut)
                .ok_or(Error::Invalid("record version display"))?;
            quoted_summary(
                display,
                wording,
                language,
                "/attributes/record_version_view/record/notes",
            )?;
        }
    } else {
        let synthetic = json!({"properties":{"source_claim":body},"source_refs":refs});
        if let Some(mut context) = assertion_context(&synthetic, &refs)? {
            context["source_record_digest"] = json!(stable_digest(body)?);
            if let Some(fields) = context.get_mut("fields").and_then(Value::as_object_mut) {
                for field in fields.values_mut() {
                    let pointer = field
                        .get("source_pointer")
                        .and_then(Value::as_str)
                        .ok_or(Error::Invalid("record assertion pointer"))?;
                    field["source_pointer"] = json!(pointer.replacen(
                        "/properties/source_claim/",
                        "/properties/record_version_view/record/",
                        1
                    ));
                }
            }
            output["semantics"]["assertion_contexts"] = json!([context]);
        }
        let qualifiers = body.get("qualifiers").and_then(Value::as_object);
        if let Some(wording) = qualifiers.and_then(|value| text(value.get("statement"))) {
            let language = qualifiers.and_then(|value| text(value.get("statement_language")));
            let display = output
                .get_mut("display")
                .and_then(Value::as_object_mut)
                .ok_or(Error::Invalid("record version display"))?;
            quoted_summary(
                display,
                wording,
                language,
                "/attributes/record_version_view/record/qualifiers/statement",
            )?;
        }
    }
    Ok(())
}

impl<'a> NavigationNodeNormalizer<'a> {
    pub fn new(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: NavigationNodeLimits,
    ) -> Result<Self> {
        limits.validate()?;
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        if vocabulary.shared_entity_id_grammars.len() != 1
            || vocabulary.shared_entity_id_grammars[0] != SHARED_ID_GRAMMAR
        {
            return Err(Error::Invalid("navigation shared identity grammar profile"));
        }
        let descriptor = SourceRow::parse(descriptor_bytes, 1024 * 1024)?;
        let identity = descriptor
            .value()
            .get("identity")
            .ok_or(Error::Invalid("navigation descriptor identity"))?;
        let source_graph_id = required(identity, "source_dossier_graph_id")?.to_owned();
        let dossier_kinds = identity
            .get("source_dossier_kinds")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("navigation descriptor dossier kinds"))?
            .iter()
            .map(|kind| {
                kind.as_str()
                    .map(str::to_owned)
                    .ok_or(Error::Invalid("navigation dossier kind"))
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if dossier_kinds.is_empty()
            || !vocabulary.sources.iter().any(|source| {
                source.source_graph_id == source_graph_id
                    && source.adapter_profile == ADAPTER_PROFILE
            })
        {
            return Err(Error::Invalid("navigation selected dossier owner"));
        }
        let entity_registry_ref = required(
            descriptor
                .value()
                .get("semantic_registry_refs")
                .and_then(|refs| refs.get("entity"))
                .ok_or(Error::Invalid("navigation entity registry descriptor"))?,
            "source_ref",
        )?
        .to_owned();
        if entity_bytes.is_empty()
            || entity_bytes.len() > MAX_REGISTRY_BYTES
            || Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256
        {
            return Err(Error::Invalid("navigation selected entity registry bytes"));
        }
        let source = SourceRow::parse(entity_bytes, MAX_REGISTRY_BYTES)?;
        let entries = source
            .value()
            .get("types")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("navigation entity registry types"))?;
        let mut types = BTreeMap::new();
        for entry in entries {
            let id = required(entry, "type_id")?.to_owned();
            let parents = entry
                .get("parent_type_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("navigation type parents"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or(Error::Invalid("navigation parent type"))
                })
                .collect::<Result<Vec<_>>>()?;
            let mut mapping_labels: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
            for mapping in entry
                .get("source_mappings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(labels) = mapping
                    .get("labels")
                    .filter(|v| v.as_object().is_some_and(|object| !object.is_empty()))
                {
                    mapping_labels
                        .entry(required(mapping, "source_graph")?.to_owned())
                        .or_default()
                        .insert(
                            required(mapping, "source_kind_id")?.to_owned(),
                            labels.clone(),
                        );
                }
            }
            if types
                .insert(
                    id,
                    TypeEntry {
                        parents,
                        labels: entry.get("labels").cloned(),
                        object_role: entry
                            .get("object_role")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        mapping_labels,
                    },
                )
                .is_some()
            {
                return Err(Error::Invalid("duplicate navigation entity type"));
            }
        }
        Ok(Self {
            registry,
            source_graph_id,
            dossier_kinds,
            entity_registry_ref,
            types,
            ancestors: BTreeMap::new(),
            ancestor_cache_bytes: 0,
            limits,
        })
    }
    fn ancestors(&mut self, type_id: &str) -> Result<Vec<String>> {
        if let Some(cached) = self.ancestors.get(type_id) {
            return Ok(cached.clone());
        }
        let mut seen = BTreeSet::new();
        let mut stack = vec![type_id.to_owned()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let entry = self
                .types
                .get(&id)
                .ok_or(Error::Invalid("unknown navigation entity type"))?;
            stack.extend(entry.parents.iter().cloned());
            if seen.len() > 4096 {
                return Err(Error::Budget("navigation type ancestors"));
            }
        }
        let values = seen.into_iter().collect::<Vec<_>>();
        let bytes = values
            .iter()
            .try_fold(0usize, |sum, value| sum.checked_add(value.len()))
            .ok_or(Error::Budget("navigation ancestor cache"))?;
        self.ancestor_cache_bytes = self
            .ancestor_cache_bytes
            .checked_add(bytes)
            .filter(|sum| *sum <= self.limits.max_ancestor_cache_bytes)
            .ok_or(Error::Budget("navigation ancestor cache"))?;
        self.ancestors.insert(type_id.to_owned(), values.clone());
        Ok(values)
    }
    /// Exact base `_normalize_node` for the supported source-navigation
    /// envelopes. No global inherited-view or readable-context claim follows.
    pub fn normalize_base(
        &mut self,
        raw: &SeekRow,
        prepared: &NavigationPrepareReceipt,
    ) -> Result<NavigationBaseNode> {
        if prepared.source_graph != self.source_graph_id
            || prepared.source_cut.is_empty()
            || prepared.final_graph_rows_written
            || Digest256::from_hex(&prepared.dependency_root_sha256).is_err()
            || raw.source_graph != prepared.source_graph
            || raw.source_order.is_some()
        {
            return Err(Error::Invalid("navigation selected raw node"));
        }
        let mut node = self.normalize_supplied_node(raw)?;
        node.source_cut = prepared.source_cut.clone();
        node.prepared_dependency_root_sha256 = prepared.dependency_root_sha256.clone();
        Ok(node)
    }
    /// Normalize an explicitly supplied source-navigation carrier without
    /// asserting sealed-cut membership or complete dependency admission.
    pub fn normalize_supplied_node(&mut self, raw: &SeekRow) -> Result<NavigationBaseNode> {
        if raw.source_graph != self.source_graph_id
            || raw.source_order.is_some()
            || raw.id.is_empty()
            || raw.id.len() > 4096
        {
            return Err(Error::Invalid("navigation supplied raw node"));
        }
        if raw.payload.len() > self.limits.max_raw_bytes {
            return Err(Error::Budget("navigation raw node bytes"));
        }
        if Digest256::of_bytes(&raw.payload).to_hex() != raw.payload_sha256 {
            return Err(Error::Invalid("navigation raw node digest"));
        }
        let source = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        let item = source.value();
        let native = required(item, "node_id")?;
        if native != raw.id || native.trim() != native {
            return Err(Error::Invalid("navigation native node ID"));
        }
        let kind = required(item, "node_kind")?;
        if required(item, "source_ref")?.trim().is_empty() {
            return Err(Error::Invalid("navigation node source ref"));
        }
        let record = kind == "record-version";
        let view = if record {
            Some(record_view(item, native)?)
        } else {
            owner_envelope(item)?;
            None
        };
        let resolved = self.registry.entity(&raw.source_graph, kind);
        let type_id = resolved.type_id.to_owned();
        let mapped = type_id != self.registry.fallback_entity_type_id();
        if record && type_id != "tos.entity.record-version" {
            return Err(Error::Invalid("record version exact type mapping"));
        }
        let ancestors = self.ancestors(&type_id)?;
        let type_entry = self
            .types
            .get(&type_id)
            .ok_or(Error::Invalid("navigation mapped type"))?;
        let labels = type_entry
            .mapping_labels
            .get(&raw.source_graph)
            .and_then(|m| m.get(kind))
            .or(type_entry.labels.as_ref());
        let display = source_navigation_node_display(
            &source,
            kind,
            labels,
            type_entry.object_role.as_deref(),
        )?;
        let attrs = attributes(item)?;
        let source_record = source.source_record(&attrs)?;
        let refs = source.source_refs(&[]);
        let mut semantics = node_semantics(item, kind, &refs)?;
        semantics["type_ancestors"] = json!(ancestors);
        let normalized_id = format!("{}:{native}", raw.source_graph);
        let entity_id = [
            item.get("properties").and_then(|p| p.get("record_id")),
            item.get("record_id"),
            item.get("node_id"),
        ]
        .into_iter()
        .find_map(|v| text(v).filter(|s| s.starts_with("tos.")))
        .unwrap_or(&normalized_id)
        .to_owned();
        let graph_layers = if item.get("graph_layers").and_then(Value::as_array).is_some() {
            string_list(item.get("graph_layers"))
        } else {
            Vec::new()
        };
        let graph_layers = if graph_layers.is_empty() {
            text(item.get("layer"))
                .map(|s| vec![Value::String(s.to_owned())])
                .unwrap_or_default()
        } else {
            graph_layers
        };
        let mut output = json!({
            "id":normalized_id,"entity_id":entity_id,"native_id":native,
            "source_graph":raw.source_graph,"kind_id":kind,"type_id":type_id,
            "type_mapping":{"status":if mapped {"mapped"} else {"unmapped"},
                "source_kind_id":kind,"registry_ref":self.entity_registry_ref},
            "display":display,"epistemic":epistemic(item),
            "graph_layers":graph_layers,"view_ids":string_list(item.get("view_ids")),
            "source_refs":refs,"attributes":attrs,
            "semantics":semantics,"source_record":source_record,
        });
        if let Some(dossier) = source_dossier_id(&self.dossier_kinds, kind, native) {
            output["source_dossier_ref"] = Value::String(dossier.to_owned());
        }
        if let Some(view) = view {
            apply_record_view(&mut output, view)?;
        }
        stamp_content_revision(&mut output, self.limits.max_output_bytes)?;
        let content_revision = required(&output, "content_revision")?.to_owned();
        Ok(NavigationBaseNode {
            value: output,
            ordered_context_raw: raw.payload.clone(),
            native_id: native.to_owned(),
            source_graph: raw.source_graph.clone(),
            source_cut: String::new(),
            prepared_dependency_root_sha256: String::new(),
            raw_node_sha256: raw.payload_sha256.clone(),
            entity_registry_sha256: self.registry.entity_sha256.clone(),
            relation_registry_sha256: self.registry.relation_sha256.clone(),
            content_revision,
        })
    }

    /// Private base for Python's missing relation-endpoint node. The caller
    /// must first prove that this exact endpoint is absent from *all* source
    /// node indexes. This method verifies the supplied owner edge itself but
    /// cannot establish sealed-input membership or global absence.
    pub fn normalize_placeholder(
        &mut self,
        raw_edge: &SeekRow,
        prepared: &NavigationPrepareReceipt,
        endpoint: NavigationEndpoint,
    ) -> Result<NavigationPlaceholderBase> {
        if prepared.source_graph != self.source_graph_id
            || prepared.source_cut.is_empty()
            || prepared.final_graph_rows_written
            || Digest256::from_hex(&prepared.edge_input_root_sha256).is_err()
            || raw_edge.source_graph != prepared.source_graph
            || raw_edge.source_order.is_some()
        {
            return Err(Error::Invalid("navigation placeholder edge binding"));
        }
        self.normalize_relation_endpoint(
            raw_edge,
            &prepared.source_cut,
            &prepared.edge_input_root_sha256,
            endpoint,
        )
    }

    /// Shared missing-endpoint kernel after the assembler has established
    /// global absence and exact raw collection membership. The edge keeps
    /// its own selected source graph and separately sealed collection root.
    pub(crate) fn normalize_relation_endpoint(
        &mut self,
        raw_edge: &SeekRow,
        source_cut: &str,
        edge_input_root: &str,
        endpoint: NavigationEndpoint,
    ) -> Result<NavigationPlaceholderBase> {
        if source_cut.is_empty()
            || raw_edge.source_order.is_some()
            || Digest256::from_hex(edge_input_root).is_err()
        {
            return Err(Error::Invalid("native placeholder input binding"));
        }
        if raw_edge.payload.len() > self.limits.max_raw_bytes {
            return Err(Error::Budget("navigation placeholder edge bytes"));
        }
        if Digest256::of_bytes(&raw_edge.payload).to_hex() != raw_edge.payload_sha256 {
            return Err(Error::Invalid("navigation placeholder edge digest"));
        }
        let edge = SourceRow::parse(&raw_edge.payload, self.limits.max_raw_bytes)?;
        let item = edge.value();
        if required(item, "edge_id")? != raw_edge.id {
            return Err(Error::Invalid("navigation placeholder edge ID"));
        }
        let (endpoint_key, source_key) = match endpoint {
            NavigationEndpoint::From => ("from_id", "from_source_graph"),
            NavigationEndpoint::To => ("to_id", "to_source_graph"),
        };
        let native = text(item.get(endpoint_key))
            .ok_or(Error::Invalid("navigation placeholder endpoint"))?;
        let source_graph = text(item.get(source_key)).unwrap_or(&raw_edge.source_graph);
        let refs = edge.source_refs(&[]);
        let synthetic = json!({"node_id":native,"node_type":"relation-endpoint",
            "source_refs":refs,"authority_layer":item.get("authority_layer")});
        let synthetic_raw = serde_json::to_vec(&synthetic)
            .map_err(|_| Error::Invalid("navigation placeholder JSON"))?;
        if synthetic_raw.len() > self.limits.max_raw_bytes {
            return Err(Error::Budget("navigation placeholder source bytes"));
        }
        let source = SourceRow::parse(&synthetic_raw, self.limits.max_raw_bytes)?;
        let resolved = self.registry.entity(source_graph, "relation-endpoint");
        let type_id = resolved.type_id.to_owned();
        let mapped = type_id != self.registry.fallback_entity_type_id();
        let ancestors = self.ancestors(&type_id)?;
        let type_entry = self
            .types
            .get(&type_id)
            .ok_or(Error::Invalid("navigation placeholder type"))?;
        let labels = type_entry
            .mapping_labels
            .get(source_graph)
            .and_then(|mapping| mapping.get("relation-endpoint"))
            .or(type_entry.labels.as_ref());
        let display = source_navigation_node_display(
            &source,
            "relation-endpoint",
            labels,
            type_entry.object_role.as_deref(),
        )?;
        let attrs = attributes(&synthetic)?;
        let source_record = source.source_record(&attrs)?;
        let normalized_id = format!("{source_graph}:{native}");
        let entity_id = if native.starts_with("tos.") {
            native
        } else {
            &normalized_id
        };
        let mut semantics = node_semantics(&synthetic, "relation-endpoint", &refs)?;
        semantics["type_ancestors"] = json!(ancestors);
        let mut value = json!({"id":normalized_id,"entity_id":entity_id,"native_id":native,
            "source_graph":source_graph,"kind_id":"relation-endpoint","type_id":type_id,
            "type_mapping":{"status":if mapped {"mapped"} else {"unmapped"},
                "source_kind_id":"relation-endpoint","registry_ref":self.entity_registry_ref},
            "display":display,"epistemic":epistemic(&synthetic),"graph_layers":[],"view_ids":[],
            "source_refs":refs,"attributes":attrs,"semantics":semantics,"source_record":source_record});
        stamp_content_revision(&mut value, self.limits.max_output_bytes)?;
        Ok(NavigationPlaceholderBase {
            content_revision: required(&value, "content_revision")?.to_owned(),
            value,
            native_id: native.to_owned(),
            source_graph: source_graph.to_owned(),
            source_cut: source_cut.to_owned(),
            edge_id: raw_edge.id.clone(),
            edge_payload_sha256: raw_edge.payload_sha256.clone(),
            edge_input_root_sha256: edge_input_root.to_owned(),
            ordered_edge_raw: raw_edge.payload.clone(),
            endpoint,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationEndpoint {
    From,
    To,
}

/// No final node may be emitted until the global node index proves this
/// endpoint absent and the edge cut is independently sealed/admitted.
pub struct NavigationPlaceholderBase {
    value: Value,
    ordered_edge_raw: Vec<u8>,
    pub native_id: String,
    pub source_graph: String,
    pub source_cut: String,
    pub edge_id: String,
    pub edge_payload_sha256: String,
    pub edge_input_root_sha256: String,
    pub endpoint: NavigationEndpoint,
    pub content_revision: String,
}
impl NavigationPlaceholderBase {
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn ordered_edge_raw(&self) -> &[u8] {
        &self.ordered_edge_raw
    }
}

/// Private base carrier. Its content revision is valid only before the
/// all-source `_apply_final_node_changes` pass; no conversion to a final-row
/// candidate or `OrderedKnowledgeSink` is offered here.
pub struct NavigationBaseNode {
    value: Value,
    ordered_context_raw: Vec<u8>,
    pub native_id: String,
    pub source_graph: String,
    pub source_cut: String,
    pub prepared_dependency_root_sha256: String,
    pub raw_node_sha256: String,
    pub entity_registry_sha256: String,
    pub relation_registry_sha256: String,
    pub content_revision: String,
}
impl NavigationBaseNode {
    pub fn value(&self) -> &Value {
        &self.value
    }
    /// Exact owner JSON bytes retain member insertion order for a later
    /// readable-context compiler. This private witness is not final context.
    pub fn ordered_context_raw(&self) -> &[u8] {
        &self.ordered_context_raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_normalization::stable_digest;

    #[test]
    fn exact_record_digest_is_independent_of_object_insertion_order() {
        let first: Value =
            serde_json::from_str(r#"{"z":{"b":2,"a":1},"a":[{"y":false,"x":null}]}"#).unwrap();
        let reordered: Value =
            serde_json::from_str(r#"{"a":[{"x":null,"y":false}],"z":{"a":1,"b":2}}"#).unwrap();
        assert_eq!(
            record_ref_digest(&first).unwrap(),
            record_ref_digest(&reordered).unwrap()
        );
    }

    // One independent frozen CPython `_normalize_node` oracle fixture spans
    // the owner's base node variants. None exercises a global finalization.
    #[test]
    fn python_oracle_native_nodes_preserve_owner_and_registry_semantics() {
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let registry = KnowledgeRegistry::parse(entity, relation).unwrap();
        let descriptor = include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        );
        let document: Value = serde_json::from_slice(descriptor).unwrap();
        let mut adapters = document["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|source| source["adapter_profile"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        adapters.push(
            document["extension_adapter_profile"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        let vocabulary = QueryVocabulary::parse(
            descriptor,
            &adapters.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .unwrap();
        let limits = NavigationNodeLimits {
            max_raw_bytes: 4096,
            max_output_bytes: 32768,
            max_ancestor_cache_bytes: 32768,
        };
        let mut normalizer =
            NavigationNodeNormalizer::new(&registry, entity, &vocabulary, descriptor, limits)
                .unwrap();
        let prepared = NavigationPrepareReceipt {
            source_graph: "source-navigation".into(),
            input_role: "source-navigation".into(),
            source_cut: "fixture-cut".into(),
            nodes: 9,
            edges: 0,
            endpoint_refs: 0,
            unresolved_endpoint_refs: 0,
            header_claim_sha256: "0".repeat(64),
            header_claim_rights_count: 0,
            node_input_root_sha256: "0".repeat(64),
            edge_input_root_sha256: "0".repeat(64),
            dependency_root_sha256: "1".repeat(64),
            external_dependencies: &[],
            final_graph_rows_written: false,
        };
        let ordinary = json!({"node_id":"tos.work.alpha","node_kind":"work","label":"Alpha",
            "source_ref":"ToS/a.json","identity_status":"not_applicable","properties":{}});
        let reference = json!({"id":"tos.claim.sample","version":1,"digest":format!("sha256:{}","0".repeat(64))});
        let native = format!("record-version:{}", record_ref_digest(&reference).unwrap());
        let view = json!({"schema_version":RECORD_SCHEMA,"record_ref":reference,"record_kind":"claim",
            "status":"missing","reason":"not retained","version_status":null,"record":null,
            "provenance":{},"grants_current_use":false,"performs_assessment":false});
        let version = json!({"node_id":native,"node_kind":"record-version","label":"Exact record version",
            "source_ref":"ToS/contracts/record-version-view.schema.json#/properties/record_ref","identity_status":"not_applicable",
            "properties":{"record_version_view":view}});
        let make = |native: &str, kind: &str, properties: Value| {
            let label = kind
                .split('-')
                .map(|word| {
                    let mut chars = word.chars();
                    chars
                        .next()
                        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join("-");
            json!({"node_id":native,"node_kind":kind,"label":label,
                "source_ref":"ToS/test.json","identity_status":"not_applicable",
                "properties":properties})
        };
        let region = make(
            "tos.region.sample",
            "region",
            json!({
            "branch_path":"ToS/philosophy/eras/regions/sample","role":"Area"}),
        );
        let place = make("tos.place.sample", "place", json!({}));
        let temporal = make(
            "tos.time.sample",
            "temporal-assertion",
            json!({"period":"2024-02"}),
        );
        let annotation_claim = make(
            "tos.claim.sample@abc",
            "annotation-claim",
            json!({
            "packet_id":"tos.packet.sample","packet_version":1,"content_available":true,
            "publication_posture":"public","claim_id":"tos.claim.sample","claim_version":1,
            "proposition":{"predicate":"speaks_about"},"claim_status":"proposed"}),
        );
        let claim = make(
            "tos.claim.nav",
            "claim",
            json!({"claim_id":"tos.claim.nav",
            "navigation_descriptor":{"schema_version":"nav","purpose":"navigate",
                "standalone":false,"state":"ready","reason":"declared","template":{},
                "claim":{},"title":{"default":"Nav title","ru":"Навигация"}}}),
        );
        let available = |kind: &str, body: Value| {
            let identity = if kind == "claim" {
                "claim_id"
            } else {
                "record_id"
            };
            let version = if kind == "claim" {
                "claim_version"
            } else {
                "record_version"
            };
            let reference = json!({"id":body[identity],"version":body[version],
                "digest":format!("sha256:{}",canonical_record_digest(&body).unwrap())});
            let native = format!("record-version:{}", record_ref_digest(&reference).unwrap());
            let view = json!({"schema_version":RECORD_SCHEMA,"record_ref":reference,
                "record_kind":kind,"status":"available","reason":"resolved",
                "version_status":"historical","record":body,
                "provenance":{"source":{"source_ref":"ToS/archive.json"}},
                "grants_current_use":false,"performs_assessment":false});
            json!({"node_id":native,"node_kind":"record-version","label":"Exact record version",
                "source_ref":"ToS/archive.json","identity_status":"not_applicable",
                "properties":{"record_version_view":view}})
        };
        let available_claim = available(
            "claim",
            json!({"claim_id":"tos.claim.historical",
            "claim_version":1,"qualifiers":{"statement":"Историческое утверждение",
                "statement_language":"ru"},"subject_ref":"tos.work.alpha",
            "predicate":"supports","object":"tos.work.beta"}),
        );
        let available_metadata = available(
            "metadata",
            json!({"record_id":"tos.work.historical",
            "record_version":2,"notes":"A preserved note",
            "field_languages":{"notes":{"language":"en"}}}),
        );
        for (source, expected_revision, expected_digest, expected_record) in [
            (
                ordinary,
                "ec2eedf9ed7c52f98640396fb018c7f9ec724d231405bafb7120c0755eaa2c68",
                "4d63af04a7f6b7822a3203dcfa0f8d76ffef5e0846124d074e86562a2b0ea0f4",
                "c789751ce807770f867b3d0054c39dc88d88244577c8eeb119d6e176440eb278",
            ),
            (
                version,
                "5ca3e9f591034ff718159812884cab1aa7c10564bce639c70649e4cc80e2badb",
                "eb70e6b8b12305ae502194559c2eec555db5cadcea9271968ab904f07a17dfdb",
                "6de9384fe8f9e5611e534d744a22ef22d9be29b151d450449e61d28091dc0f31",
            ),
            (
                region,
                "8ae5a79328bd8d784a907746031d920db0f50114f47bb1c44480b4c564e46dc0",
                "7f13e186fa63dcd2a9300f01ef57aa253fa83409a0d5c2c6fb2204287d3546f5",
                "733c84e2326e36b4d6998326895ba7ec06dc2a7603efc5fac6641ed12f8d5b71",
            ),
            (
                place,
                "1f01c7c824e9308acac6be663ce5263fda9bddfe7cf982898692e61f25404acc",
                "95d09b273a161512d36d79fbefe2d2f79190090e7e2dcc3da9d10d51068d3d1f",
                "fa6523ff0aaa015fee7cbde47221c6252817f93d2ffb61aa0ace43d78a8f35ac",
            ),
            (
                temporal,
                "0996d753da11547a929c00f3fb8c60584f00cc16b9f6ba03bf4cb1c3389fc4b9",
                "bcd4914f7c57aa6b5fcbe50418ad7e791e27c820a605bbb6ef3abe24efb55e3a",
                "a7a850600cdd862c2cf015d97350e3ac80c145cd3bf4e4e0d8b3e713ae14e286",
            ),
            (
                annotation_claim,
                "d2964e95c09e9bb37a0add27ec7587a64c51d14d65c627f840e86f534d6a8aec",
                "847490af89419ce1413b169a2b5699f4186befd4a3e693582f7fb507975d93f0",
                "be3345b47f3a6f90af46dbd2731a79eded85f96307278d82801f9be471c14bd6",
            ),
            (
                claim,
                "6db38cd9ac0ab64e1106d8957b9586d166a931abe87e6a71f4ecbb0335a5b51e",
                "508747f718c6bd80eb4112d7cc10d9db6f08aab3eb1ed4c7da348e0342d2ae6c",
                "5fe9be0f621add9eb027932c7e5a3078876daa1ca28b89a62f2cb6d1cd571fe5",
            ),
            (
                available_claim,
                "e77928f3bbc31f05f4b8659dd5f5e39bb4af74d6d09b1f48c270e6ee74f5b39a",
                "eb8b619689bc75dca3a9e6b218fd1d73f93b23f170327c01128e5f19cda899a1",
                "0c22c9eb9148ee5ee7b8ca64c889285f58357a5aedaef741ef8c7ad25911b5e8",
            ),
            (
                available_metadata,
                "18e44d33cb31196124fbf05ed599643ad598c8bd5d47fcd2488feba9ee60e58a",
                "a52401a7e8b7551dd426c15dcef4643b976dd987733f0a29bea38e5823594146",
                "71a588fb1ce877bdf5dc2f747cb53758a1e437946e95373d48c66042210a5f53",
            ),
        ] {
            let native = source["node_id"].as_str().unwrap();
            let payload = serde_json::to_vec(&source).unwrap();
            let row = SeekRow {
                id: native.into(),
                source_graph: "source-navigation".into(),
                source_order: None,
                payload_sha256: Digest256::of_bytes(&payload).to_hex(),
                payload,
            };
            let base = normalizer.normalize_base(&row, &prepared).unwrap();
            let output = base.value();
            assert_eq!(base.content_revision, expected_revision);
            assert_eq!(output["content_revision"], expected_revision);
            assert_eq!(stable_digest(output).unwrap(), expected_digest);
            assert_eq!(output["source_record"]["digest"], expected_record);
            assert_eq!(output["native_id"], native);
            if native == "tos.work.alpha" {
                assert_eq!(output["source_dossier_ref"], native);
                assert_eq!(output["display"]["kind_label"]["ru"], "Произведение");
                assert_eq!(
                    output["semantics"]["type_ancestors"]
                        .as_array()
                        .unwrap()
                        .len(),
                    4
                );
            }
        }
        let edge = json!({"edge_id":"e:missing","from_id":"tos.work.alpha",
            "to_id":"tos.work.missing","predicate_id":"related_to",
            "edge_kind":"authored_source_planting","review_status":"not_applicable",
            "source_refs":["ToS/a.json"]});
        let payload = serde_json::to_vec(&edge).unwrap();
        let row = SeekRow {
            id: "e:missing".into(),
            source_graph: "source-navigation".into(),
            source_order: None,
            payload_sha256: Digest256::of_bytes(&payload).to_hex(),
            payload,
        };
        let placeholder = normalizer
            .normalize_placeholder(&row, &prepared, NavigationEndpoint::To)
            .unwrap();
        assert_eq!(placeholder.native_id, "tos.work.missing");
        assert_eq!(
            placeholder.content_revision,
            "59d079a58f891a645a8a0d051407e16a9e0390f46f012080244180166826eade"
        );
        assert_eq!(
            stable_digest(placeholder.value()).unwrap(),
            "9cd2a4464eb99e895cc8666ed4fd10c9760af8648eb740b02d798cb7a234311e"
        );
        assert_eq!(
            placeholder.value()["source_record"]["digest"],
            "89faa7450f7d6b0ba5f5081978f6182bd1d8d5c45c599af34a4749f5ad5c86e1"
        );
        assert_eq!(placeholder.ordered_edge_raw(), row.payload.as_slice());
    }
}
