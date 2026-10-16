//! Saved Observatory view state. Opaque IDs remain UTF-16 strings; a pose is
//! local presentation state and never a source or graph authority carrier.

use std::collections::HashSet;
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
fn bounded(value: Option<&JsonValue>, low: f64, high: f64) -> bool {
    matches!(value, Some(JsonValue::Number(JsonNumber { lexeme, .. })) if lexeme.parse::<f64>().is_ok_and(|n| n.is_finite() && n >= low && n <= high))
}
fn id(value: Option<&JsonValue>, nullable: bool) -> bool {
    match value {
        Some(JsonValue::Null) => nullable,
        Some(JsonValue::String(value)) => (1..=1024).contains(&value.units().len()),
        _ => false,
    }
}
fn vector(value: Option<&JsonValue>) -> bool {
    value.and_then(JsonValue::as_array).is_some_and(|items| {
        items.len() == 3 && items.iter().all(|n| bounded(Some(n), -10000.0, 10000.0))
    })
}
fn normalize_vertex(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !id(get(value, "id"), false)
        || !bounded(get(value, "slot"), 0.0, 1000.0)
        || !get(value, "slot").is_some_and(|slot| {
            matches!(slot, JsonValue::Number(number) if number.lexeme.parse::<f64>().is_ok_and(|n| n.fract() == 0.0))
        })
        || !vector(get(value, "p"))
        || !vector(get(value, "sourcePosition"))
        || !vector(get(value, "target"))
        || !bounded(get(value, "volumeZ"), -10000.0, 10000.0)
    {
        return Err("invalid_pose_vertices");
    }
    let target = get(value, "target").unwrap().clone();
    Ok(object(vec![
        ("id", get(value, "id").unwrap().clone()),
        ("slot", get(value, "slot").unwrap().clone()),
        ("p", get(value, "p").unwrap().clone()),
        (
            "sourcePosition",
            get(value, "sourcePosition").unwrap().clone(),
        ),
        ("target", target.clone()),
        ("pos", target),
        ("volumeZ", get(value, "volumeZ").unwrap().clone()),
    ]))
}

/// Validate and project the exact durable view fields, including the derived
/// `pos` alias used by the existing reader. Unknown scene data is discarded.
pub fn normalize_observatory_pose_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 16_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_pose")?;
    let value = document.root();
    let lens = get(value, "lens").and_then(JsonValue::as_str);
    let tab = get(value, "cardTab").and_then(JsonValue::as_str);
    if !matches!(lens, Some("constellations" | "plane" | "orbits"))
        || !bounded(get(value, "yaw"), -100000.0, 100000.0)
        || !bounded(get(value, "pitch"), -2.0, 2.0)
        || !bounded(get(value, "zoom"), 0.1, 5.0)
        || !bounded(
            get(value, "pan").and_then(|pan| get(pan, "x")),
            -100000.0,
            100000.0,
        )
        || !bounded(
            get(value, "pan").and_then(|pan| get(pan, "y")),
            -100000.0,
            100000.0,
        )
        || !id(get(value, "selectedId"), true)
        || !id(get(value, "relationId"), true)
        || get(value, "panelOpen")
            .and_then(JsonValue::as_bool)
            .is_none()
        || !matches!(tab, Some("about" | "relations"))
    {
        return Err("invalid_pose");
    }
    let vertices = get(value, "vertices")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() <= 40)
        .ok_or("invalid_pose")?;
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let JsonValue::String(identifier) = get(vertex, "id").ok_or("invalid_pose_vertices")?
        else {
            return Err("invalid_pose_vertices");
        };
        if !seen.insert(identifier.units().to_vec()) {
            return Err("invalid_pose_vertices");
        }
        normalized.push(normalize_vertex(vertex)?);
    }
    let pan = get(value, "pan").unwrap();
    let result = object(vec![
        ("lens", get(value, "lens").unwrap().clone()),
        ("yaw", get(value, "yaw").unwrap().clone()),
        ("pitch", get(value, "pitch").unwrap().clone()),
        ("zoom", get(value, "zoom").unwrap().clone()),
        (
            "pan",
            object(vec![
                ("x", get(pan, "x").unwrap().clone()),
                ("y", get(pan, "y").unwrap().clone()),
            ]),
        ),
        ("selectedId", get(value, "selectedId").unwrap().clone()),
        ("relationId", get(value, "relationId").unwrap().clone()),
        ("panelOpen", get(value, "panelOpen").unwrap().clone()),
        ("cardTab", get(value, "cardTab").unwrap().clone()),
        ("vertices", JsonValue::Array(normalized)),
    ]);
    emit_value_preserved_json(
        &result,
        JsonLimits {
            max_bytes: 16_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_pose_output")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_vertex_alias_without_scene_extras() {
        let raw = br#"{"lens":"orbits","yaw":0,"pitch":0,"zoom":1,"pan":{"x":0,"y":0},"selectedId":null,"relationId":null,"panelOpen":true,"cardTab":"about","vertices":[{"id":"opaque:a","slot":0,"p":[0,1,2],"sourcePosition":[0,1,2],"target":[0,1,2],"volumeZ":2,"sceneText":"private"}],"raw":"unknown"}"#;
        let bytes = normalize_observatory_pose_v1(raw).unwrap();
        let parsed = parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        let vertex = &get(parsed.root(), "vertices").unwrap().as_array().unwrap()[0];
        assert_eq!(get(vertex, "target"), get(vertex, "pos"));
        assert!(get(vertex, "sceneText").is_none());
        assert!(get(parsed.root(), "raw").is_none());
    }
    #[test]
    fn duplicate_vertices_fail_even_with_opaque_surrogate_identity() {
        let raw = br#"{"lens":"plane","yaw":0,"pitch":0,"zoom":1,"pan":{"x":0,"y":0},"selectedId":null,"relationId":null,"panelOpen":true,"cardTab":"about","vertices":[{"id":"\ud800","slot":0,"p":[0,0,0],"sourcePosition":[0,0,0],"target":[0,0,0],"volumeZ":0},{"id":"\ud800","slot":1,"p":[0,0,0],"sourcePosition":[0,0,0],"target":[0,0,0],"volumeZ":0}]}"#;
        assert_eq!(
            normalize_observatory_pose_v1(raw),
            Err("invalid_pose_vertices")
        );
    }
}
