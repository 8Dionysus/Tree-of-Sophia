//! The public graph's semantic transport check over the final normalized
//! rows. The registries and indexed Stage own the bounded lookups; no graph
//! carrier or duplicate endpoint map is retained in process memory.

use crate::{
    Error, KnowledgeRegistry, Result,
    d1_public_capture::{
        CreationState, CreationStateHold, MAX_ROW_BYTES, PublicCapture,
        json as strict_json,
    },
    knowledge_stage::{KnowledgePayloadLayout, KnowledgeStage, WritePhase},
};
use rusqlite::{OptionalExtension, Row, Statement, params, types::ValueRef};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256;

fn field<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |value, part| value.get(part))
}
fn string<'a>(value: &'a Value, path: &str) -> Option<&'a str> {
    field(value, path)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}
fn strings<'a>(value: &'a Value, path: &str) -> impl Iterator<Item = &'a str> {
    field(value, path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|text| !text.is_empty())
}
fn member<'a>(value: &'a Value, path: &str) -> &'a Value {
    field(value, path).unwrap_or(&Value::Null)
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}
fn admitted_row_len(capture: &PublicCapture, len: i64) -> Result<usize> {
    if len < 0 {
        return Err(Error::Invalid("public D1 semantic row length"));
    }
    let length = usize::try_from(len).map_err(|_| Error::Budget("public D1 semantic row bytes"))?;
    if length > MAX_ROW_BYTES {
        return Err(Error::Budget("public D1 semantic row bytes"));
    }
    // Reserve the selected copy, digest and two decoders before row.get
    // materializes the BLOB. The Stage CASE returns NULL for bad lengths.
    capture.charge_work(
        (length as u64)
            .checked_mul(4)
            .ok_or(Error::Budget("public D1 semantic row work"))?,
    )?;
    Ok(length)
}
fn check_row(len: usize, digest: &[u8], raw: Option<Vec<u8>>) -> Result<Value> {
    let raw = raw.ok_or(Error::Invalid("public D1 semantic row length"))?;
    if raw.len() > MAX_ROW_BYTES
        || len != raw.len()
        || digest != Digest256::of_bytes(&raw).as_bytes()
    {
        return Err(Error::Invalid("public D1 semantic row digest"));
    }
    strict_json(&raw, MAX_ROW_BYTES)?;
    serde_json::from_slice(&raw).map_err(|error| Error::Source(error.to_string()))
}
fn digest_matches_hex(digest: &Digest256, expected: &str) -> bool {
    if expected.len() != 64 {
        return false;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    expected
        .as_bytes()
        .chunks_exact(2)
        .zip(digest.as_bytes())
        .all(|(pair, byte)| {
            pair[0] == HEX[(*byte >> 4) as usize] && pair[1] == HEX[(*byte & 0x0f) as usize]
        })
}
fn sql_blob_ref<'row>(row: &'row Row<'_>, column: usize) -> Result<&'row [u8]> {
    match row.get_ref(column)? {
        ValueRef::Blob(bytes) => Ok(bytes),
        _ => Err(Error::Invalid("public D1 semantic SQL blob")),
    }
}
fn sql_text_ref<'row>(row: &'row Row<'_>, column: usize) -> Result<&'row str> {
    match row.get_ref(column)? {
        ValueRef::Text(bytes) => {
            std::str::from_utf8(bytes).map_err(|_| Error::Invalid("public D1 semantic SQL text"))
        }
        _ => Err(Error::Invalid("public D1 semantic SQL text")),
    }
}
fn sql_optional_text_ref<'row>(row: &'row Row<'_>, column: usize) -> Result<Option<&'row str>> {
    match row.get_ref(column)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) => std::str::from_utf8(bytes)
            .map(Some)
            .map_err(|_| Error::Invalid("public D1 semantic SQL text")),
        _ => Err(Error::Invalid("public D1 semantic SQL text")),
    }
}
fn with_semantic_physical_row_owned<T>(
    db: &rusqlite::Connection,
    row: &Row<'_>,
    layout: KnowledgePayloadLayout,
    codec_column: usize,
    state: &CreationState<'_>,
    operation: impl FnOnce(&Value) -> Result<T>,
) -> Result<T> {
    let logical_len: i64 = row.get(0)?;
    let digest = sql_blob_ref(row, 1)?;
    let stored = sql_blob_ref(row, 2)?;
    let (codec, source_key) = if layout.uses_carriers() {
        let key = match row.get_ref(codec_column + 1)? {
            ValueRef::Null => None,
            ValueRef::Blob(key) => Some(key),
            _ => return Err(Error::Invalid("public semantic carrier key type")),
        };
        (row.get::<_, i64>(codec_column)?, key)
    } else {
        (0, None)
    };
    crate::knowledge_payload_codec::with_sql_logical_value(
        db,
        state,
        layout,
        logical_len,
        digest,
        stored,
        codec,
        source_key,
        MAX_ROW_BYTES,
        operation,
    )
}
fn node(statement: &mut Statement<'_>, capture: &PublicCapture, id: &str) -> Result<Option<Value>> {
    capture.charge_work(id.len() as u64)?;
    let mut rows = statement.query(params![id, MAX_ROW_BYTES as i64])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let len = admitted_row_len(capture, row.get(0)?)?;
    let digest: Vec<u8> = row.get(1)?;
    Ok(Some(check_row(len, &digest, row.get(2)?)?))
}
fn with_node_value<T>(
    db: &rusqlite::Connection,
    layout: KnowledgePayloadLayout,
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    id: &str,
    state: Option<&CreationState<'_>>,
    operation: impl FnOnce(&Value) -> Result<T>,
) -> Result<Option<T>> {
    if let Some(state) = state {
        state.active()?;
    }
    capture.charge_work(id.len() as u64)?;
    let mut rows = statement.query(params![id, MAX_ROW_BYTES as i64])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    if let Some(state) = state {
        let len = row.get::<_, i64>(0)?;
        if len < 0 {
            return Err(Error::Invalid("public D1 semantic row length"));
        }
        let len =
            usize::try_from(len).map_err(|_| Error::Budget("public D1 semantic row bytes"))?;
        if len > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 semantic row bytes"));
        }
        let digest = sql_blob_ref(row, 1)?;
        let raw = sql_blob_ref(row, 2)?;
        let _ = (len, raw, digest);
        with_semantic_physical_row_owned(db, row, layout, 3, state, operation).map(Some)
    } else {
        let len = admitted_row_len(capture, row.get(0)?)?;
        let digest: Vec<u8> = row.get(1)?;
        let value = check_row(len, &digest, row.get(2)?)?;
        operation(&value).map(Some)
    }
}

