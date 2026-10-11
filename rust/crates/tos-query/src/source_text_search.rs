//! Selected-source text search semantics shared by native and WASM consumers.
//!
//! This operation is deliberately separate from indexed-v2/compressed-v3
//! knowledge search. The adapter supplies one bounded, complete candidate set
//! from its selected source-text profile while its own currentness and rights
//! fence is held. This module ranks only those candidates; it grants no source
//! access, currentness, or disclosure authority.

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

use tos_foundation::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode, JsonLimits, JsonMode,
    JsonNumber, JsonNumberKind, JsonString, JsonValue, canonical_bytes_v1, canonical_count_v1,
    parse_json, python_lower_unicode16_v1, python_lower_unicode16_v1_error_state_upper_bound,
    python_lower_unicode16_v1_with_state_budget_and_check, python_strip_unicode16_v1,
    python_strip_unicode16_v1_with_check,
};

use crate::{AbortProbe, AbortReason};

pub const SOURCE_TEXT_SEARCH_OPERATION: &str = "tos_source_text_search_v1";
/// Shared raw request ceiling used by native projection adapters before the
/// Unicode16 strip/lower pass. Four UTF-8 bytes is the worst-case early guard
/// for one admitted Unicode scalar value.
pub const MAX_QUERY_RAW_CODE_POINTS: usize = 4096;
pub const MAX_QUERY_RAW_UTF8_BYTES: usize = MAX_QUERY_RAW_CODE_POINTS * 4;
const MAX_QUERY_NORMALIZED_CODE_POINTS: usize = 256;
const MAX_FILTER_VALUES: usize = 100;
const MAX_FILTER_VALUE_CODE_POINTS: usize = 1024;
const MAX_FILTER_VALUE_UTF8_BYTES: usize = 4 * MAX_FILTER_VALUE_CODE_POINTS;
const MAX_CURSOR_BYTES: usize = 64 * 1024;
const CURSOR_VERSION: &str = "tos_source_text_search_cursor_v1";
const MAX_SOURCE_TEXT_CANDIDATES: usize = 100_000;
const MAX_SOURCE_TEXT_CODE_POINTS: usize = 16_000_000;
const MAX_SOURCE_TEXT_BYTES: usize = 64 * 1024 * 1024;
const MAX_SOURCE_TEXT_EXCERPT_CODE_POINTS: usize = 2048;
const MAX_SOURCE_TEXT_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_TEXT_WORK_STEPS: u64 = 100_000_000;
const MAX_CONSUMER_ID_BYTES: usize = 256;
const MAX_DOMAIN_BYTES: usize = 1024;
const MAX_CONSUMER_CUT_BYTES: usize = 256;
const MAX_PROFILE_ID_BYTES: usize = 128;
const MAX_DOCUMENT_METADATA_BYTES: usize = 16 * 1024;
const MAX_SOURCE_PATH_BYTES: usize = 4096;
const MAX_DOCUMENT_ID_BYTES: usize = 256;
const MAX_DOCUMENT_IDENTITY_BYTES: usize = 16 * 1024;
const MAX_DOCUMENT_ALIASES: usize = 10_000;
const CALLBACK_DIAGNOSTIC_RESERVE_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceTextSearchErrorCode {
    InvalidRequest,
    BudgetExceeded,
    StaleCursor,
    CorruptCandidate,
    Cancelled,
    DeadlineExceeded,
}

impl SourceTextSearchErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::BudgetExceeded => "budget_exceeded",
            Self::StaleCursor => "stale_cursor",
            Self::CorruptCandidate => "corrupt_candidate",
            Self::Cancelled => "cancelled",
            Self::DeadlineExceeded => "deadline_exceeded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTextSearchError {
    pub code: SourceTextSearchErrorCode,
    pub message: &'static str,
}

impl fmt::Display for SourceTextSearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for SourceTextSearchError {}

pub type Result<T> = std::result::Result<T, SourceTextSearchError>;

fn error(code: SourceTextSearchErrorCode, message: &'static str) -> SourceTextSearchError {
    SourceTextSearchError { code, message }
}

fn invalid() -> SourceTextSearchError {
    error(
        SourceTextSearchErrorCode::InvalidRequest,
        "source-text search request is invalid",
    )
}

fn budget_error() -> SourceTextSearchError {
    error(
        SourceTextSearchErrorCode::BudgetExceeded,
        "source-text search budget exceeded",
    )
}

fn corrupt() -> SourceTextSearchError {
    error(
        SourceTextSearchErrorCode::CorruptCandidate,
        "selected source-text candidate is invalid",
    )
}

fn foundation_error(reason: FoundationError) -> SourceTextSearchError {
    if reason.code == FoundationErrorCode::BudgetExceeded {
        budget_error()
    } else {
        invalid()
    }
}

fn poll(probe: &dyn AbortProbe) -> Result<()> {
    match probe.reason() {
        None => Ok(()),
        Some(AbortReason::Cancelled) => Err(error(
            SourceTextSearchErrorCode::Cancelled,
            "source-text search cancelled",
        )),
        Some(AbortReason::DeadlineExceeded) => Err(error(
            SourceTextSearchErrorCode::DeadlineExceeded,
            "source-text search deadline exceeded",
        )),
    }
}

