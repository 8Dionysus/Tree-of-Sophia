//! Bounded Worker D1 adapter for the shared QRY source-value search rules.
//! The host supplies exact ordered candidates; Rust owns normalization,
//! recursive value matching, filtering, selection order, and response framing.

use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    canonical_bytes_v1, emit_value_preserved_json, parse_json,
};
use tos_query::{
    search_v2::{SearchV2Error, SearchV2ErrorCode},
    source_value_search,
};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

const MAX_CANDIDATE_BYTES: usize = 2_000_000;
const MAX_QUERY_BYTES: usize = 1_048_576;
const MAX_CANDIDATES: usize = 100_000;
const MAX_TOTAL_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_WORK_STEPS: u64 = 100_000_000;

#[derive(Clone, Copy)]
enum SearchMode {
    Corpus,
    Philosophy,
}

fn error(message: &str) -> JsValue {
    JsValue::from_str(message)
}

fn search_error(error: SearchV2Error) -> JsValue {
    JsValue::from_str(&format!("{:?}: {}", error.code, error.message))
}

fn query_budget_error() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "source-value search work budget exceeded",
    }
}

fn invalid(message: &'static str) -> JsValue {
    error(&format!("InvalidRequest: {message}"))
}

fn budget(message: &'static str) -> JsValue {
    error(&format!("BudgetExceeded: {message}"))
}

fn corrupt(message: &'static str) -> JsValue {
    error(&format!("CorruptSelectedCarrier: {message}"))
}

fn json_limits(max_bytes: usize) -> JsonLimits {
    // Match the source publisher's selected D1 row profile.
    JsonLimits::new(max_bytes, 96, 1_000_000, 4096)
        .expect("constant source-value Worker JSON limits are valid")
}

fn parse_candidate(raw: &str) -> Result<JsonValue, JsValue> {
    if raw.len() > MAX_CANDIDATE_BYTES {
        return Err(budget("source-value candidate exceeds byte budget"));
    }
    parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        json_limits(MAX_CANDIDATE_BYTES),
    )
    .map(|value| value.into_root())
    .map_err(|_| corrupt("source-value candidate JSON is invalid"))
}

fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn number(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}

#[wasm_bindgen]
pub struct WorkerSourceValueSearch {
    mode: SearchMode,
    query: String,
    needle: String,
    resource_kind: Option<String>,
    limit: usize,
    remaining_work: u64,
    total_input_bytes: usize,
    candidates: usize,
    previous_order: Option<(u8, usize)>,
    results: Vec<JsonValue>,
    finished: bool,
}

