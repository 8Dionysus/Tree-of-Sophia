//! Matched CarrierOnce logical payload codec. All owners are borrowed from the
//! same admitted creation operation; callbacks retain decoded/encoded custody.
//! This module does not activate a Stage mode or manufacture a model ABI.
use crate::{Error, Result, d1_public_capture::CreationState};
use serde::Serialize;
use serde_json::Value;
use tos_foundation::{Digest256, JsonLimits};

const CODEC: &str = "tos_knowledge_carrier_payload_v1";

#[derive(Serialize)]
struct Stored<'a> {
    codec: &'static str,
    source_len: usize,
    source_sha256: &'a [u8],
    attribute_refs: &'a [(&'a str, &'a str)],
    spine: &'a Value,
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or(Error::Budget("carrier codec state overflow"))
}
fn cap(raw: &[u8], max: usize) -> Result<()> {
    if max == 0 || raw.is_empty() || raw.len() > max {
        return Err(Error::Budget("carrier codec input bytes"));
    }
    Ok(())
}
fn charged_digest(state: &CreationState<'_>, raw: &[u8]) -> Result<Digest256> {
    state.charge_work(raw.len())?;
    let digest = Digest256::of_bytes(raw);
    state.active()?;
    Ok(digest)
}
fn source_payload(value: &Value) -> Result<&Value> {
    value
        .get("source_record")
        .and_then(|v| v.get("payload"))
        .ok_or(Error::Invalid("carrier codec source payload absent"))
}
fn field_map(value: &Value) -> Result<&serde_json::Map<String, Value>> {
    value
        .get("source_record")
        .and_then(|v| v.get("field_map"))
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("carrier codec field map absent"))
}
fn attributes(value: &Value) -> Result<&serde_json::Map<String, Value>> {
    value
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("carrier codec attributes absent"))
}

