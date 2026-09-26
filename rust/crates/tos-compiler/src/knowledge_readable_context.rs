//! Source-owned readable context from an already normalized carrier.
//!
//! This is a per-row compiler, not an admission or a graph producer. The
//! caller must retain an order-preserving JSON witness before routing source
//! objects through `serde_json::Value`, whose maps sort keys. The witness is
//! checked for exact canonical value equality with the normalized carrier.

use crate::{Error, Result, knowledge_normalization::stable_digest};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};

const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32_768;
const MAX_CONTEXTS: usize = 64;
const MAX_ENTRIES: usize = 256;
const MAX_POINTER_BYTES: usize = 2048;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const REGISTRY_REF: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const OWNER_REF: &str = "ToS/doctrine/HUMAN_FORMS.md";
const TECHNICAL: &[&str] = &[
    "schema_version",
    "record_id",
    "record_version",
    "claim_id",
    "claim_version",
    "source_record_digest",
    "source_sha256",
    "record_sha256",
    "journal_revision",
    "owner_snapshot",
    "journal_batches",
];

#[derive(Clone, Copy, Debug)]
pub struct ReadableContextLimits {
    pub max_input_bytes: usize,
    pub max_work_bytes: u64,
}
impl ReadableContextLimits {
    fn validate(self) -> Result<()> {
        if self.max_input_bytes == 0
            || self.max_input_bytes > MAX_INPUT_BYTES
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("readable context limits"));
        }
        Ok(())
    }
}

/// `Absent` means there was no presentation vocabulary or no source context.
/// `Sidecar` contains Python sorted-compact UTF-8 JSON, including an exact-root
/// `requires-exact-context` or `unavailable` state when presentation refuses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadableContextCarrier {
    Absent,
    Sidecar(Vec<u8>),
}

pub struct ReadableContextCompiler {
    vocabulary: Value,
    vocabulary_ref: Value,
    rules: BTreeMap<(String, String), Value>,
    known_schemas: BTreeSet<String>,
    limits: ReadableContextLimits,
}

fn json_limits(cap: usize) -> Result<JsonLimits> {
    JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("readable context JSON limits"))
}

fn canonical_raw(raw: &[u8], cap: usize) -> Result<Vec<u8>> {
    let limits = json_limits(cap)?;
    let parsed = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|e| Error::Source(e.to_string()))
}

fn canonical(value: &Value, cap: usize) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(value).map_err(|_| Error::Invalid("readable context JSON"))?;
    if raw.len() > cap {
        return Err(Error::Budget("readable context canonical bytes"));
    }
    canonical_raw(&raw, cap)
}

fn sha_ref(bytes: &[u8]) -> String {
    format!("sha256:{}", Digest256::of_bytes(bytes).to_hex())
}

fn pointer_parts(pointer: &str) -> Result<Vec<String>> {
    if pointer.len() > MAX_POINTER_BYTES || (!pointer.is_empty() && !pointer.starts_with('/')) {
        return Err(Error::Invalid("invalid-context-pointer"));
    }
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    pointer[1..]
        .split('/')
        .map(|part| {
            let mut text = String::new();
            let mut chars = part.chars();
            while let Some(ch) = chars.next() {
                if ch == '~' {
                    match chars.next() {
                        Some('0') => text.push('~'),
                        Some('1') => text.push('/'),
                        _ => return Err(Error::Invalid("invalid-context-pointer")),
                    }
                } else {
                    text.push(ch);
                }
            }
            Ok(text)
        })
        .collect()
}

fn at<'a>(root: &'a Value, pointer: &str) -> Result<&'a Value> {
    let mut current = root;
    for part in pointer_parts(pointer)? {
        current = match current {
            Value::Object(map) => map.get(&part),
            Value::Array(items)
                if part == "0"
                    || (!part.starts_with('0') && part.bytes().all(|c| c.is_ascii_digit())) =>
            {
                part.parse::<usize>()
                    .ok()
                    .and_then(|index| items.get(index))
            }
            _ => None,
        }
        .ok_or(Error::Invalid("unresolved-context-pointer"))?;
    }
    Ok(current)
}

fn at_ordered<'a>(root: &'a JsonValue, pointer: &str) -> Result<&'a JsonValue> {
    let mut current = root;
    for part in pointer_parts(pointer)? {
        current = match current {
            JsonValue::Object(entries) => entries
                .iter()
                .find(|(key, _)| key.as_str() == Some(part.as_str()))
                .map(|(_, value)| value),
            JsonValue::Array(items)
                if part == "0"
                    || (!part.starts_with('0') && part.bytes().all(|c| c.is_ascii_digit())) =>
            {
                part.parse::<usize>()
                    .ok()
                    .and_then(|index| items.get(index))
            }
            _ => None,
        }
        .ok_or(Error::Invalid("unresolved-context-pointer"))?;
    }
    Ok(current)
}

fn ordered_keys(root: &JsonValue, pointer: &str) -> Result<Vec<String>> {
    match at_ordered(root, pointer)? {
        JsonValue::Object(entries) => entries
            .iter()
            .map(|(key, _)| {
                key.as_str()
                    .map(str::to_owned)
                    .ok_or(Error::Invalid("context object key"))
            })
            .collect(),
        _ => Err(Error::Invalid("context ordered object")),
    }
}

fn at_ordered_mut<'a>(root: &'a mut JsonValue, pointer: &str) -> Result<&'a mut JsonValue> {
    let mut current = root;
    for part in pointer_parts(pointer)? {
        current = match current {
            JsonValue::Object(entries) => entries
                .iter_mut()
                .find(|(key, _)| key.as_str() == Some(part.as_str()))
                .map(|(_, value)| value),
            JsonValue::Array(items) => part
                .parse::<usize>()
                .ok()
                .and_then(|index| items.get_mut(index)),
            _ => None,
        }
        .ok_or(Error::Invalid("unresolved readable witness pointer"))?;
    }
    Ok(current)
}

pub(crate) fn emit_ordered(value: &JsonValue, out: &mut Vec<u8>, cap: usize) -> Result<()> {
    match value {
        JsonValue::Null => out.extend_from_slice(b"null"),
        JsonValue::Bool(value) => out.extend_from_slice(if *value { b"true" } else { b"false" }),
        JsonValue::Number(value) => out.extend_from_slice(value.lexeme.as_bytes()),
        JsonValue::String(value) => serde_json::to_writer(
            &mut *out,
            value
                .as_str()
                .ok_or(Error::Invalid("readable witness UTF-8"))?,
        )
        .map_err(|_| Error::Invalid("readable witness string"))?,
        JsonValue::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                emit_ordered(item, out, cap)?;
            }
            out.push(b']');
        }
        JsonValue::Object(items) => {
            out.push(b'{');
            for (index, (key, item)) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(
                    &mut *out,
                    key.as_str()
                        .ok_or(Error::Invalid("readable witness key UTF-8"))?,
                )
                .map_err(|_| Error::Invalid("readable witness key"))?;
                out.push(b':');
                emit_ordered(item, out, cap)?;
            }
            out.push(b'}');
        }
    }
    if out.len() > cap {
        return Err(Error::Budget("readable witness bytes"));
    }
    Ok(())
}

