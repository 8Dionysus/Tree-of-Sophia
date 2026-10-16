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
const MAX_PROJECTED_RETAINED_BYTES: usize = 32 * 1024 * 1024;
const MAX_PROJECTED_ALIASES: usize = 64;
const MAX_SUBJECT_PATH_BYTES: usize = 4096;
const AGENT_SOURCE_PATH_PREFIX: &str = "ToS/source-witnesses/agents/";

/// Explicit parser workspace for an initial data operation. The field model,
/// source byte/depth limits and projected document limits are unchanged. Routine
/// events retain Default; initial migration may select its measured workspace.
#[derive(Clone, Copy, Debug)]
pub struct SourceSearchParseLimits {
    pub max_json_visits: usize,
    pub max_json_state_bytes: usize,
}

impl Default for SourceSearchParseLimits {
    fn default() -> Self {
        Self {
            max_json_visits: MAX_JSON_VISITS,
            max_json_state_bytes: MAX_JSON_STATE_BYTES,
        }
    }
}

impl SourceSearchParseLimits {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.max_json_visits == 0
            || self.max_json_visits > 4_000_000
            || self.max_json_state_bytes == 0
            || self.max_json_state_bytes > 512 * 1024 * 1024
        {
            return Err("invalid selected source parse limits");
        }
        Ok(self)
    }
}

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
    project_source_text_with_parse_limits(subject, raw, SourceSearchParseLimits::default())
}

pub fn project_source_text_with_parse_limits(
    subject: &str,
    raw: &[u8],
    limits: SourceSearchParseLimits,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let limits = limits.validate()?;
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
        return project_jsonl(subject, raw, limits);
    }
    if !subject.ends_with(".json") {
        return Ok(Vec::new());
    }

    let value = match parse_source_json(raw, limits.max_json_visits, limits.max_json_state_bytes) {
        Ok((value, _)) => value,
        Err(SourceParseError::Budget) => return Err("source JSON exceeds parse budget"),
        Err(SourceParseError::Invalid) if contains_recognized_wrapper(raw) => {
            return Err("malformed recognized source record");
        }
        Err(SourceParseError::Invalid) => return Ok(Vec::new()),
    };
    let mut projection = Projection::default();
    project_value(subject, &value, &mut projection)?;
    Ok(projection.documents)
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
    project_source_text_v2_with_parse_limits(
        root_path,
        exact_root_bytes,
        exact_sibling_metadata,
        SourceSearchParseLimits::default(),
    )
}

pub fn project_source_text_v2_with_parse_limits(
    root_path: &str,
    exact_root_bytes: &[u8],
    exact_sibling_metadata: Option<(&str, &[u8])>,
    limits: SourceSearchParseLimits,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let limits = limits.validate()?;
    let mut projection = Projection::from_documents(project_source_text_with_parse_limits(
        root_path,
        exact_root_bytes,
        limits,
    )?)?;
    let Some(expected_metadata_path) = agent_metadata_path_for_human_forms(root_path) else {
        return if exact_sibling_metadata.is_some() {
            Err("sibling metadata input is unsupported for source path")
        } else {
            Ok(projection.documents)
        };
    };
    let Some((metadata_path, metadata_bytes)) = exact_sibling_metadata else {
        return Ok(projection.documents);
    };
    if metadata_path != expected_metadata_path.as_str() {
        return Err("sibling metadata path does not match HumanForms source");
    }
    if metadata_path.len() > MAX_SUBJECT_PATH_BYTES {
        return Err("sibling metadata path exceeds byte limit");
    }

    let (forms, forms_visits, forms_retained) = parse_foundation_json(
        exact_root_bytes,
        limits.max_json_visits,
        limits
            .max_json_state_bytes
            .saturating_sub(projection.retained_bytes),
    )?;
    if forms
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        != Some("tos_human_form_set_v1")
    {
        return Ok(projection.documents);
    }
    if forms
        .object_get("forms")
        .and_then(JsonValue::as_array)
        .is_none_or(|forms| forms.len() > MAX_PROJECTED_DOCUMENTS)
    {
        return Err("source exceeds document limit");
    }
    let (metadata, _, _) = parse_foundation_json(
        metadata_bytes,
        limits.max_json_visits.saturating_sub(forms_visits),
        limits
            .max_json_state_bytes
            .saturating_sub(projection.retained_bytes)
            .saturating_sub(forms_retained),
    )?;
    if metadata
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        != Some("tos_corpus_record_v1")
        || metadata
            .object_get("record_type")
            .and_then(JsonValue::as_str)
            != Some("agent")
    {
        return Ok(projection.documents);
    }
    let materializations =
        tos_validation::source_forms::source_copy_kernel::materialize_source_forms(
            &metadata, &forms,
        )
        .map_err(|_| "source-copy materialization failed")?;
    if materializations.len() > MAX_PROJECTED_DOCUMENTS {
        return Err("source exceeds document limit");
    }
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
        projection.push(document)?;
    }
    Ok(projection.documents)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceParseError {
    Budget,
    Invalid,
}

