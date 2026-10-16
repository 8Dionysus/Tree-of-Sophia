//! Historical receipt data remains readable by Rust. These are contract and
//! mutation checks, not a new admission of the old generated KAG artifact.
use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
fn receipt() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../../tests/fixtures/agent_surface_budget/receipt.json"
    ))
    .unwrap()
}
fn manifest() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../../tests/fixtures/agent_surface_budget/manifest.json"
    ))
    .unwrap()
}
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tos-receipt-contract-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn check(&self, m: &Value, r: &Value) -> Vec<Issue> {
        budget_receipt_contract_issues(
            &self.0,
            &mut RouteSources::new(&self.0).unwrap(),
            m,
            r,
            m["family_identity"]["content_digest"].as_str().unwrap(),
            "historical-receipt",
            Some(true),
            false,
            true,
            &AtomicI32::new(0),
            None,
        )
        .unwrap()
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn contains(issues: &[Issue], needle: &str) -> bool {
    issues.iter().any(|v| v.1.contains(needle))
}

#[test]
fn recorded_producer_contract_accepts_its_portable_metadata_and_refuses_mutations() {
    let r = receipt();
    let p = &r["producer_identity"];
    let pm = &p["procedure_manifest"];
    let e = &p["execution_inputs"];
    let mut issues = Vec::new();
    procedure(&mut issues, "receipt", pm, &p["files"], &p["action"]);
    file_inventory(&mut issues, "receipt", &p["files"], pm);
    runtime(&mut issues, "receipt", e, pm);
    environment(&mut issues, "receipt", &e["environment"], pm);
    non_python(
        &mut issues,
        "receipt",
        &e["non_python_inputs"],
        pm,
        &p["files"],
    );
    assert!(issues.is_empty(), "{issues:?}");
    for (pointer, value, expected) in [
        ("/interpreter", json!({}), "missing implementation"),
        (
            "/dependencies/0/name",
            json!([]),
            "name must be a non-empty string",
        ),
        ("/dependencies/0/state", json!([]), "state"),
        (
            "/dependencies/0/state",
            json!("unavailable"),
            "required runtime dependency python must be available",
        ),
        (
            "/dependencies/0/resolved_version",
            json!("2.7"),
            "does not match the captured interpreter",
        ),
        (
            "/dependencies/1/resolved_version",
            Value::Null,
            "resolved_version",
        ),
        ("/dependencies/1/artifact_bytes", json!(0), "artifact_bytes"),
        ("/dependencies/1/artifact_files", json!(0), "artifact_files"),
        (
            "/dependencies/1/path_digest",
            json!("0".repeat(64)),
            "path_digest",
        ),
        (
            "/dependencies/1/artifact_digest",
            json!("0".repeat(64)),
            "artifact_digest",
        ),
        (
            "/dependencies/3/resolved_version",
            json!("0.49.2"),
            "declared",
        ),
        (
            "/interpreter/artifact_digest",
            json!("0".repeat(64)),
            "artifact_digest",
        ),
    ] {
        let mut changed = e.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let mut issues = Vec::new();
        runtime(&mut issues, "receipt", &changed, pm);
        assert!(contains(&issues, expected), "{pointer}: {issues:?}");
    }
    let mut changed = e["environment"].clone();
    changed[0]["value_digest"] = json!("0".repeat(64));
    changed[0]["bytes"] = json!(2);
    let mut issues = Vec::new();
    environment(&mut issues, "receipt", &changed, pm);
    assert!(
        contains(&issues, "unset sentinel") && contains(&issues, "bytes must be zero"),
        "{issues:?}"
    );
    let mut changed = e["non_python_inputs"].clone();
    changed[0]["content_digest"] = json!("0".repeat(64));
    let mut issues = Vec::new();
    non_python(&mut issues, "receipt", &changed, pm, &p["files"]);
    assert!(
        contains(&issues, "does not match producer file"),
        "{issues:?}"
    );
}

#[test]
fn declared_dynamic_imports_are_data_with_bounded_identity_not_executed_code() {
    let r = receipt();
    let p = &r["producer_identity"];
    let mut pm = p["procedure_manifest"].clone();
    pm["python_import_closure"] = json!(["scripts/a.py", "scripts/b.py"]);
    let edge = json!({"kind":"module_from_spec","source":"scripts/a.py","target":"scripts/b.py"});
    for value in [None, Some(json!([])), Some(json!([edge.clone()]))] {
        if let Some(value) = value {
            pm["dynamic_imports"] = value;
        } else {
            pm.as_object_mut().unwrap().remove("dynamic_imports");
        }
        let mut issues = Vec::new();
        procedure(&mut issues, "receipt", &pm, &p["files"], &p["action"]);
        assert!(issues.is_empty(), "{issues:?}");
    }
    for value in [
        json!({}),
        json!([null]),
        json!([{"kind":"eval","source":"scripts/a.py","target":"scripts/b.py"}]),
        json!([{"kind":"module_from_spec","source":"scripts/a.py","target":"../b.py"}]),
        json!([{"kind":"module_from_spec","source":"scripts/outside.py","target":"scripts/b.py"}]),
        json!([{"kind":"module_from_spec","source":"scripts/a.py","target":"scripts/b.py","extra":true}]),
    ] {
        pm["dynamic_imports"] = value;
        let mut issues = Vec::new();
        procedure(&mut issues, "receipt", &pm, &p["files"], &p["action"]);
        assert!(
            contains(&issues, "dynamic import") || contains(&issues, "dynamic_imports"),
            "{issues:?}"
        );
    }
}

#[test]
fn complete_receipt_binds_source_history_action_manifest_and_identity_tuple() {
    let root = Root::new();
    let m = manifest();
    let r = receipt();
    let baseline = root.check(&m, &r);
    // An isolated contract fixture must refuse live admission: no source tree,
    // generated shards or selected external producer runtime are present.
    assert!(
        contains(&baseline, "candidate seal cannot be recomputed"),
        "{baseline:?}"
    );
    assert!(
        contains(&baseline, "live Python runtime verification is unsupported"),
        "{baseline:?}"
    );
    for (pointer, value, expected) in [
        (
            "/head_source_snapshot",
            json!(format!("sha256:{}", "0".repeat(64))),
            "source snapshot does not match the family manifest",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/base_ref",
            json!("a".repeat(40)),
            "command target base_ref does not match receipt base_ref",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/history_ref",
            json!("a".repeat(40)),
            "command target history_ref does not match receipt base_ref",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/event_history_ref",
            json!("a".repeat(40)),
            "command target event_history_ref does not match receipt base_ref",
        ),
        (
            "/producer_identity/execution_inputs/action_inputs/history-ref/value_digest",
            json!("0".repeat(64)),
            "action input history-ref value_digest does not match receipt base_ref",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/output",
            json!("wrong.json"),
            "command target output does not match canonical family output",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/repo_root/path_digest",
            json!("0".repeat(64)),
            "command target repo_root path_digest does not match canonical owner root",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/family_mode",
            json!("other"),
            "command target family_mode must be 'segmented'",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/artifact_root",
            json!({"path":"unrelated"}),
            "command target artifact_root must be null",
        ),
        (
            "/producer_identity/execution_inputs/command_targets/externalized",
            json!(true),
            "command target externalized must be false",
        ),
        (
            "/producer_identity/identity_digest",
            json!("0".repeat(64)),
            "identity digest does not match its identity material",
        ),
        (
            "/producer_identity/source_digest",
            json!("0".repeat(64)),
            "source digest does not match its files",
        ),
        (
            "/producer_identity/contract_version",
            json!(17),
            "producer identity contract is unsupported",
        ),
        (
            "/producer_identity/revision_binding",
            json!("wrong"),
            "revision binding does not match its contract version",
        ),
        (
            "/producer_identity/execution_inputs/schema_version",
            json!("wrong"),
            "runtime-input schema does not match its contract version",
        ),
        (
            "/producer_identity/procedure_manifest",
            json!({}),
            "procedure manifest digest does not match execution inputs",
        ),
        (
            "/producer_identity/procedure_manifest/manifest_path",
            json!("config/not-canonical.json"),
            "manifest_path must be config/repo-local-kag-budget-producer.json",
        ),
        (
            "/producer_identity/action/content_digest",
            json!("0".repeat(64)),
            "producer action does not match its file record",
        ),
        (
            "/producer_identity/files/0/path",
            json!("bad\npath"),
            "must not contain control characters",
        ),
        (
            "/candidate_identity",
            json!([]),
            "candidate_identity must be an object",
        ),
    ] {
        assert!(
            !contains(&baseline, expected),
            "baseline already violates {expected}: {baseline:?}"
        );
        let mut changed = r.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let issues = root.check(&m, &changed);
        assert!(contains(&issues, expected), "{pointer}: {issues:?}");
    }
    let mut changed = r.clone();
    changed["producer_identity"]["files"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v["path"] != PM);
    assert!(contains(
        &root.check(&m, &changed),
        "procedure manifest path must identify exactly one producer file"
    ));
    let mut changed = r.clone();
    changed["producer_identity"]["files"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v["path"] != ACTION);
    assert!(contains(
        &root.check(&m, &changed),
        "action path must identify exactly one producer file"
    ));
    let mut changed = r.clone();
    changed["schema_version"] = json!(V1);
    assert!(contains(
        &root.check(&m, &changed),
        "must use the identity-bound v2 schema"
    ));
}

#[test]
fn historical_jobs_are_admitted_only_when_the_procedure_declares_them() {
    let root = Root::new();
    let m = manifest();
    let mut r = receipt();
    r["producer_identity"]["execution_inputs"]["action_inputs"]["jobs"] =
        json!({"bytes":1,"kind":"bounded-integer","state":"set","value_digest":sha256_bytes(b"3")});
    r["producer_identity"]["execution_inputs"]["command_targets"]["jobs"] = json!("3");
    let issues = root.check(&m, &r);
    assert!(contains(&issues, "has unexpected jobs"), "{issues:?}");
    r["producer_identity"]["procedure_manifest"]["action_inputs"]
        .as_array_mut()
        .unwrap()
        .push(json!("jobs"));
    let issues = root.check(&m, &r);
    assert!(!contains(&issues, "has unexpected jobs"), "{issues:?}");
    assert!(
        !contains(
            &issues,
            "action input jobs value_digest does not match command target jobs"
        ),
        "{issues:?}"
    );
    r["producer_identity"]["execution_inputs"]["command_targets"]["jobs"] = json!("4");
    assert!(contains(
        &root.check(&m, &r),
        "jobs must be a bounded integer in [1, 3]"
    ));
}

#[test]
fn runtime_version_constraints_and_descriptor_paths_remain_bounded() {
    for (constraint, version, expected) in [
        (">=3.11", vec![3, 10, 9], false),
        (">=3.11", vec![3, 14, 0], true),
        ("~=3.11", vec![4, 0, 0], false),
        ("~=3.11.2", vec![3, 12, 0], false),
        (">=3.11,<4", vec![3, 12, 2], true),
        ("!=3.11.1", vec![3, 11, 1], false),
    ] {
        assert_eq!(
            satisfies(
                constraint,
                &version.into_iter().map(BigInt::from).collect::<Vec<_>>()
            ),
            Some(expected)
        );
    }
    assert!(!constraint_supported("arbitrary-code"));
    assert!(!constraint_supported("===3.11"));
    let mut m = manifest();
    m["segments"][0]["path"] = json!("../outside.jsonl");
    assert!(descriptor_paths(&m, false).is_err());
}
