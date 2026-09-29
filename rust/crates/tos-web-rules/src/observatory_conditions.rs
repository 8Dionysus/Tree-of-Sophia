//! Local Observatory filter definitions. Catalog/schema applicability is
//! checked later by the query compiler; this rule only owns saved shape.

use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
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
            max_visits: 2_000_000,
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
