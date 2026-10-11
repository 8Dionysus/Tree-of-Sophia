//! Explicit source-text projection for the selected-text search profile.
//!
//! This reads only exact source-authored fields from the versions listed
//! below. It is not a schema validator, publication decision, or replacement
//! for the complete knowledge graph. IDs remain separate from indexed text;
//! `subject` is only the source-path dependency locator.
//!
//! Both profiles select corpus/historical record `preferred_label` as title,
//! `variant_labels[].value` as aliases, and their declared `notes` (plus
//! corpus `edition_statement`) as body. Historical claims contribute only
//! `supporting_quotes[].exact`. V1 human forms contribute only `freeform.text`;
//! current forms only are considered; name-role text is a title and other
//! roles are body. Selected body fragments are joined in source order with one
//! newline. Open qualifiers, refs, IDs, prior forms, source-copy bindings,
//! templates, and metadata are never flattened.
//! V2 also materializes current Agent source-copy forms from an explicitly
//! supplied adjacent metadata source, preserving exact subject bindings and
//! recording both source paths as dependencies.

use serde_json::Value;
use tos_foundation::{
    FoundationErrorCode, JsonLimits, JsonMode, JsonValue, parse_json_with_state_budget,
};

/// Version of the field selection implemented by [`project_source_text`].
pub const SOURCE_SEARCH_PROFILE: &str = "tos_source_selected_text_v1";

/// Version of the selected-text profile that additionally resolves current
/// Agent source-copy forms from their exact adjacent `agent.json` input.
pub const SOURCE_SEARCH_PROFILE_V2: &str = "tos_source_selected_text_v2";

const CORPUS_RECORD_TYPES: &[&str] = &[
    "agent",
    "place",
    "organization",
    "work",
    "expression",
    "edition",
    "collection",
    "item",
];
const HISTORICAL_RECORD_TYPES: &[&str] =
    &["historical-event", "historical-process", "historical-state"];
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_JSON_DEPTH: usize = 64;
const MAX_JSON_VISITS: usize = 300_000;
const MAX_JSON_STATE_BYTES: usize = 64 * 1024 * 1024;
const MAX_JSONL_LINES: usize = 16_384;
const MAX_PROJECTED_DOCUMENTS: usize = 256;
const MAX_PROJECTED_TEXT_BYTES: usize = 1024 * 1024;
const MAX_PROJECTED_ALIASES: usize = 64;
const MAX_SUBJECT_PATH_BYTES: usize = 4096;
const AGENT_SOURCE_PATH_PREFIX: &str = "ToS/source-witnesses/agents/";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSearchDocument {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub body: String,
    pub dependencies: Vec<String>,
}

/// Projects exact text fields from one authored source JSON value.
///
/// Supported records are `tos_corpus_record_v1`,
/// `tos_historical_record_v1`, `tos_historical_claim_v1`,
/// `tos_human_form_set_v1`, and standalone `tos_human_form_v1`. The corpus v2
/// wrapper and source-text-unit packet are recognized but currently have no
/// declared text selection. A source-text-unit packet points to a separately
/// owned text layer and contains no literal unit text to project.
///
/// Well-formed JSON without a supported wrapper returns no documents. JSONL
/// files are read one object per line and retain the file path dependency for
/// each document. Only `.json` and `.jsonl` source paths are in profile; a
/// malformed line in a JSONL source refuses the whole projection.
pub fn project_source_text(
    subject: &str,
    raw: &[u8],
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    if raw.len() > MAX_SOURCE_BYTES {
        return Err("source exceeds byte limit");
    }
    if subject.trim().is_empty() {
        return Err("subject path is empty");
    }
    if subject.len() > MAX_SUBJECT_PATH_BYTES {
        return Err("subject path exceeds byte limit");
    }
    if subject.ends_with(".jsonl") {
        return project_jsonl(subject, raw);
    }
    if !subject.ends_with(".json") {
        return Ok(Vec::new());
    }

    match preflight_json(raw, MAX_JSON_VISITS) {
        Ok(_) => {}
        Err(PreflightError::Budget) => return Err("source JSON exceeds parse budget"),
        Err(PreflightError::Invalid) if contains_recognized_wrapper(raw) => {
            return Err("malformed recognized source record");
        }
        Err(PreflightError::Invalid) => return Ok(Vec::new()),
    }

    match serde_json::from_slice::<Value>(raw) {
        Ok(value) => project_value(subject, &value),
        // The bounded strict parser above accepted the exact bytes. A serde
        // disagreement is an implementation error; fail closed instead of
        // silently treating the source as unrelated.
        Err(_) => Err("source JSON parser disagreement"),
    }
}