fn parse_foundation_json(
    raw: &[u8],
    max_visits: usize,
    available_state_bytes: usize,
) -> Result<(JsonValue, usize, usize), &'static str> {
    if raw.len() > MAX_SOURCE_BYTES {
        return Err("source exceeds byte limit");
    }
    let limits = JsonLimits {
        max_bytes: MAX_SOURCE_BYTES,
        max_depth: MAX_JSON_DEPTH,
        max_visits,
        max_integer_digits: 4_300,
    };
    let document = match parse_json_with_state_budget(
        raw,
        JsonMode::PublishedStrict,
        limits,
        available_state_bytes,
    ) {
        Ok(document) => document,
        Err(error) if error.code == FoundationErrorCode::BudgetExceeded => {
            return Err("source JSON exceeds parse budget");
        }
        Err(_) => return Err("malformed paired source JSON"),
    };
    if !source_json_is_utf8(document.root()) {
        return Err("malformed paired source JSON");
    }
    let visits = document.visits();
    let retained_bytes = document
        .root()
        .retained_storage_bytes()
        .map_err(|_| "source JSON exceeds parse budget")?;
    Ok((document.into_root(), visits, retained_bytes))
}

/// Source JSON historically had to be accepted by serde_json after the
/// bounded parser. Preserve its UTF-8 string rule without allocating another
/// tree, including for otherwise ignored keys and fields.
fn source_json_is_utf8(value: &JsonValue) -> bool {
    match value {
        JsonValue::String(text) => text.as_str().is_some(),
        JsonValue::Array(values) => values.iter().all(source_json_is_utf8),
        JsonValue::Object(entries) => entries
            .iter()
            .all(|(key, value)| key.as_str().is_some() && source_json_is_utf8(value)),
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => true,
    }
}

fn parse_source_json(
    raw: &[u8],
    max_visits: usize,
    available_state_bytes: usize,
) -> Result<(JsonValue, usize), SourceParseError> {
    let (value, visits, _) = parse_foundation_json(raw, max_visits, available_state_bytes)
        .map_err(|error| match error {
            "source exceeds byte limit" | "source JSON exceeds parse budget" => {
                SourceParseError::Budget
            }
            _ => SourceParseError::Invalid,
        })?;
    Ok((value, visits))
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

fn project_jsonl(
    subject: &str,
    raw: &[u8],
    limits: SourceSearchParseLimits,
) -> Result<Vec<SourceSearchDocument>, &'static str> {
    let mut projection = Projection::default();
    let mut visits_remaining = limits.max_json_visits;
    let mut lines = 0usize;
    for line in raw
        .split(|byte| *byte == b'\n')
        .filter(|line| line.iter().any(|byte| !byte.is_ascii_whitespace()))
    {
        lines += 1;
        if lines > MAX_JSONL_LINES {
            return Err("source JSONL exceeds line limit");
        }
        let (value, visits) =
            parse_source_json(line, visits_remaining, limits.max_json_state_bytes).map_err(
                |error| match error {
                    SourceParseError::Budget => "source JSONL exceeds parse budget",
                    SourceParseError::Invalid => "malformed source JSONL line",
                },
            )?;
        visits_remaining = visits_remaining.saturating_sub(visits);
        project_value(subject, &value, &mut projection)?;
    }
    Ok(projection.documents)
}

