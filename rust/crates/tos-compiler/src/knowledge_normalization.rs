//! Exact bounded primitives shared by owner-source knowledge adapters.
//!
//! These reproduce selected helpers in the frozen Python `knowledge.py`.
//! They do not normalize a complete node or relation, admit a source, or
//! publish a graph. In particular, a caller must still derive display,
//! semantics, registry mapping, endpoint titles and owner placeholders.

use crate::{Error, Result};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, io::Write};
use tos_foundation::{Digest256Hasher, JsonLimits, JsonMode, parse_json};

const MAX_SOURCE_ROW_BYTES: usize = 8 * 1024 * 1024;
const MAX_JSON_VISITS: usize = 1_000_000;
const MAX_JSON_DEPTH: usize = 96;
const MAX_INTEGER_DIGITS: usize = 4096;
const DEFAULT_SOURCE_REF: &str = "ToS/source_home.manifest.json";

/// A strict owner carrier parsed under an explicit row-byte ceiling. All
/// methods below operate on this bounded tree; no path is used as identity.
pub struct SourceRow {
    value: Value,
    max_bytes: usize,
}

/// Actual decoded row owns its admission until the row drops. The source tree
/// drops before its guard, including error unwinding. Escaping output copies
/// still need their own admission under the same original owner.
pub(crate) struct OwnedSourceRow<'s,'budget> {
    row:SourceRow,
    _hold:Option<crate::d1_public_capture::CreationStateHold<'s,'budget>>,
}
impl std::ops::Deref for OwnedSourceRow<'_,'_> {
    type Target=SourceRow;
    fn deref(&self)->&SourceRow { &self.row }
}

struct CappedWriter {
    bytes: usize,
    ceiling: usize,
}

impl Write for CappedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len())
            .filter(|next| *next <= self.ceiling)
            .ok_or_else(|| std::io::Error::other("normalization JSON byte ceiling"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn check_serialized(value: &Value, ceiling: usize) -> Result<()> {
    let mut writer = CappedWriter { bytes: 0, ceiling };
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::Budget("normalization JSON bytes"))
}

pub(crate) fn write_len(hasher: &mut Digest256Hasher, len: usize) {
    hasher.update(len.to_string().as_bytes());
}

pub(crate) fn write_string(hasher: &mut Digest256Hasher, value: &str) {
    hasher.update(b"s");
    write_len(hasher, value.len());
    hasher.update(b":");
    hasher.update(value.as_bytes());
}

/// Python `_stable_digest`: sorted Unicode object keys; UTF-8 string length;
/// every JSON number coerced to finite binary64 and emitted as its big-endian
/// hexadecimal bits. Null, boolean, arrays and objects retain distinct tags.
pub(crate) fn stable_digest_value(value: &Value, hasher: &mut Digest256Hasher) -> Result<()> {
    match value {
        Value::Null => hasher.update(b"n;"),
        Value::Bool(boolean) => hasher.update(if *boolean { b"b1;" } else { b"b0;" }),
        Value::Number(number) => {
            let mut float = number
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or(Error::Invalid("non-finite stable digest number"))?;
            if float == 0.0 {
                float = 0.0;
            }
            hasher.update(b"d");
            for byte in float.to_be_bytes() {
                hasher.update(format!("{byte:02x}").as_bytes());
            }
            hasher.update(b";");
        }
        Value::String(string) => write_string(hasher, string),
        Value::Array(items) => {
            hasher.update(b"a");
            write_len(hasher, items.len());
            hasher.update(b"[");
            for item in items {
                stable_digest_value(item, hasher)?;
            }
            hasher.update(b"]");
        }
        Value::Object(items) => {
            hasher.update(b"o");
            write_len(hasher, items.len());
            hasher.update(b"{");
            // `serde_json::Map` is sorted without `preserve_order`, but sort
            // explicitly to keep this contract independent of that feature.
            let mut keys = items.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                write_string(hasher, key);
                stable_digest_value(
                    items
                        .get(key)
                        .ok_or(Error::Invalid("stable digest object key"))?,
                    hasher,
                )?;
            }
            hasher.update(b"}");
        }
    }
    Ok(())
}

