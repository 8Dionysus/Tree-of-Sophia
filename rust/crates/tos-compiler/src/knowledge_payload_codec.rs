//! Matched CarrierOnce logical payload codec. All owners are borrowed from the
//! same admitted creation operation; callbacks retain decoded/encoded custody.
//! This module does not activate a Stage mode or manufacture a model ABI.
use crate::{Error, Result, d1_public_capture::CreationState};
use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_json::Value;
use tos_foundation::{Digest256, JsonLimits};

/// Deliver one authentic Stage SQL row while the caller retains its statement,
/// connection and original CreationState. Layout comes from that Stage, never
/// from table discovery. The source packet stays borrowed through consume.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_sql_logical_payload<T>(
    db: &rusqlite::Connection,
    state: &CreationState<'_>,
    layout: crate::knowledge_stage::KnowledgePayloadLayout,
    logical_len: i64,
    logical_sha256: &[u8],
    stored: &[u8],
    codec: i64,
    source_key: Option<&[u8]>,
    max_bytes: usize,
    consume: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    use rusqlite::types::ValueRef;
    let logical_len = usize::try_from(logical_len)
        .map_err(|_| Error::Invalid("normalized SQL logical length"))?;
    if logical_len == 0
        || logical_len > max_bytes
        || stored.is_empty()
        || stored.len() > layout.physical_bound(max_bytes)?
    {
        return Err(Error::Budget("normalized SQL payload bytes"));
    }
    let digest: [u8; 32] = logical_sha256
        .try_into()
        .map_err(|_| Error::Invalid("normalized SQL logical digest"))?;
    let digest = Digest256::from_bytes(digest);
    state.active()?;
    if codec == 0 {
        if source_key.is_some() {
            return Err(Error::Invalid("normalized SQL Inline source key"));
        }
        return layout.with_decoded(state, stored, Some(logical_len), max_bytes, |raw| {
            if charged_digest(state, raw)? != digest {
                return Err(Error::Invalid("normalized SQL Inline receipt"));
            }
            consume(raw)
        });
    }
    if !layout.uses_carriers() || codec != 1 {
        return Err(Error::Invalid("normalized SQL payload codec"));
    }
    let source_key = source_key
        .filter(|key| key.len() == 32)
        .ok_or(Error::Invalid("normalized SQL source key"))?;
    // This admits bounded physical storage. The exact selected decoder below
    // enforces V1 raw length or the V2 frame before logical hash/hydration.
    let sql = c"SELECT packet_len,packet FROM knowledge_source_carriers WHERE packet_sha256=?1 AND typeof(packet_len)='integer' AND packet_len BETWEEN 1 AND ?2 AND typeof(packet)='blob' AND length(packet)<=packet_len+17";
    let _statement_hold = state.hold(
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(),
    )?;
    let sql_error = |error: tos_source_store::StoreError| {
        if error.code == tos_source_store::StoreErrorCode::BudgetExceeded {
            Error::Budget("normalized SQL source budget")
        } else {
            Error::Invalid("normalized SQL source refusal")
        }
    };
    state.charge_work(sql.to_bytes().len() + source_key.len())?;
    let mut statement =
        tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(db, sql)
            .map_err(sql_error)?;
    statement.bind_blob(1, source_key).map_err(sql_error)?;
    statement
        .bind_i64(
            2,
            i64::try_from(max_bytes).map_err(|_| Error::Budget("normalized SQL source cap"))?,
        )
        .map_err(sql_error)?;
    state.active()?;
    if !statement.step().map_err(sql_error)? {
        return Err(Error::Invalid("normalized SQL source absent"));
    }
    let packet_len = usize::try_from(statement.integer(0).map_err(sql_error)?)
        .map_err(|_| Error::Invalid("normalized SQL source length"))?;
    let source = match statement.value_ref(1).map_err(sql_error)? {
        ValueRef::Blob(source) => source,
        _ => return Err(Error::Invalid("normalized SQL source type")),
    };
    let limits = crate::knowledge_normalization::SourceRow::json_limits(max_bytes)?;
    layout.with_decoded(state, source, Some(packet_len), max_bytes, |source| {
        if charged_digest(state, source)?.as_bytes().as_slice() != source_key {
            return Err(Error::Invalid("normalized SQL source receipt"));
        }
        layout.with_decoded(state, stored, None, max_bytes, |stored| {
            with_hydrated_payload(
                state,
                stored,
                source,
                limits,
                limits,
                max_bytes,
                logical_len,
                digest,
                consume,
            )
        })
    })
}

