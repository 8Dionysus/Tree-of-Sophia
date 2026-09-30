//! Private durable reading positions. These are versioned selectors only; a
//! current owner read must revalidate the source and Claim path before display.

use std::collections::HashSet;
use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};

use crate::claim_reference::normalize_claim_reference_value;
use crate::human_forms::{content_language_v1, valid_identity_v1};

const MAX_BYTES: usize = 4_000_000;
const MAX_POSITION_KEY_UNITS: usize = 32_768;

fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value.object_get(name)
}
fn word(value: &JsonValue) -> Option<&JsonString> {
    match value {
        JsonValue::String(text) => Some(text),
        _ => None,
    }
}
fn utf8(value: &JsonValue) -> Option<&str> {
    word(value)?.as_str()
}
fn same(left: &JsonValue, right: &JsonValue) -> bool {
    word(left)
        .zip(word(right))
        .is_some_and(|(a, b)| a.units() == b.units())
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn number(value: &JsonValue, lower: f64, upper: f64) -> bool {
    matches!(value, JsonValue::Number(n) if n.lexeme.parse::<f64>().is_ok_and(|v| v.is_finite() && v >= lower && v <= upper))
}
fn one(value: &JsonValue) -> bool {
    number(value, 1.0, 1.0)
}
fn hash(value: &JsonValue) -> bool {
    utf8(value).is_some_and(|word| {
        word.len() == 64
            && word
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn language(value: &JsonValue) -> bool {
    utf8(value).is_some_and(|word| word == "default" || content_language_v1(word))
}
fn decimal(value: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
}
fn anchor_key(value: &str) -> bool {
    if let Some(digits) = value
        .strip_prefix("description:")
        .or_else(|| value.strip_prefix("statement:"))
    {
        return decimal(digits, 1, 6);
    }
    let base = if let Some(digits) = value.strip_prefix("record-context:") {
        return decimal(digits, 1, 3)
            || digits
                .split_once(":part:")
                .is_some_and(|(record, part)| decimal(record, 1, 3) && decimal(part, 1, 6));
    } else if let Some(rest) = value.strip_prefix("claim-context") {
        return rest.is_empty()
            || rest
                .strip_prefix(":part:")
                .is_some_and(|part| decimal(part, 1, 6));
    } else if let Some(rest) = value.strip_prefix("form:") {
        rest
    } else {
        return false;
    };
    let Some((role, tail)) = base.split_once(':') else {
        return false;
    };
    if ![
        "name",
        "caption",
        "hover",
        "statement",
        "grounds",
        "history",
        "technical",
    ]
    .contains(&role)
    {
        return false;
    }
    let (slot, part) = match tail.rsplit_once(":part:") {
        Some((slot, digits)) => (slot, Some(digits)),
        None => (tail, None),
    };
    if part.is_some_and(|part| !decimal(part, 1, 6)) {
        return false;
    }
    ["heading", "wording", "language", "metadata"].contains(&slot)
        || slot
            .strip_prefix("context:")
            .is_some_and(|digits| decimal(digits, 1, 3))
        || slot
            .strip_prefix("binding:")
            .is_some_and(|digits| decimal(digits, 1, 3))
}
fn normalize_anchor(value: &JsonValue) -> Option<JsonValue> {
    if value.is_null() {
        return Some(JsonValue::Null);
    }
    let key = field(value, "key")?;
    if !utf8(key).is_some_and(anchor_key) {
        return None;
    }
    let offset = field(value, "offset")?;
    if !number(offset, -10_000_000.0, 10_000_000.0) {
        return None;
    }
    Some(object(vec![
        ("key", key.clone()),
        ("offset", offset.clone()),
    ]))
}
fn normalize_detail(value: &JsonValue) -> Option<JsonValue> {
    let tuple = value.as_array()?;
    let id = tuple.first()?;
    if !matches!(utf8(id), Some("sources" | "identity")) {
        return None;
    }
    let open = tuple.get(1)?;
    if open.as_bool().is_none() {
        return None;
    }
    Some(JsonValue::Array(vec![id.clone(), open.clone()]))
}
fn normalize_position(value: &JsonValue, entry: &JsonValue) -> Option<JsonValue> {
    let pair = value.as_array()?;
    let key = pair.first()?;
    let raw_key = utf8(key)?;
    if word(key)?.units().len() > MAX_POSITION_KEY_UNITS {
        return None;
    }
    let parsed = parse_json(
        raw_key.as_bytes(),
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 200_000,
            ..JsonLimits::default()
        },
    )
    .ok()?;
    let parts = parsed.root().as_array()?;
    if ![4, 5].contains(&parts.len())
        || !same(&parts[0], field(entry, "_key")?)
        || !same(&parts[1], field(entry, "sourceRevision")?)
        || !same(&parts[2], field(entry, "contentRevision")?)
        || !language(&parts[3])
        || (parts.len() == 5 && !valid_identity_v1(utf8(&parts[4])?, utf8(&parts[3])))
    {
        return None;
    }
    let position = pair.get(1)?;
    let top = field(position, "top")?;
    if !number(top, 0.0, 10_000_000.0) {
        return None;
    }
    let details = field(position, "details")?.as_array()?;
    if details.len() > 2 {
        return None;
    }
    let details: Vec<_> = details
        .iter()
        .map(normalize_detail)
        .collect::<Option<_>>()?;
    let mut seen = HashSet::new();
    if !details
        .iter()
        .all(|detail| seen.insert(detail.as_array().unwrap()[0].as_str().unwrap()))
    {
        return None;
    }
    let anchor = normalize_anchor(field(position, "anchor")?)?;
    Some(JsonValue::Array(vec![
        key.clone(),
        object(vec![
            ("top", top.clone()),
            ("details", JsonValue::Array(details)),
            ("anchor", anchor),
        ]),
    ]))
}
fn normalize_entry(value: &JsonValue) -> Option<JsonValue> {
    let kind = field(value, "kind")?;
    if !matches!(utf8(kind), Some("node" | "relation")) {
        return None;
    }
    let id = field(value, "id")?;
    if !(1..=1024).contains(&word(id)?.units().len()) {
        return None;
    }
    let source = field(value, "sourceRevision")?;
    let content = field(value, "contentRevision")?;
    let preferred = field(value, "preferred")?;
    if !hash(source) || !hash(content) || !language(preferred) {
        return None;
    }
    let positions = field(value, "positions")?.as_array()?;
    if positions.len() > 8 {
        return None;
    }
    let positions: Vec<_> = positions
        .iter()
        .map(|position| normalize_position(position, value))
        .collect::<Option<_>>()?;
    let mut seen = HashSet::new();
    if !positions
        .iter()
        .all(|position| seen.insert(position.as_array().unwrap()[0].as_str().unwrap().to_owned()))
    {
        return None;
    }
    let mut fields = vec![
        ("kind", kind.clone()),
        ("id", id.clone()),
        ("sourceRevision", source.clone()),
        ("contentRevision", content.clone()),
        ("preferred", preferred.clone()),
        ("positions", JsonValue::Array(positions)),
    ];
    if let Some(claim) = field(value, "claimReference") {
        if utf8(kind) != Some("node") {
            return None;
        }
        fields.push((
            "claimReference",
            normalize_claim_reference_value(claim, id).ok()?,
        ));
    }
    Some(object(fields))
}

pub fn normalize_reading_resume_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_reading")?;
    let value = document.root();
    if !field(value, "v").is_some_and(one) {
        return Err("invalid_reading");
    }
    let active = field(value, "activeKey").ok_or("invalid_reading")?;
    let entries = field(value, "entries")
        .and_then(JsonValue::as_array)
        .filter(|items| items.len() <= 2)
        .ok_or("invalid_reading")?;
    let normalized: Vec<_> = entries
        .iter()
        .map(normalize_entry)
        .collect::<Option<_>>()
        .ok_or("invalid_reading")?;
    let mut seen = HashSet::new();
    let keys: Vec<_> = entries
        .iter()
        .map(|entry| field(entry, "_key").ok_or("invalid_reading"))
        .collect::<Result<_, _>>()?;
    if !keys
        .iter()
        .all(|key| word(key).is_some_and(|word| seen.insert(word.units().to_vec())))
        || (entries.is_empty() && !active.is_null())
        || (!entries.is_empty() && !keys.iter().any(|key| same(key, active)))
    {
        return Err("invalid_reading");
    }
    let result = object(vec![
        ("v", field(value, "v").unwrap().clone()),
        ("activeKey", active.clone()),
        ("entries", JsonValue::Array(normalized)),
    ]);
    emit_value_preserved_json(
        &result,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_reading")
}
