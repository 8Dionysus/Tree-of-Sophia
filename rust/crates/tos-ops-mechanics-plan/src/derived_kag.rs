//! The current source-owned tiny KAG capsule. This is a derived mechanics seam,
//! not KAG admission or a replacement for the authored source node.
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use tos_foundation::{JsonLimits, JsonMode, JsonString, JsonValue, parse_json};

const SOURCE: &str = "ToS/public-compatibility/source_node.example.json";
const CONCEPT: &str = "ToS/public-compatibility/concept_node.example.json";
const TINY: &str = "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md";
const CAPSULE: &str = "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md";
const PRETTY: &str = "ToS/derived-exports/kag_export.json";
const COMPACT: &str = "ToS/derived-exports/kag_export.min.json";
const GENERATOR: &str =
    "mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py";
const PRIMARY_QUESTION: &str = "What source-owned tiny export keeps the current Zarathustra prologue route legible for downstream KAG consumers without replacing ToS authority?";
const SUMMARY_50: &str =
    "Source-owned tiny export for the current Zarathustra prologue authority route.";
const SUMMARY_200: &str = "Source-owned tiny export capsule for the current Zarathustra prologue route, keeping the public compatibility entry surface aligned with the canonical tree while preserving the capsule and tiny-entry docs as supporting ToS-owned orientation surfaces.";
const PROVENANCE_NOTE: &str = "Guide to the current authored tree node, its public compatibility mirror, and the supporting capsule and tiny-entry slice. Follow the authored node for its full meaning and review history.";
const NON_IDENTITY_BOUNDARY: &str = "Derived export capsule for downstream KAG consumers; ToS-authored authority remains in Tree-of-Sophia ToS/canon, ToS/source-witnesses, and capsule surfaces.";
const REFS: &[(&str, &str)] = &[
    (
        "bounded_hop",
        "Tree-of-Sophia/ToS/public-compatibility/concept_node.example.json",
    ),
    (
        "capsule_surface",
        "Tree-of-Sophia/ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md",
    ),
    (
        "tiny_entry_route",
        "Tree-of-Sophia/ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md",
    ),
];

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn read(root: &Path, path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(root.join(path))?;
    let limit = JsonLimits::default().max_bytes;
    if file.metadata()?.len() > limit as u64 {
        return Err(invalid(format!(
            "derived KAG input exceeds JSON bound: {path}"
        )));
    }
    let mut raw = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() > limit {
        return Err(invalid(format!(
            "derived KAG input exceeds JSON bound: {path}"
        )));
    }
    Ok(raw)
}
fn source(root: &Path) -> io::Result<JsonValue> {
    let raw = read(root, SOURCE)?;
    Ok(
        parse_json(&raw, JsonMode::RequestLastWins, JsonLimits::default())
            .map_err(|error| invalid(format!("invalid JSON in {SOURCE}: {error}")))?
            .into_root(),
    )
}
fn root_check(root: &Path) -> io::Result<()> {
    if !root.is_absolute() || fs::canonicalize(root)? != root {
        return Err(invalid(
            "repository root must be an absolute path without symlinks",
        ));
    }
    Ok(())
}
fn payload(source: &JsonValue) -> io::Result<JsonValue> {
    if source.as_object().is_none() {
        return Err(invalid(format!("{SOURCE} must be a JSON object")));
    }
    let id = match source.object_get("node_id") {
        Some(JsonValue::String(id)) if !id.units().is_empty() => id,
        _ => return Err(invalid(format!("{SOURCE} must keep node_id"))),
    };
    let layers = source
        .object_get("interpretation_layers")
        .and_then(JsonValue::as_array)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid(format!("{SOURCE} must keep interpretation_layers")))?;
    let relations = source
        .object_get("relations")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid(format!("{SOURCE} must keep relations")))?;
    if relations.is_empty() {
        return Err(invalid(format!("{SOURCE} must keep at least one relation")));
    }
    let mut handles = Vec::with_capacity(layers.len());
    for layer in layers {
        if !matches!(layer, JsonValue::String(s) if !s.units().is_empty()) {
            return Err(invalid(
                "interpretation_layers must contain non-empty strings",
            ));
        }
        handles.push(layer.clone());
    }
    Ok(object(vec![
        ("owner_repo", string("Tree-of-Sophia")),
        ("kind", string("source_node")),
        ("object_id", JsonValue::String(id.clone())),
        ("primary_question", string(PRIMARY_QUESTION)),
        ("summary_50", string(SUMMARY_50)),
        ("summary_200", string(SUMMARY_200)),
        (
            "source_inputs",
            JsonValue::Array(vec![
                object(vec![
                    ("repo", string("Tree-of-Sophia")),
                    ("source_class", string("tos_text")),
                    ("role", string("primary")),
                ]),
                object(vec![
                    ("repo", string("Tree-of-Sophia")),
                    ("source_class", string("review_surface")),
                    ("role", string("supporting")),
                ]),
            ]),
        ),
        (
            "entry_surface",
            object(vec![
                ("repo", string("Tree-of-Sophia")),
                ("path", string(SOURCE)),
                ("match_key", string("node_id")),
                ("match_value", JsonValue::String(id.clone())),
            ]),
        ),
        ("section_handles", JsonValue::Array(handles)),
        (
            "direct_relations",
            JsonValue::Array(
                REFS.iter()
                    .map(|(kind, reference)| {
                        object(vec![
                            ("relation_type", string(kind)),
                            ("target_ref", string(reference)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("provenance_note", string(PROVENANCE_NOTE)),
        ("non_identity_boundary", string(NON_IDENTITY_BOUNDARY)),
    ]))
}
fn append(out: &mut Vec<u8>, bytes: &[u8]) -> io::Result<()> {
    if out
        .len()
        .checked_add(bytes.len())
        .is_none_or(|n| n > JsonLimits::default().max_bytes)
    {
        return Err(invalid("derived KAG JSON exceeds foundation bound"));
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn indent(out: &mut Vec<u8>, depth: usize) -> io::Result<()> {
    if depth > JsonLimits::default().max_depth {
        return Err(invalid("derived KAG JSON depth exceeded"));
    }
    for _ in 0..depth {
        append(out, b"  ")?;
    }
    Ok(())
}
fn quoted(out: &mut Vec<u8>, value: &JsonString) -> io::Result<()> {
    append(out, b"\"")?;
    for unit in value.units() {
        match *unit {
            34 => append(out, b"\\\"")?,
            92 => append(out, b"\\\\")?,
            8 => append(out, b"\\b")?,
            9 => append(out, b"\\t")?,
            10 => append(out, b"\\n")?,
            12 => append(out, b"\\f")?,
            13 => append(out, b"\\r")?,
            32..=126 => append(out, &[*unit as u8])?,
            _ => append(out, format!("\\u{unit:04x}").as_bytes())?,
        }
    }
    append(out, b"\"")
}
fn render(
    out: &mut Vec<u8>,
    value: &JsonValue,
    depth: usize,
    pretty: bool,
    visits: &mut usize,
) -> io::Result<()> {
    *visits += 1;
    if *visits > JsonLimits::default().max_visits || depth > JsonLimits::default().max_depth {
        return Err(invalid(
            "derived KAG JSON structure exceeds foundation bound",
        ));
    }
    match value {
        JsonValue::String(s) => quoted(out, s),
        JsonValue::Array(items) => {
            append(out, b"[")?;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    append(out, b",")?;
                }
                if pretty {
                    append(out, b"\n")?;
                    indent(out, depth + 1)?;
                }
                render(out, item, depth + 1, pretty, visits)?;
            }
            if pretty && !items.is_empty() {
                append(out, b"\n")?;
                indent(out, depth)?;
            }
            append(out, b"]")
        }
        JsonValue::Object(items) => {
            append(out, b"{")?;
            for (i, (key, item)) in items.iter().enumerate() {
                if i > 0 {
                    append(out, b",")?;
                }
                if pretty {
                    append(out, b"\n")?;
                    indent(out, depth + 1)?;
                }
                quoted(out, key)?;
                let separator: &[u8] = if pretty { b": " } else { b":" };
                append(out, separator)?;
                render(out, item, depth + 1, pretty, visits)?;
            }
            if pretty && !items.is_empty() {
                append(out, b"\n")?;
                indent(out, depth)?;
            }
            append(out, b"}")
        }
        _ => Err(invalid("derived KAG export contains unsupported value")),
    }
}
fn encode(value: &JsonValue, pretty: bool) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut visits = 0;
    render(&mut out, value, 0, pretty, &mut visits)?;
    append(&mut out, b"\n")?;
    Ok(out)
}
fn prepare(root: &Path) -> io::Result<(JsonValue, Vec<u8>, Vec<u8>)> {
    root_check(root)?;
    let source = source(root)?;
    let export = payload(&source)?;
    let pretty = encode(&export, true)?;
    let compact = encode(&export, false)?;
    Ok((source, pretty, compact))
}
fn text_equal(root: &Path, relative: &str, expected: &[u8], label: &str) -> io::Result<Vec<u8>> {
    let actual = match read(root, relative) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(invalid(format!("{label} is missing at {relative}")));
        }
        result => result?,
    };
    let actual = String::from_utf8(actual)
        .map_err(|e| invalid(format!("invalid UTF-8 in {relative}: {e}")))?;
    let actual = actual.replace("\r\n", "\n").replace('\r', "\n");
    if actual.as_bytes() != expected {
        return Err(invalid(format!(
            "{label} is out of date; run python {GENERATOR}"
        )));
    }
    Ok(actual.into_bytes())
}
fn validate_structure(root: &Path, source: &JsonValue, compact: &[u8]) -> io::Result<()> {
    let export = parse_json(compact, JsonMode::RequestLastWins, JsonLimits::default())
        .map_err(|e| invalid(format!("invalid JSON in {COMPACT}: {e}")))?
        .into_root();
    let entries = export
        .as_object()
        .ok_or_else(|| invalid("generated KAG export must be a JSON object"))?;
    for key in [
        "owner_repo",
        "kind",
        "object_id",
        "primary_question",
        "summary_50",
        "summary_200",
        "source_inputs",
        "entry_surface",
        "section_handles",
        "direct_relations",
        "provenance_note",
        "non_identity_boundary",
    ] {
        if !entries.iter().any(|(name, _)| name.as_str() == Some(key)) {
            return Err(invalid(format!(
                "generated KAG export is missing required key '{key}'"
            )));
        }
    }
    if export.object_get("owner_repo").and_then(JsonValue::as_str) != Some("Tree-of-Sophia") {
        return Err(invalid(
            "generated KAG export owner_repo must equal 'Tree-of-Sophia'",
        ));
    }
    if export.object_get("kind").and_then(JsonValue::as_str) != Some("source_node") {
        return Err(invalid(
            "generated KAG export kind must equal 'source_node'",
        ));
    }
    let id = source.object_get("node_id");
    if export.object_get("object_id") != id {
        return Err(invalid(format!(
            "generated KAG export object_id must stay aligned with {SOURCE}"
        )));
    }
    let entry = export
        .object_get("entry_surface")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| invalid("generated KAG export entry_surface must be an object"))?;
    let expected = object(vec![
        ("repo", string("Tree-of-Sophia")),
        ("path", string(SOURCE)),
        ("match_key", string("node_id")),
        ("match_value", id.cloned().unwrap_or(JsonValue::Null)),
    ]);
    // Python dict equality ignores insertion order.
    if entry.len() != 4
        || expected.as_object().unwrap().iter().any(|(k, v)| {
            export
                .object_get("entry_surface")
                .unwrap()
                .object_get(k.as_str().unwrap())
                != Some(v)
        })
    {
        return Err(invalid(
            "generated KAG export entry_surface must stay aligned with the current authority surface",
        ));
    }
    for path in [SOURCE, CONCEPT, TINY, CAPSULE] {
        if !root.join(path).exists() {
            return Err(invalid(format!(
                "required source-owned KAG export surface is missing: {path}"
            )));
        }
    }
    let inputs = export
        .object_get("source_inputs")
        .and_then(JsonValue::as_array)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("generated KAG export source_inputs must be a non-empty list"))?;
    let mut primary = 0;
    for (i, input) in inputs.iter().enumerate() {
        let location = format!("generated KAG export source_inputs[{i}]");
        if input.as_object().is_none() {
            return Err(invalid(format!("{location} must be an object")));
        }
        if input.object_get("role").and_then(JsonValue::as_str) == Some("primary") {
            primary += 1;
        }
        if input.object_get("repo").and_then(JsonValue::as_str) != Some("Tree-of-Sophia") {
            return Err(invalid(format!(
                "{location}.repo must equal 'Tree-of-Sophia'"
            )));
        }
    }
    if primary != 1 {
        return Err(invalid(
            "generated KAG export source_inputs must contain exactly one primary input",
        ));
    }
    if export.object_get("section_handles") != source.object_get("interpretation_layers") {
        return Err(invalid(
            "generated KAG export section_handles must mirror source_node interpretation_layers",
        ));
    }
    let relations = export
        .object_get("direct_relations")
        .and_then(JsonValue::as_array)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("generated KAG export direct_relations must be a non-empty list"))?;
    let mut refs = Vec::new();
    for (i, relation) in relations.iter().enumerate() {
        let location = format!("generated KAG export direct_relations[{i}]");
        if relation.as_object().is_none() {
            return Err(invalid(format!("{location} must be an object")));
        }
        if relation
            .object_get("relation_type")
            .and_then(JsonValue::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(invalid(format!(
                "{location}.relation_type must be a non-empty string"
            )));
        }
        let reference = relation
            .object_get("target_ref")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid(format!("{location}.target_ref must be a non-empty string")))?;
        refs.push(reference);
    }
    let mut missing: Vec<_> = REFS
        .iter()
        .map(|(_, r)| *r)
        .filter(|r| !refs.contains(r))
        .collect();
    missing.sort();
    if !missing.is_empty() {
        return Err(invalid(format!(
            "generated KAG export direct_relations must include the current bounded hop and supporting doctrine refs: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}
/// Check byte parity before inspecting the compact export's public structure.
pub fn validate(root: &Path) -> io::Result<()> {
    let (source, pretty, compact) = prepare(root)?;
    text_equal(root, PRETTY, &pretty, "generated KAG export")?;
    let raw = text_equal(root, COMPACT, &compact, "generated compact KAG export")?;
    validate_structure(root, &source, &raw)
}
/// Write the two derived files in maintained order. A second-write failure
/// leaves the first completed output, exactly as the existing generator does.
pub fn write_outputs(root: &Path) -> io::Result<Vec<&'static str>> {
    let (_, pretty, compact) = prepare(root)?;
    fs::create_dir_all(root.join("ToS/derived-exports"))?;
    let mut file = File::create(root.join(PRETTY))?;
    file.write_all(&pretty)?;
    let mut file = File::create(root.join(COMPACT))?;
    file.write_all(&compact)?;
    Ok(vec![PRETTY, COMPACT])
}

/// Derive the existing capsule from an unpublished exact-source stage.
pub(crate) fn build_payload(root: &Path) -> io::Result<JsonValue> {
    root_check(root)?;
    payload(&source(root)?)
}