// Resolve RFC6901 tokens against borrowed keys without allocating decoded
// token Strings; check/charge through traversal, including individual tokens.
fn bytes_equal(state: &CreationState<'_>, left: &[u8], right: &[u8]) -> Result<bool> {
    if left.len() != right.len() {
        state.active()?;
        return Ok(false);
    }
    for (a, b) in left.chunks(4096).zip(right.chunks(4096)) {
        state.charge_work(
            a.len()
                .checked_mul(2)
                .ok_or(Error::Budget("carrier equality work"))?,
        )?;
        if a != b {
            return Ok(false);
        }
    }
    state.active()?;
    Ok(true)
}
fn token_equal(state: &CreationState<'_>, token: &str, key: &str) -> Result<bool> {
    let raw = token.as_bytes();
    let key = key.as_bytes();
    let mut at = 0;
    let mut out = 0;
    while at < raw.len() {
        state.charge_work(1)?;
        let byte = if raw[at] == b'~' && at + 1 < raw.len() && matches!(raw[at + 1], b'0' | b'1') {
            let byte = if raw[at + 1] == b'0' { b'~' } else { b'/' };
            at += 2;
            byte
        } else {
            let byte = raw[at];
            at += 1;
            byte
        };
        if key.get(out) != Some(&byte) {
            return Ok(false);
        }
        out += 1;
    }
    state.active()?;
    Ok(out == key.len())
}
fn pointer_owned<'v>(
    state: &CreationState<'_>,
    mut value: &'v Value,
    pointer: &str,
) -> Result<Option<&'v Value>> {
    state.active()?;
    if pointer.is_empty() {
        return Ok(Some(value));
    }
    let Some(rest) = pointer.strip_prefix('/') else {
        return Ok(None);
    };
    for token in rest.split('/') {
        state.active()?;
        match value {
            Value::Object(fields) => {
                let mut found = None;
                for (key, item) in fields {
                    if token_equal(state, token, key)? {
                        found = Some(item);
                        break;
                    }
                }
                let Some(next) = found else {
                    return Ok(None);
                };
                value = next;
            }
            Value::Array(items) => {
                state.charge_work(token.len())?;
                if token.starts_with('+') || (token.starts_with('0') && token.len() != 1) {
                    return Ok(None);
                }
                let Ok(index) = token.parse::<usize>() else {
                    return Ok(None);
                };
                let Some(next) = items.get(index) else {
                    return Ok(None);
                };
                value = next;
            }
            _ => return Ok(None),
        }
    }
    state.active()?;
    Ok(Some(value))
}
fn equal_owned(
    state: &CreationState<'_>,
    left: &Value,
    right: &Value,
    depth: usize,
) -> Result<bool> {
    state.active()?;
    // Account simultaneously live recursive arguments, iterator/match locals
    // and the RAII hold itself before descending into the typed tree.
    let frame_bytes = std::mem::size_of::<(&CreationState<'_>, &Value, &Value, usize)>()
        .checked_add(2 * std::mem::size_of::<serde_json::map::Iter<'_>>())
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<std::slice::Iter<'_, Value>>()))
        .and_then(|n| n.checked_add(4 * std::mem::size_of::<Option<(&String, &Value)>>()))
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<
                crate::d1_public_capture::CreationStateHold<'_, '_>,
            >())
        })
        .and_then(|n| n.checked_add(std::mem::size_of::<Result<bool>>()))
        .ok_or(Error::Budget("carrier equality frame state"))?;
    let _frame_hold = state.hold(frame_bytes)?;
    if depth > 96 {
        return Err(Error::Budget("carrier equality depth"));
    }
    Ok(match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => {
            bytes_equal(state, a.as_str().as_bytes(), b.as_str().as_bytes())?
        }
        (Value::String(a), Value::String(b)) => bytes_equal(state, a.as_bytes(), b.as_bytes())?,
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (x, y) in a.iter().zip(b) {
                if !equal_owned(state, x, y, depth + 1)? {
                    return Ok(false);
                }
            }
            true
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (key, x) in a {
                // preserve_order maps are not sorted; find the same key by
                // explicitly checked byte comparisons instead of opaque get.
                let mut found = None;
                for (other_key, other_value) in b {
                    if bytes_equal(state, key.as_bytes(), other_key.as_bytes())? {
                        found = Some(other_value);
                        break;
                    }
                }
                let Some(y) = found else {
                    return Ok(false);
                };
                if !equal_owned(state, x, y, depth + 1)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => false,
    })
}

