//! Durable corpus text-reference semantics shared by Rust and the browser WASM ABI.
//!
//! This owns validation, canonical identity, version IDs, construction from an
//! exact native read result, stale comparison, and Unicode code-point slicing.
//! Transport, graph rediscovery, cancellation, and presentation remain host work.
use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};

pub const CORPUS_REFERENCE_SCHEMA: &str = "tos.corpus.reader.reference.v1";
pub const NATIVE_REFERENCE_SCHEMA: &str = "tos.corpus.reader.native-reference.v1";
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = MAX_INPUT_BYTES;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}

fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn integer(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value.object_get(name)
}

fn text<'a>(value: &'a JsonValue, name: &str) -> Option<&'a str> {
    field(value, name)?.as_str()
}

fn json_string(value: &JsonValue) -> Option<&JsonString> {
    match value {
        JsonValue::String(value) => Some(value),
        _ => None,
    }
}

fn nonempty_string(value: &JsonValue) -> Option<JsonValue> {
    json_string(value)
        .filter(|value| !value.units().is_empty())
        .map(|value| JsonValue::String(value.clone()))
}

fn native_string(value: &JsonValue) -> Option<JsonValue> {
    let word = json_string(value)?;
    if word.units().is_empty() || word.units().len() > 2048 || word.has_lone_surrogate() {
        return None;
    }
    let text = word.as_str()?;
    if text
        .chars()
        .any(|character| (character as u32) <= 0x1f || character == '\u{7f}')
    {
        return None;
    }
    Some(JsonValue::String(word.clone()))
}

fn identifier(value: &JsonValue) -> Option<JsonValue> {
    let canonical = native_string(value)?;
    let text = canonical.as_str()?;
    let suffix = text.strip_prefix("tos.")?;
    if suffix.is_empty() {
        return None;
    }
    let mut needs_alphanumeric = true;
    for byte in suffix.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            needs_alphanumeric = false;
        } else if matches!(byte, b'.' | b'-') && !needs_alphanumeric {
            needs_alphanumeric = true;
        } else {
            return None;
        }
    }
    (!needs_alphanumeric).then_some(canonical)
}

fn digest(value: &JsonValue) -> Option<JsonValue> {
    let text = value.as_str()?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some(value.clone())
}

fn safe_integer(value: &JsonValue, minimum: u64) -> Option<u64> {
    let JsonValue::Number(number) = value else {
        return None;
    };
    let value = number.lexeme.parse::<f64>().ok()?;
    if !value.is_finite()
        || value.fract() != 0.0
        || value < minimum as f64
        || value > MAX_SAFE_INTEGER
    {
        return None;
    }
    Some(value as u64)
}

fn canonical_integer(value: &JsonValue, minimum: u64) -> Option<JsonValue> {
    safe_integer(value, minimum).map(integer)
}

fn canonical_required_string(value: &JsonValue, key: &str) -> Result<JsonValue, &'static str> {
    field(value, key)
        .and_then(nonempty_string)
        .ok_or("invalid_corpus_reference")
}

fn canonical_bounded_digest(value: &JsonValue, key: &str) -> Result<JsonValue, &'static str> {
    field(value, key)
        .and_then(digest)
        .ok_or("invalid_corpus_reference")
}

fn canonical_corpus_target(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if value.as_object().is_none() {
        return Err("invalid_corpus_reference");
    }
    Ok(object(vec![
        ("workId", canonical_required_string(value, "workId")?),
        (
            "expressionId",
            canonical_required_string(value, "expressionId")?,
        ),
        ("editionId", canonical_required_string(value, "editionId")?),
        ("itemId", canonical_required_string(value, "itemId")?),
        ("fileId", canonical_required_string(value, "fileId")?),
        ("fileSha256", canonical_bounded_digest(value, "fileSha256")?),
        (
            "textLayerRef",
            canonical_required_string(value, "textLayerRef")?,
        ),
        (
            "textLayerSha256",
            canonical_bounded_digest(value, "textLayerSha256")?,
        ),
    ]))
}

fn canonical_corpus_selector(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if value.as_object().is_none()
        || text(value, "type") != Some("text_position")
        || text(value, "positionUnit") != Some("unicode_code_point")
        || text(value, "interval") != Some("half_open")
    {
        return Err("invalid_corpus_reference");
    }
    let start = field(value, "start")
        .and_then(|value| canonical_integer(value, 0))
        .ok_or("invalid_corpus_reference")?;
    let end_value = field(value, "end").ok_or("invalid_corpus_reference")?;
    let end = canonical_integer(end_value, 0).ok_or("invalid_corpus_reference")?;
    let start_number = safe_integer(&start, 0).ok_or("invalid_corpus_reference")?;
    let end_number = safe_integer(&end, 0).ok_or("invalid_corpus_reference")?;
    if end_number < start_number {
        return Err("reference_selector_range");
    }
    Ok(object(vec![
        ("type", string("text_position")),
        ("start", start),
        ("end", end),
        ("positionUnit", string("unicode_code_point")),
        ("interval", string("half_open")),
    ]))
}