pub(crate) fn stable_digest(value: &Value) -> Result<String> {
    let mut hasher = Digest256Hasher::new();
    stable_digest_value(value, &mut hasher)?;
    Ok(hasher.finalize().to_hex())
}

fn string(value: Option<&Value>) -> Option<&str> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn pointer_escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

// Pinned serde_json1.0.151: arrays grow geometrically. Under preserve_order,
// IndexMap2.14.2 uses a Bucket(hash,key,value) vector plus hashbrown0.17.1
// indices/control bytes; without it, BTreeMap tree nodes are covered by the
// same conservative envelope. Factors include old/new allocation overlap.
// This is a logical admitted peak bound, not an allocator/RSS measurement.
pub(crate) fn serde_array_slots_upper(n: usize) -> Result<usize> {
    n.max(4).checked_mul(4 * std::mem::size_of::<Value>())
        .ok_or(Error::Budget("raw input serde array workspace"))
}
// Source-owned pinned container geometry: IndexMap Bucket stores hash/key/
// value plus hashbrown indices/control. Eight slots include geometric old/new
// overlap; non-preserve_order uses the existing sixteen-slot BTree envelope.
// This remains a logical admission estimate, not measured allocator/RSS.
/// Preadmit one normalizer output while its consumer encodes/inserts it.
/// The family owner supplies a documented maximum of actual source/subtree
/// copies and the exact additional key/string/nested-container geometry.
/// The same original work/check precedes each forecast source-copy traversal;
/// this is an accounting bound, not a measured allocator/RSS assertion.
pub(crate) fn hold_owned_normalized_output<'s,'budget>(
    state:&'s crate::d1_public_capture::CreationState<'budget>,
    source_values:&[&Value],full_source_copies:usize,
    root_object_fields:usize,extra_heap_bytes:usize,
)->Result<crate::d1_public_capture::CreationStateHold<'s,'budget>> {
    state.active()?;
    let mut total=serde_object_slots_upper(root_object_fields)?
        .checked_add(root_object_fields.checked_mul(std::mem::size_of::<Value>())
            .ok_or(Error::Budget("owned normalized root slots"))?)
        .and_then(|n|n.checked_add(extra_heap_bytes))
        .ok_or(Error::Budget("owned normalized additional state"))?;
    for value in source_values {
        for _ in 0..full_source_copies {
            state.active()?;
            total=total.checked_add(state.value_clone_state_upper_bound(value)?)
                .ok_or(Error::Budget("owned normalized source copies"))?;
        }
    }
    state.hold(total)
}

/// Admit separately documented copy destinations for each actual input tree.
/// Each planning traversal consumes the same original work/check ledger.
pub(crate) fn hold_owned_normalized_output_with_copy_counts<'s,'budget>(
    state: &'s crate::d1_public_capture::CreationState<'budget>,
    source_values: &[(&Value, usize)], root_object_fields: usize,
    extra_heap_bytes: usize,
) -> Result<crate::d1_public_capture::CreationStateHold<'s,'budget>> {
    state.active()?;
    let mut total = serde_object_slots_upper(root_object_fields)?
        .checked_add(root_object_fields.checked_mul(std::mem::size_of::<Value>())
            .ok_or(Error::Budget("owned normalized root slots"))?)
        .and_then(|n| n.checked_add(extra_heap_bytes))
        .ok_or(Error::Budget("owned normalized additional state"))?;
    for (value, copies) in source_values {
        for _ in 0..*copies {
            state.active()?;
            total = total.checked_add(state.value_clone_state_upper_bound(value)?)
                .ok_or(Error::Budget("owned normalized source copies"))?;
        }
    }
    state.hold(total)
}

