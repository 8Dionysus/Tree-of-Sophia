//! The browser's bounded, wording-free Claim path pointer. Saved selectors
//! cannot stand in for the source owner's current path or admission judgment.

use std::collections::HashSet;
use tos_foundation::{
    emit_value_preserved_json, parse_json, JsonLimits, JsonMode, JsonString, JsonValue,
};

// 127 valid 2,048-unit IDs can each require six ASCII JSON escape bytes per
// unit (control characters or lone surrogates). Two MiB admits that worst case.
const MAX_BYTES: usize = 2_000_000;

fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value.object_get(name)
}
fn opaque(value: &JsonValue) -> Option<&JsonString> {
    match value {
        JsonValue::String(word) if (1..=2048).contains(&word.units().len()) => Some(word),
        _ => None,
    }
}
fn id_list(value: &JsonValue, maximum: usize) -> Option<&[JsonValue]> {
    let items = value.as_array()?;
    if items.len() > maximum || items.iter().any(|item| opaque(item).is_none()) {
        return None;
    }
    Some(items)
}
fn same(left: &JsonValue, right: &JsonValue) -> bool {
    opaque(left)
        .zip(opaque(right))
        .is_some_and(|(a, b)| a.units() == b.units())
}
fn distinct(items: impl Iterator<Item = Vec<u16>>) -> bool {
    let mut seen = HashSet::new();
    items.into_iter().all(|item| seen.insert(item))
}
fn units(value: &JsonValue) -> Vec<u16> {
    opaque(value).expect("validated opaque ID").units().to_vec()
}

/// Reused by durable reading after its own packet has been parsed. The source
/// owner still has to revalidate the current path before serving any wording.
pub(crate) fn normalize_claim_reference_value(
    reference: &JsonValue,
    expected: &JsonValue,
) -> Result<JsonValue, &'static str> {
    let claim = field(reference, "claimId").ok_or("invalid_claim_reference")?;
    let path = field(reference, "pathId").ok_or("invalid_claim_reference")?;
    let relation = field(reference, "relationType").ok_or("invalid_claim_reference")?;
    if !same(claim, expected) || opaque(path).is_none() || opaque(relation).is_none() {
        return Err("invalid_claim_reference");
    }
    let nodes = id_list(
        field(reference, "nodeIds").ok_or("invalid_claim_reference")?,
        3,
    )
    .ok_or("invalid_claim_reference")?;
    let relations = id_list(
        field(reference, "relationIds").ok_or("invalid_claim_reference")?,
        2,
    )
    .ok_or("invalid_claim_reference")?;
    let details = id_list(
        field(reference, "detailRelationIds").ok_or("invalid_claim_reference")?,
        78,
    )
    .ok_or("invalid_claim_reference")?;
    let closure = id_list(
        field(reference, "closureNodeIds").ok_or("invalid_claim_reference")?,
        40,
    )
    .ok_or("invalid_claim_reference")?;
    if nodes.len() != 3
        || !same(&nodes[1], claim)
        || relations.len() != 2
        || closure.is_empty()
        || !distinct(closure.iter().map(units))
        || nodes
            .iter()
            .any(|node| !closure.iter().any(|candidate| same(node, candidate)))
        || !distinct(relations.iter().chain(details).map(units))
    {
        return Err("invalid_claim_reference");
    }
    Ok(JsonValue::Object(
        [
            "claimId",
            "pathId",
            "relationType",
            "nodeIds",
            "relationIds",
            "detailRelationIds",
            "closureNodeIds",
        ]
        .into_iter()
        .map(|key| {
            (
                JsonString::from_utf8(key),
                field(reference, key).expect("validated field").clone(),
            )
        })
        .collect(),
    ))
}

/// Validate the exact seven-field Claim pointer and discard unknown carrier
/// fields, mirroring the maintained browser projection. IDs are UTF-16 opaque.
pub fn validate_claim_reference_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_claim_reference")?;
    let request = document.root();
    let expected = field(request, "claim_id").ok_or("invalid_claim_reference")?;
    let reference = field(request, "reference").ok_or("invalid_claim_reference")?;
    let output = normalize_claim_reference_value(reference, expected)?;
    emit_value_preserved_json(
        &output,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_claim_reference")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_surrogates_and_closure_identity() {
        let raw = br#"{"claim_id":"a\ud800","reference":{"claimId":"a\ud800","pathId":"p","relationType":"r","nodeIds":["x","a\ud800","z"],"relationIds":["r1","r2"],"detailRelationIds":[],"closureNodeIds":["x","a\ud800","z"],"wording":"discard"}}"#;
        let output = validate_claim_reference_v1(raw).unwrap();
        assert!(!String::from_utf8(output.clone())
            .unwrap()
            .contains("wording"));
        let bad = String::from_utf8(raw.to_vec())
            .unwrap()
            .replace("\"r2\"", "\"r1\"");
        assert_eq!(
            validate_claim_reference_v1(bad.as_bytes()),
            Err("invalid_claim_reference")
        );
    }
}