fn canonical_corpus_reference(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if value.as_object().is_none() || text(value, "schemaVersion") != Some(CORPUS_REFERENCE_SCHEMA)
    {
        return Err("invalid_corpus_reference");
    }
    let target = field(value, "target").ok_or("invalid_corpus_reference")?;
    let selector = field(value, "selector").ok_or("invalid_corpus_reference")?;
    Ok(object(vec![
        ("schemaVersion", string(CORPUS_REFERENCE_SCHEMA)),
        ("target", canonical_corpus_target(target)?),
        ("versionId", canonical_required_string(value, "versionId")?),
        ("unitId", canonical_required_string(value, "unitId")?),
        ("selector", canonical_corpus_selector(selector)?),
    ]))
}

fn canonical_native_versioned(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if value.as_object().is_none() {
        return Err("invalid_native_reference");
    }
    let id = field(value, "id")
        .and_then(identifier)
        .ok_or("invalid_native_reference")?;
    let version = field(value, "version")
        .and_then(|value| canonical_integer(value, 1))
        .ok_or("invalid_native_reference")?;
    Ok(object(vec![("id", id), ("version", version)]))
}

fn canonical_native_selector(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if value.as_object().is_none()
        || text(value, "type") != Some("text_position")
        || text(value, "positionUnit") != Some("unicode_code_point")
        || text(value, "interval") != Some("half_open")
    {
        return Err("invalid_native_reference");
    }
    let start = field(value, "start")
        .and_then(|value| canonical_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let end = field(value, "end")
        .and_then(|value| canonical_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    if safe_integer(&end, 0) < safe_integer(&start, 0) {
        return Err("invalid_native_reference");
    }
    Ok(object(vec![
        ("type", string("text_position")),
        ("start", start),
        ("end", end),
        ("positionUnit", string("unicode_code_point")),
        ("interval", string("half_open")),
    ]))
}

fn canonical_native_reference(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if value.as_object().is_none() || text(value, "schemaVersion") != Some(NATIVE_REFERENCE_SCHEMA)
    {
        return Err("invalid_native_reference");
    }
    let origin = field(value, "origin").ok_or("invalid_native_reference")?;
    if origin.as_object().is_none() || !matches!(text(origin, "kind"), Some("node" | "relation")) {
        return Err("invalid_native_reference");
    }
    let origin_id = field(origin, "id")
        .and_then(native_string)
        .ok_or("invalid_native_reference")?;
    let representation = match text(value, "representation") {
        Some("native_public_unit") => string("native_public_unit"),
        Some("native_local_unit") => string("native_local_unit"),
        _ => return Err("invalid_native_reference"),
    };
    let target = field(value, "target").ok_or("invalid_native_reference")?;
    if target.as_object().is_none() {
        return Err("invalid_native_reference");
    }

    let record_value = field(target, "record").ok_or("invalid_native_reference")?;
    if record_value.as_object().is_none() {
        return Err("invalid_native_reference");
    }
    let record_digest = text(record_value, "digest").ok_or("invalid_native_reference")?;
    let record_hash = record_digest
        .strip_prefix("sha256:")
        .and_then(|value| {
            let value = JsonValue::String(JsonString::from_utf8(value));
            digest(&value)
        })
        .ok_or("invalid_native_reference")?;
    let record = object(vec![
        (
            "id",
            field(record_value, "id")
                .and_then(identifier)
                .ok_or("invalid_native_reference")?,
        ),
        (
            "version",
            field(record_value, "version")
                .and_then(|value| canonical_integer(value, 1))
                .ok_or("invalid_native_reference")?,
        ),
        (
            "digest",
            string(&format!(
                "sha256:{}",
                record_hash.as_str().ok_or("invalid_native_reference")?
            )),
        ),
    ]);

    let packet_value = field(target, "packet").ok_or("invalid_native_reference")?;
    if packet_value.as_object().is_none() {
        return Err("invalid_native_reference");
    }
    let packet = object(vec![
        (
            "id",
            field(packet_value, "id")
                .and_then(identifier)
                .ok_or("invalid_native_reference")?,
        ),
        (
            "version",
            field(packet_value, "version")
                .and_then(|value| canonical_integer(value, 1))
                .ok_or("invalid_native_reference")?,
        ),
        (
            "sha256",
            field(packet_value, "sha256")
                .and_then(digest)
                .ok_or("invalid_native_reference")?,
        ),
    ]);
    let segmentation = canonical_native_versioned(
        field(target, "segmentation").ok_or("invalid_native_reference")?,
    )?;
    let unit =
        canonical_native_versioned(field(target, "unit").ok_or("invalid_native_reference")?)?;

    let layer_value = field(target, "textLayer").ok_or("invalid_native_reference")?;
    if layer_value.as_object().is_none() {
        return Err("invalid_native_reference");
    }
    let text_layer = object(vec![
        (
            "id",
            field(layer_value, "id")
                .and_then(identifier)
                .ok_or("invalid_native_reference")?,
        ),
        (
            "version",
            field(layer_value, "version")
                .and_then(|value| canonical_integer(value, 1))
                .ok_or("invalid_native_reference")?,
        ),
        (
            "recordSha256",
            field(layer_value, "recordSha256")
                .and_then(digest)
                .ok_or("invalid_native_reference")?,
        ),
    ]);
    let representation_sha256 = field(target, "representationSha256")
        .and_then(digest)
        .ok_or("invalid_native_reference")?;

    let span_value = field(target, "span").ok_or("invalid_native_reference")?;
    if span_value.as_object().is_none() {
        return Err("invalid_native_reference");
    }
    let span_start = field(span_value, "start")
        .and_then(|value| canonical_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let span_end = field(span_value, "end")
        .and_then(|value| canonical_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    if safe_integer(&span_end, 0) < safe_integer(&span_start, 0) {
        return Err("invalid_native_reference");
    }
    let span = object(vec![
        (
            "anchorRef",
            field(span_value, "anchorRef")
                .and_then(identifier)
                .ok_or("invalid_native_reference")?,
        ),
        ("start", span_start),
        ("end", span_end),
        (
            "exactSha256",
            field(span_value, "exactSha256")
                .and_then(digest)
                .ok_or("invalid_native_reference")?,
        ),
    ]);
    let selector =
        canonical_native_selector(field(value, "selector").ok_or("invalid_native_reference")?)?;
    let selector_start = field(&selector, "start")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let selector_end = field(&selector, "end")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let span_start_number = field(&span, "start")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let span_end_number = field(&span, "end")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    if selector_start < span_start_number || selector_end > span_end_number {
        return Err("invalid_native_reference");
    }

    Ok(object(vec![
        ("schemaVersion", string(NATIVE_REFERENCE_SCHEMA)),
        (
            "origin",
            object(vec![
                ("kind", field(origin, "kind").unwrap().clone()),
                ("id", origin_id),
            ]),
        ),
        ("representation", representation),
        (
            "target",
            object(vec![
                ("record", record),
                ("packet", packet),
                ("segmentation", segmentation),
                ("unit", unit),
                ("textLayer", text_layer),
                ("representationSha256", representation_sha256),
                ("span", span),
            ]),
        ),
        ("selector", selector),
    ]))
}

fn canonical_reference(value: &JsonValue) -> Result<JsonValue, &'static str> {
    if text(value, "schemaVersion") == Some(NATIVE_REFERENCE_SCHEMA) {
        canonical_native_reference(value)
    } else {
        canonical_corpus_reference(value)
    }
}

fn array_key(value: JsonValue) -> Result<String, &'static str> {
    let bytes = emit_value_preserved_json(
        &value,
        JsonLimits {
            max_bytes: MAX_OUTPUT_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "invalid_reference_output")?;
    String::from_utf8(bytes).map_err(|_| "invalid_reference_output")
}

fn native_key_value(reference: &JsonValue) -> Result<JsonValue, &'static str> {
    let checked = canonical_native_reference(reference)?;
    Ok(JsonValue::Array(vec![
        field(&checked, "schemaVersion")
            .ok_or("invalid_native_reference")?
            .clone(),
        field(&checked, "target")
            .ok_or("invalid_native_reference")?
            .clone(),
        field(&checked, "selector")
            .ok_or("invalid_native_reference")?
            .clone(),
    ]))
}

fn corpus_key_value(reference: &JsonValue) -> Result<JsonValue, &'static str> {
    let checked = canonical_corpus_reference(reference)?;
    let target = field(&checked, "target").ok_or("invalid_corpus_reference")?;
    let selector = field(&checked, "selector").ok_or("invalid_corpus_reference")?;
    let mut key = vec![
        field(&checked, "schemaVersion")
            .ok_or("invalid_corpus_reference")?
            .clone(),
    ];
    for name in [
        "workId",
        "expressionId",
        "editionId",
        "itemId",
        "fileId",
        "fileSha256",
        "textLayerRef",
        "textLayerSha256",
    ] {
        key.push(
            field(target, name)
                .ok_or("invalid_corpus_reference")?
                .clone(),
        );
    }
    key.push(
        field(&checked, "versionId")
            .ok_or("invalid_corpus_reference")?
            .clone(),
    );
    key.push(
        field(&checked, "unitId")
            .ok_or("invalid_corpus_reference")?
            .clone(),
    );
    for name in ["type", "start", "end", "positionUnit", "interval"] {
        key.push(
            field(selector, name)
                .ok_or("invalid_corpus_reference")?
                .clone(),
        );
    }
    Ok(JsonValue::Array(key))
}

fn reference_key_value(reference: &JsonValue) -> Result<String, &'static str> {
    if text(reference, "schemaVersion") == Some(NATIVE_REFERENCE_SCHEMA) {
        array_key(native_key_value(reference)?)
    } else {
        array_key(corpus_key_value(reference)?)
    }
}

