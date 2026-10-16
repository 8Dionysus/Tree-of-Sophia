//! Word-analysis controls on the existing disposable ReadingFixture. Only its
//! local task/request IDs and their declared artifact digests are adjusted to
//! the existing strict task schema; this creates no production source rule.
use super::reading_fixture::ReadingFixture;
use super::*;

struct NoAbort;
impl AbortProbe for NoAbort {
    fn reason(&self) -> Option<AbortReason> {
        None
    }
}

fn task_fixture() -> ReadingFixture {
    ReadingFixture::new().with_word_analysis()
}

fn request(rank: usize) -> WordAnalysisRequest {
    WordAnalysisRequest {
        query: "судьбы".into(),
        language: "ru".into(),
        rank,
        include_semantic_neighbors: false,
        request_ref: None,
    }
}

fn prepare(fixture: &ReadingFixture, rank: usize) -> Value {
    let task = execute_word_analysis_task(
        &fixture.roots,
        &ReadingSoftware::embedded(),
        &request(rank),
        ReadingSearchBudget::local_default(),
        Arc::new(NoAbort),
    )
    .unwrap();
    serde_json::from_slice(&task.body).unwrap()
}

fn candidate(task: &Value) -> Value {
    json!({
        "schema_version": "tos_zarathustra_english_translation_candidate_v1",
        "translation_candidate_id": format!(
            "tos.annotation.english-translation-candidate.sid-{}",
            &hash(task["analysis_task_id"].as_str().unwrap())[..32]
        ),
        "english_task_ref": task["english_on_demand_task_ref"],
        "request_ref": task["request_ref"],
        "source_occurrence_ref": task["source"]["occurrence_candidate_ref"],
        "source_context_unit_ref": task["source"]["context_unit_ref"],
        "target_language": "en",
        "source_echo_sha256": task["source"]["surface_sha256"],
        "source_context_echo_sha256": task["source"]["context_sha256"],
        "literal_gloss": "fate",
        "contextual_translation": "fate",
        "semantic_alternatives": [{
            "rendering": "destiny", "preserves": ["directed course"],
            "loses_or_risks": ["may overstate purpose"]
        }],
        "untranslatability_note": "These synthetic alternatives preserve different source ranges.",
        "analysis": {
            "morphology": "synthetic neuter singular noun proposal",
            "syntax": "synthetic nominal occurrence proposal",
            "historical_sense": "a synthetic cited candidate",
            "contextual_semantics": "an occurrence-bound synthetic proposal",
            "etymology_state": "sourced_candidate",
            "etymology_findings": [{
                "claim": "synthetic etymological proposal for structural validation only",
                "translation_consequence": "retain competing English renderings",
                "citations": [{
                    "reference_id": "tos-ref.synthetic",
                    "locator": "synthetic headword article",
                    "url": "https://example.invalid/Schicksal",
                    "accessed_at": "2026-09-03"
                }],
                "epistemic_status": "proposed"
            }],
            "intra_work_recurrence": "compare exact occurrence recurrence before choosing a rendering"
        },
        "russian_witness_comparison": {
            "consulted": false,
            "role": "historical_translation_comparator_not_source_authority",
            "divergence_note": "This disposable fixture provides no Russian paragraph comparator."
        },
        "maker": {
            "maker_kind": "ai", "model_id": "test-model",
            "context_isolation": "current_task_only", "training_exposure": "unknown"
        },
        "provenance_event_ref": "tos.event.synthetic.word-analysis",
        "status": "ai_generated_unreviewed_not_translation_truth",
        "accepted": false, "review_status": "unreviewed",
        "translation_truth_asserted": false, "semantic_fact_asserted": false,
        "graph_effect": false, "canon_effect": false
    })
}

fn validate(fixture: &ReadingFixture, candidate: &Value) -> Result<ReadingSearchResult> {
    validate_word_analysis_candidate(
        &fixture.roots,
        &ReadingSoftware::embedded(),
        &request(1),
        &serde_json::to_vec(candidate).unwrap(),
        ReadingSearchBudget::local_default(),
        Arc::new(NoAbort),
    )
}

fn refusal<T>(result: Result<T>) -> ReadingSearchError {
    match result {
        Ok(_) => panic!("expected word-analysis refusal"),
        Err(error) => error,
    }
}

