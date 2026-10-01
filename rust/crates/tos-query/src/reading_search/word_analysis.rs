//! Private source-bound task preparation, never analysis, admission or publication.
//! Uses the existing concept reader and preserves its held source custody.
use super::*;
use local::Root;

pub(super) const TASK_SCHEMA_REF: &str =
    "ToS/candidate-intake/zarathustra/concept-workbench-v1/word-analysis-task.v1.schema.json";
pub(super) const CANDIDATE_SCHEMA_REF: &str = "ToS/candidate-intake/zarathustra/concept-workbench-v1/english-translation-candidate.v1.schema.json";
pub(super) const PROVIDER_REF: &str = "rust/crates/tos-query/src/reading_search/word_analysis.rs";

#[derive(Clone, Debug)]
pub struct WordAnalysisRequest {
    pub query: String,
    pub language: String,
    pub rank: usize,
    pub include_semantic_neighbors: bool,
    pub request_ref: Option<String>,
}

/// The maintained standalone CLI uses Python int spelling, independently of
/// the typed MCP/HTTP rank clamp. Reuse the concept CLI's bounded decoder.
pub fn parse_word_analysis_rank(raw: &str, max_digits: usize) -> Result<usize> {
    let (_, rank) = super::concept_query::limit(raw, max_digits)?;
    if rank == 0 {
        return Err(error(
            ReadingSearchErrorCode::InvalidRequest,
            "word-analysis rank must be positive",
        ));
    }
    Ok(rank)
}