fn reference_document_id_value(reference: &JsonValue) -> Result<JsonValue, &'static str> {
    let native = text(reference, "schemaVersion") == Some(NATIVE_REFERENCE_SCHEMA);
    let checked = if native {
        canonical_native_reference(reference)?
    } else {
        canonical_corpus_reference(reference)?
    };
    let target = field(&checked, "target").ok_or("invalid_reference")?;
    let name = if native { "textLayer" } else { "workId" };
    let source = if native {
        field(target, name).ok_or("invalid_native_reference")?
    } else {
        target
    };
    field(source, "id")
        .or_else(|| if native { None } else { field(source, name) })
        .cloned()
        .ok_or("invalid_reference")
}

fn reference_version_id_value(reference: &JsonValue) -> Result<JsonValue, &'static str> {
    let native = text(reference, "schemaVersion") == Some(NATIVE_REFERENCE_SCHEMA);
    let checked = if native {
        canonical_native_reference(reference)?
    } else {
        canonical_corpus_reference(reference)?
    };
    if !native {
        return field(&checked, "versionId")
            .cloned()
            .ok_or("invalid_corpus_reference");
    }
    let target = field(&checked, "target").ok_or("invalid_native_reference")?;
    let packet = field(target, "packet").ok_or("invalid_native_reference")?;
    let segmentation = field(target, "segmentation").ok_or("invalid_native_reference")?;
    let layer = field(target, "textLayer").ok_or("invalid_native_reference")?;
    array_key(JsonValue::Array(vec![
        field(packet, "id").unwrap().clone(),
        field(packet, "version").unwrap().clone(),
        field(packet, "sha256").unwrap().clone(),
        field(segmentation, "id").unwrap().clone(),
        field(segmentation, "version").unwrap().clone(),
        field(layer, "version").unwrap().clone(),
    ]))
    .map(|value| string(&value))
}

