//! Observatory filter rules shared by the browser editor and query compiler.

use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};

fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(name, value)| (JsonString::from_utf8(name), value))
            .collect(),
    )
}
fn scalar(value: &JsonValue) -> bool {
    match get(value, "kind").and_then(JsonValue::as_str) {
        Some("null" | "boolean") => true,
        Some("string") => get(value, "units")
            .and_then(JsonValue::as_u64)
            .is_some_and(|units| units <= 1024),
        Some("number") => matches!(get(value, "finite"), Some(JsonValue::Bool(true))),
        _ => false,
    }
}
fn valid_value(value: &JsonValue) -> bool {
    if get(value, "kind").and_then(JsonValue::as_str) == Some("array") {
        let Some(length) = get(value, "length").and_then(JsonValue::as_u64) else {
            return false;
        };
        length <= 100
            && get(value, "items")
                .and_then(JsonValue::as_array)
                .is_some_and(|items| {
                    items.len() as u64 == length
                        && items.iter().all(|item| {
                            scalar(item)
                                || get(item, "kind").and_then(JsonValue::as_str) == Some("hole")
                        })
                })
    } else {
        scalar(value)
    }
}
fn rule(value: &JsonValue, relation: bool) -> Result<JsonValue, &'static str> {
    let selector = get(value, "selector").and_then(JsonValue::as_str);
    let identifier = get(value, "id");
    let op = get(value, "op").and_then(JsonValue::as_str);
    let body = get(value, "value_shape");
    if !matches!(selector, Some("field" | "property_id"))
        || relation && selector == Some("property_id")
        || !matches!(identifier, Some(JsonValue::String(text)) if (1..=256).contains(&text.units().len()))
        || !matches!(
            op,
            Some(
                "eq" | "neq"
                    | "in"
                    | "contains"
                    | "prefix"
                    | "exists"
                    | "gt"
                    | "gte"
                    | "lt"
                    | "lte"
            )
        )
        || !body.is_some_and(valid_value)
    {
        return Err("invalid_conditions_rule");
    }
    Ok(object(vec![
        ("selector", get(value, "selector").unwrap().clone()),
        ("id", identifier.unwrap().clone()),
        ("op", get(value, "op").unwrap().clone()),
    ]))
}