fn replace_ordered(
    target: &mut JsonValue,
    pointer: &str,
    source: &JsonValue,
    cap: usize,
) -> Result<()> {
    let existing = at_ordered(target, pointer)?;
    let limits = json_limits(cap)?;
    if canonical_bytes_v1(existing, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(|e| Error::Source(e.to_string()))?
        != canonical_bytes_v1(source, CanonicalProfile::SourceRecordDigestV1, limits)
            .map_err(|e| Error::Source(e.to_string()))?
    {
        return Err(Error::Invalid("readable witness copied source differs"));
    }
    *at_ordered_mut(target, pointer)? = source.clone();
    Ok(())
}

/// Rehydrate only the finite source-context copies used by the readable
/// compiler. This preserves original record/qualifier member order after the
/// normalized carrier passed through sorted serde maps. Every replacement
/// requires canonical equality; values and revisions cannot be changed here.
/// Referenced contexts require their actual ordered Claim source witnesses.
pub fn ordered_readable_witness(
    normalized_raw: &[u8],
    owner_raw: &[u8],
    referenced_sources: &[&[u8]],
    max_bytes: usize,
) -> Result<Vec<u8>> {
    if max_bytes == 0
        || max_bytes > MAX_INPUT_BYTES
        || normalized_raw.len() > max_bytes
        || owner_raw.len() > max_bytes
        || referenced_sources.len() > MAX_CONTEXTS
    {
        return Err(Error::Budget("readable source witness limits"));
    }
    let limits = json_limits(max_bytes)?;
    let parse = |raw: &[u8]| {
        parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))
    };
    let normalized: Value = serde_json::from_slice(normalized_raw)
        .map_err(|_| Error::Invalid("readable witness normalized JSON"))?;
    let mut ordered = parse(normalized_raw)?.into_root();
    let owner = parse(owner_raw)?.into_root();
    if normalized.pointer("/source_record/payload").is_some() {
        replace_ordered(&mut ordered, "/source_record/payload", &owner, max_bytes)?;
    }
    if let Some(fields) = normalized
        .pointer("/source_record/field_map")
        .and_then(Value::as_object)
    {
        for (field, pointer) in fields {
            let Some(key) = field.strip_prefix("attributes.") else {
                continue;
            };
            let pointer = pointer
                .as_str()
                .ok_or(Error::Invalid("readable witness field map"))?;
            let copied = at_ordered(&owner, pointer)?;
            replace_ordered(
                &mut ordered,
                &format!("/attributes/{}", escape(key)),
                copied,
                max_bytes,
            )?;
        }
    }
    let mut sources = vec![owner];
    let mut work = normalized_raw
        .len()
        .checked_add(owner_raw.len())
        .ok_or(Error::Budget("readable witness source work"))?;
    for raw in referenced_sources {
        work = work
            .checked_add(raw.len())
            .ok_or(Error::Budget("readable witness source work"))?;
        if raw.len() > max_bytes || work > max_bytes.saturating_mul(MAX_CONTEXTS + 2) {
            return Err(Error::Budget("readable witness source work"));
        }
        sources.push(parse(raw)?.into_root());
    }
    if let Some(contexts) = normalized
        .pointer("/semantics/assertion_contexts")
        .and_then(Value::as_array)
    {
        for (index, context) in contexts.iter().enumerate() {
            let root = format!("/semantics/assertion_contexts/{index}");
            let digest = text(context, "source_record_digest")
                .ok_or(Error::Invalid("readable witness context digest"))?;
            // Normalizers can bind an embedded exact record (record-version
            // view), so inspect that declared source layer as well as its outer.
            let mut matched = None;
            for source in &sources {
                for pointer in [
                    "",
                    "/properties/source_claim",
                    "/properties/record_version_view/record",
                ] {
                    let Ok(candidate) = at_ordered(source, pointer) else {
                        continue;
                    };
                    let mut raw = Vec::new();
                    emit_ordered(candidate, &mut raw, max_bytes)?;
                    let value: Value = serde_json::from_slice(&raw)
                        .map_err(|_| Error::Invalid("readable witness context source"))?;
                    if stable_digest(&value)? == digest {
                        matched = Some(candidate);
                        break;
                    }
                }
                if matched.is_some() {
                    break;
                }
            }
            let source =
                matched.ok_or(Error::Invalid("readable referenced source order absent"))?;
            let fields = context
                .get("fields")
                .and_then(Value::as_object)
                .ok_or(Error::Invalid("readable witness assertion fields"))?;
            for (key, field) in fields {
                let pointer = text(field, "source_pointer")
                    .ok_or(Error::Invalid("readable witness assertion pointer"))?;
                // Embedded record-version pointers refer to the outer owner
                // even when its context digest is the selected inner record.
                let copied = at_ordered(source, pointer).or_else(|_| {
                    at_ordered(
                        source,
                        pointer
                            .strip_prefix("/properties/record_version_view/record")
                            .or_else(|| pointer.strip_prefix("/properties/source_claim"))
                            .unwrap_or(pointer),
                    )
                })?;
                replace_ordered(
                    &mut ordered,
                    &format!("{root}/fields/{}/value", escape(key)),
                    copied,
                    max_bytes,
                )?;
            }
            let mut field_order = Vec::new();
            if fields.contains_key("record") {
                field_order.push("record");
            }
            for pointer in ["", "/properties", "/properties/source_claim"] {
                if let Ok(layer) = at_ordered(source, pointer) {
                    for key in crate::knowledge_source_navigation_node::ASSERTION_FIELDS {
                        if layer.object_get(key).is_some()
                            && fields.contains_key(*key)
                            && !field_order.contains(key)
                        {
                            field_order.push(key);
                        }
                    }
                }
            }
            if field_order.len() != fields.len() {
                return Err(Error::Invalid("readable assertion field order coverage"));
            }
            if let JsonValue::Object(entries) =
                at_ordered_mut(&mut ordered, &format!("{root}/fields"))?
            {
                entries.sort_by_key(|(key, _)| {
                    field_order
                        .iter()
                        .position(|field| key.as_str() == Some(*field))
                });
            }
        }
    }
    let mut result = Vec::new();
    emit_ordered(&ordered, &mut result, max_bytes)?;
    if canonical_raw(&result, max_bytes)? != canonical_raw(normalized_raw, max_bytes)? {
        return Err(Error::Invalid("readable reconstructed carrier differs"));
    }
    Ok(result)
}