fn create_corpus_reference(value: &JsonValue) -> Result<JsonValue, &'static str> {
    let source = field(value, "source").ok_or("invalid_corpus_reference")?;
    if source.as_object().is_none() {
        return Err("invalid_corpus_reference");
    }
    let target = object(vec![
        (
            "workId",
            field(source, "workId")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "expressionId",
            field(source, "expressionId")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "editionId",
            field(source, "editionId")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "itemId",
            field(source, "itemId")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "fileId",
            field(source, "fileId")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "fileSha256",
            field(source, "fileSha256")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "textLayerRef",
            field(source, "textLayerRef")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "textLayerSha256",
            field(source, "textLayerSha256")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
    ]);
    let selector = object(vec![
        ("type", string("text_position")),
        (
            "start",
            field(value, "start")
                .map(|value| canonical_integer(value, 0).ok_or("invalid_corpus_reference"))
                .unwrap_or(Ok(integer(0)))?,
        ),
        (
            "end",
            field(value, "end")
                .map(|value| canonical_integer(value, 0).ok_or("invalid_corpus_reference"))
                .unwrap_or(Ok(integer(0)))?,
        ),
        ("positionUnit", string("unicode_code_point")),
        ("interval", string("half_open")),
    ]);
    canonical_corpus_reference(&object(vec![
        ("schemaVersion", string(CORPUS_REFERENCE_SCHEMA)),
        ("target", target),
        (
            "versionId",
            field(value, "versionId")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        (
            "unitId",
            field(value, "unitId")
                .cloned()
                .ok_or("invalid_corpus_reference")?,
        ),
        ("selector", selector),
    ]))
}

fn value_truthy(value: Option<&JsonValue>) -> bool {
    match value {
        None | Some(JsonValue::Null) | Some(JsonValue::Bool(false)) => false,
        Some(JsonValue::Number(number)) => !number
            .lexeme
            .parse::<f64>()
            .is_ok_and(|value| value == 0.0 || value.is_nan()),
        Some(JsonValue::String(value)) => !value.units().is_empty(),
        Some(JsonValue::Bool(true) | JsonValue::Array(_) | JsonValue::Object(_)) => true,
    }
}

fn span_at(value: &JsonValue, index: u64) -> Result<&JsonValue, &'static str> {
    let index = usize::try_from(index).map_err(|_| "invalid_native_reference")?;
    field(
        field(value, "native_unit").ok_or("invalid_native_reference")?,
        "spans",
    )
    .and_then(JsonValue::as_array)
    .and_then(|spans| spans.get(index))
    .ok_or("invalid_native_reference")
}

