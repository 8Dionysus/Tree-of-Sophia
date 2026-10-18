//! Standalone ToS stats publication, exact packet binding and failure preservation.
#[cfg(target_os = "linux")]
#[test]
fn stats_release_uses_only_tos_inputs_and_preserves_last_success_on_invalid_source() {
    use serde_json::{Value, json};
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tos_ops_mechanics_plan::stats_release::{SOURCE_KIND, SOURCE_PATHS};
    let root = std::env::temp_dir().join(format!(
        "tos-stats-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = root.join("source");
    let release = root.join("release");
    // Software tests exercise the export protocol, independently of the
    // current authored observation and the checkout's sparse input selection.
    let packet_ref = "stats/packets/table-i-prepared-dossier-route-ratio.reference.json";
    let port_document: Value =
        serde_json::from_slice(include_bytes!("fixtures/stats-export/port.json")).unwrap();
    let packet_document: Value =
        serde_json::from_slice(include_bytes!("fixtures/stats-export/packet.json")).unwrap();
    for name in SOURCE_PATHS {
        let path = source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let raw = match name {
            "stats/port.manifest.json" => serde_json::to_vec(&port_document).unwrap(),
            _ => b"# Synthetic ToS stats export fixture\n".to_vec(),
        };
        fs::write(path, raw).unwrap();
    }
    fs::create_dir_all(source.join("stats/packets")).unwrap();
    fs::write(
        source.join(packet_ref),
        serde_json::to_vec(&packet_document).unwrap(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_tos-stats-release");
    let build = || {
        Command::new(binary)
            .args(["build", "--source-root"])
            .arg(&source)
            .arg("--release-root")
            .arg(&release)
            .env("PATH", root.join("no-tools"))
            .output()
            .unwrap()
    };
    let status = |revision: &str| -> Value {
        let out = Command::new(binary)
            .args(["status", "--release-root"])
            .arg(&release)
            .args(["--expected-revision", revision])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    assert!(status(&"0".repeat(64))["state"].is_null());
    assert!(!release.exists());
    let first = build();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first["source_kind"], SOURCE_KIND);
    let revision = first["source_revision"].as_str().unwrap();
    let identity = first["export_revision"].as_str().unwrap();
    assert_eq!(status(revision)["export_revision"], identity);
    let repeated = build();
    assert!(
        repeated.status.success(),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&repeated.stdout).unwrap(),
        first
    );
    let packet = source.join("stats/packets/table-i-prepared-dossier-route-ratio.reference.json");
    let raw = fs::read(&packet).unwrap();
    let mut altered: Value = serde_json::from_slice(&raw).unwrap();
    altered["value"]["denominator"] = serde_json::json!(0);
    fs::write(&packet, serde_json::to_vec(&altered).unwrap()).unwrap();
    assert!(!build().status.success());
    assert_eq!(status(revision)["export_revision"], identity);
    fs::write(&packet, &raw).unwrap();
    let port = source.join("stats/port.manifest.json");
    let port_raw = fs::read(&port).unwrap();
    let mut port_changed: Value = serde_json::from_slice(&port_raw).unwrap();
    port_changed["measurements"][0]["contract_version"] = Value::Null;
    altered = serde_json::from_slice(&raw).unwrap();
    altered["contract_version"] = Value::Null;
    fs::write(&port, serde_json::to_vec(&port_changed).unwrap()).unwrap();
    fs::write(&packet, serde_json::to_vec(&altered).unwrap()).unwrap();
    assert!(!build().status.success());
    assert_eq!(status(revision)["export_revision"], identity);
    fs::write(&port, port_raw).unwrap();
    fs::write(&packet, &raw).unwrap();
    let validate = || {
        Command::new(binary)
            .args(["validate", "--port"])
            .arg(&port)
            .env("PATH", root.join("no-tools"))
            .output()
            .unwrap()
    };
    for bad in [
        json!({"kind":"ratio","unit":"1"}),
        json!({"status":"invalid","kind":"ratio","unit":"1"}),
        json!({"status":"observed","kind":"ratio","unit":"1"}),
        json!({"status":"observed","kind":"count","unit":"1","numerator":1,"denominator":2,"number":0.5}),
        json!({"status":"observed","kind":"ratio","unit":"wrong","numerator":1,"denominator":2,"number":0.5}),
        json!({"status":"unknown","kind":"ratio","unit":"1"}),
        json!({"status":"unknown","kind":"ratio","unit":"1","reason":"unknown value with current freshness"}),
        json!({"status":"unknown","kind":"ratio","unit":"1","reason":"input unavailable","number":0}),
        json!({"status":"stale","kind":"ratio","unit":"1","reason":"old observation","number":0.5}),
    ] {
        altered = serde_json::from_slice(&raw).unwrap();
        altered["value"] = bad;
        if altered["value"]["status"] == "stale" {
            altered["posture"]["freshness"] = json!("stale");
        }
        fs::write(&packet, serde_json::to_vec(&altered).unwrap()).unwrap();
        assert!(!validate().status.success());
        assert!(!build().status.success());
        assert_eq!(status(revision)["export_revision"], identity);
    }
    for (pointer, value) in [
        ("/progress/state", json!("partial")),
        ("/progress/completed", json!(1)),
        ("/progress/total", json!(3)),
        ("/uncertainty/method", json!("bootstrap")),
        ("/uncertainty/status", json!("estimated")),
        ("/uncertainty/sample_size", json!(1)),
        (
            "/uncertainty",
            json!({"status":"not_applicable","method":"not_applicable","sample_size":2,"lower":0.1,"upper":0.9}),
        ),
        ("/window/end", json!("2026-10-17T00:00:01Z")),
        (
            "/provenance/evidence_refs/0/ref",
            json!("/private/operator/evidence.json"),
        ),
        (
            "/provenance/evidence_refs/0/ref",
            json!(".aoa/sessions/raw-session.jsonl"),
        ),
    ] {
        let mut changed = packet_document.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        fs::write(&packet, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(!validate().status.success(), "accepted {pointer}");
        assert!(!build().status.success(), "published {pointer}");
        assert_eq!(status(revision)["export_revision"], identity);
    }
    fs::write(&packet, serde_json::to_vec(&packet_document).unwrap()).unwrap();
    for (pointer, value) in [
        ("/measurements/0/uncertainty/required", json!(true)),
        ("/measurements/0/uncertainty/methods", json!(["bootstrap"])),
        ("/measurements/0/aggregation/operator", json!("sum")),
    ] {
        let mut changed = port_document.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        fs::write(&port, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(!validate().status.success(), "accepted {pointer}");
        assert!(!build().status.success(), "published {pointer}");
        assert_eq!(status(revision)["export_revision"], identity);
    }
    let mut zero_port = port_document.clone();
    let mut zero = packet_document.clone();
    zero["value"]["numerator"] = json!(0);
    zero["value"]["number"] = json!(0.0);
    fs::write(&port, serde_json::to_vec(&zero_port).unwrap()).unwrap();
    fs::write(&packet, serde_json::to_vec(&zero).unwrap()).unwrap();
    assert!(validate().status.success());
    zero_port["measurements"][0]["missingness"]["zero_is_observation"] = json!(false);
    fs::write(&port, serde_json::to_vec(&zero_port).unwrap()).unwrap();
    assert!(!validate().status.success());
    let mut foreign_port = port_document.clone();
    let mut foreign_packet = packet_document.clone();
    for value in [
        &mut foreign_port["measurements"][0]["measurement_id"],
        &mut foreign_packet["measurement_id"],
    ] {
        *value = json!("Unrelated-Owner/synthetic-ratio");
    }
    foreign_port["exports"][0]["measurement_id"] = json!("Unrelated-Owner/synthetic-ratio");
    fs::write(&port, serde_json::to_vec(&foreign_port).unwrap()).unwrap();
    fs::write(&packet, serde_json::to_vec(&foreign_packet).unwrap()).unwrap();
    assert!(!validate().status.success());
    fs::write(&port, serde_json::to_vec(&port_document).unwrap()).unwrap();
    for state in ["missing", "unknown", "stale"] {
        altered = serde_json::from_slice(&raw).unwrap();
        altered["value"] = json!({"status":state,"kind":"ratio","unit":"1","reason":"synthetic input unavailable"});
        if state == "stale" {
            altered["posture"]["freshness"] = json!("stale");
        } else if state == "unknown" {
            altered["posture"]["freshness"] = json!("unknown");
        }
        fs::write(&packet, serde_json::to_vec(&altered).unwrap()).unwrap();
        assert!(validate().status.success());
    }
    altered["value"] = json!({"status":"stale","kind":"ratio","unit":"1","reason":"old observation","number":0.5,"numerator":1,"denominator":2});
    fs::write(&packet, serde_json::to_vec(&altered).unwrap()).unwrap();
    assert!(!validate().status.success());
    assert!(!build().status.success());
    assert_eq!(status(revision)["export_revision"], identity);
    let mut partial = packet_document.clone();
    partial["value"] =
        json!({"status":"unknown","kind":"ratio","unit":"1","reason":"census incomplete"});
    partial["posture"]["freshness"] = json!("unknown");
    partial["sample"]["size"] = json!(1);
    partial["uncertainty"]["sample_size"] = json!(1);
    partial["progress"] = json!({"state":"partial","completed":1,"total":2});
    fs::write(&packet, serde_json::to_vec(&partial).unwrap()).unwrap();
    assert!(validate().status.success());
    let mut pending_port = port_document.clone();
    pending_port["measurements"][0]["uncertainty"]["methods"] = json!(["not_estimated"]);
    partial["uncertainty"]["status"] = json!("not_estimated");
    partial["uncertainty"]["method"] = json!("not_estimated");
    fs::write(&port, serde_json::to_vec(&pending_port).unwrap()).unwrap();
    fs::write(&packet, serde_json::to_vec(&partial).unwrap()).unwrap();
    assert!(validate().status.success());
    partial["progress"]["completed"] = json!(2);
    fs::write(&packet, serde_json::to_vec(&partial).unwrap()).unwrap();
    assert!(!validate().status.success());
    fs::write(&port, serde_json::to_vec(&port_document).unwrap()).unwrap();
    for posture in [
        json!({"raw_content_included":false,"live_state":null,"privacy":"public","freshness":"current"}),
        json!({"raw_content_included":false,"live_state":"live","privacy":"public","freshness":"current"}),
        json!({"raw_content_included":false,"live_state":"invalid","privacy":"public","freshness":"current"}),
        json!({"raw_content_included":false,"live_state":"reference","privacy":"sensitive","freshness":"current"}),
        json!({"raw_content_included":false,"live_state":"reference","privacy":"public","freshness":null}),
    ] {
        altered = serde_json::from_slice(&raw).unwrap();
        altered["posture"] = posture;
        fs::write(&packet, serde_json::to_vec(&altered).unwrap()).unwrap();
        assert!(!validate().status.success());
        assert!(!build().status.success());
        assert_eq!(status(revision)["export_revision"], identity);
    }
    fs::write(&packet, raw).unwrap();
    for option in ["--python", "--stats-root"] {
        assert!(
            !Command::new(binary)
                .args(["validate", "--port"])
                .arg(source.join("stats/port.manifest.json"))
                .arg(option)
                .arg("/unselected/foreign-runtime")
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let original_port = fs::read(&port).unwrap();
    for posture in [Value::Null, json!("live"), json!("declaration_only")] {
        let mut changed = port_document.clone();
        changed["exports"][0]["posture"] = posture;
        fs::write(&port, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(!validate().status.success());
        assert!(!build().status.success());
        assert_eq!(status(revision)["export_revision"], identity);
    }
    fs::write(&port, &original_port).unwrap();
    // The embedded format owns complete field presence/type checks, beyond
    // the export-specific ratio and custody bindings above.
    for field in [
        "status",
        "questions",
        "central_contract_refs",
        "measurements",
        "exports",
    ] {
        let mut changed = port_document.clone();
        changed.as_object_mut().unwrap().remove(field);
        fs::write(&port, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(!validate().status.success());
    }
    fs::write(&port, original_port).unwrap();
    for field in [
        "observed_at",
        "object_ref",
        "population",
        "sample",
        "window",
        "uncertainty",
        "provenance",
        "reporting_rule",
        "posture",
        "progress",
    ] {
        let mut changed = packet_document.clone();
        changed.as_object_mut().unwrap().remove(field);
        fs::write(&packet, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(!validate().status.success());
    }
    fs::write(&packet, serde_json::to_vec(&packet_document).unwrap()).unwrap();
    let second_ref = "stats/packets/another-declared-observation.json";
    let mut second_packet = packet_document.clone();
    second_packet["observation_id"] = json!("Tree-of-Sophia:synthetic-ratio:second");
    let mut expanded_port = port_document.clone();
    expanded_port["exports"][0]["packet_refs"]
        .as_array_mut()
        .unwrap()
        .push(json!(second_ref));
    fs::write(&port, serde_json::to_vec(&expanded_port).unwrap()).unwrap();
    assert!(
        !validate().status.success(),
        "a declared missing packet must not validate"
    );
    fs::write(
        source.join(second_ref),
        serde_json::to_vec(&second_packet).unwrap(),
    )
    .unwrap();
    assert!(validate().status.success());
    let expanded = build();
    assert!(
        expanded.status.success(),
        "{}",
        String::from_utf8_lossy(&expanded.stderr)
    );
    let expanded: Value = serde_json::from_slice(&expanded.stdout).unwrap();
    let selected = release
        .join("releases")
        .join(expanded["export_revision"].as_str().unwrap());
    let checked = Command::new(binary)
        .args(["verify", "--release"])
        .arg(selected)
        .env("PATH", root.join("no-tools"))
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let checked: Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(checked["observations"].as_array().unwrap().len(), 2);
    assert!(
        checked["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == second_ref)
    );
    let mut dimensioned_port = expanded_port.clone();
    dimensioned_port["measurements"][0]["dimensions"]["allowed"] =
        json!([{"name":"cohort","max_cardinality":2,"sensitivity":"public"}]);
    let mut dimensioned_packet = packet_document.clone();
    dimensioned_packet["dimensions"] = json!({"cohort":"first"});
    second_packet["dimensions"] = json!({"cohort":"second"});
    fs::write(&port, serde_json::to_vec(&dimensioned_port).unwrap()).unwrap();
    fs::write(&packet, serde_json::to_vec(&dimensioned_packet).unwrap()).unwrap();
    fs::write(
        source.join(second_ref),
        serde_json::to_vec(&second_packet).unwrap(),
    )
    .unwrap();
    assert!(validate().status.success());
    let dimensioned = build();
    assert!(
        dimensioned.status.success(),
        "{}",
        String::from_utf8_lossy(&dimensioned.stderr)
    );
    let dimensioned: Value = serde_json::from_slice(&dimensioned.stdout).unwrap();
    dimensioned_port["measurements"][0]["dimensions"]["allowed"][0]["sensitivity"] =
        json!("sensitive");
    fs::write(&port, serde_json::to_vec(&dimensioned_port).unwrap()).unwrap();
    assert!(!validate().status.success());
    dimensioned_port["measurements"][0]["dimensions"]["allowed"][0]["sensitivity"] =
        json!("public");
    dimensioned_port["measurements"][0]["dimensions"]["allowed"][0]["max_cardinality"] = json!(1);
    fs::write(&port, serde_json::to_vec(&dimensioned_port).unwrap()).unwrap();
    assert!(!validate().status.success());
    dimensioned_port["measurements"][0]["dimensions"]["allowed"][0]["max_cardinality"] = json!(2);
    fs::write(&port, serde_json::to_vec(&dimensioned_port).unwrap()).unwrap();
    dimensioned_packet["dimensions"]["undeclared"] = json!("refuse");
    fs::write(&packet, serde_json::to_vec(&dimensioned_packet).unwrap()).unwrap();
    assert!(!validate().status.success());
    assert!(!build().status.success());
    assert_eq!(
        status(dimensioned["source_revision"].as_str().unwrap())["export_revision"],
        dimensioned["export_revision"]
    );
    let exported = release.join("releases").join(identity);
    fs::write(exported.join("extra.json"), b"{}").unwrap();
    assert!(
        !Command::new(binary)
            .args(["verify", "--release"])
            .arg(&exported)
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}
