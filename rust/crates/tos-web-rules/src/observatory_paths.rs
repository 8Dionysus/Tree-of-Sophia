//! Saved Observatory path-draft, schema/catalog, and query-wire rules.
//! The browser retains condition values and catalog/UI presentation; condition
//! admissibility and normalization delegate to the shared condition rule.

use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};

use crate::observatory_conditions::{compile_condition_rules, normalize_observatory_conditions_v1};

fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn at<'a>(value: Option<&'a JsonValue>, key: &str) -> Option<&'a JsonValue> {
    value.and_then(|value| get(value, key))
}
fn values<'a>(value: Option<&'a JsonValue>) -> &'a [JsonValue] {
    value
        .and_then(JsonValue::as_array)
        .map_or(&[], |items| items)
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(name, value)| (JsonString::from_utf8(name), value))
            .collect(),
    )
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn bool(value: bool) -> JsonValue {
    JsonValue::Bool(value)
}
fn number(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn is_hole(value: &JsonValue) -> bool {
    matches!(get(value, "hole"), Some(JsonValue::Bool(true)))
}
fn descriptor_kind<'a>(value: &'a JsonValue, name: &str) -> Option<&'a str> {
    get(value, name).and_then(JsonValue::as_str)
}
fn descriptor_text<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonString> {
    match get(value, name) {
        Some(JsonValue::String(text)) => Some(text),
        _ => None,
    }
}
fn allowed_keys(value: &JsonValue, allowed: &[&str]) -> bool {
    get(value, "keys")
        .and_then(JsonValue::as_array)
        .is_some_and(|keys| {
            keys.len() <= allowed.len()
                && keys
                    .iter()
                    .all(|key| key.as_str().is_some_and(|key| allowed.contains(&key)))
        })
}
fn allowed_path_id(value: &JsonString) -> bool {
    let units = value.units();
    (1..=128).contains(&units.len())
        && units.iter().enumerate().all(|(index, unit)| {
            let ascii = (65..=90).contains(unit)
                || (97..=122).contains(unit)
                || (48..=57).contains(unit)
                || matches!(*unit, 46 | 95 | 58 | 45);
            ascii && (index != 0 || !matches!(*unit, 46 | 95 | 58 | 45))
        })
}
fn valid_type_list(value: &JsonValue) -> Result<(), &'static str> {
    if descriptor_kind(value, "kind") != Some("array") {
        return Err("invalid_path_types");
    }
    let length = get(value, "length")
        .and_then(JsonValue::as_u64)
        .ok_or("invalid_path_types")?;
    if length > 100 {
        return Err("invalid_path_types");
    }
    let items = get(value, "items")
        .and_then(JsonValue::as_array)
        .ok_or("invalid_path_types")?;
    if items.len() as u64 != length {
        return Err("invalid_path_types");
    }
    let mut seen = Vec::<Vec<u16>>::new();
    let mut saw_hole = false;
    for item in items {
        if is_hole(item) {
            if saw_hole {
                return Err("invalid_path_types");
            }
            saw_hole = true;
            continue;
        }
        let Some(value) = descriptor_text(item, "value") else {
            return Err("invalid_path_types");
        };
        if descriptor_kind(item, "kind") != Some("string")
            || value.units().is_empty()
            || value.units().len() > 1024
        {
            return Err("invalid_path_types");
        }
        if seen.iter().any(|known| known.as_slice() == value.units()) {
            return Err("invalid_path_types");
        }
        seen.push(value.units().to_vec());
    }
    Ok(())
}
fn condition_group(request: &JsonValue, kind: &str) -> Result<JsonValue, &'static str> {
    let condition_request = get(request, "condition_request").ok_or("invalid_path_conditions")?;
    let bytes = emit_value_preserved_json(condition_request, JsonLimits::default())
        .map_err(|_| "invalid_path_conditions")?;
    let normalized = normalize_observatory_conditions_v1(&bytes)?;
    let document = parse_json(
        &normalized,
        JsonMode::RequestLastWins,
        JsonLimits::default(),
    )
    .map_err(|_| "invalid_path_conditions")?;
    get(document.root(), kind)
        .cloned()
        .ok_or("invalid_path_conditions")
}
fn normalize_query(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if descriptor_kind(value, "kind") == Some("undefined") {
        return Ok(object(vec![
            ("enabled", bool(true)),
            ("match", text("all")),
            ("conditions", JsonValue::Array(Vec::new())),
        ]));
    }
    if descriptor_kind(value, "kind") != Some("object")
        || !allowed_keys(value, &["enabled", "match", "conditions"])
    {
        return Err("invalid_path_query_shape");
    }
    let enabled = get(value, "enabled").ok_or("invalid_path_query_values")?;
    let enabled = match descriptor_kind(enabled, "kind") {
        Some("undefined") => true,
        Some("boolean") => matches!(get(enabled, "value"), Some(JsonValue::Bool(true))),
        _ => return Err("invalid_path_query_values"),
    };
    let matched = get(value, "match").ok_or("invalid_path_query_values")?;
    let matched = match descriptor_kind(matched, "kind") {
        Some("undefined") => "all",
        Some("string") => {
            match descriptor_text(matched, "value").and_then(|value| value.as_str()) {
                Some("all") => "all",
                Some("any") => "any",
                _ => return Err("invalid_path_query_values"),
            }
        }
        _ => return Err("invalid_path_query_values"),
    };
    let conditions = get(value, "conditions_kind").ok_or("invalid_path_conditions")?;
    let conditions = match conditions.as_str() {
        Some("undefined") => 0,
        Some("array") => get(value, "conditions_length")
            .and_then(JsonValue::as_u64)
            .filter(|length| *length <= 32)
            .ok_or("invalid_path_conditions")? as usize,
        _ => return Err("invalid_path_conditions"),
    };
    let source_kind = get(value, "condition_kind")
        .and_then(JsonValue::as_str)
        .unwrap_or("nodes");
    let normalized = match source_kind {
        "nodes" => condition_group(value, "nodes")?,
        "relations" => condition_group(value, "relations")?,
        _ => return Err("invalid_path_conditions"),
    };
    let actual_length = normalized.as_array().map_or(0, |items| items.len());
    if actual_length != conditions {
        return Err("invalid_path_conditions");
    }
    Ok(object(vec![
        ("enabled", bool(enabled)),
        ("match", text(matched)),
        ("conditions", normalized),
    ]))
}
fn normalize_step(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if is_hole(value) {
        return Ok(object(vec![("hole", bool(true))]));
    }
    if descriptor_kind(value, "kind") != Some("object")
        || !allowed_keys(
            value,
            &[
                "direction",
                "kinds",
                "predicates",
                "nodeQuery",
                "relationQuery",
            ],
        )
    {
        return Err("invalid_path_step");
    }
    let direction = get(value, "direction").ok_or("invalid_path_direction")?;
    let direction = match descriptor_kind(direction, "kind") {
        Some("undefined") => "outgoing",
        Some("string") => {
            match descriptor_text(direction, "value").and_then(|value| value.as_str()) {
                Some("outgoing") => "outgoing",
                Some("incoming") => "incoming",
                Some("either") => "either",
                _ => return Err("invalid_path_direction"),
            }
        }
        _ => return Err("invalid_path_direction"),
    };
    valid_type_list(get(value, "kinds").ok_or("invalid_path_types")?)?;
    valid_type_list(get(value, "predicates").ok_or("invalid_path_types")?)?;
    let node_query = normalize_query(get(value, "node_query").ok_or("invalid_path_query_shape")?)?;
    let relation_query =
        normalize_query(get(value, "relation_query").ok_or("invalid_path_query_shape")?)?;
    Ok(object(vec![
        ("direction", text(direction)),
        ("nodeQuery", node_query),
        ("relationQuery", relation_query),
    ]))
}
fn normalize_path(value: &JsonValue, ids: &mut Vec<Vec<u16>>) -> Result<JsonValue, &'static str> {
    if is_hole(value) {
        return Ok(object(vec![("hole", bool(true))]));
    }
    if descriptor_kind(value, "kind") != Some("object")
        || !allowed_keys(value, &["pathId", "quantifier", "steps"])
    {
        return Err("invalid_path_shape");
    }
    let path_id = get(value, "path_id").ok_or("invalid_path_id")?;
    let path_id = descriptor_text(path_id, "value")
        .filter(|_| descriptor_kind(path_id, "kind") == Some("string"))
        .ok_or("invalid_path_id")?;
    if !allowed_path_id(path_id) || ids.iter().any(|seen| seen.as_slice() == path_id.units()) {
        return Err("invalid_path_id");
    }
    ids.push(path_id.units().to_vec());
    let quantifier = get(value, "quantifier").ok_or("invalid_path_quantifier")?;
    let quantifier = match descriptor_kind(quantifier, "kind") {
        Some("undefined") => "exists",
        Some("string") => {
            match descriptor_text(quantifier, "value").and_then(|value| value.as_str()) {
                Some("exists") => "exists",
                Some("not_exists") => "not_exists",
                _ => return Err("invalid_path_quantifier"),
            }
        }
        _ => return Err("invalid_path_quantifier"),
    };
    let steps = get(value, "steps").ok_or("invalid_path_steps")?;
    if descriptor_kind(steps, "kind") != Some("array") {
        return Err("invalid_path_steps");
    }
    let length = get(steps, "length")
        .and_then(JsonValue::as_u64)
        .filter(|length| (1..=4).contains(length))
        .ok_or("invalid_path_steps")?;
    let items = get(steps, "items")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() as u64 == length)
        .ok_or("invalid_path_steps")?;
    let normalized_steps = items
        .iter()
        .map(normalize_step)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(object(vec![
        ("pathId", JsonValue::String(path_id.clone())),
        ("quantifier", text(quantifier)),
        ("steps", JsonValue::Array(normalized_steps)),
    ]))
}