struct ScopedIdentity<'state, 'budget> {
    type_id: String,
    entity_id: String,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
struct ScopedSupportingClaim<'state, 'budget> {
    source_graph: String,
    id: String,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
struct ScopedClaimSummary<'state, 'budget> {
    claim_id: Option<String>,
    claim_version: Value,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
fn owned_claim_summary<'state, 'budget>(
    state: &'state CreationState<'budget>,
    claim: &Value,
) -> Result<ScopedClaimSummary<'state, 'budget>> {
    state.active()?;
    let claim_id = string(claim, "claim_id");
    let claim_version = member(claim, "claim_version");
    let version_bytes = serde_value_owned_state(claim_version, 0, state)?;
    let strings_bytes = claim_id.map_or(0, str::len);
    let _ = state.charge_work(
        strings_bytes
            .checked_add(version_bytes)
            .ok_or(Error::Budget("public D1 Claim summary work"))?,
    )?;
    let hold = state.hold(
        strings_bytes
            .checked_add(version_bytes)
            .ok_or(Error::Budget("public D1 Claim summary state"))?,
    )?;
    Ok(ScopedClaimSummary {
        claim_id: claim_id.map(str::to_owned),
        claim_version: claim_version.clone(),
        _hold: Some(hold),
    })
}
fn node_identity<'state, 'budget>(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    id: &str,
    state: Option<&'state CreationState<'budget>>,
) -> Result<Option<ScopedIdentity<'state, 'budget>>> {
    if let Some(state) = state {
        state.active()?;
    }
    capture.charge_work(id.len() as u64)?;
    if let Some(state) = state {
        let mut rows = statement.query([id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let type_id = sql_text_ref(row, 0)?;
        let entity_id = sql_text_ref(row, 1)?;
        capture.charge_work((type_id.len() + entity_id.len()) as u64)?;
        let hold = state.hold(
            type_id
                .len()
                .checked_add(entity_id.len())
                .ok_or(Error::Budget("public D1 node identity state"))?,
        )?;
        return Ok(Some(ScopedIdentity {
            type_id: type_id.to_owned(),
            entity_id: entity_id.to_owned(),
            _hold: Some(hold),
        }));
    }
    let found: Option<(String, String)> = statement
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    if let Some((type_id, entity_id)) = &found {
        capture.charge_work((type_id.len() + entity_id.len()) as u64)?;
    }
    Ok(found.map(|(type_id, entity_id)| ScopedIdentity {
        type_id,
        entity_id,
        _hold: None,
    }))
}
fn claim_edge(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    id: &str,
    predicate: &str,
    target: &str,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    if let Some(state) = state {
        state.active()?;
    }
    capture.charge_work((id.len() + predicate.len()) as u64)?;
    let mut rows = statement.query(params![id, predicate])?;
    let first_matches = if let Some(row) = rows.next()? {
        if let Some(state) = state {
            state.active()?;
        }
        let first = sql_text_ref(row, 0)?;
        capture.charge_work(first.len() as u64)?;
        first == target
    } else {
        false
    };
    let second = rows.next()?.is_some();
    if second || !first_matches {
        return Err(Error::Invalid("public D1 Claim incidence"));
    }
    Ok(())
}
fn supporting_claim<'state, 'budget>(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    claim_ref: &str,
    state: Option<&'state CreationState<'budget>>,
) -> Result<Option<ScopedSupportingClaim<'state, 'budget>>> {
    if let Some(state) = state {
        state.active()?;
    }
    capture.charge_work(claim_ref.len() as u64)?;
    let mut rows = statement.query([claim_ref])?;
    let mut last: Option<ScopedSupportingClaim<'state, 'budget>> = None;
    while let Some(row) = rows.next()? {
        if let Some(state) = state {
            state.active()?;
        }
        // Python's final nodes are ordered by (source_graph,id), and its
        // claims_by_entity assignment retains the last matching Claim.
        // Select that one while streaming the existing entity index rather
        // than asking SQLite to materialize a per-lookup sorter.
        let source_graph = sql_text_ref(row, 0)?;
        let id = sql_text_ref(row, 1)?;
        capture.charge_work((source_graph.len() + id.len() + 16) as u64)?;
        if last
            .as_ref()
            .is_none_or(|old| (source_graph, id) > (old.source_graph.as_str(), old.id.as_str()))
        {
            let hold = if let Some(state) = state {
                Some(
                    state.hold(
                        source_graph
                            .len()
                            .checked_add(id.len())
                            .ok_or(Error::Budget("public D1 supporting Claim state"))?,
                    )?,
                )
            } else {
                None
            };
            last = Some(ScopedSupportingClaim {
                source_graph: source_graph.to_owned(),
                id: id.to_owned(),
                _hold: hold,
            });
        }
    }
    Ok(last)
}
fn record_cardinality(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    direction: i64,
    endpoint: &str,
    relation_type: &str,
    scope: Option<&str>,
    source_order: i64,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    if let Some(state) = state {
        state.active()?;
    }
    let key_bytes = endpoint
        .len()
        .checked_add(relation_type.len())
        .and_then(|n| n.checked_add(scope.map_or(0, str::len)))
        .ok_or(Error::Budget("public D1 cardinality key bytes"))?;
    // The scalar key is written to the TEMP row, uniqueness index and
    // first-seen order index. This charges logical work, not physical SQLite
    // pages or journaling; those need separate whole-build admission.
    capture.charge_work(
        (key_bytes as u64)
            .checked_mul(3)
            .and_then(|n| n.checked_add(96))
            .ok_or(Error::Budget("public D1 cardinality key work"))?,
    )?;
    let scope_kind = i64::from(scope.is_some());
    let changed = statement.execute(params![
        direction,
        endpoint,
        relation_type,
        scope_kind,
        scope.unwrap_or(""),
        source_order
    ])?;
    if changed != 1 {
        return Err(Error::Budget("public D1 cardinality count"));
    }
    Ok(())
}
// This is the serialized report width, separate from the retained Value/map
// allocation below. Counting the actual UTF-8/escape sequence avoids charging
// every ordinary ASCII ID as if every byte required a six-byte JSON escape.
fn semantic_gap_encoded_len(id: &str, kind: &str) -> Result<usize> {
    #[derive(serde::Serialize)]
    struct Gap<'a> { id: &'a str, kind: &'a str }
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.checked_add(bytes.len())
                .ok_or_else(|| std::io::Error::other("semantic gap size overflow"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut count = Count(0);
    serde_json::to_writer(&mut count, &Gap { id, kind })
        .map_err(|_| Error::Budget("public D1 semantic report bytes"))?;
    // One separator per element is a conservative allowance for the array.
    count.0.checked_add(1).ok_or(Error::Budget("public D1 semantic report bytes"))
}

fn push_gap(
    capture: &PublicCapture,
    gaps: &mut Vec<Value>,
    live_bytes: &mut usize,
    id: &str,
    kind: &str,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    if let Some(state) = state {
        state.active()?;
    }
    let bytes = semantic_gap_encoded_len(id, kind)?;
    *live_bytes = live_bytes
        .checked_add(bytes)
        .filter(|n| *n <= crate::knowledge_seal::MAX_GRAPH_HEADER_BYTES)
        .ok_or(Error::Budget("public D1 semantic report bytes"))?;
    capture.charge_work(bytes as u64)?;
    if let Some(state) = state {
        let object_bytes = crate::knowledge_normalization::serde_object_slots_upper(2)?
            .checked_add("id".len() + "kind".len())
            .and_then(|n| n.checked_add(id.len()))
            .and_then(|n| n.checked_add(kind.len()))
            .ok_or(Error::Budget("public D1 semantic report object"))?;
        state.retain(object_bytes)?;
        if gaps.len() == gaps.capacity() {
            let next = gaps
                .capacity()
                .checked_mul(2)
                .filter(|capacity| *capacity > gaps.len())
                .unwrap_or(4);
            let added = next
                .checked_sub(gaps.capacity())
                .and_then(|slots| slots.checked_mul(std::mem::size_of::<Value>()))
                .ok_or(Error::Budget("public D1 semantic report vector"))?;
            state.retain(added)?;
            gaps.reserve_exact(next.saturating_sub(gaps.len()));
        }
        let mut object = serde_json::Map::new();
        object.insert("id".to_owned(), Value::String(id.to_owned()));
        object.insert("kind".to_owned(), Value::String(kind.to_owned()));
        gaps.push(Value::Object(object));
    } else {
        gaps.push(json!({"id":id,"kind":kind}));
    }
    Ok(())
}
fn registry_entries<'a>(
    registry: &'a Value,
    key: &str,
    id: &str,
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
) -> Result<BTreeMap<String, &'a Value>> {
    let rows = registry
        .get(key)
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("public D1 semantic registry collection"))?;
    if rows.len() > 4096 {
        return Err(Error::Budget("public D1 semantic registry entries"));
    }
    let mut entries = BTreeMap::new();
    for row in rows {
        if let Some(state) = state {
            state.active()?;
        }
        let name = string(row, id).ok_or(Error::Invalid("public D1 semantic registry ID"))?;
        capture.charge_work((name.len() + 32) as u64)?;
        if entries.insert(name.to_owned(), row).is_some() {
            return Err(Error::Invalid("public D1 duplicate semantic registry ID"));
        }
    }
    Ok(entries)
}
fn semantic_tree_node<K, V>() -> Result<usize> {
    11usize
        .checked_mul(std::mem::size_of::<(K, V)>())
        .and_then(|n| n.checked_add(16 * std::mem::size_of::<usize>()))
        .ok_or(Error::Budget("public D1 semantic B-tree geometry"))
}
fn registry_entries_owned_state(
    registry: &Value,
    key: &str,
    id: &str,
    state: &CreationState<'_>,
) -> Result<usize> {
    let rows = registry
        .get(key)
        .and_then(Value::as_array)
        .filter(|rows| rows.len() <= 4096)
        .ok_or(Error::Invalid("public D1 semantic registry collection"))?;
    let per_entry = semantic_tree_node::<String, &Value>()?;
    let mut bytes = rows
        .len()
        .checked_mul(per_entry)
        .ok_or(Error::Budget("public D1 semantic registry entries"))?;
    for row in rows {
        state.active()?;
        let name = string(row, id).ok_or(Error::Invalid("public D1 semantic registry ID"))?;
        bytes = bytes
            .checked_add(name.len())
            .ok_or(Error::Budget("public D1 semantic registry entries"))?;
    }
    Ok(bytes)
}
fn serde_value_owned_state(
    value: &Value,
    depth: usize,
    state: &CreationState<'_>,
) -> Result<usize> {
    if depth > 96 {
        return Err(Error::Budget("public D1 semantic owned value depth"));
    }
    state.active()?;
    let mut total = 0usize;
    match value {
        Value::Null | Value::Bool(_) => (),
        Value::Number(number) => {
            total = total
                .checked_add(number.as_str().len())
                .ok_or(Error::Budget("public D1 semantic owned value"))?;
        }
        Value::String(text) => {
            total = total
                .checked_add(text.len())
                .ok_or(Error::Budget("public D1 semantic owned value"))?;
        }
        Value::Array(items) => {
            total = total
                .checked_add(
                    items
                        .len()
                        .checked_mul(std::mem::size_of::<Value>())
                        .ok_or(Error::Budget("public D1 semantic owned array"))?,
                )
                .ok_or(Error::Budget("public D1 semantic owned array"))?;
            for item in items {
                state.active()?;
                total = total
                    .checked_add(serde_value_owned_state(item, depth + 1, state)?)
                    .ok_or(Error::Budget("public D1 semantic owned array"))?;
            }
        }
        Value::Object(fields) => {
            let slots = crate::knowledge_normalization::serde_object_slots_upper(fields.len())?;
            total = total
                .checked_add(std::mem::size_of::<serde_json::Map<String, Value>>())
                .and_then(|n| n.checked_add(slots))
                .ok_or(Error::Budget("public D1 semantic owned object"))?;
            for (key, item) in fields {
                state.active()?;
                total = total
                    .checked_add(key.len())
                    .and_then(|n| {
                        n.checked_add(serde_value_owned_state(item, depth + 1, state).ok()?)
                    })
                    .ok_or(Error::Budget("public D1 semantic owned object"))?;
            }
        }
    }
    Ok(total)
}
fn language_tag(value: &str) -> bool {
    let mut parts = value.split('-');
    let first = parts.next().unwrap_or("");
    let ordinary = (2..=8).contains(&first.len()) && first.bytes().all(|b| b.is_ascii_alphabetic());
    let private = matches!(first, "i" | "I" | "x" | "X");
    if !ordinary && !private {
        return false;
    }
    let mut suffixes = 0usize;
    for part in parts {
        suffixes += 1;
        if !(1..=8).contains(&part.len()) || !part.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return false;
        }
    }
    ordinary || suffixes > 0
}
fn property_field(field: &str) -> bool {
    const NODE_FIELDS: &[&str] = &[
        "id",
        "entity_id",
        "native_id",
        "source_dossier_ref",
        "source_graph",
        "kind_id",
        "type_id",
        "type_mapping.status",
        "type_mapping.source_kind_id",
        "display.title.default",
        "display.title.ru",
        "display.title.en",
        "display.kind_label.default",
        "display.summary.default",
        "display.summary.ru",
        "display.summary.en",
        "display.summary_state",
        "epistemic.authority_layer",
        "epistemic.canon_status",
        "epistemic.review_posture",
        "epistemic.confidence",
        "graph_layers",
        "view_ids",
        "source_refs",
    ];
    if NODE_FIELDS.contains(&field) {
        return true;
    }
    let mut parts = field.split('.');
    let prefix = parts.next();
    let display = parts.next();
    let language = parts.next();
    if parts.next().is_none()
        && prefix == Some("display")
        && matches!(display, Some("title" | "kind_label" | "summary"))
        && language.is_some_and(|language| {
            matches!(language, "default" | "original") || language_tag(language)
        })
    {
        return true;
    }
    let Some(tail) = field
        .strip_prefix("attributes.")
        .or_else(|| field.strip_prefix("semantics."))
    else {
        return false;
    };
    (1..=128).contains(&tail.len())
        && tail.as_bytes()[0].is_ascii_alphanumeric()
        && tail
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        && !tail
            .split('.')
            .any(|part| matches!(part, "__proto__" | "prototype" | "constructor"))
}
fn supersession_chain(
    entries: &BTreeMap<String, &Value>,
    field: &str,
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    for start in entries.keys() {
        let _seen_hold = if let Some(state) = state {
            state.active()?;
            Some(
                state.hold(
                    entries
                        .len()
                        .checked_mul(semantic_tree_node::<&str, ()>()?)
                        .ok_or(Error::Budget("public D1 supersession state"))?,
                )?,
            )
        } else {
            None
        };
        let mut seen = BTreeSet::new();
        let mut current = start.as_str();
        loop {
            if let Some(state) = state {
                state.active()?;
            }
            capture.charge_work((current.len() + 16) as u64)?;
            if !seen.insert(current) {
                return Err(Error::Invalid("public D1 registry supersession cycle"));
            }
            let Some(next) = entries
                .get(current)
                .and_then(|entry| entry.get(field))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            else {
                break;
            };
            if !entries.contains_key(next) {
                return Err(Error::Invalid("public D1 registry missing supersession"));
            }
            current = next;
        }
    }
    Ok(())
}
fn template_labels(
    value: &Value,
    languages: &BTreeSet<&str>,
    state: Option<&CreationState<'_>>,
) -> Result<bool> {
    let Some(labels) = value.as_object() else {
        return Ok(false);
    };
    if labels.len() != languages.len() {
        return Ok(false);
    }
    for language in languages {
        if let Some(state) = state {
            state.active()?;
        }
        let Some(label) = labels.get(*language).and_then(Value::as_str) else {
            return Ok(false);
        };
        if let Some(state) = state {
            state.charge_work(label.len())?;
        }
        if label.trim().is_empty() || label.chars().take(257).count() > 256 {
            return Ok(false);
        }
    }
    Ok(true)
}
fn json_integer(value: &Value) -> Option<i128> {
    // Python's current registry predicate uses isinstance(value, int), which
    // includes booleans for this comparison.
    value
        .as_bool()
        .map(|value| i128::from(u8::from(value)))
        .or_else(|| value.as_i64().map(i128::from))
        .or_else(|| value.as_u64().map(i128::from))
}
fn current_claim_template(
    relation: &Value,
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    let Some(template) = relation.get("claim_navigation_template") else {
        return Ok(());
    };
    if let Some(state) = state {
        state.active()?;
    }
    let invalid = || Error::Invalid("public D1 claim navigation template");
    let object = template.as_object().ok_or_else(invalid)?;
    const FIELDS: &[&str] = &[
        "template_id",
        "template_version",
        "reader",
        "purpose",
        "owner_ref",
        "default_language",
        "max_output_bytes",
        "marker",
        "status_labels",
        "renderings",
    ];
    if !(FIELDS.len()..=FIELDS.len() + 1).contains(&object.len())
        || FIELDS.iter().any(|field| !object.contains_key(*field))
        || object
            .keys()
            .any(|key| !FIELDS.contains(&key.as_str()) && key.as_str() != "object_label_adapters")
    {
        return Err(invalid());
    }
    let id = template
        .get("template_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if let Some(state) = state {
        state.charge_work(id.len())?;
    }
    let suffix = id
        .strip_prefix("tos.navigation-template.")
        .ok_or_else(invalid)?;
    if suffix.is_empty()
        || suffix.split(['.', '-']).any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
        || template.get("reader").and_then(Value::as_str) != Some("claim-navigation-v1")
        || template.get("purpose").and_then(Value::as_str) != Some("claim-navigation-only")
        || template.get("owner_ref").and_then(Value::as_str) != Some("ToS/doctrine/HUMAN_FORMS.md")
        || !template["max_output_bytes"]
            .as_u64()
            .is_some_and(|n| (128..=16384).contains(&n))
    {
        return Err(invalid());
    }
    let version = template["template_version"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(invalid)?;
    if let Some(adapters) = object.get("object_label_adapters") {
        let adapters = adapters
            .as_array()
            .filter(|a| (1..=2).contains(&a.len()))
            .ok_or_else(invalid)?;
        let _adapter_hold = if let Some(state) = state {
            Some(
                state.hold(
                    adapters
                        .len()
                        .checked_mul(semantic_tree_node::<&str, ()>()?)
                        .ok_or(Error::Budget("public D1 navigation adapter state"))?,
                )?,
            )
        } else {
            None
        };
        let mut seen = BTreeSet::new();
        for adapter in adapters {
            if let Some(state) = state {
                state.active()?;
            }
            let name = adapter.as_str().ok_or_else(invalid)?;
            if !matches!(
                name,
                "historical-time-source-wording-v1" | "document-catalogue-time-source-wording-v1"
            ) || !seen.insert(name)
                || version < 2
                || (name == "document-catalogue-time-source-wording-v1" && version < 3)
            {
                return Err(invalid());
            }
        }
    }
    let renderings = template["renderings"]
        .as_object()
        .filter(|r| (1..=16).contains(&r.len()))
        .ok_or_else(invalid)?;
    let statuses = template["status_labels"]
        .as_object()
        .filter(|s| s.len() == 2)
        .ok_or_else(invalid)?;
    let _language_hold = if let Some(state) = state {
        let mut bytes = renderings
            .len()
            .checked_mul(
                semantic_tree_node::<&str, ()>()?
                    .checked_add(semantic_tree_node::<String, ()>()?)
                    .ok_or(Error::Budget("public D1 navigation language state"))?,
            )
            .ok_or(Error::Budget("public D1 navigation language state"))?;
        for language in renderings.keys() {
            state.active()?;
            bytes = bytes
                .checked_add(language.len())
                .ok_or(Error::Budget("public D1 navigation language state"))?;
        }
        Some(state.hold(bytes)?)
    } else {
        None
    };
    let mut languages = BTreeSet::new();
    let mut folded = BTreeSet::new();
    for language in renderings.keys() {
        if let Some(state) = state {
            state.charge_work(language.len())?;
        }
        capture.charge_work((language.len() + 16) as u64)?;
        let lower = language.to_ascii_lowercase();
        if language.len() > 64
            || !language_tag(language)
            || matches!(lower.as_str(), "default" | "original" | "auto")
            || !folded.insert(lower)
        {
            return Err(invalid());
        }
        languages.insert(language.as_str());
    }
    if !template
        .get("default_language")
        .and_then(Value::as_str)
        .is_some_and(|v| languages.contains(v))
        || !template_labels(&template["marker"], &languages, state)?
    {
        return Err(invalid());
    }
    for (key, expected) in [
        (
            "epistemic_status",
            &[
                "observed",
                "inferred",
                "reported",
                "interpreted",
                "uncertain",
                "disputed",
            ][..],
        ),
        (
            "review_status",
            &[
                "unreviewed",
                "accepted",
                "accepted_with_limits",
                "rejected",
                "ambiguous",
                "deferred",
                "superseded",
            ][..],
        ),
    ] {
        if let Some(state) = state {
            state.active()?;
        }
        let labels = statuses
            .get(key)
            .and_then(Value::as_object)
            .filter(|v| v.len() == expected.len())
            .ok_or_else(invalid)?;
        for name in expected {
            if let Some(state) = state {
                state.active()?;
            }
            let value = labels.get(*name).ok_or_else(invalid)?;
            if !template_labels(value, &languages, state)? {
                return Err(invalid());
            }
        }
    }
    const SLOTS: &[&str] = &[
        "claim-marker",
        "subject-label",
        "predicate-label",
        "object-label",
        "declared-epistemic-status",
        "declared-review-status",
    ];
    for parts in renderings.values() {
        if let Some(state) = state {
            state.active()?;
        }
        let parts = parts
            .as_array()
            .filter(|p| (6..=32).contains(&p.len()))
            .ok_or_else(invalid)?;
        if parts[0].as_object().is_none_or(|p| {
            p.len() != 1 || p.get("slot").and_then(Value::as_str) != Some("claim-marker")
        }) {
            return Err(invalid());
        }
        let _slot_hold = if let Some(state) = state {
            Some(
                state.hold(
                    SLOTS
                        .len()
                        .checked_mul(semantic_tree_node::<&str, ()>()?)
                        .ok_or(Error::Budget("public D1 navigation slots state"))?,
                )?,
            )
        } else {
            None
        };
        let mut seen = BTreeSet::new();
        for part in parts {
            if let Some(state) = state {
                state.active()?;
            }
            capture.charge_work(32)?;
            let object = part
                .as_object()
                .filter(|p| p.len() == 1)
                .ok_or_else(invalid)?;
            if let Some(slot) = object.get("slot") {
                let slot = slot.as_str().ok_or_else(invalid)?;
                if !SLOTS.contains(&slot) || !seen.insert(slot) {
                    return Err(invalid());
                }
            } else if !object
                .get("literal")
                .and_then(Value::as_str)
                .is_some_and(|literal| (1..=256).contains(&literal.chars().count()))
            {
                return Err(invalid());
            }
        }
        if seen.len() != SLOTS.len() {
            return Err(invalid());
        }
    }
    Ok(())
}
/// Validate only the current selected registries used by this full public
/// build. Historical registry migration checks belong to their source owner.
pub(crate) fn validate_public_current_registries(
    capture: &PublicCapture,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
) -> Result<KnowledgeRegistry> {
    const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;
    if entity_bytes.len() > MAX_REGISTRY_BYTES || relation_bytes.len() > MAX_REGISTRY_BYTES {
        return Err(Error::Budget("public D1 semantic registry bytes"));
    }
    let total = (entity_bytes.len() as u64)
        .checked_add(relation_bytes.len() as u64)
        .ok_or(Error::Budget("public D1 semantic registry work"))?;
    capture.charge_work(
        total
            .checked_mul(4)
            .ok_or(Error::Budget("public D1 semantic registry work"))?,
    )?;
    let registry = KnowledgeRegistry::parse(entity_bytes, relation_bytes)?;
    let entity: Value = serde_json::from_slice(entity_bytes)
        .map_err(|_| Error::Invalid("public D1 entity registry JSON"))?;
    let relation: Value = serde_json::from_slice(relation_bytes)
        .map_err(|_| Error::Invalid("public D1 relation registry JSON"))?;
    validate_public_current_registries_values(capture, &entity, &relation, &registry, None)?;
    Ok(registry)
}

pub(crate) fn validate_public_current_registries_owned(
    capture: &PublicCapture,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    state: &CreationState<'_>,
) -> Result<KnowledgeRegistry> {
    KnowledgeRegistry::with_parsed_owned(
        entity_bytes,
        relation_bytes,
        state,
        |entity, relation, registry| {
            validate_public_current_registries_values(
                capture,
                entity,
                relation,
                registry,
                Some(state),
            )
        },
    )
    .map(|(registry, ())| registry)
}

fn validate_public_current_registries_values(
    capture: &PublicCapture,
    entity: &Value,
    relation: &Value,
    registry: &KnowledgeRegistry,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;
    if let Some(state) = state {
        state.active()?;
    }
    let _entries_hold = if let Some(state) = state {
        let entries_bytes = registry_entries_owned_state(entity, "types", "type_id", state)?
            .checked_add(registry_entries_owned_state(
                relation,
                "relations",
                "relation_type_id",
                state,
            )?)
            .ok_or(Error::Budget("public D1 semantic registry entries"))?;
        Some(state.hold(entries_bytes)?)
    } else {
        None
    };
    let entities = registry_entries(&entity, "types", "type_id", capture, state)?;
    let relations = registry_entries(&relation, "relations", "relation_type_id", capture, state)?;
    if let Some(presentation) = entity.get("context_presentation").filter(|v| !v.is_null()) {
        if let Some(state) = state {
            crate::knowledge_readable_context::validate_current_context_presentation_owned(
                presentation,
                state,
            )?;
        } else {
            crate::knowledge_readable_context::validate_current_context_presentation(presentation)?;
        }
    }
    let properties = entity
        .get("property_definitions")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("public D1 property registry"))?;
    let _property_ids_hold = if let Some(state) = state {
        let property_set_bytes = properties
            .len()
            .checked_mul(semantic_tree_node::<&str, ()>()?)
            .ok_or(Error::Budget("public D1 property registry state"))?;
        Some(state.hold(property_set_bytes)?)
    } else {
        None
    };
    let mut property_ids = BTreeSet::new();
    for definition in properties {
        if let Some(state) = state {
            state.active()?;
        }
        capture.charge_work(64)?;
        let id = definition
            .get("property_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(Error::Invalid("public D1 property ID"))?;
        let path = definition
            .get("field")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("public D1 property field"))?;
        let owners = member(definition, "applies_to")
            .as_array()
            .ok_or(Error::Invalid("public D1 property owners"))?;
        if !property_ids.insert(id) || !property_field(path) {
            return Err(Error::Invalid("public D1 property registry contract"));
        }
        for owner in owners {
            if let Some(state) = state {
                state.active()?;
            }
            if owner
                .as_str()
                .is_none_or(|owner| !entities.contains_key(owner))
            {
                return Err(Error::Invalid("public D1 property registry contract"));
            }
        }
    }
    for entry in entities.values() {
        if let Some(state) = state {
            state.active()?;
        }
        capture.charge_work(32)?;
        if truthy(member(entry, "abstract"))
            && member(entry, "source_mappings")
                .as_array()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(Error::Invalid("public D1 abstract entity mapping"));
        }
    }
    supersession_chain(&entities, "supersedes_type_id", capture, state)?;
    supersession_chain(&relations, "supersedes_relation_type_id", capture, state)?;
    for (id, entry) in &relations {
        if let Some(state) = state {
            state.active()?;
        }
        capture.charge_work((id.len() + 64) as u64)?;
        if truthy(member(entry, "abstract"))
            && member(entry, "source_mappings")
                .as_array()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(Error::Invalid("public D1 abstract relation mapping"));
        }
        for field in ["domain_type_ids", "range_type_ids"] {
            let endpoints = member(entry, field)
                .as_array()
                .ok_or(Error::Invalid("public D1 registry endpoint list"))?;
            for endpoint in endpoints {
                if let Some(state) = state {
                    state.active()?;
                }
                let endpoint = endpoint
                    .as_str()
                    .ok_or(Error::Invalid("public D1 registry endpoint type"))?;
                capture.charge_work(endpoint.len() as u64)?;
                if !entities.contains_key(endpoint) {
                    return Err(Error::Invalid("public D1 registry endpoint type"));
                }
            }
        }
        for parent in strings(entry, "parent_relation_type_ids") {
            if let Some(state) = state {
                state.active()?;
            }
            if !relations.contains_key(parent) {
                return Err(Error::Invalid("public D1 relation parent"));
            }
        }
        if let Some(inverse) = string(entry, "inverse_relation_type_id") {
            let counterpart = relations
                .get(inverse)
                .ok_or(Error::Invalid("public D1 relation inverse"))?;
            if string(counterpart, "inverse_relation_type_id") != Some(id.as_str()) {
                return Err(Error::Invalid("public D1 relation inverse reciprocity"));
            }
        }
        for (minimum, maximum) in [
            ("cardinality.per_subject_min", "cardinality.per_subject_max"),
            ("cardinality.per_object_min", "cardinality.per_object_max"),
        ] {
            if let Some(state) = state {
                state.active()?;
            }
            if let (Some(low), Some(high)) = (
                json_integer(member(entry, minimum)),
                json_integer(member(entry, maximum)),
            ) {
                if low > high {
                    return Err(Error::Invalid("public D1 relation cardinality registry"));
                }
            }
        }
    }
    current_claim_template(&relation, capture, state)
}
fn is_a(
    type_id: &str,
    allowed: &Value,
    entities: &BTreeMap<String, &Value>,
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
    hierarchy_edges: usize,
) -> Result<bool> {
    let Some(allowed) = allowed.as_array() else {
        return Ok(false);
    };
    is_a_matches(
        type_id,
        allowed.len(),
        |candidate| {
            for item in allowed {
                if let Some(state) = state {
                    state.active()?;
                }
                if item.as_str() == Some(candidate) {
                    return Ok(true);
                }
            }
            Ok(false)
        },
        entities,
        capture,
        state,
        hierarchy_edges,
    )
}
fn is_a_one(
    type_id: &str,
    allowed: &str,
    entities: &BTreeMap<String, &Value>,
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
    hierarchy_edges: usize,
) -> Result<bool> {
    is_a_matches(
        type_id,
        1,
        |candidate| Ok(candidate == allowed),
        entities,
        capture,
        state,
        hierarchy_edges,
    )
}
fn is_a_matches(
    type_id: &str,
    allowed_count: usize,
    allowed_matches: impl Fn(&str) -> Result<bool>,
    entities: &BTreeMap<String, &Value>,
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
    hierarchy_edges: usize,
) -> Result<bool> {
    capture.charge_work(type_id.len() as u64)?;
    if let Some(state) = state {
        state.active()?;
    }
    let current_capacity = hierarchy_edges
        .checked_add(1)
        .ok_or(Error::Budget("public D1 hierarchy stack"))?;
    let _hierarchy_hold = if let Some(state) = state {
        let stack_bytes = current_capacity
            .checked_mul(std::mem::size_of::<&str>())
            .ok_or(Error::Budget("public D1 hierarchy state"))?;
        let seen_bytes = entities
            .len()
            .checked_mul(semantic_tree_node::<&str, ()>()?)
            .ok_or(Error::Budget("public D1 hierarchy state"))?;
        Some(
            state.hold(
                stack_bytes
                    .checked_add(seen_bytes)
                    .ok_or(Error::Budget("public D1 hierarchy state"))?,
            )?,
        )
    } else {
        None
    };
    let mut current = Vec::with_capacity(current_capacity);
    current.push(type_id);
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = current.pop() {
        if let Some(state) = state {
            state.active()?;
        }
        capture.charge_work((id.len() + allowed_count + 16) as u64)?;
        if allowed_matches(id)? {
            return Ok(true);
        }
        if !seen.insert(id) {
            continue;
        }
        if let Some(entry) = entities.get(id) {
            if let Some(state) = state {
                state.active()?;
            }
            capture.charge_work(
                strings(entry, "parent_type_ids")
                    .map(str::len)
                    .sum::<usize>() as u64,
            )?;
            current.extend(strings(entry, "parent_type_ids"));
        }
    }
    Ok(false)
}
fn mapped_node_type<'a>(registry: &'a KnowledgeRegistry, value: &Value) -> &'a str {
    registry
        .entity(
            string(value, "source_graph").unwrap_or(""),
            string(value, "type_mapping.source_kind_id").unwrap_or(""),
        )
        .type_id
}
fn mapped_relation_type<'a>(registry: &'a KnowledgeRegistry, value: &Value) -> &'a str {
    registry
        .relation(
            string(value, "source_graph").unwrap_or(""),
            string(value, "predicate_mapping.source_predicate_id").unwrap_or(""),
            "edge",
        )
        .type_id
}
fn exact_ref(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != 3
        || !object.contains_key("id")
        || !object.contains_key("version")
        || !object.contains_key("digest")
        || value
            .get("id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || !member(value, "version")
            .as_u64()
            .is_some_and(|number| (1..=9_007_199_254_740_991).contains(&number))
    {
        return false;
    }
    value
        .get("digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| {
            digest.len() == 71
                && digest.starts_with("sha256:")
                && digest[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}
fn exact_record_digest(capture: &PublicCapture, value: &Value) -> Result<String> {
    let raw =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("public D1 exact record JSON"))?;
    if raw.len() > MAX_ROW_BYTES {
        return Err(Error::Budget("public D1 exact record bytes"));
    }
    capture.charge_work(raw.len() as u64)?;
    Ok(Digest256::of_bytes(&raw).to_hex())
}
fn exact_record_digest_owned(
    capture: &PublicCapture,
    state: &CreationState<'_>,
    value: &Value,
) -> Result<Digest256> {
    state.with_json_encoded(value, MAX_ROW_BYTES, |raw| {
        capture.charge_work(raw.len() as u64)?;
        Ok(Digest256::of_bytes(raw))
    })
}
fn exact_record_matches(
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
    left: &Value,
    right: &Value,
) -> Result<bool> {
    if let Some(state) = state {
        let left_digest = exact_record_digest_owned(capture, state, left)?;
        let right_digest = exact_record_digest_owned(capture, state, right)?;
        Ok(left_digest.as_bytes() == right_digest.as_bytes())
    } else {
        Ok(exact_record_digest(capture, left)? == exact_record_digest(capture, right)?)
    }
}
#[derive(Serialize)]
struct ExactCurrentRef<'a> {
    digest: &'a str,
    id: &'a str,
    version: u64,
}
fn current_ref_digest_owned(
    capture: &PublicCapture,
    state: &CreationState<'_>,
    id: &str,
    version: u64,
    record_digest: &Digest256,
) -> Result<([u8; 71], Digest256)> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut digest_text = [0u8; 71];
    digest_text[..7].copy_from_slice(b"sha256:");
    for (index, byte) in record_digest.as_bytes().iter().enumerate() {
        state.active()?;
        digest_text[7 + index * 2] = HEX[(byte >> 4) as usize];
        digest_text[8 + index * 2] = HEX[(byte & 0x0f) as usize];
    }
    let digest_str = std::str::from_utf8(&digest_text)
        .map_err(|_| Error::Invalid("public D1 metadata digest text"))?;
    let value = ExactCurrentRef {
        digest: digest_str,
        id,
        version,
    };
    let digest = state.with_json_encoded(&value, MAX_ROW_BYTES, |raw| {
        capture.charge_work(raw.len() as u64)?;
        Ok(Digest256::of_bytes(raw))
    })?;
    Ok((digest_text, digest))
}
fn metadata_history_refs<'a>(
    capture: &PublicCapture,
    node: &'a Value,
    state: Option<&CreationState<'_>>,
) -> Result<&'a [Value]> {
    if let Some(state) = state {
        state.active()?;
    }
    let history = member(node, "attributes.record_history");
    let record_value = member(node, "attributes.source_record");
    let (Some(history), Some(record)) = (history.as_object(), record_value.as_object()) else {
        return Err(Error::Invalid("public D1 metadata history envelope"));
    };
    const FIELDS: [&str; 10] = [
        "schema_version",
        "status",
        "reason",
        "record_id",
        "current_ref",
        "refs",
        "provenance",
        "grants_current_use",
        "performs_assessment",
        "writes_to_source",
    ];
    if history.len() != FIELDS.len()
        || FIELDS.iter().any(|name| !history.contains_key(*name))
        || string(member(node, "attributes"), "record_history.schema_version")
            != Some("tos_metadata_record_history_v1")
        || string(member(node, "attributes"), "record_history.status") != Some("available")
        || !string(member(node, "attributes"), "record_history.reason")
            .is_some_and(|reason| reason.len() <= 256)
        || [
            "grants_current_use",
            "performs_assessment",
            "writes_to_source",
        ]
        .iter()
        .any(|name| history.get(*name) != Some(&Value::Bool(false)))
        || history
            .get("provenance")
            .and_then(Value::as_object)
            .is_none_or(|value| value.is_empty())
    {
        return Err(Error::Invalid("public D1 metadata history fields"));
    }
    let identity = match string(record_value, "schema_version") {
        Some("tos_canonical_node_v1") => "node_id",
        Some("tos_scholarly_composite_witness_v1") => "composite_id",
        Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => "artifact_id",
        _ => "record_id",
    };
    let id =
        string(record_value, identity).ok_or(Error::Invalid("public D1 metadata record ID"))?;
    let version = member(record_value, "record_version")
        .as_u64()
        .filter(|version| (1..=9_007_199_254_740_991).contains(version))
        .ok_or(Error::Invalid("public D1 metadata record version"))?;
    if string(node, "entity_id") != Some(id)
        || string(member(node, "attributes"), "record_history.record_id") != Some(id)
    {
        return Err(Error::Invalid("public D1 metadata record identity"));
    }
    let declared = member(node, "attributes.record_history.current_ref");
    let (current_digest_text, current_value) = if let Some(state) = state {
        let record_digest =
            exact_record_digest_owned(capture, state, member(node, "attributes.source_record"))?;
        let (text, current_digest) =
            current_ref_digest_owned(capture, state, id, version, &record_digest)?;
        let declared_digest = exact_record_digest_owned(capture, state, declared)?;
        if !exact_ref(declared) || declared_digest.as_bytes() != current_digest.as_bytes() {
            return Err(Error::Invalid("public D1 metadata current ref"));
        }
        (Some(text), None)
    } else {
        let current = json!({
            "id":id,"version":version,
            "digest":format!("sha256:{}",exact_record_digest(capture, member(node, "attributes.source_record"))?)
        });
        if !exact_ref(declared)
            || exact_record_digest(capture, &current)? != exact_record_digest(capture, declared)?
        {
            return Err(Error::Invalid("public D1 metadata current ref"));
        }
        (None, Some(current))
    };
    let refs = history
        .get("refs")
        .and_then(Value::as_array)
        .filter(|refs| (1..=129).contains(&refs.len()))
        .ok_or(Error::Invalid("public D1 metadata history refs"))?;
    let mut previous = None;
    for reference in refs {
        if let Some(state) = state {
            state.active()?;
        }
        let next = member(reference, "version")
            .as_u64()
            .ok_or(Error::Invalid("public D1 metadata history version"))?;
        if !exact_ref(reference)
            || string(reference, "id") != Some(id)
            || previous.is_some_and(|old: u64| next != old.saturating_add(1))
        {
            return Err(Error::Invalid("public D1 metadata history sequence"));
        }
        previous = Some(next);
    }
    let head_matches = if let Some(digest_text) = current_digest_text.as_ref() {
        refs.last().is_some_and(|head| {
            exact_ref(head)
                && string(head, "id") == Some(id)
                && member(head, "version").as_u64() == Some(version)
                && string(head, "digest") == std::str::from_utf8(digest_text).ok()
        })
    } else {
        refs.last() == current_value.as_ref()
    };
    if !head_matches {
        return Err(Error::Invalid("public D1 metadata history head"));
    }
    Ok(refs)
}
fn endpoint_contract(
    from: Option<&ScopedIdentity<'_, '_>>,
    to: Option<&ScopedIdentity<'_, '_>>,
    relation_type: &str,
    fallback_relation: &str,
    relation_entry: &Value,
    entities: &BTreeMap<String, &Value>,
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
    hierarchy_edges: usize,
) -> Result<()> {
    if relation_type == fallback_relation {
        return Ok(());
    }
    let (Some(from), Some(to)) = (from, to) else {
        return Err(Error::Invalid("public D1 unresolved normalized endpoint"));
    };
    if !is_a(
        &from.type_id,
        member(relation_entry, "domain_type_ids"),
        entities,
        capture,
        state,
        hierarchy_edges,
    )? || !is_a(
        &to.type_id,
        member(relation_entry, "range_type_ids"),
        entities,
        capture,
        state,
        hierarchy_edges,
    )? {
        return Err(Error::Invalid("public D1 semantic endpoint type"));
    }
    Ok(())
}
fn property_type(value: &Value, kind: &str, state: Option<&CreationState<'_>>) -> Result<bool> {
    Ok(match kind {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "string-array" => {
            let Some(items) = value.as_array() else {
                return Ok(false);
            };
            let mut valid = true;
            for item in items {
                if let Some(state) = state {
                    state.active()?;
                }
                valid &= item.is_string();
                if !valid {
                    break;
                }
            }
            valid
        }
        _ => false,
    })
}
fn check_node(
    value: &Value,
    registry: &KnowledgeRegistry,
    entities: &BTreeMap<String, &Value>,
    properties: &[Value],
    capture: &PublicCapture,
    state: Option<&CreationState<'_>>,
    hierarchy_edges: usize,
) -> Result<()> {
    if let Some(state) = state {
        state.active()?;
    }
    let id = string(value, "id").ok_or(Error::Invalid("public D1 node ID"))?;
    let type_id = string(value, "type_id").ok_or(Error::Invalid("public D1 node type"))?;
    let entry = entities
        .get(type_id)
        .ok_or(Error::Invalid("public D1 unregistered node type"))?;
    if truthy(member(entry, "abstract"))
        || type_id != mapped_node_type(registry, value)
        || string(value, "type_mapping.status")
            != Some(if type_id == registry.fallback_entity_type_id() {
                "unmapped"
            } else {
                "mapped"
            })
        || string(value, "type_mapping.source_kind_id").is_none()
        || string(value, "entity_id").is_none()
        || strings(value, "source_refs").next().is_none()
    {
        return Err(Error::Invalid("public D1 node semantic mapping/source"));
    }
    capture.charge_work(properties.len() as u64)?;
    for definition in properties {
        if let Some(state) = state {
            state.active()?;
        }
        let inherited = truthy(member(definition, "inherited"));
        let applies = if inherited {
            is_a(
                type_id,
                member(definition, "applies_to"),
                entities,
                capture,
                state,
                hierarchy_edges,
            )?
        } else {
            let mut applies = false;
            for owner in strings(definition, "applies_to") {
                if let Some(state) = state {
                    state.active()?;
                }
                applies |= owner == type_id;
                if applies {
                    break;
                }
            }
            applies
        };
        if !applies {
            continue;
        }
        let path = string(definition, "field").ok_or(Error::Invalid("public D1 property field"))?;
        match field(value, path) {
            None | Some(Value::Null) if truthy(member(definition, "required")) => {
                return Err(Error::Invalid("public D1 required node property"));
            }
            None | Some(Value::Null) => continue,
            Some(field) => {
                if !property_type(field, string(definition, "value_type").unwrap_or(""), state)? {
                    return Err(Error::Invalid("public D1 node property type"));
                }
            }
        }
    }
    match type_id {
        "tos.entity.temporal-assertion"
            if !matches!(
                string(value, "semantics.time.normalization_status"),
                Some("structured-source" | "source-literal-parsed" | "source-literal-unparsed")
            ) =>
        {
            return Err(Error::Invalid("public D1 temporal source value"));
        }
        "tos.entity.place" if string(value, "semantics.space.kind") != Some("place-identity") => {
            return Err(Error::Invalid("public D1 place identity"));
        }
        "tos.entity.navigation-region"
            if string(value, "semantics.space.kind") != Some("navigation-region") =>
        {
            return Err(Error::Invalid("public D1 navigation region marker"));
        }
        _ => (),
    }
    if id.len() > 4096 {
        return Err(Error::Budget("public D1 semantic node ID"));
    }
    Ok(())
}

