use super::*;
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
fn monotonic_channel_measurement_preserves_source_and_supersession() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_address = dead.local_addr().unwrap();
    drop(dead);
    let worker = std::thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut request = [0u8; 2048];
            let n = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..n]);
            let (header, body) = if request.starts_with("GET /missing ") {
                (
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 7\r\nConnection: close\r\n\r\n"
                        .into(),
                    b"missing".to_vec(),
                )
            } else if request.starts_with("GET /redirect ") {
                (
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{address}/long\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    ),
                    vec![],
                )
            } else {
                (
                    "HTTP/1.1 200 OK\r\nContent-Length: 32768\r\nConnection: close\r\n\r\n".into(),
                    vec![b'x'; 32768],
                )
            };
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    let temp = temp(1001);
    let path = "ToS/source-witnesses/discovery/runs/original.json";
    fs::create_dir_all(temp.0.join(path).parent().unwrap()).unwrap();
    let urls = [
        format!("http://{address}/long"),
        format!("http://{address}/missing"),
        format!("http://{address}/redirect"),
        format!("http://{dead_address}/absent"),
    ];
    let source = json!({"discovery_id":"tos.discovery.original","record_version":4,"channels":urls.iter().enumerate().map(|(i,url)|json!({"channel_id":format!("channel-{i}"),"endpoint_url":url})).collect::<Vec<_>>(),"channel_comparison":(0..4).map(|i|json!({"channel_id":format!("channel-{i}"),"human_minutes":0,"machine_seconds":0,"notes":"Measured automatically. Timer sentinel"})).collect::<Vec<_>>()});
    let raw = render(&source).unwrap();
    fs::write(temp.0.join(path), &raw).unwrap();
    let cancel = AtomicI32::new(0);
    assert!(
        measure(
            &temp.0,
            path,
            MeasureOptions {
                output: Some(path),
                timeout_seconds: 1.,
                ..Default::default()
            },
            &cancel
        )
        .is_err()
    );
    assert!(
        measure(
            &temp.0,
            path,
            MeasureOptions {
                superseding_output: Some("new.json"),
                new_discovery_id: Some("foo"),
                supersedes_ref: Some("tos.discovery.original"),
                provenance_event_ref: Some("tos.event.discovery.next"),
                timeout_seconds: 1.,
                ..Default::default()
            },
            &cancel
        )
        .is_err()
    );
    assert!(
        measure(
            &temp.0,
            path,
            MeasureOptions {
                superseding_output: Some("new.json"),
                new_discovery_id: Some("tos.discovery.next"),
                supersedes_ref: Some("tos.discovery.wrong"),
                provenance_event_ref: Some("tos.event.discovery.next"),
                timeout_seconds: 1.,
                ..Default::default()
            },
            &cancel
        )
        .is_err()
    );
    assert!(!temp.0.join("new.json").exists());
    let next = "ToS/source-witnesses/discovery/runs/next.json";
    let receipt = measure(
        &temp.0,
        path,
        MeasureOptions {
            output: Some("receipt.json"),
            superseding_output: Some(next),
            new_discovery_id: Some("tos.discovery.next"),
            supersedes_ref: Some("tos.discovery.original"),
            provenance_event_ref: Some("tos.event.discovery.next"),
            timeout_seconds: 1.,
            ..Default::default()
        },
        &cancel,
    )
    .unwrap();
    worker.join().unwrap();
    assert_eq!(fs::read_to_string(temp.0.join(path)).unwrap(), raw);
    assert_eq!(receipt["discovery_id"], "tos.discovery.next");
    assert_eq!(receipt["discovery_ref"], next);
    for (i, outcome, bytes) in [
        (0, "success", 16384),
        (1, "http-error", 7),
        (2, "success", 16384),
        (3, "transport-error", 0),
    ] {
        let m = &receipt["measurements"][i]["measurement"];
        assert_eq!(m["clock"], "rust.std.time.Instant");
        assert_eq!(m["outcome"], outcome);
        assert_eq!(m["response_bytes_observed"], bytes);
        assert!(m["elapsed_seconds"].as_f64().unwrap() > 0.);
    }
    let next: Value = serde_json::from_slice(&fs::read(temp.0.join(next)).unwrap()).unwrap();
    assert_eq!(next["record_version"], 5);
    assert_eq!(next["supersedes_discovery_ref"], "tos.discovery.original");
    assert_eq!(
        next["provenance_event_refs"],
        json!(["tos.event.discovery.next"])
    );
    assert_eq!(next["channel_comparison"][0]["human_minutes"], 0);
    assert!(!text(&next["channel_comparison"][0]["notes"]).contains("sentinel"));
    timings(&next, &receipt, text(&receipt["discovery_ref"]), None).unwrap();
    let schema: Value = serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../ToS/contracts/open-work-channel-timing-receipt.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .unwrap()
        .validate(&receipt)
        .unwrap();
}