#[derive(Default)]
struct Projection {
    documents: Vec<SourceSearchDocument>,
    retained_bytes: usize,
}

impl Projection {
    fn from_documents(documents: Vec<SourceSearchDocument>) -> Result<Self, &'static str> {
        if documents.len() > MAX_PROJECTED_DOCUMENTS {
            return Err("source exceeds document limit");
        }
        let retained_bytes = documents.iter().try_fold(0usize, |total, document| {
            document_retained_bytes(document)
                .ok()
                .and_then(|bytes| total.checked_add(bytes))
        });
        let retained_bytes = retained_bytes.ok_or("projected output exceeds byte limit")?;
        if retained_bytes > MAX_PROJECTED_RETAINED_BYTES {
            return Err("projected output exceeds byte limit");
        }
        Ok(Self {
            documents,
            retained_bytes,
        })
    }

    fn push(&mut self, document: SourceSearchDocument) -> Result<(), &'static str> {
        let bytes = document_retained_bytes(&document)?;
        let next = self
            .retained_bytes
            .checked_add(bytes)
            .ok_or("projected output exceeds byte limit")?;
        if self.documents.len() >= MAX_PROJECTED_DOCUMENTS {
            return Err("source exceeds document limit");
        }
        if next > MAX_PROJECTED_RETAINED_BYTES {
            return Err("projected output exceeds byte limit");
        }
        self.documents
            .try_reserve_exact(1)
            .map_err(|_| "source document allocation refused")?;
        self.documents.push(document);
        self.retained_bytes = next;
        Ok(())
    }
}

fn document_retained_bytes(document: &SourceSearchDocument) -> Result<usize, &'static str> {
    document
        .id
        .len()
        .checked_add(document.kind.len())
        .and_then(|bytes| bytes.checked_add(document.title.len()))
        .and_then(|bytes| bytes.checked_add(document.body.len()))
        .and_then(|bytes| {
            document
                .aliases
                .iter()
                .chain(document.dependencies.iter())
                .try_fold(bytes, |total, value| total.checked_add(value.len()))
        })
        .ok_or("projected output exceeds byte limit")
}

fn project_value(
    subject: &str,
    value: &JsonValue,
    projection: &mut Projection,
) -> Result<(), &'static str> {
    if value.as_object().is_none() {
        return Ok(());
    };
    let Some(schema_version) = value
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
    else {
        return Ok(());
    };

    match schema_version {
        "tos_corpus_record_v1" => project_corpus_record(subject, value, projection),
        "tos_historical_record_v1" => project_historical_record(subject, value, projection),
        "tos_historical_claim_v1" => project_historical_claim(subject, value, projection),
        "tos_human_form_set_v1" => project_human_form_set(subject, value, projection),
        "tos_human_form_v1" => {
            if let Some(document) = project_human_form(subject, value)? {
                projection.push(document)?;
            }
            Ok(())
        }
        // There is no maintained tos_corpus_record_v2 contract in this
        // checkout. Do not guess that its fields match v1.
        "tos_corpus_record_v2" | "tos_source_text_unit_packet_v1" => Ok(()),
        _ => Ok(()),
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

fn required_text<'a>(value: &'a JsonValue, field: &'static str) -> Result<&'a str, &'static str> {
    value
        .object_get(field)
        .and_then(JsonValue::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or(field)
}

fn optional_text<'a>(
    value: &'a JsonValue,
    field: &'static str,
) -> Result<Option<&'a str>, &'static str> {
    match value.object_get(field) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(text)) => {
            let text = text.as_str().ok_or(field)?;
            Ok((!text.trim().is_empty()).then_some(text))
        }
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
    value: &JsonValue,
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

fn aliases(value: &JsonValue) -> Result<Vec<String>, &'static str> {
    let Some(labels) = value.object_get("variant_labels") else {
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
    value: &JsonValue,
    projection: &mut Projection,
) -> Result<(), &'static str> {
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
    projection.push(document(
        subject,
        id,
        kind,
        title,
        aliases,
        selected_body(body_parts)?,
    )?)
}

