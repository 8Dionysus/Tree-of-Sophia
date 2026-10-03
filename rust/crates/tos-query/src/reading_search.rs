//! Source-only, occurrence-bound reading consumer over explicitly selected
//! local source and analysis datasets. Roots select data only; the installed
//! software holder selects schema and semantic-adapter provenance bytes.
//! No publication, review, translation, graph or canon authority is granted.
mod concept;
mod concept_query;
mod enrich;
mod local;
pub use concept_query::{ConceptSearchRequest, execute_concept_search};
mod word_analysis;
pub use word_analysis::{
    WordAnalysisRequest, execute_word_analysis_task, parse_word_analysis_rank,
    validate_word_analysis_candidate,
};

use crate::{AbortProbe, AbortReason};
use local::{Reader, array, budget_error, corrupt, hash, string};
pub use local::{ReadingSearchCharge, ReadingSearchResult};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tos_foundation::JsonLimits;

pub const READING_SEARCH_OPERATION: &str = "tos_zarathustra_reading_search";
pub const READING_MANIFEST_REF: &str =
    "ToS/candidate-intake/zarathustra/reading-workbench-v1/manifest.v1.json";
pub const READING_SCHEMA_REF: &str =
    "ToS/candidate-intake/zarathustra/reading-workbench-v1/reading-search-result.v1.schema.json";
pub const CONCEPT_SCHEMA_REF: &str =
    "ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-search-result.v1.schema.json";
pub const REQUEST_SCHEMA_REF: &str =
    "ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-request.v2.schema.json";
pub const DEFAULT_REQUEST_REF: &str =
    "ToS/candidate-intake/zarathustra/concept-workbench-v1/requests/fate.concept-request.v2.json";
pub const READING_PROVIDER_REF: &str = "rust/crates/tos-query/src/reading_search.rs";
const READING_ADAPTER_REF: &str = READING_PROVIDER_REF;
const CONCEPT_ADAPTER_REF: &str = "rust/crates/tos-query/src/reading_search/concept.rs";

#[derive(Clone, Debug)]
pub struct ExplicitReadingRoots {
    pub source_root: PathBuf,
    pub analysis_root: PathBuf,
}
/// Exact software contracts/provider bytes embedded by the protected product.
/// This has no runtime path and cannot be selected by data or ambient cwd.
#[derive(Clone, Debug)]
pub struct ReadingSoftware {
    _embedded: (),
}
impl ReadingSoftware {
    pub const fn embedded() -> Self {
        Self { _embedded: () }
    }
}
#[derive(Clone, Debug)]
pub struct ReadingSearchRequest {
    pub query: String,
    pub language: String,
    pub limit: usize,
    pub include_semantic_neighbors: bool,
    pub group_by: Vec<String>,
    /// Root-relative request data member; absolute paths are deliberately absent.
    pub request_ref: Option<String>,
}
#[derive(Clone, Copy, Debug)]
pub struct ReadingSearchBudget {
    pub json: JsonLimits,
    pub max_open_files: usize,
    pub max_file_bytes: u64,
    pub max_total_file_bytes: u64,
    pub max_sql_vm_steps: u64,
    pub max_sql_rows: u64,
    pub max_sql_decoded_bytes: u64,
    pub max_materialized_json_bytes: u64,
    pub max_work_steps: u64,
    pub max_response_bytes: usize,
}

