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
    assert_eq!(engine.validate_batch(&[], Caps::CURRENT, deadline, &cancelled), Err(Incomplete::BatchBudget));
    cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(engine.validate(Unit { root_uri: ROOT, raw: b"{}", input_mode: InputMode::PublishedStrict }, Caps::CURRENT, deadline, &cancelled), Err(Incomplete::Cancelled));
}