/// A valid report is produced only after every graph row and assertion is
/// checked. Invalid rows refuse the disposable build before any completion
/// marker; they are never converted to a fabricated `valid: true` packet.
pub(crate) fn validate_public_semantics(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
) -> Result<Value> {
    if !stage.public_build() {
        return Err(Error::Invalid("public D1 stage required"));
    }
    let state = stage.owned_creation_state();
    validate_public_semantics_captured(
        stage,
        capture,
        registry,
        entity_bytes,
        relation_bytes,
        state,
    )
}

pub(crate) fn validate_native_snapshot_semantics(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
) -> Result<Value> {
    if stage.public_build()
        || stage.exact_receipt()?.binding.owner_profile != "tos-native-projection-snapshot-v1"
    {
        return Err(Error::Invalid("native snapshot stage required"));
    }
    let state = stage.owned_creation_state();
    validate_public_semantics_captured(
        stage,
        capture,
        registry,
        entity_bytes,
        relation_bytes,
        state,
    )
}

fn validate_public_semantics_captured(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    state: Option<&CreationState<'_>>,
) -> Result<Value> {
    let registry_work = entity_bytes
        .len()
        .checked_add(relation_bytes.len())
        .and_then(|n| n.checked_mul(2))
        .ok_or(Error::Budget("public D1 semantic registry work"))?;
    if let Some(state) = state {
        state.charge_work(registry_work)?;
    } else {
        capture.charge_work(registry_work as u64)?;
    }
    if !digest_matches_hex(&Digest256::of_bytes(entity_bytes), &registry.entity_sha256)
        || !digest_matches_hex(
            &Digest256::of_bytes(relation_bytes),
            &registry.relation_sha256,
        )
    {
        return Err(Error::Invalid("public D1 semantic registry binding"));
    }
    if let Some(state) = state {
        let limits = tos_foundation::JsonLimits::new(4 * 1024 * 1024, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("public D1 semantic registry limits"))?;
        return state.with_serde_owned_value_with_limits(entity_bytes, limits, |entity| {
            state.with_serde_owned_value_with_limits(relation_bytes, limits, |relation| {
                validate_public_semantics_values(
                    stage,
                    capture,
                    registry,
                    &entity,
                    &relation,
                    Some(state),
                )
            })
        });
    }
    let entity: Value = serde_json::from_slice(entity_bytes)
        .map_err(|_| Error::Invalid("public D1 entity registry JSON"))?;
    let relation: Value = serde_json::from_slice(relation_bytes)
        .map_err(|_| Error::Invalid("public D1 relation registry JSON"))?;
    validate_public_semantics_values(stage, capture, registry, &entity, &relation, None)
}