/// Normalize bounded saved path drafts. Source values remain in the browser;
/// only safe IDs, shape metadata, and condition descriptors cross this rule.
pub fn normalize_observatory_paths_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_path_list")?;
    let request = document.root();
    if descriptor_kind(request, "kind") == Some("undefined") {
        return emit_value_preserved_json(&JsonValue::Array(Vec::new()), JsonLimits::default())
            .map_err(|_| "invalid_path_output");
    }
    if descriptor_kind(request, "kind") != Some("array") {
        return Err("invalid_path_list");
    }
    let length = get(request, "length")
        .and_then(JsonValue::as_u64)
        .filter(|length| *length <= 4)
        .ok_or("invalid_path_list")?;
    let items = get(request, "items")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() as u64 == length)
        .ok_or("invalid_path_list")?;
    let mut ids = Vec::new();
    let normalized = items
        .iter()
        .map(|path| normalize_path(path, &mut ids))
        .collect::<Result<Vec<_>, _>>()?;
    emit_value_preserved_json(
        &JsonValue::Array(normalized),
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_path_output")
}

fn import_string(value: &JsonValue) -> Option<&JsonString> {
    (descriptor_kind(value, "kind") == Some("string"))
        .then(|| descriptor_text(value, "value"))
        .flatten()
}
fn import_scalar(value: &JsonValue) -> bool {
    match descriptor_kind(value, "kind") {
        Some("null" | "boolean") => true,
        Some("string") => get(value, "units")
            .and_then(JsonValue::as_u64)
            .is_some_and(|units| units <= 1024),
        Some("number") => matches!(get(value, "finite"), Some(JsonValue::Bool(true))),
        _ => false,
    }
}
fn import_value_shape(value: &JsonValue) -> bool {
    if descriptor_kind(value, "kind") != Some("array") {
        return import_scalar(value);
    }
    let Some(length) = get(value, "length").and_then(JsonValue::as_u64) else {
        return false;
    };
    length <= 100
        && get(value, "items")
            .and_then(JsonValue::as_array)
            .is_some_and(|items| {
                items.len() as u64 == length
                    && items.iter().all(|item| {
                        import_scalar(item) || descriptor_kind(item, "kind") == Some("hole")
                    })
            })
}
fn import_property_id(value: &JsonString) -> bool {
    let units = value.units();
    const PREFIX: &[u16] = &[
        116, 111, 115, 46, 112, 114, 111, 112, 101, 114, 116, 121, 46,
    ];
    units.len() > PREFIX.len()
        && units.starts_with(PREFIX)
        && units[PREFIX.len()..]
            .iter()
            .all(|unit| (97..=122).contains(unit) || (48..=57).contains(unit) || *unit == 45)
}
fn import_type_values(value: &JsonValue, shape: &JsonValue) -> Option<Vec<Option<Vec<u16>>>> {
    if descriptor_kind(value, "kind") != Some("array") {
        return None;
    }
    let length = get(value, "length")?.as_u64()?;
    if length == 0
        || length > 100
        || get(shape, "kind").and_then(JsonValue::as_str) != Some("array")
        || get(shape, "length").and_then(JsonValue::as_u64)? != length
    {
        return None;
    }
    let items = get(value, "items")?.as_array()?;
    let shape_items = get(shape, "items")?.as_array()?;
    if items.len() as u64 != length || shape_items.len() as u64 != length {
        return None;
    }
    let mut seen = Vec::<Vec<u16>>::new();
    let mut saw_hole = false;
    let mut result = Vec::with_capacity(items.len());
    for (item, shape) in items.iter().zip(shape_items) {
        if descriptor_kind(item, "kind") == Some("hole") {
            if descriptor_kind(shape, "kind") != Some("hole") {
                return None;
            }
            if saw_hole {
                return None;
            }
            saw_hole = true;
            result.push(None);
            continue;
        }
        if descriptor_kind(shape, "kind") != Some("string")
            || get(shape, "units").and_then(JsonValue::as_u64)?
                != import_string(item)?.units().len() as u64
        {
            return None;
        }
        let text = import_string(item)?;
        if text.units().is_empty()
            || text.units().len() > 1024
            || seen.iter().any(|known| known.as_slice() == text.units())
        {
            return None;
        }
        seen.push(text.units().to_vec());
        result.push(Some(text.units().to_vec()));
    }
    Some(result)
}
fn import_rule_request(rules: &[JsonValue], kind: &str) -> JsonValue {
    let items = JsonValue::Array(rules.to_vec());
    let empty = JsonValue::Array(Vec::new());
    let (nodes, relations, keys): (JsonValue, JsonValue, Vec<JsonValue>) = if kind == "nodes" {
        (
            object(vec![("length", number(rules.len())), ("items", items)]),
            object(vec![("length", number(0)), ("items", empty)]),
            vec![text("nodes")],
        )
    } else {
        (
            object(vec![("length", number(0)), ("items", empty)]),
            object(vec![("length", number(rules.len())), ("items", items)]),
            vec![text("relations")],
        )
    };
    object(vec![
        ("shape", text("object")),
        ("keys", JsonValue::Array(keys)),
        ("max_conditions", number(32)),
        ("nodes", nodes),
        ("relations", relations),
    ])
}
fn normalize_import_query(value: &JsonValue, kind: &str) -> Result<JsonValue, &'static str> {
    if descriptor_kind(value, "kind") == Some("undefined") {
        return Ok(object(vec![
            ("enabled", bool(true)),
            ("match", text("all")),
            ("conditions", JsonValue::Array(Vec::new())),
            (
                if kind == "nodes" {
                    "kinds_index"
                } else {
                    "predicates_index"
                },
                JsonValue::Null,
            ),
        ]));
    }
    if descriptor_kind(value, "kind") != Some("object")
        || !allowed_keys(value, &["enabled", "match", "filters"])
    {
        return Err("invalid_path_spec_query_shape");
    }
    let enabled = get(value, "enabled").ok_or("invalid_path_spec_query_values")?;
    let enabled = match descriptor_kind(enabled, "kind") {
        Some("undefined") => true,
        Some("boolean") => match get(enabled, "value") {
            Some(JsonValue::Bool(value)) => *value,
            _ => return Err("invalid_path_spec_query_values"),
        },
        _ => return Err("invalid_path_spec_query_values"),
    };
    let matched = get(value, "match").ok_or("invalid_path_spec_query_values")?;
    let matched = match import_string(matched).and_then(JsonString::as_str) {
        None if descriptor_kind(matched, "kind") == Some("undefined") => "all",
        Some("all") => "all",
        Some("any") => "any",
        _ => return Err("invalid_path_spec_query_values"),
    };
    let filters_kind = get(value, "filters_kind")
        .and_then(JsonValue::as_str)
        .ok_or("invalid_path_spec_filter_limit")?;
    let filters = match filters_kind {
        "undefined" => &[][..],
        "array" => {
            let length = get(value, "filters_length")
                .and_then(JsonValue::as_u64)
                .filter(|length| *length <= 32)
                .ok_or("invalid_path_spec_filter_limit")?;
            get(value, "filters")
                .and_then(JsonValue::as_array)
                .filter(|items| items.len() as u64 == length)
                .ok_or("invalid_path_spec_filter_limit")?
        }
        _ => return Err("invalid_path_spec_filter_limit"),
    };
    let type_field = if kind == "nodes" {
        "kind_id"
    } else {
        "predicate_id"
    };
    let mut conditions = Vec::<JsonValue>::new();
    let mut value_indexes = Vec::<usize>::new();
    let mut type_index = None;
    for (index, filter) in filters.iter().enumerate() {
        if is_hole(filter)
            || descriptor_kind(filter, "kind") != Some("object")
            || !allowed_keys(filter, &["field", "property_id", "op", "value"])
        {
            return Err("invalid_path_spec_filter");
        }
        let has_field = matches!(get(filter, "field_present"), Some(JsonValue::Bool(true)));
        let has_property = matches!(
            get(filter, "property_id_present"),
            Some(JsonValue::Bool(true))
        );
        if has_field == has_property
            || kind == "relations" && has_property
            || !matches!(get(filter, "value_present"), Some(JsonValue::Bool(true)))
        {
            return Err("invalid_path_spec_filter");
        }
        let identifier = if has_field {
            import_string(get(filter, "field").ok_or("invalid_path_spec_filter")?)
        } else {
            import_string(get(filter, "property_id").ok_or("invalid_path_spec_filter")?)
        }
        .ok_or("invalid_path_spec_filter")?;
        if has_property && !import_property_id(identifier) {
            return Err("invalid_path_spec_filter");
        }
        let operator = import_string(get(filter, "op").ok_or("invalid_path_spec_filter")?)
            .and_then(JsonString::as_str)
            .ok_or("invalid_path_spec_filter")?;
        if !matches!(
            operator,
            "eq" | "neq" | "in" | "contains" | "prefix" | "exists" | "gt" | "gte" | "lt" | "lte"
        ) {
            return Err("invalid_path_spec_filter");
        }
        let value_shape = get(filter, "value_shape").ok_or("invalid_path_spec_filter")?;
        if !import_value_shape(value_shape) {
            return Err("invalid_path_spec_filter");
        }
        let candidate = has_field
            && identifier.as_str() == Some(type_field)
            && operator == "in"
            && type_index.is_none();
        if candidate {
            if get(filter, "type_values")
                .and_then(|values| import_type_values(values, value_shape))
                .is_some()
            {
                type_index = Some(index);
                continue;
            }
        }
        conditions.push(object(vec![
            (
                "selector",
                text(if has_field { "field" } else { "property_id" }),
            ),
            ("id", JsonValue::String(identifier.clone())),
            ("op", text(operator)),
            ("value_shape", value_shape.clone()),
        ]));
        value_indexes.push(index);
    }
    let request = import_rule_request(&conditions, kind);
    let request_bytes = emit_value_preserved_json(&request, JsonLimits::default())
        .map_err(|_| "invalid_path_spec_filter")?;
    let normalized = normalize_observatory_conditions_v1(&request_bytes)?;
    let document = parse_json(
        &normalized,
        JsonMode::RequestLastWins,
        JsonLimits::default(),
    )
    .map_err(|_| "invalid_conditions_output")?;
    let rules = get(document.root(), kind)
        .and_then(JsonValue::as_array)
        .ok_or("invalid_conditions_output")?;
    let conditions = rules
        .iter()
        .zip(value_indexes)
        .map(|(rule, value_index)| {
            let mut fields = rule
                .as_object()
                .ok_or("invalid_conditions_output")?
                .to_vec();
            fields.push((JsonString::from_utf8("value_index"), number(value_index)));
            Ok(JsonValue::Object(fields))
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let mut query = vec![
        ("enabled", bool(enabled)),
        ("match", text(matched)),
        ("conditions", JsonValue::Array(conditions)),
    ];
    query.push((
        if kind == "nodes" {
            "kinds_index"
        } else {
            "predicates_index"
        },
        type_index.map_or(JsonValue::Null, number),
    ));
    Ok(JsonValue::Object(
        query
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    ))
}
fn normalize_import_step(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if is_hole(value) {
        return Ok(JsonValue::Null);
    }
    if descriptor_kind(value, "kind") != Some("object")
        || !allowed_keys(value, &["direction", "node_query", "relation_query"])
    {
        return Err("invalid_path_spec_step");
    }
    let direction = get(value, "direction").ok_or("invalid_path_spec_step")?;
    let direction = match descriptor_kind(direction, "kind") {
        Some("undefined") => "outgoing",
        Some("string") => match import_string(direction).and_then(JsonString::as_str) {
            Some("outgoing") => "outgoing",
            Some("incoming") => "incoming",
            Some("either") => "either",
            _ => return Err("invalid_path_spec_step"),
        },
        _ => return Err("invalid_path_spec_step"),
    };
    let node_query = normalize_import_query(
        get(value, "node_query").ok_or("invalid_path_spec_query_shape")?,
        "nodes",
    )?;
    let relation_query = normalize_import_query(
        get(value, "relation_query").ok_or("invalid_path_spec_query_shape")?,
        "relations",
    )?;
    Ok(object(vec![
        ("direction", text(direction)),
        ("nodeQuery", node_query),
        ("relationQuery", relation_query),
    ]))
}
fn normalize_import_path(
    value: &JsonValue,
    ids: &mut Vec<Vec<u16>>,
) -> Result<JsonValue, &'static str> {
    if is_hole(value) {
        return Ok(JsonValue::Null);
    }
    if descriptor_kind(value, "kind") != Some("object")
        || !allowed_keys(value, &["path_id", "quantifier", "steps"])
    {
        return Err("invalid_path_spec_shape");
    }
    let path_id = import_string(get(value, "path_id").ok_or("invalid_path_spec_shape")?)
        .filter(|text| allowed_path_id(text))
        .ok_or("invalid_path_spec_shape")?;
    ids.push(path_id.units().to_vec());
    let quantifier = get(value, "quantifier").ok_or("invalid_path_spec_shape")?;
    let quantifier = match descriptor_kind(quantifier, "kind") {
        Some("undefined") => "exists",
        Some("string") => match import_string(quantifier).and_then(JsonString::as_str) {
            Some("exists") => "exists",
            Some("not_exists") => "not_exists",
            _ => return Err("invalid_path_spec_shape"),
        },
        _ => return Err("invalid_path_spec_shape"),
    };
    let steps = get(value, "steps").ok_or("invalid_path_spec_shape")?;
    let length = get(steps, "length")
        .and_then(JsonValue::as_u64)
        .filter(|length| (1..=4).contains(length))
        .ok_or("invalid_path_spec_shape")?;
    let items = get(steps, "items")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() as u64 == length)
        .ok_or("invalid_path_spec_shape")?;
    let normalized = items
        .iter()
        .map(normalize_import_step)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(object(vec![
        ("pathId", JsonValue::String(path_id.clone())),
        ("quantifier", text(quantifier)),
        ("steps", JsonValue::Array(normalized)),
    ]))
}

