//! Receiving CLI parity against frozen pre-migration owner scripts.
//! The oracle is test-only; maintained commands never invoke it as fallback.
#![cfg(target_os = "linux")]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};
const GENERATOR: &[u8] = include_bytes!(
    "../../../../tests/fixtures/decision_records_oracle/generate_decision_indexes.py"
);
const VALIDATOR: &[u8] = include_bytes!(
    "../../../../tests/fixtures/decision_records_oracle/validate_decision_records.py"
);
const INDEXES: &[&str] = &[
    "README.md",
    "by-number.md",
    "by-date.md",
    "by-surface.md",
    "by-tos-layer.md",
    "by-tree-class.md",
    "by-guard.md",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tos-decision-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let f = Self(root);
        f.write("scripts/generate_decision_indexes.py", GENERATOR);
        f.write("scripts/validate_decision_records.py", VALIDATOR);
        f.write("docs/decisions/AGENTS.md", b"# Owner\n");
        f.write("docs/decisions/README.md", b"# Entry\n");
        f.write("docs/decisions/TEMPLATE.md", b"# Template\n");
        f.write(
            "docs/decisions/indexes/index_contract.yaml",
            include_bytes!("../../../../docs/decisions/indexes/index_contract.yaml"),
        );
        f.record(1, "2026-06-04", "Alpha, alpha, Θέμα");
        f.record(2, "20260605", "Beta");
        f.record(3, "2026-W23-6", "Beta, none");
        f
    }
    fn write(&self, p: &str, b: &[u8]) {
        let path = self.0.join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b).unwrap();
    }
    fn record(&self, n: u32, date: &str, surface: &str) {
        let text = format!(
            "# Decision {n} — source\n\n## Index Metadata\n\n- Decision ID: TOS-D-{n:04}\n- Original date: {date}\n- Surface classes: {surface}\n- ToS layers: docs\n- Tree classes: none\n- Guard families: source-first authority\n- Posture: accepted\n\n## Rationale\n\nOwned source.\n"
        );
        self.write(
            &format!("docs/decisions/TOS-D-{n:04}-example.md"),
            text.as_bytes(),
        );
    }
    fn oracle(&self, builder: bool, check: bool) -> Output {
        let mut c = Command::new("/usr/bin/python3");
        c.arg("-B").arg(self.0.join(if builder {
            "scripts/generate_decision_indexes.py"
        } else {
            "scripts/validate_decision_records.py"
        }));
        if builder && check {
            c.arg("--check");
        }
        c.output().unwrap()
    }
    fn native(&self, builder: bool, check: bool) -> Output {
        let executable = std::env::var_os("TOS_DECISION_RECORDS_TEST_EXECUTABLE")
            .or_else(|| std::env::var_os("TOS_MECHANICS_TEST_EXECUTABLE"))
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
        let mut c = Command::new(executable);
        c.arg("--repo-root").arg(&self.0).arg(if builder {
            "--decision-index-build"
        } else {
            "--decision-records-validate"
        });
        if builder && check {
            c.arg("--check");
        }
        c.output().unwrap()
    }
    fn compare(&self, builder: bool, check: bool) -> Output {
        let oracle = self.oracle(builder, check);
        let native = self.native(builder, check);
        assert_eq!(
            native.status.code(),
            oracle.status.code(),
            "native stderr:{}\noracle stderr:{}",
            String::from_utf8_lossy(&native.stderr),
            String::from_utf8_lossy(&oracle.stderr)
        );
        assert_eq!(native.stdout, oracle.stdout, "stdout parity");
        assert_eq!(native.stderr, oracle.stderr, "stderr parity");
        native
    }
    fn index(&self, p: &str) -> PathBuf {
        self.0.join("docs/decisions/indexes").join(p)
    }
    fn clear_indexes(&self) {
        for p in INDEXES {
            let _ = fs::remove_file(self.index(p));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn receiving_cli_preserves_record_validation_and_all_seven_generated_bytes() {
    let f = Fixture::new();
    let built = f.oracle(true, false);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stdout)
    );
    assert!(f.compare(false, false).status.success());
    assert!(f.compare(true, true).status.success());
    let expected = INDEXES
        .iter()
        .map(|p| fs::read(f.index(p)).unwrap())
        .collect::<Vec<_>>();
    f.clear_indexes();
    let native = f.native(true, false);
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert!(native.stdout.is_empty());
    assert!(native.stderr.is_empty());
    for (p, expected) in INDEXES.iter().zip(expected) {
        assert_eq!(
            fs::read(f.index(p)).unwrap(),
            expected,
            "byte parity for {p}"
        );
    }
    assert!(f.compare(true, true).status.success());
}
#[test]
fn receiving_cli_preserves_stale_missing_and_typed_source_failure_order() {
    let f = Fixture::new();
    assert!(f.oracle(true, false).status.success());
    fs::write(f.index("by-date.md"), b"stale\n").unwrap();
    fs::remove_file(f.index("by-guard.md")).unwrap();
    let stale = f.compare(true, true);
    assert_eq!(stale.status.code(), Some(1));
    assert!(String::from_utf8(stale.stdout).unwrap().contains("Stale decision indexes:\n- docs/decisions/indexes/by-date.md\n- docs/decisions/indexes/by-guard.md\n"));
    let p = f.0.join("docs/decisions/TOS-D-0002-example.md");
    let text = fs::read_to_string(&p)
        .unwrap()
        .replace("TOS-D-0002", "TOS-D-0001")
        .replace("20260605", "2026-02-29");
    fs::write(p, text).unwrap();
    f.write("docs/decisions/foreign.md", b"# Unowned root Markdown\n");
    f.write("docs/decisions/nested/extra.txt", b"Unmodeled\n");
    let record = f.compare(false, false);
    assert_eq!(record.status.code(), Some(1));
    let body = String::from_utf8(record.stdout).unwrap();
    assert!(body.starts_with("Decision record validation failed.\n"));
    assert!(body.contains("duplicate Decision ID also used by"));
    assert!(body.contains("Original date must use YYYY-MM-DD"));
    assert!(body.contains("unmodeled decision-lane surface"));
    assert_eq!(f.compare(true, true).status.code(), Some(1));
}
#[test]
fn receiving_cli_preserves_empty_lane_contract_and_modeled_surface_boundaries() {
    let f = Fixture::new();
    for n in 1..=3 {
        fs::remove_file(f.0.join(format!("docs/decisions/TOS-D-{n:04}-example.md"))).unwrap();
    }
    f.write("docs/decisions/non-record.md", b"# foreign\n");
    for (entry, diagnostic) in [
        (
            "docs/decisions/non-record.md",
            "modeled_surfaces must not include root non-record Markdown",
        ),
        (
            "../foreign.txt",
            "modeled_surfaces entry must be a normalized repo-relative path",
        ),
        (
            "outside/file.txt",
            "modeled_surfaces entry must live under docs/decisions",
        ),
        (
            "docs/decisions/missing.txt",
            "modeled_surfaces entry does not exist",
        ),
    ] {
        f.write(
            "docs/decisions/indexes/index_contract.yaml",
            format!("modeled_surfaces:\n  - {entry}\n").as_bytes(),
        );
        let out = f.compare(false, false);
        let body = String::from_utf8(out.stdout).unwrap();
        assert!(body.contains("no canonical decision records found"));
        assert!(body.contains("no decision records available for validation"));
        assert!(body.contains(diagnostic));
        assert_eq!(f.compare(true, true).status.code(), Some(1));
    }
}
#[test]
fn wrappers_select_native_before_importing_owner_helpers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let f = Fixture::new();
    for (name, bytes) in [
        (
            "validate_decision_records.py",
            fs::read(root.join("scripts/validate_decision_records.py")).unwrap(),
        ),
        (
            "generate_decision_indexes.py",
            fs::read(root.join("scripts/generate_decision_indexes.py")).unwrap(),
        ),
    ] {
        f.write(&format!("scripts/{name}"), &bytes);
    }
    let executable = std::env::var_os("TOS_DECISION_RECORDS_TEST_EXECUTABLE")
        .or_else(|| std::env::var_os("TOS_MECHANICS_TEST_EXECUTABLE"))
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let built = Command::new("/usr/bin/python3")
        .arg("-B")
        .arg(f.0.join("scripts/generate_decision_indexes.py"))
        .env("TOS_OPS_MECHANICS_EXECUTOR", &executable)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    fs::write(
        f.0.join("scripts/generate_decision_indexes.py"),
        b"raise RuntimeError('owner helpers must not load for validator CLI')\n",
    )
    .unwrap();
    let validated = Command::new("/usr/bin/python3")
        .arg("-B")
        .arg(f.0.join("scripts/validate_decision_records.py"))
        .env("TOS_OPS_MECHANICS_EXECUTOR", &executable)
        .output()
        .unwrap();
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
    assert_eq!(validated.stdout, b"[ok] decision records validated\n");
}
