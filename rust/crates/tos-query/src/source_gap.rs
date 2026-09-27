//! Maintained public source-gap projection over exact released ledger members.
//! The release owner supplies the complete declared subset and retains its
//! current hold through flush. This kernel grants no arbitrary source access.
use crate::knowledge_lens_spec::{py_string, truthy};
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use crate::source_read_projection::{object, text};
use crate::{AbortProbe, AbortReason};
use std::collections::BTreeSet;
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonValue, RelativePath,
    canonical_bytes_v1, parse_json, python_casefold_unicode16_v1, python_lower_unicode16_v1,
    python_strip_unicode16_v1,
};

pub const SOURCE_GAP_OPERATION: &str = "tos.source-gaps.search";
pub const SOURCE_GAP_LEDGER_PREFIX: &str = "ToS/source-witnesses/access-requests/public-ledger/";
pub const SOURCE_GAP_RECORD_SUFFIX: &str = ".access-request.json";
// Existing maintained selection and record profile, not configurable expansion.
pub const SOURCE_GAP_MAX_RECORDS: usize = 100;
pub const SOURCE_GAP_MAX_RECORD_BYTES: usize = 256_000;

/// Borrowed exact bytes from the selected release's declared member inventory.
/// The caller verifies completeness, SHA/size and current release custody.
pub struct PublicSourceGapRecord<'a> {
    pub source_ref: &'a str,
    pub raw: &'a [u8],
}
#[derive(Clone, Debug)]
pub struct SourceGapRequest {
    pub query: String,
    pub limit: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct SourceGapBudget {
    pub json: JsonLimits,
    pub max_work_steps: u64,
    pub max_response_bytes: usize,
}
fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn budget_error() -> SearchV2Error {
    error(
        SearchV2ErrorCode::BudgetExceeded,
        "source-gap budget exceeded",
    )
}
fn corrupt() -> SearchV2Error {
    error(
        SearchV2ErrorCode::CorruptSelectedCarrier,
        "selected public source-gap record invalid",
    )
}
fn get<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn strings(v: &JsonValue) -> Vec<String> {
    v.as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(JsonValue::as_str)
        .map(str::to_owned)
        .collect()
}
fn texts(v: impl IntoIterator<Item = String>) -> JsonValue {
    JsonValue::Array(v.into_iter().map(|s| text(&s)).collect())
}
fn fallback(v: &JsonValue, default: &str) -> String {
    if truthy(v) {
        py_string(v)
    } else {
        default.to_owned()
    }
}
fn check(probe: &dyn AbortProbe) -> Result<(), SearchV2Error> {
    match probe.reason() {
        Some(AbortReason::Cancelled) => Err(error(
            SearchV2ErrorCode::Cancelled,
            "source-gap query cancelled",
        )),
        Some(AbortReason::DeadlineExceeded) => Err(error(
            SearchV2ErrorCode::DeadlineExceeded,
            "source-gap query deadline exceeded",
        )),
        None => Ok(()),
    }
}
struct Work<'a> {
    remaining: u64,
    probe: &'a dyn AbortProbe,
}
impl Work<'_> {
    fn charge(&mut self, n: usize) -> Result<(), SearchV2Error> {
        self.remaining = self
            .remaining
            .checked_sub(u64::try_from(n).map_err(|_| budget_error())?)
            .ok_or_else(budget_error)?;
        check(self.probe)
    }
    fn fold(&mut self, s: &str, casefold: bool) -> Result<String, SearchV2Error> {
        let chars = s.chars().count();
        self.charge(chars)?;
        let out_chars = chars.checked_mul(3).ok_or_else(budget_error)?;
        let out_bytes = s.len().checked_mul(3).ok_or_else(budget_error)?;
        let result = if casefold {
            python_casefold_unicode16_v1(s, chars, out_chars, out_bytes)
        } else {
            python_lower_unicode16_v1(s, chars, out_chars, out_bytes)
        }
        .map_err(|_| budget_error())?;
        check(self.probe)?;
        Ok(result)
    }
    // The maintained _contains recurses over values only and lowers strings;
    // it does not casefold haystacks or match JSON keys/numbers/serialization.
    fn contains(&mut self, v: &JsonValue, needle: &str) -> Result<bool, SearchV2Error> {
        self.charge(1)?;
        if let Some(s) = v.as_str() {
            return Ok(self.fold(s, false)?.contains(needle));
        }
        if let Some(a) = v.as_array() {
            for v in a {
                if self.contains(v, needle)? {
                    return Ok(true);
                }
            }
        }
        if let Some(o) = v.as_object() {
            for (_, v) in o {
                if self.contains(v, needle)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

/// Compute the entire maintained packet. Release selection/hold stays with
/// the existing consumer owner; missing members must never become an empty set.
pub fn compute_source_gap_packet(
    records: &[PublicSourceGapRecord<'_>],
    request: &SourceGapRequest,
    budget: SourceGapBudget,
    probe: &dyn AbortProbe,
) -> Result<Vec<u8>, SearchV2Error> {
    check(probe)?;
    if budget.max_work_steps == 0 || budget.max_response_bytes == 0 {
        return Err(budget_error());
    }
    let mut work = Work {
        remaining: budget.max_work_steps,
        probe,
    };
    work.charge(request.query.len())?;
    let query = python_strip_unicode16_v1(&request.query, request.query.chars().count())
        .map_err(|_| budget_error())?;
    if query.chars().count() > 256 || !(1..=100).contains(&request.limit) {
        return Err(error(
            SearchV2ErrorCode::InvalidRequest,
            "invalid source-gap request",
        ));
    }
    let needle = work.fold(query, true)?;
    let mut ranked = Vec::new();
    let mut prior = None;
    for selected in records.iter().take(SOURCE_GAP_MAX_RECORDS) {
        work.charge(
            selected
                .raw
                .len()
                .checked_add(selected.source_ref.len())
                .ok_or_else(budget_error)?,
        )?;
        let path = selected.source_ref;
        let filename = path
            .strip_prefix(SOURCE_GAP_LEDGER_PREFIX)
            .ok_or_else(corrupt)?;
        if RelativePath::parse(path).is_err()
            || filename.contains('/')
            || !filename.ends_with(SOURCE_GAP_RECORD_SUFFIX)
            || prior.is_some_and(|p| p >= path)
            || selected.raw.len() > SOURCE_GAP_MAX_RECORD_BYTES
        {
            return Err(corrupt());
        }
        prior = Some(path);
        let mut limits = budget.json;
        limits.max_bytes = limits.max_bytes.min(SOURCE_GAP_MAX_RECORD_BYTES);
        let record = parse_json(selected.raw, JsonMode::PublishedStrict, limits)
            .map_err(|reason| {
                if reason.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                    budget_error()
                } else {
                    corrupt()
                }
            })?
            .into_root();
        if record.as_object().is_none()
            || get(&record, "schema_version").as_str() != Some("tos_access_request_v1")
            || get(&record, "personal_or_confidential_data_committed") == &JsonValue::Bool(true)
        {
            return Err(corrupt());
        }
        if !needle.is_empty() && !work.contains(&record, &needle)? {
            continue;
        }
        let material = get(&record, "material");
        let response = get(&record, "response");
        // Path.stem removes only .json, preserving the maintained ID fallback.
        let request_id = fallback(
            get(&record, "request_id"),
            filename.strip_suffix(".json").ok_or_else(corrupt)?,
        );
        let title = fallback(get(material, "title"), &request_id);
        let tos_refs = strings(get(material, "tos_refs"));
        let mut refs = vec![path.to_owned()];
        refs.extend(strings(get(material, "discovery_refs")));
        refs.extend(strings(get(&record, "rights_record_refs")));
        refs.extend(strings(get(response, "safe_evidence_refs")));
        let mut seen = BTreeSet::new();
        refs.retain(|r| seen.insert(r.clone()));
        let from_id = tos_refs
            .first()
            .map(String::as_str)
            .unwrap_or("tos.subject.friedrich-nietzsche");
        let from_label = fallback(
            get(material, "edition_or_resource"),
            tos_refs
                .first()
                .map(String::as_str)
                .unwrap_or("Tree of Sophia"),
        );
        let access = fallback(get(&record, "access_status"), "unknown");
        let status = fallback(get(&record, "request_status"), "unknown");
        let state = fallback(get(response, "state"), "none");
        let title_key = work.fold(&title, true)?;
        let identity = work.fold(
            &format!(
                "{} {}",
                request_id,
                fallback(get(material, "responsibility"), "")
            ),
            true,
        )?;
        let score = if !needle.is_empty() {
            8 * u8::from(title_key.contains(&needle)) + 4 * u8::from(identity.contains(&needle))
        } else {
            0
        };
        let packet = object(vec![
            (
                "edge_id",
                text(&format!("cluster-relation:source-gap:{request_id}")),
            ),
            ("from_id", text(from_id)),
            ("to_id", text(&request_id)),
            ("from_label", text(&from_label)),
            ("to_label", text(&title)),
            ("predicate_id", text("source_access_gap")),
            ("source_refs", texts(refs)),
            ("access_status", text(&access)),
            ("request_status", text(&status)),
            ("response_state", text(&state)),
            (
                "request_sent",
                JsonValue::Bool(truthy(get(&record, "sent_at"))),
            ),
            ("authority_posture", text("source_witness_public_ledger")),
            ("review_posture", text(&status)),
            ("canon_status", text("not_applicable")),
            ("confidence", text("recorded_status")),
            (
                "properties",
                object(vec![
                    (
                        "public_summary_en",
                        text(&format!(
                            "ToS records {title} as {access}; request status is {status} and response state is {state}."
                        )),
                    ),
                    (
                        "research_purpose",
                        text(&fallback(get(&record, "research_purpose"), "")),
                    ),
                ]),
            ),
        ]);
        ranked.push((score, title_key, packet));
    }
    // Stable sort keeps path encounter order for equal score and casefold title.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let gaps = ranked
        .into_iter()
        .take(request.limit)
        .map(|(_, _, v)| v)
        .collect::<Vec<_>>();
    let result = object(vec![
        ("schema", text("tos_source_gap_search_v1")),
        ("query", text(query)),
        (
            "result_count",
            JsonValue::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme: gaps.len().to_string(),
            }),
        ),
        ("gaps", JsonValue::Array(gaps)),
        (
            "authority_note",
            text(
                "These are recorded source-access gaps in a bounded public runtime set; this is not a corpus-completeness or legal conclusion. No request is sent and no source or canon is changed.",
            ),
        ),
    ]);
    check(probe)?;
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_response_bytes);
    let body = canonical_bytes_v1(&result, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(|_| budget_error())?;
    check(probe)?;
    Ok(body)
}
