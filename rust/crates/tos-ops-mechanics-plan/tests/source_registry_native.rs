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
    command
        .arg(operation)
        .arg("--packet-root")
        .arg(root.join("packet"));
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
    // Strict profile decoding rejects duplicate keys before publication.
    let profile = root.join("third-adapter.json");
    fs::write(&profile, b"{\"version\":1,\"version\":2}").unwrap();
    assert!(!call(root, "normalize", &tail).status.success());
    assert_eq!(
        json(&root.join("packet/current.json"))["snapshot_id"],
        second["snapshot_id"]
    );
}