fn create_native_reference(value: &JsonValue) -> Result<JsonValue, &'static str> {
    let result = field(value, "result").ok_or("invalid_native_reference")?;
    if text(result, "status") != Some("available")
        || !value_truthy(field(result, "native_unit"))
        || !value_truthy(field(result, "selection"))
    {
        return Err("invalid_native_reference");
    }
    let span_index = field(value, "spanIndex")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let span = span_at(result, span_index)?;
    let unit = field(result, "native_unit").ok_or("invalid_native_reference")?;
    let summary = field(unit, "summary").ok_or("invalid_native_reference")?;
    let span_selector = field(span, "selector").ok_or("invalid_native_reference")?;
    let selection = field(result, "selection").ok_or("invalid_native_reference")?;

    let selected_start = field(value, "start")
        .filter(|value| !value.is_null())
        .map(|value| canonical_integer(value, 0).ok_or("invalid_native_reference"))
        .unwrap_or_else(|| {
            field(span_selector, "start")
                .and_then(|value| canonical_integer(value, 0))
                .ok_or("invalid_native_reference")
        })?;
    let selected_end = field(value, "end")
        .filter(|value| !value.is_null())
        .map(|value| canonical_integer(value, 0).ok_or("invalid_native_reference"))
        .unwrap_or_else(|| {
            field(span_selector, "end")
                .and_then(|value| canonical_integer(value, 0))
                .ok_or("invalid_native_reference")
        })?;

    let record = field(result, "record_ref")
        .ok_or("invalid_native_reference")?
        .clone();
    let packet = field(unit, "packet")
        .ok_or("invalid_native_reference")?
        .clone();
    let record_ref = object(vec![
        (
            "id",
            field(&record, "id")
                .ok_or("invalid_native_reference")?
                .clone(),
        ),
        (
            "version",
            field(&record, "version")
                .ok_or("invalid_native_reference")?
                .clone(),
        ),
        (
            "digest",
            field(&record, "digest")
                .ok_or("invalid_native_reference")?
                .clone(),
        ),
    ]);
    let packet_ref = object(vec![
        (
            "id",
            field(&packet, "id")
                .ok_or("invalid_native_reference")?
                .clone(),
        ),
        (
            "version",
            field(&packet, "version")
                .ok_or("invalid_native_reference")?
                .clone(),
        ),
        (
            "sha256",
            field(&packet, "sha256")
                .ok_or("invalid_native_reference")?
                .clone(),
        ),
    ]);
    let representation = if text(unit, "schema_version") == Some("tos_native_local_unit_return_v1")
    {
        "native_local_unit"
    } else {
        "native_public_unit"
    };
    let native = object(vec![
        ("schemaVersion", string(NATIVE_REFERENCE_SCHEMA)),
        (
            "origin",
            object(vec![
                (
                    "kind",
                    field(selection, "kind")
                        .ok_or("invalid_native_reference")?
                        .clone(),
                ),
                (
                    "id",
                    field(selection, "id")
                        .ok_or("invalid_native_reference")?
                        .clone(),
                ),
            ]),
        ),
        ("representation", string(representation)),
        (
            "target",
            object(vec![
                ("record", record_ref),
                ("packet", packet_ref),
                (
                    "segmentation",
                    object(vec![
                        (
                            "id",
                            field(summary, "segmentation_id")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                        (
                            "version",
                            field(summary, "segmentation_version")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                    ]),
                ),
                (
                    "unit",
                    object(vec![
                        (
                            "id",
                            field(summary, "unit_id")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                        (
                            "version",
                            field(summary, "unit_version")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                    ]),
                ),
                (
                    "textLayer",
                    object(vec![
                        (
                            "id",
                            field(summary, "layer_id")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                        (
                            "version",
                            field(summary, "layer_version")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                        (
                            "recordSha256",
                            field(unit, "layer_record_sha256")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                    ]),
                ),
                (
                    "representationSha256",
                    field(unit, "representation_sha256")
                        .ok_or("invalid_native_reference")?
                        .clone(),
                ),
                (
                    "span",
                    object(vec![
                        (
                            "anchorRef",
                            field(span, "anchor_ref")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                        (
                            "start",
                            field(span_selector, "start")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                        (
                            "end",
                            field(span_selector, "end")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                        (
                            "exactSha256",
                            field(span, "exact_sha256")
                                .ok_or("invalid_native_reference")?
                                .clone(),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "selector",
            object(vec![
                ("type", string("text_position")),
                ("start", selected_start),
                ("end", selected_end),
                ("positionUnit", string("unicode_code_point")),
                ("interval", string("half_open")),
            ]),
        ),
    ]);
    canonical_native_reference(&native)
}

fn codepoint_boundaries(units: &[u16]) -> Vec<usize> {
    let mut boundaries = Vec::with_capacity(units.len().saturating_add(1));
    boundaries.push(0);
    let mut index = 0;
    while index < units.len() {
        let high = (0xd800..=0xdbff).contains(&units[index]);
        let low_follows = index + 1 < units.len() && (0xdc00..=0xdfff).contains(&units[index + 1]);
        index += if high && low_follows { 2 } else { 1 };
        boundaries.push(index);
    }
    boundaries
}

fn native_selected_text(
    reference: &JsonValue,
    text: &JsonValue,
) -> Result<JsonValue, &'static str> {
    let checked = canonical_native_reference(reference)?;
    let input = json_string(text).ok_or("invalid_native_reference")?;
    let target = field(&checked, "target").ok_or("invalid_native_reference")?;
    let span = field(target, "span").ok_or("invalid_native_reference")?;
    let selector = field(&checked, "selector").ok_or("invalid_native_reference")?;
    let points = codepoint_boundaries(input.units());
    let span_start = field(span, "start")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let span_end = field(span, "end")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let selector_start = field(selector, "start")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    let selector_end = field(selector, "end")
        .and_then(|value| safe_integer(value, 0))
        .ok_or("invalid_native_reference")?;
    if points.len().saturating_sub(1) as u64 != span_end - span_start {
        return Err("invalid_native_reference");
    }
    let start =
        usize::try_from(selector_start - span_start).map_err(|_| "invalid_native_reference")?;
    let end = usize::try_from(selector_end - span_start).map_err(|_| "invalid_native_reference")?;
    let from = *points.get(start).ok_or("invalid_native_reference")?;
    let to = *points.get(end).ok_or("invalid_native_reference")?;
    Ok(JsonValue::String(JsonString::from_utf16(
        &input.units()[from..to],
    )))
}

fn native_match(value: &JsonValue) -> Result<JsonValue, &'static str> {
    let reference = field(value, "reference").ok_or("invalid_native_reference")?;
    let checked = canonical_native_reference(reference)?;
    let result = field(value, "result");
    let status = result.and_then(|result| field(result, "status"));
    let native_unit = result.and_then(|result| field(result, "native_unit"));
    if status.and_then(JsonValue::as_str) != Some("available") || !value_truthy(native_unit) {
        let fallback = status
            .filter(|status| !status.is_null())
            .cloned()
            .unwrap_or_else(|| string("unavailable"));
        return Ok(object(vec![
            ("status", fallback),
            ("spanIndex", JsonValue::Null),
        ]));
    }
    let result = result.ok_or("invalid_native_reference")?;
    let spans = field(native_unit.ok_or("invalid_native_reference")?, "spans")
        .and_then(JsonValue::as_array)
        .ok_or("invalid_native_reference")?;
    let anchor = field(
        field(
            field(&checked, "target").ok_or("invalid_native_reference")?,
            "span",
        )
        .ok_or("invalid_native_reference")?,
        "anchorRef",
    )
    .ok_or("invalid_native_reference")?;
    let index = spans
        .iter()
        .position(|span| field(span, "anchor_ref") == Some(anchor));
    let Some(index) = index else {
        return Ok(object(vec![
            ("status", string("stale")),
            ("spanIndex", JsonValue::Null),
        ]));
    };
    let creation = object(vec![
        ("result", result.clone()),
        ("spanIndex", integer(index as u64)),
        (
            "start",
            field(
                field(&checked, "selector").ok_or("invalid_native_reference")?,
                "start",
            )
            .ok_or("invalid_native_reference")?
            .clone(),
        ),
        (
            "end",
            field(
                field(&checked, "selector").ok_or("invalid_native_reference")?,
                "end",
            )
            .ok_or("invalid_native_reference")?
            .clone(),
        ),
    ]);
    match create_native_reference(&creation) {
        Ok(current)
            if array_key(native_key_value(&current)?)?
                == array_key(native_key_value(&checked)?)? =>
        {
            Ok(object(vec![
                ("status", string("exact")),
                ("spanIndex", integer(index as u64)),
            ]))
        }
        _ => Ok(object(vec![
            ("status", string("stale")),
            ("spanIndex", JsonValue::Null),
        ])),
    }
}

fn dispatch(operation: &str, value: &JsonValue) -> Result<JsonValue, &'static str> {
    match operation {
        "validate_reference" => canonical_reference(value),
        "validate_native_reference" => canonical_native_reference(value),
        "reference_key" => reference_key_value(value).map(|value| string(&value)),
        "native_reference_key" => array_key(native_key_value(value)?).map(|value| string(&value)),
        "reference_document_id" => reference_document_id_value(value),
        "reference_version_id" => reference_version_id_value(value),
        "create_reference" => create_corpus_reference(value),
        "create_native_reference" => create_native_reference(value),
        "match_native_reference" => native_match(value),
        "native_selected_text" => native_selected_text(
            field(value, "reference").ok_or("invalid_native_reference")?,
            field(value, "text").ok_or("invalid_native_reference")?,
        ),
        _ => Err("invalid_input"),
    }
}

/// Run one bounded corpus-reference operation. This is the sole semantic entry
/// used by the browser adapter and the native Rust tests.
pub fn corpus_reference_rule_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: MAX_INPUT_BYTES,
            ..Default::default()
        },
    )
    .map_err(|_| "invalid_input")?;
    let request = document.root();
    if request.as_object().is_none()
        || field(request, "operation")
            .and_then(JsonValue::as_str)
            .is_none()
        || field(request, "value").is_none()
    {
        return Err("invalid_input");
    }
    let operation = text(request, "operation").ok_or("invalid_input")?;
    let value = field(request, "value").ok_or("invalid_input")?;
    let output = dispatch(operation, value)?;
    emit_value_preserved_json(
        &output,
        JsonLimits {
            max_bytes: MAX_OUTPUT_BYTES,
            ..Default::default()
        },
    )
    .map_err(|_| "invalid_reference_output")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(operation: &str, value: &str) -> Result<Vec<u8>, &'static str> {
        corpus_reference_rule_v1(
            format!(r#"{{"operation":"{}","value":{}}}"#, operation, value).as_bytes(),
        )
    }

    const H: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn native_reference() -> String {
        format!(
            r#"{{"schemaVersion":"tos.corpus.reader.native-reference.v1","origin":{{"kind":"node","id":"source:tos.text-unit.fixture"}},"representation":"native_public_unit","target":{{"record":{{"id":"tos.text-unit.fixture","version":1,"digest":"sha256:{}"}},"packet":{{"id":"tos.source-text-unit-packet.fixture","version":1,"sha256":"{}"}},"segmentation":{{"id":"tos.text-segmentation.fixture","version":1}},"unit":{{"id":"tos.text-unit.fixture","version":1}},"textLayer":{{"id":"tos.text-layer.fixture","version":1,"recordSha256":"{}"}},"representationSha256":"{}","span":{{"anchorRef":"tos.anchor.one","start":20,"end":26,"exactSha256":"{}"}}}},"selector":{{"type":"text_position","start":21,"end":24,"positionUnit":"unicode_code_point","interval":"half_open"}}}}"#,
            H, H, H, B, H
        )
    }

    fn native_result() -> String {
        format!(
            r#"{{"status":"available","selection":{{"kind":"node","id":"source:tos.text-unit.fixture"}},"record_ref":{{"id":"tos.text-unit.fixture","version":1,"digest":"sha256:{}"}},"native_unit":{{"schema_version":"tos_native_public_unit_return_v1","packet":{{"id":"tos.source-text-unit-packet.fixture","version":1,"sha256":"{}"}},"summary":{{"unit_id":"tos.text-unit.fixture","unit_version":1,"segmentation_id":"tos.text-segmentation.fixture","segmentation_version":1,"layer_id":"tos.text-layer.fixture","layer_version":1}},"layer_record_sha256":"{}","representation_sha256":"{}","spans":[{{"anchor_ref":"tos.anchor.one","selector":{{"start":20,"end":26}},"exact_sha256":"{}","text":"A😀éאב"}}]}}}}"#,
            H, H, H, B, H
        )
    }

    #[test]
    fn native_key_keeps_the_historical_json_string_field_order() {
        let reference = native_reference();
        let key = call("native_reference_key", &reference).unwrap();
        assert_eq!(call("reference_key", &reference).unwrap(), key);
        let key = parse_json(&key, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        let actual = key.root().as_str().unwrap();
        let expected = format!(
            r#"["tos.corpus.reader.native-reference.v1",{{"record":{{"id":"tos.text-unit.fixture","version":1,"digest":"sha256:{}"}},"packet":{{"id":"tos.source-text-unit-packet.fixture","version":1,"sha256":"{}"}},"segmentation":{{"id":"tos.text-segmentation.fixture","version":1}},"unit":{{"id":"tos.text-unit.fixture","version":1}},"textLayer":{{"id":"tos.text-layer.fixture","version":1,"recordSha256":"{}"}},"representationSha256":"{}","span":{{"anchorRef":"tos.anchor.one","start":20,"end":26,"exactSha256":"{}"}}}},{{"type":"text_position","start":21,"end":24,"positionUnit":"unicode_code_point","interval":"half_open"}}]"#,
            H, H, H, B, H
        );
        assert_eq!(actual, expected);
        assert_eq!(
            String::from_utf8(call("reference_document_id", &reference).unwrap()).unwrap(),
            r#""tos.text-layer.fixture""#
        );
        let version_id = call("reference_version_id", &reference).unwrap();
        let version_id = parse_json(
            &version_id,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let expected_version_id = format!(
            r#"["tos.source-text-unit-packet.fixture",1,"{}","tos.text-segmentation.fixture",1,1]"#,
            H
        );
        assert_eq!(
            version_id.root().as_str(),
            Some(expected_version_id.as_str())
        );
    }

    #[test]
    fn corpus_reference_key_keeps_legacy_order_and_range_error() {
        let reference = format!(
            r#"{{"schemaVersion":"tos.corpus.reader.reference.v1","target":{{"workId":"w","expressionId":"x","editionId":"e","itemId":"i","fileId":"f","fileSha256":"{}","textLayerRef":"l","textLayerSha256":"{}"}},"versionId":"v","unitId":"u","selector":{{"type":"text_position","start":0,"end":2,"positionUnit":"unicode_code_point","interval":"half_open"}}}}"#,
            H, B
        );
        let key = call("reference_key", &reference).unwrap();
        let key = parse_json(&key, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        let expected = format!(
            r#"["tos.corpus.reader.reference.v1","w","x","e","i","f","{}","l","{}","v","u","text_position",0,2,"unicode_code_point","half_open"]"#,
            H, B
        );
        assert_eq!(key.root().as_str(), Some(expected.as_str()));
        let backwards = reference.replace(r#""start":0,"end":2"#, r#""start":3,"end":2"#);
        assert_eq!(
            call("validate_reference", &backwards),
            Err("reference_selector_range")
        );
    }

    #[test]
    fn native_match_and_slice_pin_exact_code_points() {
        let reference = native_reference();
        let result = native_result();
        let input = format!(r#"{{"reference":{},"result":{}}}"#, reference, result);
        let matched = call("match_native_reference", &input).unwrap();
        assert!(
            String::from_utf8(matched)
                .unwrap()
                .contains(r#""status":"exact""#)
        );
        let slice = call(
            "native_selected_text",
            &format!(r#"{{"reference":{},"text":"A😀éאב"}}"#, reference),
        )
        .unwrap();
        let slice = parse_json(&slice, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        assert_eq!(slice.root().as_str(), Some("😀é"));
        let changed = result.replace(B, H);
        let input = format!(r#"{{"reference":{},"result":{}}}"#, reference, changed);
        let stale = call("match_native_reference", &input).unwrap();
        assert!(
            String::from_utf8(stale)
                .unwrap()
                .contains(r#""status":"stale""#)
        );
    }

    #[test]
    fn malformed_native_references_and_non_well_formed_ids_fail_closed() {
        let reference = native_reference().replace(r#""end":24"#, r#""end":27"#);
        assert_eq!(
            call("validate_native_reference", &reference),
            Err("invalid_native_reference")
        );
        let unpaired = r#"{"schemaVersion":"tos.corpus.reader.native-reference.v1","origin":{"kind":"node","id":"\ud800"}}"#;
        assert_eq!(
            call("validate_native_reference", unpaired),
            Err("invalid_native_reference")
        );
    }
}