fn shell_word(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn check_request(request: &WordAnalysisRequest, budget: &ReadingSearchBudget) -> Result<()> {
    if request.rank == 0 {
        return Err(error(
            ReadingSearchErrorCode::InvalidRequest,
            "word-analysis rank must be positive",
        ));
    }
    let bytes = request
        .query
        .len()
        .checked_add(request.language.len())
        .and_then(|n| n.checked_add(request.request_ref.as_ref().map_or(0, String::len)))
        .ok_or_else(budget_error)?;
    if bytes > budget.json.max_bytes {
        return Err(budget_error());
    }
    Ok(())
}

fn prepare(
    read: &mut Reader<'_>,
    request: &WordAnalysisRequest,
    source_root: &std::path::Path,
) -> Result<(Value, Vec<u8>)> {
    let mut normalized = normalize_reading_request(&ReadingSearchRequest {
        query: request.query.clone(),
        language: request.language.clone(),
        limit: 1,
        include_semantic_neighbors: request.include_semantic_neighbors,
        group_by: Vec::new(),
        request_ref: request.request_ref.clone(),
    })?;
    // The maintained script accepts an absolute --request. Keep that input
    // shape only inside the explicitly selected source root; the existing
    // protected Reader still checks every relative component and exact member.
    if let Some(reference) = normalized.request_ref.as_deref() {
        let path = std::path::Path::new(reference);
        if path.is_absolute() {
            normalized.request_ref = Some(
                path.strip_prefix(source_root)
                    .ok()
                    .and_then(std::path::Path::to_str)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        error(
                            ReadingSearchErrorCode::InvalidRequest,
                            "word-analysis request must belong to selected source root",
                        )
                    })?
                    .to_owned(),
            );
        }
    }
    read.work(
        request
            .query
            .len()
            .checked_add(request.language.len())
            .and_then(|n| n.checked_add(request.request_ref.as_ref().map_or(0, String::len)))
            .ok_or_else(budget_error)?,
    )?;
    let (search, _) =
        concept::baseline_with_limit(read, &request.query, &normalized, request.rank as u64)?;
    let rows = array(&search, "results")?;
    let row = rows.get(request.rank - 1).ok_or_else(|| {
        error(
            ReadingSearchErrorCode::InvalidRequest,
            "word-analysis rank exceeds source results",
        )
    })?;
    // Charge the copied row and request before constructing the task. The
    // installed schema and final emitter independently bound the output.
    read.admit_value(row)?;
    read.admit_value(&search["query_analysis"])?;
    let selected = read.json(
        Root::Source,
        normalized
            .request_ref
            .as_deref()
            .unwrap_or(DEFAULT_REQUEST_REF),
        false,
    )?;
    let mut source = serde_json::Map::new();
    source.insert("language".into(), json!("de"));
    for (target, original) in [
        (
            "occurrence_candidate_ref",
            "source_occurrence_candidate_ref",
        ),
        ("existing_occurrence_ref", "source_existing_occurrence_ref"),
        ("context_unit_ref", "source_context_unit_ref"),
        ("surface", "source_surface"),
        ("analysis_form", "source_analysis_form"),
        ("headword_candidate", "source_headword_candidate"),
        ("headword_status", "source_headword_status"),
        ("exact_context", "source_context"),
        ("anchor_refs", "anchor_refs"),
        ("part", "part"),
        ("reading_ref", "reading_ref"),
        ("unit_kind", "unit_kind"),
        ("witness_order", "witness_order"),
        ("token_ordinal", "token_ordinal"),
        ("speaker", "speaker"),
    ] {
        source.insert(
            target.into(),
            row.get(original)
                .ok_or_else(|| corrupt("word-analysis source field absent"))?
                .clone(),
        );
    }
    source.insert(
        "surface_sha256".into(),
        json!(hash(string(row, "source_surface")?)),
    );
    source.insert(
        "context_sha256".into(),
        json!(hash(string(row, "source_context")?)),
    );
    let mut comparators = Vec::new();
    for comparator in array(row, "russian_comparators")? {
        let mut value = comparator.clone();
        let digest = hash(string(comparator, "exact_text")?);
        value
            .as_object_mut()
            .ok_or_else(|| corrupt("word-analysis comparator invalid"))?
            .insert("context_sha256".into(), json!(digest));
        comparators.push(value);
    }
    let total = search["coverage"]["total_source_results"]
        .as_u64()
        .ok_or_else(|| corrupt("word-analysis total absent"))?;
    let task_id = format!(
        "tos.annotation.word-analysis-task.sid-{}",
        &hash(&format!(
            "zarathustra-word-analysis-v1\n{}",
            string(row, "source_occurrence_candidate_ref")?
        ))[..32]
    );
    let mut command = format!(
        "tos --root {} word-analysis --query {} --language {} --rank {}",
        shell_word(source_root.to_str().ok_or_else(|| error(
            ReadingSearchErrorCode::InvalidRequest,
            "word-analysis root must be UTF-8"
        ))?),
        shell_word(&request.query),
        normalized.language,
        request.rank
    );
    if normalized.include_semantic_neighbors {
        command.push_str(" --include-semantic-neighbors");
    }
    if let Some(reference) = &normalized.request_ref {
        command.push_str(" --request ");
        command.push_str(&shell_word(reference));
    }
    command.push_str(" --validate-candidate CANDIDATE.json");
    let task = json!({
        "schema_version":"tos_zarathustra_word_analysis_task_v1", "analysis_task_id":task_id,
        "request_ref":search["concept_search_route"]["current_request_id"],
        "concept_search_result_ref":search["search_result_id"], "query_analysis":search["query_analysis"],
        "source":source, "russian_comparators":comparators,
        "recurrence_navigation":{
            "operation_id":"tos.zarathustra.word-analysis.prepare", "total_source_results":total,
            "current_rank":request.rank, "next_rank":if (request.rank as u64)<total { Some(request.rank+1) } else { None },
            "query":request.query,"language":normalized.language,
            "semantic_neighbors_included":search["coverage"]["semantic_neighbors_included"]
        },
        "english_on_demand_task_ref":row["english_on_demand_task_ref"],
        "analysis_policy":{
            "required_stages":["morphology","syntax","historical_sense","sourced_etymology","contextual_semantics","intra_work_recurrence","russian_witness_comparison","english_rendering"],
            "source_language":"de","output_language":"en","lemma_posture":"candidate_until_occurrence_visible_review",
            "etymology":{
                "state":"citation_required_before_claim","minimum_citation_count":1,"point_consultation_only":true,"bulk_ingest_allowed":false,
                "evidence_route_ref":selected["english_generation"]["etymology_route"]["ref"],
                "reference_register_ref":selected["english_generation"]["reference_register"]["ref"],
                "etymological_fallacy_guard":"word_history_may_inform_but_never_determine_contextual_meaning_or_authorial_intent"
            },
            "translation_posture":"german_source_first_russian_comparator_english_unreviewed_candidate"
        },
        "response_contract":{"schema_ref":CANDIDATE_SCHEMA_REF,"validation_command":command,"persistence":"none_return_candidate_to_caller"},
        "authority":{
            "task_status":"ready_for_on_demand_agent_analysis","content_posture":"local_runtime_exact_source_return_not_for_public_bundle",
            "accepted":false,"review_status":"unreviewed","translation_truth_asserted":false,
            "semantic_fact_asserted":false,"graph_effect":false,"canon_effect":false
        },
        "provenance":{
            "concept_search_adapter_ref":CONCEPT_ADAPTER_REF,"concept_search_adapter_sha256":read.file_hash(Root::Software,CONCEPT_ADAPTER_REF)?,
            "task_builder_ref":PROVIDER_REF,"task_builder_sha256":read.file_hash(Root::Software,PROVIDER_REF)?,
            "task_schema_ref":TASK_SCHEMA_REF,"task_schema_sha256":read.file_hash(Root::Software,TASK_SCHEMA_REF)?
        }
    });
    let body = read.emit(&task)?;
    read.validate(TASK_SCHEMA_REF, &body)?;
    Ok((task, body))
}

