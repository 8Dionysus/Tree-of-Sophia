//! Foundation-backed Worker/D1 search-domain controls. The host owns URL,
//! base64, database and HTTP framing; query and continuation decisions live in
//! `tos-query` and cross WASM through this versioned JSON envelope.

use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};
use tos_query::{
    search_index::{INDEXED_SEARCH_GRAM_CODEPOINTS_V1, unique_search_grams},
    search_v2::{SearchRank, SearchV2Error},
    worker_search_controls::{
        NormalizedWorkerSearch, WORKER_SEARCH_CURSOR_SCHEMA_V3,
        WORKER_SEARCH_CURSOR_TOKEN_MAX_CHARS, WORKER_SEARCH_ID_MAX_BYTES,
        WORKER_SEARCH_INDEXED_SCHEMA_V2, WORKER_SEARCH_LEGACY_SCHEMA_V1,
        WORKER_SEARCH_MAX_CANDIDATES, WORKER_SEARCH_MAX_INTERSECTION_GRAMS,
        WORKER_SEARCH_MAX_OFFSET, WORKER_SEARCH_MAX_PAGE_SIZE, WORKER_SEARCH_MAX_VERIFY_CHARS,
        WORKER_SEARCH_RANK_CLASSES, WorkerGramSelection, WorkerPreflightOutcome,
        WorkerSearchControlError, WorkerSearchInput, WorkerSearchKind, WorkerWindowOutcome,
        normalize_worker_search, select_worker_search_grams, worker_rank_sql,
        worker_search_identity_matches_lower, worker_search_preflight, worker_search_window,
    },
};

const ENVELOPE_VERSION: usize = 1;
const MAX_CURSOR_BYTES: usize = WORKER_SEARCH_CURSOR_TOKEN_MAX_CHARS;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

fn invalid(message: &str) -> WorkerSearchControlError {
    WorkerSearchControlError {
        code: "invalid_request",
        message: message.to_owned(),
    }
}

fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn integer(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn integer_u64(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(name, value)| (JsonString::from_utf8(name), value))
            .collect(),
    )
}
fn strings(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|value| text(value)).collect())
}
fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn required_string<'a>(
    value: &'a JsonValue,
    key: &str,
) -> Result<&'a str, WorkerSearchControlError> {
    get(value, key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| WorkerSearchControlError {
            code: "invalid_request",
            message: format!("Worker search control field `{key}` must be a string"),
        })
}
fn optional_strings(
    value: &JsonValue,
    key: &str,
) -> Result<Option<Vec<String>>, WorkerSearchControlError> {
    match get(value, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Array(values)) => values
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| WorkerSearchControlError {
                        code: "invalid_request",
                        message: format!("Worker search control `{key}` must contain strings"),
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => Err(WorkerSearchControlError {
            code: "invalid_request",
            message: format!("Worker search control `{key}` must be an array or null"),
        }),
    }
}
fn required_strings(value: &JsonValue, key: &str) -> Result<Vec<String>, WorkerSearchControlError> {
    optional_strings(value, key)?.ok_or_else(|| WorkerSearchControlError {
        code: "invalid_request",
        message: format!("Worker search control `{key}` must be an array"),
    })
}
fn required_usize(value: &JsonValue, key: &str) -> Result<usize, WorkerSearchControlError> {
    get(value, key)
        .and_then(JsonValue::as_u64)
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| WorkerSearchControlError {
            code: "invalid_request",
            message: format!("Worker search control field `{key}` must be a nonnegative integer"),
        })
}
fn ok(value: JsonValue) -> JsonValue {
    object(vec![
        ("schema_version", integer(ENVELOPE_VERSION)),
        ("ok", JsonValue::Bool(true)),
        ("value", value),
    ])
}
fn failed(error: WorkerSearchControlError) -> JsonValue {
    object(vec![
        ("schema_version", integer(ENVELOPE_VERSION)),
        ("ok", JsonValue::Bool(false)),
        (
            "error",
            object(vec![
                ("code", text(error.code)),
                ("message", text(&error.message)),
            ]),
        ),
    ])
}
fn parse(bytes: &[u8], maximum: usize) -> Result<JsonValue, WorkerSearchControlError> {
    parse_json(
        bytes,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: maximum,
            ..JsonLimits::default()
        },
    )
    .map(|parsed| parsed.into_root())
    .map_err(|_| WorkerSearchControlError {
        code: "invalid_request",
        message: "invalid Worker search control JSON".to_owned(),
    })
}
fn emit(value: &JsonValue) -> Vec<u8> {
    emit_value_preserved_json(value, JsonLimits::default())
        .unwrap_or_else(|_| b"{\"schema_version\":1,\"ok\":false,\"error\":{\"code\":\"invalid_request\",\"message\":\"Worker search control output is invalid\"}}".to_vec())
}
fn search_error(error: SearchV2Error) -> WorkerSearchControlError {
    WorkerSearchControlError {
        code: match error.code {
            tos_query::search_v2::SearchV2ErrorCode::BudgetExceeded => "budget_exceeded",
            _ => "invalid_request",
        },
        message: error.message.to_owned(),
    }
}
fn normalized(value: NormalizedWorkerSearch) -> JsonValue {
    let mut sources_sorted = value.sources.clone();
    let mut kinds_sorted = value.kind_ids.clone();
    let mut predicates_sorted = value.predicate_ids.clone();
    sources_sorted.sort_by(|a, b| a.chars().cmp(b.chars()));
    kinds_sorted.sort_by(|a, b| a.chars().cmp(b.chars()));
    predicates_sorted.sort_by(|a, b| a.chars().cmp(b.chars()));
    object(vec![
        ("query", text(&value.query)),
        ("needle", text(&value.needle)),
        ("sources", strings(&value.sources)),
        ("kind_ids", strings(&value.kind_ids)),
        ("predicate_ids", strings(&value.predicate_ids)),
        ("offset", integer(value.offset)),
        ("limit", integer(value.limit)),
        (
            "filters",
            object(vec![
                ("sources", strings(&sources_sorted)),
                ("kind_ids", strings(&kinds_sorted)),
                ("predicate_ids", strings(&predicates_sorted)),
            ]),
        ),
    ])
}