/// Return the exact adjacent Agent HumanForms path for a canonical Agent
/// metadata source path.
pub fn agent_human_forms_path_for_metadata(record_path: &str) -> Option<String> {
    let slug = agent_slug(record_path, "/agent.json")?;
    let path = format!("{AGENT_SOURCE_PATH_PREFIX}{slug}/agent.human-forms.json");
    (path.len() <= MAX_SUBJECT_PATH_BYTES).then_some(path)
}

/// Return the exact adjacent Agent metadata path for a canonical HumanForms
/// source path.
pub fn agent_metadata_path_for_human_forms(forms_path: &str) -> Option<String> {
    let slug = agent_slug(forms_path, "/agent.human-forms.json")?;
    let path = format!("{AGENT_SOURCE_PATH_PREFIX}{slug}/agent.json");
    (path.len() <= MAX_SUBJECT_PATH_BYTES).then_some(path)
}

fn agent_slug<'a>(path: &'a str, suffix: &str) -> Option<&'a str> {
    if path.len() > MAX_SUBJECT_PATH_BYTES {
        return None;
    }
    let slug = path
        .strip_prefix(AGENT_SOURCE_PATH_PREFIX)?
        .strip_suffix(suffix)?;
    if slug.is_empty()
        || slug == "."
        || slug == ".."
        || slug.contains('/')
        || slug.contains('\\')
        || slug.contains('\0')
    {
        return None;
    }
    Some(slug)
}

/// Projects an exact source file under the v2 selected-text profile.
///
/// V2 preserves v1 behavior for all source paths. For an Agent HumanForms
/// file, an optional exact adjacent metadata input enables current
/// source-copy forms through the maintained source-copy kernel. No filesystem
/// reads or inferred references are performed; a supplied sibling must use
/// the path returned by [`agent_metadata_path_for_human_forms`]. Materialized
/// form documents depend on both the HumanForms root and Agent metadata, in
/// that order. Non-ready materializations (including stale bindings) are not
/// indexed.
pub fn project_source_text_v2(
    root_path: &str,
    exact_root_bytes: &[u8],
    exact_sibling_metadata: Option<(&str, &[u8])>,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let mut documents = project_source_text(root_path, exact_root_bytes)?;
    let Some(expected_metadata_path) = agent_metadata_path_for_human_forms(root_path) else {
        return if exact_sibling_metadata.is_some() {
            Err("sibling metadata input is unsupported for source path")
        } else {
            Ok(documents)
        };
    };
    let Some((metadata_path, metadata_bytes)) = exact_sibling_metadata else {
        return Ok(documents);
    };
    if metadata_path != expected_metadata_path.as_str() {
        return Err("sibling metadata path does not match HumanForms source");
    }
    if metadata_path.len() > MAX_SUBJECT_PATH_BYTES {
        return Err("sibling metadata path exceeds byte limit");
    }

    let forms = parse_foundation_json(exact_root_bytes)?;
    if forms
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        != Some("tos_human_form_set_v1")
    {
        return Ok(documents);
    }
    let metadata = parse_foundation_json(metadata_bytes)?;
    if metadata
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        != Some("tos_corpus_record_v1")
        || metadata
            .object_get("record_type")
            .and_then(JsonValue::as_str)
            != Some("agent")
    {
        return Ok(documents);
    }

    let materializations =
        tos_validation::source_forms::source_copy_kernel::materialize_source_forms(
            &metadata, &forms,
        )
        .map_err(|_| "source-copy materialization failed")?;
    let mut additions = Vec::new();
    additions
        .try_reserve_exact(materializations.len())
        .map_err(|_| "source-copy document allocation refused")?;
    for view in materializations {
        if view.object_get("state").and_then(JsonValue::as_str) != Some("ready")
            || view.object_get("derivation").and_then(JsonValue::as_str) != Some("source-copy")
        {
            continue;
        }
        let form = view
            .object_get("form")
            .ok_or("source-copy materialization omitted form reference")?;
        let id = form
            .object_get("id")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.trim().is_empty())
            .ok_or("source-copy materialization omitted form ID")?;
        let role = view
            .object_get("role")
            .and_then(JsonValue::as_str)
            .filter(|role| !role.trim().is_empty())
            .ok_or("source-copy materialization omitted role")?;
        let display_text = view
            .object_get("display_text")
            .and_then(JsonValue::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or("source-copy materialization omitted display text")?;
        let text = bounded_text(display_text, "form text")?;
        let (title, body) = if role == "name" {
            (text, String::new())
        } else {
            (String::new(), text)
        };
        let mut document = document(
            root_path,
            id,
            format!("human_form/{role}"),
            title,
            Vec::new(),
            body,
        )?;
        append_dependency(&mut document, metadata_path)?;
        additions.push(document);
    }
    if documents.len().saturating_add(additions.len()) > MAX_PROJECTED_DOCUMENTS {
        return Err("source exceeds document limit");
    }
    documents
        .try_reserve(additions.len())
        .map_err(|_| "source document allocation refused")?;
    documents.extend(additions);
    Ok(documents)
}