fn escape(text: &str) -> String {
    text.replace('~', "~0").replace('/', "~1")
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn exact_ref(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    map.len() == 3
        && map.contains_key("id")
        && map.contains_key("version")
        && map.contains_key("digest")
        && text(value, "id").is_some_and(|id| !id.is_empty())
        && value
            .get("version")
            .and_then(Value::as_u64)
            .is_some_and(|n| n > 0 && n <= MAX_SAFE_INTEGER)
        && text(value, "digest").is_some_and(|digest| {
            digest.len() == 71
                && digest.starts_with("sha256:")
                && digest[7..]
                    .bytes()
                    .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
        })
}

fn source_identity(record: &Value, form_root: bool) -> Result<(String, u64)> {
    if text(record, "schema_version") == Some("tos_canonical_node_v1") {
        let kind =
            text(record, "node_type").ok_or(Error::Invalid("invalid-context-source-record"))?;
        if !matches!(
            kind,
            "source"
                | "concept"
                | "principle"
                | "lineage"
                | "event"
                | "state"
                | "support"
                | "context"
                | "analogy"
                | "synthesis"
        ) || record.get("record_id").is_some()
        {
            return Err(Error::Invalid("invalid-context-source-record"));
        }
        let id = text(record, "node_id").ok_or(Error::Invalid("invalid-context-source-record"))?;
        let prefix = format!("tos.{kind}.");
        if !id.starts_with(&prefix)
            || id[prefix.len()..].is_empty()
            || !id[prefix.len()..].bytes().all(|ch| {
                ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'.' || ch == b'-'
            })
            || id.ends_with('.')
            || id.ends_with('-')
            || id.contains("..")
            || id.contains("--")
            || id.contains(".-")
            || id.contains("-.")
        {
            return Err(Error::Invalid("invalid-context-source-record"));
        }
        return Ok((
            id.to_owned(),
            record
                .get("record_version")
                .and_then(Value::as_u64)
                .filter(|n| *n > 0 && *n <= MAX_SAFE_INTEGER)
                .ok_or(Error::Invalid("invalid-context-source-record"))?,
        ));
    }
    let ids = if form_root {
        &[
            "record_id",
            "claim_id",
            "form_id",
            "artifact_id",
            "composite_id",
        ][..]
    } else {
        &["record_id", "claim_id", "artifact_id", "composite_id"][..]
    };
    let id = ids
        .iter()
        .find(|name| record.get(**name).is_some())
        .and_then(|name| text(record, name))
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("invalid-context-source-record"))?;
    let version_key = if record.get("claim_id").is_some() {
        "claim_version"
    } else if form_root && record.get("form_id").is_some() {
        "form_version"
    } else {
        "record_version"
    };
    let version = record
        .get(version_key)
        .and_then(Value::as_u64)
        .filter(|n| *n > 0 && *n <= MAX_SAFE_INTEGER)
        .ok_or(Error::Invalid("invalid-context-source-record"))?;
    Ok((id.to_owned(), version))
}

fn root_pointers(item: &Value) -> Vec<Value> {
    let attrs = item.get("attributes").and_then(Value::as_object);
    let mut roots = Vec::new();
    if item
        .pointer("/semantics/assertion_contexts")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        roots.push(Value::String("/semantics/assertion_contexts".into()));
    }
    if attrs
        .and_then(|a| a.get("human_forms"))
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        roots.push(Value::String("/attributes/human_forms".into()));
    }
    for key in ["source_record", "source_claim"] {
        if attrs.and_then(|a| a.get(key)).is_some_and(|v| !v.is_null()) {
            roots.push(Value::String(format!("/attributes/{key}")));
        }
    }
    if roots.is_empty() {
        roots.push(Value::String("/semantics".into()));
    }
    roots
}

fn labels(value: &Value, languages: &BTreeSet<String>) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    map.len() == languages.len()
        && map.keys().all(|key| languages.contains(key))
        && map.values().all(|v| {
            v.as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= 1024)
        })
}