fn policy() -> JsonValue {
    object(vec![
        (
            "gram_code_points",
            integer(INDEXED_SEARCH_GRAM_CODEPOINTS_V1 as usize),
        ),
        ("max_candidates", integer_u64(WORKER_SEARCH_MAX_CANDIDATES)),
        (
            "max_verify_chars",
            integer_u64(WORKER_SEARCH_MAX_VERIFY_CHARS),
        ),
        (
            "max_intersection_grams",
            integer(WORKER_SEARCH_MAX_INTERSECTION_GRAMS),
        ),
        ("cursor_schema", text(WORKER_SEARCH_CURSOR_SCHEMA_V3)),
        ("indexed_schema", text(WORKER_SEARCH_INDEXED_SCHEMA_V2)),
        ("legacy_schema", text(WORKER_SEARCH_LEGACY_SCHEMA_V1)),
        ("rank_classes", integer(WORKER_SEARCH_RANK_CLASSES as usize)),
        (
            "cursor_token_max_chars",
            integer(WORKER_SEARCH_CURSOR_TOKEN_MAX_CHARS),
        ),
        ("identity_max_bytes", integer(WORKER_SEARCH_ID_MAX_BYTES)),
    ])
}

fn same_string(value: Option<&JsonValue>, expected: &str) -> bool {
    value.and_then(JsonValue::as_str) == Some(expected)
}
fn exact_keys(value: &JsonValue, expected: &[&str]) -> bool {
    let Some(entries) = value.as_object() else {
        return false;
    };
    let mut actual: Vec<&str> = entries.iter().filter_map(|(key, _)| key.as_str()).collect();
    if actual.len() != entries.len() {
        return false;
    }
    let mut expected = expected.to_vec();
    actual.sort_unstable();
    expected.sort_unstable();
    actual == expected
}
fn safe_int(value: Option<&JsonValue>) -> Option<u64> {
    let JsonValue::Number(number) = value? else {
        return None;
    };
    if number.kind != JsonNumberKind::Int {
        return None;
    }
    let value = number.lexeme.parse::<u64>().ok()?;
    (value <= MAX_SAFE_INTEGER).then_some(value)
}

fn safe_count(value: Option<&JsonValue>) -> Option<u64> {
    let JsonValue::Number(number) = value? else {
        return None;
    };
    let parsed = number.lexeme.parse::<f64>().ok()?;
    (parsed.is_finite()
        && parsed >= 0.0
        && parsed.fract() == 0.0
        && parsed <= MAX_SAFE_INTEGER as f64)
        .then_some(parsed as u64)
}

fn unavailable(message: &'static str) -> WorkerSearchControlError {
    WorkerSearchControlError {
        code: "unavailable",
        message: message.to_owned(),
    }
}

fn packet_counts<'a>(
    value: &JsonValue,
    keys: &'a [&'a str],
) -> Result<Vec<(&'a str, JsonValue)>, WorkerSearchControlError> {
    keys.iter()
        .map(|key| {
            safe_count(get(value, key))
                .map(|number| (*key, integer_u64(number)))
                .ok_or_else(|| unavailable("knowledge search result counts are invalid"))
        })
        .collect()
}

