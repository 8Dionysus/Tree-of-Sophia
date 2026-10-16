//! Historical golden bytes cover the old registry contract through the real CLI.
//! The fixture archives have fixed ZIP metadata; no legacy engine runs here.
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tos_compiler::source_registry::{decoded, encoded};
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/source_registry")
        .join(name)
}
fn json(path: &Path) -> Value {
    decoded(
        &fs::read(path).unwrap(),
        path.extension().is_some_and(|e| e == "gz"),
    )
    .unwrap()
}
fn call(root: &Path, operation: &str, tail: &[&str]) -> std::process::Output {
    let bin = std::env::var_os("TOS_REGISTRY_TEST_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_tos-source-registry")));
    let mut command = Command::new(bin);
    command.arg(operation).current_dir(std::env::temp_dir());
    if !matches!(operation, "coverage" | "reconcile") {
        command.arg("--packet-root").arg(root.join("packet"));
    }
    if operation != "inspect" {
        command.arg("--source-root").arg(root);
    }
    command.args(tail).output().unwrap()
}
fn success(root: &Path, operation: &str, tail: &[&str]) -> Value {
    let out = call(root, operation, tail);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    decoded(&out.stdout, false).unwrap()
}
fn input(root: &Path, n: usize) {
    let manifest = json(&fixture(&format!("manifest-{n}.json")));
    fs::write(
        root.join("packet/input.manifest.json"),
        encoded(&manifest).unwrap(),
    )
    .unwrap();
    let spec = &manifest["documents"][0];
    for (kind, name) in [
        ("xlsx", format!("workbook-{n}.xlsx")),
        ("docx", "report.docx".into()),
    ] {
        fs::copy(
            fixture(&name),
            root.join("incoming")
                .join(spec[kind]["source_path"].as_str().unwrap()),
        )
        .unwrap();
    }
}
#[test]
fn registry_cli_preserves_sources_identity_history_and_reports() {
    let lexical = json(&fixture("lexical.json"));
    for case in lexical["cases"].as_array().unwrap() {
        let raw = case["raw"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| (v[0].as_str().unwrap().to_owned(), v[1].clone()))
            .collect::<Vec<_>>();
        let actual = tos_compiler::source_registry::values::normalize(
            case["kind"].as_str().unwrap(),
            &raw,
            &lexical["profile"],
            &Default::default(),
        )
        .unwrap();
        assert_eq!(actual, case["expected"], "lexical input: {:?}", case["raw"]);
    }
    let temp =
        Temp(std::env::temp_dir().join(format!("tos-registry-native-{}", std::process::id())));
    assert!(!temp.0.exists());
    let root = &temp.0;
    fs::create_dir_all(root.join("packet")).unwrap();
    fs::create_dir(root.join("incoming")).unwrap();
    fs::create_dir_all(root.join("ToS/contracts")).unwrap();
    fs::copy(fixture("profile.json"), root.join("third-adapter.json")).unwrap();
    fs::write(
        root.join("ToS/contracts/source-registry-normalization.schema.json"),
        include_bytes!("../../../../ToS/contracts/source-registry-normalization.schema.json"),
    )
    .unwrap();
    input(root, 1);
    let incoming = root.join("incoming");
    let tail = ["--input-root", incoming.to_str().unwrap()];
    let first = success(root, "normalize", &tail);
    let first_path = root
        .join("packet/snapshots")
        .join(first["snapshot_id"].as_str().unwrap());
    let name = first["documents"][0].as_str().unwrap();
    assert_eq!(
        json(&first_path.join(name)),
        json(&fixture("document-1.json"))
    );
    assert_eq!(
        json(&first_path.join("links.json.gz")),
        json(&fixture("links-1.json"))
    );
    let original = fs::read(first_path.join(name)).unwrap();
    assert_eq!(first, success(root, "normalize", &tail));
    assert_eq!(original, fs::read(first_path.join(name)).unwrap());
    assert_eq!(first, success(root, "check", &[]));
    assert_eq!(first, success(root, "validate", &[]));
    let record = success(
        root,
        "inspect",
        &[
            "--corpus",
            "third",
            "--document",
            "new-doc",
            "--record",
            "R1",
        ],
    );
    assert_eq!(record[0]["raw_fields"][1]["value"], "Recorded name");
    assert_eq!(record[0]["reported_fields"][2]["value"]["precision"], "day");
    let id = record[0]["record_id"].clone();
    let report = success(
        root,
        "inspect",
        &["--corpus", "third", "--document", "new-doc"],
    );
    let blocks = report[0]["report_parts"][0]["blocks"].as_array().unwrap();
    assert!(
        blocks
            .iter()
            .any(|b| b["text"] == "R10" && b["record_mentions"].as_array().unwrap().is_empty())
    );
    assert!(
        blocks
            .iter()
            .any(|b| b["record_mentions"].as_array().unwrap().contains(&id))
    );
    input(root, 2);
    let second = success(root, "normalize", &tail);
    assert_ne!(first["snapshot_id"], second["snapshot_id"]);
    let second_path = root
        .join("packet/snapshots")
        .join(second["snapshot_id"].as_str().unwrap());
    assert_eq!(
        json(&second_path.join(name)),
        json(&fixture("document-2.json"))
    );
    let mut delta = json(&second_path.join("delta.json"));
    assert_eq!(delta["previous_snapshot_id"], first["snapshot_id"]);
    delta
        .as_object_mut()
        .unwrap()
        .remove("previous_snapshot_id");
    assert_eq!(delta, json(&fixture("delta.json")));
    assert_eq!(original, fs::read(first_path.join(name)).unwrap());
    assert_eq!(second, success(root, "check", &[]));
    assert_eq!(second, success(root, "validate", &[]));
    let record = success(
        root,
        "inspect",
        &[
            "--corpus",
            "third",
            "--document",
            "new-doc",
            "--record",
            "R1",
        ],
    );
    assert_eq!(record[0]["record_id"], id);
    registry_views(root);
    let source_hash = first["manifest"]["documents"][0]["xlsx"]["sha256"]
        .as_str()
        .unwrap();
    assert_eq!(
        fs::read(fixture("workbook-1.xlsx")).unwrap(),
        fs::read(
            root.join("packet/originals")
                .join(format!("{source_hash}.xlsx"))
        )
        .unwrap()
    );
    // A damaged published document is not repaired silently or accepted by check.
    let path = second_path.join(name);
    let original = fs::read(&path).unwrap();
    fs::write(&path, b"broken gzip").unwrap();
    assert!(!call(root, "check", &[]).status.success());
    assert!(!call(root, "normalize", &tail).status.success());
    fs::write(&path, original).unwrap();
    assert_eq!(second, success(root, "check", &[]));
    // Gzip wrappers may vary, but decompressed generated bytes remain exact.
    let original = fs::read(&path).unwrap();
    let raw = tos_compiler::source_registry::decompressed(&original).unwrap();
    let mut wrapper = original.clone();
    wrapper[9] = 3;
    fs::write(&path, &wrapper).unwrap();
    assert_eq!(second, success(root, "check", &[]));
    let mut drift = raw.clone();
    drift.push(b' ');
    fs::write(
        &path,
        tos_compiler::source_registry::compressed(&drift).unwrap(),
    )
    .unwrap();
    assert!(!call(root, "check", &[]).status.success());
    fs::write(&path, original).unwrap();
    // Strict profile decoding rejects duplicate keys before publication.
    let profile = root.join("third-adapter.json");
    fs::write(&profile, b"{\"version\":1,\"version\":2}").unwrap();
    assert!(!call(root, "normalize", &tail).status.success());
    assert_eq!(
        json(&root.join("packet/current.json"))["snapshot_id"],
        second["snapshot_id"]
    );
}