/// Import bounded wire path drafts through the same Rust-owned filter and
/// path admission rules. Values remain with the browser and return as indexes.
pub fn draft_observatory_paths_from_spec_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_path_spec_list")?;
    let request = document.root();
    if descriptor_kind(request, "kind") == Some("undefined") {
        return emit_value_preserved_json(&JsonValue::Array(Vec::new()), JsonLimits::default())
            .map_err(|_| "invalid_path_output");
    }
    if descriptor_kind(request, "kind") != Some("array") {
        return Err("invalid_path_spec_list");
    }
    let length = get(request, "length")
        .and_then(JsonValue::as_u64)
        .filter(|length| *length <= 4)
        .ok_or("invalid_path_spec_list")?;
    let items = get(request, "items")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() as u64 == length)
        .ok_or("invalid_path_spec_list")?;
    let mut ids = Vec::new();
    let paths = items
        .iter()
        .map(|path| normalize_import_path(path, &mut ids))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, path_id) in ids.iter().enumerate() {
        if ids[..index].iter().any(|seen| seen == path_id) {
            return Err("invalid_path_id");
        }
    }
    emit_value_preserved_json(&JsonValue::Array(paths), JsonLimits::default())
        .map_err(|_| "invalid_path_output")
}

#[derive(Clone)]
struct PathLimits {
    conditions: usize,
    steps: usize,
    node_filters: usize,
    relation_filters: usize,
    quantifiers: Vec<JsonValue>,
    directions: Vec<JsonValue>,
    node_matches: Vec<JsonValue>,
    relation_matches: Vec<JsonValue>,
    node_filter_max: JsonValue,
    relation_filter_max: JsonValue,
}
fn js_positive_integer(value: &JsonValue) -> Option<f64> {
    let JsonValue::Number(number) = value else {
        return None;
    };
    let value = number.lexeme.parse::<f64>().ok()?;
    (value.is_finite() && value >= 1.0 && value.fract() == 0.0).then_some(value)
}
fn path_limit_count(values: &[f64]) -> usize {
    values.iter().copied().fold(f64::INFINITY, f64::min) as usize
}
fn keep_known(values: &[JsonValue], allowed: &[&str]) -> Vec<JsonValue> {
    values
        .iter()
        .filter(|value| value.as_str().is_some_and(|value| allowed.contains(&value)))
        .cloned()
        .collect()
}
fn path_limits(context: &JsonValue) -> Result<PathLimits, &'static str> {
    let schema = get(context, "schema").unwrap_or(&JsonValue::Null);
    let catalog = get(context, "catalog").unwrap_or(&JsonValue::Null);
    let properties = at(get(schema, "properties"), "path_query");
    let path = at(get(schema, "$defs"), "pathCondition");
    let step_items = at(
        at(
            at(get(path.unwrap_or(&JsonValue::Null), "properties"), "steps"),
            "items",
        ),
        "properties",
    );
    let node_query = at(get(schema, "$defs"), "nodeQuery");
    let relation_query = at(get(schema, "$defs"), "relationQuery");
    let path_max = at(properties, "maxItems").and_then(js_positive_integer);
    let step_max = at(
        at(get(path.unwrap_or(&JsonValue::Null), "properties"), "steps"),
        "maxItems",
    )
    .and_then(js_positive_integer);
    let node_filter_value = at(
        at(
            get(node_query.unwrap_or(&JsonValue::Null), "properties"),
            "filters",
        ),
        "maxItems",
    );
    let relation_filter_value = at(
        at(
            get(relation_query.unwrap_or(&JsonValue::Null), "properties"),
            "filters",
        ),
        "maxItems",
    );
    let node_filter_max = node_filter_value.and_then(js_positive_integer);
    let relation_filter_max = relation_filter_value.and_then(js_positive_integer);
    let quantifier_values = at(
        at(
            get(path.unwrap_or(&JsonValue::Null), "properties"),
            "quantifier",
        ),
        "enum",
    );
    let direction_values = at(at(step_items, "direction"), "enum");
    let node_match_values = at(
        at(
            get(node_query.unwrap_or(&JsonValue::Null), "properties"),
            "match",
        ),
        "enum",
    );
    let relation_match_values = at(
        at(
            get(relation_query.unwrap_or(&JsonValue::Null), "properties"),
            "match",
        ),
        "enum",
    );
    if path_max.is_none()
        || step_max.is_none()
        || node_filter_max.is_none()
        || relation_filter_max.is_none()
        || quantifier_values.and_then(JsonValue::as_array).is_none()
        || direction_values.and_then(JsonValue::as_array).is_none()
        || node_match_values.and_then(JsonValue::as_array).is_none()
        || relation_match_values
            .and_then(JsonValue::as_array)
            .is_none()
    {
        return Err("path_schema_limits");
    }
    let advertised_path = at(get(catalog, "capabilities"), "path_query");
    let (advertised_conditions, advertised_steps, advertised_quantifiers) =
        if let Some(advertised) = advertised_path {
            if advertised.as_object().is_none() {
                return Err("path_catalog_limits");
            }
            let conditions = get(advertised, "conditions").and_then(js_positive_integer);
            let steps = get(advertised, "steps_per_condition").and_then(js_positive_integer);
            let quantifiers = get(advertised, "quantifiers").and_then(JsonValue::as_array);
            if conditions.is_none()
                || steps.is_none()
                || quantifiers.is_none()
                || quantifiers.is_some_and(|items| {
                    !items
                        .iter()
                        .all(|item| matches!(item, JsonValue::String(_)))
                })
                || get(advertised, "combination").and_then(JsonValue::as_str) != Some("all")
                || get(advertised, "scope").and_then(JsonValue::as_str)
                    != Some("node-selector-roots-and-selected-sources")
            {
                return Err("path_catalog_limits");
            }
            (conditions, steps, quantifiers)
        } else {
            (None, None, None)
        };
    let mut condition_bounds = vec![4.0, path_max.unwrap()];
    let mut step_bounds = vec![4.0, step_max.unwrap()];
    if let Some(value) = advertised_conditions {
        condition_bounds.push(value);
    }
    if let Some(value) = advertised_steps {
        step_bounds.push(value);
    }
    let conditions = path_limit_count(&condition_bounds);
    let steps = path_limit_count(&step_bounds);
    let quantifier_allowance = advertised_quantifiers.unwrap_or(&[]);
    let quantifiers = keep_known(values(quantifier_values), &["exists", "not_exists"])
        .into_iter()
        .filter(|value| {
            advertised_path.is_none() || quantifier_allowance.iter().any(|allowed| allowed == value)
        })
        .collect::<Vec<_>>();
    let directions = keep_known(
        values(direction_values),
        &["outgoing", "incoming", "either"],
    );
    let node_matches = keep_known(values(node_match_values), &["all", "any"]);
    let relation_matches = keep_known(values(relation_match_values), &["all", "any"]);
    if conditions < 1
        || steps < 1
        || quantifiers.is_empty()
        || directions.is_empty()
        || node_matches.is_empty()
        || relation_matches.is_empty()
    {
        return Err("path_schema_compat");
    }
    Ok(PathLimits {
        conditions,
        steps,
        node_filters: node_filter_max.unwrap().min(usize::MAX as f64) as usize,
        relation_filters: relation_filter_max.unwrap().min(usize::MAX as f64) as usize,
        quantifiers,
        directions,
        node_matches,
        relation_matches,
        node_filter_max: node_filter_value.unwrap().clone(),
        relation_filter_max: relation_filter_value.unwrap().clone(),
    })
}
fn path_limits_json(limits: &PathLimits) -> JsonValue {
    object(vec![
        ("conditions", number(limits.conditions)),
        ("steps", number(limits.steps)),
        ("nodeFilters", limits.node_filter_max.clone()),
        ("relationFilters", limits.relation_filter_max.clone()),
        ("quantifiers", JsonValue::Array(limits.quantifiers.clone())),
        ("directions", JsonValue::Array(limits.directions.clone())),
        ("nodeMatches", JsonValue::Array(limits.node_matches.clone())),
        (
            "relationMatches",
            JsonValue::Array(limits.relation_matches.clone()),
        ),
    ])
}

