//! Exact source targets projected from retained normalized envelopes only.
//! These are navigation targets, never source handles or current admission.
use std::collections::BTreeMap;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonNumberKind, JsonString, JsonValue,
    canonical_bytes_v1,
};

pub(crate) fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
pub(crate) fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value.object_get(key)?.as_str()
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn integer(value: &JsonValue) -> Option<u64> {
    let JsonValue::Number(number) = value else {
        return None;
    };
    if number.kind != JsonNumberKind::Int {
        return None;
    }
    let n = number.lexeme.parse::<u64>().ok()?;
    (n > 0 && n <= 9_007_199_254_740_991).then_some(n)
}
fn valid_id(id: &str, claim: bool) -> bool {
    let Some(body) = id.strip_prefix(if claim { "tos.claim." } else { "tos." }) else {
        return false;
    };
    if !claim && body.starts_with("claim.") {
        return false;
    }
    !body.is_empty()
        && body.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
        })
}
fn record_target(record: &JsonValue, claim: bool, limits: JsonLimits, canonical: &mut dyn FnMut(&JsonValue, JsonLimits) -> Option<Vec<u8>>) -> Option<JsonValue> {
    record.as_object()?;
    let (kind, field) = if claim {
        (None, "claim_id")
    } else {
        match get(record, "schema_version") {
            Some("tos_scholarly_composite_witness_v1") => (Some("composite"), "composite_id"),
            Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => {
                (Some("artifact"), "artifact_id")
            }
            _ => (Some(get(record, "record_type")?), "record_id"),
        }
    };
    if !claim
        && field != "record_id"
        && (record.object_get("record_id").is_some() || record.object_get("record_type").is_some())
    {
        return None;
    }
    let id = get(record, field)?;
    if !valid_id(id, claim) {
        return None;
    }
    if let Some(kind) = kind {
        if kind.is_empty()
            || kind.len() > 64
            || !kind.as_bytes()[0].is_ascii_lowercase()
            || !kind
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return None;
        }
        if field != "record_id" && !id.starts_with(&format!("tos.{kind}.")) {
            return None;
        }
    }
    let version = record.object_get(if claim {
        "claim_version"
    } else {
        "record_version"
    })?;
    integer(version)?;
    let sha = format!(
        "sha256:{}",
        Digest256::of_bytes(
            &canonical(record, limits)?
        )
        .to_hex()
    );
    let mut fields = vec![
        (
            "layer",
            text(if claim {
                "claim_record"
            } else {
                "metadata_record"
            }),
        ),
        (
            "record_ref",
            object(vec![
                ("id", text(id)),
                ("version", version.clone()),
                ("digest", text(&sha)),
            ]),
        ),
        ("content_revision", text(&sha)),
    ];
    if let Some(kind) = kind {
        fields.insert(1, ("record_type", text(kind)));
    }
    Some(object(fields))
}
fn target(item: &JsonValue, limits: JsonLimits, canonical: &mut dyn FnMut(&JsonValue, JsonLimits) -> Option<Vec<u8>>) -> Option<JsonValue> {
    let envelope = item.object_get("source_record")?;
    let fields = envelope.as_object()?;
    if fields.len() != 4
        || !["payload", "digest", "transform_version", "field_map"]
            .iter()
            .all(|key| envelope.object_get(key).is_some())
        || !digest(get(envelope, "digest")?)
        || get(envelope, "transform_version") != Some("tos-knowledge-normalization-v2")
        || !envelope
            .object_get("field_map")?
            .as_object()?
            .iter()
            .all(|(key, value)| key.as_str().is_some() && value.as_str().is_some())
    {
        return None;
    }
    let payload = envelope.object_get("payload")?;
    payload.as_object()?;
    let properties = payload.object_get("properties")?;
    properties.as_object()?;
    let metadata = properties
        .object_get("source_record")
        .filter(|value| !matches!(value, JsonValue::Null));
    let claim = properties
        .object_get("source_claim")
        .filter(|value| !matches!(value, JsonValue::Null));
    match (metadata, claim) {
        (None, Some(record)) => record_target(record, true, limits, canonical),
        (Some(record), None) => {
            if payload.object_get("pack_id").is_some() || payload.object_get("edge_id").is_some() {
                let pack = get(payload, "pack_id")?;
                let edge = get(payload, "edge_id")?;
                if pack.len() > 2048
                    || !(pack.starts_with("canon/relations/")
                        || pack.starts_with("candidate-intake/"))
                    || pack.contains(['\\', '\0'])
                    || pack
                        .split('/')
                        .any(|p| p.is_empty() || p.starts_with('.') || p == "payload")
                    || edge.is_empty()
                    || edge.len() > 2048
                    || edge.contains('\0')
                {
                    return None;
                }
                if !record.as_object()?.iter().all(|(key, value)| {
                    key.as_str().is_some()
                        && (value.as_str().is_some() || matches!(value, JsonValue::Null))
                }) {
                    return None;
                }
                let row = properties.object_get("source_row")?;
                integer(row)?;
                let file_sha = get(properties, "source_file_sha256")?;
                if !digest(file_sha) {
                    return None;
                }
                let sha = format!(
                    "sha256:{}",
                    Digest256::of_bytes(
                        &canonical(record, limits)?
                    )
                    .to_hex()
                );
                Some(object(vec![
                    ("layer", text("authored_csv_record")),
                    ("pack_id", text(pack)),
                    ("edge_id", text(edge)),
                    ("source_row", row.clone()),
                    ("source_file_sha256", text(file_sha)),
                    ("content_revision", text(&sha)),
                ]))
            } else {
                record_target(record, false, limits, canonical)
            }
        }
        _ => None,
    }
}
pub(crate) fn source_read_targets(
    items: &[JsonValue],
    revision: &str,
    limits: JsonLimits,
) -> JsonValue {
    if !digest(revision) {
        return object(vec![]);
    }
    project_targets(items, "source_revision", revision, limits)
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn managed_source_read_targets(
    items: &[JsonValue],
    root: &str,
    limits: JsonLimits,
) -> JsonValue {
    project_targets(items, "managed_source_root_sha256", root, limits)
}
fn project_targets(
    items: &[JsonValue],
    identity_field: &str,
    identity: &str,
    limits: JsonLimits,
) -> JsonValue {
    let mut canonical = |record: &JsonValue, limits| canonical_bytes_v1(record,
        CanonicalProfile::SourceRecordDigestV1, limits).ok();
    project_targets_with_canonical(items, identity_field, identity, limits, &mut canonical)
}

/// Controlled callers supply the original-state canonicalizer. A refusal is
/// returned after projection traversal; it can never become a missing target.
pub(crate) fn controlled_source_read_targets(
    items: &[JsonValue], identity: &str, managed: bool, limits: JsonLimits,
    mut canonical: impl FnMut(&JsonValue, JsonLimits) -> Result<Vec<u8>, crate::search_v2::SearchV2Error>,
) -> Result<JsonValue, crate::search_v2::SearchV2Error> {
    let mut refused = None;
    let mut checked = |record: &JsonValue, limits| {
        if refused.is_some() { return None; }
        match canonical(record, limits) {
        Ok(raw) => Some(raw),
        Err(error) => { refused = Some(error); None }
        }
    };
    let projected = project_targets_with_canonical(items,
        if managed { "managed_source_root_sha256" } else { "source_revision" },
        identity, limits, &mut checked);
    match refused { Some(error) => Err(error), None => Ok(projected) }
}

fn project_targets_with_canonical(
    items: &[JsonValue],
    identity_field: &str,
    identity: &str,
    limits: JsonLimits,
    canonical: &mut dyn FnMut(&JsonValue, JsonLimits) -> Option<Vec<u8>>,
) -> JsonValue {
    let mut seen: BTreeMap<String, (Option<JsonValue>, bool)> = BTreeMap::new();
    for item in items {
        let Some(id) = get(item, "id").filter(|id| !id.is_empty()) else {
            continue;
        };
        let value = target(item, limits, canonical);
        seen.entry(id.to_owned())
            .and_modify(|(old, conflict)| *conflict |= *old != value)
            .or_insert((value, false));
    }
    JsonValue::Object(
        seen.into_iter()
            .filter_map(|(id, (target, conflict))| {
                if conflict {
                    return None;
                }
                Some((
                    JsonString::from_utf8(&id),
                    object(vec![(identity_field, text(identity)), ("target", target?)]),
                ))
            })
            .collect(),
    )
}