/// Unicode-16 strip plus lowercase shared by request matching and PG index
/// construction. The caller supplies exact input/output caps; overflow fails
/// closed rather than truncating an indexed field.
pub fn normalize_source_text(
    value: &str,
    max_input_code_points: usize,
    max_output_code_points: usize,
    max_output_bytes: usize,
) -> Result<String> {
    if value.len() > max_input_code_points.saturating_mul(4) {
        return Err(budget_error());
    }
    let stripped =
        python_strip_unicode16_v1(value, max_input_code_points).map_err(foundation_error)?;
    python_lower_unicode16_v1(
        stripped,
        max_input_code_points,
        max_output_code_points,
        max_output_bytes,
    )
    .map_err(foundation_error)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTextSearchRequest {
    pub query: String,
    pub limit: usize,
    /// Exact owner document-type identifiers. Empty means no type filter.
    pub document_types: Vec<String>,
    /// Exact selected source paths. Empty means no path filter.
    pub source_paths: Vec<String>,
    /// Opaque continuation state. It is consistency state, never authority.
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedSourceTextSearchRequest {
    query: String,
    limit: usize,
    document_types: Vec<String>,
    source_paths: Vec<String>,
    request_sha256: Digest256,
}

impl NormalizedSourceTextSearchRequest {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn document_types(&self) -> &[String] {
        &self.document_types
    }

    pub fn source_paths(&self) -> &[String] {
        &self.source_paths
    }
}

impl SourceTextSearchRequest {
    /// Parse the bounded request object through the shared FND JSON parser.
    pub fn from_json(raw: &[u8], mut limits: JsonLimits) -> Result<Self> {
        limits.max_bytes = limits.max_bytes.min(MAX_CURSOR_BYTES);
        limits.max_depth = limits.max_depth.min(16);
        limits.max_visits = limits.max_visits.min(4096);
        limits.max_integer_digits = limits.max_integer_digits.min(20);
        let document =
            parse_json(raw, JsonMode::RequestLastWins, limits).map_err(foundation_error)?;
        Self::from_value(document.root())
    }

    pub fn from_value(value: &JsonValue) -> Result<Self> {
        let Some(fields) = value.as_object() else {
            return Err(invalid());
        };
        if fields.iter().any(|(key, _)| {
            !matches!(
                key.as_str(),
                Some("query" | "limit" | "document_types" | "source_paths" | "cursor")
            )
        }) {
            return Err(invalid());
        }
        let query = value
            .object_get("query")
            .and_then(JsonValue::as_str)
            .ok_or_else(invalid)?
            .to_owned();
        let limit = value
            .object_get("limit")
            .and_then(JsonValue::as_u64)
            .and_then(|number| usize::try_from(number).ok())
            .filter(|number| (1..=100).contains(number))
            .ok_or_else(invalid)?;
        let document_types = string_array(value.object_get("document_types"), MAX_FILTER_VALUES)?;
        let source_paths = string_array(value.object_get("source_paths"), MAX_FILTER_VALUES)?;
        let cursor = match value.object_get("cursor") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::String(value)) => Some(value.as_str().ok_or_else(invalid)?.to_owned()),
            _ => return Err(invalid()),
        };
        Ok(Self {
            query,
            limit,
            document_types,
            source_paths,
            cursor,
        })
    }

    pub fn normalize(&self) -> Result<NormalizedSourceTextSearchRequest> {
        if !(1..=100).contains(&self.limit)
            || self.query.contains('\0')
            || self
                .cursor
                .as_ref()
                .is_some_and(|cursor| cursor.len() > MAX_CURSOR_BYTES)
        {
            return Err(invalid());
        }
        let query = normalize_source_text(
            &self.query,
            MAX_QUERY_RAW_CODE_POINTS,
            MAX_QUERY_NORMALIZED_CODE_POINTS,
            MAX_QUERY_NORMALIZED_CODE_POINTS * 4,
        )
        .map_err(|_| invalid())?;
        let query_points = query.chars().count();
        if !(1..=MAX_QUERY_NORMALIZED_CODE_POINTS).contains(&query_points) {
            return Err(invalid());
        }
        let document_types = canonical_filter(self.document_types.clone())?;
        let source_paths = canonical_filter(self.source_paths.clone())?;
        let digest_value = object(vec![
            ("operation_profile", text(SOURCE_TEXT_SEARCH_OPERATION)),
            ("query", text(&query)),
            ("limit", number(self.limit as u64)),
            ("document_types", string_values(&document_types)),
            ("source_paths", string_values(&source_paths)),
        ]);
        let digest_bytes = canonical_bytes_v1(
            &digest_value,
            CanonicalProfile::SourceRecordDigestV1,
            json_limits(256 * 1024),
        )
        .map_err(foundation_error)?;
        Ok(NormalizedSourceTextSearchRequest {
            query,
            limit: self.limit,
            document_types,
            source_paths,
            request_sha256: Digest256::of_bytes(&digest_bytes),
        })
    }
}

fn string_array(value: Option<&JsonValue>, max_count: usize) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(invalid)?;
    if values.len() > max_count {
        return Err(invalid());
    }
    values
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or_else(invalid))
        .collect()
}

fn canonical_filter(values: Vec<String>) -> Result<Vec<String>> {
    if values.len() > MAX_FILTER_VALUES {
        return Err(invalid());
    }
    let mut canonical = BTreeSet::new();
    for value in values {
        if value.is_empty()
            || value.len() > MAX_FILTER_VALUE_UTF8_BYTES
            || value.chars().count() > MAX_FILTER_VALUE_CODE_POINTS
        {
            return Err(invalid());
        }
        canonical.insert(value);
    }
    Ok(canonical.into_iter().collect())
}

/// Exact selected consumer/profile context from the owner adapter. These
/// fields bind cursors but do not prove that the adapter's selection is current.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTextSearchSelection {
    pub consumer_id: String,
    pub domain: String,
    pub consumer_cut: String,
    pub profile_id: String,
    pub profile_sha256: Digest256,
    pub selection_seq: u64,
}

impl SourceTextSearchSelection {
    pub fn from_json(raw: &[u8], mut limits: JsonLimits) -> Result<Self> {
        limits.max_bytes = limits.max_bytes.min(16 * 1024);
        limits.max_depth = limits.max_depth.min(8);
        limits.max_visits = limits.max_visits.min(128);
        limits.max_integer_digits = limits.max_integer_digits.min(20);
        let document =
            parse_json(raw, JsonMode::PublishedStrict, limits).map_err(foundation_error)?;
        let value = document.root();
        if !only_fields(
            value,
            &[
                "consumer_id",
                "domain",
                "consumer_cut",
                "profile_id",
                "profile_sha256",
                "selection_seq",
            ],
        ) {
            return Err(invalid());
        }
        let selection = Self {
            consumer_id: required_string(value, "consumer_id")?,
            domain: required_string(value, "domain")?,
            consumer_cut: required_string(value, "consumer_cut")?,
            profile_id: required_string(value, "profile_id")?,
            profile_sha256: Digest256::from_hex(
                value
                    .object_get("profile_sha256")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| invalid())?,
            selection_seq: value
                .object_get("selection_seq")
                .and_then(JsonValue::as_u64)
                .ok_or_else(invalid)?,
        };
        validate_selection(&selection)?;
        Ok(selection)
    }
}

/// One candidate already selected and bounded by its data owner. The query
/// kernel never opens paths or loads additional documents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTextDocument {
    pub document_type: String,
    pub source_path: String,
    pub custody_revision: String,
    pub document_id: String,
    pub selection_seq: u64,
    pub identity: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub body: String,
}

/// The adapter must return the full bounded candidate set for this request.
/// A false completeness bit is a budget refusal, never an empty successful page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTextCandidateSet<'a> {
    pub complete: bool,
    pub documents: &'a [SourceTextDocument],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedSourceTextCandidateSet {
    pub complete: bool,
    pub documents: Vec<SourceTextDocument>,
}

impl OwnedSourceTextCandidateSet {
    pub fn from_json(raw: &[u8], mut limits: JsonLimits) -> Result<Self> {
        limits.max_bytes = limits.max_bytes.min(64 * 1024 * 1024);
        limits.max_depth = limits.max_depth.min(32);
        limits.max_visits = limits.max_visits.min(1_000_000);
        limits.max_integer_digits = limits.max_integer_digits.min(20);
        let document =
            parse_json(raw, JsonMode::PublishedStrict, limits).map_err(foundation_error)?;
        let value = document.root();
        if !only_fields(value, &["complete", "documents"]) {
            return Err(invalid());
        }
        let complete = value
            .object_get("complete")
            .and_then(JsonValue::as_bool)
            .ok_or_else(invalid)?;
        let document_values = value
            .object_get("documents")
            .and_then(JsonValue::as_array)
            .ok_or_else(invalid)?;
        if document_values.len() > MAX_SOURCE_TEXT_CANDIDATES {
            return Err(budget_error());
        }
        let documents = document_values
            .iter()
            .map(document_from_value)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            complete,
            documents,
        })
    }

    pub fn as_candidate_set(&self) -> SourceTextCandidateSet<'_> {
        SourceTextCandidateSet {
            complete: self.complete,
            documents: &self.documents,
        }
    }
}