/// Resolve path bounds and supported choices against the exact delivered
/// schema/catalog projection used by the browser editor.
pub fn observatory_path_limits_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_path_context")?;
    let limits = path_limits(get(document.root(), "context").unwrap_or(&JsonValue::Null))?;
    emit_value_preserved_json(&path_limits_json(&limits), JsonLimits::default())
        .map_err(|_| "invalid_path_limits_output")
}

fn type_values(value: &JsonValue) -> Result<Vec<Option<JsonString>>, &'static str> {
    valid_type_list(value)?;
    Ok(values(get(value, "items"))
        .iter()
        .map(|item| {
            if is_hole(item) {
                None
            } else {
                descriptor_text(item, "value").cloned()
            }
        })
        .collect())
}
fn catalog_ids(catalog: &JsonValue, key: &str) -> Vec<JsonString> {
    values(get(catalog, key))
        .iter()
        .filter_map(|item| {
            let field = if key == "node_kinds" {
                "kind_id"
            } else {
                "predicate_id"
            };
            get(item, field).and_then(|value| match value {
                JsonValue::String(id) => Some(id.clone()),
                _ => None,
            })
        })
        .collect()
}
fn supports_in(context: &JsonValue, kind: &str) -> bool {
    let wire_kind = if kind == "nodes" { "node" } else { "relation" };
    let filter_name = format!("{wire_kind}Filter");
    let filter = at(at(get(context, "schema"), "$defs"), &filter_name);
    at(at(at(filter, "properties"), "op"), "enum").is_some_and(|values| includes(values, "in"))
        && at(get(context, "catalog"), "capabilities")
            .and_then(|caps| get(caps, "filter_operators"))
            .is_some_and(|values| includes(values, "in"))
}
fn includes(value: &JsonValue, needle: &str) -> bool {
    values(Some(value))
        .iter()
        .any(|item| item.as_str() == Some(needle))
}
fn query_rules<'a>(query: Option<&'a JsonValue>, kind: &str) -> Result<JsonValue, &'static str> {
    if query.is_some_and(|query| descriptor_kind(query, "kind") == Some("undefined"))
        || query.is_none()
    {
        return Ok(object(vec![
            ("length", number(0)),
            ("items", JsonValue::Array(Vec::new())),
        ]));
    }
    let request = get(query.unwrap(), "condition_request").ok_or("invalid_path_conditions")?;
    let rules = get(request, kind).ok_or("invalid_path_conditions")?;
    Ok(rules.clone())
}
fn compile_path_query(
    context: &JsonValue,
    normalized_path: &JsonValue,
    source_path: &JsonValue,
    limits: &PathLimits,
    kinds_catalog: &[JsonString],
    predicates_catalog: &[JsonString],
) -> Result<JsonValue, &'static str> {
    let normalized_steps = get(normalized_path, "steps")
        .and_then(JsonValue::as_array)
        .ok_or("invalid_path_steps")?;
    let source_steps_descriptor = get(source_path, "steps").ok_or("invalid_path_steps")?;
    let source_steps = get(source_steps_descriptor, "items")
        .and_then(JsonValue::as_array)
        .ok_or("invalid_path_steps")?;
    let quantifier = get(normalized_path, "quantifier").ok_or("invalid_path_quantifier")?;
    if !limits
        .quantifiers
        .iter()
        .any(|allowed| allowed == quantifier)
    {
        return Err("path_server_quantifier");
    }
    if normalized_steps.len() > limits.steps {
        return Err("path_server_step_count");
    }
    let mut steps = Vec::with_capacity(normalized_steps.len());
    for (index, step) in normalized_steps.iter().enumerate() {
        if matches!(step, JsonValue::Null) || is_hole(step) {
            return Err("invalid_path_step");
        }
        let source = source_steps.get(index).ok_or("invalid_path_step")?;
        if is_hole(source) {
            steps.push(JsonValue::Null);
            continue;
        }
        let direction = get(step, "direction").ok_or("invalid_path_direction")?;
        if !limits.directions.iter().any(|allowed| allowed == direction) {
            return Err("path_server_direction");
        }
        let node_query = get(step, "nodeQuery").ok_or("invalid_path_query_shape")?;
        let relation_query = get(step, "relationQuery").ok_or("invalid_path_query_shape")?;
        if !limits
            .node_matches
            .iter()
            .any(|allowed| allowed == get(node_query, "match").unwrap_or(&JsonValue::Null))
            || !limits
                .relation_matches
                .iter()
                .any(|allowed| allowed == get(relation_query, "match").unwrap_or(&JsonValue::Null))
        {
            return Err("path_server_match");
        }
        let kinds = type_values(get(source, "kinds").ok_or("invalid_path_types")?)?;
        let predicates = type_values(get(source, "predicates").ok_or("invalid_path_types")?)?;
        if (!kinds.is_empty() && !supports_in(context, "nodes"))
            || (!predicates.is_empty() && !supports_in(context, "relations"))
        {
            return Err("path_server_type_operation");
        }
        let mut node_filters = Vec::new();
        for id in &kinds {
            let Some(id) = id.as_ref() else {
                return Err("path_type_catalog_changed");
            };
            if !kinds_catalog.iter().any(|known| known == id) {
                return Err("path_type_catalog_changed");
            }
        }
        if !kinds.is_empty() {
            node_filters.push(object(vec![
                ("field", text("kind_id")),
                ("op", text("in")),
                (
                    "value",
                    JsonValue::Array(
                        kinds
                            .iter()
                            .filter_map(Clone::clone)
                            .map(JsonValue::String)
                            .collect(),
                    ),
                ),
            ]));
        }
        let mut relation_filters = Vec::new();
        for id in &predicates {
            let Some(id) = id.as_ref() else {
                return Err("path_type_catalog_changed");
            };
            if !predicates_catalog.iter().any(|known| known == id) {
                return Err("path_type_catalog_changed");
            }
        }
        if !predicates.is_empty() {
            relation_filters.push(object(vec![
                ("field", text("predicate_id")),
                ("op", text("in")),
                (
                    "value",
                    JsonValue::Array(
                        predicates
                            .iter()
                            .filter_map(Clone::clone)
                            .map(JsonValue::String)
                            .collect(),
                    ),
                ),
            ]));
        }
        let source_node = get(source, "node_query");
        let source_relation = get(source, "relation_query");
        let node_rules = query_rules(source_node, "nodes")?;
        let relation_rules = query_rules(source_relation, "relations")?;
        let compiled_nodes = compile_condition_rules(
            context,
            "nodes",
            &node_rules,
            Some(limits.node_filters),
            !kinds.is_empty(),
        )
        .map_err(|error| {
            if error == "condition_limit" {
                "condition_limit"
            } else {
                error
            }
        })?;
        let compiled_relations = compile_condition_rules(
            context,
            "relations",
            &relation_rules,
            Some(limits.relation_filters),
            !predicates.is_empty(),
        )
        .map_err(|error| {
            if error == "condition_limit" {
                "condition_limit"
            } else {
                error
            }
        })?;
        node_filters.extend(compiled_nodes);
        relation_filters.extend(compiled_relations);
        if node_filters.len() > limits.node_filters
            || relation_filters.len() > limits.relation_filters
        {
            return Err("path_filter_limit");
        }
        let node_wire = object(vec![
            (
                "enabled",
                get(node_query, "enabled").cloned().unwrap_or(bool(true)),
            ),
            (
                "match",
                get(node_query, "match")
                    .cloned()
                    .unwrap_or_else(|| text("all")),
            ),
            ("filters", JsonValue::Array(node_filters)),
        ]);
        let relation_wire = object(vec![
            (
                "enabled",
                get(relation_query, "enabled")
                    .cloned()
                    .unwrap_or(bool(true)),
            ),
            (
                "match",
                get(relation_query, "match")
                    .cloned()
                    .unwrap_or_else(|| text("all")),
            ),
            ("filters", JsonValue::Array(relation_filters)),
        ]);
        steps.push(object(vec![
            ("direction", direction.clone()),
            ("node_query", node_wire),
            ("relation_query", relation_wire),
        ]));
    }
    Ok(object(vec![
        (
            "path_id",
            get(normalized_path, "pathId")
                .cloned()
                .ok_or("invalid_path_id")?,
        ),
        ("quantifier", quantifier.clone()),
        ("steps", JsonValue::Array(steps)),
    ]))
}