pub fn execute_word_analysis_task(
    roots: &ExplicitReadingRoots,
    software: &ReadingSoftware,
    request: &WordAnalysisRequest,
    budget: ReadingSearchBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<ReadingSearchResult> {
    check(probe.as_ref())?;
    check_request(request, &budget)?;
    let mut read = Reader::new(roots, software, budget, &probe)?;
    let (_, body) = prepare(&mut read, request, &roots.source_root)?;
    read.finish(body)
}

/// Validate the exact caller-supplied candidate against the SAME freshly
/// prepared source task. This returns a binding receipt, not translation review.
pub fn validate_word_analysis_candidate(
    roots: &ExplicitReadingRoots,
    software: &ReadingSoftware,
    request: &WordAnalysisRequest,
    candidate: &[u8],
    budget: ReadingSearchBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<ReadingSearchResult> {
    check(probe.as_ref())?;
    check_request(request, &budget)?;
    if candidate.len() > budget.json.max_bytes {
        return Err(budget_error());
    }
    let mut read = Reader::new(roots, software, budget, &probe)?;
    let (task, _) = prepare(&mut read, request, &roots.source_root)?;
    read.validate(CANDIDATE_SCHEMA_REF, candidate)?;
    let candidate = read.parse(candidate)?;
    let bindings = [
        (
            "english task",
            &candidate["english_task_ref"],
            &task["english_on_demand_task_ref"],
        ),
        ("request", &candidate["request_ref"], &task["request_ref"]),
        (
            "source occurrence",
            &candidate["source_occurrence_ref"],
            &task["source"]["occurrence_candidate_ref"],
        ),
        (
            "source context",
            &candidate["source_context_unit_ref"],
            &task["source"]["context_unit_ref"],
        ),
        (
            "source surface digest",
            &candidate["source_echo_sha256"],
            &task["source"]["surface_sha256"],
        ),
        (
            "source context digest",
            &candidate["source_context_echo_sha256"],
            &task["source"]["context_sha256"],
        ),
    ];
    for (name, actual, expected) in &bindings {
        if actual != expected {
            let message = match *name {
                "english task" => "word-analysis candidate english task mismatch",
                "request" => "word-analysis candidate request mismatch",
                "source occurrence" => "word-analysis candidate source occurrence mismatch",
                "source context" => "word-analysis candidate source context mismatch",
                "source surface digest" => "word-analysis candidate source surface digest mismatch",
                _ => "word-analysis candidate source context digest mismatch",
            };
            return Err(error(ReadingSearchErrorCode::InvalidRequest, message));
        }
    }
    let mut names = bindings
        .iter()
        .map(|(name, _, _)| *name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    let receipt = json!({"schema_version":"tos_zarathustra_word_analysis_validation_receipt_v1",
        "analysis_task_ref":task["analysis_task_id"],"candidate_ref":candidate["translation_candidate_id"],
        "valid":true,"binding_checks":names,"authority":task["authority"]});
    let body = read.emit(&receipt)?;
    read.finish(body)
}
