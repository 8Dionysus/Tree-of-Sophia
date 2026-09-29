//! Local research shelf packet and transition rules. These packets are private
//! addresses and annotations; validation does not assess source or rights.

use std::collections::HashSet;
use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};

const MAX_SAFE: u64 = 9_007_199_254_740_991;
// At most two bounded shelf records plus framing enter one rule call. Whole
// exports remain in the host; this rule does not duplicate their contents.
const INPUT_BYTES: usize = 1_048_576;

fn invalid(code: &'static str) -> Result<JsonValue, &'static str> {
    Err(code)
}
fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn string<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    get(value, key)?.as_str()
}
fn number(value: &JsonValue, key: &str, min: u64) -> Option<u64> {
    get(value, key)?
        .as_u64()
        .filter(|n| *n >= min && *n <= MAX_SAFE)
}
fn obj(items: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        items
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn num(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn exact(value: &JsonValue, required: &[&str], optional: &[&str]) -> bool {
    value.as_object().is_some_and(|entries| {
        required.iter().all(|key| get(value, key).is_some())
            && entries.iter().all(|(key, _)| {
                key.as_str()
                    .is_some_and(|name| required.contains(&name) || optional.contains(&name))
            })
    })
}
fn bounded(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.encode_utf16().count() <= max
        && !value.chars().any(|c| c < '\u{20}' || c == '\u{7f}')
}
fn js_trim(ch: char) -> bool {
    matches!(ch,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|
        '\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
fn ids(value: &JsonValue, max_count: usize, max_len: usize) -> bool {
    value.as_array().is_some_and(|values| {
        values.len() <= max_count && {
            let mut seen = HashSet::new();
            values.iter().all(|v| {
                v.as_str()
                    .is_some_and(|s| bounded(s, max_len) && seen.insert(s))
            })
        }
    })
}
fn timestamp(value: &str) -> bool {
    // The host canonicalizes Date values before calling this rule. No local
    // source chronology can be inferred from this stored timestamp.
    value.len() <= 64 && value.ends_with('Z') && value.contains('T') && value.contains('.')
}
fn timestamp_parts(value: &str) -> Option<(i32, &str)> {
    let width = if value.starts_with('+') || value.starts_with('-') {
        7
    } else {
        4
    };
    Some((value.get(..width)?.parse().ok()?, value.get(width..)?))
}
fn timestamp_before(left: &str, right: &str) -> bool {
    match (timestamp_parts(left), timestamp_parts(right)) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn material(value: &JsonValue) -> bool {
    exact(
        value,
        &["kind", "id", "sourceRevision", "contentRevision"],
        &["claimReference"],
    ) && string(value, "kind").is_some_and(|s| ["node", "relation"].contains(&s))
        && string(value, "id").is_some_and(|s| bounded(s, 1024))
        && string(value, "sourceRevision").is_some_and(digest)
        && string(value, "contentRevision").is_some_and(digest)
        && get(value, "claimReference").is_none_or(|claim| {
            string(value, "kind") == Some("node")
                && exact(
                    claim,
                    &[
                        "claimId",
                        "pathId",
                        "relationType",
                        "nodeIds",
                        "relationIds",
                        "detailRelationIds",
                        "closureNodeIds",
                    ],
                    &[],
                )
                && ["claimId", "pathId", "relationType"]
                    .iter()
                    .all(|key| string(claim, key).is_some_and(|s| bounded(s, 1024)))
                && [
                    ("nodeIds", 3),
                    ("relationIds", 2),
                    ("detailRelationIds", 78),
                    ("closureNodeIds", 40),
                ]
                .iter()
                .all(|(key, max)| get(claim, key).is_some_and(|v| ids(v, *max, 1024)))
        })
}
fn target(kind: &str, value: &JsonValue) -> bool {
    match kind {
        "material" => material(value),
        "form" => {
            exact(value, &["material", "role", "form"], &[])
                && get(value, "material").is_some_and(material)
                && string(value, "role").is_some_and(|role| {
                    [
                        "name",
                        "caption",
                        "hover",
                        "statement",
                        "grounds",
                        "history",
                        "technical",
                    ]
                    .contains(&role)
                })
                && get(value, "form").is_some_and(|form| {
                    exact(form, &["id", "version", "digest"], &[])
                        && string(form, "id").is_some_and(|s| bounded(s, 1024))
                        && number(form, "version", 1).is_some()
                        && string(form, "digest")
                            .is_some_and(|s| s.strip_prefix("sha256:").is_some_and(digest))
                })
        }
        "text" => {
            exact(value, &["reference"], &[])
                && get(value, "reference").is_some_and(|v| v.as_object().is_some())
        }
        "lens" => {
            exact(value, &["draft"], &[])
                && get(value, "draft").is_some_and(|v| v.as_object().is_some())
        }
        "route" => {
            exact(value, &["origin", "options"], &[])
                && get(value, "origin").is_some_and(material)
                && get(value, "options").is_some_and(|options| {
                    exact(
                        options,
                        &[
                            "profile",
                            "direction",
                            "max_depth",
                            "sources",
                            "predicate_ids",
                        ],
                        &[],
                    ) && string(options, "profile")
                        .is_some_and(|s| ["overview", "all"].contains(&s))
                        && string(options, "direction")
                            .is_some_and(|s| ["either", "incoming", "outgoing"].contains(&s))
                        && number(options, "max_depth", 0).is_some_and(|n| n <= 10)
                        && get(options, "sources").is_some_and(|v| {
                            ids(v, 100, 1024) && v.as_array().is_some_and(|a| !a.is_empty())
                        })
                        && get(options, "predicate_ids").is_some_and(|v| ids(v, 100, 1024))
                })
        }
        _ => false,
    }
}
fn validate_record(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(
        value,
        &[
            "id",
            "title",
            "type",
            "target",
            "collectionIds",
            "createdAt",
            "updatedAt",
            "revision",
        ],
        &[],
    ) {
        return invalid("invalid-target");
    }
    let id = string(value, "id")
        .filter(|s| bounded(s, 1024))
        .ok_or("invalid-target")?;
    let title = string(value, "title")
        .filter(|s| bounded(s, 256))
        .ok_or("invalid-target")?;
    let kind = string(value, "type")
        .filter(|s| ["material", "form", "text", "lens", "route"].contains(s))
        .ok_or("invalid-record")?;
    if title.trim_matches(js_trim).is_empty() {
        return invalid("invalid-record");
    }
    if !get(value, "target").is_some_and(|v| target(kind, v))
        || !get(value, "collectionIds").is_some_and(|v| ids(v, 32, 256))
    {
        return invalid("invalid-target");
    }
    let created = string(value, "createdAt")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    let updated = string(value, "updatedAt")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    if timestamp_before(updated, created) {
        return invalid("invalid-record");
    }
    let revision = number(value, "revision", 1).ok_or("invalid-input")?;
    let record = obj(vec![
        ("id", JsonValue::String(JsonString::from_utf8(id))),
        (
            "title",
            JsonValue::String(JsonString::from_utf8(title.trim_matches(js_trim))),
        ),
        ("type", JsonValue::String(JsonString::from_utf8(kind))),
        ("target", get(value, "target").unwrap().clone()),
        (
            "collectionIds",
            get(value, "collectionIds").unwrap().clone(),
        ),
        (
            "createdAt",
            JsonValue::String(JsonString::from_utf8(created)),
        ),
        (
            "updatedAt",
            JsonValue::String(JsonString::from_utf8(updated)),
        ),
        ("revision", num(revision)),
    ]);
    // The owning target decoder runs in the caller. This is the shelf's exact
    // emitted record cap, applied before returning a second host object.
    emit_value_preserved_json(
        &record,
        JsonLimits {
            max_bytes: 208_192,
            ..Default::default()
        },
    )
    .map_err(|_| "limit")?;
    Ok(record)
}
fn validate_collection(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(
        value,
        &["id", "title", "createdAt", "updatedAt", "revision"],
        &[],
    ) {
        return invalid("invalid-target");
    }
    let id = string(value, "id")
        .filter(|s| bounded(s, 256))
        .ok_or("invalid-target")?;
    let title = string(value, "title")
        .filter(|s| bounded(s, 128))
        .ok_or("invalid-target")?;
    if title.trim_matches(js_trim).is_empty() {
        return invalid("invalid-record");
    }
    let created = string(value, "createdAt")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    let updated = string(value, "updatedAt")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    if timestamp_before(updated, created) {
        return invalid("invalid-record");
    }
    let revision = number(value, "revision", 1).ok_or("invalid-input")?;
    Ok(obj(vec![
        ("id", JsonValue::String(JsonString::from_utf8(id))),
        (
            "title",
            JsonValue::String(JsonString::from_utf8(title.trim_matches(js_trim))),
        ),
        (
            "createdAt",
            JsonValue::String(JsonString::from_utf8(created)),
        ),
        (
            "updatedAt",
            JsonValue::String(JsonString::from_utf8(updated)),
        ),
        ("revision", num(revision)),
    ]))
}
fn field_or(value: &JsonValue, key: &str, fallback: JsonValue) -> JsonValue {
    match get(value, key) {
        None | Some(JsonValue::Null) => fallback,
        Some(item) => item.clone(),
    }
}
fn create_record(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["input", "id", "now"], &[]) {
        return invalid("invalid-input");
    }
    let input = get(value, "input").ok_or("invalid-input")?;
    if !exact(
        input,
        &[],
        &[
            "id",
            "title",
            "type",
            "target",
            "collectionIds",
            "createdAt",
            "updatedAt",
            "revision",
        ],
    ) || ["createdAt", "updatedAt", "revision"]
        .iter()
        .any(|key| get(input, key).is_some())
    {
        return invalid("invalid-input");
    }
    let id = string(value, "id")
        .filter(|s| bounded(s, 1024))
        .ok_or("invalid-target")?;
    let at = string(value, "now")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    if get(input, "id").is_some_and(|v| v.as_str() != Some(id)) {
        return invalid("invalid-input");
    }
    validate_record(&obj(vec![
        ("id", JsonValue::String(JsonString::from_utf8(id))),
        (
            "title",
            get(input, "title").cloned().unwrap_or(JsonValue::Null),
        ),
        (
            "type",
            get(input, "type").cloned().unwrap_or(JsonValue::Null),
        ),
        (
            "target",
            get(input, "target").cloned().unwrap_or(JsonValue::Null),
        ),
        (
            "collectionIds",
            field_or(input, "collectionIds", JsonValue::Array(Vec::new())),
        ),
        ("createdAt", JsonValue::String(JsonString::from_utf8(at))),
        ("updatedAt", JsonValue::String(JsonString::from_utf8(at))),
        ("revision", num(1)),
    ]))
}
fn update_record(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["old", "input", "now"], &[]) {
        return invalid("invalid-input");
    }
    let old = get(value, "old").ok_or("invalid-record")?;
    let input = get(value, "input").ok_or("invalid-input")?;
    if !exact(
        input,
        &[],
        &[
            "id",
            "title",
            "type",
            "target",
            "collectionIds",
            "createdAt",
            "updatedAt",
            "revision",
        ],
    ) {
        return invalid("invalid-input");
    }
    let id = string(old, "id").ok_or("invalid-record")?;
    let kind = string(old, "type").ok_or("invalid-record")?;
    if get(input, "id").is_some_and(|v| v.as_str() != Some(id))
        || get(input, "type").is_some_and(|v| v.as_str() != Some(kind))
        || get(input, "target").is_some_and(|v| Some(v) != get(old, "target"))
        || get(input, "createdAt").is_some_and(|v| Some(v) != get(old, "createdAt"))
    {
        return invalid("invalid-input");
    }
    let revision = number(old, "revision", 1)
        .ok_or("invalid-record")?
        .checked_add(1)
        .filter(|n| *n <= MAX_SAFE)
        .ok_or("invalid-input")?;
    let at = string(value, "now")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    validate_record(&obj(vec![
        ("id", get(old, "id").unwrap().clone()),
        (
            "title",
            field_or(input, "title", get(old, "title").unwrap().clone()),
        ),
        ("type", get(old, "type").unwrap().clone()),
        ("target", get(old, "target").unwrap().clone()),
        (
            "collectionIds",
            field_or(
                input,
                "collectionIds",
                get(old, "collectionIds").unwrap().clone(),
            ),
        ),
        ("createdAt", get(old, "createdAt").unwrap().clone()),
        ("updatedAt", JsonValue::String(JsonString::from_utf8(at))),
        ("revision", num(revision)),
    ]))
}
fn create_collection(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["input", "id", "now"], &[]) {
        return invalid("invalid-input");
    }
    let input = get(value, "input").ok_or("invalid-input")?;
    if !exact(input, &[], &["id", "title"]) {
        return invalid("invalid-input");
    }
    let id = string(value, "id")
        .filter(|s| bounded(s, 256))
        .ok_or("invalid-target")?;
    if get(input, "id").is_some_and(|v| !v.is_null() && v.as_str() != Some(id)) {
        return invalid("invalid-input");
    }
    let at = string(value, "now")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    validate_collection(&obj(vec![
        ("id", JsonValue::String(JsonString::from_utf8(id))),
        (
            "title",
            get(input, "title").cloned().unwrap_or(JsonValue::Null),
        ),
        ("createdAt", JsonValue::String(JsonString::from_utf8(at))),
        ("updatedAt", JsonValue::String(JsonString::from_utf8(at))),
        ("revision", num(1)),
    ]))
}
fn update_collection(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["old", "input", "now"], &[]) {
        return invalid("invalid-input");
    }
    let old = get(value, "old").ok_or("invalid-record")?;
    let input = get(value, "input").ok_or("invalid-input")?;
    if !exact(input, &[], &["id", "title"]) {
        return invalid("invalid-input");
    }
    if get(input, "id").is_some_and(|v| Some(v) != get(old, "id")) {
        return invalid("invalid-input");
    }
    let revision = number(old, "revision", 1)
        .ok_or("invalid-record")?
        .checked_add(1)
        .filter(|n| *n <= MAX_SAFE)
        .ok_or("invalid-input")?;
    let at = string(value, "now")
        .filter(|s| timestamp(s))
        .ok_or("invalid-record")?;
    validate_collection(&obj(vec![
        ("id", get(old, "id").ok_or("invalid-record")?.clone()),
        (
            "title",
            field_or(
                input,
                "title",
                get(old, "title").ok_or("invalid-record")?.clone(),
            ),
        ),
        (
            "createdAt",
            get(old, "createdAt").ok_or("invalid-record")?.clone(),
        ),
        ("updatedAt", JsonValue::String(JsonString::from_utf8(at))),
        ("revision", num(revision)),
    ]))
}
fn transition(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(
        value,
        &["kind", "operation", "generation", "expected", "old"],
        &["incoming"],
    ) {
        return invalid("invalid-input");
    }
    let kind = string(value, "kind").ok_or("invalid-input")?;
    let op = string(value, "operation").ok_or("invalid-input")?;
    if !["record", "collection"].contains(&kind) || !["save", "delete"].contains(&op) {
        return invalid("invalid-input");
    }
    let generation = number(value, "generation", 0).ok_or("invalid-input")?;
    let old = get(value, "old").ok_or("invalid-input")?;
    let expected = get(value, "expected").ok_or("invalid-input")?;
    let old_revision = if old.is_null() {
        None
    } else {
        Some(number(old, "revision", 1).ok_or("invalid-record")?)
    };
    let expected_revision = if expected.is_null() {
        None
    } else {
        Some(
            expected
                .as_u64()
                .filter(|n| *n > 0 && *n <= MAX_SAFE)
                .ok_or("invalid-input")?,
        )
    };
    if op == "delete" && old_revision.is_none() {
        return invalid("not-found");
    }
    if op == "save" && old_revision.is_some() != expected_revision.is_some() {
        return invalid("conflict");
    }
    if expected_revision.is_some() && expected_revision != old_revision {
        return invalid("conflict");
    }
    if op == "save" {
        let incoming = get(value, "incoming").ok_or("invalid-input")?;
        if kind == "record" {
            validate_record(incoming)?;
        } else {
            validate_collection(incoming)?;
        }
        if number(incoming, "revision", 1) != Some(old_revision.unwrap_or(0) + 1) {
            return invalid("invalid-record");
        }
        if old_revision.is_some() && string(incoming, "id") != string(old, "id") {
            return invalid("invalid-record");
        }
    }
    let next = generation
        .checked_add(1)
        .filter(|n| *n <= MAX_SAFE)
        .ok_or("limit")?;
    Ok(obj(vec![("generation", num(next))]))
}
fn detach(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["record", "collectionId", "now"], &[]) {
        return invalid("invalid-input");
    }
    let record = validate_record(get(value, "record").ok_or("invalid-record")?)?;
    let id = string(value, "collectionId").ok_or("invalid-input")?;
    let at = string(value, "now").ok_or("invalid-input")?;
    if !bounded(id, 256) || !timestamp(at) {
        return invalid("invalid-input");
    }
    let old_at = string(&record, "updatedAt").ok_or("invalid-record")?;
    let revision = number(&record, "revision", 1)
        .ok_or("invalid-record")?
        .checked_add(1)
        .filter(|n| *n <= MAX_SAFE)
        .ok_or("invalid-input")?;
    let entries = record.as_object().ok_or("invalid-record")?;
    let changed = entries
        .iter()
        .map(|(key, item)| {
            let name = key.as_str().unwrap_or("");
            let next = match name {
                "collectionIds" => JsonValue::Array(
                    item.as_array()
                        .unwrap_or(&[])
                        .iter()
                        .filter(|v| v.as_str() != Some(id))
                        .cloned()
                        .collect(),
                ),
                "revision" => num(revision),
                "updatedAt" => {
                    JsonValue::String(JsonString::from_utf8(if timestamp_before(at, old_at) {
                        old_at
                    } else {
                        at
                    }))
                }
                _ => item.clone(),
            };
            (key.clone(), next)
        })
        .collect();
    validate_record(&JsonValue::Object(changed))
}
fn list_options(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["limit", "type", "collectionId", "cursor"], &[])
        || !number(value, "limit", 1).is_some_and(|n| n <= 100)
        || !get(value, "type").is_some_and(|v| {
            v.is_null()
                || v.as_str()
                    .is_some_and(|s| ["material", "form", "text", "lens", "route"].contains(&s))
        })
        || !get(value, "collectionId")
            .is_some_and(|v| v.is_null() || v.as_str().is_some_and(|s| bounded(s, 256)))
        || !get(value, "cursor")
            .is_some_and(|v| v.is_null() || v.as_str().is_some_and(|s| bounded(s, 16_384)))
    {
        return invalid("invalid-input");
    }
    Ok(value.clone())
}
fn cursor(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(
        value,
        &["decoded", "generation", "type", "collectionId"],
        &[],
    ) {
        return invalid("invalid-cursor");
    }
    let decoded = get(value, "decoded").ok_or("invalid-cursor")?;
    let filter = get(decoded, "filter").ok_or("invalid-cursor")?;
    if !exact(
        decoded,
        &["v", "generation", "filter", "updatedAt", "id"],
        &[],
    ) || !exact(filter, &["type", "collectionId"], &[])
        || number(decoded, "v", 1) != Some(1)
        || number(decoded, "generation", 0) != number(value, "generation", 0)
        || get(filter, "type") != get(value, "type")
        || get(filter, "collectionId") != get(value, "collectionId")
        || !string(decoded, "updatedAt").is_some_and(timestamp)
        || !string(decoded, "id").is_some_and(|s| bounded(s, 1024))
    {
        return invalid("invalid-cursor");
    }
    Ok(decoded.clone())
}
fn sorted(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(sorted).collect()),
        JsonValue::Object(items) => {
            let mut entries: Vec<_> = items.iter().map(|(k, v)| (k.clone(), sorted(v))).collect();
            entries.sort_by(|a, b| a.0.units().cmp(b.0.units()));
            JsonValue::Object(entries)
        }
        _ => value.clone(),
    }
}
fn string_units(value: &JsonValue) -> Option<&[u16]> {
    match value {
        JsonValue::String(s) => Some(s.units()),
        _ => None,
    }
}