fn project_historical_record(
    subject: &str,
    value: &JsonValue,
    projection: &mut Projection,
) -> Result<(), &'static str> {
    let id = required_text(value, "record_id")?;
    let kind = corpus_kind(value, "record_type", "historical_record")?;
    let title = bounded_text(required_text(value, "preferred_label")?, "preferred_label")?;
    let aliases = aliases(value)?;
    let body = selected_body(
        optional_text(value, "notes")?
            .map(|text| bounded_text(text, "notes"))
            .into_iter(),
    )?;
    projection.push(document(subject, id, kind, title, aliases, body)?)
}

fn project_historical_claim(
    subject: &str,
    value: &JsonValue,
    projection: &mut Projection,
) -> Result<(), &'static str> {
    let id = required_text(value, "claim_id")?;
    let quotes = match value.object_get("supporting_quotes") {
        None => &[][..],
        Some(JsonValue::Array(quotes)) => quotes.as_slice(),
        Some(_) => return Err("supporting_quotes"),
    };
    let body =
        selected_body(quotes.iter().map(|quote| {
            required_text(quote, "exact").and_then(|text| bounded_text(text, "exact"))
        }))?;
    projection.push(document(
        subject,
        id,
        "historical_claim".to_owned(),
        String::new(),
        Vec::new(),
        body,
    )?)
}

fn project_human_form_set(
    subject: &str,
    value: &JsonValue,
    projection: &mut Projection,
) -> Result<(), &'static str> {
    let forms = value
        .object_get("forms")
        .and_then(JsonValue::as_array)
        .filter(|forms| !forms.is_empty())
        .ok_or("forms")?;
    if forms.len() > MAX_PROJECTED_DOCUMENTS {
        return Err("forms exceeds document limit");
    }
    for form in forms {
        if let Some(document) = project_human_form(subject, form)? {
            projection.push(document)?;
        }
    }
    Ok(())
}