fn language_tag(value: &str) -> bool {
    let mut parts = value.split('-');
    let first = parts.next().unwrap_or("");
    let private = first.eq_ignore_ascii_case("i") || first.eq_ignore_ascii_case("x");
    if private && !value.contains('-') {
        return false;
    }
    if !private
        && (first.len() < 2 || first.len() > 8 || !first.bytes().all(|b| b.is_ascii_alphabetic()))
    {
        return false;
    }
    parts.all(|part| {
        !part.is_empty() && part.len() <= 8 && part.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

fn source_schema_tag(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos_") else {
        return false;
    };
    let Some((body, version)) = rest.rsplit_once("_v") else {
        return false;
    };
    !body.is_empty()
        && body
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && !version.is_empty()
        && version.bytes().all(|b| b.is_ascii_digit())
}

fn vocabulary(
    value: &Value,
    cap: usize,
) -> Result<(Value, BTreeMap<(String, String), Value>, BTreeSet<String>)> {
    let map = value
        .as_object()
        .ok_or(Error::Invalid("context presentation object"))?;
    let fields = [
        "schema_version",
        "presentation_id",
        "presentation_version",
        "owner_ref",
        "purpose",
        "default_language",
        "languages",
        "max_output_bytes",
        "max_entries",
        "record_schema_versions",
        "field_rules",
        "unclassified",
    ];
    if map.len() != fields.len()
        || fields.iter().any(|field| !map.contains_key(*field))
        || text(value, "schema_version") != Some("tos_context_presentation_v1")
        || text(value, "presentation_id") != Some("tos.context-presentation.governing")
        || text(value, "owner_ref") != Some(OWNER_REF)
        || text(value, "purpose") != Some("source-context-reading-not-assessment")
        || value["presentation_version"]
            .as_u64()
            .is_none_or(|v| v == 0 || v > MAX_SAFE_INTEGER)
        || value["max_output_bytes"]
            .as_u64()
            .is_none_or(|v| !(1024..=MAX_OUTPUT_BYTES as u64).contains(&v))
        || value["max_entries"]
            .as_u64()
            .is_none_or(|v| !(1..=MAX_ENTRIES as u64).contains(&v))
    {
        return Err(Error::Invalid("context presentation finite owner contract"));
    }
    let languages = value["languages"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 8)
        .ok_or(Error::Invalid("context presentation languages"))?;
    let mut language_set = BTreeSet::new();
    let mut folded = BTreeSet::new();
    for language in languages {
        let lang = language
            .as_str()
            .ok_or(Error::Invalid("context presentation language"))?;
        if matches!(
            lang.to_ascii_lowercase().as_str(),
            "default" | "original" | "auto"
        ) || !language_tag(lang)
            || !folded.insert(lang.to_ascii_lowercase())
        {
            return Err(Error::Invalid("context presentation language"));
        }
        language_set.insert(lang.to_owned());
    }
    if !text(value, "default_language").is_some_and(|s| language_set.contains(s)) {
        return Err(Error::Invalid("context presentation default language"));
    }
    let schemas = value["record_schema_versions"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 128)
        .ok_or(Error::Invalid("context source-schema selectors"))?;
    let mut known_schemas = BTreeSet::new();
    for schema in schemas {
        let name = schema
            .as_str()
            .ok_or(Error::Invalid("context source schema"))?;
        if !source_schema_tag(name) || !known_schemas.insert(name.to_owned()) {
            return Err(Error::Invalid("context source schema"));
        }
    }
    let unknown = value["unclassified"]
        .as_object()
        .ok_or(Error::Invalid("context unclassified labels"))?;
    if unknown.len() != 2
        || !unknown
            .get("label")
            .is_some_and(|v| labels(v, &language_set))
        || !unknown
            .get("explanation")
            .is_some_and(|v| labels(v, &language_set))
    {
        return Err(Error::Invalid("context unclassified labels"));
    }
    let rules = value["field_rules"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 128)
        .ok_or(Error::Invalid("context field rules"))?;
    let mut routed = BTreeMap::new();
    for rule in rules {
        let row = rule
            .as_object()
            .ok_or(Error::Invalid("context field rule"))?;
        let names = [
            "field",
            "targets",
            "category",
            "label",
            "explanation",
            "value_labels",
        ];
        let field = text(rule, "field").ok_or(Error::Invalid("context rule field"))?;
        if row.len() != names.len()
            || names.iter().any(|n| !row.contains_key(*n))
            || field.len() > 96
            || !field.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
            || !field
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            || !matches!(text(rule, "category"), Some("governing" | "technical"))
            || (text(rule, "category") == Some("technical") && !TECHNICAL.contains(&field))
            || !labels(&rule["label"], &language_set)
            || (!rule["explanation"].is_null() && !labels(&rule["explanation"], &language_set))
        {
            return Err(Error::Invalid("context field rule"));
        }
        if let Some(value_labels) = rule["value_labels"].as_object() {
            if value_labels.is_empty()
                || value_labels.len() > 64
                || value_labels.iter().any(|(key, label)| {
                    key.is_empty() || key.len() > 128 || !labels(label, &language_set)
                })
            {
                return Err(Error::Invalid("context value labels"));
            }
        } else if !rule["value_labels"].is_null() {
            return Err(Error::Invalid("context value labels"));
        }
        let targets = rule["targets"]
            .as_array()
            .filter(|a| !a.is_empty())
            .ok_or(Error::Invalid("context rule targets"))?;
        let mut local = BTreeSet::new();
        for target in targets {
            let name = target
                .as_str()
                .ok_or(Error::Invalid("context rule target"))?;
            if !matches!(
                name,
                "record"
                    | "assertion"
                    | "language-context"
                    | "subject-assessment"
                    | "assessment-snapshot"
            ) || !local.insert(name)
                || routed
                    .insert((name.to_owned(), field.to_owned()), rule.clone())
                    .is_some()
            {
                return Err(Error::Invalid("context ambiguous field rule"));
            }
        }
    }
    let packet = canonical(value, cap)?;
    let reference = json!({"id":value["presentation_id"],"version":value["presentation_version"],
        "source_ref":REGISTRY_REF,"digest":sha_ref(&packet)});
    Ok((reference, routed, known_schemas))
}

impl ReadableContextCompiler {
    /// `None` is the historical no-vocabulary state. The caller owns the
    /// selected registry bytes and all source/rights checks.
    pub fn from_selected_registry_bytes(
        raw: &[u8],
        expected_sha256: &str,
        limits: ReadableContextLimits,
    ) -> Result<Option<Self>> {
        limits.validate()?;
        if raw.is_empty() || raw.len() > MAX_REGISTRY_BYTES {
            return Err(Error::Budget("context registry bytes"));
        }
        if Digest256::of_bytes(raw).to_hex() != expected_sha256 {
            return Err(Error::Invalid("selected context registry SHA"));
        }
        parse_json(
            raw,
            JsonMode::PublishedStrict,
            json_limits(MAX_REGISTRY_BYTES)?,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        let registry: Value =
            serde_json::from_slice(raw).map_err(|_| Error::Invalid("context registry JSON"))?;
        let Some(presentation) = registry.get("context_presentation") else {
            return Ok(None);
        };
        if presentation.is_null() {
            return Ok(None);
        }
        let (reference, rules, known_schemas) = vocabulary(presentation, MAX_REGISTRY_BYTES)?;
        Ok(Some(Self {
            vocabulary: presentation.clone(),
            vocabulary_ref: reference,
            rules,
            known_schemas,
            limits,
        }))
    }

    /// `ordered_raw` must retain source-object member order from before
    /// `serde_json::Value` normalization. Canonical equality rejects a forged
    /// witness whose values differ. Without it, context-bearing input refuses.
    pub fn compile(
        &self,
        normalized_raw: &[u8],
        ordered_raw: Option<&[u8]>,
    ) -> Result<ReadableContextCarrier> {
        if normalized_raw.is_empty() || normalized_raw.len() > self.limits.max_input_bytes {
            return Err(Error::Budget("normalized context carrier bytes"));
        }
        let limit = json_limits(self.limits.max_input_bytes)?;
        parse_json(normalized_raw, JsonMode::PublishedStrict, limit)
            .map_err(|e| Error::Source(e.to_string()))?;
        let item: Value = serde_json::from_slice(normalized_raw)
            .map_err(|_| Error::Invalid("normalized context JSON"))?;
        let attrs = item.get("attributes").and_then(Value::as_object);
        let has_context = attrs
            .and_then(|a| a.get("source_record"))
            .is_some_and(|v| !v.is_null())
            || attrs
                .and_then(|a| a.get("source_claim"))
                .is_some_and(|v| !v.is_null())
            || attrs
                .and_then(|a| a.get("human_forms"))
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty())
            || item
                .pointer("/semantics/assertion_contexts")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty());
        if !has_context {
            return Ok(ReadableContextCarrier::Absent);
        }
        let ordered_raw =
            ordered_raw.ok_or(Error::Invalid("context source member order witness absent"))?;
        if ordered_raw.is_empty() || ordered_raw.len() > self.limits.max_input_bytes {
            return Err(Error::Budget("ordered context witness bytes"));
        }
        let ordered = parse_json(ordered_raw, JsonMode::PublishedStrict, limit)
            .map_err(|e| Error::Source(e.to_string()))?;
        if canonical_raw(normalized_raw, self.limits.max_input_bytes)?
            != canonical_bytes_v1(
                ordered.root(),
                CanonicalProfile::SourceRecordDigestV1,
                limit,
            )
            .map_err(|e| Error::Source(e.to_string()))?
        {
            return Err(Error::Invalid("context order witness value mismatch"));
        }
        let mut build = Builder::new(self, &item, ordered.root());
        let attempt = build
            .charge(normalized_raw.len().saturating_add(ordered_raw.len()))
            .and_then(|()| build.populate());
        let result = match attempt {
            Ok(()) => build.result("complete", "all-returned-context-covered"),
            Err(Error::Budget(_)) => {
                build.refusal("requires-exact-context", "context-presentation-budget")
            }
            Err(Error::Invalid(reason)) => build.refusal("unavailable", reason),
            Err(error) => return Err(error),
        };
        let bytes = canonical(&result, MAX_OUTPUT_BYTES)?;
        if bytes.len() > self.vocabulary["max_output_bytes"].as_u64().unwrap_or(0) as usize {
            return Err(Error::Budget("readable context refusal bytes"));
        }
        Ok(ReadableContextCarrier::Sidecar(bytes))
    }
}

struct Builder<'a> {
    compiler: &'a ReadableContextCompiler,
    item: &'a Value,
    ordered: &'a JsonValue,
    contexts: Vec<Value>,
    pointers: Vec<Value>,
    materials: Vec<Value>,
    material_index: BTreeMap<String, usize>,
    input_contexts: u64,
    entries: u64,
    unclassified: u64,
    work: u64,
}