fn cursor_json(value: &JsonValue) -> String {
    String::from_utf8(emit_value_preserved_json(value, JsonLimits::default()).unwrap_or_default())
        .unwrap_or_default()
}

fn gram_plan(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let Some(stats) = get(request, "stats").and_then(JsonValue::as_array) else {
        return Err(unavailable(
            "indexed knowledge search gram statistics are invalid",
        ));
    };
    let mut values = Vec::with_capacity(stats.len());
    for value in stats {
        let (Some(gram), Some(postings)) = (
            get(value, "gram").and_then(JsonValue::as_str),
            safe_count(get(value, "postings")),
        ) else {
            return Err(unavailable(
                "indexed knowledge search gram statistics are invalid",
            ));
        };
        values.push(WorkerGramSelection {
            gram: gram.to_owned(),
            postings,
        });
    }
    let plan = select_worker_search_grams(&values)?;
    let selection = |value: &WorkerGramSelection| {
        object(vec![
            ("gram", text(&value.gram)),
            ("postings", integer_u64(value.postings)),
        ])
    };
    Ok(object(vec![
        ("selected", selection(&plan.selected)),
        (
            "selections",
            JsonValue::Array(plan.selections.iter().map(selection).collect()),
        ),
    ]))
}

fn validate_posting_closure(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let expected = safe_count(get(request, "expected"));
    let total = safe_count(get(request, "total"));
    let missing = safe_count(get(request, "missing"));
    let (Some(expected), Some(total), Some(missing)) = (expected, total, missing) else {
        return Err(unavailable(
            "indexed knowledge search posting metadata is invalid",
        ));
    };
    if total > WORKER_SEARCH_MAX_CANDIDATES {
        return Err(WorkerSearchControlError {
            code: "budget_exceeded",
            message: "indexed knowledge search candidate budget exceeded".to_owned(),
        });
    }
    if total != expected || missing != 0 {
        return Err(unavailable(
            "indexed knowledge search posting closure is incomplete",
        ));
    }
    Ok(object(vec![("valid", JsonValue::Bool(true))]))
}

fn preflight(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let (outcome, candidate_rows, verified_chars, rank_chars) = worker_search_preflight(
        safe_count(get(request, "candidate_rows")),
        safe_count(get(request, "verified_chars")),
        safe_count(get(request, "rank_chars")),
        safe_count(get(request, "invalid_budgets")),
        safe_count(get(request, "invalid_rank_metadata")),
    )?;
    let outcome = match outcome {
        WorkerPreflightOutcome::Empty => "empty",
        WorkerPreflightOutcome::Full => "full",
        WorkerPreflightOutcome::Window => "window",
    };
    Ok(object(vec![
        ("outcome", text(outcome)),
        ("candidate_rows", integer_u64(candidate_rows)),
        ("verified_chars", integer_u64(verified_chars)),
        ("rank_chars", integer_u64(rank_chars)),
    ]))
}

fn window(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let outcome = worker_search_window(
        safe_count(get(request, "id_bytes")),
        safe_count(get(request, "id_lower_bytes")),
        safe_count(get(request, "position")),
        safe_count(get(request, "search_rank")),
        safe_count(get(request, "prefix_chars")),
        safe_count(get(request, "prefix_rows")),
        safe_count(get(request, "remaining_rows")),
    )?;
    let Some(outcome) = outcome else {
        return Ok(object(vec![("outcome", text("empty"))]));
    };
    Ok(window_object(&outcome))
}

fn window_object(value: &WorkerWindowOutcome) -> JsonValue {
    object(vec![
        ("outcome", text("window")),
        ("position", integer_u64(value.position)),
        ("search_rank", integer_u64(value.search_rank)),
        ("prefix_chars", integer_u64(value.prefix_chars)),
        ("prefix_rows", integer_u64(value.prefix_rows)),
        ("remaining_rows", integer_u64(value.remaining_rows)),
        ("has_more", JsonValue::Bool(value.has_more)),
    ])
}