fn parse_foundation_json(raw: &[u8]) -> Result<JsonValue, &'static str> {
    if raw.len() > MAX_SOURCE_BYTES {
        return Err("source exceeds byte limit");
    }
    let limits = JsonLimits {
        max_bytes: MAX_SOURCE_BYTES,
        max_depth: MAX_JSON_DEPTH,
        max_visits: MAX_JSON_VISITS,
        max_integer_digits: 4_300,
    };
    match parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, MAX_JSON_STATE_BYTES)
    {
        Ok(document) => Ok(document.into_root()),
        Err(error) if error.code == FoundationErrorCode::BudgetExceeded => {
            Err("source JSON exceeds parse budget")
        }
        Err(_) => Err("malformed paired source JSON"),
    }
}

fn append_dependency(
    document: &mut SourceSearchDocument,
    dependency: &str,
) -> Result<(), &'static str> {
    if dependency.trim().is_empty() || dependency.len() > MAX_SUBJECT_PATH_BYTES {
        return Err("dependency path exceeds byte limit");
    }
    let mut retained_text_bytes = document.id.len();
    for length in [
        document.kind.len(),
        document.title.len(),
        document.body.len(),
        dependency.len(),
    ]
    .into_iter()
    .chain(document.aliases.iter().map(String::len))
    .chain(document.dependencies.iter().map(String::len))
    {
        retained_text_bytes = retained_text_bytes
            .checked_add(length)
            .ok_or("projected document exceeds byte limit")?;
    }
    if retained_text_bytes > MAX_PROJECTED_TEXT_BYTES {
        return Err("projected document exceeds byte limit");
    }
    document
        .dependencies
        .try_reserve_exact(1)
        .map_err(|_| "dependency allocation refused")?;
    document.dependencies.push(dependency.to_owned());
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreflightError {
    Budget,
    Invalid,
}

/// Applies the maintained strict JSON grammar and its visit/depth/state
/// budgets to the exact bytes before the serde projection tree is allocated.
fn preflight_json(raw: &[u8], max_visits: usize) -> Result<usize, PreflightError> {
    let limits = JsonLimits {
        max_bytes: MAX_SOURCE_BYTES,
        max_depth: MAX_JSON_DEPTH,
        max_visits,
        max_integer_digits: 4_300,
    };
    match parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, MAX_JSON_STATE_BYTES)
    {
        Ok(document) => Ok(document.visits()),
        Err(error) if error.code == FoundationErrorCode::BudgetExceeded => {
            Err(PreflightError::Budget)
        }
        Err(_) => Err(PreflightError::Invalid),
    }
}

fn project_jsonl(subject: &str, raw: &[u8]) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let mut documents = Vec::new();
    let mut visits_remaining = MAX_JSON_VISITS;
    let mut lines = 0usize;
    for line in raw
        .split(|byte| *byte == b'\n')
        .filter(|line| line.iter().any(|byte| !byte.is_ascii_whitespace()))
    {
        lines += 1;
        if lines > MAX_JSONL_LINES {
            return Err("source JSONL exceeds line limit");
        }
        let visits = preflight_json(line, visits_remaining).map_err(|error| match error {
            PreflightError::Budget => "source JSONL exceeds parse budget",
            PreflightError::Invalid => "malformed source JSONL line",
        })?;
        visits_remaining = visits_remaining.saturating_sub(visits);
        let value =
            serde_json::from_slice::<Value>(line).map_err(|_| "source JSON parser disagreement")?;
        let next = project_value(subject, &value)?;
        if documents.len().saturating_add(next.len()) > MAX_PROJECTED_DOCUMENTS {
            return Err("source JSONL exceeds document limit");
        }
        documents.extend(next);
    }
    Ok(documents)
}