/// Factor only byte-roundtrippable normalized fields. Unmatched/derived fields
/// remain inline. The physical envelope and its full rehydrated logical bytes
/// stay held until the synchronous consumer returns. Returned owned output
/// requires its caller's independent admission, as for all CreationState scopes.
pub(crate) fn with_factored_payload<T>(
    state: &CreationState<'_>,
    normalized: &[u8],
    source: &[u8],
    normalized_limits: JsonLimits,
    source_limits: JsonLimits,
    stored_limits: JsonLimits,
    max_row_bytes: usize,
    consume: impl FnOnce(&[u8], Digest256) -> Result<T>,
) -> Result<T> {
    state.active()?;
    cap(normalized, max_row_bytes)?;
    cap(source, max_row_bytes)?;
    let source_digest = charged_digest(state, source)?;
    let normalized_digest = charged_digest(state, normalized)?;
    state.with_serde_owned_with_limits(source, source_limits, |source_value| {
        state.with_serde_owned_with_limits(normalized, normalized_limits, |logical| {
            // The equality traversal is distinct from hashing/parsing work.
            state.charge_work(add(source.len(), normalized.len())?)?;
            if !equal_owned(state, source_payload(logical)?, source_value, 0)? {
                return Err(Error::Invalid("carrier codec source value differs"));
            }
            let fields = field_map(logical)?;
            let attrs = attributes(logical)?;
            let slots = fields
                .len()
                .checked_mul(std::mem::size_of::<(&str, &str)>())
                .ok_or(Error::Budget("carrier codec reference slots"))?;
            let refs_hold = state.hold(slots)?;
            let mut refs = Vec::with_capacity(fields.len());
            for (name, pointer) in fields {
                state.active()?;
                let Some(key) = name.strip_prefix("attributes.") else {
                    continue;
                };
                let pointer = pointer
                    .as_str()
                    .ok_or(Error::Invalid("carrier codec field pointer type"))?;
                state.charge_work(add(add(pointer.len(), source.len())?, normalized.len())?)?;
                if let (Some(actual), Some(original)) =
                    (attrs.get(key), pointer_owned(state, source_value, pointer)?)
                {
                    if equal_owned(state, actual, original, 0)? {
                        refs.push((key, pointer));
                    }
                }
            }
            let result = state.with_clone_value(logical, |mut spine| {
                for (key, _) in &refs {
                    state.active()?;
                    *spine
                        .get_mut("attributes")
                        .and_then(Value::as_object_mut)
                        .and_then(|v| v.get_mut(*key))
                        .ok_or(Error::Invalid("carrier codec attribute slot absent"))? =
                        Value::Null;
                }
                *spine
                    .get_mut("source_record")
                    .and_then(|v| v.get_mut("payload"))
                    .ok_or(Error::Invalid("carrier codec source slot absent"))? = Value::Null;
                let physical = Stored {
                    codec: CODEC,
                    source_len: source.len(),
                    source_sha256: source_digest.as_bytes(),
                    attribute_refs: &refs,
                    spine: &spine,
                };
                state.with_json_encoded(&physical, max_row_bytes, |encoded| {
                    // Verify exact spelling/order/numbers, not just equal JSON.
                    with_hydrated_payload(
                        state,
                        encoded,
                        source,
                        stored_limits,
                        source_limits,
                        max_row_bytes,
                        normalized.len(),
                        normalized_digest,
                        |hydrated| {
                            state.charge_work(normalized.len())?;
                            if !bytes_equal(state, hydrated, normalized)? {
                                return Err(Error::Invalid("carrier codec logical bytes changed"));
                            }
                            state.active()?;
                            consume(encoded, source_digest)
                        },
                    )
                })
            });
            drop(refs);
            drop(refs_hold);
            result
        })
    })
}

fn ref_pair(value: &Value) -> Result<(&str, &str)> {
    let pair = value
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or(Error::Invalid("carrier codec reference tuple"))?;
    Ok((
        pair[0]
            .as_str()
            .ok_or(Error::Invalid("carrier codec reference key"))?,
        pair[1]
            .as_str()
            .ok_or(Error::Invalid("carrier codec reference pointer"))?,
    ))
}

