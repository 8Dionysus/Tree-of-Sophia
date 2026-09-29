//! Personal Observatory interface preferences. This is local user state;
//! it cannot change source, review, rights or canon authority.

use std::collections::HashSet;
use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};

const TOOLS: &[&str] = &[
    "search",
    "lenses",
    "workspace",
    "navigation",
    "builder",
    "evidence",
    "sources",
    "reader",
];
const PANELS: &[&str] = &[
    "inspector",
    "workspace",
    "evidence",
    "navigation",
    "builder",
    "studio",
    "reader",
    "history",
    "copy",
    "settings",
    "search",
    "lenses",
];

fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn choice<'a>(value: &'a JsonValue, key: &str, allowed: &[&str]) -> Option<&'a str> {
    get(value, key)?
        .as_str()
        .filter(|item| allowed.contains(item))
}
fn default_choice<'a>(
    value: &'a JsonValue,
    key: &str,
    fallback: &'a str,
    allowed: &[&str],
) -> Option<&'a str> {
    match get(value, key) {
        None | Some(JsonValue::Null) => Some(fallback),
        Some(item) => item.as_str().filter(|text| allowed.contains(text)),
    }
}
fn bounded(value: Option<&JsonValue>, low: f64, high: f64) -> bool {
    matches!(value, Some(JsonValue::Number(number)) if number.lexeme.parse::<f64>().is_ok_and(|n| n.is_finite() && n >= low && n <= high))
}
fn truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null | JsonValue::Bool(false) => false,
        JsonValue::String(text) => text.as_str() != Some(""),
        JsonValue::Number(number) => number.lexeme.parse::<f64>().is_ok_and(|n| n != 0.0),
        _ => true,
    }
}
fn panel_values(
    value: &JsonValue,
    key: &str,
    first: &str,
    second: &str,
    first_min: f64,
    first_max: f64,
    second_min: f64,
    second_max: f64,
) -> Result<JsonValue, &'static str> {
    let Some(record) = get(value, key) else {
        return Ok(object(Vec::new()));
    };
    let mut rows = Vec::new();
    for id in PANELS {
        let Some(row) = get(record, id) else { continue };
        if !truthy(row) {
            continue;
        }
        if !bounded(get(row, first), first_min, first_max)
            || !bounded(get(row, second), second_min, second_max)
        {
            return Err(if key == "sizes" {
                "invalid_interface_size"
            } else {
                "invalid_interface_position"
            });
        }
        rows.push((
            JsonString::from_utf8(id),
            object(vec![
                (first, get(row, first).unwrap().clone()),
                (second, get(row, second).unwrap().clone()),
            ]),
        ));
    }
    Ok(JsonValue::Object(rows))
}

/// Decode and normalize the supported v1 preferences, dropping unknown keys.
/// The browser still owns LocalStorage reads, text limits and presentation.
pub fn normalize_interface_preferences_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
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
    .map_err(|_| "invalid_interface")?;
    let value = document.root();
    if get(value, "v").and_then(JsonValue::as_u64) != Some(1) {
        return Err("invalid_interface");
    }
    let pinned = get(value, "pinned")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() <= TOOLS.len())
        .ok_or("invalid_interface")?;
    let mut seen = HashSet::new();
    if !pinned.iter().all(|item| {
        item.as_str()
            .is_some_and(|id| TOOLS.contains(&id) && seen.insert(id))
    }) {
        return Err("invalid_interface");
    }
    let dock = choice(value, "dock", &["auto", "left", "right"]).ok_or("invalid_interface")?;
    let text = choice(value, "text", &["comfortable", "large"]).ok_or("invalid_interface")?;
    let labels = choice(value, "labels", &["normal", "large"]).ok_or("invalid_interface")?;
    if !get(value, "sizes")
        .is_some_and(|item| matches!(item, JsonValue::Object(_) | JsonValue::Array(_)))
    {
        return Err("invalid_interface");
    }
    let input_mode = match get(value, "inputMode") {
        None => None,
        Some(item) => Some(
            item.as_str()
                .filter(|text| ["trackpad", "mouse"].contains(text))
                .ok_or("invalid_interface_control")?,
        ),
    };
    let scroll_default = if input_mode == Some("mouse") {
        "zoom"
    } else {
        "auto"
    };
    let scroll = default_choice(
        value,
        "scrollAction",
        scroll_default,
        &["auto", "pan", "zoom"],
    )
    .ok_or("invalid_interface_control")?;
    let drag = default_choice(value, "dragAction", "rotate", &["rotate", "pan"])
        .ok_or("invalid_interface_control")?;
    let sensitivity = default_choice(
        value,
        "sensitivity",
        "normal",
        &["gentle", "normal", "fast"],
    )
    .ok_or("invalid_interface_control")?;
    let motion = default_choice(value, "motion", "system", &["system", "paused", "running"])
        .ok_or("invalid_interface_control")?;
    let language = default_choice(value, "uiLanguage", "ru", &["ru", "en", "es"])
        .ok_or("invalid_interface_control")?;
    let theme = default_choice(value, "theme", "dark", &["dark", "light"])
        .ok_or("invalid_interface_control")?;
    if get(value, "positions").is_some_and(|item| !matches!(item, JsonValue::Object(_))) {
        return Err("invalid_interface_position");
    }
    let sizes = panel_values(
        value, "sizes", "width", "height", 280.0, 760.0, 240.0, 800.0,
    )?;
    let positions = panel_values(value, "positions", "x", "y", 0.0, 1.0, 0.0, 1.0)?;
    let normalized = object(vec![
        ("v", get(value, "v").unwrap().clone()),
        ("pinned", JsonValue::Array(pinned.to_vec())),
        ("dock", string(dock)),
        ("text", string(text)),
        ("labels", string(labels)),
        ("scrollAction", string(scroll)),
        ("dragAction", string(drag)),
        ("sensitivity", string(sensitivity)),
        ("motion", string(motion)),
        ("uiLanguage", string(language)),
        ("theme", string(theme)),
        ("sizes", sizes),
        ("positions", positions),
    ]);
    emit_value_preserved_json(
        &normalized,
        JsonLimits {
            max_bytes: 16_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_interface_output")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_mouse_defaults_and_panel_projection() {
        let raw = br#"{"v":1,"pinned":["reader","search"],"dock":"left","text":"large","labels":"large","inputMode":"mouse","sizes":{"reader":{"width":640,"height":620},"unused":{"width":1}}}"#;
        let bytes = normalize_interface_preferences_v1(raw).unwrap();
        let output = parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        let value = output.root();
        assert_eq!(
            get(value, "scrollAction").and_then(JsonValue::as_str),
            Some("zoom")
        );
        assert_eq!(get(get(value, "sizes").unwrap(), "unused"), None);
        assert_eq!(
            get(
                get(get(value, "sizes").unwrap(), "reader").unwrap(),
                "width"
            )
            .and_then(JsonValue::as_u64),
            Some(640)
        );
    }
    #[test]
    fn unsupported_or_damaged_controls_fail() {
        let raw = br#"{"v":1,"pinned":[],"dock":"auto","text":"large","labels":"normal","sizes":{},"positions":{"settings":{"x":1.5,"y":0}}}"#;
        assert_eq!(
            normalize_interface_preferences_v1(raw),
            Err("invalid_interface_position")
        );
    }
}
