use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tos_compiler::source_registry as codec;
use tos_ops_mechanics_plan::open_work_queue::QUEUE;
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
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_tos-open-work-queue")));
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