fn encode_cursor(request: &JsonValue, outer: bool) -> Result<JsonValue, WorkerSearchControlError> {
    let revision = required_string(request, "source_revision")?;
    let query = required_string(request, "query")?;
    let snapshot_epoch = safe_count(get(request, "snapshot_epoch"))
        .filter(|value| *value <= MAX_SAFE_INTEGER)
        .ok_or_else(|| {
            cursor_error("invalid_request", "Worker search snapshot epoch is invalid")
        })?;
    let filters = get(request, "filters")
        .filter(|value| exact_keys(value, &["sources", "kind_ids", "predicate_ids"]))
        .ok_or_else(|| cursor_error("invalid_request", "Worker search filters are invalid"))?;
    let cursor = if outer {
        let nodes = get(request, "nodes").cloned().unwrap_or(JsonValue::Null);
        let relations = get(request, "relations")
            .cloned()
            .unwrap_or(JsonValue::Null);
        let nodes_exhausted = get(request, "nodes_exhausted")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| {
                cursor_error("invalid_request", "Worker search cursor state is invalid")
            })?;
        let relations_exhausted = get(request, "relations_exhausted")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| {
                cursor_error("invalid_request", "Worker search cursor state is invalid")
            })?;
        let valid = |value: &JsonValue, exhausted: bool| match (exhausted, value) {
            (true, JsonValue::Null) => true,
            (false, JsonValue::String(value)) => {
                value.as_str().is_some_and(|value| !value.is_empty())
            }
            _ => false,
        };
        if !valid(&nodes, nodes_exhausted) || !valid(&relations, relations_exhausted) {
            return Err(cursor_error(
                "invalid_request",
                "Worker search cursor state is invalid",
            ));
        }
        object(vec![
            ("schema", text(WORKER_SEARCH_CURSOR_SCHEMA_V3)),
            ("source_revision", text(revision)),
            ("snapshot_epoch", integer_u64(snapshot_epoch)),
            ("query", text(query)),
            ("filters", filters.clone()),
            ("nodes", nodes),
            ("relations", relations),
            ("nodes_exhausted", JsonValue::Bool(nodes_exhausted)),
            ("relations_exhausted", JsonValue::Bool(relations_exhausted)),
        ])
    } else {
        let kind = required_string(request, "kind")?;
        let rank = safe_count(get(request, "rank"))
            .filter(|rank| *rank < WORKER_SEARCH_RANK_CLASSES)
            .ok_or_else(|| {
                cursor_error("invalid_request", "invalid indexed knowledge search cursor")
            })?;
        let position = safe_count(get(request, "position")).ok_or_else(|| {
            cursor_error("invalid_request", "invalid indexed knowledge search cursor")
        })?;
        let id = required_string(request, "id")?;
        if !matches!(kind, "nodes" | "relations") || id.is_empty() {
            return Err(cursor_error(
                "invalid_request",
                "invalid indexed knowledge search cursor",
            ));
        }
        object(vec![
            ("schema", text(WORKER_SEARCH_CURSOR_SCHEMA_V3)),
            ("source_revision", text(revision)),
            ("snapshot_epoch", integer_u64(snapshot_epoch)),
            ("kind", text(kind)),
            ("query", text(query)),
            ("filters", filters.clone()),
            ("rank", integer_u64(rank)),
            ("id", text(id)),
            ("position", integer_u64(position)),
        ])
    };
    Ok(object(vec![("cursor_json", text(&cursor_json(&cursor)))]))
}