impl<'a> Builder<'a> {
    fn new(compiler: &'a ReadableContextCompiler, item: &'a Value, ordered: &'a JsonValue) -> Self {
        Self {
            compiler,
            item,
            ordered,
            contexts: Vec::new(),
            pointers: Vec::new(),
            materials: Vec::new(),
            material_index: BTreeMap::new(),
            input_contexts: 0,
            entries: 0,
            unclassified: 0,
            work: 0,
        }
    }
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.work = self
            .work
            .checked_add(bytes as u64)
            .ok_or(Error::Budget("readable context work"))?;
        if self.work > self.compiler.limits.max_work_bytes {
            return Err(Error::Budget("readable context work"));
        }
        Ok(())
    }
    fn same_canonical(&mut self, left: &Value, right: &Value) -> Result<bool> {
        let first = canonical(left, self.compiler.limits.max_input_bytes)?;
        self.charge(first.len())?;
        let second = canonical(right, self.compiler.limits.max_input_bytes)?;
        self.charge(second.len())?;
        Ok(first == second)
    }
    fn result(&self, state: &str, reason: &str) -> Value {
        json!({"schema_version":"tos_readable_context_v1","state":state,"reason":reason,
            "vocabulary":self.compiler.vocabulary_ref,"contexts":self.contexts,
            "exact_context_pointers":self.pointers,"exact_materials":self.materials,
            "coverage":{"input_contexts":self.input_contexts,"returned_contexts":self.contexts.len(),
                "entries":self.entries,"unclassified_entries":self.unclassified},
            "performs_semantic_assessment":false,"performs_translation":false})
    }
    fn refusal(&self, state: &str, reason: &str) -> Value {
        json!({"schema_version":"tos_readable_context_v1","state":state,
            "reason":reason.chars().take(128).collect::<String>(),"vocabulary":self.compiler.vocabulary_ref,
            "contexts":[],"exact_context_pointers":root_pointers(self.item),"exact_materials":[],
            "coverage":{"input_contexts":self.input_contexts,"returned_contexts":0,
                "entries":0,"unclassified_entries":0},
            "performs_semantic_assessment":false,"performs_translation":false})
    }
    fn check_budget(&mut self) -> Result<()> {
        if self.entries
            > self.compiler.vocabulary["max_entries"]
                .as_u64()
                .unwrap_or(0)
        {
            return Err(Error::Budget("context presentation entries"));
        }
        let bytes = canonical(
            &self.result("complete", "all-returned-context-covered"),
            self.compiler.limits.max_input_bytes,
        )?;
        self.charge(bytes.len())?;
        if bytes.len()
            > self.compiler.vocabulary["max_output_bytes"]
                .as_u64()
                .unwrap_or(0) as usize
        {
            return Err(Error::Budget("context presentation output"));
        }
        Ok(())
    }
    fn add_context(&mut self, pointer: &str, form: Option<&Value>) -> Result<usize> {
        pointer_parts(pointer)?;
        at(self.item, pointer)?;
        if self.pointers.len() >= MAX_CONTEXTS {
            return Err(Error::Budget("context count"));
        }
        self.pointers.push(Value::String(pointer.to_owned()));
        self.contexts
            .push(json!({"origin_pointer":pointer,"form":form,"entries":[]}));
        Ok(self.contexts.len() - 1)
    }
    fn add_material(&mut self, pointer: &str, value: &Value) -> Result<()> {
        let item = self.item;
        if !self.same_canonical(at(item, pointer)?, value)? {
            return Err(Error::Invalid("exact-material-origin-mismatch"));
        }
        let bytes = canonical(value, self.compiler.limits.max_input_bytes)?;
        self.charge(bytes.len())?;
        let digest = sha_ref(&bytes);
        let index = if let Some(index) = self.material_index.get(&digest) {
            *index
        } else {
            if self.materials.len() >= MAX_ENTRIES {
                return Err(Error::Budget("context materials"));
            }
            let index = self.materials.len();
            let canonical_json =
                String::from_utf8(bytes).map_err(|_| Error::Invalid("context material UTF-8"))?;
            self.materials.push(
                json!({"digest":digest,"canonical_json":canonical_json,"origin_pointers":[]}),
            );
            self.material_index.insert(digest, index);
            index
        };
        let origins = self.materials[index]["origin_pointers"]
            .as_array_mut()
            .ok_or(Error::Invalid("context material origins"))?;
        if !origins
            .iter()
            .any(|origin| origin.as_str() == Some(pointer))
        {
            if origins.len() >= MAX_ENTRIES {
                return Err(Error::Budget("context material origins"));
            }
            origins.push(Value::String(pointer.to_owned()));
        }
        self.check_budget()
    }
    fn language(record: Option<&Value>, key: &str) -> (Value, Value) {
        let declared = record
            .and_then(|r| r.get("field_languages"))
            .and_then(|v| v.get(key))
            .filter(|v| v.is_object())
            .or_else(|| record.and_then(|r| r.get(key)));
        let Some(map) = declared.and_then(Value::as_object) else {
            return (Value::Null, Value::Null);
        };
        let language = map
            .get("language")
            .and_then(Value::as_str)
            .filter(|s| language_tag(s))
            .map(|s| Value::String(s.to_owned()))
            .unwrap_or(Value::Null);
        let script = map
            .get("script")
            .and_then(Value::as_str)
            .filter(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphabetic()))
            .map(|s| Value::String(s.to_owned()))
            .unwrap_or(Value::Null);
        (language, script)
    }
    fn add_entry(
        &mut self,
        context: usize,
        key: &str,
        value: &Value,
        pointer: &str,
        binding: Value,
        target: &str,
        record: Option<&Value>,
        force_unknown: bool,
    ) -> Result<()> {
        let item = self.item;
        if !self.same_canonical(at(item, pointer)?, value)? {
            return Err(Error::Invalid("context-value-pointer-mismatch"));
        }
        let rule = if force_unknown {
            None
        } else {
            self.compiler
                .rules
                .get(&(target.to_owned(), key.to_owned()))
        };
        let enum_labels = rule
            .and_then(|r| r.get("value_labels"))
            .and_then(Value::as_object);
        let enum_label = value
            .as_str()
            .and_then(|s| enum_labels.and_then(|labels| labels.get(s)));
        let unknown_enum = enum_labels.is_some() && enum_label.is_none();
        let category = if unknown_enum {
            "unclassified"
        } else {
            rule.and_then(|r| text(r, "category"))
                .unwrap_or("unclassified")
        };
        let label = rule
            .map(|r| r["label"].clone())
            .unwrap_or_else(|| self.compiler.vocabulary["unclassified"]["label"].clone());
        let explanation = if !unknown_enum {
            rule.map(|r| r["explanation"].clone())
        } else {
            None
        }
        .unwrap_or_else(|| self.compiler.vocabulary["unclassified"]["explanation"].clone());
        let value_mode = if category == "technical" {
            "exact-reference"
        } else if enum_label.is_some() {
            "vocabulary-value"
        } else {
            "source-value"
        };
        let (language, script) = Self::language(record, key);
        let mut entry = json!({"key":key,"category":category,"label":label,"explanation":explanation,
            "value_mode":value_mode,"value_label":enum_label,"language":language,"script":script,
            "binding":binding,"value_pointer":pointer});
        if category != "technical" {
            entry["value"] = value.clone();
        }
        self.contexts
            .get_mut(context)
            .ok_or(Error::Invalid("context index"))?["entries"]
            .as_array_mut()
            .ok_or(Error::Invalid("context entries"))?
            .push(entry);
        self.entries = self
            .entries
            .checked_add(1)
            .ok_or(Error::Budget("context entries"))?;
        if category == "unclassified" {
            self.unclassified += 1;
        }
        self.check_budget()
    }
    fn source_ref(&mut self, record: &Value) -> Result<Value> {
        let (id, version) = source_identity(record, false)?;
        let bytes = canonical(record, self.compiler.limits.max_input_bytes)?;
        self.charge(bytes.len())?;
        let reference = json!({"id":id,"version":version,"digest":sha_ref(&bytes)});
        if let Some(declared) = self.item.pointer("/attributes/source_sha256") {
            if !declared.is_null()
                && declared.as_str().map(|s| format!("sha256:{s}"))
                    != reference["digest"].as_str().map(str::to_owned)
            {
                return Err(Error::Invalid("context-source-record-digest-mismatch"));
            }
        }
        Ok(reference)
    }

    fn populate(&mut self) -> Result<()> {
        let item = self.item;
        let empty_attrs = Map::new();
        let empty_semantics = Map::new();
        let empty_contexts = Vec::new();
        let empty_forms = Vec::new();
        let attrs = match item.get("attributes") {
            None | Some(Value::Null) => &empty_attrs,
            Some(Value::Object(map)) => map,
            _ => return Err(Error::Invalid("invalid-context-carrier")),
        };
        let semantics = match item.get("semantics") {
            None => &empty_semantics,
            Some(Value::Object(map)) => map,
            _ => return Err(Error::Invalid("invalid-context-carrier")),
        };
        let contexts = match semantics.get("assertion_contexts") {
            None => &empty_contexts,
            Some(Value::Array(rows)) => rows,
            _ => return Err(Error::Invalid("invalid-context-collection")),
        };
        let forms = match attrs.get("human_forms") {
            None => &empty_forms,
            Some(Value::Array(rows)) => rows,
            _ => return Err(Error::Invalid("invalid-context-collection")),
        };
        let source_record = attrs.get("source_record").filter(|v| !v.is_null());
        let source_claim = attrs.get("source_claim").filter(|v| !v.is_null());
        if source_record.is_some() && source_claim.is_some() {
            return Err(Error::Invalid("ambiguous-context-source-record"));
        }
        let known = source_record.or(source_claim);
        let known_key = if source_record.is_some() {
            "source_record"
        } else {
            "source_claim"
        };
        let ready = forms
            .iter()
            .filter(|f| text(f, "state") == Some("ready"))
            .count();
        self.input_contexts = (contexts.len()
            + ready
            + usize::from(known.is_some_and(Value::is_object) && ready == 0))
            as u64;
        if let Some(record) = known.filter(|v| v.is_object()) {
            self.add_material(&format!("/attributes/{known_key}"), record)?;
            if ready == 0 {
                self.direct_record(record, known_key)?;
            }
        }
        for (index, context) in contexts.iter().enumerate() {
            self.assertion_context(index, context, known)?;
        }
        for (index, form) in forms.iter().enumerate() {
            self.form_context(index, form, known)?;
        }
        self.check_budget()
    }

    fn direct_record(&mut self, record: &Value, key: &str) -> Result<()> {
        let pointer = format!("/attributes/{key}");
        let context = self.add_context(&pointer, None)?;
        let reference = self.source_ref(record)?;
        let known_schema =
            text(record, "schema_version").is_some_and(|s| self.compiler.known_schemas.contains(s));
        for field in ordered_keys(self.ordered, &pointer)? {
            let value = record
                .get(&field)
                .ok_or(Error::Invalid("context record field"))?;
            self.add_entry(context, &field, value, &format!("{pointer}/{}", escape(&field)),
                json!({"kind":"record","record":reference,"source_pointer":format!("/{}", escape(&field))}),
                "record", Some(record), !known_schema)?;
        }
        Ok(())
    }

    fn assertion_context(
        &mut self,
        index: usize,
        raw: &Value,
        known: Option<&Value>,
    ) -> Result<()> {
        let pointer = format!("/semantics/assertion_contexts/{index}");
        let context = self.add_context(&pointer, None)?;
        let digest =
            text(raw, "source_record_digest").ok_or(Error::Invalid("invalid-assertion-context"))?;
        if text(raw, "schema_version") != Some("tos_assertion_context_v1")
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || raw.get("fields").and_then(Value::as_object).is_none()
            || raw.get("conflicts").and_then(Value::as_array).is_none()
        {
            return Err(Error::Invalid("invalid-assertion-context"));
        }
        let item = self.item;
        let own = item.get("source_record").and_then(Value::as_object);
        let source = own
            .filter(|o| o.get("digest").and_then(Value::as_str) == Some(digest))
            .and_then(|o| o.get("payload"));
        if text(raw, "binding_role") == Some("carrier")
            && own.is_some()
            && own.and_then(|o| o.get("digest")).and_then(Value::as_str) != Some(digest)
        {
            return Err(Error::Invalid("assertion-carrier-digest-mismatch"));
        }
        if source.is_some_and(|v| stable_digest(v).ok().as_deref() != Some(digest)) {
            return Err(Error::Invalid("assertion-source-digest-mismatch"));
        }
        self.add_material(&pointer, raw)?;
        let field_pointer = format!("{pointer}/fields");
        for key in ordered_keys(self.ordered, &field_pointer)? {
            let field = raw["fields"]
                .get(&key)
                .ok_or(Error::Invalid("invalid-assertion-field"))?;
            let map = field
                .as_object()
                .ok_or(Error::Invalid("invalid-assertion-field"))?;
            if map.len() != 2 || !map.contains_key("value") || !map.contains_key("source_pointer") {
                return Err(Error::Invalid("invalid-assertion-field"));
            }
            let source_pointer =
                text(field, "source_pointer").ok_or(Error::Invalid("invalid-assertion-field"))?;
            pointer_parts(source_pointer)?;
            let value = &field["value"];
            if let Some(source) = source {
                if !self.same_canonical(at(source, source_pointer)?, value)? {
                    return Err(Error::Invalid("assertion-source-pointer-mismatch"));
                }
            }
            let value_pointer = format!("{field_pointer}/{}/value", escape(&key));
            if key == "record" && value.is_object() {
                let known_schema = text(value, "schema_version")
                    .is_some_and(|s| self.compiler.known_schemas.contains(s));
                for member in ordered_keys(self.ordered, &value_pointer)? {
                    let member_value = value
                        .get(&member)
                        .ok_or(Error::Invalid("assertion record field"))?;
                    self.add_entry(
                        context,
                        &member,
                        member_value,
                        &format!("{value_pointer}/{}", escape(&member)),
                        json!({"kind":"assertion-context","source_record_digest":digest,
                            "source_pointer":format!("{source_pointer}/{}", escape(&member))}),
                        "record",
                        Some(value),
                        !known_schema,
                    )?;
                }
            } else {
                self.add_entry(
                    context,
                    &key,
                    value,
                    &value_pointer,
                    json!({"kind":"assertion-context","source_record_digest":digest,
                        "source_pointer":source_pointer}),
                    "assertion",
                    known,
                    false,
                )?;
            }
        }
        let conflicts = &raw["conflicts"];
        if conflicts.as_array().is_some_and(|rows| !rows.is_empty()) {
            self.add_entry(context, "conflicts", conflicts, &format!("{pointer}/conflicts"),
                json!({"kind":"assertion-context","source_record_digest":digest,"source_pointer":""}),
                "assertion", None, true)?;
        }
        Ok(())
    }

    fn form_context(&mut self, index: usize, packet: &Value, known: Option<&Value>) -> Result<()> {
        if text(packet, "schema_version") != Some("tos_human_form_materialization_v1") {
            return Err(Error::Invalid("invalid-form-context-packet"));
        }
        if text(packet, "state") != Some("ready") {
            if packet
                .get("context")
                .and_then(Value::as_array)
                .is_none_or(|rows| !rows.is_empty())
                || packet
                    .get("display_text")
                    .is_some_and(|text| !text.is_null())
            {
                return Err(Error::Invalid("nonready-form-context-has-wording"));
            }
            return Ok(());
        }
        let form = packet
            .get("form")
            .ok_or(Error::Invalid("invalid-form-context-binding"))?;
        let subject = packet
            .get("subject")
            .ok_or(Error::Invalid("invalid-form-context-binding"))?;
        let entries = packet
            .get("context")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("invalid-form-context-binding"))?;
        if !exact_ref(form) || !exact_ref(subject) {
            return Err(Error::Invalid("invalid-form-context-binding"));
        }
        let packet_pointer = format!("/attributes/human_forms/{index}");
        let pointer = format!("{packet_pointer}/context");
        let context = self.add_context(&pointer, Some(form))?;
        let mut records: BTreeMap<Vec<u8>, Value> = BTreeMap::new();
        if let Some(record) = known.filter(|v| v.is_object()) {
            let reference = self.source_ref(record)?;
            if reference != *subject {
                return Err(Error::Invalid("form-source-version-or-digest-mismatch"));
            }
            records.insert(
                canonical(&reference, self.compiler.limits.max_input_bytes)?,
                record.clone(),
            );
        }
        for (ordinal, entry) in entries.iter().enumerate() {
            let binding = entry
                .get("binding")
                .ok_or(Error::Invalid("invalid-form-source-binding"))?;
            let binding_map = binding
                .as_object()
                .ok_or(Error::Invalid("invalid-form-source-binding"))?;
            if binding_map.len() != 2
                || !binding_map.contains_key("record")
                || !binding_map.contains_key("pointer")
                || !exact_ref(&binding["record"])
                || !entry.get("value").is_some()
            {
                return Err(Error::Invalid("invalid-form-source-binding"));
            }
            let source_pointer = binding["pointer"]
                .as_str()
                .ok_or(Error::Invalid("invalid-form-source-binding"))?;
            pointer_parts(source_pointer)?;
            if source_pointer.is_empty() {
                let value = &entry["value"];
                if binding["record"]["digest"].as_str()
                    != Some(
                        sha_ref(&canonical(value, self.compiler.limits.max_input_bytes)?).as_str(),
                    )
                {
                    return Err(Error::Invalid("form-context-record-digest-mismatch"));
                }
                let (id, version) = source_identity(value, true)?;
                if binding["record"]["id"].as_str() != Some(id.as_str())
                    || binding["record"]["version"].as_u64() != Some(version)
                {
                    return Err(Error::Invalid("form-context-record-version-mismatch"));
                }
                records.insert(
                    canonical(&binding["record"], self.compiler.limits.max_input_bytes)?,
                    value.clone(),
                );
                self.add_material(&format!("{pointer}/{ordinal}/value"), value)?;
            }
        }
        for (ordinal, entry) in entries.iter().enumerate() {
            let binding = &entry["binding"];
            let source_pointer =
                text(binding, "pointer").ok_or(Error::Invalid("invalid-form-source-binding"))?;
            let record = records
                .get(&canonical(
                    &binding["record"],
                    self.compiler.limits.max_input_bytes,
                )?)
                .ok_or(Error::Invalid("form-context-pointer-or-version-mismatch"))?;
            let value = &entry["value"];
            if !self.same_canonical(at(record, source_pointer)?, value)? {
                return Err(Error::Invalid("form-context-pointer-or-version-mismatch"));
            }
            let value_pointer = format!("{pointer}/{ordinal}/value");
            if source_pointer.is_empty() && value.is_object() {
                let known_schema = text(record, "schema_version")
                    .is_some_and(|s| self.compiler.known_schemas.contains(s));
                for key in ordered_keys(self.ordered, &value_pointer)? {
                    let member = value
                        .get(&key)
                        .ok_or(Error::Invalid("form context field"))?;
                    self.add_entry(
                        context,
                        &key,
                        member,
                        &format!("{value_pointer}/{}", escape(&key)),
                        json!({"kind":"record","record":binding["record"],
                            "source_pointer":format!("/{}", escape(&key))}),
                        "record",
                        Some(record),
                        !known_schema,
                    )?;
                }
            } else {
                let key = source_pointer
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .replace("~1", "/")
                    .replace("~0", "~");
                let key = if key.is_empty() {
                    text(entry, "slot").unwrap_or("")
                } else {
                    &key
                };
                let direct = source_pointer.matches('/').count() == 1;
                self.add_entry(context, key, value, &value_pointer,
                    json!({"kind":"record","record":binding["record"],"source_pointer":source_pointer}),
                    "record", Some(record), !direct || !text(record, "schema_version").is_some_and(|s| self.compiler.known_schemas.contains(s)))?;
            }
        }
        for key in [
            "subject_assessment",
            "assessment_snapshot",
            "language_context",
        ] {
            if let Some(value) = packet.get(key) {
                let value_pointer = format!("{packet_pointer}/{key}");
                self.add_material(&value_pointer, value)?;
                self.add_entry(
                    context,
                    key,
                    value,
                    &value_pointer,
                    json!({"kind":"form-materialization","form":form,"subject":subject,
                        "packet_digest":stable_digest(packet)?,"source_pointer":format!("/{key}")}),
                    "assertion",
                    None,
                    false,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTITY: &[u8] =
        include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
    const ORDERED: &str = r#"{"attributes":{"source_claim":{"review_status":"unreviewed","qualifiers":{"negation":false,"calendar":null},"claim_version":1,"claim_id":"tos.claim.test.readable","schema_version":"tos_historical_claim_v1"}},"semantics":{}}"#;
    const ORACLE_COMPLETE: &str =
        "56230a4cfff7d2d0fced0411ba890cb9df44297162d394d159f2de94608a1010";
    const ORACLE_BUDGET: &str = "277e1bca970aa3c183bd42b4a8743763a1a473208a0d4c65a1d749764357848a";
    const ORACLE_READY_FORM: &str =
        "f9d73c74b9e135b84f691fa63639593e2f96719bf9572c20547962ab22e96fb3";
    const ORACLE_NUMERIC: &str = "4fa7e8819c7868c444f36f95f8ab19622cb5b952df4110c2c71923b393f8a153";

    fn limits() -> ReadableContextLimits {
        ReadableContextLimits {
            max_input_bytes: 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
        }
    }
    fn normalized() -> Vec<u8> {
        let item: Value = serde_json::from_str(ORDERED).unwrap();
        serde_json::to_vec(&item).unwrap()
    }
    #[test]
    fn ordered_source_claim_matches_independent_python_oracle() {
        assert!(
            ReadableContextCompiler::from_selected_registry_bytes(
                ENTITY,
                &"0".repeat(64),
                limits()
            )
            .is_err()
        );
        let compiler = ReadableContextCompiler::from_selected_registry_bytes(
            ENTITY,
            &Digest256::of_bytes(ENTITY).to_hex(),
            limits(),
        )
        .unwrap()
        .unwrap();
        let raw = normalized();
        let ReadableContextCarrier::Sidecar(packet) =
            compiler.compile(&raw, Some(ORDERED.as_bytes())).unwrap()
        else {
            panic!("source claim requires sidecar");
        };
        assert_eq!(Digest256::of_bytes(&packet).to_hex(), ORACLE_COMPLETE);
        let sidecar: Value = serde_json::from_slice(&packet).unwrap();
        let entries = sidecar["contexts"][0]["entries"].as_array().unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|v| v["key"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "review_status",
                "qualifiers",
                "claim_version",
                "claim_id",
                "schema_version"
            ]
        );
        assert_eq!(entries[1]["value"]["calendar"], Value::Null);
        assert_eq!(entries[1]["value"]["negation"], Value::Bool(false));
        assert!(compiler.compile(&raw, None).is_err());
        let altered = ORDERED.replace("unreviewed", "reviewed");
        assert!(compiler.compile(&raw, Some(altered.as_bytes())).is_err());

        // Real native normalization copies this ordered owner record into
        // sorted attributes and the source-record envelope. Reconstruct only
        // those exact copies before applying the same existing Python oracle.
        let owner = br#"{"node_id":"n","properties":{"source_claim":{"review_status":"unreviewed","qualifiers":{"negation":false,"calendar":null},"claim_version":1,"claim_id":"tos.claim.test.readable","schema_version":"tos_historical_claim_v1"}}}"#;
        let owner = crate::knowledge_normalization::SourceRow::parse(owner, 1024 * 1024).unwrap();
        let mut native: Value = serde_json::from_slice(&raw).unwrap();
        native["source_record"] = owner
            .source_record(native["attributes"].as_object().unwrap())
            .unwrap();
        let native_raw = serde_json::to_vec(&native).unwrap();
        let source_raw = br#"{"node_id":"n","properties":{"source_claim":{"review_status":"unreviewed","qualifiers":{"negation":false,"calendar":null},"claim_version":1,"claim_id":"tos.claim.test.readable","schema_version":"tos_historical_claim_v1"}}}"#;
        let witness = ordered_readable_witness(&native_raw, source_raw, &[], 1024 * 1024).unwrap();
        let ReadableContextCarrier::Sidecar(rehydrated) =
            compiler.compile(&native_raw, Some(&witness)).unwrap()
        else {
            panic!("native source claim context absent");
        };
        assert_eq!(Digest256::of_bytes(&rehydrated).to_hex(), ORACLE_COMPLETE);
        let different = String::from_utf8(source_raw.to_vec())
            .unwrap()
            .replace("unreviewed", "reviewed");
        assert!(
            ordered_readable_witness(&native_raw, different.as_bytes(), &[], 1024 * 1024).is_err()
        );
    }
    #[test]
    fn owner_entry_budget_returns_exact_root_without_partial_context() {
        let mut registry: Value = serde_json::from_slice(ENTITY).unwrap();
        registry["context_presentation"]["max_entries"] = json!(1);
        let raw_registry = serde_json::to_vec(&registry).unwrap();
        let compiler = ReadableContextCompiler::from_selected_registry_bytes(
            &raw_registry,
            &Digest256::of_bytes(&raw_registry).to_hex(),
            limits(),
        )
        .unwrap()
        .unwrap();
        let ReadableContextCarrier::Sidecar(packet) = compiler
            .compile(&normalized(), Some(ORDERED.as_bytes()))
            .unwrap()
        else {
            panic!("source claim requires refusal sidecar");
        };
        assert_eq!(Digest256::of_bytes(&packet).to_hex(), ORACLE_BUDGET);
        let sidecar: Value = serde_json::from_slice(&packet).unwrap();
        assert_eq!(sidecar["state"], "requires-exact-context");
        assert_eq!(
            sidecar["exact_context_pointers"],
            json!(["/attributes/source_claim"])
        );
        assert_eq!(sidecar["contexts"], json!([]));
    }

    #[test]
    fn ready_form_pointer_and_material_match_independent_python_oracle() {
        let compiler = ReadableContextCompiler::from_selected_registry_bytes(
            ENTITY,
            &Digest256::of_bytes(ENTITY).to_hex(),
            limits(),
        )
        .unwrap()
        .unwrap();
        let record = json!({"claim_id":"tos.claim.test.form","claim_version":1,
            "qualifiers":{"negation":false},"schema_version":"tos_historical_claim_v1"});
        let reference = json!({"id":"tos.claim.test.form","version":1,
            "digest":sha_ref(&canonical(&record, 1024 * 1024).unwrap())});
        let item = json!({"attributes":{"source_claim":record,
            "human_forms":[{"schema_version":"tos_human_form_materialization_v1",
                "state":"ready","form":{"id":"tos.form.test","version":1,
                    "digest":format!("sha256:{}", "0".repeat(64))},"subject":reference,
                "context":[{"binding":{"record":reference,"pointer":"/qualifiers"},
                    "value":{"negation":false}}],"subject_assessment":{"status":"pending"},
                "display_text":"source wording"}]},"semantics":{}});
        let raw = serde_json::to_vec(&item).unwrap();
        let ReadableContextCarrier::Sidecar(packet) = compiler.compile(&raw, Some(&raw)).unwrap()
        else {
            panic!("ready form requires context sidecar");
        };
        assert_eq!(Digest256::of_bytes(&packet).to_hex(), ORACLE_READY_FORM);
        let mut wrong: Value = serde_json::from_slice(&raw).unwrap();
        wrong["attributes"]["human_forms"][0]["context"][0]["value"]["negation"] = json!(true);
        let wrong_raw = serde_json::to_vec(&wrong).unwrap();
        let ReadableContextCarrier::Sidecar(refusal) =
            compiler.compile(&wrong_raw, Some(&wrong_raw)).unwrap()
        else {
            panic!("pointer mismatch must refuse");
        };
        let refusal: Value = serde_json::from_slice(&refusal).unwrap();
        assert_eq!(refusal["state"], "unavailable");
        assert_eq!(refusal["contexts"], json!([]));
    }

    #[test]
    fn exact_numeric_material_uses_python_canonical_spelling() {
        let compiler = ReadableContextCompiler::from_selected_registry_bytes(
            ENTITY,
            &Digest256::of_bytes(ENTITY).to_hex(),
            limits(),
        )
        .unwrap()
        .unwrap();
        let ordered = br#"{"attributes":{"source_claim":{"claim_id":"tos.claim.test.numeric","claim_version":1,"schema_version":"tos_historical_claim_v1","unknown_numeric_extension":[1,1.0,9007199254740993,-0.0,1e-7,1e21,false]}},"semantics":{}}"#;
        let value: Value = serde_json::from_slice(ordered).unwrap();
        let normalized = serde_json::to_vec(&value).unwrap();
        let ReadableContextCarrier::Sidecar(packet) =
            compiler.compile(&normalized, Some(ordered)).unwrap()
        else {
            panic!("numeric source context requires sidecar");
        };
        assert_eq!(Digest256::of_bytes(&packet).to_hex(), ORACLE_NUMERIC);
        let sidecar: Value = serde_json::from_slice(&packet).unwrap();
        assert!(
            sidecar["exact_materials"][0]["canonical_json"]
                .as_str()
                .unwrap()
                .contains("[1,1.0,9007199254740993,-0.0,1e-07,1e+21,false]")
        );
    }
}
