//! The bounded WebMCP *page* response for knowledge search. It handles a
//! browser command result, not the backend search plan or its opaque cursor.

use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_AGENT_ITEMS_PER_KIND: usize = 6;
const MAX_CURSOR_UNITS: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeEnvelopeErrorCode {
    InvalidInput,
    InvalidModeSchema,
    InvalidPage,
    PageExceedsLimit,
    InvalidItem,
    InvalidRevision,
    AuthorityBoundary,
    OutputBudget,
}

impl KnowledgeEnvelopeErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidModeSchema => "invalid_mode_schema",
            Self::InvalidPage => "invalid_page",
            Self::PageExceedsLimit => "page_exceeds_limit",
            Self::InvalidItem => "invalid_item",
            Self::InvalidRevision => "invalid_revision",
            Self::AuthorityBoundary => "authority_boundary",
            Self::OutputBudget => "output_budget",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnowledgeEnvelopeError { pub code: KnowledgeEnvelopeErrorCode }

impl KnowledgeEnvelopeError {
    const fn new(code: KnowledgeEnvelopeErrorCode) -> Self { Self { code } }
}

fn field<'a>(object: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    object.object_get(name)
}

fn string<'a>(object: &'a JsonValue, name: &str) -> Option<&'a str> {
    field(object, name)?.as_str()
}

fn member(name: &str, value: JsonValue) -> (JsonString, JsonValue) {
    (JsonString::from_utf8(name), value)
}

fn text(value: &str) -> JsonValue { JsonValue::String(JsonString::from_utf8(value)) }

fn object(entries: Vec<(JsonString, JsonValue)>) -> JsonValue { JsonValue::Object(entries) }

fn integer(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber { kind: JsonNumberKind::Int, lexeme: value.to_string() })
}

fn identity(object: &JsonValue, name: &str) -> JsonValue {
    match field(object, name) { Some(JsonValue::String(value)) => JsonValue::String(value.clone()), _ => text("") }
}

fn first_nonempty_string<'a>(object: &'a JsonValue, names: &[&str]) -> Option<&'a JsonValue> {
    names.iter().filter_map(|name| field(object, name))
        .find(|value| matches!(value, JsonValue::String(value) if !value.units().is_empty()))
}

/// Match JS `slice(0, maximum-1) + "…"` in UTF-16 code units. The FND JSON
/// reader retains a possible split surrogate as WTF-16; the writer escapes it.
fn clipped(value: Option<&JsonValue>, maximum: usize) -> Result<JsonValue, KnowledgeEnvelopeError> {
    let Some(JsonValue::String(value)) = value else { return Ok(text("")) };
    if value.units().len() <= maximum { return Ok(JsonValue::String(value.clone())) }
    let mut escaped = String::with_capacity(maximum * 6 + 8);
    escaped.push('"');
    for unit in &value.units()[..maximum - 1] {
        use std::fmt::Write;
        write!(&mut escaped, "\\u{unit:04x}").expect("writing to String cannot fail");
    }
    escaped.push_str("\\u2026\"");
    parse_json(escaped.as_bytes(), JsonMode::RequestLastWins, JsonLimits::default())
        .map(|document| document.into_root())
        .map_err(|_| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::OutputBudget))
}

fn compact_item(item: &JsonValue) -> Result<JsonValue, KnowledgeEnvelopeError> {
    if item.as_object().is_none() { return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidItem)) }
    let id = identity(item, "id");
    if !matches!(&id, JsonValue::String(value) if !value.units().is_empty()) {
        return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidItem));
    }
    let mut result = vec![
        member("id", id),
        member("kind", clipped(first_nonempty_string(item, &["semantic_kind", "kind", "knowledge_search_kind"]), 48)?),
        member("label", clipped(field(item, "label"), 120)?),
        member("subtitle", clipped(first_nonempty_string(item, &["subtitle", "node_type", "predicate_id"]), 80)?),
    ];
    for name in ["from_id", "to_id"] {
        if let Some(JsonValue::String(value)) = field(item, name) {
            if !value.units().is_empty() { result.push(member(name, JsonValue::String(value.clone()))); }
        }
    }
    let refs = field(item, "source_refs").and_then(JsonValue::as_array)
        .map_or_else(Vec::new, |items| items.iter().take(3)
            .map(|item| match item { JsonValue::String(value) => JsonValue::String(value.clone()), _ => text("") })
            .collect());
    result.push(member("source_refs", JsonValue::Array(refs)));
    Ok(object(result))
}