fn project_human_form(
    subject: &str,
    value: &JsonValue,
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
    let content = value.object_get("content").ok_or("content")?;
    let content_kind = required_text(content, "kind")?;
    let text = match content_kind {
        "freeform" => Some(required_text(content, "text")?),
        "source-copy" => {
            required_text(content, "slot")?;
            None
        }
        "template" => {
            if !content
                .object_get("template")
                .is_some_and(|template| template.as_object().is_some())
            {
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

    // Keep paired source-copy bytes inside the Rust fixture tree for sparse CI;
    // these canonical virtual paths exercise adjacency without ToS/ checkout bytes.
    const FIXTURE_AGENT_PATH: &str = "ToS/source-witnesses/agents/revision-fixture/agent.json";
    const FIXTURE_FORMS_PATH: &str =
        "ToS/source-witnesses/agents/revision-fixture/agent.human-forms.json";
    const FIXTURE_AGENT_BYTES: &[u8] = include_bytes!(
        "../tests/fixtures/source-native-python-v1/fixtures/record-family-3/ToS/source-witnesses/history/synthetic/episode/agent.json"
    );
    const FIXTURE_FORMS_BYTES: &[u8] = include_bytes!(
        "../tests/fixtures/source-native-python-v1/fixtures/record-family-3/ToS/source-witnesses/history/synthetic/episode/agent.human-forms.json"
    );
    const HISTORY_CLAIMS_PATH: &str = concat!(
        "ToS/source-witnesses/history/",
        "friedrich-nietzsche/jenseits-1886-commission/historical-claims.jsonl"
    );

    #[test]
    fn v2_maps_only_the_exact_adjacent_agent_source_paths() {
        assert_eq!(
            agent_human_forms_path_for_metadata(FIXTURE_AGENT_PATH).as_deref(),
            Some(FIXTURE_FORMS_PATH)
        );
        assert_eq!(
            agent_metadata_path_for_human_forms(FIXTURE_FORMS_PATH).as_deref(),
            Some(FIXTURE_AGENT_PATH)
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
            FIXTURE_FORMS_PATH,
            FIXTURE_FORMS_BYTES,
            Some((FIXTURE_AGENT_PATH, FIXTURE_AGENT_BYTES)),
        )
        .unwrap();

        assert_eq!(documents.len(), 2);
        let name = documents
            .iter()
            .find(|document| document.id == "tos.form.revision-name")
            .unwrap();
        assert_eq!(name.kind, "human_form/name");
        assert_eq!(name.title, "Условный эпизод");
        assert_eq!(name.body, "");
        let hover = documents
            .iter()
            .find(|document| document.kind == "human_form/hover")
            .unwrap();
        assert_eq!(hover.id, "tos.form.revision-note");
        assert_eq!(hover.body, "Не историческое свидетельство.");
        assert!(
            documents.iter().all(|document| {
                document.dependencies == [FIXTURE_FORMS_PATH, FIXTURE_AGENT_PATH]
            })
        );
    }

    #[test]
    fn v2_withholds_source_copy_when_exact_agent_input_makes_pair_stale() {
        let mut changed_agent: serde_json::Value =
            serde_json::from_slice(FIXTURE_AGENT_BYTES).unwrap();
        changed_agent["preferred_label"] = json!("Изменённый эпизод");
        let changed_agent = serde_json::to_vec(&changed_agent).unwrap();
        let documents = project_source_text_v2(
            FIXTURE_FORMS_PATH,
            FIXTURE_FORMS_BYTES,
            Some((FIXTURE_AGENT_PATH, changed_agent.as_slice())),
        )
        .unwrap();
        assert!(documents.is_empty());
    }

    #[test]
    fn v2_rejects_non_adjacent_sibling_bytes() {
        assert_eq!(
            project_source_text_v2(
                FIXTURE_FORMS_PATH,
                FIXTURE_FORMS_BYTES,
                Some((
                    "ToS/source-witnesses/agents/other/agent.json",
                    FIXTURE_AGENT_BYTES
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
    fn initial_parse_workspace_keeps_routine_limits_and_strict_grammar() {
        let line = "{\"schema_version\":\"unindexed\",\"values\":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19]}\n";
        let raw = line.repeat(16_000);
        assert_eq!(
            project_source_text("input.jsonl", raw.as_bytes()),
            Err("source JSONL exceeds parse budget")
        );
        // Four object/string/container visits plus twenty values per line.
        let constrained = SourceSearchParseLimits {
            max_json_visits: 20,
            max_json_state_bytes: MAX_JSON_STATE_BYTES,
        };
        assert_eq!(
            project_source_text_with_parse_limits("input.jsonl", raw.as_bytes(), constrained),
            Err("source JSONL exceeds parse budget")
        );
        let initial = SourceSearchParseLimits {
            max_json_visits: 1_000_000,
            max_json_state_bytes: 128 * 1024 * 1024,
        };
        assert_eq!(
            project_source_text_with_parse_limits("input.jsonl", raw.as_bytes(), initial),
            Ok(Vec::new())
        );
        assert_eq!(
            SourceSearchParseLimits::default().max_json_visits,
            MAX_JSON_VISITS
        );
        assert_eq!(
            SourceSearchParseLimits::default().max_json_state_bytes,
            MAX_JSON_STATE_BYTES
        );
        assert_eq!(
            project_source_text_with_parse_limits("input.jsonl", b"{\"a\":1,\"a\":2}", initial),
            Err("malformed source JSONL line")
        );
        let invalid = SourceSearchParseLimits {
            max_json_visits: usize::MAX,
            ..initial
        };
        assert_eq!(
            project_source_text_with_parse_limits("input.json", b"{}", invalid),
            Err("invalid selected source parse limits")
        );
    }

    #[test]
    fn recognized_jsonl_output_has_a_total_document_limit() {
        let record = r#"{"schema_version":"tos_historical_claim_v1","claim_id":"tos.claim.x","supporting_quotes":[]}"#;
        let raw = std::iter::repeat(record)
            .take(MAX_PROJECTED_DOCUMENTS + 1)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            project_source_text(HISTORY_CLAIMS_PATH, raw.as_bytes()),
            Err("source exceeds document limit")
        );
    }

    #[test]
    fn strict_parser_rejects_duplicate_decoded_field_names() {
        assert_eq!(
            project_source_text(
                "record.json",
                br#"{"schema_version":"tos_corpus_record_v1","record_id":"first","record\u005fid":"second"}"#,
            ),
            Err("malformed recognized source record")
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
