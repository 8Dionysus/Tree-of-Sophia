//! Same-contract saved Lens draft admission and v1-to-v2 normalization.
//! The host provides bounded typed descriptors and native Set cardinalities;
//! it retains exact JS values and iterator behavior. Conditions and paths have
//! their own owners and are normalized after this root rule succeeds.

use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};

fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn boolean(value: &JsonValue, key: &str) -> bool {
    get(value, key).and_then(JsonValue::as_bool) == Some(true)
}
fn integer(value: Option<&JsonValue>) -> Option<f64> {
    let JsonValue::Number(number) = value? else {
        return None;
    };
    number
        .lexeme
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && value.fract() == 0.0)
}
fn text(value: Option<&JsonValue>, min: u64, max: u64) -> bool {
    value.is_some_and(|value| {
        get(value, "kind").and_then(JsonValue::as_str) == Some("string")
            && get(value, "units")
                .and_then(JsonValue::as_u64)
                .is_some_and(|units| (min..=max).contains(&units))
    })
}
fn strings(value: Option<&JsonValue>, max: u64) -> Option<u64> {
    let value = value?;
    let length = get(value, "length")?.as_u64()?;
    if length > max || get(value, "unique_count").and_then(JsonValue::as_u64) != Some(length) {
        return None;
    }
    let items = get(value, "items")?.as_array()?;
    if items.len() as u64 != length {
        return None;
    }
    items
        .iter()
        .all(|item| {
            let hole = get(item, "kind").and_then(JsonValue::as_str) == Some("hole");
            hole || text(Some(item), 1, 1024)
        })
        .then_some(length)
}
fn one_of(value: &JsonValue, key: &str, allowed: &[&str]) -> bool {
    get(value, key)
        .and_then(JsonValue::as_str)
        .is_some_and(|value| allowed.contains(&value))
}

/// Return only the version/compatibility policy needed by the real host
/// normalizer. No names, identifiers, conditions or path payloads are echoed.
pub fn normalize_observatory_draft_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(raw, JsonMode::RequestLastWins, JsonLimits::default())
        .map_err(|_| "invalid_draft_shape")?;
    let value = document.root();
    let max_nodes = integer(get(value, "max_nodes"))
        .filter(|n| *n > 0.0)
        .ok_or("invalid_draft_shape")?;
    let sources = strings(get(value, "sources"), 7);
    let nodes = strings(get(value, "node_ids"), max_nodes as u64);
    let version = integer(get(value, "v"));
    let focus = get(value, "focus_id");
    let null_focus =
        focus.is_some_and(|focus| get(focus, "kind").and_then(JsonValue::as_str) == Some("null"));
    if !boolean(value, "present")
        || !matches!(version, Some(1.0 | 2.0))
        || !text(get(value, "name"), 0, 64)
        || !get(value, "name").is_some_and(|name| boolean(name, "nonempty"))
        || !one_of(value, "scope", &["area", "focus", "all"])
        || sources.is_none_or(|n| n == 0)
        || nodes.is_none()
        || strings(get(value, "kinds"), 100).is_none()
        || strings(get(value, "predicates"), 100).is_none()
        || !text(get(value, "query"), 0, 256)
        || !(null_focus || text(focus, 1, 1024))
        || integer(get(value, "depth")).is_none_or(|n| !(0.0..=3.0).contains(&n))
        || !one_of(value, "direction", &["either", "outgoing", "incoming"])
        || !one_of(value, "profile", &["overview", "all"])
        || integer(get(value, "limit")).is_none_or(|n| n < 1.0 || n > max_nodes)
        || get(value, "relations")
            .and_then(JsonValue::as_bool)
            .is_none()
    {
        return Err("invalid_draft_shape");
    }
    if get(value, "scope").and_then(JsonValue::as_str) == Some("area") && nodes == Some(0) {
        return Err("empty_draft_area");
    }
    if get(value, "scope").and_then(JsonValue::as_str) == Some("focus") && null_focus {
        return Err("empty_draft_focus");
    }
    let legacy = version == Some(1.0);
    if legacy
        && (boolean(value, "conditions_defined")
            || boolean(value, "paths_defined")
                && (!boolean(value, "paths_array")
                    || get(value, "paths_length").and_then(JsonValue::as_u64) != Some(0)))
    {
        return Err("invalid_draft_version");
    }
    let result = JsonValue::Object(vec![
        (
            JsonString::from_utf8("v"),
            JsonValue::Number(tos_foundation::JsonNumber {
                kind: tos_foundation::JsonNumberKind::Int,
                lexeme: "2".to_owned(),
            }),
        ),
        (JsonString::from_utf8("legacy"), JsonValue::Bool(legacy)),
        (
            JsonString::from_utf8("retain_paths"),
            JsonValue::Bool(boolean(value, "retain_paths")),
        ),
    ]);
    emit_value_preserved_json(&result, JsonLimits::default()).map_err(|_| "invalid_draft_output")
}