/// Normalize one bounded set of node and relation conditions. The host
/// transports owned selector fields, bounded scalar descriptors and original
/// enumerable root keys. Accepted value bytes never cross or echo through this
/// rule: the synchronous host clones its original value exactly once.
pub fn normalize_observatory_conditions_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_conditions_shape")?;
    let request = document.root();
    if get(request, "shape").and_then(JsonValue::as_str) != Some("object") {
        return Err("invalid_conditions_shape");
    }
    let keys = get(request, "keys")
        .and_then(JsonValue::as_array)
        .ok_or("invalid_conditions_shape")?;
    if keys.len() > 2
        || !keys
            .iter()
            .all(|item| matches!(item.as_str(), Some("nodes" | "relations")))
    {
        return Err("invalid_conditions_shape");
    }
    let max = get(request, "max_conditions")
        .and_then(|value| match value {
            JsonValue::Number(number) => number.lexeme.parse::<f64>().ok(),
            _ => None,
        })
        .filter(|number| number.is_finite() && *number >= 0.0 && number.fract() == 0.0)
        .ok_or("invalid_conditions_limit")?;
    let mut normalized = Vec::with_capacity(2);
    for (kind, relation) in [("nodes", false), ("relations", true)] {
        let group = get(request, kind).ok_or("invalid_conditions_limit")?;
        let length = get(group, "length")
            .and_then(JsonValue::as_u64)
            .ok_or("invalid_conditions_limit")?;
        if length as f64 > max {
            return Err("invalid_conditions_limit");
        }
        let entries = get(group, "items")
            .and_then(JsonValue::as_array)
            .ok_or("invalid_conditions_limit")?;
        if entries.len() as u64 != length {
            return Err("invalid_conditions_limit");
        }
        let rules = entries
            .iter()
            .map(|item| {
                if matches!(get(item, "hole"), Some(JsonValue::Bool(true))) {
                    Ok(JsonValue::Null)
                } else {
                    rule(item, relation)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        normalized.push((kind, JsonValue::Array(rules)));
    }
    emit_value_preserved_json(
        &object(normalized),
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_conditions_output")
}

#[derive(Clone)]
struct ConditionEntry {
    selector: &'static str,
    id: JsonString,
    value_type: &'static str,
    operators: Vec<&'static str>,
}

const OPERATORS: &[&str] = &[
    "eq", "neq", "in", "contains", "prefix", "exists", "gt", "gte", "lt", "lte",
];

fn at<'a>(value: Option<&'a JsonValue>, key: &str) -> Option<&'a JsonValue> {
    value.and_then(|value| get(value, key))
}
fn values<'a>(value: Option<&'a JsonValue>) -> &'a [JsonValue] {
    value
        .and_then(JsonValue::as_array)
        .map_or(&[], |items| items)
}
fn includes_string(value: Option<&JsonValue>, needle: &str) -> bool {
    values(value)
        .iter()
        .any(|item| item.as_str() == Some(needle))
}
fn field_type(id: &str) -> Option<&'static str> {
    Some(match id {
        "kind_id"
        | "type_id"
        | "predicate_id"
        | "relation_type_id"
        | "display.title.default"
        | "display.title.ru"
        | "display.title.en"
        | "display.summary.default"
        | "display.summary_state"
        | "display.label.default"
        | "display.statement.default"
        | "display.explanation_state"
        | "epistemic.authority_layer"
        | "epistemic.review_posture"
        | "epistemic.canon_status"
        | "source_dossier_ref" => "string",
        "graph_layers" | "view_ids" | "source_refs" => "string-array",
        _ => return None,
    })
}
fn type_operators(value_type: &str) -> &'static [&'static str] {
    match value_type {
        "string" => &["eq", "neq", "in", "contains", "prefix", "exists"],
        "string-array" => &["eq", "neq", "in", "contains", "exists"],
        "number" => &["eq", "neq", "in", "gt", "gte", "lt", "lte", "exists"],
        "boolean" => &["eq", "neq", "exists"],
        _ => &[],
    }
}
fn value_contract(operator: &str) -> Option<&'static str> {
    Some(match operator {
        "eq" => "scalar",
        "neq" => "scalar",
        "in" => "scalar-or-scalar-array",
        "contains" => "scalar-or-scalar-array",
        "prefix" => "string",
        "exists" => "boolean",
        "gt" | "gte" | "lt" | "lte" => "number",
        _ => return None,
    })
}
fn allowed_field(context: &JsonValue, kind: &str, id: &str) -> bool {
    let wire_kind = if kind == "nodes" { "node" } else { "relation" };
    let field_name = format!("{wire_kind}Field");
    let field = at(at(get(context, "schema"), "$defs"), &field_name);
    values(at(field, "anyOf"))
        .iter()
        .any(|part| includes_string(get(part, "enum"), id))
}
fn field_filter<'a>(context: &'a JsonValue, kind: &str) -> Option<&'a JsonValue> {
    let wire_kind = if kind == "nodes" { "node" } else { "relation" };
    let filter_name = format!("{wire_kind}Filter");
    at(at(get(context, "schema"), "$defs"), &filter_name)
}
fn operators_for(
    context: &JsonValue,
    kind: &str,
    value_type: &str,
    allowed: Option<&JsonValue>,
) -> Vec<&'static str> {
    let caps = at(get(context, "catalog"), "capabilities");
    let contracts = get(caps.unwrap_or(&JsonValue::Null), "operator_value_contracts");
    let catalog_ops = get(caps.unwrap_or(&JsonValue::Null), "filter_operators");
    let filter_ops = at(field_filter(context, kind), "properties")
        .and_then(|properties| get(properties, "op"))
        .and_then(|op| get(op, "enum"));
    OPERATORS
        .iter()
        .copied()
        .filter(|operator| type_operators(value_type).contains(operator))
        .filter(|operator| includes_string(allowed, operator))
        .filter(|operator| includes_string(catalog_ops, operator))
        .filter(|operator| includes_string(filter_ops, operator))
        .filter(|operator| {
            value_contract(operator).is_some_and(|expected| {
                get(contracts.unwrap_or(&JsonValue::Null), operator).and_then(JsonValue::as_str)
                    == Some(expected)
            })
        })
        .collect()
}
fn property_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("tos.property.") else {
        return false;
    };
    !suffix.is_empty()
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
fn js_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(value) => *value,
        JsonValue::Number(number) => number.lexeme.parse::<f64>().is_ok_and(|value| value != 0.0),
        JsonValue::String(value) => !value.units().is_empty(),
        JsonValue::Array(_) | JsonValue::Object(_) => true,
    }
}
fn property_selector_supported(context: &JsonValue) -> bool {
    let caps = at(get(context, "catalog"), "capabilities");
    let contract = get(caps.unwrap_or(&JsonValue::Null), "property_filters");
    if get(contract.unwrap_or(&JsonValue::Null), "selector").and_then(JsonValue::as_str)
        != Some("property_id")
        || get(contract.unwrap_or(&JsonValue::Null), "scope").and_then(JsonValue::as_str)
            != Some("node-query-and-path-node-query")
        || get(contract.unwrap_or(&JsonValue::Null), "binding").and_then(JsonValue::as_str)
            != Some("same-graph-snapshot")
        || get(contract.unwrap_or(&JsonValue::Null), "operators").and_then(JsonValue::as_str)
            != Some("declared-per-property")
    {
        return false;
    }
    let filter = field_filter(context, "nodes").unwrap_or(&JsonValue::Null);
    if !at(get(filter, "properties"), "property_id").is_some_and(js_truthy) {
        return false;
    }
    values(get(filter, "oneOf")).iter().any(|part| {
        values(get(part, "required"))
            .iter()
            .any(|required| required.as_str() == Some("property_id"))
    })
}
fn valid_applies_to(value: &JsonValue) -> bool {
    value.as_array().is_some_and(|items| {
        items.iter().all(|item| {
            matches!(item, JsonValue::String(text) if (1..=256).contains(&text.units().len()))
        })
    })
}
fn condition_entries(context: &JsonValue, kind: &str) -> Vec<ConditionEntry> {
    let wire_kind = if kind == "nodes" { "node" } else { "relation" };
    let caps = at(get(context, "catalog"), "capabilities");
    let fields_name = format!("{wire_kind}_fields");
    let field_ids = get(caps.unwrap_or(&JsonValue::Null), &fields_name);
    let all_operators = JsonValue::Array(
        OPERATORS
            .iter()
            .map(|operator| JsonValue::String(JsonString::from_utf8(operator)))
            .collect(),
    );
    let mut entries = Vec::new();
    for field in values(field_ids) {
        let Some(id) = field.as_str() else {
            continue;
        };
        let Some(value_type) = field_type(id) else {
            continue;
        };
        if !allowed_field(context, kind, id) {
            continue;
        }
        let operators = operators_for(context, kind, value_type, Some(&all_operators));
        if !operators.is_empty() {
            entries.push(ConditionEntry {
                selector: "field",
                id: JsonString::from_utf8(id),
                value_type,
                operators,
            });
        }
    }
    if kind != "nodes" || !property_selector_supported(context) {
        return entries;
    }
    let properties = at(
        at(get(context, "catalog"), "semantic_registries"),
        "properties",
    );
    let Some(properties) = properties
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() <= 5000)
    else {
        return entries;
    };
    let mut counts = Vec::<(String, usize)>::new();
    for item in properties {
        let Some(id) = get(item, "property_id").and_then(JsonValue::as_str) else {
            continue;
        };
        if let Some((_, count)) = counts.iter_mut().find(|(known, _)| known == id) {
            *count += 1;
        } else {
            counts.push((id.to_owned(), 1));
        }
    }
    for item in properties {
        let Some(id) = get(item, "property_id").and_then(JsonValue::as_str) else {
            continue;
        };
        if !property_id(id)
            || counts
                .iter()
                .find(|(known, _)| known == id)
                .map(|(_, count)| *count)
                != Some(1)
            || !valid_applies_to(get(item, "applies_to").unwrap_or(&JsonValue::Null))
        {
            continue;
        }
        let Some(value_type) = get(item, "value_type").and_then(JsonValue::as_str) else {
            continue;
        };
        if !type_operators(value_type).is_empty() {
            let operators = operators_for(context, kind, value_type, get(item, "operators"));
            if !operators.is_empty() {
                entries.push(ConditionEntry {
                    selector: "property_id",
                    id: JsonString::from_utf8(id),
                    value_type: match value_type {
                        "string" => "string",
                        "string-array" => "string-array",
                        "number" => "number",
                        "boolean" => "boolean",
                        _ => continue,
                    },
                    operators,
                });
            }
        }
    }
    entries
}
fn number(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn condition_entry_json(entry: &ConditionEntry) -> JsonValue {
    object(vec![
        (
            "selector",
            JsonValue::String(JsonString::from_utf8(entry.selector)),
        ),
        ("id", JsonValue::String(entry.id.clone())),
        (
            "value_type",
            JsonValue::String(JsonString::from_utf8(entry.value_type)),
        ),
        (
            "operators",
            JsonValue::Array(
                entry
                    .operators
                    .iter()
                    .map(|operator| JsonValue::String(JsonString::from_utf8(operator)))
                    .collect(),
            ),
        ),
    ])
}
fn parse_condition_request(raw: &[u8]) -> Result<tos_foundation::JsonDocument, &'static str> {
    parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_condition_request")
}

