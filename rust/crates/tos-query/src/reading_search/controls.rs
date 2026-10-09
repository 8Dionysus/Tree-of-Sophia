//! Disposable original synthetic witness/provider controls in the existing
//! QRY lib test target. No installed corpus or publication is queried.
use super::reading_fixture::ReadingFixture as Fixture;
use super::*;
use std::{fs, os::unix::fs::PermissionsExt};
struct NoAbort;
impl AbortProbe for NoAbort {
    fn reason(&self) -> Option<AbortReason> {
        None
    }
}
fn code<T>(r: Result<T>) -> ReadingSearchErrorCode {
    match r {
        Ok(_) => panic!("expected refusal"),
        Err(e) => e.code,
    }
}
#[test]
fn original_whole_consumer_keeps_all_groups_and_separate_normalization_candidates() {
    let fixture = Fixture::new();
    let packet = fixture.packet(&fixture.request());
    assert_eq!(packet["coverage"]["total_source_results"], 2);
    assert_eq!(packet["coverage"]["returned_source_results"], 1);
    assert_eq!(
        packet["query_analysis"]["match_method"],
        "morphology_alias_candidate"
    );
    assert_eq!(packet["results"][0]["speaker"]["role"], "dwarf");
    assert_eq!(
        packet["results"][0]["speaker_predecessor"]["role"],
        "narrator"
    );
    assert_eq!(
        packet["results"][0]["source_occurrence_spans"][0]["start_offset"],
        3
    );
    assert_eq!(packet["coverage"]["speaker_status_counts"]["proposed"], 2);
    assert_eq!(packet["groups"]["by_speaker"].as_array().unwrap().len(), 2);
    assert_eq!(packet["groups"]["no_formula_membership_count"], 1);
    assert_eq!(packet["groups"]["by_formula"][0]["formula_id"], "member");
    assert_eq!(
        packet["results"][0]["context_formula_memberships"][0]["formula_id"],
        "nearby"
    );
    assert_eq!(
        packet["additional_source_candidates"][0]["source_surface"],
        "Schick¬\nsal"
    );
    assert_eq!(packet["coverage"]["additional_source_candidate_count"], 1);
    assert_eq!(
        packet["additional_source_candidates"][0]["source_existing_occurrence_ref"],
        Value::Null
    );
    assert_eq!(packet["provenance"]["adapter_ref"], READING_PROVIDER_REF);
    for flag in [
        "accepted",
        "semantic_fact_asserted",
        "translation_truth_asserted",
        "graph_effect",
        "canon_effect",
    ] {
        assert_eq!(packet["results"][0][flag], false);
        assert_eq!(packet["additional_source_candidates"][0][flag], false);
    }
    let mut counts_only = fixture.request();
    counts_only.limit = 0;
    let p = fixture.packet(&counts_only);
    assert_eq!(p["results"], json!([]));
    assert_eq!(p["coverage"]["total_source_results"], 2);
    assert!(
        p["groups"]["by_speaker"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["returned_occurrence_refs"] == json!([]))
    );
    let mut semantic = fixture.request();
    semantic.include_semantic_neighbors = true;
    assert_eq!(
        fixture.packet(&semantic)["coverage"]["total_source_results"],
        3
    );
}
#[test]
fn missing_artifacts_and_all_authority_fixity_mode_and_cost_controls_refuse() {
    let fixture = Fixture::new();
    let r = fixture.request();
    let reading_db = fixture.roots.analysis_root.join("reading #db.sqlite3");
    fs::set_permissions(&reading_db, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        code(fixture.query(&r, ReadingSearchBudget::local_default())),
        ReadingSearchErrorCode::CorruptSelectedCarrier
    );
    fs::set_permissions(&reading_db, fs::Permissions::from_mode(0o600)).unwrap();
    let mut b = ReadingSearchBudget::local_default();
    b.max_sql_vm_steps = 1;
    assert_eq!(
        code(fixture.query(&r, b)),
        ReadingSearchErrorCode::BudgetExceeded
    );
    let mut b = ReadingSearchBudget::local_default();
    b.max_sql_rows = 1;
    assert_eq!(
        code(fixture.query(&r, b)),
        ReadingSearchErrorCode::BudgetExceeded
    );
    let mut b = ReadingSearchBudget::local_default();
    b.max_total_file_bytes = 1;
    assert_eq!(
        code(fixture.query(&r, b)),
        ReadingSearchErrorCode::BudgetExceeded
    );
    let mut b = ReadingSearchBudget::local_default();
    b.max_response_bytes = 32;
    assert_eq!(
        code(fixture.query(&r, b)),
        ReadingSearchErrorCode::BudgetExceeded
    );
    let manifest = fixture.roots.analysis_root.join(READING_MANIFEST_REF);
    let raw = fs::read(&manifest).unwrap();
    let mut value: Value = serde_json::from_slice(&raw).unwrap();
    value["accepted"] = json!(true);
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        code(fixture.query(&r, ReadingSearchBudget::local_default())),
        ReadingSearchErrorCode::CorruptSelectedCarrier
    );
    fs::write(&manifest, &raw).unwrap();
    fs::write(fixture.roots.source_root.join("policy.json"), b"drift").unwrap();
    assert_eq!(
        code(fixture.query(&r, ReadingSearchBudget::local_default())),
        ReadingSearchErrorCode::CorruptSelectedCarrier
    );
    fs::remove_file(&manifest).unwrap();
    assert_eq!(
        code(fixture.query(&r, ReadingSearchBudget::local_default())),
        ReadingSearchErrorCode::Unavailable
    );
}
#[test]
fn delivery_pin_observes_source_mutation_and_abort() {
    let fixture = Fixture::new();
    let mut result = fixture
        .query(&fixture.request(), ReadingSearchBudget::local_default())
        .unwrap();
    assert!(
        result.charge.fixity_bytes_hashed > 0
            && result.charge.sql_vm_steps > 0
            && result.charge.schema_validations == 3
    );
    fs::write(fixture.roots.source_root.join("policy.json"), b"drift").unwrap();
    assert_eq!(
        result.recheck(&NoAbort).unwrap_err().code,
        ReadingSearchErrorCode::CorruptSelectedCarrier
    );
    struct Cancel;
    impl AbortProbe for Cancel {
        fn reason(&self) -> Option<AbortReason> {
            Some(AbortReason::Cancelled)
        }
    }
    assert_eq!(
        code(execute_reading_search(
            &fixture.roots,
            &ReadingSoftware::embedded(),
            &fixture.request(),
            ReadingSearchBudget::local_default(),
            Arc::new(Cancel)
        )),
        ReadingSearchErrorCode::Cancelled
    );
}
#[test]
fn native_python16_original_provider_semantics_and_explicit_provenance_difference() {
    let fixture = Fixture::new();
    let software = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut cases = vec![fixture.request()];
    for (query, lang, limit, semantic, groups) in [
        ("Schicksal", "de", 0, false, vec![]),
        ("Ｆａｔｅ", "en", 2, false, vec!["formula".into()]),
        ("случай", "ru", 3, false, vec!["speaker".into()]),
        (
            "судьба",
            "ru",
            3,
            true,
            vec!["speaker".into(), "formula".into()],
        ),
    ] {
        cases.push(ReadingSearchRequest {
            query: query.into(),
            language: lang.into(),
            limit,
            include_semantic_neighbors: semantic,
            group_by: groups,
            request_ref: None,
        });
    }
    for request in cases {
        let program = r#"import importlib.util,json,sys,unicodedata
from pathlib import Path
assert unicodedata.unidata_version=='16.0.0', unicodedata.unidata_version
path=Path(sys.argv[1])/'scripts/query_zarathustra_reading_workbench_v1.py'
spec=importlib.util.spec_from_file_location('reading_original_oracle',path)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
r=m.build_result(sys.argv[4],sys.argv[5],limit=int(sys.argv[6]),include_semantic_neighbors=sys.argv[7]=='true',group_by=tuple(json.loads(sys.argv[8])),source_root=Path(sys.argv[2]),analysis_root=Path(sys.argv[3]))
print(json.dumps(r,ensure_ascii=False))"#;
        let python =
            std::env::var("TOS_READING_ORACLE_PYTHON").unwrap_or_else(|_| "python3".into());
        let output = std::process::Command::new(python)
            .arg("-c")
            .arg(program)
            .arg(&software)
            .arg(&fixture.roots.source_root)
            .arg(&fixture.roots.analysis_root)
            .arg(&request.query)
            .arg(&request.language)
            .arg(request.limit.to_string())
            .arg(request.include_semantic_neighbors.to_string())
            .arg(serde_json::to_string(&request.group_by).unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "oracle failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut oracle: Value = serde_json::from_slice(&output.stdout).unwrap();
        let mut native = fixture.packet(&request);
        assert_eq!(native["provenance"]["adapter_ref"], READING_PROVIDER_REF);
        assert_eq!(
            oracle["provenance"]["adapter_ref"],
            "scripts/query_zarathustra_reading_workbench_v1.py"
        );
        assert_eq!(
            native["provenance"]["reading_layer"],
            oracle["provenance"]["reading_layer"]
        );
        assert_eq!(
            native["provenance"]["result_schema_sha256"],
            oracle["provenance"]["result_schema_sha256"]
        );
        for path in ["adapter_ref", "adapter_sha256"] {
            native["provenance"].as_object_mut().unwrap().remove(path);
            oracle["provenance"].as_object_mut().unwrap().remove(path);
        }
        for path in ["query_adapter_ref", "query_adapter_sha256"] {
            native["provenance"]["concept_predecessor"]
                .as_object_mut()
                .unwrap()
                .remove(path);
            oracle["provenance"]["concept_predecessor"]
                .as_object_mut()
                .unwrap()
                .remove(path);
        }
        println!("RETAINED_READING_ORACLE {}", serde_json::to_string(&json!({
            "query": request.query, "language": request.language, "limit": request.limit,
            "include_semantic_neighbors": request.include_semantic_neighbors,
            "group_by": request.group_by, "expected": oracle
        })).unwrap());
        assert_eq!(
            native, oracle,
            "entire original synthetic result differs for {}:{}",
            request.language, request.query
        );
    }
}

#[test]
fn core_normalization_alias_negatives_and_finite_pin_cohort_limits() {
    let fixture = Fixture::new();
    let normalized = fixture.packet(&fixture.request());
    let mut duplicate = fixture.request();
    duplicate.query = "  судьбы  ".into();
    duplicate.language = " RU ".into();
    duplicate.group_by = vec!["speaker".into(), "speaker".into(), "formula".into()];
    assert_eq!(fixture.packet(&duplicate), normalized);
    let mut negative = fixture.request();
    negative.language = "de".into();
    negative.query = "los".into();
    assert_eq!(
        code(fixture.query(&negative, ReadingSearchBudget::local_default())),
        ReadingSearchErrorCode::InvalidRequest
    );
    let mut b = ReadingSearchBudget::local_default();
    b.max_open_files = 3;
    assert_eq!(
        code(fixture.query(&fixture.request(), b)),
        ReadingSearchErrorCode::BudgetExceeded
    );
    let mut b = ReadingSearchBudget::local_default();
    b.max_materialized_json_bytes = 64;
    assert_eq!(
        code(fixture.query(&fixture.request(), b)),
        ReadingSearchErrorCode::BudgetExceeded
    );
}

fn rebind_synthetic_reading_db(fixture: &Fixture) {
    let manifest_path = fixture.roots.analysis_root.join(READING_MANIFEST_REF);
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["private_database"]["sha256"] = json!(
        tos_foundation::Digest256::of_bytes(
            &fs::read(fixture.roots.analysis_root.join("reading #db.sqlite3")).unwrap()
        )
        .to_hex()
    );
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
}
#[test]
fn exact_anchor_stale_predecessor_alignment_authority_and_crossing_voice_controls() {
    for sql in [
        "UPDATE occurrence_spans SET exact_sha256='bad'",
        "UPDATE occurrence_spans SET context_unit_ref='other'",
        "UPDATE metadata SET value='stale' WHERE key='concept_workbench_sha256'",
        "UPDATE translation_alignments SET human_acceptance=1",
    ] {
        let fixture = Fixture::new();
        let db =
            rusqlite::Connection::open(fixture.roots.analysis_root.join("reading #db.sqlite3"))
                .unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        rebind_synthetic_reading_db(&fixture);
        assert_eq!(
            code(fixture.query(&fixture.request(), ReadingSearchBudget::local_default())),
            ReadingSearchErrorCode::CorruptSelectedCarrier,
            "{sql}"
        );
    }
    let fixture = Fixture::new();
    let db = rusqlite::Connection::open(fixture.roots.analysis_root.join("reading #db.sqlite3"))
        .unwrap();
    db.execute(
        "DELETE FROM occurrence_spans WHERE existing_occurrence_ref='old-0'",
        [],
    )
    .unwrap();
    drop(db);
    rebind_synthetic_reading_db(&fixture);
    let p = fixture.packet(&fixture.request());
    assert_eq!(p["results"][0]["speaker"]["role"], "unresolved");
    assert_eq!(
        p["results"][0]["source_occurrence_anchor_status"],
        "deferred"
    );
    let fixture = Fixture::new();
    let db = rusqlite::Connection::open(fixture.roots.analysis_root.join("reading #db.sqlite3"))
        .unwrap();
    let text: String = db
        .query_row(
            "SELECT exact_text FROM contexts WHERE context_unit_ref='ctx'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let cut = 6usize;
    for (id, start, end) in [("quoted", 0, cut), ("narrated", cut, text.chars().count())] {
        let exact = text
            .chars()
            .skip(start)
            .take(end - start)
            .collect::<String>();
        db.execute("UPDATE discourse_segments SET start_offset=?,end_offset=?,exact_text=?,exact_sha256=? WHERE segment_id=?",rusqlite::params![start as i64,end as i64,&exact,hash(&exact),id]).unwrap();
    }
    drop(db);
    rebind_synthetic_reading_db(&fixture);
    let p = fixture.packet(&fixture.request());
    assert_eq!(p["results"][0]["speaker"]["role"], "unresolved");
    assert_eq!(p["results"][0]["speaker"]["status"], "ambiguous");
    assert_eq!(
        p["results"][0]["speaker"]["candidates"],
        json!(["dwarf", "narrator"])
    );
}
#[test]
fn data_cannot_select_executable_and_unknown_schema_drift_refuses() {
    let fixture = Fixture::new_shared_root();
    let expected = fixture.packet(&fixture.request());
    let poisoned = fixture
        .roots
        .source_root
        .join("scripts/query_zarathustra_reading_workbench_v1.py");
    fs::create_dir_all(poisoned.parent().unwrap()).unwrap();
    fs::write(&poisoned, b"raise RuntimeError('data is not code')").unwrap();
    assert_eq!(fixture.packet(&fixture.request()), expected);
    let manifest_path = fixture
        .roots
        .source_root
        .join("ToS/candidate-intake/zarathustra/concept-workbench-v1/manifest.v1.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["concept_search_result_schema_sha256"] = json!("0".repeat(64));
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        code(fixture.query(&fixture.request(), ReadingSearchBudget::local_default())),
        ReadingSearchErrorCode::CorruptSelectedCarrier
    );
}