/// One export's sorted-ID index. Only the last exact ID and counts persist;
/// owner packet bodies and record targets never accumulate in WASM.
pub struct ShelfPacketIndex {
    last_record_id: Option<JsonString>,
    last_collection_id: Option<JsonString>,
    seen_records: usize,
    seen_collections: usize,
    expected_records: usize,
    expected_collections: usize,
}
impl ShelfPacketIndex {
    pub fn new(header: &[u8]) -> Result<Self, &'static str> {
        let document = parse_json(
            header,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: 4096,
                ..Default::default()
            },
        )
        .map_err(|_| "invalid-packet")?;
        let value = document.root();
        if !exact(
            value,
            &[
                "schema",
                "version",
                "generation",
                "recordCount",
                "collectionCount",
            ],
            &["exportedAt"],
        ) || string(value, "schema") != Some("tos.research_shelf.export.v1")
            || number(value, "version", 1) != Some(1)
            || number(value, "generation", 0).is_none()
            || get(value, "exportedAt").is_some_and(|v| v.as_str().is_none_or(|s| !timestamp(s)))
        {
            return Err("invalid-packet");
        }
        let records = number(value, "recordCount", 0).ok_or("invalid-packet")?;
        let collections = number(value, "collectionCount", 0).ok_or("invalid-packet")?;
        if records > 200_000 || collections > 2_000 {
            return Err("limit");
        }
        Ok(Self {
            last_record_id: None,
            last_collection_id: None,
            seen_records: 0,
            seen_collections: 0,
            expected_records: records as usize,
            expected_collections: collections as usize,
        })
    }
    fn accept(&mut self, raw: &[u8], records: bool) -> Result<(), &'static str> {
        let document = parse_json(
            raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: INPUT_BYTES,
                ..Default::default()
            },
        )
        .map_err(|_| "invalid-packet")?;
        let values = document.root().as_array().ok_or("invalid-packet")?;
        let (last, seen, expected, max_len) = if records {
            (
                &mut self.last_record_id,
                &mut self.seen_records,
                self.expected_records,
                1024,
            )
        } else {
            (
                &mut self.last_collection_id,
                &mut self.seen_collections,
                self.expected_collections,
                256,
            )
        };
        if values.len() > expected.saturating_sub(*seen) {
            return Err("limit");
        }
        for value in values {
            if !value.as_str().is_some_and(|id| bounded(id, max_len)) {
                return Err("invalid-packet");
            }
        }
        if values.first().is_some_and(|first| {
            last.as_ref()
                .is_some_and(|prior| prior.units() >= string_units(first).unwrap_or(&[]))
        }) || values.windows(2).any(|pair| {
            string_units(&pair[0]).unwrap_or(&[]) >= string_units(&pair[1]).unwrap_or(&[])
        }) {
            return Err("invalid-packet");
        }
        if let Some(JsonValue::String(id)) = values.last() {
            *last = Some(id.clone());
        }
        *seen += values.len();
        Ok(())
    }
    pub fn accept_records(&mut self, ids: &[u8]) -> Result<(), &'static str> {
        self.accept(ids, true)
    }
    pub fn accept_collections(&mut self, ids: &[u8]) -> Result<(), &'static str> {
        self.accept(ids, false)
    }
    pub fn finish(&self) -> Result<(), &'static str> {
        if self.seen_records != self.expected_records
            || self.seen_collections != self.expected_collections
        {
            return Err("invalid-packet");
        }
        Ok(())
    }
}

