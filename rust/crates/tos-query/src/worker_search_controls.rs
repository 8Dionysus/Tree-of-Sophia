//! Shared D1 Worker search input, order and cursor policy.
//!
//! Cloudflare owns URL/D1 framing and bounded SQL execution. This module owns
//! the search-domain constants and decisions that must agree with native QRY.

use crate::search_v2::{
    SearchRank, SearchV2Error, SearchV2ErrorCode, normalize_worker_search_query,
};
use tos_foundation::python_lower_unicode16_v1;

pub const WORKER_KNOWLEDGE_SOURCES_V1: [&str; 7] = [
    "philosophy",
    "canon",
    "candidate-intake",
    "source-navigation",
    "source-claims",
    "semantic-interchange",
    "repository",
];
pub const WORKER_SEARCH_MAX_OFFSET: usize = 100_000;
pub const WORKER_SEARCH_MAX_PAGE_SIZE: usize = 100;
pub const WORKER_SEARCH_MAX_FILTER_VALUES: usize = 100;
pub const WORKER_SEARCH_MAX_FILTER_CODE_POINTS: usize = 256;
pub const WORKER_SEARCH_MAX_CANDIDATES: u64 = 50_000;
pub const WORKER_SEARCH_MAX_VERIFY_CHARS: u64 = 16_000_000;
pub const WORKER_SEARCH_MAX_INTERSECTION_GRAMS: usize = 3;
pub const WORKER_SEARCH_CURSOR_SCHEMA_V3: &str = "tos_knowledge_search_indexed_cursor_v3";
pub const WORKER_SEARCH_INDEXED_SCHEMA_V2: &str = "tos_knowledge_search_indexed_v2";
pub const WORKER_SEARCH_LEGACY_SCHEMA_V1: &str = "tos_knowledge_search_v1";
pub const WORKER_SEARCH_RANK_CLASSES: u64 = SearchRank::CLASS_COUNT as u64;
pub const WORKER_SEARCH_CURSOR_TOKEN_MAX_CHARS: usize = 8_192;
pub const WORKER_SEARCH_ID_MAX_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerGramSelection {
    pub gram: String,
    pub postings: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerGramPlan {
    pub selected: WorkerGramSelection,
    pub selections: Vec<WorkerGramSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerPreflightOutcome {
    Empty,
    Full,
    Window,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerWindowOutcome {
    pub position: u64,
    pub search_rank: u64,
    pub prefix_chars: u64,
    pub prefix_rows: u64,
    pub remaining_rows: u64,
    pub has_more: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerSearchKind {
    Legacy,
    Indexed,
}

#[derive(Clone, Debug)]
pub struct WorkerSearchInput<'a> {
    pub kind: WorkerSearchKind,
    pub query: &'a str,
    pub sources: Option<&'a [String]>,
    pub kind_ids: &'a [String],
    pub predicate_ids: &'a [String],
    pub offset: usize,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedWorkerSearch {
    pub query: String,
    pub needle: String,
    pub sources: Vec<String>,
    pub kind_ids: Vec<String>,
    pub predicate_ids: Vec<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerSearchControlError {
    pub code: &'static str,
    pub message: String,
}

fn error(code: &'static str, message: impl Into<String>) -> WorkerSearchControlError {
    WorkerSearchControlError {
        code,
        message: message.into(),
    }
}

fn invalid(message: impl Into<String>) -> WorkerSearchControlError {
    error("invalid_request", message)
}

fn search_invalid(message: &'static str) -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::InvalidRequest,
        message,
    }
}

fn query_error(value: SearchV2Error, indexed: bool) -> WorkerSearchControlError {
    match value.code {
        SearchV2ErrorCode::QueryTooLong => invalid("knowledge search query exceeds 256 characters"),
        SearchV2ErrorCode::QueryTooShort => {
            invalid("indexed knowledge search requires a query of at least three characters")
        }
        SearchV2ErrorCode::BudgetExceeded => error("budget_exceeded", value.message),
        _ => invalid(if indexed {
            "indexed knowledge search query is invalid"
        } else {
            "knowledge search query is invalid"
        }),
    }
}

fn code_point_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    left.chars().cmp(right.chars())
}

fn unique(values: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        if !value.is_empty() && !result.contains(value) {
            result.push(value.clone());
        }
    }
    result
}

fn unique_all(values: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        if !result.contains(value) {
            result.push(value.clone());
        }
    }
    result
}

fn canonical(values: &[String]) -> Vec<String> {
    let mut result = unique(values);
    result.sort_by(|left, right| code_point_cmp(left, right));
    result
}

fn canonical_all(values: &[String]) -> Vec<String> {
    let mut result = unique_all(values);
    result.sort_by(|left, right| code_point_cmp(left, right));
    result
}

/// Apply the request-level policy shared by legacy and indexed Worker routes.
/// The host has already split URL list/integer values and retains their HTTP
/// error framing; this function receives the typed request shape.
pub fn normalize_worker_search(
    input: WorkerSearchInput<'_>,
) -> Result<NormalizedWorkerSearch, WorkerSearchControlError> {
    let indexed = input.kind == WorkerSearchKind::Indexed;
    let (query, needle) = normalize_worker_search_query(input.query, indexed)
        .map_err(|value| query_error(value, indexed))?;
    let default_sources: Vec<String> = WORKER_KNOWLEDGE_SOURCES_V1
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
    let requested_sources = input.sources.filter(|values| !values.is_empty());
    let mut sources = requested_sources.map(unique_all).unwrap_or(default_sources);
    let mut unknown: Vec<String> = sources
        .iter()
        .filter(|value| !WORKER_KNOWLEDGE_SOURCES_V1.contains(&value.as_str()))
        .cloned()
        .collect();
    unknown.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    if !unknown.is_empty() {
        return Err(invalid(format!(
            "unsupported knowledge sources: {}",
            unknown.join(", ")
        )));
    }
    if !(1..=WORKER_SEARCH_MAX_PAGE_SIZE).contains(&input.limit) {
        return Err(invalid("limit must be between 1 and 100"));
    }
    if !indexed && input.offset > WORKER_SEARCH_MAX_OFFSET {
        return Err(invalid("offset must be between 0 and 100000"));
    }
    if indexed && input.offset != 0 {
        return Err(invalid(
            "indexed knowledge search uses cursor continuation, not offset",
        ));
    }
    let (kind_ids, predicate_ids) = if indexed {
        let kind_filter_values = sources.len().checked_add(input.kind_ids.len());
        let predicate_filter_values = sources.len().checked_add(input.predicate_ids.len());
        if kind_filter_values.is_none_or(|count| count > WORKER_SEARCH_MAX_FILTER_VALUES)
            || predicate_filter_values.is_none_or(|count| count > WORKER_SEARCH_MAX_FILTER_VALUES)
            || input
                .kind_ids
                .iter()
                .chain(input.predicate_ids)
                .any(|value| value.chars().count() > WORKER_SEARCH_MAX_FILTER_CODE_POINTS)
        {
            return Err(invalid(
                "knowledge search filters exceed bounded query input",
            ));
        }
        (
            canonical_all(input.kind_ids),
            canonical_all(input.predicate_ids),
        )
    } else {
        let kind_ids = unique(input.kind_ids);
        let predicate_ids = unique(input.predicate_ids);
        if kind_ids.len() > WORKER_SEARCH_MAX_FILTER_VALUES
            || predicate_ids.len() > WORKER_SEARCH_MAX_FILTER_VALUES
        {
            return Err(invalid(
                "knowledge search kind and predicate filters must contain at most 100 values",
            ));
        }
        (kind_ids, predicate_ids)
    };
    if indexed {
        sources = canonical(&sources);
    }
    Ok(NormalizedWorkerSearch {
        query,
        needle,
        sources,
        kind_ids,
        predicate_ids,
        offset: input.offset,
        limit: input.limit,
    })
}

/// D1's SQL expression is the transport implementation of QRY's stable four
/// rank classes; it is generated here so adapters cannot diverge in rank or
/// bind order.
pub fn worker_rank_sql(alias: &str, has_needle: bool) -> Result<(String, usize), SearchV2Error> {
    if !matches!(alias, "s" | "c") {
        return Err(SearchV2Error {
            code: SearchV2ErrorCode::InvalidRequest,
            message: "knowledge search SQL alias is invalid",
        });
    }
    if !has_needle {
        return Ok((
            format!(
                "CASE WHEN 1 THEN {} END",
                SearchRank::OtherSerializedCarrierSubstring.class()
            ),
            0,
        ));
    }
    let exact = format!(
        "{alias}.id_lower = ? OR {alias}.native_id_lower = ? OR EXISTS (SELECT 1 FROM json_each({alias}.identity_values) v WHERE v.value = ?)"
    );
    let prefix = format!(
        "instr({alias}.id_lower, ?) = 1 OR instr({alias}.native_id_lower, ?) = 1 OR EXISTS (SELECT 1 FROM json_each({alias}.identity_values) v WHERE instr(v.value, ?) = 1)"
    );
    let visible = format!(
        "EXISTS (SELECT 1 FROM json_each({alias}.visible_values) v WHERE instr(v.value, ?) > 0)"
    );
    Ok((
        format!(
            "CASE WHEN {exact} THEN {} WHEN {prefix} THEN {} WHEN {visible} THEN {} ELSE {} END",
            SearchRank::ExactIdentity.class(),
            SearchRank::IdentityPrefix.class(),
            SearchRank::VisibleDisplaySubstring.class(),
            SearchRank::OtherSerializedCarrierSubstring.class(),
        ),
        7,
    ))
}

/// Lower an already bounded id using the same native Unicode profile used to
/// validate a cursor's order key.
pub fn worker_search_lower_id(value: &str) -> Result<String, SearchV2Error> {
    lower_id_with_limit(value, WORKER_SEARCH_CURSOR_TOKEN_MAX_CHARS)
}

/// Validate the selected D1 identity pair with the same Unicode lower rule
/// used for authored native index identities. The byte ceiling matches the
/// adapter's existing bounded identity projection.
pub fn worker_search_identity_matches_lower(
    id: &str,
    id_lower: &str,
) -> Result<bool, SearchV2Error> {
    if id_lower.len() > WORKER_SEARCH_ID_MAX_BYTES
        || id_lower.chars().count() > WORKER_SEARCH_ID_MAX_BYTES
    {
        return Err(search_invalid("invalid indexed knowledge search cursor"));
    }
    Ok(lower_id_with_limit(id, WORKER_SEARCH_ID_MAX_BYTES)? == id_lower)
}

fn lower_id_with_limit(value: &str, maximum: usize) -> Result<String, SearchV2Error> {
    if value.len() > maximum || value.chars().count() > maximum {
        return Err(search_invalid("invalid indexed knowledge search cursor"));
    }
    python_lower_unicode16_v1(value, maximum, maximum, maximum)
        .map_err(|_| search_invalid("invalid indexed knowledge search cursor"))
}

/// Stable rarest-first posting closure selection for D1. The seed tie goes
/// to the first query-order gram; subsequent equal-size postings retain query
/// order. The complete selected closure stays within the same candidate cap.
pub fn select_worker_search_grams(
    stats: &[WorkerGramSelection],
) -> Result<WorkerGramPlan, WorkerSearchControlError> {
    let Some(first) = stats.first() else {
        return Err(invalid(
            "indexed knowledge search query has no three-code-point gram",
        ));
    };
    let mut selected = first.clone();
    for candidate in &stats[1..] {
        if candidate.postings < selected.postings {
            selected = candidate.clone();
        }
    }
    if selected.postings > WORKER_SEARCH_MAX_CANDIDATES {
        return Err(error(
            "budget_exceeded",
            "indexed knowledge search candidate budget exceeded; narrow the query or use the legacy route",
        ));
    }
    let mut ordered = stats.to_vec();
    ordered.sort_by_key(|candidate| candidate.postings);
    let mut selections = vec![selected.clone()];
    let mut closure = selected.postings;
    if selected.postings > 0 {
        for candidate in ordered {
            if candidate.gram == selected.gram {
                continue;
            }
            if selections.len() >= WORKER_SEARCH_MAX_INTERSECTION_GRAMS
                || closure.saturating_add(candidate.postings) > WORKER_SEARCH_MAX_CANDIDATES
            {
                break;
            }
            closure += candidate.postings;
            selections.push(candidate);
        }
    }
    Ok(WorkerGramPlan {
        selected,
        selections,
    })
}

/// Validate the aggregate carrier-only preflight before the adapter executes
/// rank SQL or loads search text. SQL and response framing remain adapter work.
pub fn worker_search_preflight(
    candidate_rows: Option<u64>,
    verified_chars: Option<u64>,
    rank_chars: Option<u64>,
    invalid_budgets: Option<u64>,
    invalid_rank_metadata: Option<u64>,
) -> Result<(WorkerPreflightOutcome, u64, u64, u64), WorkerSearchControlError> {
    let (
        Some(candidate_rows),
        Some(verified_chars),
        Some(rank_chars),
        Some(invalid_budgets),
        Some(invalid_rank_metadata),
    ) = (
        candidate_rows,
        verified_chars,
        rank_chars,
        invalid_budgets,
        invalid_rank_metadata,
    )
    else {
        return Err(error(
            "unavailable",
            "indexed knowledge search carrier has invalid document budgets",
        ));
    };
    if invalid_budgets != 0
        || invalid_rank_metadata != 0
        || candidate_rows > 9_007_199_254_740_991
        || verified_chars > 9_007_199_254_740_991
        || rank_chars > 9_007_199_254_740_991
    {
        return Err(error(
            "unavailable",
            "indexed knowledge search carrier has invalid document budgets",
        ));
    }
    if candidate_rows > WORKER_SEARCH_MAX_CANDIDATES {
        return Err(error(
            "budget_exceeded",
            "indexed knowledge search candidate budget exceeded",
        ));
    }
    if rank_chars >= WORKER_SEARCH_MAX_VERIFY_CHARS {
        return Err(error(
            "budget_exceeded",
            "indexed knowledge search rank metadata budget exceeded; narrow the query",
        ));
    }
    let outcome = if candidate_rows == 0 {
        WorkerPreflightOutcome::Empty
    } else if verified_chars.saturating_add(rank_chars) > WORKER_SEARCH_MAX_VERIFY_CHARS {
        WorkerPreflightOutcome::Window
    } else {
        WorkerPreflightOutcome::Full
    };
    Ok((outcome, candidate_rows, verified_chars, rank_chars))
}

/// Validate a SQL verification-window row and derive its bounded progress
/// state. A zero remaining-row count is the exact empty continuation case.
pub fn worker_search_window(
    id_bytes: Option<u64>,
    id_lower_bytes: Option<u64>,
    position: Option<u64>,
    search_rank: Option<u64>,
    prefix_chars: Option<u64>,
    prefix_rows: Option<u64>,
    remaining_rows: Option<u64>,
) -> Result<Option<WorkerWindowOutcome>, WorkerSearchControlError> {
    let remaining_rows = remaining_rows.ok_or_else(|| {
        error(
            "unavailable",
            "indexed knowledge search verification window is invalid",
        )
    })?;
    if remaining_rows == 0 {
        return Ok(None);
    }
    let Some(prefix_rows) = prefix_rows else {
        return Err(error(
            "budget_exceeded",
            "indexed knowledge search first remaining document exceeds verification budget",
        ));
    };
    let (Some(id_bytes), Some(id_lower_bytes)) = (id_bytes, id_lower_bytes) else {
        return Err(error(
            "budget_exceeded",
            "indexed knowledge search window identity exceeds delivery budget",
        ));
    };
    let (Some(position), Some(search_rank), Some(prefix_chars)) =
        (position, search_rank, prefix_chars)
    else {
        return Err(error(
            "unavailable",
            "indexed knowledge search verification window is invalid",
        ));
    };
    if id_bytes == 0
        || id_lower_bytes == 0
        || id_bytes > WORKER_SEARCH_ID_MAX_BYTES as u64
        || id_lower_bytes > WORKER_SEARCH_ID_MAX_BYTES as u64
        || position > 9_007_199_254_740_991
        || search_rank >= WORKER_SEARCH_RANK_CLASSES
        || prefix_chars > 9_007_199_254_740_991
        || prefix_rows == 0
        || prefix_rows > remaining_rows
        || remaining_rows > 9_007_199_254_740_991
    {
        return Err(error(
            "unavailable",
            "indexed knowledge search verification window is invalid",
        ));
    }
    Ok(Some(WorkerWindowOutcome {
        position,
        search_rank,
        prefix_chars,
        prefix_rows,
        remaining_rows,
        has_more: prefix_rows < remaining_rows,
    }))
}