/// Return the live schema/catalog intersection used by the filter editor.
/// Translations, labels and facet suggestions remain presentation data.
pub fn observatory_condition_catalog_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_condition_request(raw)?;
    let request = document.root();
    let kind = get(request, "kind")
        .and_then(JsonValue::as_str)
        .unwrap_or("relations");
    let entries = condition_entries(get(request, "context").unwrap_or(&JsonValue::Null), kind);
    emit_value_preserved_json(
        &JsonValue::Array(entries.iter().map(condition_entry_json).collect()),
        JsonLimits::default(),
    )
    .map_err(|_| "invalid_condition_catalog_output")
}

fn rule_typed_value(
    shape: &JsonValue,
    value_type: &str,
    operator: &str,
) -> Result<(), &'static str> {
    let kind = get(shape, "kind").and_then(JsonValue::as_str);
    if operator == "exists" {
        return if kind == Some("boolean") {
            Ok(())
        } else {
            Err("condition_exists_value")
        };
    }
    let is_array = kind == Some("array");
    let items = if is_array {
        let Some(length) = get(shape, "length").and_then(JsonValue::as_u64) else {
            return Err("condition_value_type");
        };
        let Some(items) = get(shape, "items").and_then(JsonValue::as_array) else {
            return Err("condition_value_type");
        };
        if length == 0 || length > 100 || items.len() as u64 != length {
            return Err("condition_value_type");
        }
        items
    } else {
        std::slice::from_ref(shape)
    };
    let expected = if value_type == "string-array" {
        "string"
    } else {
        value_type
    };
    let values_match = items.iter().all(|item| {
        get(item, "kind").and_then(JsonValue::as_str) == Some("hole")
            || get(item, "kind").and_then(JsonValue::as_str) == Some(expected)
                && (expected != "string"
                    || get(item, "units")
                        .and_then(JsonValue::as_u64)
                        .is_some_and(|units| units <= 1024))
                && (expected != "number"
                    || matches!(get(item, "finite"), Some(JsonValue::Bool(true))))
    });
    if !values_match {
        return Err("condition_value_type");
    }
    if is_array && ["eq", "neq", "prefix", "gt", "gte", "lt", "lte"].contains(&operator) {
        return Err("condition_single_value");
    }
    if operator == "contains" && is_array && value_type != "string-array" {
        return Err("condition_contains_value");
    }
    Ok(())
}
fn filter_limit(context: &JsonValue, kind: &str) -> Option<usize> {
    let wire_kind = if kind == "nodes" { "node" } else { "relation" };
    let query_name = format!("{wire_kind}Query");
    let raw = at(
        at(at(get(context, "schema"), "$defs"), &query_name),
        "properties",
    )
    .and_then(|properties| get(properties, "filters"))
    .and_then(|filters| get(filters, "maxItems"))?;
    js_positive_integer(raw)
}
fn js_positive_integer(value: &JsonValue) -> Option<usize> {
    let JsonValue::Number(number) = value else {
        return None;
    };
    let value = number.lexeme.parse::<f64>().ok()?;
    if !value.is_finite() || value < 1.0 || value.fract() != 0.0 {
        return None;
    }
    if value >= usize::MAX as f64 {
        Some(usize::MAX)
    } else {
        Some(value as usize)
    }
}
pub(crate) fn compile_condition_rules(
    context: &JsonValue,
    kind: &str,
    rules: &JsonValue,
    max_filters: Option<usize>,
    reserve_type_slot: bool,
) -> Result<Vec<JsonValue>, &'static str> {
    let length = get(rules, "length")
        .and_then(JsonValue::as_u64)
        .ok_or("invalid_conditions_limit")?;
    let items = get(rules, "items")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() as u64 == length)
        .ok_or("invalid_conditions_limit")?;
    if length == 0 {
        return Ok(Vec::new());
    }
    let limit = max_filters.ok_or("condition_limit")?;
    let reserved = usize::from(reserve_type_slot);
    if length > 0
        && usize::try_from(length)
            .unwrap_or(usize::MAX)
            .saturating_add(reserved)
            > limit
    {
        return Err("condition_limit");
    }
    let entries = condition_entries(context, kind);
    let mut compiled = Vec::with_capacity(items.len());
    for (index, rule) in items.iter().enumerate() {
        if matches!(get(rule, "hole"), Some(JsonValue::Bool(true))) {
            compiled.push(JsonValue::Null);
            continue;
        }
        let selector = get(rule, "selector").and_then(JsonValue::as_str);
        let id = get(rule, "id").and_then(|value| match value {
            JsonValue::String(id) => Some(id),
            _ => None,
        });
        let operator = get(rule, "op").and_then(JsonValue::as_str);
        let Some(entry) = entries
            .iter()
            .find(|entry| Some(entry.selector) == selector && id == Some(&entry.id))
        else {
            return Err("condition_not_supported");
        };
        let Some(operator) = operator.filter(|operator| entry.operators.contains(operator)) else {
            return Err("condition_not_supported");
        };
        let shape = get(rule, "value_shape").ok_or("invalid_conditions_rule")?;
        if !valid_value(shape) {
            return Err("condition_value_type");
        }
        rule_typed_value(shape, entry.value_type, operator)?;
        let source_index = number(index);
        compiled.push(object(vec![
            (entry.selector, JsonValue::String(entry.id.clone())),
            ("op", JsonValue::String(JsonString::from_utf8(operator))),
            ("value_index", source_index),
        ]));
    }
    Ok(compiled)
}