impl ReadingSearchBudget {
    /// Finite installed local profile. A 512-byte adapter wrapper reserve is
    /// already excluded from the 1 MiB result ceiling. Larger data/ref cohorts
    /// require an explicit owner-reviewed budget, never prefix counts.
    pub fn local_default() -> Self {
        Self {
            json: JsonLimits {
                max_bytes: 8 * 1024 * 1024,
                max_depth: 64,
                max_visits: 1_000_000,
                max_integer_digits: 4300,
            },
            max_open_files: 256,
            max_file_bytes: 128 * 1024 * 1024,
            max_total_file_bytes: 256 * 1024 * 1024,
            max_sql_vm_steps: 20_000_000,
            max_sql_rows: 250_000,
            max_sql_decoded_bytes: 64 * 1024 * 1024,
            max_materialized_json_bytes: 128 * 1024 * 1024,
            max_work_steps: 512 * 1024 * 1024,
            max_response_bytes: 1024 * 1024 - 512,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadingSearchErrorCode {
    InvalidRequest,
    Unavailable,
    CorruptSelectedCarrier,
    BudgetExceeded,
    Cancelled,
    DeadlineExceeded,
    Unsupported,
}
#[derive(Clone, Debug)]
pub struct ReadingSearchError {
    pub code: ReadingSearchErrorCode,
    pub message: &'static str,
}
impl std::fmt::Display for ReadingSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for ReadingSearchError {}
type Result<T> = std::result::Result<T, ReadingSearchError>;
fn error(code: ReadingSearchErrorCode, message: &'static str) -> ReadingSearchError {
    ReadingSearchError { code, message }
}
fn check(probe: &dyn AbortProbe) -> Result<()> {
    match probe.reason() {
        None => Ok(()),
        Some(AbortReason::Cancelled) => Err(error(
            ReadingSearchErrorCode::Cancelled,
            "reading query cancelled",
        )),
        Some(AbortReason::DeadlineExceeded) => Err(error(
            ReadingSearchErrorCode::DeadlineExceeded,
            "reading query deadline exceeded",
        )),
    }
}

/// The maintained core's normalization before provider invocation. Adapter
/// framing/coercion remains adapter-owned; accepted typed values normalize here.
pub fn normalize_reading_request(request: &ReadingSearchRequest) -> Result<ReadingSearchRequest> {
    fn strip(s: &str) -> Result<&str> {
        tos_foundation::python_strip_unicode16_v1(s, s.chars().count()).map_err(|_| budget_error())
    };
    let query = strip(&request.query)?.to_owned();
    let language = strip(&request.language)?;
    let count = language.chars().count();
    let language = tos_foundation::python_lower_unicode16_v1(
        language,
        count,
        count.saturating_mul(3),
        language.len().saturating_mul(3),
    )
    .map_err(|_| budget_error())?;
    if query.is_empty()
        || query.chars().count() > 256
        || request.limit > 100
        || !["de", "ru", "en"].contains(&language.as_str())
    {
        return Err(error(
            ReadingSearchErrorCode::InvalidRequest,
            "invalid reading-search request",
        ));
    }
    let mut group_by = Vec::new();
    for group in &request.group_by {
        if !["speaker", "formula"].contains(&group.as_str()) {
            return Err(error(
                ReadingSearchErrorCode::InvalidRequest,
                "invalid reading group_by",
            ));
        }
        if !group_by.contains(group) {
            group_by.push(group.clone());
        }
    }
    Ok(ReadingSearchRequest {
        query,
        language,
        group_by,
        ..request.clone()
    })
}

/// Entire predecessor selection, source fixity, database binding, enrichment,
/// grouping, normalization additions and installed-schema validation. The
/// result retains every admitted inode and selected pathname for final recheck;
/// the adapter retains its actual current-observation/disclosure fence through
/// response flush. These pins observe selected local bytes; they do not issue
/// review, rights, source or publication authority.
pub fn execute_reading_search(
    roots: &ExplicitReadingRoots,
    software: &ReadingSoftware,
    request: &ReadingSearchRequest,
    budget: ReadingSearchBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<ReadingSearchResult> {
    check(probe.as_ref())?;
    let mut read = Reader::new(roots, software, budget, &probe)?;
    read.work(
        request
            .query
            .len()
            .checked_add(request.language.len())
            .ok_or_else(budget_error)?,
    )?;
    for group in &request.group_by {
        read.work(group.len())?;
    }
    let normalized = normalize_reading_request(request)?;
    let request = &normalized;
    let query = request.query.clone();
    let manifest = read.json(local::Root::Analysis, READING_MANIFEST_REF, false)?;
    if string(&manifest, "schema_version")? != "tos_zarathustra_reading_manifest_v1"
        || ["accepted", "human_review", "canon_effect"]
            .iter()
            .any(|f| manifest.get(f) != Some(&Value::Bool(false)))
        || string(&manifest, "publication_posture")? != "excluded_from_public_bundle"
    {
        return Err(corrupt("reading manifest exceeds candidate boundary"));
    }
    let descriptor = manifest
        .get("private_database")
        .ok_or_else(|| corrupt("reading database descriptor absent"))?;
    let reading_db_ref = string(descriptor, "ref")?.to_owned();
    let reading_db_sha = string(descriptor, "sha256")?.to_owned();
    let db_file = read.verified_file(
        local::Root::Analysis,
        &reading_db_ref,
        &reading_db_sha,
        true,
    )?;
    for artifact in manifest
        .get("artifacts")
        .map(|v| {
            v.as_array()
                .map(Vec::as_slice)
                .ok_or_else(|| corrupt("reading artifacts invalid"))
        })
        .transpose()?
        .unwrap_or(&[])
    {
        read.verified_file(
            local::Root::Analysis,
            string(artifact, "ref")?,
            string(artifact, "sha256")?,
            false,
        )?;
    }
    for input in manifest
        .get("inputs")
        .map(|v| {
            v.as_array()
                .map(Vec::as_slice)
                .ok_or_else(|| corrupt("reading inputs invalid"))
        })
        .transpose()?
        .unwrap_or(&[])
    {
        if input.get("manifest_ref").is_some_and(local::truthy)
            || input.get("role").and_then(Value::as_str) == Some("source_visible_voice_policy")
        {
            let reference = input
                .get("manifest_ref")
                .and_then(Value::as_str)
                .unwrap_or(string(input, "ref")?);
            let expected = input
                .get("manifest_sha256")
                .and_then(Value::as_str)
                .unwrap_or(string(input, "sha256")?);
            read.verified_file(local::Root::Source, reference, expected, false)?;
        }
    }
    let db = read.database(&db_file)?;
    let metadata_rows = read.sql(&db, "SELECT key,value FROM metadata", &[])?;
    let metadata = metadata_rows
        .into_iter()
        .map(|r| {
            Ok((
                string(&r, "key")?.to_owned(),
                string(&r, "value")?.to_owned(),
            ))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    let (mut baseline, concept_db_hashes) = concept::baseline(&mut read, &query, request)?;
    if !concept_db_hashes
        .iter()
        .any(|sha| Some(sha) == metadata.get("concept_workbench_sha256"))
    {
        return Err(corrupt("reading database stale relative to concept source"));
    }
    let alignments = enrich::alignment_index(&mut read, &db)?;
    let mut cards = Vec::new();
    for card in array(&baseline, "results")? {
        let card = read.clone_value(card)?;
        cards.push(enrich::enrich(&mut read, &db, card, &alignments, None)?);
    }
    if cards.len() as u64
        != baseline["coverage"]["total_source_results"]
            .as_u64()
            .ok_or_else(|| corrupt("baseline count absent"))?
    {
        return Err(corrupt(
            "predecessor did not return all matches before limit",
        ));
    }
    let additions = enrich::normalization_candidates(&mut read, &db, &cards, &alignments)?;
    let mut returned = Vec::new();
    for card in cards.iter().take(request.limit) {
        returned.push(read.clone_value(card)?);
    }
    let groups = enrich::groups(&mut read, &cards, &returned, &request.group_by)?;
    let source_provenance = baseline["provenance"].clone();
    let reading_provenance = json!({
        "manifest_ref": READING_MANIFEST_REF, "manifest_sha256": read.file_hash(local::Root::Analysis, READING_MANIFEST_REF)?,
        "private_database_ref": reading_db_ref, "private_database_sha256": reading_db_sha,
    });
    let reading_id = format!(
        "tos.navigation.reading-search-result.sid-{}",
        &hash(&format!(
            "{}{}{}{}",
            string(&baseline, "search_result_id")?,
            reading_db_sha,
            request.limit,
            request.group_by.join(",")
        ))[..32]
    );
    baseline["schema_version"] = json!("tos_zarathustra_reading_search_result_v1");
    baseline["reading_search_result_id"] = json!(reading_id);
    // Actual compiled native provider bytes and exact embedded software schema.
    // The compatible v1 schema preserves historical Python result validation.
    baseline["provenance"] = json!({
        "concept_predecessor": source_provenance, "reading_layer": reading_provenance,
        "adapter_ref": READING_ADAPTER_REF, "adapter_sha256": read.file_hash(local::Root::Software, READING_ADAPTER_REF)?,
        "result_schema_ref": READING_SCHEMA_REF, "result_schema_sha256": read.file_hash(local::Root::Software, READING_SCHEMA_REF)?,
        "method_version": metadata.get("method_version"),
    });
    let coverage = baseline["coverage"]
        .as_object_mut()
        .ok_or_else(|| corrupt("baseline coverage invalid"))?;
    let mut status_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut anchor_counts = std::collections::BTreeMap::<String, usize>::new();
    for c in &cards {
        *status_counts
            .entry(string(&c["speaker"], "status")?.to_owned())
            .or_default() += 1;
        *anchor_counts
            .entry(string(c, "source_occurrence_anchor_status")?.to_owned())
            .or_default() += 1;
    }
    for (k,v) in json!({"returned_source_results":returned.len(), "limit":request.limit,
        "grouping_scope":"all_matching_source_occurrences_before_limit", "speaker_status_counts":status_counts,
        "occurrence_anchor_status_counts":anchor_counts,
        "occurrences_with_formula_membership":cards.iter().filter(|c| c["formula_memberships"].as_array().is_some_and(|a| !a.is_empty())).count(),
        "additional_source_candidate_count":additions.len(), "returned_additional_source_candidates":additions.len().min(request.limit),
        "primary_result_scope":"verified_predecessor_concept_request_occurrences", "grouping_excludes_additional_candidates":true,
        "whole_book_semantic_recall_asserted":false}).as_object().unwrap() { coverage.insert(k.clone(),v.clone()); }
    baseline["groups"] = groups;
    baseline["results"] = json!(returned);
    baseline["additional_source_candidates"] = json!(
        additions
            .into_iter()
            .take(request.limit)
            .collect::<Vec<_>>()
    );
    baseline["limitations"] = json!([
        "Speaker attribution and sentence/clause alignment remain reviewable candidates, not accepted interpretation.",
        "Morphology and dependencies are inherited heuristic candidates; this layer does not repair full contextual syntax.",
        "Counts cover the selected concept request, not every possible semantic mention of a concept.",
        "Additional explicit line-break normalization candidates have separate IDs/counts and are not hidden in predecessor result groups.",
        "Formula membership is exact after declared normalization; nearby formulas are not membership.",
        "English translation and etymological research are on-demand agent work and have not been executed by this query."
    ]);
    let body = read.emit(&baseline)?;
    read.validate(READING_SCHEMA_REF, &body)?;
    read.finish(body)
}

#[cfg(test)]
mod controls;

#[cfg(any(test, feature = "test-fixture"))]
pub mod reading_fixture;

#[cfg(test)]
mod word_analysis_tests;