#[cfg(test)]
mod tests {
    use super::*;
    const DRAFT: &str = r#"{"present":true,"max_nodes":40,"v":2,"name":{"kind":"string","units":1,"nonempty":true},"scope":"all","sources":{"length":1,"unique_count":1,"items":[{"kind":"string","units":1}]},"node_ids":{"length":0,"unique_count":0,"items":[]},"kinds":{"length":0,"unique_count":0,"items":[]},"predicates":{"length":0,"unique_count":0,"items":[]},"query":{"kind":"string","units":0},"focus_id":{"kind":"null"},"depth":0,"direction":"either","profile":"overview","limit":40,"relations":false,"conditions_defined":false,"paths_defined":false,"paths_array":false,"paths_length":null,"retain_paths":false}"#;

    #[test]
    fn root_policy_preserves_version_path_retention_and_error_precedence() {
        assert_eq!(
            normalize_observatory_draft_v1(DRAFT.as_bytes()).unwrap(),
            br#"{"v":2,"legacy":false,"retain_paths":false}"#
        );
        let legacy = DRAFT.replace("\"v\":2", "\"v\":1");
        assert_eq!(
            normalize_observatory_draft_v1(legacy.as_bytes()).unwrap(),
            br#"{"v":2,"legacy":true,"retain_paths":false}"#
        );
        let conditions = legacy.replace(
            "\"conditions_defined\":false",
            "\"conditions_defined\":true",
        );
        assert_eq!(
            normalize_observatory_draft_v1(conditions.as_bytes()),
            Err("invalid_draft_version")
        );
        let area = conditions.replace("\"scope\":\"all\"", "\"scope\":\"area\"");
        assert_eq!(
            normalize_observatory_draft_v1(area.as_bytes()),
            Err("empty_draft_area")
        );
        let focus = conditions.replace("\"scope\":\"all\"", "\"scope\":\"focus\"");
        assert_eq!(
            normalize_observatory_draft_v1(focus.as_bytes()),
            Err("empty_draft_focus")
        );
        let paths = legacy.replace("\"paths_defined\":false", "\"paths_defined\":true");
        assert_eq!(
            normalize_observatory_draft_v1(paths.as_bytes()),
            Err("invalid_draft_version")
        );
        let empty_paths = paths
            .replace("\"paths_array\":false", "\"paths_array\":true")
            .replace("\"paths_length\":null", "\"paths_length\":0")
            .replace("\"retain_paths\":false", "\"retain_paths\":true");
        assert_eq!(
            normalize_observatory_draft_v1(empty_paths.as_bytes()).unwrap(),
            br#"{"v":2,"legacy":true,"retain_paths":true}"#
        );
    }

    #[test]
    fn typed_array_measurements_preserve_sparse_and_unique_boundaries() {
        for (raw, expected) in [
            (
                r#"{"length":1,"unique_count":1,"items":[{"kind":"hole"}]}"#,
                Some(1),
            ),
            (
                r#"{"length":2,"unique_count":1,"items":[{"kind":"hole"},{"kind":"hole"}]}"#,
                None,
            ),
            (
                r#"{"length":1,"unique_count":1,"items":[{"kind":"invalid"}]}"#,
                None,
            ),
            (
                r#"{"length":1,"unique_count":1,"items":[{"kind":"string","units":1024}]}"#,
                Some(1),
            ),
            (
                r#"{"length":1,"unique_count":1,"items":[{"kind":"string","units":1025}]}"#,
                None,
            ),
        ] {
            let document = parse_json(
                raw.as_bytes(),
                JsonMode::RequestLastWins,
                JsonLimits::default(),
            )
            .unwrap();
            assert_eq!(strings(Some(document.root()), 100), expected);
        }
    }
}