fn packet_metadata(
    request: &JsonValue,
    indexed: bool,
) -> Result<JsonValue, WorkerSearchControlError> {
    let query = required_string(request, "query")?;
    let filters = get(request, "filters").cloned().unwrap_or(JsonValue::Null);
    if indexed {
        let source_revision = required_string(request, "source_revision")?;
        let cursor = get(request, "cursor").cloned().unwrap_or(JsonValue::Null);
        let next_cursor = get(request, "next_cursor")
            .cloned()
            .unwrap_or(JsonValue::Null);
        let limit = safe_count(get(request, "limit"))
            .filter(|value| (1..=WORKER_SEARCH_MAX_PAGE_SIZE as u64).contains(value))
            .ok_or_else(|| invalid("limit must be between 1 and 100"))?;
        let has_cursor = get(request, "has_cursor")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| invalid("Worker search page state is invalid"))?;
        let nodes_has_more = get(request, "nodes_has_more")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| invalid("Worker search page state is invalid"))?;
        let relations_has_more = get(request, "relations_has_more")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| invalid("Worker search page state is invalid"))?;
        let nodes_count = safe_count(get(request, "nodes_count"))
            .ok_or_else(|| unavailable("knowledge search result counts are invalid"))?;
        let relations_count = safe_count(get(request, "relations_count"))
            .ok_or_else(|| unavailable("knowledge search result counts are invalid"))?;
        let matching_nodes = if !has_cursor && !nodes_has_more {
            integer_u64(nodes_count)
        } else {
            JsonValue::Null
        };
        let matching_relations = if !has_cursor && !relations_has_more {
            integer_u64(relations_count)
        } else {
            JsonValue::Null
        };
        let has_more = !matches!(&next_cursor, &JsonValue::Null);
        let returned_nodes = safe_count(get(request, "returned_nodes"))
            .ok_or_else(|| unavailable("knowledge search result counts are invalid"))?;
        let returned_relations = safe_count(get(request, "returned_relations"))
            .ok_or_else(|| unavailable("knowledge search result counts are invalid"))?;
        let node_work = get(request, "node_work")
            .cloned()
            .ok_or_else(|| invalid("Worker search page work is invalid"))?;
        let relation_work = get(request, "relation_work")
            .cloned()
            .ok_or_else(|| invalid("Worker search page work is invalid"))?;
        Ok(object(vec![
            ("schema", text(WORKER_SEARCH_INDEXED_SCHEMA_V2)),
            ("source_revision", text(source_revision)),
            ("query", text(query)),
            ("filters", filters),
            (
                "page",
                object(vec![
                    ("cursor", cursor),
                    ("next_cursor", next_cursor),
                    ("limit_per_kind", integer_u64(limit)),
                    ("ordering_scope", text("global-rank")),
                    ("has_more", JsonValue::Bool(has_more)),
                ]),
            ),
            (
                "counts",
                object(vec![
                    ("matching_nodes", matching_nodes),
                    ("matching_relations", matching_relations),
                    ("returned_nodes", integer_u64(returned_nodes)),
                    ("returned_relations", integer_u64(returned_relations)),
                    (
                        "scope",
                        text("exact-if-kind-exhausted-without-continuation"),
                    ),
                ]),
            ),
            ("nodes", JsonValue::Null),
            ("relations", JsonValue::Null),
            ("authority_boundary", JsonValue::Null),
            (
                "work",
                object(vec![("nodes", node_work), ("relations", relation_work)]),
            ),
        ]))
    } else {
        let offset = safe_count(get(request, "offset"))
            .filter(|value| *value <= WORKER_SEARCH_MAX_OFFSET as u64)
            .ok_or_else(|| invalid("offset must be between 0 and 100000"))?;
        let limit = safe_count(get(request, "limit"))
            .filter(|value| (1..=WORKER_SEARCH_MAX_PAGE_SIZE as u64).contains(value))
            .ok_or_else(|| invalid("limit must be between 1 and 100"))?;
        let mut fields = vec![
            ("schema", text(WORKER_SEARCH_LEGACY_SCHEMA_V1)),
            ("source_revision", JsonValue::Null),
            ("query", text(query)),
            ("filters", filters),
            (
                "page",
                object(vec![
                    ("offset", integer_u64(offset)),
                    ("limit_per_kind", integer_u64(limit)),
                ]),
            ),
        ];
        let mut counts = packet_counts(
            request,
            &[
                "matching_nodes",
                "matching_relations",
                "returned_nodes",
                "returned_relations",
            ],
        )?;
        fields.push(("counts", object(std::mem::take(&mut counts))));
        fields.push(("nodes", JsonValue::Null));
        fields.push(("relations", JsonValue::Null));
        fields.push(("authority_boundary", JsonValue::Null));
        Ok(object(fields))
    }
}

fn indexed_work(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let phase = required_string(request, "phase")?;
    let grams = safe_count(get(request, "grams"))
        .ok_or_else(|| invalid("Worker search gram count is invalid"))?;
    let selections = safe_count(get(request, "selections"))
        .ok_or_else(|| invalid("Worker search posting selection count is invalid"))?;
    let candidate_rows = safe_count(get(request, "candidate_rows"));
    let verified_chars = safe_count(get(request, "verified_chars"));
    let rank_chars = safe_count(get(request, "rank_chars"));
    let preflight = safe_count(get(request, "preflight_rows_read"));
    let window = safe_count(get(request, "window_rows_read"));
    let result = safe_count(get(request, "result_rows_read"));
    let (candidate_rows, verified_chars, rank_chars, sql_pages, selection_rows_read) = match phase {
        "zero_postings" => (0, 0, None, grams.saturating_add(1), None),
        "preflight_empty" => (
            0,
            0,
            rank_chars,
            grams.saturating_add(selections).saturating_add(1),
            preflight,
        ),
        "window_empty" => (
            0,
            0,
            rank_chars,
            grams.saturating_add(selections).saturating_add(2),
            preflight
                .zip(window)
                .map(|(left, right)| left.saturating_add(right)),
        ),
        "page" => (
            candidate_rows.ok_or_else(|| invalid("Worker search candidate count is invalid"))?,
            verified_chars
                .ok_or_else(|| invalid("Worker search verified character count is invalid"))?,
            rank_chars,
            grams
                .saturating_add(selections)
                .saturating_add(2)
                .saturating_add(
                    if get(request, "has_window")
                        .and_then(JsonValue::as_bool)
                        .unwrap_or(false)
                    {
                        1
                    } else {
                        0
                    },
                ),
            preflight
                .zip(result)
                .zip(window)
                .map(|((left, middle), right)| left.saturating_add(middle).saturating_add(right)),
        ),
        _ => return Err(invalid("Worker search work phase is invalid")),
    };
    if sql_pages > MAX_SAFE_INTEGER
        || selection_rows_read.is_some_and(|value| value > MAX_SAFE_INTEGER)
    {
        return Err(unavailable("Worker search work counters are invalid"));
    }
    let mut fields = vec![
        ("candidate_rows", integer_u64(candidate_rows)),
        ("verified_chars", integer_u64(verified_chars)),
    ];
    if let Some(rank_chars) = rank_chars {
        fields.push(("rank_chars", integer_u64(rank_chars)));
    }
    fields.push(("sql_pages", integer_u64(sql_pages)));
    if let Some(selection_rows_read) = selection_rows_read {
        fields.push(("selection_rows_read", integer_u64(selection_rows_read)));
    }
    Ok(object(fields))
}