/// Resolve an explicit physical codec under the same owner and check the
/// existing logical payload length/digest before any SQL/output callback.
/// The caller supplies the carrier through its verified SHA/length lookup;
/// physical table presence never selects this codec or authenticates a source.
pub(crate) fn with_hydrated_payload<T>(
    state: &CreationState<'_>,
    stored: &[u8],
    source: &[u8],
    stored_limits: JsonLimits,
    source_limits: JsonLimits,
    max_row_bytes: usize,
    logical_len: usize,
    logical_digest: Digest256,
    consume: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    state.active()?;
    cap(stored, max_row_bytes)?;
    cap(source, max_row_bytes)?;
    if logical_len == 0 || logical_len > max_row_bytes {
        return Err(Error::Budget("carrier codec logical bytes"));
    }
    let source_digest = charged_digest(state, source)?;
    state.with_serde_owned_with_limits(stored, stored_limits, |physical| {
        let map = physical
            .as_object()
            .filter(|v| v.len() == 5)
            .ok_or(Error::Invalid("carrier codec envelope fields"))?;
        if map.get("codec").and_then(Value::as_str) != Some(CODEC)
            || map.get("source_len").and_then(Value::as_u64) != Some(source.len() as u64)
        {
            return Err(Error::Invalid("carrier codec identity differs"));
        }
        let digest = map
            .get("source_sha256")
            .and_then(Value::as_array)
            .filter(|v| v.len() == 32)
            .ok_or(Error::Invalid("carrier codec digest shape"))?;
        for (actual, expected) in digest.iter().zip(source_digest.as_bytes()) {
            state.active()?;
            if actual.as_u64() != Some(*expected as u64) {
                return Err(Error::Invalid("carrier codec exact source digest differs"));
            }
        }
        let refs = map
            .get("attribute_refs")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("carrier codec references absent"))?;
        let spine = map
            .get("spine")
            .ok_or(Error::Invalid("carrier codec spine absent"))?;
        if !source_payload(spine)?.is_null() {
            return Err(Error::Invalid("carrier codec source slot not null"));
        }
        let fields = field_map(spine)?;
        let attrs = attributes(spine)?;
        // Bound duplicate detection without allocating an unpriced set. The
        // same original work charges each pairwise scan before it executes.
        for (at, reference) in refs.iter().enumerate() {
            state.active()?;
            let (key, pointer) = ref_pair(reference)?;
            if !attrs.get(key).is_some_and(Value::is_null) {
                return Err(Error::Invalid("carrier codec referenced slot not null"));
            }
            state.charge_work(add(
                fields
                    .len()
                    .checked_mul(add(key.len(), pointer.len())?)
                    .ok_or(Error::Budget("carrier codec pointer work"))?,
                at.checked_mul(key.len())
                    .ok_or(Error::Budget("carrier codec duplicate work"))?,
            )?)?;
            let mut matched = false;
            for (field, value) in fields {
                state.active()?;
                if let (Some(name), Some(actual_pointer)) =
                    (field.strip_prefix("attributes."), value.as_str())
                {
                    if bytes_equal(state, name.as_bytes(), key.as_bytes())?
                        && bytes_equal(state, actual_pointer.as_bytes(), pointer.as_bytes())?
                    {
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                return Err(Error::Invalid("carrier codec reference not in field map"));
            }
            for earlier in &refs[..at] {
                if bytes_equal(state, ref_pair(earlier)?.0.as_bytes(), key.as_bytes())? {
                    return Err(Error::Invalid("carrier codec duplicate reference"));
                }
            }
        }
        state.with_serde_owned_with_limits(source, source_limits, |source_value| {
            // Admit all added clone geometry before any subtree is copied;
            // these owners remain live until rebuilt bytes and callback close.
            let mut extra = state.value_clone_state_upper_bound(source_value)?;
            for reference in refs {
                let (_, pointer) = ref_pair(reference)?;
                state.charge_work(add(pointer.len(), source.len())?)?;
                let item = pointer_owned(state, source_value, pointer)?
                    .ok_or(Error::Invalid("carrier codec source pointer absent"))?;
                extra = add(extra, state.value_clone_state_upper_bound(item)?)?;
            }
            let extra_hold = state.hold(extra)?;
            let result = state.with_clone_value(spine, |mut rebuilt| {
                state.charge_work(extra)?;
                state.with_clone_value(source_value, |copy| {
                    *rebuilt
                        .get_mut("source_record")
                        .and_then(|v| v.get_mut("payload"))
                        .ok_or(Error::Invalid("carrier codec source slot absent"))? = copy;
                    Ok(())
                })?;
                for reference in refs {
                    state.active()?;
                    let (key, pointer) = ref_pair(reference)?;
                    state.charge_work(add(pointer.len(), source.len())?)?;
                    let item = pointer_owned(state, source_value, pointer)?
                        .ok_or(Error::Invalid("carrier codec source pointer absent"))?;
                    state.with_clone_value(item, |copy| {
                        *rebuilt
                            .get_mut("attributes")
                            .and_then(Value::as_object_mut)
                            .and_then(|v| v.get_mut(key))
                            .ok_or(Error::Invalid("carrier codec attribute slot absent"))? = copy;
                        Ok(())
                    })?;
                }
                let result = state.with_json_encoded(&rebuilt, max_row_bytes, |logical| {
                    if logical.len() != logical_len
                        || charged_digest(state, logical)? != logical_digest
                    {
                        return Err(Error::Invalid(
                            "carrier codec logical length or digest differs",
                        ));
                    }
                    state.active()?;
                    consume(logical)
                });
                drop(rebuilt);
                result
            });
            drop(extra_hold);
            result
        })
    })
}
