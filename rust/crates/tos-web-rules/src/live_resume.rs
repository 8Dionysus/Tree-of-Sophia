//! Private live-view addresses. The host owns source requery, target and
//! geometry normalization, IndexedDB and presentation. This rule binds one
//! saved selector to the exact current revision and Claim path identity.

use std::collections::HashSet;
use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};

use crate::claim_reference::normalize_claim_reference_value;

const MAX_WIRE_BYTES: usize = 1_000_000;
const MAX_RESUME_BYTES: usize = 128 * 1024;
const INVALID: &str = "invalid_live_resume";

fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn string<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    get(value, key)?.as_str()
}
fn exact(value: &JsonValue, fields: &[&str]) -> bool {
    value.as_object().is_some_and(|entries| {
        fields.iter().all(|field| get(value, field).is_some())
            && entries
                .iter()
                .all(|(key, _)| key.as_str().is_some_and(|name| fields.contains(&name)))
    })
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn same(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::String(a), JsonValue::String(b)) => a.units() == b.units(),
        _ => false,
    }
}
fn same_field(left: &JsonValue, right: &JsonValue, key: &str) -> bool {
    get(left, key)
        .zip(get(right, key))
        .is_some_and(|(a, b)| same(a, b))
}
fn same_order(left: &JsonValue, right: &JsonValue, key: &str) -> bool {
    get(left, key)
        .and_then(JsonValue::as_array)
        .zip(get(right, key).and_then(JsonValue::as_array))
        .is_some_and(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b)))
}
fn same_set(left: &JsonValue, right: &JsonValue, key: &str) -> bool {
    get(left, key)
        .and_then(JsonValue::as_array)
        .zip(get(right, key).and_then(JsonValue::as_array))
        .is_some_and(|(a, b)| {
            a.len() == b.len() && {
                let expected: HashSet<Vec<u16>> = a
                    .iter()
                    .filter_map(|v| match v {
                        JsonValue::String(s) => Some(s.units().to_vec()),
                        _ => None,
                    })
                    .collect();
                expected.len() == a.len()
                    && b.iter().all(|v| match v {
                        JsonValue::String(s) => expected.contains(s.units()),
                        _ => false,
                    })
            }
        })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn parse(raw: &[u8]) -> Result<tos_foundation::JsonDocument, &'static str> {
    parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: MAX_WIRE_BYTES,
            ..Default::default()
        },
    )
    .map_err(|_| INVALID)
}
fn emit(value: &JsonValue) -> Result<Vec<u8>, &'static str> {
    emit_value_preserved_json(
        value,
        JsonLimits {
            max_bytes: MAX_WIRE_BYTES,
            ..Default::default()
        },
    )
    .map_err(|_| INVALID)
}

/// The owner-normalized target, selector, layout and pose arrive as one
/// packet. Reject additional durable fields and bind its source revisions.
pub fn validate_live_resume_v1(raw: &[u8]) -> Result<(), &'static str> {
    if raw.len() > MAX_RESUME_BYTES {
        return Err(INVALID);
    }
    let document = parse(raw)?;
    let value = document.root();
    let area = get(value, "area").ok_or(INVALID)?;
    let selection = get(value, "selection").ok_or(INVALID)?;
    let presentation = get(value, "presentation").ok_or(INVALID)?;
    if !exact(
        value,
        &[
            "schema",
            "sourceRevision",
            "area",
            "selection",
            "presentation",
        ],
    ) || string(value, "schema") != Some("tos.live.resume.v1")
        || !string(value, "sourceRevision").is_some_and(digest)
        || !exact(area, &["type", "target"])
        || !matches!(string(area, "type"), Some("route" | "lens"))
        || !exact(presentation, &["layout", "pose", "mode"])
        || !matches!(
            string(presentation, "mode"),
            Some("compact" | "grouped" | "raw")
        )
        || !same_field(selection, value, "sourceRevision")
    {
        return Err(INVALID);
    }
    if string(area, "type") == Some("route") {
        let origin = get(area, "target")
            .and_then(|target| get(target, "origin"))
            .ok_or(INVALID)?;
        if !same_field(origin, value, "sourceRevision") {
            return Err(INVALID);
        }
    }
    Ok(())
}

/// The browser re-queries a source packet and resolves its current path. The
/// rule compares only addresses; neither old nor current wording enters WASM.
pub fn rebind_live_resume_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse(raw)?;
    let request = document.root();
    if !exact(
        request,
        &[
            "saved",
            "sourceRevision",
            "contentRevision",
            "currentClaimReference",
        ],
    ) {
        return Err(INVALID);
    }
    let saved = get(request, "saved").ok_or(INVALID)?;
    let kind = string(saved, "kind").ok_or(INVALID)?;
    let id = get(saved, "id").ok_or(INVALID)?;
    if !matches!(kind, "node" | "relation") || id.as_str().is_none() {
        return Err(INVALID);
    }
    let associated = same_field(saved, request, "sourceRevision")
        && same_field(saved, request, "contentRevision");
    let result = if let Some(expected) = get(saved, "claimReference") {
        if kind != "node" {
            return Err(INVALID);
        }
        let expected = normalize_claim_reference_value(expected, id).map_err(|_| INVALID)?;
        let current = get(request, "currentClaimReference").ok_or(INVALID)?;
        if current.is_null() || !associated {
            JsonValue::Null
        } else {
            let current = normalize_claim_reference_value(current, id).map_err(|_| INVALID)?;
            if ["pathId", "relationType"]
                .iter()
                .all(|key| same_field(&expected, &current, key))
                && ["nodeIds", "relationIds"]
                    .iter()
                    .all(|key| same_order(&expected, &current, key))
                && ["detailRelationIds", "closureNodeIds"]
                    .iter()
                    .all(|key| same_set(&expected, &current, key))
            {
                object(vec![
                    (
                        "kind",
                        JsonValue::String(JsonString::from_utf8("claim-path")),
                    ),
                    ("id", get(&current, "pathId").ok_or(INVALID)?.clone()),
                    ("claimId", get(&current, "claimId").ok_or(INVALID)?.clone()),
                ])
            } else {
                JsonValue::Null
            }
        }
    } else if associated {
        object(vec![
            ("kind", JsonValue::String(JsonString::from_utf8(kind))),
            ("id", id.clone()),
        ])
    } else {
        JsonValue::Null
    };
    emit(&result)
}