fn registry_views(root: &Path) {
    let source = root.join("views-source");
    let output = root.join("views-output");
    for (name, file) in json(&fixture("views-source.json")).as_object().unwrap() {
        let path = source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes = if file["gzip"] == true {
            tos_compiler::source_registry::compressed(&encoded(&file["body"]).unwrap()).unwrap()
        } else {
            file["body"].as_str().unwrap().as_bytes().to_vec()
        };
        fs::write(path, bytes).unwrap();
    }
    let out = output.to_str().unwrap();
    let packet = tos_compiler::source_registry::PACKET;
    success(&source, "reconcile", &["--output-root", out]);
    let actual = output.join(packet).join("reconciliation.current.json.gz");
    assert_eq!(json(&actual), json(&fixture("reconciliation.json")));
    fs::copy(
        actual,
        source.join(packet).join("reconciliation.current.json.gz"),
    )
    .unwrap();
    success(&source, "coverage", &["--output-root", out]);
    assert_eq!(
        json(&output.join(packet).join("coverage.current.json.gz")),
        json(&fixture("coverage.json"))
    );
    success(&source, "coverage", &["--output-root", out, "--check"]);
    let markdown = fs::read_to_string(output.join(packet).join("COVERAGE.md")).unwrap();
    assert!(markdown.contains("tos-source-registry coverage"));
    assert!(!markdown.contains(root.to_str().unwrap()));
    assert!(
        !call(
            &source,
            "coverage",
            &["--output-root", source.to_str().unwrap()]
        )
        .status
        .success()
    );
    assert!(
        !call(&source, "coverage", &["--verify-local"])
            .status
            .success()
    );
    let payload = source.join("source/item/payload/original.xml");
    fs::create_dir_all(payload.parent().unwrap()).unwrap();
    fs::write(&payload, b"exac").unwrap();
    let live = success(&source, "coverage", &["--verify-local"]);
    assert_eq!(live["targets"][0]["local_now"]["state"], "verified");
    fs::write(&payload, b"bad!").unwrap();
    let bad = call(&source, "coverage", &["--verify-local"]);
    assert!(!bad.status.success());
    let value = decoded(&bad.stdout, false).unwrap();
    assert_eq!(
        value["targets"][0]["local_now"]["files"][0]["state"],
        "fixity_mismatch"
    );
    // The portable projection is identical despite absent or damaged payload.
    success(&source, "coverage", &["--output-root", out, "--check"]);
    let discovery = source.join("source/discovery.json");
    let raw = fs::read(&discovery).unwrap();
    let mut d = json(&discovery);
    d["target"]["known_tos_refs"] = serde_json::json!(["tos.item.other-language"]);
    fs::write(&discovery, encoded(&d).unwrap()).unwrap();
    success(&source, "coverage", &["--output-root", out]);
    assert_eq!(
        json(&output.join(packet).join("coverage.current.json.gz"))["targets"][0]["status"],
        "acquired_version_needs_branch"
    );
    fs::write(discovery, raw).unwrap();
    let receipt = source.join(
        "ToS/source-witnesses/discovery/batch/translations/preparation-checkpoint-receipt.json",
    );
    let mut r = json(&receipt);
    r["manifest_sha256"] = serde_json::json!("stale");
    fs::write(receipt, encoded(&r).unwrap()).unwrap();
    assert!(
        !call(&source, "coverage", &["--output-root", out])
            .status
            .success()
    );
    let work = source.join("source/work.json");
    let mut w = json(&work);
    w["preferred_label"] = serde_json::json!("stale catalog label");
    fs::write(work, encoded(&w).unwrap()).unwrap();
    assert!(
        !call(&source, "reconcile", &["--output-root", out])
            .status
            .success()
    );
}
