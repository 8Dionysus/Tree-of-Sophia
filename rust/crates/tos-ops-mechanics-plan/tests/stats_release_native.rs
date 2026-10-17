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
    let measurement = "Tree-of-Sophia/synthetic-ratio";
    let port_document = json!({
        "schema_version":"aoa_stats_local_port_v1", "contract_version":"1.0.0",
        "owner_repo":"Tree-of-Sophia",
        "evidence_posture":{"raw_content_allowed":false,"live_state":"reference_only","privacy":"public"},
        "measurements":[{"schema_version":"aoa_stats_measurement_contract_v1",
            "measurement_id":measurement,"contract_version":"1.0.0",
            "owner_repo":"Tree-of-Sophia","statistic":"ratio"}],
        "exports":[{"measurement_id":measurement,"packet_refs":[packet_ref]}]
    });
    let packet_document = json!({
        "schema_version":"aoa_stats_measurement_packet_v1",
        "contract_ref":"stats/port.manifest.json#/measurements/0","contract_version":"1.0.0",
        "measurement_id":measurement,"writer_repo":"Tree-of-Sophia",
        "observation_id":"synthetic-ratio:fixture-v1","observed_at":"2026-10-17T00:00:00Z",
        "provenance":{"source_revision":"synthetic-source:fixture-v1"},
        "posture":{"raw_content_included":false,"live_state":"reference_only"},
        "value":{"status":"observed","numerator":1,"denominator":2,"number":0.5}
    });
    for name in SOURCE_PATHS {
        let path = source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let raw = match name {
            "stats/port.manifest.json" => serde_json::to_vec(&port_document).unwrap(),
            name if name == packet_ref => serde_json::to_vec(&packet_document).unwrap(),
            _ => b"# Synthetic ToS stats export fixture\n".to_vec(),
        };
        fs::write(path, raw).unwrap();
    }
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