#[test]
fn word_analysis_task_keeps_exact_source_digest_rank_and_candidate_authority() {
    let fixture = task_fixture();
    let first = prepare(&fixture, 1);
    assert_eq!(first["query_analysis"]["matched_alias"], "судьба");
    assert_eq!(first["source"]["language"], "de");
    let surface = first["source"]["surface"].as_str().unwrap();
    let context = first["source"]["exact_context"].as_str().unwrap();
    assert!(context.contains(surface));
    assert_eq!(first["source"]["surface_sha256"], hash(surface));
    assert_eq!(first["source"]["context_sha256"], hash(context));
    assert_eq!(
        first["analysis_policy"]["required_stages"],
        json!([
            "morphology",
            "syntax",
            "historical_sense",
            "sourced_etymology",
            "contextual_semantics",
            "intra_work_recurrence",
            "russian_witness_comparison",
            "english_rendering"
        ])
    );
    assert_eq!(
        first["analysis_policy"]["etymology"]["minimum_citation_count"],
        1
    );
    assert_eq!(first["recurrence_navigation"]["total_source_results"], 2);
    assert_eq!(first["recurrence_navigation"]["current_rank"], 1);
    assert_eq!(first["recurrence_navigation"]["next_rank"], 2);
    assert_eq!(first["source"]["occurrence_candidate_ref"], "candidate-0");
    assert!(
        first["analysis_policy"]["etymology"]["evidence_route_ref"]
            .as_str()
            .unwrap()
            .ends_with("ETYMOLOGY_EVIDENCE_ROUTE_RESEARCH.md")
    );
    assert_eq!(
        first["provenance"]["task_builder_ref"],
        super::word_analysis::PROVIDER_REF
    );
    for flag in [
        "accepted",
        "semantic_fact_asserted",
        "translation_truth_asserted",
        "graph_effect",
        "canon_effect",
    ] {
        assert_eq!(first["authority"][flag], false);
    }
    let second = prepare(&fixture, 2);
    assert_eq!(second["source"]["occurrence_candidate_ref"], "candidate-1");
    assert_eq!(second["recurrence_navigation"]["current_rank"], 2);
    assert_eq!(second["recurrence_navigation"]["next_rank"], Value::Null);
    assert_ne!(second["analysis_task_id"], first["analysis_task_id"]);
    assert_ne!(
        second["english_on_demand_task_ref"],
        first["english_on_demand_task_ref"]
    );
    for rank in [0, 3] {
        let error = refusal(execute_word_analysis_task(
            &fixture.roots,
            &ReadingSoftware::embedded(),
            &request(rank),
            ReadingSearchBudget::local_default(),
            Arc::new(NoAbort),
        ));
        assert_eq!(error.code, ReadingSearchErrorCode::InvalidRequest);
    }
}

#[test]
fn word_analysis_valid_candidate_then_wrong_context_digest_and_uncited_claim_refuse() {
    let fixture = task_fixture();
    let task = prepare(&fixture, 1);
    let mut candidate = candidate(&task);
    let result = validate(&fixture, &candidate).unwrap();
    let receipt: Value = serde_json::from_slice(&result.body).unwrap();
    assert_eq!(receipt["valid"], true);
    assert_eq!(receipt["analysis_task_ref"], task["analysis_task_id"]);
    assert_eq!(
        receipt["candidate_ref"],
        candidate["translation_candidate_id"]
    );
    assert_eq!(
        receipt["binding_checks"],
        json!([
            "english task",
            "request",
            "source context",
            "source context digest",
            "source occurrence",
            "source surface digest"
        ])
    );
    assert_eq!(receipt["authority"], task["authority"]);

    candidate["source_context_echo_sha256"] = json!(hash("a different synthetic source context"));
    let mismatch = refusal(validate(&fixture, &candidate));
    assert_eq!(mismatch.code, ReadingSearchErrorCode::InvalidRequest);
    assert_eq!(
        mismatch.message,
        "word-analysis candidate source context digest mismatch"
    );

    candidate["source_context_echo_sha256"] = task["source"]["context_sha256"].clone();
    candidate["analysis"]["etymology_findings"][0]["citations"] = json!([]);
    let uncited = refusal(validate(&fixture, &candidate));
    assert_eq!(uncited.code, ReadingSearchErrorCode::CorruptSelectedCarrier);
    assert_eq!(uncited.message, "reading packet violates installed schema");
}

#[test]
fn word_analysis_request_budget_refuses_before_opening_selected_roots() {
    let roots = ExplicitReadingRoots {
        source_root: std::path::PathBuf::from("/not-opened-word-source"),
        analysis_root: std::path::PathBuf::from("/not-opened-word-analysis"),
    };
    let mut budget = ReadingSearchBudget::local_default();
    budget.json.max_bytes = 1;
    let refused = refusal(execute_word_analysis_task(
        &roots,
        &ReadingSoftware::embedded(),
        &request(1),
        budget,
        Arc::new(NoAbort),
    ));
    assert_eq!(refused.code, ReadingSearchErrorCode::BudgetExceeded);
}