fn validate_public_semantics_values(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity: &Value,
    relation: &Value,
    state: Option<&CreationState<'_>>,
) -> Result<Value> {
    let entries_bytes = if let Some(state) = state {
        let bytes = registry_entries_owned_state(entity, "types", "type_id", state)?
            .checked_add(registry_entries_owned_state(
                relation,
                "relations",
                "relation_type_id",
                state,
            )?)
            .ok_or(Error::Budget("public D1 semantic registry entries"))?;
        Some(state.hold(bytes)?)
    } else {
        None
    };
    let _entries_hold = entries_bytes;
    let entities = registry_entries(entity, "types", "type_id", capture, state)?;
    let relations = registry_entries(relation, "relations", "relation_type_id", capture, state)?;
    let properties = entity
        .get("property_definitions")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("public D1 property registry"))?;
    let mut hierarchy_edges = 0usize;
    for entry in entities.values() {
        if let Some(state) = state {
            state.active()?;
        }
        for _parent in strings(entry, "parent_type_ids") {
            if let Some(state) = state {
                state.active()?;
            }
            hierarchy_edges = hierarchy_edges
                .checked_add(1)
                .ok_or(Error::Budget("public D1 semantic hierarchy edges"))?;
        }
    }
    let fallback_relation = string(&relation, "fallback_relation_type_id")
        .ok_or(Error::Invalid("public D1 relation fallback"))?;
    let mut registered_nodes = 0u64;
    let mut unmapped_nodes = 0u64;
    let mut registered_relations = 0u64;
    let mut unmapped_relations = 0u64;
    let mut claim_count = 0u64;
    let mut cross_layer = 0u64;
    let mut gaps = Vec::<Value>::new();
    let mut claim_gap_count = 0usize;
    let mut live_gap_bytes = 256usize;
    let layout = stage.payload_layout();
    if layout.uses_carriers() && state.is_none() {
        return Err(Error::Invalid("public semantic carrier owner absent"));
    }
    stage.with_connection(WritePhase::Finalize, |db| {
        // The Stage owns this disposable scalar summary under its existing
        // SQLite VM, TEMP page, cache and absolute deadline guards. A public
        // Stage has no active aggregate host-quota isolation guard.
        // No relation payload or graph-sized Rust incidence map is retained.
        db.execute_batch("CREATE TEMP TABLE d1_semantic_cardinality (
            direction INTEGER NOT NULL, endpoint TEXT NOT NULL,
            relation_type TEXT NOT NULL, scope_kind INTEGER NOT NULL,
            scope_value TEXT NOT NULL, first_order INTEGER NOT NULL,
            tally INTEGER NOT NULL DEFAULT 1,
            UNIQUE(direction,endpoint,relation_type,scope_kind,scope_value)
        );
        CREATE INDEX d1_semantic_cardinality_order
        ON d1_semantic_cardinality(direction,first_order);")?;
        let mut incidence_insert = db.prepare("INSERT INTO d1_semantic_cardinality
            (direction,endpoint,relation_type,scope_kind,scope_value,first_order)
            VALUES (?1,?2,?3,?4,?5,?6)
            ON CONFLICT(direction,endpoint,relation_type,scope_kind,scope_value)
            DO UPDATE SET tally=tally+1 WHERE tally < 9223372036854775807")?;
        let mut node_lookup = db.prepare(if layout.uses_carriers() { "SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?2 AND length(payload)<=?2+17 THEN payload ELSE NULL END,payload_codec,source_packet_sha256 FROM knowledge_nodes WHERE id=?1" } else { "SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?2 AND length(payload)=payload_len THEN payload ELSE NULL END FROM knowledge_nodes WHERE id=?1" })?;
        let mut identity_lookup = db.prepare("SELECT type_id,entity_id FROM knowledge_nodes WHERE id=?1")?;
        let mut claim_edges = db.prepare("SELECT to_id FROM knowledge_relations WHERE from_id=?1 AND relation_type_id=?2 LIMIT 2")?;
        let mut supporting_lookup = db.prepare("SELECT source_graph,id FROM knowledge_nodes WHERE entity_id=?1 AND type_id='tos.entity.claim'")?;
        {
            let mut statement = db.prepare(if layout.uses_carriers() { "SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?1 AND length(payload)<=?1+17 THEN payload ELSE NULL END,type_id,entity_id,source_graph,payload_codec,source_packet_sha256 FROM knowledge_nodes ORDER BY source_order" } else { "SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?1 AND length(payload)=payload_len THEN payload ELSE NULL END,type_id,entity_id,source_graph FROM knowledge_nodes ORDER BY source_order" })?;
            let mut rows = statement.query([MAX_ROW_BYTES as i64])?;
            while let Some(row) = rows.next()? {
                if let Some(state) = state {
                    state.active()?;
                }
                let mut process = |value: &Value,
                                   stored_type: &str,
                                   stored_entity: Option<&str>,
                                   stored_source: &str|
                 -> Result<()> {
                    if let Some(state) = state {
                        state.active()?;
                    }
                    capture.charge_work(
                        (stored_type.len()
                            + stored_entity.map_or(0, str::len)
                            + stored_source.len()) as u64,
                    )?;
                    if string(value, "type_id") != Some(stored_type)
                        || string(value, "entity_id") != stored_entity
                        || string(value, "source_graph") != Some(stored_source)
                    {
                        return Err(Error::Invalid("public D1 indexed node identity"));
                    }
                    check_node(
                        value,
                        registry,
                        &entities,
                        properties,
                        capture,
                        state,
                        hierarchy_edges,
                    )?;
                    let type_id = string(value, "type_id").unwrap_or("");
                    registered_nodes += u64::from(entities.contains_key(type_id));
                    unmapped_nodes += u64::from(type_id == registry.fallback_entity_type_id());
                    if type_id == "tos.entity.claim" {
                        claim_count += 1;
                        let id = string(value, "id").ok_or(Error::Invalid("public D1 Claim ID"))?;
                        let claim = member(value, "semantics.claim");
                        for (predicate, target) in [
                            ("tos.relation.has-subject", "subject_node_id"),
                            ("tos.relation.has-object", "object_node_id"),
                        ] {
                            if let Some(state) = state {
                                state.active()?;
                            }
                            let target = string(claim, target)
                                .ok_or(Error::Invalid("public D1 Claim endpoint"))?;
                            claim_edge(&mut claim_edges, capture, id, predicate, target, state)?;
                        }
                        let subject = string(claim, "subject_node_id")
                            .ok_or(Error::Invalid("public D1 Claim subject"))?;
                        let object = string(claim, "object_node_id")
                            .ok_or(Error::Invalid("public D1 Claim object"))?;
                        let relation_type =
                            string(claim, "relation_type_id").unwrap_or(fallback_relation);
                        let relation_entry = relations
                            .get(relation_type)
                            .ok_or(Error::Invalid("public D1 Claim relation type"))?;
                        let left = node_identity(
                            &mut identity_lookup,
                            capture,
                            subject,
                            state,
                        )?;
                        let right = node_identity(
                            &mut identity_lookup,
                            capture,
                            object,
                            state,
                        )?;
                        endpoint_contract(
                            left.as_ref(),
                            right.as_ref(),
                            relation_type,
                            fallback_relation,
                            relation_entry,
                            &entities,
                            capture,
                            state,
                            hierarchy_edges,
                        )?;
                        let mut evidence = strings(claim, "evidence_node_ids").peekable();
                        if evidence.peek().is_none() {
                            push_gap(
                                capture,
                                &mut gaps,
                                &mut live_gap_bytes,
                                id,
                                "claim-evidence-not-projected",
                                state,
                            )?;
                            claim_gap_count = claim_gap_count
                                .checked_add(1)
                                .ok_or(Error::Budget("public D1 claim gap count"))?;
                        }
                        for evidence_id in evidence {
                            if let Some(state) = state {
                                state.active()?;
                            }
                            if node_identity(
                                &mut identity_lookup,
                                capture,
                                evidence_id,
                                state,
                            )?
                            .is_none()
                            {
                                return Err(Error::Invalid("public D1 unresolved Claim evidence"));
                            }
                        }
                    }
                    Ok(())
                };
                if let Some(state) = state {
                    let len = row.get::<_, i64>(0)?;
                    if len < 0 {
                        return Err(Error::Invalid("public D1 semantic row length"));
                    }
                    let len = usize::try_from(len)
                        .map_err(|_| Error::Budget("public D1 semantic row bytes"))?;
                    if len > MAX_ROW_BYTES {
                        return Err(Error::Budget("public D1 semantic row bytes"));
                    }
                    let digest = sql_blob_ref(row, 1)?;
                    let raw = sql_blob_ref(row, 2)?;
                    let stored_type = sql_text_ref(row, 3)?;
                    let stored_entity = sql_optional_text_ref(row, 4)?;
                    let stored_source = sql_text_ref(row, 5)?;
                    if layout == KnowledgePayloadLayout::InlineV1 && len != raw.len() {
                        return Err(Error::Invalid("public D1 semantic row length"));
                    }
                    capture.charge_work(
                        (stored_type.len()
                            + stored_entity.map_or(0, str::len)
                            + stored_source.len()) as u64,
                    )?;
                    with_semantic_physical_row_owned(db, row, layout, 6, state, |value| {
                        process(value, stored_type, stored_entity, stored_source)
                    })?;
                } else {
                    let len = admitted_row_len(capture, row.get(0)?)?;
                    let digest: Vec<u8> = row.get(1)?;
                    let value = check_row(len, &digest, row.get(2)?)?;
                    let stored_type: String = row.get(3)?;
                    let stored_entity: Option<String> = row.get(4)?;
                    let stored_source: String = row.get(5)?;
                    process(
                        &value,
                        &stored_type,
                        stored_entity.as_deref(),
                        &stored_source,
                    )?;
                }
            }
        }
        {
            let mut statement = db.prepare(if layout.uses_carriers() { "SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?1 AND length(payload)<=?1+17 THEN payload ELSE NULL END,relation_type_id,from_id,to_id,source_graph,source_order,payload_codec,source_packet_sha256 FROM knowledge_relations ORDER BY source_order" } else { "SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?1 AND length(payload)=payload_len THEN payload ELSE NULL END,relation_type_id,from_id,to_id,source_graph,source_order FROM knowledge_relations ORDER BY source_order" })?;
            let mut rows = statement.query([MAX_ROW_BYTES as i64])?;
            while let Some(row) = rows.next()? {
                if let Some(state) = state {
                    state.active()?;
                }
                let mut process = |value: &Value,
                                   stored_type: &str,
                                   stored_from: &str,
                                   stored_to: &str,
                                   stored_source: &str,
                                   source_order: i64|
                 -> Result<()> {
                    if let Some(state) = state {
                        state.active()?;
                    }
                    capture.charge_work(
                        (stored_type.len()
                            + stored_from.len()
                            + stored_to.len()
                            + stored_source.len()) as u64,
                    )?;
                    if string(value, "relation_type_id") != Some(stored_type)
                        || string(value, "from_id") != Some(stored_from)
                        || string(value, "to_id") != Some(stored_to)
                        || string(value, "source_graph") != Some(stored_source)
                    {
                        return Err(Error::Invalid("public D1 indexed relation identity"));
                    }
                    let id = string(value, "id").ok_or(Error::Invalid("public D1 relation ID"))?;
                    let relation_type = string(value, "relation_type_id")
                        .ok_or(Error::Invalid("public D1 relation type"))?;
                    let entry = relations
                        .get(relation_type)
                        .ok_or(Error::Invalid("public D1 unregistered relation type"))?;
                    registered_relations += 1;
                    unmapped_relations += u64::from(relation_type == fallback_relation);
                    cross_layer += u64::from(string(value, "source_graph") == Some("semantic-interchange"));
                    if truthy(member(entry, "abstract"))
                        || relation_type != mapped_relation_type(registry, value)
                        || string(value, "predicate_mapping.status")
                            != Some(if relation_type == fallback_relation { "unmapped" } else { "mapped" })
                        || string(value, "predicate_mapping.source_predicate_id").is_none()
                        || (truthy(member(entry, "evidence_required"))
                            && strings(value, "source_refs").next().is_none())
                        || (member(entry, "review_requirement").as_str() != Some("none")
                            && string(value, "epistemic.review_posture").is_none())
                    {
                        return Err(Error::Invalid("public D1 relation semantic mapping/evidence"));
                    }
                    if string(value, "epistemic.review_posture") == Some("not-recorded") {
                        push_gap(capture, &mut gaps, &mut live_gap_bytes, id, "review-not-recorded", state)?;
                    }
                    let from_id = string(value, "from_id")
                        .ok_or(Error::Invalid("public D1 relation from"))?;
                    let to_id = string(value, "to_id")
                        .ok_or(Error::Invalid("public D1 relation to"))?;
                    let left = node_identity(&mut identity_lookup, capture, from_id, state)?;
                    let right = node_identity(&mut identity_lookup, capture, to_id, state)?;
                    endpoint_contract(
                        left.as_ref(),
                        right.as_ref(),
                        relation_type,
                        fallback_relation,
                        entry,
                        &entities,
                        capture,
                        state,
                        hierarchy_edges,
                    )?;
                    let claim_ref = string(value, "attributes.claim_ref").unwrap_or("");
                    if string(entry, "assertion_mode") == Some("reified-claim") {
                        let supporting = supporting_claim(
                            &mut supporting_lookup,
                            capture,
                            claim_ref,
                            state,
                        )?
                        .ok_or(Error::Invalid("public D1 unresolved supporting Claim"))?;
                        if left.as_ref().is_some_and(|left| left.type_id != "tos.entity.claim") {
                            let endpoints_match = if let Some(state) = state {
                                with_node_value(db, layout, &mut node_lookup, capture, &supporting.id, Some(state), |claim_node| {
                                    Ok(left.as_ref().is_some_and(|left| {
                                        left.entity_id == string(claim_node, "semantics.claim.subject_entity_id").unwrap_or("")
                                    }) && right.as_ref().is_some_and(|right| {
                                        right.entity_id == string(claim_node, "semantics.claim.object_entity_id").unwrap_or("")
                                    }))
                                })?.unwrap_or(false)
                            } else {
                                let claim_node = node(&mut node_lookup, capture, &supporting.id)?
                                    .ok_or(Error::Invalid("public D1 missing supporting Claim"))?;
                                left.as_ref().map(|item| item.entity_id.as_str())
                                    == string(&claim_node, "semantics.claim.subject_entity_id")
                                    && right.as_ref().map(|item| item.entity_id.as_str())
                                        == string(&claim_node, "semantics.claim.object_entity_id")
                            };
                            if !endpoints_match {
                                return Err(Error::Invalid("public D1 supporting Claim endpoints"));
                            }
                        }
                    }
                    if relation_type == "tos.relation.projects"
                        && left.as_ref().map(|item| item.entity_id.as_str())
                            != right.as_ref().map(|item| item.entity_id.as_str())
                    {
                        return Err(Error::Invalid("public D1 projection entity identity"));
                    }
                    if relation_type == "tos.relation.same-as" {
                        let left = left.as_ref().ok_or(Error::Invalid("public D1 same-as left"))?;
                        let right = right.as_ref().ok_or(Error::Invalid("public D1 same-as right"))?;
                        if !(is_a_one(&left.type_id, &right.type_id, &entities, capture, state, hierarchy_edges)?
                            || is_a_one(&right.type_id, &left.type_id, &entities, capture, state, hierarchy_edges)?)
                            || !matches!(string(value, "epistemic.review_posture"), Some("accepted" | "verified" | "reviewed_equivalence"))
                            || strings(value, "source_refs").next().is_none()
                        {
                            return Err(Error::Invalid("public D1 same-as evidence/review"));
                        }
                        let review_id = string(value, "attributes.review_node_id")
                            .ok_or(Error::Invalid("public D1 same-as review ID"))?;
                        let supporting = supporting_claim(
                            &mut supporting_lookup,
                            capture,
                            claim_ref,
                            state,
                        )?
                        .ok_or(Error::Invalid("public D1 same-as Claim"))?;
                        if let Some(state) = state {
                            let claim_summary = with_node_value(db, layout,
                                &mut node_lookup,
                                capture,
                                &supporting.id,
                                Some(state),
                                |claim_node| {
                                    let claim = member(claim_node, "semantics.claim");
                                    if string(claim, "relation_type_id") != Some(relation_type)
                                        || member(claim, "claim_version").is_null()
                                        || strings(claim, "evidence_node_ids").next().is_none()
                                    {
                                        return Err(Error::Invalid("public D1 same-as exact Claim"));
                                    }
                                    let pair = [left.entity_id.as_str(), right.entity_id.as_str()];
                                    let expected = [
                                        string(claim, "subject_entity_id"),
                                        string(claim, "object_entity_id"),
                                    ];
                                    if !((expected[0] == Some(pair[0]) && expected[1] == Some(pair[1]))
                                        || (expected[0] == Some(pair[1]) && expected[1] == Some(pair[0])))
                                    {
                                        return Err(Error::Invalid("public D1 same-as exact Claim"));
                                    }
                                    for evidence in strings(claim, "evidence_node_ids") {
                                        state.active()?;
                                        let identity = node_identity(
                                            &mut identity_lookup,
                                            capture,
                                            evidence,
                                            Some(state),
                                        )?
                                        .ok_or(Error::Invalid("public D1 same-as evidence"))?;
                                        if !is_a_one(
                                            &identity.type_id,
                                            "tos.entity.evidence",
                                            &entities,
                                            capture,
                                            Some(state),
                                            hierarchy_edges,
                                        )? {
                                            return Err(Error::Invalid("public D1 same-as evidence type"));
                                        }
                                    }
                                    owned_claim_summary(state, claim)
                                },
                            )?
                            .ok_or(Error::Invalid("public D1 same-as Claim node"))?;
                            let review_valid = with_node_value(db, layout,
                                &mut node_lookup,
                                capture,
                                review_id,
                                Some(state),
                                |review| {
                                    let review_type = string(review, "type_id").unwrap_or("");
                                    let registered = is_a_one(
                                        review_type,
                                        "tos.entity.review",
                                        &entities,
                                        capture,
                                        Some(state),
                                        hierarchy_edges,
                                    )?;
                                    Ok(registered
                                        && string(review, "attributes.claim_ref")
                                            == claim_summary.claim_id.as_deref()
                                        && member(review, "attributes.claim_version")
                                            == &claim_summary.claim_version
                                        && string(review, "attributes.decision") == Some("accepted"))
                                },
                            )?
                            .unwrap_or(false);
                            if !review_valid {
                                return Err(Error::Invalid("public D1 same-as exact Claim"));
                            }
                        } else {
                            let review = node(&mut node_lookup, capture, review_id)?
                                .ok_or(Error::Invalid("public D1 same-as review"))?;
                            let claim_node = node(&mut node_lookup, capture, &supporting.id)?
                                .ok_or(Error::Invalid("public D1 same-as Claim node"))?;
                            let claim = member(&claim_node, "semantics.claim");
                            let pair = [left.entity_id.as_str(), right.entity_id.as_str()];
                            let expected = [string(claim, "subject_entity_id"), string(claim, "object_entity_id")];
                            if !is_a_one(string(&review, "type_id").unwrap_or(""), "tos.entity.review", &entities, capture, None, hierarchy_edges)?
                                || string(&review, "attributes.claim_ref") != string(claim, "claim_id")
                                || member(&review, "attributes.claim_version") != member(claim, "claim_version")
                                || string(&review, "attributes.decision") != Some("accepted")
                                || string(claim, "relation_type_id") != Some(relation_type)
                                || member(claim, "claim_version").is_null()
                                || strings(claim, "evidence_node_ids").next().is_none()
                                || !((expected[0] == Some(pair[0]) && expected[1] == Some(pair[1]))
                                    || (expected[0] == Some(pair[1]) && expected[1] == Some(pair[0])))
                            {
                                return Err(Error::Invalid("public D1 same-as exact Claim"));
                            }
                            for evidence in strings(claim, "evidence_node_ids") {
                                let identity = node_identity(&mut identity_lookup, capture, evidence, None)?
                                    .ok_or(Error::Invalid("public D1 same-as evidence"))?;
                                if !is_a_one(&identity.type_id, "tos.entity.evidence", &entities, capture, None, hierarchy_edges)? {
                                    return Err(Error::Invalid("public D1 same-as evidence type"));
                                }
                            }
                        }
                    }
                    if relation_type == "tos.relation.promotion-basis-version" {
                        let valid = if let Some(state) = state {
                            let candidate_digest = with_node_value(db, layout,
                                &mut node_lookup,
                                capture,
                                from_id,
                                Some(state),
                                |left| {
                                    let candidate = member(left, "attributes.source_record.promotion_basis.candidate");
                                    if !exact_ref(candidate) {
                                        return Err(Error::Invalid("public D1 promotion basis exact version"));
                                    }
                                    exact_record_digest_owned(capture, state, candidate)
                                },
                            )?
                            .ok_or(Error::Invalid("public D1 promotion Sign"))?;
                            let reference_digest = with_node_value(db, layout,
                                &mut node_lookup,
                                capture,
                                to_id,
                                Some(state),
                                |right| {
                                    let reference = member(right, "semantics.record_version.record_ref");
                                    if !exact_ref(reference) {
                                        return Err(Error::Invalid("public D1 promotion basis exact version"));
                                    }
                                    exact_record_digest_owned(capture, state, reference)
                                },
                            )?
                            .ok_or(Error::Invalid("public D1 promotion Version"))?;
                            candidate_digest.as_bytes() == reference_digest.as_bytes()
                        } else {
                            let left = node(&mut node_lookup, capture, from_id)?.ok_or(Error::Invalid("public D1 promotion Sign"))?;
                            let right = node(&mut node_lookup, capture, to_id)?.ok_or(Error::Invalid("public D1 promotion Version"))?;
                            let candidate = member(&left, "attributes.source_record.promotion_basis.candidate");
                            let reference = member(&right, "semantics.record_version.record_ref");
                            exact_ref(candidate) && exact_ref(reference)
                                && exact_record_matches(capture, None, candidate, reference)?
                        };
                        if !valid {
                            return Err(Error::Invalid("public D1 promotion basis exact version"));
                        }
                    }
                    if relation_type == "tos.relation.has-record-version" {
                        if let Some(state) = state {
                            let reference_digest = with_node_value(db, layout,
                                &mut node_lookup,
                                capture,
                                to_id,
                                Some(state),
                                |right| {
                                    let reference = member(right, "semantics.record_version.record_ref");
                                    if string(right, "semantics.record_version.record_kind") != Some("metadata")
                                        || !exact_ref(reference)
                                    {
                                        return Err(Error::Invalid("public D1 record history reference"));
                                    }
                                    exact_record_digest_owned(capture, state, reference)
                                },
                            )?
                            .ok_or(Error::Invalid("public D1 record Version"))?;
                            let found = with_node_value(db, layout,
                                &mut node_lookup,
                                capture,
                                from_id,
                                Some(state),
                                |left| {
                                    let history = metadata_history_refs(capture, left, Some(state))?;
                                    for candidate in history {
                                        state.active()?;
                                        if exact_ref(candidate)
                                            && exact_record_digest_owned(capture, state, candidate)?.as_bytes()
                                                == reference_digest.as_bytes()
                                        {
                                            return Ok(true);
                                        }
                                    }
                                    Ok(false)
                                },
                            )?
                            .ok_or(Error::Invalid("public D1 record source"))?;
                            if !found {
                                return Err(Error::Invalid("public D1 record history exact member"));
                            }
                        } else {
                            let right = node(&mut node_lookup, capture, to_id)?.ok_or(Error::Invalid("public D1 record Version"))?;
                            let reference = member(&right, "semantics.record_version.record_ref");
                            if string(&right, "semantics.record_version.record_kind") != Some("metadata") || !exact_ref(reference) {
                                return Err(Error::Invalid("public D1 record history reference"));
                            }
                            let left = node(&mut node_lookup, capture, from_id)?.ok_or(Error::Invalid("public D1 record source"))?;
                            let history = metadata_history_refs(capture, &left, None)?;
                            let reference_digest = exact_record_digest(capture, reference)?;
                            let mut found = false;
                            for candidate in history {
                                if exact_ref(candidate)
                                    && exact_record_digest(capture, candidate)? == reference_digest
                                {
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                return Err(Error::Invalid("public D1 record history exact member"));
                            }
                        }
                    }
                    // Only the verified scalar assertion context is retained.
                    // A missing/null Claim ref and an actual string are distinct;
                    // other types cannot satisfy the supporting-Claim rule above.
                    let scope = if string(entry, "assertion_mode") == Some("reified-claim") {
                        match member(value, "attributes.claim_ref") {
                            Value::Null => None,
                            Value::String(reference) => Some(reference.as_str()),
                            _ => return Err(Error::Invalid("public D1 Claim assertion scope")),
                        }
                    } else {
                        None
                    };
                    for (direction, endpoint, maximum) in [
                        (0, from_id, "per_subject_max"),
                        (1, to_id, "per_object_max"),
                    ] {
                        if let Some(state) = state {
                            state.active()?;
                        }
                        let maximum = if maximum == "per_subject_max" {
                            member(entry, "cardinality.per_subject_max")
                        } else {
                            member(entry, "cardinality.per_object_max")
                        };
                        if maximum.as_u64().is_some() {
                            record_cardinality(
                                &mut incidence_insert,
                                capture,
                                direction,
                                endpoint,
                                relation_type,
                                scope,
                                source_order,
                                state,
                            )?;
                        }
                    }
                    Ok(())
                };
                if let Some(state) = state {
                    let len = row.get::<_, i64>(0)?;
                    if len < 0 {
                        return Err(Error::Invalid("public D1 semantic row length"));
                    }
                    let len = usize::try_from(len)
                        .map_err(|_| Error::Budget("public D1 semantic row bytes"))?;
                    if len > MAX_ROW_BYTES {
                        return Err(Error::Budget("public D1 semantic row bytes"));
                    }
                    let digest = sql_blob_ref(row, 1)?;
                    let raw = sql_blob_ref(row, 2)?;
                    let stored_type = sql_text_ref(row, 3)?;
                    let stored_from = sql_text_ref(row, 4)?;
                    let stored_to = sql_text_ref(row, 5)?;
                    let stored_source = sql_text_ref(row, 6)?;
                    let source_order = row.get::<_, i64>(7)?;
                    if layout == KnowledgePayloadLayout::InlineV1 && len != raw.len() {
                        return Err(Error::Invalid("public D1 semantic row length"));
                    }
                    capture.charge_work(
                        (stored_type.len() + stored_from.len() + stored_to.len() + stored_source.len()) as u64,
                    )?;
                    with_semantic_physical_row_owned(db, row, layout, 8, state, |value| {
                        process(value, stored_type, stored_from, stored_to, stored_source, source_order)
                    })?;
                } else {
                    let len = admitted_row_len(capture, row.get(0)?)?;
                    let digest: Vec<u8> = row.get(1)?;
                    let value = check_row(len, &digest, row.get(2)?)?;
                    let stored_type: String = row.get(3)?;
                    let stored_from: String = row.get(4)?;
                    let stored_to: String = row.get(5)?;
                    let stored_source: String = row.get(6)?;
                    let source_order: i64 = row.get(7)?;
                    process(
                        &value,
                        &stored_type,
                        &stored_from,
                        &stored_to,
                        &stored_source,
                        source_order,
                    )?;
                }
            }
        }
        drop(incidence_insert);
        // Python checks outgoing groups first, then incoming groups, each in
        // first-observed order. The private order index avoids a late sorter.
        {
            let mut counts = db.prepare("SELECT length(CAST(endpoint AS BLOB)),
                length(CAST(relation_type AS BLOB)),direction,relation_type,tally
                FROM d1_semantic_cardinality ORDER BY direction,first_order")?;
            let mut rows = counts.query([])?;
            while let Some(row) = rows.next()? {
                if let Some(state) = state {
                    state.active()?;
                }
                let endpoint_len: i64 = row.get(0)?;
                let relation_len: i64 = row.get(1)?;
                if endpoint_len < 0 || relation_len < 0 {
                    return Err(Error::Invalid("public D1 cardinality key length"));
                }
                let endpoint_len = usize::try_from(endpoint_len)
                    .map_err(|_| Error::Budget("public D1 cardinality key bytes"))?;
                let relation_len = usize::try_from(relation_len)
                    .map_err(|_| Error::Budget("public D1 cardinality key bytes"))?;
                let output_len = endpoint_len
                    .checked_add(relation_len)
                    .ok_or(Error::Budget("public D1 cardinality key bytes"))?;
                if output_len > MAX_ROW_BYTES {
                    return Err(Error::Budget("public D1 cardinality key bytes"));
                }
                capture.charge_work((output_len as u64).checked_mul(2)
                    .and_then(|n| n.checked_add(32))
                    .ok_or(Error::Budget("public D1 cardinality group work"))?)?;
                let direction: i64 = row.get(2)?;
                let relation_type = sql_text_ref(row, 3)?;
                let tally: i64 = row.get(4)?;
                if (direction != 0 && direction != 1) || tally < 1 {
                    return Err(Error::Invalid("public D1 cardinality summary"));
                }
                let entry = relations.get(relation_type)
                    .ok_or(Error::Invalid("public D1 cardinality relation type"))?;
                let maximum = member(entry, if direction == 0 {
                    "cardinality.per_subject_max"
                } else {
                    "cardinality.per_object_max"
                })
                    .as_u64()
                    .ok_or(Error::Invalid("public D1 cardinality maximum"))?;
                if tally as u64 > maximum {
                    return Err(Error::Invalid("public D1 scoped relation cardinality"));
                }
            }
        }
        db.execute_batch("DROP TABLE d1_semantic_cardinality")?;
        Ok(())
    })?;
    let report_work = live_gap_bytes.saturating_add(256);
    if let Some(state) = state {
        state.charge_work(report_work)?;
    } else {
        capture.charge_work(report_work as u64)?;
    }
    if let Some(state) = state {
        state.active()?;
    }
    gaps.rotate_left(claim_gap_count);
    if let Some(state) = state {
        const REPORT_KEYS: &[&str] = &[
            "valid",
            "violations",
            "registered_node_count",
            "unmapped_node_count",
            "registered_relation_count",
            "unmapped_relation_count",
            "claim_contract_count",
            "cross_layer_relation_count",
            "gaps",
        ];
        let report_bytes =
            crate::knowledge_normalization::serde_object_slots_upper(REPORT_KEYS.len())?
                .checked_add(REPORT_KEYS.iter().map(|key| key.len()).sum::<usize>())
                .ok_or(Error::Budget("public D1 semantic report state"))?;
        state.retain(report_bytes)?;
    }
    let mut report = serde_json::Map::new();
    report.insert("valid".into(), Value::Bool(true));
    report.insert("violations".into(), Value::Array(Vec::new()));
    for (name, count) in [
        ("registered_node_count", registered_nodes),
        ("unmapped_node_count", unmapped_nodes),
        ("registered_relation_count", registered_relations),
        ("unmapped_relation_count", unmapped_relations),
        ("claim_contract_count", claim_count),
        ("cross_layer_relation_count", cross_layer),
    ] {
        if let Some(state) = state {
            state.active()?;
        }
        report.insert(name.into(), Value::from(count));
    }
    report.insert("gaps".into(), Value::Array(gaps));
    Ok(Value::Object(report))
}

#[cfg(test)]
mod report_tests {
    use super::*;
    #[test]
    fn report_width_counts_real_json_escaping_and_keeps_all_gaps() {
        for id in ["philosophy:edge:example", "\0\n\r\t\u{8}\u{c}\"\\", "Русский Größe 🦉\u{2028}"] {
            let gap = json!({"id":id,"kind":"review-not-recorded"});
            assert_eq!(semantic_gap_encoded_len(id, "review-not-recorded").unwrap(),
                serde_json::to_vec(&gap).unwrap().len() + 1);
        }
        let mut bytes = 256usize;
        let mut old_bound = 256usize;
        for i in 0..11_692 {
            let id = format!("philosophy:edge:candidate-relation:table-i-a001-relation-{i:05}");
            bytes += semantic_gap_encoded_len(&id, "review-not-recorded").unwrap();
            old_bound += (id.len() + "review-not-recorded".len()) * 6 + 64;
        }
        assert!(bytes > 1024 * 1024);
        assert!(bytes < crate::knowledge_seal::MAX_GRAPH_HEADER_BYTES);
        assert!(old_bound > crate::knowledge_seal::MAX_GRAPH_HEADER_BYTES);
    }
}