pub(crate) fn serde_object_slots_upper(n: usize) -> Result<usize> {
    let entry = std::mem::size_of::<(String, Value)>()
        + 2 * std::mem::size_of::<usize>() + 1;
    let factor = if cfg!(all(feature = "preserve-order-workspace",
        target_pointer_width = "64", target_arch = "x86_64")) { 8 } else { 16 };
    n.max(4).checked_mul(factor * entry)
        .ok_or(Error::Budget("raw input serde object workspace"))
}
pub(crate) fn serde_text_workspace_upper(units: usize) -> Result<usize> {
    units.checked_mul(12).and_then(|n| n.checked_add(64))
        .ok_or(Error::Budget("raw input serde string workspace"))
}

pub(crate) fn serde_input_workspace_upper(
    root: &tos_foundation::JsonValue,
    raw_bytes: usize,
) -> Result<usize> {
    fn add(total: &mut usize, n: usize) -> Result<()> {
        *total = total
            .checked_add(n)
            .ok_or(Error::Budget("raw input serde workspace"))?;
        Ok(())
    }
    fn slots(n: usize, width: usize) -> Result<usize> {
        n.checked_mul(width)
            .ok_or(Error::Budget("raw input serde workspace"))
    }
    fn text(total: &mut usize, units: usize) -> Result<()> {
        // <=3 UTF8 bytes per UTF16 unit; geometric string growth and reallocation.
        add(total, serde_text_workspace_upper(units)?)
    }
    fn walk(v: &tos_foundation::JsonValue, depth: usize, total: &mut usize) -> Result<()> {
        use tos_foundation::JsonValue;
        if depth > MAX_JSON_DEPTH {
            return Err(Error::Budget("raw input serde depth"));
        }
        add(total, std::mem::size_of::<Value>())?;
        match v {
            JsonValue::String(s) => text(total, s.units().len())?,
            JsonValue::Number(n) => {
                add(total, slots(n.lexeme.len(), 4)?)?;
                add(total, 128)?;
            }
            JsonValue::Array(a) => {
                add(
                    total,
                    serde_array_slots_upper(a.len())?,
                )?;
                for child in a {
                    walk(child, depth + 1, total)?;
                }
            }
            JsonValue::Object(o) => {
                add(total, std::mem::size_of::<Map<String, Value>>())?;
                add(total, serde_object_slots_upper(o.len())?)?;
                for (key, child) in o {
                    text(total, key.units().len())?;
                    walk(child, depth + 1, total)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    // Deserializer escaped-string/numeric scratch plus bounded recursive frames.
    let mut total = slots(raw_bytes, 8)?;
    add(
        &mut total,
        slots(
            MAX_JSON_DEPTH + 1,
            std::mem::size_of::<Value>() + std::mem::size_of::<tos_foundation::JsonValue>() + 512,
        )?,
    )?;
    walk(root, 0, &mut total)?;
    Ok(total)
}

impl SourceRow {
    /// Maintained normalized row grammar shared by Stage logical delivery.
    /// The caller intersects these limits with original remaining visits/state.
    pub(crate) fn json_limits(max_bytes: usize) -> Result<JsonLimits> {
        JsonLimits::new(max_bytes, MAX_JSON_DEPTH, MAX_JSON_VISITS, MAX_INTEGER_DIGITS)
            .map_err(|_| Error::Budget("normalization JSON limits"))
    }
    pub(crate) fn parse_scoped_with_optional_owned_state<'s,'budget>(raw:&[u8],max_bytes:usize,
        state:Option<&'s crate::d1_public_capture::CreationState<'budget>>) -> Result<OwnedSourceRow<'s,'budget>> {
        match state {
            Some(state)=>Self::parse_scoped_with_owned_state(raw,max_bytes,state),
            None=>Ok(OwnedSourceRow {row:Self::parse(raw,max_bytes)?,_hold:None}),
        }
    }
    pub(crate) fn parse_scoped_with_owned_state<'s,'budget>(raw:&[u8],max_bytes:usize,
        state:&'s crate::d1_public_capture::CreationState<'budget>) -> Result<OwnedSourceRow<'s,'budget>> {
        if max_bytes==0 || max_bytes>MAX_SOURCE_ROW_BYTES || raw.len()>max_bytes {
            return Err(Error::Budget("normalization source row bytes"));
        }
        let limits=JsonLimits::new(max_bytes,MAX_JSON_DEPTH,MAX_JSON_VISITS,MAX_INTEGER_DIGITS)
            .map_err(|_|Error::Budget("normalization JSON limits"))?;
        let (value,hold)=state.serde_scoped_with_limits(raw,limits)?;
        if !value.is_object() {return Err(Error::Invalid("normalization source object"));}
        Ok(OwnedSourceRow {row:Self {value,max_bytes},_hold:Some(hold)})
    }
    pub(crate) fn with_owned_state<T>(raw: &[u8], max_bytes: usize,
        state: &crate::d1_public_capture::CreationState<'_>,
        operation: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        if max_bytes == 0 || max_bytes > MAX_SOURCE_ROW_BYTES || raw.len() > max_bytes {
            return Err(Error::Budget("normalization source row bytes"));
        }
        let limits = JsonLimits::new(max_bytes, MAX_JSON_DEPTH, MAX_JSON_VISITS, MAX_INTEGER_DIGITS)
            .map_err(|_| Error::Budget("normalization JSON limits"))?;
        state.with_serde_owned_value_with_limits(raw, limits, |value| {
            if !value.is_object() { return Err(Error::Invalid("normalization source object")); }
            let source = Self { value, max_bytes };
            operation(&source)
        })
    }
    pub(crate) fn parse_with_owned_state(raw: &[u8], max_bytes: usize,
        state: &crate::d1_public_capture::CreationState<'_>) -> Result<Self> {
        if max_bytes == 0 || max_bytes > MAX_SOURCE_ROW_BYTES || raw.len() > max_bytes {
            return Err(Error::Budget("normalization source row bytes"));
        }
        let value = state.serde_owned(raw, max_bytes)?;
        if !value.is_object() { return Err(Error::Invalid("normalization source object")); }
        Ok(Self { value, max_bytes })
    }

    pub fn parse(raw: &[u8], max_bytes: usize) -> Result<Self> {
        if max_bytes == 0 || max_bytes > MAX_SOURCE_ROW_BYTES || raw.len() > max_bytes {
            return Err(Error::Budget("normalization source row bytes"));
        }
        Self::parse_inner(raw, max_bytes, false)
    }
    /// Computational raw input only; does not widen normalized row semantics.
    pub(crate) fn parse_raw_input(raw: &[u8], max_bytes: usize) -> Result<Self> {
        if max_bytes == 0 || max_bytes > 64 * 1024 * 1024 || raw.len() > max_bytes {
            return Err(Error::Budget("raw input source bytes"));
        }
        Self::parse_inner(raw, max_bytes, true)
    }
    /// Same strict grammar under a caller-owned temporary workspace ceiling.
    /// The metered FND tree is dropped before the unchanged serde decode.
    pub(crate) fn parse_raw_input_with_state_budget(
        raw: &[u8],
        max_bytes: usize,
        available: usize,
    ) -> Result<(Self, usize)> {
        if max_bytes == 0 || max_bytes > 64 * 1024 * 1024 || raw.len() > max_bytes {
            return Err(Error::Budget("raw input source bytes"));
        }
        let limits = JsonLimits::new(
            max_bytes,
            MAX_JSON_DEPTH,
            MAX_JSON_VISITS,
            MAX_INTEGER_DIGITS,
        )
        .map_err(|_| Error::Budget("normalization JSON limits"))?;
        let document = tos_foundation::parse_json_with_state_budget(
            raw,
            JsonMode::PublishedStrict,
            limits,
            available,
        )
        .map_err(|error| {
            if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                Error::Budget("raw input JSON workspace")
            } else {
                Error::Source(error.to_string())
            }
        })?;
        let upper = serde_input_workspace_upper(document.root(), raw.len())?;
        if upper > available {
            return Err(Error::Budget("raw input serde workspace"));
        }
        drop(document);
        let value: Value =
            serde_json::from_slice(raw).map_err(|_| Error::Invalid("normalization source JSON"))?;
        if !value.is_object() {
            return Err(Error::Invalid("normalization source object"));
        }
        Ok((Self { value, max_bytes }, upper))
    }
    fn parse_inner(raw: &[u8], max_bytes: usize, classify_budget: bool) -> Result<Self> {
        let limits = JsonLimits::new(
            max_bytes,
            MAX_JSON_DEPTH,
            MAX_JSON_VISITS,
            MAX_INTEGER_DIGITS,
        )
        .map_err(|_| Error::Budget("normalization JSON limits"))?;
        parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|error| {
            if classify_budget && error.code == tos_foundation::FoundationErrorCode::BudgetExceeded
            {
                Error::Budget("raw input JSON limits")
            } else {
                Error::Source(error.to_string())
            }
        })?;
        let value: Value =
            serde_json::from_slice(raw).map_err(|_| Error::Invalid("normalization source JSON"))?;
        if !value.is_object() {
            return Err(Error::Invalid("normalization source object"));
        }
        Ok(Self { value, max_bytes })
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn stable_digest(&self) -> Result<String> {
        stable_digest(&self.value)
    }

    /// Python `_source_refs`: source_refs string members retain their bytes;
    /// named path fields are trimmed, then all references sort and dedupe.
    pub fn source_refs(&self, fallbacks: &[&str]) -> Vec<String> {
        let mut refs = BTreeSet::new();
        if let Some(values) = self.value.get("source_refs").and_then(Value::as_array) {
            for value in values {
                if let Some(text) = value.as_str().filter(|text| !text.is_empty()) {
                    refs.insert(text.to_owned());
                }
            }
        }
        for key in ["source_ref", "source_path", "path", "owner_surface"] {
            if let Some(text) = string(self.value.get(key)) {
                refs.insert(text.to_owned());
            }
        }
        refs.extend(
            fallbacks
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| (*s).to_owned()),
        );
        if refs.is_empty() {
            refs.insert(DEFAULT_SOURCE_REF.to_owned());
        }
        refs.into_iter().collect()
    }

    /// Python `_source_record` over caller-derived attributes. The payload is
    /// the complete source carrier; field_map points to its original fields.
    pub fn source_record(&self, attributes: &Map<String, Value>) -> Result<Value> {
        let properties = self.value.get("properties").and_then(Value::as_object);
        let mut fields = Map::new();
        for key in attributes.keys() {
            let prefix = if properties.is_some_and(|props| props.contains_key(key)) {
                "/properties/"
            } else {
                "/"
            };
            fields.insert(
                format!("attributes.{key}"),
                Value::String(format!("{prefix}{}", pointer_escape(key))),
            );
        }
        let mut record = Map::new();
        record.insert("payload".into(), self.value.clone());
        record.insert("digest".into(), Value::String(self.stable_digest()?));
        record.insert(
            "transform_version".into(),
            Value::String("tos-knowledge-normalization-v2".into()),
        );
        record.insert("field_map".into(), Value::Object(fields));
        let value = Value::Object(record);
        check_serialized(
            &value,
            self.max_bytes.saturating_mul(4).min(MAX_SOURCE_ROW_BYTES),
        )?;
        Ok(value)
    }
}

/// Python `_stamp_content_revision` over a bounded normalized envelope.
/// Re-stamping excludes the previous revision and restores it on failure.
pub fn stamp_content_revision(value: &mut Value, max_bytes: usize) -> Result<()> {
    if max_bytes == 0 || max_bytes > MAX_SOURCE_ROW_BYTES {
        return Err(Error::Budget("normalization content revision bytes"));
    }
    let object = value
        .as_object_mut()
        .ok_or(Error::Invalid("normalization revision object"))?;
    let previous = object.remove("content_revision");
    let result = (|| {
        check_serialized(value, max_bytes)?;
        stable_digest(value)
    })();
    match result {
        Ok(digest) => {
            let object = value.as_object_mut().expect("checked object");
            object.insert("content_revision".into(), Value::String(digest));
            if let Err(error) = check_serialized(value, max_bytes) {
                let object = value.as_object_mut().expect("checked object");
                object.remove("content_revision");
                if let Some(previous) = previous {
                    object.insert("content_revision".into(), previous);
                }
                return Err(error);
            }
            Ok(())
        }
        Err(error) => {
            if let Some(previous) = previous {
                value
                    .as_object_mut()
                    .expect("checked object")
                    .insert("content_revision".into(), previous);
            }
            Err(error)
        }
    }
}


// Same Python digest grammar as stable_digest_value, with every actual walk,
// comparison, copy and hash admitted under the original owner before action.
fn stable_digest_value_owned(
    value: &Value, hasher: &mut Digest256Hasher,
    state: &crate::d1_public_capture::CreationState<'_>, depth: usize,
) -> Result<()> {
    fn emit(hasher: &mut Digest256Hasher, bytes: &[u8],
        state: &crate::d1_public_capture::CreationState<'_>) -> Result<()> {
        state.charge_work(bytes.len())?;
        hasher.update(bytes);
        Ok(())
    }
    fn length(hasher: &mut Digest256Hasher, mut n: usize,
        state: &crate::d1_public_capture::CreationState<'_>) -> Result<()> {
        state.charge_work(20 + 20)?;
        let mut bytes = [0u8; 20];
        let mut start = bytes.len();
        loop {
            start -= 1;
            bytes[start] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 { break; }
        }
        emit(hasher, &bytes[start..], state)
    }
    fn string(hasher: &mut Digest256Hasher, text: &str,
        state: &crate::d1_public_capture::CreationState<'_>) -> Result<()> {
        emit(hasher, b"s", state)?;
        length(hasher, text.len(), state)?;
        emit(hasher, b":", state)?;
        emit(hasher, text.as_bytes(), state)
    }
    type Entry<'v> = (&'v String, &'v Value);
    fn greater(left: &str, right: &str,
        state: &crate::d1_public_capture::CreationState<'_>) -> Result<bool> {
        state.charge_work(left.len().checked_add(right.len())
            .ok_or(Error::Budget("owned digest comparison work"))?)?;
        Ok(left > right)
    }
    fn sift(entries: &mut [Entry<'_>], mut root: usize, end: usize,
        state: &crate::d1_public_capture::CreationState<'_>) -> Result<()> {
        loop {
            state.active()?;
            let Some(mut child) = root.checked_mul(2).and_then(|n| n.checked_add(1)) else {
                return Err(Error::Budget("owned digest sort index"));
            };
            if child >= end { return Ok(()); }
            if child + 1 < end && greater(entries[child + 1].0, entries[child].0, state)? {
                child += 1;
            }
            if !greater(entries[child].0, entries[root].0, state)? { return Ok(()); }
            state.charge_work(2 * std::mem::size_of::<Entry<'_>>())?;
            entries.swap(root, child);
            root = child;
        }
    }
    // This frame describes the recursive controller and bounded fixed spelling
    // scratch. Key-vector heap storage has its own simultaneous local hold.
    type Frame<'v> = (&'v Value, &'v mut Digest256Hasher, usize,
        Vec<Entry<'v>>, [u8; 20], [u8; 16], Result<()>);
    let _frame = state.hold(std::mem::size_of::<Frame<'_>>())?;
    state.active()?;
    if depth > MAX_JSON_DEPTH { return Err(Error::Budget("owned digest depth")); }
    state.charge_work(std::mem::size_of::<Value>())?;
    match value {
        Value::Null => emit(hasher, b"n;", state)?,
        Value::Bool(v) => emit(hasher, if *v { b"b1;" } else { b"b0;" }, state)?,
        Value::Number(number) => {
            // arbitrary_precision's maintained Number::as_f64 parses this
            // exact borrowed spelling, so admit its scan before conversion.
            state.charge_work(number.as_str().len())?;
            let mut v = number.as_f64().filter(|v| v.is_finite())
                .ok_or(Error::Invalid("non-finite stable digest number"))?;
            if v == 0.0 { v = 0.0; }
            state.charge_work(16 + 16 + 8)?;
            let mut hex = [0u8; 16];
            const DIGITS: &[u8; 16] = b"0123456789abcdef";
            for (i, byte) in v.to_be_bytes().into_iter().enumerate() {
                hex[2 * i] = DIGITS[(byte >> 4) as usize];
                hex[2 * i + 1] = DIGITS[(byte & 15) as usize];
            }
            emit(hasher, b"d", state)?;
            emit(hasher, &hex, state)?;
            emit(hasher, b";", state)?;
        }
        Value::String(text) => string(hasher, text, state)?,
        Value::Array(items) => {
            emit(hasher, b"a", state)?;
            length(hasher, items.len(), state)?;
            emit(hasher, b"[", state)?;
            for item in items {
                state.active()?;
                stable_digest_value_owned(item, hasher, state, depth + 1)?;
            }
            emit(hasher, b"]", state)?;
        }
        Value::Object(items) => {
            let bytes = items.len().checked_mul(std::mem::size_of::<Entry<'_>>())
                .ok_or(Error::Budget("owned digest key workspace"))?;
            let _keys_hold = state.hold(bytes)?;
            let mut entries = Vec::new();
            entries.try_reserve_exact(items.len())
                .map_err(|_| Error::Budget("owned digest key allocation"))?;
            for (key, value) in items {
                state.charge_work(std::mem::size_of::<Entry<'_>>())?;
                entries.push((key, value));
            }
            // Fallible heapsort gives the original owner a checkpoint and
            // honest byte charge before each key comparison and slot swap.
            for root in (0..entries.len()/2).rev() {
                let end = entries.len();
                sift(&mut entries, root, end, state)?;
            }
            for end in (1..entries.len()).rev() {
                state.charge_work(2 * std::mem::size_of::<Entry<'_>>())?;
                entries.swap(0, end);
                sift(&mut entries, 0, end, state)?;
            }
            emit(hasher, b"o", state)?;
            length(hasher, items.len(), state)?;
            emit(hasher, b"{", state)?;
            for (key, value) in &entries {
                string(hasher, key, state)?;
                stable_digest_value_owned(value, hasher, state, depth + 1)?;
            }
            emit(hasher, b"}", state)?;
        }
    }
    state.active()
}

// Price a complete conservative lookup path in the actual Map before
// remove/insert, including this planning pass and every possible key comparison.
// This covers both maintained BTreeMap and preserve_order's hashed map choice.
fn charge_revision_map_lookup(value: &Value,
    state: &crate::d1_public_capture::CreationState<'_>) -> Result<()> {
    let fields = value.as_object().ok_or(Error::Invalid("normalization revision object"))?;
    for key in fields.keys() {
        state.charge_work(key.len().checked_mul(2)
            .and_then(|n| n.checked_add("content_revision".len()))
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, Value)>()))
            .ok_or(Error::Budget("owned revision map comparison work"))?)?;
    }
    state.charge_work("content_revision".len())
}

/// Consume the owned normalized tree through content revision and encoded
/// SQL delivery. Only unit escapes: the tree and temporary revision drop
/// before their admission, including every error and unwinding path. The
/// incoming tree's own existing admission must remain held by the caller.
pub(crate) fn with_content_revision_owned(
    state: &crate::d1_public_capture::CreationState<'_>, value: Value,
    max_bytes: usize, consume: impl FnOnce(&Value, &[u8]) -> Result<()>,
) -> Result<()> {
    if max_bytes == 0 || max_bytes > MAX_SOURCE_ROW_BYTES {
        return Err(Error::Budget("normalization content revision bytes"));
    }
    struct RevisionOwner<'s, 'b> {
        value: Value,
        _hold: crate::d1_public_capture::CreationStateHold<'s, 'b>,
    }
    let extra = serde_object_slots_upper(1)?.checked_add(64 + "content_revision".len())
        .and_then(|n| n.checked_add(std::mem::size_of::<Value>()
            + std::mem::size_of::<Digest256Hasher>()))
        .ok_or(Error::Budget("owned content revision state"))?;
    let hold = state.hold(extra)?;
    let mut owner = RevisionOwner { value, _hold: hold };
    charge_revision_map_lookup(&owner.value, state)?;
    if let Some(previous) = owner.value.get("content_revision") {
        // The previous revision may be any valid serde subtree, not a trusted
        // scalar. Price its actual recursive destruction before Map::remove.
        state.value_clone_state_upper_bound(previous)?;
    }
    owner.value.as_object_mut().ok_or(Error::Invalid("normalization revision object"))?
        .remove("content_revision");
    state.with_json_encoded(&owner.value, max_bytes, |_| Ok(()))?;
    let mut hash = Digest256Hasher::new();
    stable_digest_value_owned(&owner.value, &mut hash, state, 0)?;
    state.charge_work(64 + "content_revision".len())?;
    let digest = hash.finalize().to_hex();
    charge_revision_map_lookup(&owner.value, state)?;
    owner.value.as_object_mut().expect("checked object")
        .insert("content_revision".to_owned(), Value::String(digest));
    state.with_json_encoded(&owner.value, max_bytes,
        |bytes| consume(&owner.value, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Frozen Python oracle: PYTHONPATH=access/src python3, then call
    // tos_access.knowledge._stable_digest/_source_refs/_source_record/
    // _stamp_content_revision on these exact fixture values.
    #[test]
    fn python_oracle_numbers_unicode_and_nested_values() {
        let row = SourceRow::parse(
            r#"{"z":-0.0,"a":[null,true,false,1,1.5,"Élan","a\u2028b"],"obj":{"b":2,"a":"λ"}}"#
                .as_bytes(),
            1024,
        )
        .unwrap();
        assert_eq!(
            row.stable_digest().unwrap(),
            "04690743fdd38d359a6947278fc0359b6fe8a7eed932a2319bc285b7d4723afb"
        );
        assert_eq!(row.source_refs(&[]), vec![DEFAULT_SOURCE_REF]);
    }

    #[test]
    fn python_oracle_source_refs_record_and_revision() {
        let row = SourceRow::parse(
            r#"{"node_id":"tos.work.one","node_kind":"work","label":"Élan","source_ref":" ToS/a.json ","source_refs":["z","a","z",""],"path":" ToS/p.json ","properties":{"a/b":1,"x~y":false}}"#.as_bytes(),
            1024,
        )
        .unwrap();
        assert_eq!(
            row.stable_digest().unwrap(),
            "080c50f6da3ba2ad9c2581b2a364cca9c9de6a8c933bb77914bb76774ee17e50"
        );
        assert_eq!(
            row.source_refs(&[]),
            vec!["ToS/a.json", "ToS/p.json", "a", "z"]
        );
        let attributes = json!({"a/b":1,"x~y":false,"outside":null});
        let record = row.source_record(attributes.as_object().unwrap()).unwrap();
        assert_eq!(record["payload"], *row.value());
        assert_eq!(record["digest"], row.stable_digest().unwrap());
        assert_eq!(
            record["transform_version"],
            "tos-knowledge-normalization-v2"
        );
        assert_eq!(
            record["field_map"],
            json!({
                "attributes.a/b":"/properties/a~1b",
                "attributes.x~y":"/properties/x~0y",
                "attributes.outside":"/outside"
            })
        );
        let mut normalized = json!({"id":"source-navigation:tos.work.one","source_record":record});
        stamp_content_revision(&mut normalized, 2048).unwrap();
        assert_eq!(
            normalized["content_revision"],
            "15894278e3a77618affe4bdff66dfee15a3cdd38f02a95a206fb973e4166b8b6"
        );
    }

    #[test]
    fn strict_source_and_budgets_fail_closed() {
        assert!(SourceRow::parse(br#"{"x":1,"x":2}"#, 1024).is_err());
        assert!(SourceRow::parse(br#"{"x":1}"#, 4).is_err());
        assert!(SourceRow::parse(br#"[]"#, 1024).is_err());
        let row = SourceRow::parse(br#"{"source_refs":["a","a"]}"#, 1024).unwrap();
        assert_eq!(row.source_refs(&["b", "a"]), vec!["a", "b"]);
        let mut normalized = json!({"a":"more than tiny"});
        assert!(stamp_content_revision(&mut normalized, 2).is_err());
        assert!(normalized.get("content_revision").is_none());
    }
}