fn validate_selected(
    request: &JsonValue,
    ranked: bool,
) -> Result<JsonValue, WorkerSearchControlError> {
    let window = get(request, "context").and_then(JsonValue::as_str) == Some("window");
    let message = if window {
        "indexed knowledge search verification window is invalid"
    } else if ranked {
        "indexed knowledge search selected rank carrier is invalid"
    } else {
        "knowledge search selected rank carrier is invalid"
    };
    if safe_count(get(request, "position")).is_none()
        || (ranked
            && safe_count(get(request, "search_rank"))
                .is_none_or(|rank| rank >= WORKER_SEARCH_RANK_CLASSES))
    {
        return Err(unavailable(message));
    }
    let (Some(id), Some(id_lower)) = (
        get(request, "id").and_then(JsonValue::as_str),
        get(request, "id_lower").and_then(JsonValue::as_str),
    ) else {
        return Err(unavailable(message));
    };
    let matches =
        worker_search_identity_matches_lower(id, id_lower).map_err(|_| unavailable(message))?;
    if !matches {
        return Err(unavailable(message));
    }
    Ok(object(vec![("valid", JsonValue::Bool(true))]))
}

fn indexed_page(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let row_count = safe_count(get(request, "result_rows"))
        .ok_or_else(|| unavailable("indexed knowledge search selected rank carrier is invalid"))?;
    let limit = safe_count(get(request, "limit"))
        .filter(|value| (1..=WORKER_SEARCH_MAX_PAGE_SIZE as u64).contains(value))
        .ok_or_else(|| invalid("limit must be between 1 and 100"))?;
    let window_has_more = get(request, "window_has_more")
        .and_then(JsonValue::as_bool)
        .ok_or_else(|| invalid("Worker search window state is invalid"))?;
    let more_matches = row_count > limit;
    let has_more = more_matches || window_has_more;
    let last_source = if more_matches {
        "result"
    } else if window_has_more {
        "window"
    } else {
        "none"
    };
    Ok(object(vec![
        ("returned_rows", integer_u64(row_count.min(limit))),
        ("more_matches", JsonValue::Bool(more_matches)),
        ("has_more", JsonValue::Bool(has_more)),
        ("last_source", text(last_source)),
    ]))
}

fn validate_legacy_closure(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let limit = safe_count(get(request, "limit"));
    let offset = safe_count(get(request, "offset"));
    let count = safe_count(get(request, "matching_rows"));
    let actual = safe_count(get(request, "returned_rows"));
    let (Some(limit), Some(offset), Some(count), Some(actual)) = (limit, offset, count, actual)
    else {
        return Err(unavailable(
            "knowledge search selected rank closure is incomplete",
        ));
    };
    if actual != limit.min(count.saturating_sub(offset)) {
        return Err(unavailable(
            "knowledge search selected rank closure is incomplete",
        ));
    }
    Ok(object(vec![("valid", JsonValue::Bool(true))]))
}
fn filter_equal(value: Option<&JsonValue>, expected: Option<&JsonValue>) -> bool {
    let (Some(value), Some(expected)) = (value, expected) else {
        return false;
    };
    if !exact_keys(value, &["sources", "kind_ids", "predicate_ids"])
        || !exact_keys(expected, &["sources", "kind_ids", "predicate_ids"])
    {
        return false;
    }
    ["sources", "kind_ids", "predicate_ids"]
        .iter()
        .all(|key| get(value, key) == get(expected, key))
}
fn cursor_error(code: &'static str, message: &'static str) -> WorkerSearchControlError {
    WorkerSearchControlError {
        code,
        message: message.to_owned(),
    }
}

