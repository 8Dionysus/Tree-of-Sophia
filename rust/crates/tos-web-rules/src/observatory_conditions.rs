//! Local Observatory filter definitions. Catalog/schema applicability is
//! checked later by the query compiler; this rule only owns saved shape.

use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonString, JsonValue, emit_value_preserved_json, parse_json,
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
    match value {
        JsonValue::Null | JsonValue::Bool(_) => true,
        JsonValue::String(text) => text.units().len() <= 1024,
        JsonValue::Number(JsonNumber { lexeme, .. }) => {
            lexeme.parse::<f64>().is_ok_and(f64::is_finite)
        }
        _ => false,
    }
}
fn valid_value(value: &JsonValue) -> bool {
    match value {
        JsonValue::Array(items) => items.len() <= 100 && items.iter().all(scalar),
        other => scalar(other),
    }
}
fn rule(value: &JsonValue, relation: bool) -> Result<JsonValue, &'static str> {
    let selector = get(value, "selector").and_then(JsonValue::as_str);
    let identifier = get(value, "id");
    let op = get(value, "op").and_then(JsonValue::as_str);
    let body = get(value, "value");
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
        ("value", body.unwrap().clone()),
    ]))
}

/// Normalize one bounded set of node and relation conditions. The host
/// transports only owned rule fields and the original enumerable root keys.
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
        .and_then(JsonValue::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("invalid_conditions_limit")?;
    let mut normalized = Vec::with_capacity(2);
    for (kind, relation) in [("nodes", false), ("relations", true)] {
        let entries = get(request, kind)
            .and_then(JsonValue::as_array)
            .ok_or("invalid_conditions_limit")?;
        if entries.len() > max {
            return Err("invalid_conditions_limit");
        }
        let rules = entries
            .iter()
            .map(|item| rule(item, relation))
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
    fn preserves_order_duplicates_and_wtf16_scalar() {
        let raw = br#"{"shape":"object","keys":["nodes","relations"],"max_conditions":12,"nodes":[{"selector":"field","id":"\ud800","op":"eq","value":[0,false,"\ud800"],"ignored":"private"},{"selector":"field","id":"\ud800","op":"eq","value":null}],"relations":[]}"#;
        let bytes = normalize_observatory_conditions_v1(raw).unwrap();
        let parsed = parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        let nodes = get(parsed.root(), "nodes").unwrap().as_array().unwrap();
        assert_eq!(nodes.len(), 2);
        assert!(get(&nodes[0], "ignored").is_none());
        assert_eq!(get(&nodes[0], "id"), get(&nodes[1], "id"));
    }
    #[test]
    fn relation_scope_and_limit_fail_closed() {
        let raw = br#"{"shape":"object","keys":["nodes","relations"],"max_conditions":12,"nodes":[],"relations":[{"selector":"property_id","id":"p","op":"eq","value":true}]}"#;
        assert_eq!(
            normalize_observatory_conditions_v1(raw),
            Err("invalid_conditions_rule")
        );
        let limit = br#"{"shape":"object","keys":["nodes","relations"],"max_conditions":0,"nodes":[{"selector":"field","id":"p","op":"eq","value":true}],"relations":[]}"#;
        assert_eq!(
            normalize_observatory_conditions_v1(limit),
            Err("invalid_conditions_limit")
        );
    }
}