#[wasm_bindgen]
impl WorkerSourceValueSearch {
    #[wasm_bindgen(constructor)]
    pub fn new(
        mode: &str,
        query: &str,
        limit: usize,
        resource_kind: Option<String>,
    ) -> Result<WorkerSourceValueSearch, JsValue> {
        let mode = match mode {
            "corpus" => SearchMode::Corpus,
            "philosophy" if resource_kind.is_none() => SearchMode::Philosophy,
            "philosophy" => return Err(invalid("philosophy search has no resource-kind filter")),
            _ => return Err(invalid("unsupported source-value search mode")),
        };
        if !(1..=100).contains(&limit) {
            return Err(invalid("source-value search limit must be 1..100"));
        }
        if query.len() > MAX_QUERY_BYTES
            || resource_kind
                .as_deref()
                .is_some_and(|value| value.len() > MAX_QUERY_BYTES)
        {
            return Err(budget("source-value search request exceeds byte budget"));
        }
        let mut remaining_work = MAX_WORK_STEPS;
        let needle = source_value_search::normalize_query(query, &mut |steps| {
            remaining_work = remaining_work
                .checked_sub(u64::try_from(steps).map_err(|_| query_budget_error())?)
                .ok_or_else(query_budget_error)?;
            Ok(())
        })
        .map_err(search_error)?;
        Ok(Self {
            mode,
            query: query.to_owned(),
            needle,
            resource_kind,
            limit,
            remaining_work,
            total_input_bytes: 0,
            candidates: 0,
            previous_order: None,
            results: Vec::with_capacity(limit),
            finished: false,
        })
    }

    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.results.len())
    }

    /// Candidate and byte budgets cover the entire ordered D1 scan. The host
    /// uses these remaining values when choosing each next page size, so a
    /// fresh SQL page cannot restart either budget.
    pub fn scan_remaining(&self) -> usize {
        MAX_CANDIDATES.saturating_sub(self.candidates)
    }

    pub fn input_bytes_remaining(&self) -> usize {
        MAX_TOTAL_INPUT_BYTES.saturating_sub(self.total_input_bytes)
    }

    pub fn collection_order(&self) -> Result<String, JsValue> {
        let mode = match self.mode {
            SearchMode::Corpus => "corpus",
            SearchMode::Philosophy => "philosophy",
        };
        let collections = source_value_search::collections(mode)
            .ok_or_else(|| invalid("source-value search mode is unavailable"))?;
        let bytes = emit_value_preserved_json(
            &JsonValue::Array(collections.iter().map(|value| string(value)).collect()),
            json_limits(1024),
        )
        .map_err(|_| corrupt("source-value collection order is invalid"))?;
        String::from_utf8(bytes).map_err(|_| corrupt("source-value collection order is invalid"))
    }

    /// Observe one exact selected D1 row. `search_json` is the full source
    /// value used by QRY matching; `result_json` is the existing public row.
    pub fn observe_candidate(
        &mut self,
        collection: &str,
        ordinal: usize,
        search_json: &str,
        result_json: &str,
    ) -> Result<bool, JsValue> {
        if self.finished || self.remaining() == 0 {
            return Err(invalid("source-value search is complete"));
        }
        self.candidates = self
            .candidates
            .checked_add(1)
            .filter(|count| *count <= MAX_CANDIDATES)
            .ok_or_else(|| budget("source-value candidate-count budget exceeded"))?;
        if search_json.len() > MAX_CANDIDATE_BYTES || result_json.len() > MAX_CANDIDATE_BYTES {
            return Err(budget("source-value candidate exceeds byte budget"));
        }
        let bytes = if search_json == result_json {
            search_json.len()
        } else {
            search_json
                .len()
                .checked_add(result_json.len())
                .ok_or_else(|| budget("source-value input-byte budget exceeded"))?
        };
        self.total_input_bytes = self
            .total_input_bytes
            .checked_add(bytes)
            .filter(|total| *total <= MAX_TOTAL_INPUT_BYTES)
            .ok_or_else(|| budget("source-value input-byte budget exceeded"))?;
        let rank = match self.mode {
            SearchMode::Corpus => source_value_search::collection_rank("corpus", collection),
            SearchMode::Philosophy => {
                source_value_search::collection_rank("philosophy", collection)
            }
        }
        .ok_or_else(|| corrupt("source-value candidate collection is unsupported"))?;
        if self
            .previous_order
            .is_some_and(|(previous_rank, previous_ordinal)| {
                rank < previous_rank || (rank == previous_rank && ordinal <= previous_ordinal)
            })
        {
            return Err(corrupt("source-value candidate order differs"));
        }
        self.previous_order = Some((rank, ordinal));
        self.remaining_work = self
            .remaining_work
            .checked_sub(1)
            .ok_or_else(|| budget("source-value search work budget exceeded"))?;

        let search_value = parse_candidate(search_json)?;
        if search_value.as_object().is_none() {
            return Err(corrupt("source-value search candidate is not an object"));
        }
        if matches!(self.mode, SearchMode::Corpus)
            && self
                .resource_kind
                .as_deref()
                .filter(|value| !value.is_empty())
                .is_some_and(|expected| {
                    search_value
                        .object_get("resource_kind")
                        .and_then(JsonValue::as_str)
                        != Some(expected)
                })
        {
            return Ok(false);
        }
        let matches = if self.needle.is_empty() {
            true
        } else {
            let mut remaining_work = self.remaining_work;
            let result =
                source_value_search::contains_values(&search_value, &self.needle, &mut |steps| {
                    remaining_work = remaining_work
                        .checked_sub(u64::try_from(steps).map_err(|_| query_budget_error())?)
                        .ok_or_else(query_budget_error)?;
                    Ok(())
                });
            self.remaining_work = remaining_work;
            result.map_err(search_error)?
        };
        if !matches {
            return Ok(false);
        }
        let result_value = if search_json == result_json {
            search_value
        } else {
            parse_candidate(result_json)?
        };
        if result_value.as_object().is_none() {
            return Err(corrupt("source-value result candidate is not an object"));
        }
        self.results.push(object(vec![
            ("collection", string(collection)),
            ("item", result_value),
        ]));
        Ok(true)
    }

    pub fn finish(&mut self) -> Result<Vec<u8>, JsValue> {
        if self.finished {
            return Err(invalid("source-value search session is already complete"));
        }
        self.finished = true;
        let result_count = self.results.len();
        let results = JsonValue::Array(std::mem::take(&mut self.results));
        let packet = match self.mode {
            SearchMode::Corpus => object(vec![
                ("schema", string("tos_corpus_mcp_search_v2")),
                ("query", string(&self.query)),
                (
                    "resource_kind",
                    self.resource_kind
                        .as_deref()
                        .map(string)
                        .unwrap_or(JsonValue::Null),
                ),
                ("result_count", number(result_count)),
                ("results", results),
                (
                    "authority_note",
                    string(
                        "Tree-of-Sophia owns corpus meaning; this MCP packet is an abyss-stack access-plane view.",
                    ),
                ),
            ]),
            SearchMode::Philosophy => object(vec![
                ("schema", string("tos_philosophy_mcp_search_v2")),
                ("query", string(&self.query)),
                ("result_count", number(result_count)),
                ("results", results),
                (
                    "authority_note",
                    string(
                        "Tree-of-Sophia owns philosophy meaning; this MCP search result is an access-plane packet.",
                    ),
                ),
            ]),
        };
        canonical_bytes_v1(
            &packet,
            CanonicalProfile::SourceRecordDigestV1,
            json_limits(MAX_RESPONSE_BYTES),
        )
        .map_err(|_| budget("source-value response exceeds byte budget"))
    }
}