fn items<'a>(value: &'a JsonValue, name: &str, limit: usize) -> Result<Vec<JsonValue>, KnowledgeEnvelopeError> {
    let values = field(value, name).and_then(JsonValue::as_array)
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage))?;
    if values.len() > limit { return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::PageExceedsLimit)) }
    values.iter().map(compact_item).collect()
}

/// Input: `{limit, expected_mode?, result}` where `result` is the page-command
/// registry result. The host must bind limit <= 6 before executing the command.
/// Counts, authority boundary and the opaque cursor are copied without
/// semantic interpretation. A page larger than its requested limit fails
/// instead of truncating and losing unreported continuation members.
pub fn compact_knowledge_search_page_v1(raw: &[u8]) -> Result<Vec<u8>, KnowledgeEnvelopeError> {
    let limits = JsonLimits { max_bytes: MAX_INPUT_BYTES, ..JsonLimits::default() };
    let document = parse_json(raw, JsonMode::RequestLastWins, limits)
        .map_err(|_| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput))?;
    let request = document.root();
    let limit = field(request, "limit").and_then(JsonValue::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| (1..=MAX_AGENT_ITEMS_PER_KIND).contains(value))
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput))?;
    let result = field(request, "result")
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput))?;
    if string(result, "schema") != Some("tos_page_command_result_v1")
        || string(result, "command_id") != Some("tos.page.knowledge-search") {
        return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput));
    }
    let value = field(result, "value")
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput))?;
    let mode = string(value, "search_mode")
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidModeSchema))?;
    let schema = match mode {
        "indexed" => "tos_knowledge_search_indexed_v2",
        "compressed" => "tos_knowledge_search_compressed_v3",
        _ => return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidModeSchema)),
    };
    if string(value, "schema") != Some(schema)
        || field(request, "expected_mode").is_some_and(|expected| expected.as_str() != Some(mode)) {
        return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidModeSchema));
    }
    let revision = string(value, "source_revision")
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidRevision))?;
    if revision.is_empty() || revision.encode_utf16().count() > 96 {
        return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidRevision));
    }
    if let Some(boundary) = field(value, "authority_boundary") {
        if boundary.as_object().is_none() || field(boundary, "writes_to_tree").and_then(JsonValue::as_bool) != Some(false) {
            return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::AuthorityBoundary));
        }
    }
    let page = field(value, "page")
        .filter(|page| page.as_object().is_some())
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage))?;
    let has_more = field(page, "has_more").and_then(JsonValue::as_bool)
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage))?;
    let next_cursor = field(page, "next_cursor")
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage))?;
    let next_cursor = match (has_more, next_cursor) {
        (true, JsonValue::String(cursor)) if !cursor.units().is_empty() && cursor.units().len() <= MAX_CURSOR_UNITS => JsonValue::String(cursor.clone()),
        (false, JsonValue::Null) => JsonValue::Null,
        _ => return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage)),
    };
    let nodes = items(value, "nodes", limit)?;
    let relations = items(value, "relations", limit)?;
    let result_count = field(value, "result_count").and_then(JsonValue::as_u64)
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage))?;
    if result_count != (nodes.len() + relations.len()) as u64 {
        return Err(KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage));
    }
    let counts = field(value, "counts")
        .filter(|counts| counts.as_object().is_some())
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidPage))?;
    let context = field(result, "context")
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput))?;
    let deep_link = string(context, "deep_link")
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput))?;
    let context_revision = field(result, "context_revision").and_then(JsonValue::as_u64)
        .ok_or_else(|| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::InvalidInput))?;
    let mut output = vec![
        member("schema", text(schema)),
        member("search_mode", text(mode)),
        member("query", clipped(field(value, "query"), 160)?),
        member("result_count", integer(result_count)),
        member("nodes", JsonValue::Array(nodes)),
        member("relations", JsonValue::Array(relations)),
        member("counts", counts.clone()),
        member("next_cursor", next_cursor),
        member("has_more", JsonValue::Bool(has_more)),
        member("source_revision", text(revision)),
        member("context_revision", integer(context_revision)),
        member("deep_link", text(deep_link)),
        member("next_action", text(if has_more { "invoke this tool again with next_cursor" } else { "select one returned stable id" })),
    ];
    if let Some(boundary) = field(value, "authority_boundary") {
        output.push(member("authority_boundary", boundary.clone()));
    }
    let output_limits = JsonLimits { max_bytes: MAX_OUTPUT_BYTES, ..JsonLimits::default() };
    emit_value_preserved_json(&object(output), output_limits)
        .map_err(|_| KnowledgeEnvelopeError::new(KnowledgeEnvelopeErrorCode::OutputBudget))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKET: &str = r#"{"limit":6,"result":{"schema":"tos_page_command_result_v1","command_id":"tos.page.knowledge-search","context_revision":2,"context":{"deep_link":"https://tos.example/?view=observatory"},"value":{"schema":"tos_knowledge_search_indexed_v2","search_mode":"indexed","query":"fate","result_count":2,"nodes":[{"id":"node:1","kind":"node","label":"First","source_refs":["a","b","c","d"]}],"relations":[{"id":"edge:1","kind":"relation","label":"Second"}],"counts":{"matching_nodes":null,"matching_relations":null},"page":{"has_more":true,"next_cursor":"opaque + /"},"source_revision":"revision:test","authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false}}}}"#;

    #[test]
    fn keeps_page_cursor_counts_revision_authority_and_full_ids() {
        let bytes = compact_knowledge_search_page_v1(PACKET.as_bytes()).unwrap();
        let document = parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        let output = document.root();
        assert_eq!(string(output, "search_mode"), Some("indexed"));
        assert_eq!(string(output, "next_cursor"), Some("opaque + /"));
        assert_eq!(string(output, "source_revision"), Some("revision:test"));
        assert_eq!(field(output, "counts"), field(&parse_json(PACKET.as_bytes(), JsonMode::RequestLastWins, JsonLimits::default()).unwrap().into_root(), "result").and_then(|result| field(result, "value")).and_then(|value| field(value, "counts")));
        assert_eq!(field(output, "authority_boundary").and_then(|value| field(value, "writes_to_tree")).and_then(JsonValue::as_bool), Some(false));
        assert_eq!(field(output, "nodes").and_then(JsonValue::as_array).unwrap().len(), 1);
    }

    #[test]
    fn rejects_mode_schema_mismatch_and_oversize_page() {
        let mismatch = PACKET.replace("tos_knowledge_search_indexed_v2", "tos_knowledge_search_compressed_v3");
        assert_eq!(compact_knowledge_search_page_v1(mismatch.as_bytes()).unwrap_err().code, KnowledgeEnvelopeErrorCode::InvalidModeSchema);
        let overflow = PACKET.replace("\"limit\":6", "\"limit\":1")
            .replace("}],\"relations\"", "},{\"id\":\"node:2\"}],\"relations\"");
        assert_eq!(compact_knowledge_search_page_v1(overflow.as_bytes()).unwrap_err().code, KnowledgeEnvelopeErrorCode::PageExceedsLimit);
    }

    #[test]
    fn rejects_cursor_and_authority_inconsistency() {
        let cursor = PACKET.replace("\"has_more\":true", "\"has_more\":false");
        assert_eq!(compact_knowledge_search_page_v1(cursor.as_bytes()).unwrap_err().code, KnowledgeEnvelopeErrorCode::InvalidPage);
        let authority = PACKET.replace("\"writes_to_tree\":false", "\"writes_to_tree\":true");
        assert_eq!(compact_knowledge_search_page_v1(authority.as_bytes()).unwrap_err().code, KnowledgeEnvelopeErrorCode::AuthorityBoundary);
    }

    #[test]
    fn preserves_maximum_opaque_cursor_and_utf16_clip_boundary() {
        let cursor = "x".repeat(MAX_CURSOR_UNITS);
        let label = format!("{}😀x", "a".repeat(118));
        let input = PACKET.replace("opaque + /", &cursor).replace("First", &label);
        let bytes = compact_knowledge_search_page_v1(input.as_bytes()).unwrap();
        let document = parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
        assert_eq!(string(document.root(), "next_cursor"), Some(cursor.as_str()));
        let encoded = String::from_utf8(bytes).unwrap();
        assert!(encoded.contains("\\ud83d\\u2026"));
        let too_long = PACKET.replace("opaque + /", &"x".repeat(MAX_CURSOR_UNITS + 1));
        assert_eq!(compact_knowledge_search_page_v1(too_long.as_bytes()).unwrap_err().code, KnowledgeEnvelopeErrorCode::InvalidPage);
    }
}