fn project_value(subject: &str, value: &Value) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let Some(root) = value.as_object() else {
        return Ok(Vec::new());
    };
    let Some(schema_version) = root.get("schema_version").and_then(Value::as_str) else {
        return Ok(Vec::new());
    };

    match schema_version {
        "tos_corpus_record_v1" => project_corpus_record(subject, value),
        "tos_historical_record_v1" => project_historical_record(subject, value),
        "tos_historical_claim_v1" => project_historical_claim(subject, value),
        "tos_human_form_set_v1" => project_human_form_set(subject, value),
        "tos_human_form_v1" => {
            project_human_form(subject, value).map(|document| document.into_iter().collect())
        }
        // There is no maintained tos_corpus_record_v2 contract in this
        // checkout. Do not guess that its fields match v1.
        "tos_corpus_record_v2" | "tos_source_text_unit_packet_v1" => Ok(Vec::new()),
        _ => Ok(Vec::new()),
    }
}

// For a malformed `.json` file, recognized markers distinguish a malformed
// record from an unrelated non-JSON source that this profile ignores.
fn contains_recognized_wrapper(raw: &[u8]) -> bool {
    [
        "tos_corpus_record_v1",
        "tos_corpus_record_v2",
        "tos_historical_record_v1",
        "tos_historical_claim_v1",
        "tos_human_form_set_v1",
        "tos_human_form_v1",
        "tos_source_text_unit_packet_v1",
    ]
    .iter()
    .any(|marker| {
        raw.windows(marker.len())
            .any(|window| window == marker.as_bytes())
    })
}

fn required_text<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, &'static str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or(field)
}

fn optional_text<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<Option<&'a str>, &'static str> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok((!text.trim().is_empty()).then_some(text.as_str())),
        Some(_) => Err(field),
    }
}

fn selected_body(
    parts: impl IntoIterator<Item = Result<String, &'static str>>,
) -> Result<String, &'static str> {
    // Source input is byte-capped before parsing. The worker also imposes a
    // per-document text ceiling; enforce it while joining rather than first
    // retaining a temporary vector of copied fragments.
    let mut body = String::new();
    for part in parts {
        let part = part?;
        if part.trim().is_empty() {
            continue;
        }
        let separator: usize = if body.is_empty() { 0 } else { 1 };
        let additional = separator.saturating_add(part.len());
        if body.len().saturating_add(additional) > MAX_PROJECTED_TEXT_BYTES {
            return Err("projected body exceeds byte limit");
        }
        body.try_reserve(additional)
            .map_err(|_| "projected body allocation refused")?;
        if separator != 0 {
            body.push('\n');
        }
        body.push_str(&part);
    }
    Ok(body)
}

fn document(
    subject: &str,
    id: &str,
    kind: String,
    title: String,
    aliases: Vec<String>,
    body: String,
) -> Result<SourceSearchDocument, &'static str> {
    if subject.trim().is_empty() {
        return Err("subject path is empty");
    }
    if id.len() > 256 {
        return Err("document ID exceeds byte limit");
    }
    if kind.len() > MAX_PROJECTED_TEXT_BYTES
        || title.len() > MAX_PROJECTED_TEXT_BYTES
        || body.len() > MAX_PROJECTED_TEXT_BYTES
        || aliases.len() > MAX_PROJECTED_ALIASES
    {
        return Err("projected document exceeds field limit");
    }
    let retained_text_bytes = aliases
        .iter()
        .try_fold(
            id.len() + kind.len() + title.len() + body.len() + subject.len(),
            |sum, alias| {
                if alias.len() > MAX_PROJECTED_TEXT_BYTES {
                    return None;
                }
                sum.checked_add(alias.len())
            },
        )
        .ok_or("projected document exceeds byte limit")?;
    if retained_text_bytes > MAX_PROJECTED_TEXT_BYTES {
        return Err("projected document exceeds byte limit");
    }
    Ok(SourceSearchDocument {
        id: id.to_owned(),
        kind,
        title,
        aliases,
        body,
        dependencies: vec![subject.to_owned()],
    })
}