/// Compile path drafts to the exact query wire shape after applying the live
/// schema/catalog intersection. Condition values are represented by indexes.
pub fn compile_observatory_paths_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_path_compile_request")?;
    let request = document.root();
    let paths_descriptor = get(request, "paths").ok_or("invalid_path_list")?;
    let path_bytes = emit_value_preserved_json(paths_descriptor, JsonLimits::default())
        .map_err(|_| "invalid_path_list")?;
    let normalized_bytes = normalize_observatory_paths_v1(&path_bytes)?;
    let normalized_document = parse_json(
        &normalized_bytes,
        JsonMode::RequestLastWins,
        JsonLimits::default(),
    )
    .map_err(|_| "invalid_path_list")?;
    let normalized_paths = normalized_document
        .root()
        .as_array()
        .ok_or("invalid_path_list")?;
    if normalized_paths.is_empty() {
        return emit_value_preserved_json(&JsonValue::Array(Vec::new()), JsonLimits::default())
            .map_err(|_| "invalid_path_compile_output");
    }
    let source_paths = get(paths_descriptor, "items")
        .and_then(JsonValue::as_array)
        .ok_or("invalid_path_list")?;
    let context = get(request, "context").unwrap_or(&JsonValue::Null);
    let limits = path_limits(context)?;
    if normalized_paths.len() > limits.conditions {
        return Err("path_server_condition_count");
    }
    let catalog = get(context, "catalog").unwrap_or(&JsonValue::Null);
    let kinds_catalog = catalog_ids(catalog, "node_kinds");
    let predicates_catalog = catalog_ids(catalog, "predicates");
    // Match the former browser gate: every server-owned path constraint is
    // checked before any condition is compiled, so a later stale direction or
    // catalog ID is not masked by an earlier unsupported filter.
    for (index, path) in normalized_paths.iter().enumerate() {
        if matches!(path, JsonValue::Null) || is_hole(path) {
            return Err("invalid_path_shape");
        }
        let source = source_paths.get(index).ok_or("invalid_path_shape")?;
        if is_hole(source) {
            return Err("invalid_path_shape");
        }
        let quantifier = get(path, "quantifier").ok_or("invalid_path_quantifier")?;
        if !limits
            .quantifiers
            .iter()
            .any(|allowed| allowed == quantifier)
        {
            return Err("path_server_quantifier");
        }
        let steps = get(path, "steps")
            .and_then(JsonValue::as_array)
            .ok_or("invalid_path_steps")?;
        if steps.len() > limits.steps {
            return Err("path_server_step_count");
        }
        let source_steps = get(source, "steps")
            .and_then(|steps| get(steps, "items"))
            .and_then(JsonValue::as_array)
            .ok_or("invalid_path_steps")?;
        for (step_index, step) in steps.iter().enumerate() {
            if matches!(step, JsonValue::Null) || is_hole(step) {
                return Err("invalid_path_step");
            }
            let source_step = source_steps.get(step_index).ok_or("invalid_path_step")?;
            if is_hole(source_step) {
                return Err("invalid_path_step");
            }
            let direction = get(step, "direction").ok_or("invalid_path_direction")?;
            if !limits.directions.iter().any(|allowed| allowed == direction) {
                return Err("path_server_direction");
            }
            let node_query = get(step, "nodeQuery").ok_or("invalid_path_query_shape")?;
            let relation_query = get(step, "relationQuery").ok_or("invalid_path_query_shape")?;
            if !limits
                .node_matches
                .iter()
                .any(|allowed| allowed == get(node_query, "match").unwrap_or(&JsonValue::Null))
                || !limits.relation_matches.iter().any(|allowed| {
                    allowed == get(relation_query, "match").unwrap_or(&JsonValue::Null)
                })
            {
                return Err("path_server_match");
            }
            let kinds = type_values(get(source_step, "kinds").ok_or("invalid_path_types")?)?;
            let predicates =
                type_values(get(source_step, "predicates").ok_or("invalid_path_types")?)?;
            if (!kinds.is_empty() && !supports_in(context, "nodes"))
                || (!predicates.is_empty() && !supports_in(context, "relations"))
            {
                return Err("path_server_type_operation");
            }
            for id in &kinds {
                let Some(id) = id.as_ref() else {
                    return Err("path_type_catalog_changed");
                };
                if !kinds_catalog.iter().any(|known| known == id) {
                    return Err("path_type_catalog_changed");
                }
            }
            for id in &predicates {
                let Some(id) = id.as_ref() else {
                    return Err("path_type_catalog_changed");
                };
                if !predicates_catalog.iter().any(|known| known == id) {
                    return Err("path_type_catalog_changed");
                }
            }
        }
    }
    let mut compiled = Vec::with_capacity(normalized_paths.len());
    for (index, path) in normalized_paths.iter().enumerate() {
        let source = source_paths.get(index).ok_or("invalid_path_shape")?;
        compiled.push(compile_path_query(
            context,
            path,
            source,
            &limits,
            &kinds_catalog,
            &predicates_catalog,
        )?);
    }
    emit_value_preserved_json(
        &JsonValue::Array(compiled),
        JsonLimits {
            max_bytes: 24_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_path_compile_output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_ordered_nested_conditions_are_normalized() {
        let raw = br#"{"kind":"array","length":1,"items":[{"kind":"object","keys":["pathId","steps"],"path_id":{"kind":"string","value":"p"},"quantifier":{"kind":"undefined"},"steps":{"kind":"array","length":1,"items":[{"kind":"object","keys":[],"direction":{"kind":"undefined"},"kinds":{"kind":"array","length":0,"items":[]},"predicates":{"kind":"array","length":0,"items":[]},"node_query":{"kind":"undefined"},"relation_query":{"kind":"undefined"}}]}}]}"#;
        assert_eq!(
            normalize_observatory_paths_v1(raw).unwrap(),
            br#"[{"pathId":"p","quantifier":"exists","steps":[{"direction":"outgoing","nodeQuery":{"enabled":true,"match":"all","conditions":[]},"relationQuery":{"enabled":true,"match":"all","conditions":[]}}]}]"#
        );
    }

    #[test]
    fn path_ids_and_step_bounds_fail_with_their_owner_codes() {
        let duplicate = br#"{"kind":"array","length":2,"items":[{"kind":"object","keys":["pathId","steps"],"path_id":{"kind":"string","value":"p"},"quantifier":{"kind":"undefined"},"steps":{"kind":"array","length":1,"items":[{"hole":true}] }},{"kind":"object","keys":["pathId","steps"],"path_id":{"kind":"string","value":"p"},"quantifier":{"kind":"undefined"},"steps":{"kind":"array","length":1,"items":[{"hole":true}] }}]}"#;
        assert_eq!(
            normalize_observatory_paths_v1(duplicate),
            Err("invalid_path_id")
        );
        let empty = br#"{"kind":"array","length":1,"items":[{"kind":"object","keys":["pathId","steps"],"path_id":{"kind":"string","value":"p"},"quantifier":{"kind":"undefined"},"steps":{"kind":"array","length":0,"items":[]}}]}"#;
        assert_eq!(
            normalize_observatory_paths_v1(empty),
            Err("invalid_path_steps")
        );
    }
}