fn validate_cursor(
    request: &JsonValue,
    outer: bool,
) -> Result<JsonValue, WorkerSearchControlError> {
    let raw = required_string(request, "raw")?;
    let cursor = parse(raw.as_bytes(), MAX_CURSOR_BYTES)?;
    if cursor.as_object().is_none() {
        return Err(cursor_error(
            "cursor_invalid",
            "invalid indexed knowledge search cursor",
        ));
    }
    if same_string(
        get(&cursor, "schema"),
        "tos_knowledge_search_indexed_cursor_v2",
    ) {
        return Err(cursor_error(
            "cursor_stale",
            "indexed knowledge search cursor predates native search; restart the query",
        ));
    }
    if !same_string(
        get(&cursor, "schema"),
        "tos_knowledge_search_indexed_cursor_v3",
    ) {
        return Err(cursor_error(
            "cursor_invalid",
            "invalid indexed knowledge search cursor",
        ));
    }
    let expected_keys: &[&str] = if outer {
        &[
            "filters",
            "nodes",
            "nodes_exhausted",
            "query",
            "relations",
            "relations_exhausted",
            "schema",
            "snapshot_epoch",
            "source_revision",
        ]
    } else {
        &[
            "filters",
            "id",
            "kind",
            "position",
            "query",
            "rank",
            "schema",
            "snapshot_epoch",
            "source_revision",
        ]
    };
    if !exact_keys(&cursor, expected_keys) {
        return Err(cursor_error(
            "cursor_invalid",
            "invalid indexed knowledge search cursor",
        ));
    }
    let epoch = safe_int(get(&cursor, "snapshot_epoch"));
    if epoch.is_none() {
        return Err(cursor_error(
            "cursor_invalid",
            "invalid indexed knowledge search cursor",
        ));
    }
    let expected_epoch = safe_int(get(request, "snapshot_epoch"));
    let revision = required_string(request, "source_revision")?;
    let query = required_string(request, "query")?;
    if epoch != expected_epoch
        || !same_string(get(&cursor, "source_revision"), revision)
        || !same_string(get(&cursor, "query"), query)
        || (!outer && !same_string(get(&cursor, "kind"), required_string(request, "kind")?))
        || !filter_equal(get(&cursor, "filters"), get(request, "filters"))
    {
        return Err(cursor_error(
            "cursor_stale",
            "indexed knowledge search cursor does not match the current snapshot/query",
        ));
    }
    let Some(expected_epoch) = expected_epoch else {
        return Err(cursor_error(
            "invalid_request",
            "Worker search snapshot epoch is invalid",
        ));
    };
    if epoch != Some(expected_epoch) {
        return Err(cursor_error(
            "cursor_stale",
            "indexed knowledge search cursor does not match the current snapshot/query",
        ));
    }
    if outer {
        let nodes_exhausted = get(&cursor, "nodes_exhausted").and_then(JsonValue::as_bool);
        let relations_exhausted = get(&cursor, "relations_exhausted").and_then(JsonValue::as_bool);
        let nodes = get(&cursor, "nodes");
        let relations = get(&cursor, "relations");
        let node_valid = match (nodes_exhausted, nodes) {
            (Some(true), Some(JsonValue::Null)) => true,
            (Some(false), Some(JsonValue::String(value))) => {
                value.as_str().is_some_and(|value| !value.is_empty())
            }
            _ => false,
        };
        let relation_valid = match (relations_exhausted, relations) {
            (Some(true), Some(JsonValue::Null)) => true,
            (Some(false), Some(JsonValue::String(value))) => {
                value.as_str().is_some_and(|value| !value.is_empty())
            }
            _ => false,
        };
        if !node_valid || !relation_valid {
            return Err(cursor_error(
                "cursor_invalid",
                "invalid indexed knowledge search cursor",
            ));
        }
        return Ok(object(vec![
            (
                "nodes_exhausted",
                JsonValue::Bool(nodes_exhausted.unwrap_or(false)),
            ),
            (
                "relations_exhausted",
                JsonValue::Bool(relations_exhausted.unwrap_or(false)),
            ),
            ("nodes", nodes.cloned().unwrap_or(JsonValue::Null)),
            ("relations", relations.cloned().unwrap_or(JsonValue::Null)),
        ]));
    }
    let kind = required_string(request, "kind")?;
    let rank = safe_int(get(&cursor, "rank"));
    let position = safe_int(get(&cursor, "position"));
    let id = get(&cursor, "id").and_then(JsonValue::as_str);
    if !matches!(kind, "nodes" | "relations")
        || rank.is_none_or(|rank| rank >= WORKER_SEARCH_RANK_CLASSES)
        || position.is_none()
        || id.is_none_or(str::is_empty)
    {
        return Err(cursor_error(
            "cursor_invalid",
            "invalid indexed knowledge search cursor",
        ));
    }
    let id = id.unwrap_or_default();
    if tos_query::worker_search_controls::worker_search_lower_id(id).map_err(search_error)? != id {
        return Err(cursor_error(
            "cursor_invalid",
            "invalid indexed knowledge search cursor",
        ));
    }
    Ok(object(vec![
        ("rank", integer_u64(rank.unwrap_or(0))),
        ("id", text(id)),
        ("position", integer_u64(position.unwrap_or(0))),
    ]))
}

