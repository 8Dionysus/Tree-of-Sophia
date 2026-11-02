#![cfg(all(feature = "native", target_os = "linux", target_pointer_width = "64"))]

use std::{path::PathBuf, sync::atomic::AtomicBool, time::{Duration, Instant}};
use tos_foundation::Digest256;
use tos_validation::{
    executor::{BatchBudget, BatchCoverageExpectation, BatchUnit, BoundedSchemaExecutor, ExactWorkerIdentity, SchemaDiagnosticsOutcome, schema_diagnostics::{Caps, Status, Failure}},
    in_process::{InProcessSchemaEngine, Incomplete, InputMode, Unit},
    FormatProfile, SchemaBackendProbe, SchemaResource,
};

const ROOT: &str = "https://example.test/root";
fn resources() -> Vec<SchemaResource> {
    vec![SchemaResource { uri: ROOT.into(), raw: br##"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://example.test/root","$defs":{"number":{"type":"integer","minimum":2},"date":{"type":"string","format":"date-time"}},"type":"object","properties":{"value":{"$ref":"#/$defs/number"},"date":{"$ref":"#/$defs/date"}},"required":["value"]}"##.to_vec() }]
}

#[test]
fn finite_worker_and_in_process_diagnostics_match() {
    let path = PathBuf::from(env!("CARGO_BIN_EXE_tos-schema-worker"));
    let worker = ExactWorkerIdentity { sha256: Digest256::of_bytes(&std::fs::read(&path).unwrap()), absolute_path: path };
    let cases = [(ROOT, br#"{"value":3}"#.as_slice()), (ROOT, br#"{"value":1,"date":"bad"}"#.as_slice()), (ROOT, br#"{"value":1,"value":3}"#.as_slice()), ("https://example.test/root#/$defs/number", b"1".as_slice()), ("https://example.test/root#/$defs/date", br#""bad""#.as_slice())];
    for profile in [FormatProfile::LegacyPythonObserved20260923, FormatProfile::AssertedSourceCandidateV1] {
        let units: Vec<_> = cases.iter().enumerate().map(|(ordinal, (root, raw))| BatchUnit { ordinal: ordinal as u64, member_id: format!("m{ordinal}"), relative_path: format!("r{ordinal}"), root_uri: (*root).into(), raw_instance: raw.to_vec() }).collect();
        let expected = BatchCoverageExpectation::from_units(&units).unwrap();
        let SchemaDiagnosticsOutcome::Complete { units: worker_reports, .. } = BoundedSchemaExecutor::evaluate_batch_with_diagnostics(&worker, &resources(), profile, units, expected, BatchBudget::laboratory()) else { panic!("worker did not complete") };
        let mut engine = InProcessSchemaEngine::new(resources(), profile).unwrap();
        let request: Vec<_> = cases.iter().map(|(root, raw)| Unit { root_uri: root, raw, input_mode: InputMode::PublishedStrict }).collect();
        let reports = engine.validate_batch(&request, Caps::CURRENT, Instant::now() + Duration::from_secs(60), &AtomicBool::new(false)).unwrap();
        for (worker, actual) in worker_reports.iter().zip(reports) {
            let expected = &worker.report;
            assert_eq!((actual.status, actual.failure, actual.total_issue_count, actual.truncated, actual.issues), (expected.status, expected.failure, expected.total_issue_count, expected.truncated, expected.issues.clone()));
        }
    }
}

#[test]
fn limits_and_cancellation_are_explicit() {
    let mut engine = InProcessSchemaEngine::new(resources(), FormatProfile::AssertedSourceCandidateV1).unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(30);
    let raw = vec![b' '; SchemaBackendProbe::MAX_INSTANCE_BYTES + 1];
    let report = engine.validate(Unit { root_uri: ROOT, raw: &raw, input_mode: InputMode::PublishedStrict }, Caps::CURRENT, deadline, &cancelled).unwrap();
    assert_eq!((report.status, report.failure), (Status::InputRejected, Failure::InputBudget));
    let deep = format!("{}0{}", "[".repeat(65), "]".repeat(65));
    let report = engine.validate(Unit { root_uri: ROOT, raw: deep.as_bytes(), input_mode: InputMode::PublishedStrict }, Caps::CURRENT, deadline, &cancelled).unwrap();
    assert_eq!((report.status, report.failure), (Status::InputRejected, Failure::InputBudget));
    assert_eq!(engine.validate_batch(&[], Caps::CURRENT, deadline, &cancelled), Err(Incomplete::BatchBudget));
    assert_eq!(engine.validate(Unit { root_uri: ROOT, raw: b"{}", input_mode: InputMode::PublishedStrict }, Caps::CURRENT, Instant::now(), &cancelled), Err(Incomplete::Deadline));
    cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(engine.validate(Unit { root_uri: ROOT, raw: b"{}", input_mode: InputMode::PublishedStrict }, Caps::CURRENT, deadline, &cancelled), Err(Incomplete::Cancelled));
}

#[test]
fn legacy_exceptional_values_match_worker_diagnostics() {
    use std::collections::BTreeMap;
    use tos_foundation::SourceRevision;
    use tos_validation::executor::ExecutorBudget;
    use tos_validation::source_cut::{CutSchemaDiagnosticsLimits, CutWorkerLimits, CutWorkerSchemaExecutor};

    let path = PathBuf::from(env!("CARGO_BIN_EXE_tos-schema-worker"));
    let worker = ExactWorkerIdentity { sha256: Digest256::of_bytes(&std::fs::read(&path).unwrap()), absolute_path: path };
    let contract = "ToS/contracts/in-process-parity.schema.json";
    let root = format!("https://tree-of-sophia.local/{contract}");
    let mut schema = resources();
    schema[0].uri = root.clone();
    schema[0].raw = String::from_utf8(schema[0].raw.clone()).unwrap().replace(ROOT, &root).replace(r#","format":"date-time""#, "").into_bytes();
    let contracts = BTreeMap::from([(contract.to_owned(), (root.clone(), Digest256::of_bytes(&schema[0].raw)))]);
    let deadline = Instant::now() + Duration::from_secs(60);
    let cancelled = AtomicBool::new(false);
    let mut worker = CutWorkerSchemaExecutor::from_selected_resources(
        SourceRevision(Digest256::of_bytes(b"cut")), contracts, schema.clone(),
        FormatProfile::LegacyPythonObserved20260923, worker, ExecutorBudget::laboratory(),
        CutWorkerLimits { max_receipts: 8, max_receipt_bytes: 200000 }, deadline, &cancelled,
    ).unwrap();
    worker.enable_diagnostics_v2(CutSchemaDiagnosticsLimits { max_total_issues: 128, max_total_report_bytes: 200000, max_total_state_bytes: 16 * 1024 * 1024 }).unwrap();
    let mut engine = InProcessSchemaEngine::new(schema, FormatProfile::LegacyPythonObserved20260923).unwrap();
    for raw in [br#"{"value":NaN}"#.as_slice(), br#"{"value":Infinity}"#, br#"{"value":-Infinity}"#, br#"{"value":1,"value":3}"#, br#"{"value":3,"date":"\uD800"}"#] {
        let expected = worker.check_diagnostics_v2("record.json", raw, contract, deadline, &cancelled);
        let actual = engine.validate(Unit { root_uri: &root, raw, input_mode: InputMode::LegacyPythonObserved }, Caps::CURRENT, deadline, &cancelled).unwrap();
        if expected.is_err() {
            assert_eq!((actual.status, actual.failure), (Status::Indeterminate, Failure::UnsupportedInputSemantics));
            continue;
        }
        let expected = expected.unwrap();
        let r = expected.report();
        assert_eq!((actual.status, actual.failure, actual.total_issue_count, actual.truncated, actual.issues), (r.status, r.failure, r.total_issue_count, r.truncated, r.issues.clone()), "{}", String::from_utf8_lossy(raw));
    }
}

#[test]
fn in_process_evidence_round_trips_rejections_and_truncation() {
    use tos_validation::in_process::Report;
    let mut engine = InProcessSchemaEngine::new(resources(), FormatProfile::AssertedSourceCandidateV1).unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(30);
    let caps = Caps { max_issues_per_unit: 1, ..Caps::CURRENT };
    for raw in [br#"{"value":1,"date":"bad"}"#.as_slice(), br#"{"value":1,"value":3}"#] {
        let report = engine.validate(Unit { root_uri: ROOT, raw, input_mode: InputMode::PublishedStrict }, caps, deadline, &cancelled).unwrap();
        let encoded = report.to_evidence_json().unwrap();
        assert_eq!(Report::from_evidence_json(&encoded), Ok(report));
        assert!(!String::from_utf8(encoded).unwrap().contains("worker_sha256"));
    }
}

#[test]
fn cut_adapter_binds_source_and_candidate_separately() {
    use std::collections::BTreeMap;
    use tos_foundation::SourceRevision;
    use tos_validation::source_cut::{CutSchemaExecutor, InProcessCutIdentity, InProcessCutSchemaExecutor};
    let schema = resources();
    let digest = Digest256::of_bytes(&schema[0].raw);
    let contracts = BTreeMap::from([("contract".into(), (ROOT.into(), digest))]);
    let revision = SourceRevision(Digest256::of_bytes(b"source revision"));
    let mut cut = InProcessCutSchemaExecutor::<String>::new(InProcessCutIdentity::Source(revision), contracts.clone(), schema.clone(), FormatProfile::AssertedSourceCandidateV1, InputMode::PublishedStrict).unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(30);
    assert_eq!(cut.selected_source_revision(), Some(revision));
    assert!(matches!(cut.schema_execution_binding(), Some(tos_foundation::SchemaExecutionBinding::InProcessV1(binding)) if binding.engine == tos_validation::in_process::ENGINE_ID));
    assert!(cut.check("row", br#"{"value":3}"#, "contract", deadline, &cancelled).unwrap());
    let mut candidate = InProcessCutSchemaExecutor::new(InProcessCutIdentity::Candidate("candidate".to_owned()), contracts, schema, FormatProfile::AssertedSourceCandidateV1, InputMode::PublishedStrict).unwrap();
    assert_eq!(candidate.selected_source_revision(), None);
    let evidence = candidate.diagnostic_for("row", "contract#/$defs/number", b"1", Caps::CURRENT, deadline, &cancelled).unwrap();
    assert_eq!(evidence.identity, InProcessCutIdentity::Candidate("candidate".to_owned()));
    assert_eq!(evidence.report.status, Status::Invalid);
    let encoded = evidence.to_evidence_json().unwrap();
    assert_eq!(tos_validation::source_cut::InProcessCutDiagnostic::<String>::from_evidence_json(&encoded).unwrap(), evidence);
}

#[test]
fn executor_native_integer_fixture_matches_in_process() {
    let root = "https://treeofsophia.local/tests/integer.schema.json";
    let schemas = vec![SchemaResource {
        uri: root.into(),
        raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://treeofsophia.local/tests/integer.schema.json","type":"integer"}"#.to_vec(),
    }];
    let path = PathBuf::from(env!("CARGO_BIN_EXE_tos-schema-worker"));
    let worker = ExactWorkerIdentity { sha256: Digest256::of_bytes(&std::fs::read(&path).unwrap()), absolute_path: path };
    let raw = [b"7".as_slice(), br#""x""#, b"7", b"7.5", b"null"];
    for profile in [FormatProfile::LegacyPythonObserved20260923, FormatProfile::AssertedSourceCandidateV1] {
        let units: Vec<_> = raw.iter().enumerate().map(|(ordinal, value)| BatchUnit { ordinal: ordinal as u64, member_id: format!("m{ordinal}"), relative_path: format!("r{ordinal}"), root_uri: root.into(), raw_instance: value.to_vec() }).collect();
        let expected = BatchCoverageExpectation::from_units(&units).unwrap();
        let SchemaDiagnosticsOutcome::Complete { units: worker_reports, .. } = BoundedSchemaExecutor::evaluate_batch_with_diagnostics(&worker, &schemas, profile, units, expected, BatchBudget::laboratory()) else { panic!("worker did not complete") };
        let mut engine = InProcessSchemaEngine::new(schemas.clone(), profile).unwrap();
        for (value, worker) in raw.iter().zip(worker_reports) {
            let actual = engine.validate(Unit { root_uri: root, raw: value, input_mode: InputMode::PublishedStrict }, Caps::CURRENT, Instant::now() + Duration::from_secs(30), &AtomicBool::new(false)).unwrap();
            let expected = worker.report;
            assert_eq!((actual.status, actual.failure, actual.total_issue_count, actual.truncated, actual.issues), (expected.status, expected.failure, expected.total_issue_count, expected.truncated, expected.issues));
        }
    }
}

#[test]
fn executor_scoped_reference_fixture_has_diagnostic_parity() {
    let root = "https://treeofsophia.local/tests/root.json";
    let target = "https://treeofsophia.local/tests/nested/types.json";
    let schemas = vec![
        SchemaResource { uri: root.into(), raw: format!(r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{root}","$defs":{{"scoped":{{"$id":"nested/child.json","properties":{{"a/b~c":{{"$ref":"types.json#/$defs/code"}}}}}},"deny":false}}}}"#).into_bytes() },
        SchemaResource { uri: target.into(), raw: format!(r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{target}","$defs":{{"code":{{"const":"owned"}}}}}}"#).into_bytes() },
    ];
    let cases = [(format!("{root}#/$defs/scoped/properties/a~1b~0c"), br#""owned""#.as_slice()), (format!("{root}#/$defs/scoped/properties/a~1b~0c"), br#""other""#.as_slice()), (format!("{root}#/$defs/deny"), b"3".as_slice())];
    let path = PathBuf::from(env!("CARGO_BIN_EXE_tos-schema-worker"));
    let worker = ExactWorkerIdentity { sha256: Digest256::of_bytes(&std::fs::read(&path).unwrap()), absolute_path: path };
    for profile in [FormatProfile::LegacyPythonObserved20260923, FormatProfile::AssertedSourceCandidateV1] {
        let units: Vec<_> = cases.iter().enumerate().map(|(i, (uri, raw))| BatchUnit { ordinal: i as u64, member_id: format!("m{i}"), relative_path: format!("r{i}"), root_uri: uri.clone(), raw_instance: raw.to_vec() }).collect();
        let expected = BatchCoverageExpectation::from_units(&units).unwrap();
        let SchemaDiagnosticsOutcome::Complete { units: worker_reports, .. } = BoundedSchemaExecutor::evaluate_batch_with_diagnostics(&worker, &schemas, profile, units, expected, BatchBudget::laboratory()) else { panic!("worker did not complete") };
        let mut engine = InProcessSchemaEngine::new(schemas.clone(), profile).unwrap();
        for ((uri, raw), worker) in cases.iter().zip(worker_reports) {
            let actual = engine.validate(Unit { root_uri: uri, raw, input_mode: InputMode::PublishedStrict }, Caps::CURRENT, Instant::now() + Duration::from_secs(30), &AtomicBool::new(false)).unwrap();
            let expected = worker.report;
            assert_eq!((actual.status, actual.failure, actual.total_issue_count, actual.truncated, actual.issues), (expected.status, expected.failure, expected.total_issue_count, expected.truncated, expected.issues));
        }
    }
}