fn corpus_kind(
    value: &Value,
    field: &'static str,
    family: &'static str,
) -> Result<String, &'static str> {
    let value = required_text(value, field)?;
    let allowed = if family == "corpus_record" {
        CORPUS_RECORD_TYPES
    } else {
        HISTORICAL_RECORD_TYPES
    };
    if !allowed.contains(&value) {
        return Err(field);
    }
    Ok(format!("{family}/{value}"))
}

fn aliases(value: &Value) -> Result<Vec<String>, &'static str> {
    let Some(labels) = value.get("variant_labels") else {
        return Ok(Vec::new());
    };
    let labels = labels.as_array().ok_or("variant_labels")?;
    if labels.len() > MAX_PROJECTED_ALIASES {
        return Err("variant_labels exceeds count limit");
    }
    let mut projected = Vec::new();
    projected
        .try_reserve_exact(labels.len())
        .map_err(|_| "variant label allocation refused")?;
    let mut bytes = 0usize;
    for label in labels {
        let text = required_text(label, "value")?;
        bytes = bytes
            .checked_add(text.len())
            .ok_or("variant labels exceed byte limit")?;
        if bytes > MAX_PROJECTED_TEXT_BYTES {
            return Err("variant labels exceed byte limit");
        }
        projected.push(bounded_text(text, "variant label")?);
    }
    Ok(projected)
}

fn bounded_text(text: &str, field: &'static str) -> Result<String, &'static str> {
    if text.len() > MAX_PROJECTED_TEXT_BYTES {
        return Err(field);
    }
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| "projected text allocation refused")?;
    result.push_str(text);
    Ok(result)
}

fn project_corpus_record(
    subject: &str,
    value: &Value,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let id = required_text(value, "record_id")?;
    let kind = corpus_kind(value, "record_type", "corpus_record")?;
    let title = bounded_text(required_text(value, "preferred_label")?, "preferred_label")?;
    let aliases = aliases(value)?;
    let mut body_parts = Vec::new();
    if let Some(notes) = optional_text(value, "notes")? {
        body_parts.push(bounded_text(notes, "notes"));
    }
    if let Some(edition_statement) = optional_text(value, "edition_statement")? {
        body_parts.push(bounded_text(edition_statement, "edition_statement"));
    }
    Ok(vec![document(
        subject,
        id,
        kind,
        title,
        aliases,
        selected_body(body_parts)?,
    )?])
}

fn project_historical_record(
    subject: &str,
    value: &Value,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let id = required_text(value, "record_id")?;
    let kind = corpus_kind(value, "record_type", "historical_record")?;
    let title = bounded_text(required_text(value, "preferred_label")?, "preferred_label")?;
    let aliases = aliases(value)?;
    let body = selected_body(
        optional_text(value, "notes")?
            .map(|text| bounded_text(text, "notes"))
            .into_iter(),
    )?;
    Ok(vec![document(subject, id, kind, title, aliases, body)?])
}

fn project_historical_claim(
    subject: &str,
    value: &Value,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let id = required_text(value, "claim_id")?;
    let quotes = match value.get("supporting_quotes") {
        None => &[][..],
        Some(Value::Array(quotes)) => quotes.as_slice(),
        Some(_) => return Err("supporting_quotes"),
    };
    let body =
        selected_body(quotes.iter().map(|quote| {
            required_text(quote, "exact").and_then(|text| bounded_text(text, "exact"))
        }))?;
    Ok(vec![document(
        subject,
        id,
        "historical_claim".to_owned(),
        String::new(),
        Vec::new(),
        body,
    )?])
}

fn project_human_form_set(
    subject: &str,
    value: &Value,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let forms = value
        .get("forms")
        .and_then(Value::as_array)
        .filter(|forms| !forms.is_empty())
        .ok_or("forms")?;
    if forms.len() > MAX_PROJECTED_DOCUMENTS {
        return Err("forms exceeds document limit");
    }
    forms
        .iter()
        .map(|form| project_human_form(subject, form))
        .collect::<Result<Vec<_>, _>>()
        .map(|documents| documents.into_iter().flatten().collect())
}