fn import_plan(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(
        value,
        &[
            "generation",
            "currentRecords",
            "currentCollections",
            "recordAdditions",
            "collectionAdditions",
            "expectedGeneration",
        ],
        &[],
    ) {
        return invalid("invalid-input");
    }
    let generation = number(value, "generation", 0).ok_or("invalid-input")?;
    let expected = get(value, "expectedGeneration").ok_or("invalid-input")?;
    if !expected.is_null() && expected.as_u64() != Some(generation) {
        return invalid("conflict");
    }
    let current_records = number(value, "currentRecords", 0).ok_or("invalid-input")?;
    let current_collections = number(value, "currentCollections", 0).ok_or("invalid-input")?;
    let add_records = number(value, "recordAdditions", 0).ok_or("invalid-input")?;
    let add_collections = number(value, "collectionAdditions", 0).ok_or("invalid-input")?;
    if current_records
        .checked_add(add_records)
        .is_none_or(|n| n > 200_000)
        || current_collections
            .checked_add(add_collections)
            .is_none_or(|n| n > 2_000)
    {
        return invalid("limit");
    }
    let changed = add_records > 0 || add_collections > 0;
    let next = if changed {
        generation
            .checked_add(1)
            .filter(|n| *n <= MAX_SAFE)
            .ok_or("limit")?
    } else {
        generation
    };
    Ok(obj(vec![
        ("generation", num(next)),
        (
            "counts",
            obj(vec![
                ("records", num(add_records)),
                ("collections", num(add_collections)),
            ]),
        ),
        ("changed", JsonValue::Bool(changed)),
    ]))
}
fn migration_id(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["type", "target"], &[]) {
        return invalid("invalid-input");
    }
    let kind = string(value, "type").ok_or("invalid-input")?;
    let body = get(value, "target").ok_or("invalid-input")?;
    if !["material", "lens"].contains(&kind) || !target(kind, body) {
        return invalid("invalid-target");
    }
    let stable = sorted(&obj(vec![("target", body.clone())]));
    let bytes = emit_value_preserved_json(
        &stable,
        JsonLimits {
            max_bytes: INPUT_BYTES,
            ..Default::default()
        },
    )
    .map_err(|_| "limit")?;
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3)
    }
    Ok(JsonValue::String(JsonString::from_utf8(&format!(
        "import:{kind}:{hash:016x}"
    ))))
}
fn migration_policy(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["source", "section"], &[]) {
        return invalid("invalid-input");
    }
    let source = string(value, "source").ok_or("invalid-input")?;
    let section = string(value, "section").ok_or("invalid-input")?;
    if !["reading-resume", "research-workspace", "workspace-copy"].contains(&source) {
        return invalid("migration-required");
    }
    let posture = match section {
        "entries" if source != "research-workspace" => "retain-material",
        "lenses" if source != "reading-resume" => "retain-lens",
        "entries.positions" if source != "research-workspace" => "non-portable-reading-position",
        "selected_lens" => "non-portable-lens-selection",
        "excluded_edge_ids" | "route_snapshots" | "places" | "history" | "resume" => {
            "legacy-graph-pose"
        }
        "hypotheses" | "proposals" => "non-portable-research-record",
        "notes" => "non-portable-note",
        _ => return invalid("invalid-input"),
    };
    Ok(JsonValue::String(JsonString::from_utf8(posture)))
}
// A single canonical owner target enters at a time. Full workspace-copy bodies
// and all output records never coexist in one WASM request.
fn migration_record(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(
        value,
        &["source", "section", "index", "target", "now"],
        &["hasPositions"],
    ) {
        return invalid("invalid-input");
    }
    let source = string(value, "source").ok_or("migration-required")?;
    let section = string(value, "section").ok_or("invalid-input")?;
    let index = number(value, "index", 0).ok_or("invalid-input")?;
    let body = get(value, "target").ok_or("invalid-target")?;
    let kind = match (source, section) {
        ("reading-resume" | "workspace-copy", "entries") if index < 2 => "material",
        ("research-workspace" | "workspace-copy", "lenses") if index < 12 => "lens",
        _ => return invalid("invalid-input"),
    };
    let has_positions = match get(value, "hasPositions") {
        None => false,
        Some(value) => value.as_bool().ok_or("invalid-input")?,
    };
    if kind != "material" && has_positions {
        return invalid("invalid-input");
    }
    let skipped = if has_positions {
        vec![obj(vec![
            ("source", JsonValue::String(JsonString::from_utf8(source))),
            (
                "section",
                JsonValue::String(JsonString::from_utf8("entries")),
            ),
            ("index", num(index)),
            (
                "reason",
                JsonValue::String(JsonString::from_utf8("non-portable-reading-position")),
            ),
        ])]
    } else {
        Vec::new()
    };
    let id = migration_id(&obj(vec![
        ("type", JsonValue::String(JsonString::from_utf8(kind))),
        ("target", body.clone()),
    ]))?;
    let title = if kind == "material" {
        "Материал"
    } else {
        get(body, "draft")
            .and_then(|draft| string(draft, "name"))
            .ok_or("invalid-target")?
    };
    let record = create_record(&obj(vec![
        (
            "input",
            obj(vec![
                ("title", JsonValue::String(JsonString::from_utf8(title))),
                ("type", JsonValue::String(JsonString::from_utf8(kind))),
                ("target", body.clone()),
                ("collectionIds", JsonValue::Array(Vec::new())),
            ]),
        ),
        ("id", id.clone()),
        ("now", get(value, "now").ok_or("invalid-record")?.clone()),
    ]))?;
    Ok(obj(vec![
        ("record", record),
        ("skipped", JsonValue::Array(skipped)),
        (
            "retained",
            obj(vec![
                ("source", JsonValue::String(JsonString::from_utf8(source))),
                ("section", JsonValue::String(JsonString::from_utf8(section))),
                ("index", num(index)),
                ("type", JsonValue::String(JsonString::from_utf8(kind))),
                ("recordId", id),
            ]),
        ),
    ]))
}
fn migration_skips(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if !exact(value, &["source", "section", "count", "single"], &[]) {
        return invalid("invalid-input");
    }
    let source = string(value, "source").ok_or("migration-required")?;
    let section = string(value, "section").ok_or("invalid-input")?;
    let count = number(value, "count", 0)
        .filter(|n| *n <= 256)
        .ok_or("limit")?;
    let single = get(value, "single")
        .and_then(JsonValue::as_bool)
        .ok_or("invalid-input")?;
    if single && count > 1 {
        return invalid("invalid-input");
    }
    let reason = migration_policy(&obj(vec![
        ("source", JsonValue::String(JsonString::from_utf8(source))),
        ("section", JsonValue::String(JsonString::from_utf8(section))),
    ]))?;
    if reason.as_str().is_some_and(|s| s.starts_with("retain-")) {
        return invalid("invalid-input");
    }
    Ok(JsonValue::Array(
        (0..count)
            .map(|index| {
                obj(vec![
                    ("source", JsonValue::String(JsonString::from_utf8(source))),
                    ("section", JsonValue::String(JsonString::from_utf8(section))),
                    ("index", if single { JsonValue::Null } else { num(index) }),
                    ("reason", reason.clone()),
                ])
            })
            .collect(),
    ))
}
pub fn research_shelf_rule_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let doc = parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: INPUT_BYTES,
            ..Default::default()
        },
    )
    .map_err(|_| "invalid-input")?;
    let root = doc.root();
    if !exact(root, &["operation", "value"], &[]) {
        return Err("invalid-input");
    }
    let value = get(root, "value").ok_or("invalid-input")?;
    let response = match string(root, "operation").ok_or("invalid-input")? {
        "validate_target" => {
            let kind = string(value, "type").ok_or("invalid-target")?;
            let body = get(value, "target").ok_or("invalid-target")?;
            if !target(kind, body) {
                return Err("invalid-target");
            };
            body.clone()
        }
        "validate_record" => validate_record(value)?,
        "validate_collection" => validate_collection(value)?,
        "create_record" => create_record(value)?,
        "update_record" => update_record(value)?,
        "create_collection" => create_collection(value)?,
        "update_collection" => update_collection(value)?,
        "transition" => transition(value)?,
        "detach" => detach(value)?,
        "list_options" => list_options(value)?,
        "cursor" => cursor(value)?,
        "same_value" => {
            if !exact(value, &["left", "right"], &[]) {
                return Err("invalid-input");
            }
            JsonValue::Bool(
                sorted(get(value, "left").unwrap()) == sorted(get(value, "right").unwrap()),
            )
        }
        "import_plan" => import_plan(value)?,
        "migration_id" => migration_id(value)?,
        "migration_policy" => migration_policy(value)?,
        "migration_record" => migration_record(value)?,
        "migration_skips" => migration_skips(value)?,
        _ => return Err("invalid-input"),
    };
    emit_value_preserved_json(
        &response,
        JsonLimits {
            max_bytes: INPUT_BYTES,
            ..Default::default()
        },
    )
    .map_err(|_| "limit")
}