const CODEC: &str = "tos_knowledge_carrier_payload_v1";

// These serializers borrow admitted source subtrees. Repeated logical fields
// still emit their exact bytes and order without materializing duplicate trees.
struct BorrowedAttributes<'a>(&'a [(&'a str, &'a Value)]);
impl Serialize for BorrowedAttributes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}
struct BorrowedSourceRecord<'a> {
    record: &'a serde_json::Map<String, Value>,
    source: &'a Value,
}
impl Serialize for BorrowedSourceRecord<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.record.len()))?;
        for (key, value) in self.record {
            map.serialize_entry(key, if key == "payload" { self.source } else { value })?;
        }
        map.end()
    }
}
struct BorrowedHydratedSpine<'a> {
    spine: &'a serde_json::Map<String, Value>,
    attributes: BorrowedAttributes<'a>,
    source_record: BorrowedSourceRecord<'a>,
}
impl Serialize for BorrowedHydratedSpine<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.spine.len()))?;
        for (key, value) in self.spine {
            match key.as_str() {
                "attributes" => map.serialize_entry(key, &self.attributes)?,
                "source_record" => map.serialize_entry(key, &self.source_record)?,
                _ => map.serialize_entry(key, value)?,
            }
        }
        map.end()
    }
}

#[derive(Serialize)]
struct Stored<'a> {
    codec: &'static str,
    source_len: usize,
    source_sha256: &'a [u8],
    attribute_refs: &'a [(String, String)],
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
    state.charge_work(2 * std::mem::size_of::<usize>())?;
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
    // Splitting the pointer scans its own bytes, not the complete source row.
    state.charge_work(add(pointer.len(), std::mem::size_of::<Value>())?)?;
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
    // Charge each visited pair, including scalar and empty-container cases.
    state.charge_work(2 * std::mem::size_of::<Value>())?;
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
    state.with_serde_owned_value_with_limits(source, source_limits, |source_value| {
        state.with_serde_owned_value_with_limits(normalized, normalized_limits, |logical| {
            if !equal_owned(state, source_payload(&logical)?, &source_value, 0)? {
                return Err(Error::Invalid("carrier codec source value differs"));
            }
            let fields = field_map(&logical)?;
            let attrs = attributes(&logical)?;
            let slots = fields
                .len()
                .checked_mul(std::mem::size_of::<(String, String)>())
                .ok_or(Error::Budget("carrier codec reference slots"))?;
            // Own the small references so the admitted decoded row can become
            // the spine in place, without cloning its full source payload.
            let strings = fields.iter().try_fold(0usize, |bytes, (name, pointer)| {
                state.charge_work(name.len())?;
                let Some(key) = name.strip_prefix("attributes.") else {
                    return Ok(bytes);
                };
                let pointer = pointer
                    .as_str()
                    .ok_or(Error::Invalid("carrier codec field pointer type"))?;
                state.charge_work(pointer.len())?;
                add(bytes, add(key.len(), pointer.len())?)
            })?;
            let refs_hold = state.hold(add(slots, strings)?)?;
            let mut refs = Vec::with_capacity(fields.len());
            for (name, pointer) in fields {
                state.active()?;
                let Some(key) = name.strip_prefix("attributes.") else {
                    continue;
                };
                let pointer = pointer
                    .as_str()
                    .ok_or(Error::Invalid("carrier codec field pointer type"))?;
                state.charge_work(name.len())?;
                if let (Some(actual), Some(original)) = (
                    attrs.get(key),
                    pointer_owned(state, &source_value, pointer)?,
                ) {
                    if equal_owned(state, actual, original, 0)? {
                        state.charge_work(add(key.len(), pointer.len())?)?;
                        refs.push((key.to_owned(), pointer.to_owned()));
                    }
                }
            }
            let mut spine = logical;
            let result = (|| {
                for (key, _) in &refs {
                    state.active()?;
                    *spine
                        .get_mut("attributes")
                        .and_then(Value::as_object_mut)
                        .and_then(|v| v.get_mut(key.as_str()))
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
                    with_hydrated_payload_from_source(
                        state,
                        encoded,
                        source.len(),
                        source_digest,
                        source_value,
                        stored_limits,
                        max_row_bytes,
                        normalized.len(),
                        normalized_digest,
                        |hydrated| {
                            if !bytes_equal(state, hydrated, normalized)? {
                                return Err(Error::Invalid("carrier codec logical bytes changed"));
                            }
                            state.active()?;
                            consume(encoded, source_digest)
                        },
                    )
                })
            })();
            drop(spine);
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
    state.with_serde_owned_value_with_limits(source, source_limits, |source_value| {
        with_hydrated_payload_from_source(
            state,
            stored,
            source.len(),
            source_digest,
            source_value,
            stored_limits,
            max_row_bytes,
            logical_len,
            logical_digest,
            consume,
        )
    })
}

// The caller retains the admitted decode owner through this callback. Factoring
// can move that same exact source tree into its roundtrip check instead of
// hashing, grammar-parsing and decoding it a second time. The stored envelope
// is still parsed and the reconstructed logical bytes are still authenticated.
#[allow(clippy::too_many_arguments)]
fn with_hydrated_payload_from_source<T>(
    state: &CreationState<'_>,
    stored: &[u8],
    source_len: usize,
    source_digest: Digest256,
    source_value: Value,
    stored_limits: JsonLimits,
    max_row_bytes: usize,
    logical_len: usize,
    logical_digest: Digest256,
    consume: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    state.active()?;
    state.with_serde_owned_value_with_limits(stored, stored_limits, |physical| {
        let map = physical
            .as_object()
            .filter(|v| v.len() == 5)
            .ok_or(Error::Invalid("carrier codec envelope fields"))?;
        if map.get("codec").and_then(Value::as_str) != Some(CODEC)
            || map.get("source_len").and_then(Value::as_u64) != Some(source_len as u64)
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
        let slots = attrs
            .len()
            .checked_mul(std::mem::size_of::<(&str, &Value)>())
            .ok_or(Error::Budget("carrier codec borrowed attribute slots"))?;
        let attributes_hold = state.hold(slots)?;
        let mut selected_attributes = Vec::with_capacity(attrs.len());
        for (key, stored_value) in attrs {
            state.active()?;
            let mut selected = stored_value;
            for reference in refs {
                let (ref_key, pointer) = ref_pair(reference)?;
                if bytes_equal(state, key.as_bytes(), ref_key.as_bytes())? {
                    selected = pointer_owned(state, &source_value, pointer)?
                        .ok_or(Error::Invalid("carrier codec source pointer absent"))?;
                    break;
                }
            }
            selected_attributes.push((key.as_str(), selected));
        }
        let rebuilt = BorrowedHydratedSpine {
            spine: spine
                .as_object()
                .ok_or(Error::Invalid("carrier codec spine object"))?,
            attributes: BorrowedAttributes(&selected_attributes),
            source_record: BorrowedSourceRecord {
                record: spine
                    .get("source_record")
                    .and_then(Value::as_object)
                    .ok_or(Error::Invalid("carrier codec source record object"))?,
                source: &source_value,
            },
        };
        let result = state.with_json_encoded(&rebuilt, max_row_bytes, |logical| {
            if logical.len() != logical_len || charged_digest(state, logical)? != logical_digest {
                return Err(Error::Invalid(
                    "carrier codec logical length or digest differs",
                ));
            }
            state.active()?;
            consume(logical)
        });
        drop(selected_attributes);
        drop(attributes_hold);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn borrowed_hydration_preserves_exact_nested_order_numbers_and_utf8() {
        let source: Value = serde_json::from_str(r#"{"label":"声\nточные bytes","n":18446744073709551617001,"nested":{"z":[1.2500,null,true],"a":"é"}}"#).unwrap();
        let physical: Value = serde_json::from_str(r#"{"z":1,"source_record":{"payload":null,"field_map":{"attributes.a":"/nested","attributes.b":"/label"},"tail":false},"attributes":{"untouched":[2,1],"a":null,"b":null},"a":2}"#).unwrap();
        let attrs = physical["attributes"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, item)| {
                (
                    key.as_str(),
                    match key.as_str() {
                        "a" => &source["nested"],
                        "b" => &source["label"],
                        _ => item,
                    },
                )
            })
            .collect::<Vec<_>>();
        let borrowed = BorrowedHydratedSpine {
            spine: physical.as_object().unwrap(),
            attributes: BorrowedAttributes(&attrs),
            source_record: BorrowedSourceRecord {
                record: physical["source_record"].as_object().unwrap(),
                source: &source,
            },
        };
        let mut owned = physical.clone();
        owned["source_record"]["payload"] = source.clone();
        owned["attributes"]["a"] = source["nested"].clone();
        owned["attributes"]["b"] = source["label"].clone();
        assert_eq!(
            serde_json::to_vec(&borrowed).unwrap(),
            serde_json::to_vec(&owned).unwrap()
        );
        assert!(physical["attributes"]["a"].is_null());
        assert!(physical["source_record"]["payload"].is_null());
    }
}