/// Compile bounded filter drafts using the same live schema/catalog contract
/// exposed to the editor. Values remain in the host and are reattached by index.
pub fn compile_observatory_conditions_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_condition_request(raw)?;
    let request = document.root();
    let context = get(request, "context").unwrap_or(&JsonValue::Null);
    let kind = get(request, "kind")
        .and_then(JsonValue::as_str)
        .unwrap_or("relations");
    let rules = get(request, "rules").ok_or("invalid_conditions_limit")?;
    let reserve_type_slot = matches!(
        get(request, "reserve_type_slot"),
        Some(JsonValue::Bool(true))
    );
    let compiled = compile_condition_rules(
        context,
        kind,
        rules,
        filter_limit(context, kind),
        reserve_type_slot,
    )?;
    emit_value_preserved_json(
        &JsonValue::Array(compiled),
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_conditions_output")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_order_duplicates_and_wtf16_identifier_without_value_echo() {
        let raw = br#"{"shape":"object","keys":["nodes","relations"],"max_conditions":12,"nodes":{"length":3,"items":[{"selector":"field","id":"\ud800","op":"eq","value_shape":{"kind":"array","length":3,"items":[{"kind":"number","finite":true},{"kind":"boolean"},{"kind":"string","units":1}]},"ignored":"private"},{"hole":true},{"selector":"field","id":"\ud800","op":"eq","value_shape":{"kind":"null"}}]},"relations":{"length":0,"items":[]}}"#;
        let bytes = normalize_observatory_conditions_v1(raw).unwrap();
        let parsed = parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        let nodes = get(parsed.root(), "nodes").unwrap().as_array().unwrap();
        assert_eq!(nodes.len(), 3);
        assert!(get(&nodes[0], "ignored").is_none());
        assert!(get(&nodes[0], "value").is_none());
        assert!(get(&nodes[0], "value_shape").is_none());
        assert!(nodes[1].is_null());
        assert_eq!(get(&nodes[0], "id"), get(&nodes[2], "id"));
    }
    #[test]
    fn relation_scope_and_limit_fail_closed() {
        let raw = br#"{"shape":"object","keys":["nodes","relations"],"max_conditions":12,"nodes":{"length":0,"items":[]},"relations":{"length":1,"items":[{"selector":"property_id","id":"p","op":"eq","value_shape":{"kind":"boolean"}}]}}"#;
        assert_eq!(
            normalize_observatory_conditions_v1(raw),
            Err("invalid_conditions_rule")
        );
        let limit = br#"{"shape":"object","keys":["nodes","relations"],"max_conditions":0,"nodes":{"length":1,"items":[]},"relations":{"length":0,"items":[]}}"#;
        assert_eq!(
            normalize_observatory_conditions_v1(limit),
            Err("invalid_conditions_limit")
        );
    }

    #[test]
    fn scalar_descriptors_keep_the_original_admission_boundaries() {
        let shape = |kind: &str| {
            object(vec![(
                "kind",
                JsonValue::String(JsonString::from_utf8(kind)),
            )])
        };
        assert!(scalar(&shape("null")));
        assert!(scalar(&shape("boolean")));
        assert!(!scalar(&shape("invalid")));
        assert!(!scalar(&shape("hole")));
        for (units, accepted) in [(1024, true), (1025, false)] {
            let raw = format!(r#"{{"kind":"string","units":{units}}}"#);
            let parsed = parse_json(
                raw.as_bytes(),
                JsonMode::RequestLastWins,
                JsonLimits::default(),
            )
            .unwrap();
            assert_eq!(scalar(parsed.root()), accepted);
        }
        for finite in [true, false] {
            let raw = format!(r#"{{"kind":"number","finite":{finite}}}"#);
            let parsed = parse_json(
                raw.as_bytes(),
                JsonMode::RequestLastWins,
                JsonLimits::default(),
            )
            .unwrap();
            assert_eq!(scalar(parsed.root()), finite);
        }
        let sparse = br#"{"kind":"array","length":2,"items":[{"kind":"hole"},{"kind":"null"}]}"#;
        let parsed = parse_json(sparse, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        assert!(valid_value(parsed.root()));
        let malformed = br#"{"kind":"array","length":2,"items":[{"kind":"null"}]}"#;
        let parsed =
            parse_json(malformed, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        assert!(!valid_value(parsed.root()));
    }
}