fn document_from_value(value: &JsonValue) -> Result<SourceTextDocument> {
    if !only_fields(
        value,
        &[
            "document_type",
            "source_path",
            "custody_revision",
            "document_id",
            "selection_seq",
            "identity",
            "title",
            "aliases",
            "body",
        ],
    ) {
        return Err(invalid());
    }
    Ok(SourceTextDocument {
        document_type: required_string(value, "document_type")?,
        source_path: required_string(value, "source_path")?,
        custody_revision: required_string(value, "custody_revision")?,
        document_id: required_string(value, "document_id")?,
        selection_seq: value
            .object_get("selection_seq")
            .and_then(JsonValue::as_u64)
            .ok_or_else(invalid)?,
        identity: required_string(value, "identity")?,
        title: required_string(value, "title")?,
        aliases: string_array(value.object_get("aliases"), MAX_DOCUMENT_ALIASES)?,
        body: required_string(value, "body")?,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTextSearchBudget {
    pub max_candidates: usize,
    pub max_text_code_points: usize,
    pub max_text_bytes: usize,
    pub max_excerpt_code_points: usize,
    pub max_response_bytes: usize,
    pub max_work_steps: u64,
}

impl SourceTextSearchBudget {
    pub fn from_json(raw: &[u8], mut limits: JsonLimits) -> Result<Self> {
        limits.max_bytes = limits.max_bytes.min(4096);
        limits.max_depth = limits.max_depth.min(8);
        limits.max_visits = limits.max_visits.min(64);
        limits.max_integer_digits = limits.max_integer_digits.min(20);
        let document =
            parse_json(raw, JsonMode::PublishedStrict, limits).map_err(foundation_error)?;
        let value = document.root();
        if !only_fields(
            value,
            &[
                "max_candidates",
                "max_text_code_points",
                "max_text_bytes",
                "max_excerpt_code_points",
                "max_response_bytes",
                "max_work_steps",
            ],
        ) {
            return Err(invalid());
        }
        let budget = Self {
            max_candidates: required_usize(value, "max_candidates")?,
            max_text_code_points: required_usize(value, "max_text_code_points")?,
            max_text_bytes: required_usize(value, "max_text_bytes")?,
            max_excerpt_code_points: required_usize(value, "max_excerpt_code_points")?,
            max_response_bytes: required_usize(value, "max_response_bytes")?,
            max_work_steps: value
                .object_get("max_work_steps")
                .and_then(JsonValue::as_u64)
                .ok_or_else(invalid)?,
        };
        validate_budget(&budget)?;
        Ok(budget)
    }
}

fn validate_budget(budget: &SourceTextSearchBudget) -> Result<()> {
    if budget.max_candidates == 0
        || budget.max_text_code_points == 0
        || budget.max_text_bytes == 0
        || budget.max_excerpt_code_points == 0
        || budget.max_response_bytes == 0
        || budget.max_work_steps == 0
    {
        Err(invalid())
    } else if budget.max_candidates > MAX_SOURCE_TEXT_CANDIDATES
        || budget.max_text_code_points > MAX_SOURCE_TEXT_CODE_POINTS
        || budget.max_text_bytes > MAX_SOURCE_TEXT_BYTES
        || budget.max_excerpt_code_points > MAX_SOURCE_TEXT_EXCERPT_CODE_POINTS
        || budget.max_response_bytes > MAX_SOURCE_TEXT_RESPONSE_BYTES
        || budget.max_work_steps > MAX_SOURCE_TEXT_WORK_STEPS
    {
        Err(budget_error())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceTextExcerptField {
    Title,
    Body,
}

impl SourceTextExcerptField {
    fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Body => "body",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTextSearchHit {
    pub document_type: String,
    pub source_path: String,
    pub custody_revision: String,
    pub document_id: String,
    pub selection_seq: u64,
    /// Integer priority: 0 exact identity, 1 exact title, 2 title substring,
    /// 3 exact alias, 4 alias substring, 5 body substring.
    pub rank: u8,
    pub excerpt_field: SourceTextExcerptField,
    /// A bounded prefix copied from the original title/body bytes.
    pub excerpt: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTextSearchPage {
    pub selection: SourceTextSearchSelection,
    pub normalized_query: String,
    pub hits: Vec<SourceTextSearchHit>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

impl SourceTextSearchPage {
    /// Render the bounded result with the shared FND canonical JSON writer.
    pub fn render_json(&self, max_bytes: usize, probe: &dyn AbortProbe) -> Result<Vec<u8>> {
        poll(probe)?;
        let max_bytes = max_bytes.min(MAX_SOURCE_TEXT_RESPONSE_BYTES);
        ensure_page_text_size(self, max_bytes)?;
        let value = page_value(self);
        let mut limits = json_limits(max_bytes);
        limits.max_visits = 100_000;
        let bytes = canonical_bytes_v1(&value, CanonicalProfile::SourceRecordDigestV1, limits)
            .map_err(foundation_error)?;
        poll(probe)?;
        Ok(bytes)
    }
}

/// Execute over one complete bounded adapter-provided candidate set. The
/// selected source fence remains the adapter's responsibility.
pub fn execute_source_text_search(
    request: &SourceTextSearchRequest,
    selection: &SourceTextSearchSelection,
    candidates: SourceTextCandidateSet<'_>,
    budget: SourceTextSearchBudget,
    probe: &dyn AbortProbe,
) -> Result<SourceTextSearchPage> {
    poll(probe)?;
    validate_budget(&budget)?;
    validate_selection(selection)?;
    let normalized = request.normalize()?;
    if !candidates.complete || candidates.documents.len() > budget.max_candidates {
        return Err(budget_error());
    }
    let cursor = request
        .cursor
        .as_deref()
        .map(|raw| parse_cursor(raw, &normalized, selection))
        .transpose()?;
    let mut work = WorkMeter::new(budget, probe)?;
    let mut selected = Vec::<ScoredIndex>::new();
    let mut candidate_identities = BTreeSet::<(String, String)>::new();
    let mut largest_candidate_identity_key_bytes = 0usize;
    let retain_count = normalized.limit.checked_add(1).ok_or_else(budget_error)?;

    for (index, document) in candidates.documents.iter().enumerate() {
        let candidate_comparison_count = 8u64
            .checked_add(ceil_log2(budget.max_candidates))
            .ok_or_else(budget_error)?;
        work.charge_work(candidate_comparison_count)?;
        validate_document(document, selection)?;
        let input_bytes = candidate_input_bytes(document, &mut work)?;
        work.charge_text_bytes(input_bytes)?;
        let candidate_identity_bytes = document
            .source_path
            .len()
            .checked_add(document.document_id.len())
            .ok_or_else(budget_error)?;
        work.charge_text_bytes(candidate_identity_bytes)?;
        let candidate_comparison_bytes = candidate_identity_bytes
            .checked_add(largest_candidate_identity_key_bytes)
            .ok_or_else(budget_error)?;
        let candidate_comparison_work = candidate_comparison_count
            .checked_mul(u64::try_from(candidate_comparison_bytes).map_err(|_| budget_error())?)
            .ok_or_else(budget_error)?;
        // The uniqueness tree compares path and ID bytes at each level. Charge
        // the existing conservative comparison-count bound in byte units too.
        work.charge_work(candidate_comparison_work)?;
        if !candidate_identities
            .insert((document.source_path.clone(), document.document_id.clone()))
        {
            return Err(corrupt());
        }
        largest_candidate_identity_key_bytes =
            largest_candidate_identity_key_bytes.max(candidate_identity_bytes);
        if !matches_filter(&normalized, document) {
            continue;
        }
        let identity = normalize_document_text(&document.identity, &mut work)?;
        let rank = match_document(document, &normalized.query, &identity, &mut work)?;
        let Some(rank) = rank else {
            continue;
        };
        let key_copy_bytes = document
            .source_path
            .len()
            .checked_add(document.document_id.len())
            .ok_or_else(budget_error)?;
        work.charge_text_bytes(key_copy_bytes)?;
        let key = OrderKey {
            rank,
            identity,
            source_path: document.source_path.clone(),
            document_id: document.document_id.clone(),
        };
        if let Some(cursor) = &cursor {
            work.charge_work(order_key_comparison_work(&key, &cursor.after)?)?;
            if key.cmp(&cursor.after) != Ordering::Greater {
                continue;
            }
        }
        let selected_shift = u64::try_from(selected.len()).map_err(|_| budget_error())?;
        let selected_search = ceil_log2(selected.len().saturating_add(1));
        work.charge_work(
            selected_shift
                .checked_add(selected_search)
                .ok_or_else(budget_error)?,
        )?;
        let scored = ScoredIndex { key, index };
        let position = order_key_position(&selected, &scored.key, &mut work)?;
        selected.insert(position, scored);
        if selected.len() > retain_count {
            selected.pop();
        }
        poll(probe)?;
    }

    let has_more = selected.len() > normalized.limit;
    selected.truncate(normalized.limit);
    let mut hits = Vec::with_capacity(selected.len());
    for scored in &selected {
        let document = &candidates.documents[scored.index];
        let (excerpt_field, excerpt_source) = excerpt_source(document, scored.key.rank);
        let metadata_bytes = document
            .document_type
            .len()
            .checked_add(document.source_path.len())
            .and_then(|count| count.checked_add(document.custody_revision.len()))
            .and_then(|count| count.checked_add(document.document_id.len()))
            .ok_or_else(budget_error)?;
        work.charge_text_bytes(metadata_bytes)?;
        let excerpt_byte_budget = budget.max_response_bytes / selected.len().max(1);
        let excerpt = bounded_excerpt(
            excerpt_source,
            budget.max_excerpt_code_points,
            excerpt_byte_budget,
            &mut work,
        )?;
        hits.push(SourceTextSearchHit {
            document_type: document.document_type.clone(),
            source_path: document.source_path.clone(),
            custody_revision: document.custody_revision.clone(),
            document_id: document.document_id.clone(),
            selection_seq: document.selection_seq,
            rank: scored.key.rank,
            excerpt_field,
            excerpt,
        });
    }
    let next_cursor = if has_more {
        selected
            .last()
            .map(|last| render_cursor(&normalized, selection, &last.key))
            .transpose()?
    } else {
        None
    };
    if let Some(cursor) = &next_cursor {
        work.charge_text_bytes(cursor.len())?;
    }
    let selection_copy_bytes = selection
        .consumer_id
        .len()
        .checked_add(selection.domain.len())
        .and_then(|bytes| bytes.checked_add(selection.consumer_cut.len()))
        .and_then(|bytes| bytes.checked_add(selection.profile_id.len()))
        .and_then(|bytes| bytes.checked_add(normalized.query.len()))
        .ok_or_else(budget_error)?;
    work.charge_text_bytes(selection_copy_bytes)?;
    let page = SourceTextSearchPage {
        selection: selection.clone(),
        normalized_query: normalized.query,
        hits,
        has_more,
        next_cursor,
    };
    validate_output_size(&page, budget.max_response_bytes, probe)?;
    Ok(page)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OrderKey {
    rank: u8,
    identity: String,
    source_path: String,
    document_id: String,
}

impl Ord for OrderKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank
            .cmp(&other.rank)
            .then_with(|| self.identity.as_bytes().cmp(other.identity.as_bytes()))
            .then_with(|| {
                self.source_path
                    .as_bytes()
                    .cmp(other.source_path.as_bytes())
            })
            .then_with(|| {
                self.document_id
                    .as_bytes()
                    .cmp(other.document_id.as_bytes())
            })
    }
}

impl PartialOrd for OrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn order_key_byte_len(key: &OrderKey) -> Result<usize> {
    key.identity
        .len()
        .checked_add(key.source_path.len())
        .and_then(|bytes| bytes.checked_add(key.document_id.len()))
        .ok_or_else(budget_error)
}

fn order_key_comparison_work(left: &OrderKey, right: &OrderKey) -> Result<u64> {
    let compared_bytes = order_key_byte_len(left)?
        .checked_add(order_key_byte_len(right)?)
        .ok_or_else(budget_error)?;
    u64::try_from(compared_bytes).map_err(|_| budget_error())
}

fn order_key_position(
    selected: &[ScoredIndex],
    key: &OrderKey,
    work: &mut WorkMeter<'_>,
) -> Result<usize> {
    let mut low = 0usize;
    let mut high = selected.len();
    while low < high {
        let middle = low + (high - low) / 2;
        work.charge_work(order_key_comparison_work(&selected[middle].key, key)?)?;
        if selected[middle].key.cmp(key) == Ordering::Less {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    Ok(low)
}

#[derive(Clone, Debug)]
struct ScoredIndex {
    key: OrderKey,
    index: usize,
}

struct WorkMeter<'a> {
    remaining_work: u64,
    remaining_text_code_points: usize,
    remaining_text_bytes: usize,
    probe: &'a dyn AbortProbe,
}

impl<'a> WorkMeter<'a> {
    fn new(budget: SourceTextSearchBudget, probe: &'a dyn AbortProbe) -> Result<Self> {
        // Keep FND's diagnostic reserve in this caller's original byte ledger,
        // outside the workspace offered to its Unicode helper.
        let diagnostic_reserve = python_lower_unicode16_v1_error_state_upper_bound()
            .checked_add(CALLBACK_DIAGNOSTIC_RESERVE_BYTES)
            .ok_or_else(budget_error)?;
        let remaining_text_bytes = budget
            .max_text_bytes
            .checked_sub(diagnostic_reserve)
            .ok_or_else(budget_error)?;
        Ok(Self {
            remaining_work: budget.max_work_steps,
            remaining_text_code_points: budget.max_text_code_points,
            remaining_text_bytes,
            probe,
        })
    }

    fn charge_work(&mut self, amount: u64) -> Result<()> {
        poll(self.probe)?;
        self.remaining_work = self
            .remaining_work
            .checked_sub(amount)
            .ok_or_else(budget_error)?;
        Ok(())
    }

    fn charge_text_bytes(&mut self, amount: usize) -> Result<()> {
        self.charge_work(1)?;
        self.remaining_text_bytes = self
            .remaining_text_bytes
            .checked_sub(amount)
            .ok_or_else(budget_error)?;
        Ok(())
    }
}

fn normalize_document_text(value: &str, work: &mut WorkMeter<'_>) -> Result<String> {
    let input_points = work.remaining_text_code_points;
    let output_code_points = work
        .remaining_text_code_points
        .checked_mul(3)
        .ok_or_else(budget_error)?;
    let output_bytes = work.remaining_text_bytes;
    let available_state = work.remaining_text_bytes;
    let charging_input_points = Cell::new(true);
    let mut callback_error = None;
    let mut callback = || -> tos_foundation::Result<()> {
        if let Err(failure) = work.charge_work(1) {
            callback_error = Some(failure);
            return Err(FoundationError::new(
                FoundationErrorCode::BudgetExceeded,
                "source-text work stopped",
            ));
        }
        if charging_input_points.get() {
            work.remaining_text_code_points = match work.remaining_text_code_points.checked_sub(1) {
                Some(remaining) => remaining,
                None => {
                    callback_error = Some(budget_error());
                    return Err(FoundationError::new(
                        FoundationErrorCode::BudgetExceeded,
                        "source-text text budget exceeded",
                    ));
                }
            };
        }
        Ok(())
    };
    let stripped = match python_strip_unicode16_v1_with_check(value, input_points, &mut callback) {
        Ok(stripped) => stripped,
        Err(reason) => {
            drop(callback);
            return Err(callback_error.unwrap_or_else(|| foundation_error(reason)));
        }
    };
    charging_input_points.set(false);
    let lowered = python_lower_unicode16_v1_with_state_budget_and_check(
        stripped,
        input_points,
        output_code_points,
        output_bytes,
        available_state,
        &mut callback,
    );
    drop(callback);
    let lowered =
        lowered.map_err(|reason| callback_error.unwrap_or_else(|| foundation_error(reason)))?;
    work.charge_text_bytes(lowered.len())?;
    Ok(lowered)
}

fn candidate_input_bytes(document: &SourceTextDocument, work: &mut WorkMeter<'_>) -> Result<usize> {
    let mut bytes = document
        .document_type
        .len()
        .checked_add(document.source_path.len())
        .and_then(|count| count.checked_add(document.custody_revision.len()))
        .and_then(|count| count.checked_add(document.document_id.len()))
        .and_then(|count| count.checked_add(document.identity.len()))
        .and_then(|count| count.checked_add(document.title.len()))
        .and_then(|count| count.checked_add(document.body.len()))
        .ok_or_else(budget_error)?;
    for alias in &document.aliases {
        work.charge_work(1)?;
        bytes = bytes.checked_add(alias.len()).ok_or_else(budget_error)?;
    }
    Ok(bytes)
}

fn validate_selection(selection: &SourceTextSearchSelection) -> Result<()> {
    if selection.consumer_id.is_empty()
        || selection.domain.is_empty()
        || selection.consumer_cut.is_empty()
        || selection.profile_id.is_empty()
        || selection.consumer_id.len() > MAX_CONSUMER_ID_BYTES
        || selection.domain.len() > MAX_DOMAIN_BYTES
        || selection.consumer_cut.len() > MAX_CONSUMER_CUT_BYTES
        || selection.profile_id.len() > MAX_PROFILE_ID_BYTES
    {
        Err(invalid())
    } else {
        Ok(())
    }
}

fn validate_document(
    document: &SourceTextDocument,
    selection: &SourceTextSearchSelection,
) -> Result<()> {
    if document.document_type.is_empty()
        || document.source_path.is_empty()
        || document.custody_revision.is_empty()
        || document.document_id.is_empty()
        || document.identity.is_empty()
        || document.selection_seq == 0
        || document.selection_seq > selection.selection_seq
    {
        Err(corrupt())
    } else if document.document_type.len() > MAX_DOCUMENT_METADATA_BYTES
        || document.source_path.len() > MAX_SOURCE_PATH_BYTES
        || document.custody_revision.len() > MAX_DOCUMENT_METADATA_BYTES
        || document.document_id.len() > MAX_DOCUMENT_ID_BYTES
        || document.identity.len() > MAX_DOCUMENT_IDENTITY_BYTES
        || document.aliases.len() > MAX_DOCUMENT_ALIASES
    {
        Err(budget_error())
    } else {
        Ok(())
    }
}

fn matches_filter(
    request: &NormalizedSourceTextSearchRequest,
    document: &SourceTextDocument,
) -> bool {
    (request.document_types.is_empty()
        || request
            .document_types
            .binary_search(&document.document_type)
            .is_ok())
        && (request.source_paths.is_empty()
            || request
                .source_paths
                .binary_search(&document.source_path)
                .is_ok())
}

fn match_document(
    document: &SourceTextDocument,
    query: &str,
    identity: &str,
    work: &mut WorkMeter<'_>,
) -> Result<Option<u8>> {
    charge_match_work(identity, query, work)?;
    if identity == query {
        return Ok(Some(0));
    }
    let title = normalize_document_text(&document.title, work)?;
    charge_match_work(&title, query, work)?;
    if title == query {
        return Ok(Some(1));
    }
    if title.contains(query) {
        return Ok(Some(2));
    }
    let mut alias_match = false;
    for alias in &document.aliases {
        work.charge_work(1)?;
        let normalized = normalize_document_text(alias, work)?;
        charge_match_work(&normalized, query, work)?;
        if normalized == query {
            return Ok(Some(3));
        }
        if normalized.contains(query) {
            alias_match = true;
        }
    }
    if alias_match {
        return Ok(Some(4));
    }
    let body = normalize_document_text(&document.body, work)?;
    charge_match_work(&body, query, work)?;
    Ok(body.contains(query).then_some(5))
}

fn charge_match_work(value: &str, query: &str, work: &mut WorkMeter<'_>) -> Result<()> {
    let steps = value
        .len()
        .checked_add(query.len())
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(budget_error)?;
    work.charge_work(steps)
}

fn ceil_log2(value: usize) -> u64 {
    u64::from(usize::BITS - value.saturating_sub(1).leading_zeros())
}

fn excerpt_source(document: &SourceTextDocument, rank: u8) -> (SourceTextExcerptField, &str) {
    if rank == 5 && !document.body.is_empty() {
        (SourceTextExcerptField::Body, &document.body)
    } else if !document.title.is_empty() {
        (SourceTextExcerptField::Title, &document.title)
    } else {
        (SourceTextExcerptField::Body, &document.body)
    }
}

fn bounded_excerpt(
    value: &str,
    max_code_points: usize,
    max_bytes: usize,
    work: &mut WorkMeter<'_>,
) -> Result<String> {
    let mut chars = value.chars();
    let mut excerpt = String::new();
    let mut truncated = false;
    for _ in 0..max_code_points {
        work.charge_work(1)?;
        let Some(ch) = chars.next() else {
            return Ok(excerpt);
        };
        if excerpt
            .len()
            .checked_add(ch.len_utf8())
            .filter(|size| *size <= max_bytes)
            .is_none()
        {
            truncated = true;
            break;
        }
        work.charge_text_bytes(ch.len_utf8())?;
        excerpt.push(ch);
    }
    if !truncated {
        work.charge_work(1)?;
        truncated = chars.next().is_some();
    }
    if truncated {
        while excerpt
            .len()
            .checked_add('…'.len_utf8())
            .filter(|size| *size <= max_bytes)
            .is_none()
        {
            let Some(_removed) = excerpt.pop() else {
                break;
            };
            work.charge_work(1)?;
        }
        if excerpt
            .len()
            .checked_add('…'.len_utf8())
            .is_some_and(|size| size <= max_bytes)
        {
            work.charge_text_bytes('…'.len_utf8())?;
            excerpt.push('…');
        }
    }
    Ok(excerpt)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceTextCursor {
    request_sha256: Digest256,
    selection: SourceTextSearchSelection,
    after: OrderKey,
}

fn parse_cursor(
    raw: &str,
    request: &NormalizedSourceTextSearchRequest,
    selection: &SourceTextSearchSelection,
) -> Result<SourceTextCursor> {
    if raw.len() > MAX_CURSOR_BYTES {
        return Err(invalid());
    }
    let mut limits = json_limits(MAX_CURSOR_BYTES);
    limits.max_depth = 8;
    limits.max_visits = 64;
    let document =
        parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits).map_err(|_| invalid())?;
    let value = document.root();
    if !only_fields(value, &["version", "request_sha256", "selection", "after"])
        || required_string(value, "version")? != CURSOR_VERSION
    {
        return Err(invalid());
    }
    let encoded = canonical_bytes_v1(value, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(foundation_error)?;
    if encoded != raw.as_bytes() {
        return Err(invalid());
    }
    let request_sha256 = Digest256::from_hex(
        value
            .object_get("request_sha256")
            .and_then(JsonValue::as_str)
            .ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?;
    let encoded_selection =
        selection_from_value(value.object_get("selection").ok_or_else(invalid)?)?;
    let after_value = value.object_get("after").ok_or_else(invalid)?;
    if !only_fields(
        after_value,
        &["rank", "identity", "source_path", "document_id"],
    ) {
        return Err(invalid());
    }
    let after = OrderKey {
        rank: after_value
            .object_get("rank")
            .and_then(JsonValue::as_u64)
            .and_then(|rank| u8::try_from(rank).ok())
            .filter(|rank| *rank <= 5)
            .ok_or_else(invalid)?,
        identity: required_string(after_value, "identity")?,
        source_path: required_string(after_value, "source_path")?,
        document_id: required_string(after_value, "document_id")?,
    };
    if request_sha256 != request.request_sha256 || &encoded_selection != selection {
        return Err(error(
            SourceTextSearchErrorCode::StaleCursor,
            "source-text continuation belongs to another request or selected consumer",
        ));
    }
    Ok(SourceTextCursor {
        request_sha256,
        selection: encoded_selection,
        after,
    })
}

fn render_cursor(
    request: &NormalizedSourceTextSearchRequest,
    selection: &SourceTextSearchSelection,
    after: &OrderKey,
) -> Result<String> {
    let value = object(vec![
        ("version", text(CURSOR_VERSION)),
        ("request_sha256", text(&request.request_sha256.to_hex())),
        ("selection", selection_value(selection)),
        (
            "after",
            object(vec![
                ("rank", number(u64::from(after.rank))),
                ("identity", text(&after.identity)),
                ("source_path", text(&after.source_path)),
                ("document_id", text(&after.document_id)),
            ]),
        ),
    ]);
    let bytes = canonical_bytes_v1(
        &value,
        CanonicalProfile::SourceRecordDigestV1,
        json_limits(MAX_CURSOR_BYTES),
    )
    .map_err(foundation_error)?;
    String::from_utf8(bytes).map_err(|_| invalid())
}

fn selection_from_value(value: &JsonValue) -> Result<SourceTextSearchSelection> {
    if !only_fields(
        value,
        &[
            "consumer_id",
            "domain",
            "consumer_cut",
            "profile_id",
            "profile_sha256",
            "selection_seq",
        ],
    ) {
        return Err(invalid());
    }
    let selection = SourceTextSearchSelection {
        consumer_id: required_string(value, "consumer_id")?,
        domain: required_string(value, "domain")?,
        consumer_cut: required_string(value, "consumer_cut")?,
        profile_id: required_string(value, "profile_id")?,
        profile_sha256: Digest256::from_hex(
            value
                .object_get("profile_sha256")
                .and_then(JsonValue::as_str)
                .ok_or_else(invalid)?,
        )
        .map_err(|_| invalid())?,
        selection_seq: value
            .object_get("selection_seq")
            .and_then(JsonValue::as_u64)
            .ok_or_else(invalid)?,
    };
    validate_selection(&selection)?;
    Ok(selection)
}

fn validate_output_size(
    page: &SourceTextSearchPage,
    max_bytes: usize,
    probe: &dyn AbortProbe,
) -> Result<()> {
    poll(probe)?;
    ensure_page_text_size(page, max_bytes)?;
    let mut limits = json_limits(max_bytes);
    limits.max_visits = 100_000;
    canonical_count_v1(
        &page_value(page),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(foundation_error)?;
    poll(probe)
}

fn ensure_page_text_size(page: &SourceTextSearchPage, max_bytes: usize) -> Result<()> {
    let mut text_bytes = 0usize;
    let mut add = |value: &str| -> Result<()> {
        text_bytes = text_bytes
            .checked_add(value.len())
            .filter(|size| *size <= max_bytes)
            .ok_or_else(budget_error)?;
        Ok(())
    };
    add(&page.selection.consumer_id)?;
    add(&page.selection.domain)?;
    add(&page.selection.consumer_cut)?;
    add(&page.selection.profile_id)?;
    add(&page.normalized_query)?;
    if let Some(cursor) = &page.next_cursor {
        add(cursor)?;
    }
    for hit in &page.hits {
        add(&hit.document_type)?;
        add(&hit.source_path)?;
        add(&hit.custody_revision)?;
        add(&hit.document_id)?;
        add(&hit.excerpt)?;
    }
    Ok(())
}

fn page_value(page: &SourceTextSearchPage) -> JsonValue {
    object(vec![
        ("operation_profile", text(SOURCE_TEXT_SEARCH_OPERATION)),
        ("query", text(&page.normalized_query)),
        ("selection", selection_value(&page.selection)),
        (
            "hits",
            JsonValue::Array(
                page.hits
                    .iter()
                    .map(|hit| {
                        object(vec![
                            ("document_type", text(&hit.document_type)),
                            ("source_path", text(&hit.source_path)),
                            ("custody_revision", text(&hit.custody_revision)),
                            ("document_id", text(&hit.document_id)),
                            ("selection_seq", number(hit.selection_seq)),
                            ("rank", number(u64::from(hit.rank))),
                            ("excerpt_field", text(hit.excerpt_field.as_str())),
                            ("excerpt", text(&hit.excerpt)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("has_more", JsonValue::Bool(page.has_more)),
        (
            "next_cursor",
            page.next_cursor.as_deref().map_or(JsonValue::Null, text),
        ),
    ])
}

fn selection_value(selection: &SourceTextSearchSelection) -> JsonValue {
    object(vec![
        ("consumer_id", text(&selection.consumer_id)),
        ("domain", text(&selection.domain)),
        ("consumer_cut", text(&selection.consumer_cut)),
        ("profile_id", text(&selection.profile_id)),
        ("profile_sha256", text(&selection.profile_sha256.to_hex())),
        ("selection_seq", number(selection.selection_seq)),
    ])
}

fn only_fields(value: &JsonValue, allowed: &[&str]) -> bool {
    value.as_object().is_some_and(|fields| {
        fields
            .iter()
            .all(|(key, _)| key.as_str().is_some_and(|name| allowed.contains(&name)))
    })
}

fn required_string(value: &JsonValue, key: &str) -> Result<String> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(invalid)
}

fn required_usize(value: &JsonValue, key: &str) -> Result<usize> {
    value
        .object_get(key)
        .and_then(JsonValue::as_u64)
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(invalid)
}

fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

fn string_values(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|value| text(value)).collect())
}

fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}

fn json_limits(max_bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes,
        max_depth: 16,
        max_visits: 100_000,
        max_integer_digits: 20,
    }
}

/// Portable raw-JSON adapter for WASM parity checks and native fixtures. Live
/// adapters should keep candidate text behind the selected owner boundary.
pub fn execute_source_text_search_json(
    request_json: &[u8],
    selection_json: &[u8],
    candidate_set_json: &[u8],
    budget_json: &[u8],
    limits: JsonLimits,
    probe: &dyn AbortProbe,
) -> Result<Vec<u8>> {
    poll(probe)?;
    let request = SourceTextSearchRequest::from_json(request_json, limits)?;
    let selection = SourceTextSearchSelection::from_json(selection_json, limits)?;
    let candidates = OwnedSourceTextCandidateSet::from_json(candidate_set_json, limits)?;
    let budget = SourceTextSearchBudget::from_json(budget_json, limits)?;
    let page = execute_source_text_search(
        &request,
        &selection,
        candidates.as_candidate_set(),
        budget,
        probe,
    )?;
    page.render_json(budget.max_response_bytes, probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoAbort;
    impl AbortProbe for NoAbort {
        fn reason(&self) -> Option<AbortReason> {
            None
        }
    }

    struct Cancel;
    impl AbortProbe for Cancel {
        fn reason(&self) -> Option<AbortReason> {
            Some(AbortReason::Cancelled)
        }
    }

    fn selection() -> SourceTextSearchSelection {
        SourceTextSearchSelection {
            consumer_id: "web-local".to_owned(),
            domain: "tos-local".to_owned(),
            consumer_cut: "cut-7".to_owned(),
            profile_id: "tos_source_search_projection_v2".to_owned(),
            profile_sha256: Digest256::of_bytes(b"profile"),
            selection_seq: 7,
        }
    }

    fn document(
        id: &str,
        identity: &str,
        title: &str,
        aliases: &[&str],
        body: &str,
    ) -> SourceTextDocument {
        SourceTextDocument {
            document_type: "agent".to_owned(),
            source_path: format!("ToS/agents/{id}.json"),
            custody_revision: "sha256:source".to_owned(),
            document_id: id.to_owned(),
            selection_seq: 7,
            identity: identity.to_owned(),
            title: title.to_owned(),
            aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
            body: body.to_owned(),
        }
    }

    fn budget() -> SourceTextSearchBudget {
        SourceTextSearchBudget {
            max_candidates: 20,
            max_text_code_points: 4096,
            max_text_bytes: 32 * 1024,
            max_excerpt_code_points: 64,
            max_response_bytes: 16 * 1024,
            max_work_steps: 2_000_000,
        }
    }

    fn request(query: &str, limit: usize, cursor: Option<String>) -> SourceTextSearchRequest {
        SourceTextSearchRequest {
            query: query.to_owned(),
            limit,
            document_types: Vec::new(),
            source_paths: Vec::new(),
            cursor,
        }
    }

    #[test]
    fn unicode16_normalization_accepts_one_code_point_and_uses_final_sigma() {
        assert_eq!(normalize_source_text("  Я  ", 8, 8, 32).unwrap(), "я");
        assert_eq!(normalize_source_text("ΟΣ", 8, 8, 32).unwrap(), "ος");
        assert_eq!(request("  Я ", 1, None).normalize().unwrap().query(), "я");
        assert_eq!(
            request("   ", 1, None).normalize().unwrap_err().code,
            SourceTextSearchErrorCode::InvalidRequest
        );
        assert_eq!(
            request("a\0b", 1, None).normalize().unwrap_err().code,
            SourceTextSearchErrorCode::InvalidRequest
        );
    }

    #[test]
    fn ranking_is_deterministic_and_cursor_binds_request_and_consumer() {
        let docs = vec![
            document("c", "id-c", "Something", &["needle"], "body"),
            document("b", "id-b", "needle title", &[], "body"),
            document("a", "needle", "Other", &[], "body"),
        ];
        let selection = selection();
        let first = execute_source_text_search(
            &request("needle", 1, None),
            &selection,
            SourceTextCandidateSet {
                complete: true,
                documents: &docs,
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        assert_eq!(first.hits[0].document_id, "a");
        let cursor = first.next_cursor.clone().expect("second page exists");
        let second = execute_source_text_search(
            &request("needle", 1, Some(cursor.clone())),
            &selection,
            SourceTextCandidateSet {
                complete: true,
                documents: &docs,
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        assert_eq!(second.hits[0].document_id, "b");
        let third = execute_source_text_search(
            &request(
                "needle",
                1,
                Some(second.next_cursor.clone().unwrap_or_default()),
            ),
            &selection,
            SourceTextCandidateSet {
                complete: true,
                documents: &docs,
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        assert_eq!(third.hits[0].document_id, "c");

        let mut other_consumer = selection.clone();
        other_consumer.consumer_id = "other".to_owned();
        assert_eq!(
            execute_source_text_search(
                &request("needle", 1, Some(cursor.clone())),
                &other_consumer,
                SourceTextCandidateSet {
                    complete: true,
                    documents: &docs,
                },
                budget(),
                &NoAbort,
            )
            .unwrap_err()
            .code,
            SourceTextSearchErrorCode::StaleCursor
        );
        assert_eq!(
            execute_source_text_search(
                &request("other", 1, Some(cursor)),
                &selection,
                SourceTextCandidateSet {
                    complete: true,
                    documents: &docs,
                },
                budget(),
                &NoAbort,
            )
            .unwrap_err()
            .code,
            SourceTextSearchErrorCode::StaleCursor
        );
    }

    #[test]
    fn all_six_match_classes_use_the_declared_priority_order() {
        let docs = vec![
            document("f", "id-f", "Other", &[], "body needle"),
            document("e", "id-e", "Other", &["before needle"], "Other"),
            document("d", "id-d", "Other", &["needle"], "Other"),
            document("c", "id-c", "needle title", &[], "Other"),
            document("b", "id-b", "needle", &[], "Other"),
            document("a", "needle", "Other", &[], "Other"),
        ];
        let page = execute_source_text_search(
            &request("needle", 10, None),
            &selection(),
            SourceTextCandidateSet {
                complete: true,
                documents: &docs,
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        assert_eq!(
            page.hits.iter().map(|hit| hit.rank).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5]
        );
        assert_eq!(
            page.hits
                .iter()
                .map(|hit| hit.document_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c", "d", "e", "f"]
        );
    }

    #[test]
    fn incomplete_candidate_set_and_cancelled_work_refuse() {
        let docs = [document("a", "id-a", "Title", &[], "needle")];
        assert_eq!(
            execute_source_text_search(
                &request("needle", 10, None),
                &selection(),
                SourceTextCandidateSet {
                    complete: false,
                    documents: &docs,
                },
                budget(),
                &NoAbort,
            )
            .unwrap_err()
            .code,
            SourceTextSearchErrorCode::BudgetExceeded
        );
        assert_eq!(
            execute_source_text_search(
                &request("needle", 10, None),
                &selection(),
                SourceTextCandidateSet {
                    complete: true,
                    documents: &docs,
                },
                budget(),
                &Cancel,
            )
            .unwrap_err()
            .code,
            SourceTextSearchErrorCode::Cancelled
        );
    }

    #[test]
    fn empty_initial_selection_at_sequence_zero_returns_empty_page() {
        let mut initial = selection();
        initial.consumer_cut = "0".to_owned();
        initial.selection_seq = 0;
        let page = execute_source_text_search(
            &request("needle", 10, None),
            &initial,
            SourceTextCandidateSet {
                complete: true,
                documents: &[],
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        assert!(page.hits.is_empty());
        assert!(!page.has_more);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn equal_rank_ties_use_c_style_byte_order() {
        let mut lowercase = document("lower", "same", "needle", &[], "body");
        lowercase.source_path = "z/path".to_owned();
        let mut uppercase = document("upper", "same", "needle", &[], "body");
        uppercase.source_path = "A/path".to_owned();
        let docs = [lowercase, uppercase];
        let page = execute_source_text_search(
            &request("needle", 10, None),
            &selection(),
            SourceTextCandidateSet {
                complete: true,
                documents: &docs,
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        assert_eq!(
            page.hits
                .iter()
                .map(|hit| hit.document_id.as_str())
                .collect::<Vec<_>>(),
            vec!["upper", "lower"]
        );
    }

    #[test]
    fn owner_max_identity_and_path_fields_remain_searchable_and_cursor_bounded() {
        let long_path = format!("ToS/{}", "😀".repeat(1023));
        assert_eq!(long_path.len(), MAX_SOURCE_PATH_BYTES);
        assert_eq!(long_path.chars().count(), 1027);
        let long_identity = "x".repeat(MAX_DOCUMENT_IDENTITY_BYTES);
        let first_id = "a".repeat(MAX_DOCUMENT_ID_BYTES);
        let second_id = "b".repeat(MAX_DOCUMENT_ID_BYTES);
        let mut first_document = document(&first_id, &long_identity, "", &[], "needle");
        first_document.source_path = long_path.clone();
        first_document.selection_seq = u64::MAX;
        let mut second_document = document(&second_id, &long_identity, "", &[], "needle");
        second_document.source_path = long_path.clone();
        second_document.selection_seq = u64::MAX;

        let mut selection = selection();
        selection.consumer_id = "c".repeat(MAX_CONSUMER_ID_BYTES);
        selection.domain = "d".repeat(MAX_DOMAIN_BYTES);
        selection.consumer_cut = "c".repeat(MAX_CONSUMER_CUT_BYTES);
        selection.profile_id = "p".repeat(MAX_PROFILE_ID_BYTES);
        selection.selection_seq = u64::MAX;
        let documents = [first_document, second_document];
        let mut budget = budget();
        budget.max_candidates = documents.len();
        budget.max_text_code_points = 40_000;
        budget.max_text_bytes = 256 * 1024;
        budget.max_response_bytes = 64 * 1024;
        budget.max_work_steps = 2_000_000;
        let mut request = request("needle", 1, None);

        let first_page = execute_source_text_search(
            &request,
            &selection,
            SourceTextCandidateSet {
                complete: true,
                documents: &documents,
            },
            budget,
            &NoAbort,
        )
        .unwrap();
        assert_eq!(first_page.hits[0].document_id, first_id);
        assert_eq!(first_page.hits[0].source_path, long_path);
        let cursor = first_page.next_cursor.clone().expect("second page exists");
        assert!(cursor.len() > 2048);
        assert!(cursor.len() < MAX_CURSOR_BYTES);
        let rendered = first_page.render_json(64 * 1024, &NoAbort).unwrap();
        assert!(rendered.len() <= 64 * 1024);

        let mut second_request = request.clone();
        second_request.cursor = Some(cursor);
        let second_page = execute_source_text_search(
            &second_request,
            &selection,
            SourceTextCandidateSet {
                complete: true,
                documents: &documents,
            },
            budget,
            &NoAbort,
        )
        .unwrap();
        assert_eq!(second_page.hits[0].document_id, second_id);
        assert!(!second_page.has_more);
    }

    #[test]
    fn duplicate_source_path_document_identity_is_corrupt_even_when_unmatched() {
        let first = document("a", "id-a", "needle", &[], "body");
        let second = document("a", "id-other", "other", &[], "other");
        assert_eq!(first.source_path, second.source_path);
        assert_eq!(first.document_id, second.document_id);
        assert_eq!(
            execute_source_text_search(
                &request("needle", 10, None),
                &selection(),
                SourceTextCandidateSet {
                    complete: true,
                    documents: &[first, second],
                },
                budget(),
                &NoAbort,
            )
            .unwrap_err()
            .code,
            SourceTextSearchErrorCode::CorruptCandidate
        );
    }

    #[test]
    fn page_renders_only_bounded_original_excerpt_and_metadata() {
        let docs = [document(
            "a",
            "id-a",
            "A title",
            &[],
            "prefix needle suffix",
        )];
        let page = execute_source_text_search(
            &request("needle", 10, None),
            &selection(),
            SourceTextCandidateSet {
                complete: true,
                documents: &docs,
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        let bytes = page.render_json(16 * 1024, &NoAbort).unwrap();
        let rendered = String::from_utf8(bytes).unwrap();
        assert!(rendered.contains("prefix needle suffix"));
        assert!(rendered.contains("custody_revision"));
        assert!(!rendered.contains("aliases"));
        assert!(rendered.contains("\"excerpt_field\":\"body\""));
        assert!(!rendered.contains("\"body\":"));
    }

    #[test]
    fn budget_and_filters_fail_closed() {
        let docs = [document("a", "id-a", "Needle", &[], "body")];
        let mut too_small = budget();
        too_small.max_response_bytes = 16;
        assert_eq!(
            execute_source_text_search(
                &request("needle", 10, None),
                &selection(),
                SourceTextCandidateSet {
                    complete: true,
                    documents: &docs,
                },
                too_small,
                &NoAbort,
            )
            .unwrap_err()
            .code,
            SourceTextSearchErrorCode::BudgetExceeded
        );
        let mut filtered = request("needle", 10, None);
        filtered.document_types = vec!["text".to_owned()];
        let page = execute_source_text_search(
            &filtered,
            &selection(),
            SourceTextCandidateSet {
                complete: true,
                documents: &docs,
            },
            budget(),
            &NoAbort,
        )
        .unwrap();
        assert!(page.hits.is_empty());
    }
}
