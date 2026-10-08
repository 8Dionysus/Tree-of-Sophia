use super::*;
use std::process::Command;
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/open_work_queue")
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn temp(case: usize) -> Temp {
    let p = std::env::temp_dir().join(format!("tos-open-work-{}-{case}", std::process::id()));
    fs::create_dir(&p).unwrap();
    Temp(p)
}
fn materialize(case: &Value, root: &Path) {
    for (path, hash) in case["files"].as_object().unwrap() {
        let bytes = fs::read(fixture().join(hash.as_str().unwrap())).unwrap();
        assert_eq!(codec::digest(&bytes), hash.as_str().unwrap());
        let p = root.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
}
fn records(v: &Value) -> Result<Records> {
    if v.is_null() {
        return Ok(Records::new());
    }
    obj(v)?
        .iter()
        .map(|(id, pair)| Ok((id.clone(), (pair[0].clone(), txt(&pair[1])?.into()))))
        .collect()
}
fn located(v: &Value) -> Result<Vec<Located>> {
    arr(v)?
        .iter()
        .map(|pair| Ok((pair[0].clone(), txt(&pair[1])?.into())))
        .collect()
}
fn time_arg(v: &Value) -> Result<Option<i64>> {
    if v.is_null() {
        Ok(None)
    } else {
        optional_stamp(v.get("$timestamp").unwrap_or(v))
    }
}
fn dispatch(repo: &mut Repo<'_>, case: &Value) -> Result<Value> {
    let a = &case["args"];
    let discovery_map = records(&a["discoveries"])?;
    let event_map = records(&a["provenance_events"])?;
    match txt(&case["op"])? {
        "build_payload" => build_inner(repo),
        "build_readiness_payload" => {
            let base = build_inner(repo)?;
            readiness::project(repo, base, a["readiness_plan"]["$path"].as_str())
        }
        "_load_candidates" => Ok(json!(load_candidates(repo)?)),
        "_validate_receipt_version_timestamp_order" => {
            version_order(arr(&a["receipts"])?).map(|_| Value::Null)
        }
        "_validate_target_binding" => {
            target_binding(&a["candidate"], &a["discovery"], &a["receipt"]).map(|_| Value::Null)
        }
        "_validate_target_resolution" => target_resolution(
            repo,
            &a["target_resolution"],
            a.get("candidate"),
            a.get("discovery"),
        )
        .map(|_| Value::Null),
        "_validate_active_discovery_timings" => timings(
            &a["discovery"],
            &a["timing"],
            req(a, "discovery_ref")?,
            time_arg(&a["receipt_issued_at"])?,
        )
        .map(|_| Value::Null),
        "_validate_receipt_acquisition_closure" => receipt_closure(
            repo,
            &a["receipt"],
            &a["candidate"],
            &a["discovery"],
            &discovery_map,
            &event_map,
            a["validate_planting_chronology"].as_bool().unwrap_or(true),
            a["validate_lineage_chronology"].as_bool().unwrap_or(true),
        )
        .map(|_| Value::Null),
        "_validate_acquisition_closure" => acquisition_closure(
            repo,
            &a["acquisition"],
            req(a, "candidate_id")?,
            &a["candidate"],
            &a["discovery"],
            &discovery_map,
            &strings(
                a["receipt_context_refs"]
                    .get("$set")
                    .unwrap_or(&a["receipt_context_refs"]),
            )?,
            a["receipt_discovery_id"].as_str(),
            a["receipt_discovery_ref"].as_str(),
            &event_map,
            time_arg(&a["receipt_issued_at"])?,
            a["validate_lineage_chronology"].as_bool().unwrap_or(true),
        )
        .map(|_| Value::Null),
        "_validate_planting_refs" => planting_refs(
            repo,
            &a["refs"],
            &a["candidate"],
            &a["receipt"],
            &a["discovery"],
            &discovery_map,
            arr(&a["acquisitions"])?,
            &event_map,
            time_arg(&a["receipt_issued_at"])?,
        )
        .map(|_| Value::Null),
        "_has_independent_snapshot_witness" => snapshot_witness(
            repo,
            &a["receipt"],
            req(a, "candidate_id")?,
            req(a, "candidate_label")?,
            &discovery_map,
            &event_map,
        )
        .map(|v| json!(v)),
        "_reconstruct_pre_run_queue_sha256" => {
            let events = if a.get("provenance_events").is_none() {
                provenance(repo)?
            } else {
                event_map
            };
            reconstruct(
                repo,
                &a["current_receipt"],
                arr(&a["ordered_receipts"])?,
                &located(&a["candidates_with_locations"])?,
                &discovery_map,
                &events,
                LEGACY_PRODUCER,
            )
            .map(|v| json!(v))
        }
        other => Err(invalid(format!("unknown fixture operation {other}"))),
    }
}
fn first_difference(a: &Value, b: &Value, prefix: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = first_difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{prefix}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(d) = first_difference(a, b, format!("{prefix}/{i}")) {
                    return Some(d);
                }
            }
        }
    }
    Some(format!("{prefix}: actual={a} expected={b}"))
}
#[test]
fn historical_queue_contract_cases() {
    let manifest: Value =
        serde_json::from_slice(&fs::read(fixture().join("cases.json")).unwrap()).unwrap();
    let mut failures = vec![];
    let cancel = AtomicI32::new(0);
    for (index, case) in manifest["cases"].as_array().unwrap().iter().enumerate() {
        let temp = temp(index);
        materialize(case, &temp.0);
        let mut repo = Repo::new(&temp.0, &cancel).unwrap();
        let result = dispatch(&mut repo, case);
        if case["ok"] == false {
            if result.is_ok() {
                failures.push(format!(
                    "{index} {} {} unexpectedly accepted",
                    case["op"], case["test"]
                ));
            }
            continue;
        }
        match result {
            Err(e) => failures.push(format!("{index} {} {}: {e}", case["op"], case["test"])),
            Ok(actual) => {
                let mut expected = case["value"].clone();
                if matches!(
                    case["op"].as_str(),
                    Some("build_payload" | "build_readiness_payload")
                ) {
                    expected["generated_by"] = json!(PRODUCER);
                    if expected.get("chronological_queue_sha256").is_some() {
                        expected["chronological_queue_sha256"] =
                            build_inner(&mut repo).unwrap()["queue_sha256"].clone();
                    }
                    expected["queue_sha256"] = json!(queue_digest(&expected).unwrap());
                }
                if let Some(diff) = first_difference(&actual, &expected, String::new()) {
                    failures.push(format!("{index} {} {}: {diff}", case["op"], case["test"]));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} contract failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
#[test]
fn native_queue_cli_requires_explicit_sources_and_checks_generated_bytes() {
    let manifest: Value =
        serde_json::from_slice(&fs::read(fixture().join("cases.json")).unwrap()).unwrap();
    let case = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["op"] == "build_payload" && c["ok"] == true)
        .unwrap();
    let temp = temp(1000);
    materialize(case, &temp.0);
    let schema_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../ToS/contracts");
    fs::create_dir_all(temp.0.join("ToS/contracts")).unwrap();
    for name in [
        "open-work-candidate",
        "open-work-candidate-receipt",
        "open-work-candidate-queue",
        "open-work-channel-timing-receipt",
    ] {
        let name = format!("{name}.schema.json");
        fs::copy(
            schema_root.join(&name),
            temp.0.join("ToS/contracts").join(name),
        )
        .unwrap();
    }
    let bin = std::env::var_os("TOS_QUEUE_TEST_EXECUTABLE")
        .map(PathBuf::from)
        .expect("test launcher supplies exact queue CLI");
    let call = |operation: &str, tail: &[&str]| {
        let mut c = Command::new(&bin);
        c.arg(operation).current_dir(std::env::temp_dir());
        if operation != "--version" && operation != "--help" {
            c.arg("--source-root").arg(&temp.0);
        }
        c.args(tail).output().unwrap()
    };
    let missing = Command::new(&bin)
        .arg("build")
        .current_dir(&temp.0)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    for operation in ["--version", "--help", "build", "check", "validate"] {
        let out = call(operation, &[]);
        assert!(
            out.status.success(),
            "{operation}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = call("build", &["--dry-run"]);
    assert!(out.status.success());
    let stored = fs::read(temp.0.join(QUEUE)).unwrap();
    assert_eq!(out.stdout, stored);
    let out = call("readiness", &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["selection_mode"], "readiness");
    assert_eq!(fs::read(temp.0.join(QUEUE)).unwrap(), stored);
    let mut drift = stored;
    drift.push(b' ');
    fs::write(temp.0.join(QUEUE), drift).unwrap();
    assert!(!call("check", &[]).status.success());
    assert!(!call("validate", &[]).status.success());
}