fn run(request: &JsonValue) -> Result<JsonValue, WorkerSearchControlError> {
    let operation = required_string(request, "operation")?;
    match operation {
        "policy" => Ok(policy()),
        "normalize" => {
            let mode = required_string(request, "mode")?;
            let kind = match mode {
                "legacy" => WorkerSearchKind::Legacy,
                "indexed" => WorkerSearchKind::Indexed,
                _ => {
                    return Err(WorkerSearchControlError {
                        code: "invalid_request",
                        message: "knowledge search mode must be legacy or indexed".to_owned(),
                    });
                }
            };
            let query = required_string(request, "query")?;
            let sources = optional_strings(request, "sources")?;
            let kind_ids = required_strings(request, "kind_ids")?;
            let predicate_ids = required_strings(request, "predicate_ids")?;
            let offset = required_usize(request, "offset")?;
            let limit = required_usize(request, "limit")?;
            normalize_worker_search(WorkerSearchInput {
                kind,
                query,
                sources: sources.as_deref(),
                kind_ids: &kind_ids,
                predicate_ids: &predicate_ids,
                offset,
                limit,
            })
            .map(normalized)
        }
        "grams" => {
            let query = required_string(request, "query")?;
            let grams = unique_search_grams(query);
            Ok(object(vec![("grams", strings(&grams))]))
        }
        "gram_plan" => gram_plan(request),
        "posting_closure" => validate_posting_closure(request),
        "preflight" => preflight(request),
        "window" => window(request),
        "rank_sql" => {
            let alias = required_string(request, "alias")?;
            let has_needle = get(request, "has_needle")
                .and_then(JsonValue::as_bool)
                .unwrap_or(true);
            let (sql, binding_count) = worker_rank_sql(alias, has_needle).map_err(search_error)?;
            let continuation = format!(
                "({sql} > ? OR ({sql} = ? AND ({alias}.id_lower > ? OR ({alias}.id_lower = ? AND {alias}.position > ?))))"
            );
            let window_bound = format!(
                "({sql}<? OR ({sql}=? AND ({alias}.id_lower<? OR ({alias}.id_lower=? AND {alias}.position<=?))))"
            );
            Ok(object(vec![
                ("sql", text(&sql)),
                ("binding_count", integer(binding_count)),
                (
                    "order_by",
                    text(&format!("search_rank, {alias}.id_lower, {alias}.position")),
                ),
                (
                    "candidate_order_by",
                    text("search_rank, c.id_lower, c.position"),
                ),
                ("window_order_by", text("search_rank, id_lower, position")),
                (
                    "window_reverse_order_by",
                    text("search_rank DESC, id_lower DESC, position DESC"),
                ),
                ("continuation", text(&continuation)),
                ("window_bound", text(&window_bound)),
                ("rank_classes", integer(WORKER_SEARCH_RANK_CLASSES as usize)),
            ]))
        }
        "encode_cursor_kind" => encode_cursor(request, false),
        "encode_cursor_outer" => encode_cursor(request, true),
        "legacy_packet_metadata" => packet_metadata(request, false),
        "indexed_packet_metadata" => packet_metadata(request, true),
        "indexed_work" => indexed_work(request),
        "indexed_page" => indexed_page(request),
        "validate_legacy_closure" => validate_legacy_closure(request),
        "empty_page_work" => Ok(object(vec![
            ("candidate_rows", integer(0)),
            ("verified_chars", integer(0)),
            ("sql_pages", integer(0)),
        ])),
        "validate_selected" => validate_selected(request, true),
        "validate_legacy_selected" => validate_selected(request, false),
        "cursor_outer" => validate_cursor(request, true),
        "cursor_kind" => validate_cursor(request, false),
        _ => Err(WorkerSearchControlError {
            code: "invalid_request",
            message: "unknown Worker search control operation".to_owned(),
        }),
    }
}

/// Single generated binding. Errors are data so the host can preserve its
/// established 400/409/413 HTTP boundary without exposing Rust exceptions.
pub fn worker_knowledge_search_controls_wasm_v1(raw_request: &[u8]) -> Vec<u8> {
    let response = match parse(raw_request, 1_048_576).and_then(|request| run(&request)) {
        Ok(value) => ok(value),
        Err(error) => failed(error),
    };
    emit(&response)
}