fn project_human_form(
    subject: &str,
    value: &Value,
) -> Result<Option<SourceSearchDocument>, &'static str> {
    if required_text(value, "schema_version")? != "tos_human_form_v1" {
        return Err("schema_version");
    }
    let id = required_text(value, "form_id")?;
    let role = required_text(value, "role")?;
    if ![
        "name",
        "caption",
        "hover",
        "statement",
        "grounds",
        "history",
        "technical",
    ]
    .contains(&role)
    {
        return Err("role");
    }
    let content = value.get("content").ok_or("content")?;
    let content_kind = required_text(content, "kind")?;
    let text = match content_kind {
        "freeform" => Some(required_text(content, "text")?),
        "source-copy" => {
            required_text(content, "slot")?;
            None
        }
        "template" => {
            if !content.get("template").is_some_and(Value::is_object) {
                return Err("template");
            }
            None
        }
        _ => return Err("content.kind"),
    };
    let Some(text) = text else {
        return Ok(None);
    };
    let (title, body) = if role == "name" {
        (bounded_text(text, "form text")?, String::new())
    } else {
        (String::new(), bounded_text(text, "form text")?)
    };
    document(
        subject,
        id,
        format!("human_form/{role}"),
        title,
        Vec::new(),
        body,
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NIETZSCHE_AGENT_PATH: &str = "ToS/source-witnesses/agents/friedrich-nietzsche/agent.json";
    const NIETZSCHE_FORMS_PATH: &str =
        "ToS/source-witnesses/agents/friedrich-nietzsche/agent.human-forms.json";
    const NIETZSCHE_AGENT_BYTES: &[u8] =
        include_bytes!("../../../../ToS/source-witnesses/agents/friedrich-nietzsche/agent.json");
    const NIETZSCHE_FORMS_BYTES: &[u8] = include_bytes!(
        "../../../../ToS/source-witnesses/agents/friedrich-nietzsche/agent.human-forms.json"
    );
    const HISTORY_CLAIMS_PATH: &str = concat!(
        "ToS/source-witnesses/history/",
        "friedrich-nietzsche/jenseits-1886-commission/historical-claims.jsonl"
    );

    #[test]
    fn v2_maps_only_the_exact_adjacent_agent_source_paths() {
        assert_eq!(
            agent_human_forms_path_for_metadata(NIETZSCHE_AGENT_PATH).as_deref(),
            Some(NIETZSCHE_FORMS_PATH)
        );
        assert_eq!(
            agent_metadata_path_for_human_forms(NIETZSCHE_FORMS_PATH).as_deref(),
            Some(NIETZSCHE_AGENT_PATH)
        );
        assert!(
            agent_human_forms_path_for_metadata("ToS/source-witnesses/works/foo/work.json")
                .is_none()
        );
        assert!(
            agent_metadata_path_for_human_forms(
                "ToS/source-witnesses/agents/a/b/agent.human-forms.json"
            )
            .is_none()
        );
    }

    #[test]
    fn v2_materializes_only_current_ready_agent_source_copy_forms() {
        let documents = project_source_text_v2(
            NIETZSCHE_FORMS_PATH,
            NIETZSCHE_FORMS_BYTES,
            Some((NIETZSCHE_AGENT_PATH, NIETZSCHE_AGENT_BYTES)),
        )
        .unwrap();

        assert_eq!(documents.len(), 3);
        let name = documents
            .iter()
            .find(|document| {
                document
                    .id
                    .ends_with("ee414f73c4461ed7de9865bd361798a4fc4ad33b4c9014cd42f69b8868cf0eb3")
            })
            .unwrap();
        assert_eq!(name.kind, "human_form/name");
        assert_eq!(name.title, "Friedrich Nietzsche");
        assert_eq!(name.body, "");
        let hover = documents
            .iter()
            .find(|document| document.kind == "human_form/hover")
            .unwrap();
        assert_eq!(
            hover.body,
            "Author identity is established for the current route. The DNB/GND authority identifier is reconciled through the exact Ecce Homo work and 1908-edition discovery record."
        );
        assert!(documents.iter().all(|document| {
            document.dependencies == [NIETZSCHE_FORMS_PATH, NIETZSCHE_AGENT_PATH]
        }));
    }

    #[test]
    fn v2_withholds_source_copy_when_exact_agent_input_makes_pair_stale() {
        let changed_agent = std::str::from_utf8(NIETZSCHE_AGENT_BYTES)
            .unwrap()
            .replacen("Friedrich Nietzsche", "Changed Nietzsche", 1);
        let documents = project_source_text_v2(
            NIETZSCHE_FORMS_PATH,
            NIETZSCHE_FORMS_BYTES,
            Some((NIETZSCHE_AGENT_PATH, changed_agent.as_bytes())),
        )
        .unwrap();
        assert!(documents.is_empty());
    }

    #[test]
    fn v2_rejects_non_adjacent_sibling_bytes() {
        assert_eq!(
            project_source_text_v2(
                NIETZSCHE_FORMS_PATH,
                NIETZSCHE_FORMS_BYTES,
                Some((
                    "ToS/source-witnesses/agents/other/agent.json",
                    NIETZSCHE_AGENT_BYTES
                )),
            ),
            Err("sibling metadata path does not match HumanForms source")
        );
    }

    #[test]
    fn corpus_projection_keeps_selected_text_and_locator_separate() {
        let raw = json!({
            "schema_version": "tos_corpus_record_v1",
            "record_id": "tos.agent.socrates",
            "record_type": "agent",
            "preferred_label": "Socrates",
            "identity_status": "verified",
            "source_refs": ["private/source.json"],
            "variant_labels": [{
                "value": "Sokrates",
                "language": "de",
                "source_ref": "private/source.json",
                "status": "reported"
            }],
            "notes": "A philosopher.",
            "edition_statement": null,
            "external_identifiers": [{
                "scheme": "x",
                "value": "1",
                "source_ref": "private/source.json",
                "status": "reported"
            }],
            "same_as_posture": "no_equivalence_claim",
            "record_version": 1
        });
        let docs = project_source_text(
            "ToS/source-witnesses/agents/socrates/agent.json",
            raw.to_string().as_bytes(),
        )
        .unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, "tos.agent.socrates");
        assert_eq!(docs[0].kind, "corpus_record/agent");
        assert_eq!(docs[0].title, "Socrates");
        assert_eq!(docs[0].aliases, ["Sokrates"]);
        assert_eq!(docs[0].body, "A philosopher.");
        assert_eq!(
            docs[0].dependencies,
            ["ToS/source-witnesses/agents/socrates/agent.json"]
        );
        assert!(!docs[0].body.contains("tos.agent.socrates"));
        assert!(!docs[0].body.contains("private/source.json"));
    }

    #[test]
    fn exact_source_path_with_hash_is_not_treated_as_a_reference() {
        let raw = json!({
            "schema_version": "tos_corpus_record_v1",
            "record_id": "tos.agent.socrates",
            "record_type": "agent",
            "preferred_label": "Socrates",
            "variant_labels": [],
            "notes": "A philosopher."
        });
        let source_path = "ToS/source-witnesses/agents/socrates#record.json";
        let docs = project_source_text(source_path, raw.to_string().as_bytes()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].dependencies, [source_path]);
    }

    #[test]
    fn source_byte_limit_is_checked_before_extension_filtering() {
        let raw = vec![b'x'; MAX_SOURCE_BYTES + 1];
        assert_eq!(
            project_source_text("unindexed.rs", &raw),
            Err("source exceeds byte limit")
        );
    }

    #[test]
    fn nested_unrecognized_json_still_has_a_parse_budget() {
        let depth = MAX_JSON_DEPTH + 1;
        let raw = format!("{}null{}", "[".repeat(depth), "]".repeat(depth));
        assert_eq!(
            project_source_text("ToS/unrelated.json", raw.as_bytes()),
            Err("source JSON exceeds parse budget")
        );
    }

    #[test]
    fn historical_claim_projects_only_exact_supporting_quotes() {
        let raw = json!({
            "schema_version": "tos_historical_claim_v1",
            "claim_id": "tos.claim.example",
            "supporting_quotes": [{
                "anchor_ref": "tos.anchor.example",
                "exact": "The exact words."
            }],
            "qualifiers": {"statement": "Not part of the selected contract."}
        });
        let source_path = HISTORY_CLAIMS_PATH;
        let docs = project_source_text(source_path, raw.to_string().as_bytes()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, "tos.claim.example");
        assert_eq!(docs[0].title, "");
        assert_eq!(docs[0].body, "The exact words.");
        assert!(!docs[0].body.contains("Not part of the selected contract."));
        assert_eq!(docs[0].dependencies, [source_path]);
    }

    #[test]
    fn historical_claim_jsonl_projects_each_record_and_keeps_file_dependency() {
        let first = json!({
            "schema_version": "tos_historical_claim_v1",
            "claim_id": "tos.claim.first",
            "supporting_quotes": [{"anchor_ref": "tos.anchor.first", "exact": "First quote."}]
        });
        let second = json!({
            "schema_version": "tos_historical_claim_v1",
            "claim_id": "tos.claim.second",
            "supporting_quotes": []
        });
        let raw = format!("{first}\n{second}\n");
        let source_path = HISTORY_CLAIMS_PATH;
        let docs = project_source_text(source_path, raw.as_bytes()).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].id, "tos.claim.first");
        assert_eq!(docs[1].id, "tos.claim.second");
        assert_eq!(docs[1].body, "");
        assert!(docs.iter().all(|doc| doc.dependencies == [source_path]));
    }

    #[test]
    fn human_forms_include_freeform_text_but_never_resolve_external_content() {
        let raw = json!({
            "schema_version": "tos_human_form_set_v1",
            "subject": {
                "id": "tos.agent.example",
                "version": 1,
                "digest": format!("sha256:{}", "0".repeat(64))
            },
            "forms": [
                {
                    "schema_version": "tos_human_form_v1",
                    "form_id": "tos.form.example.name",
                    "form_version": 1,
                    "subject": {
                        "id": "tos.agent.example",
                        "version": 1,
                        "digest": format!("sha256:{}", "0".repeat(64))
                    },
                    "role": "name",
                    "language": "en",
                    "script": null,
                    "creator_id": "tos.agent.creator",
                    "bindings": {},
                    "content": {"kind": "freeform", "text": "Example name"},
                    "revises": null
                },
                {
                    "schema_version": "tos_human_form_v1",
                    "form_id": "tos.form.example.copy",
                    "form_version": 1,
                    "subject": {
                        "id": "tos.agent.example",
                        "version": 1,
                        "digest": format!("sha256:{}", "0".repeat(64))
                    },
                    "role": "statement",
                    "language": "en",
                    "script": null,
                    "creator_id": "tos.agent.creator",
                    "bindings": {},
                    "content": {"kind": "source-copy", "slot": "label"},
                    "revises": null
                }
            ],
            "prior_forms": [{
                "schema_version": "tos_human_form_v1",
                "form_id": "tos.form.example.name",
                "form_version": 1,
                "subject": {
                    "id": "tos.agent.example",
                    "version": 1,
                    "digest": format!("sha256:{}", "0".repeat(64))
                },
                "role": "name",
                "language": "en",
                "script": null,
                "creator_id": "tos.agent.creator",
                "bindings": {},
                "content": {"kind": "freeform", "text": "Retained predecessor"},
                "revises": null
            }]
        });
        let docs =
            project_source_text("example.human-forms.json", raw.to_string().as_bytes()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, "tos.form.example.name");
        assert_eq!(docs[0].title, "Example name");
        assert_eq!(docs[0].body, "");
        assert_eq!(docs[0].kind, "human_form/name");
    }

    #[test]
    fn unsupported_wrappers_are_empty_and_malformed_supported_records_error() {
        assert!(
            project_source_text(
                "record.json",
                br#"{"schema_version":"tos_corpus_record_v2"}"#
            )
            .unwrap()
            .is_empty()
        );
        assert!(
            project_source_text(
                "src/worker.rs",
                br#"const PROFILE: &str = "tos_corpus_record_v1";"#
            )
            .unwrap()
            .is_empty()
        );
        assert!(
            project_source_text(
                "packet.json",
                br#"{"schema_version":"tos_source_text_unit_packet_v1","units":[]}"#
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            project_source_text(
                "broken.json",
                br#"{"schema_version":"tos_corpus_record_v1","record_id":}"#
            ),
            Err("malformed recognized source record")
        );
        assert_eq!(
            project_source_text(
                "broken.jsonl",
                concat!(
                    "{unknown malformed line",
                    "\n",
                    r#"{"schema_version":"tos_historical_claim_v1","claim_id":"tos.claim.ok","#,
                    r#"supporting_quotes":[]}"#
                )
                .as_bytes()
            ),
            Err("malformed source JSONL line")
        );
        assert_eq!(
            project_source_text(
                "broken.json",
                br#"{"schema_version":"tos_historical_record_v1"}"#
            ),
            Err("record_id")
        );
        assert!(
            project_source_text("unrelated.json", b"not-json")
                .unwrap()
                .is_empty()
        );
    }
}
