//! Native command-authority coverage retained from the retired Python loader.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tos_ops_mechanics_plan::validation_lanes::{self as lanes, ReleasePhase};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tos-native-lanes-{}-{unique}", std::process::id()));
        fs::create_dir_all(root.join("docs/validation")).unwrap();
        Self(root)
    }
    fn manifest(&self, value: Value) {
        fs::write(
            self.0.join("docs/validation/validation_lanes.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn repository() -> PathBuf {
    super::repository()
}
fn step(label: &str, command: &[&str]) -> Value {
    json!({"label":label,"command":command})
}

#[test]
fn sequence_preserves_order_and_arguments_and_refuses_invalid_phase_boundaries() {
    let root = Fixture::new();
    root.manifest(json!({"command_sequences":{"sample":[step("first", &["native", "a b", "{repo_root}"]),step("second", &["tool", "--check"])]}}));
    assert_eq!(
        lanes::command_sequence(&root.0, "sample").unwrap(),
        vec![
            (
                "first".into(),
                vec![
                    "native".into(),
                    "a b".into(),
                    root.0.to_str().unwrap().into()
                ]
            ),
            ("second".into(), vec!["tool".into(), "--check".into()]),
        ]
    );
    assert!(lanes::command_sequence(&root.0, "missing").is_err());
    for invalid in [
        json!([]),
        json!([null]),
        json!([{"label":"bad","command":[]}]),
        json!([{"label":"bad","command":["tool",null]}]),
    ] {
        root.manifest(json!({"command_sequences":{"sample":invalid}}));
        assert!(lanes::command_sequence(&root.0, "sample").is_err());
    }
    for valid in [
        vec![
            step("contracts", &["a"]),
            step("run tests: access", &["b"]),
            step("run tests: source", &["c"]),
        ],
        vec![step("contracts", &["a"]), step("run tests", &["b"])],
    ] {
        root.manifest(json!({"command_sequences":{"release_check":valid}}));
        let mut combined = lanes::release_steps(&root.0, ReleasePhase::Checks).unwrap();
        combined.extend(lanes::release_steps(&root.0, ReleasePhase::Tests).unwrap());
        assert_eq!(
            combined,
            lanes::release_steps(&root.0, ReleasePhase::All).unwrap()
        );
    }
    for labels in [
        vec!["other"],
        vec!["run tests: one", "later"],
        vec!["run tests", "run tests"],
        vec!["run tests: one", "run tests"],
        vec!["run tests: "],
    ] {
        root.manifest(json!({"command_sequences":{"release_check":labels.iter().map(|label|step(label,&["tool"])).collect::<Vec<_>>()}}));
        assert!(lanes::release_steps(&root.0, ReleasePhase::Checks).is_err());
        assert!(lanes::release_steps(&root.0, ReleasePhase::Tests).is_err());
    }
}

#[test]
fn maintained_manifest_check_and_selection_reject_python_commands() {
    let root = Fixture::new();
    fs::write(root.0.join("owner.md"), "fixture owner\n").unwrap();
    let commands: &[&[&str]] = &[
        &["python", "-m", "pytest"][..],
        &["python3", "-m", "pytest"],
        &["python3.14", "-m", "pytest"],
        &["python2.7", "-m", "pytest"],
        &["/usr/bin/python3", "-m", "pytest"],
        &["./.venv/bin/python3.14", "-m", "pytest"],
        &["scripts/check.py"],
        &["/opt/owner/scripts/check.py"],
    ];
    for &command in commands {
        root.manifest(json!({
            "schema_version":"tos_validation_lanes_v1",
            "owner_repo":"Tree-of-Sophia",
            "command_authority":"docs/validation/validation_lanes.json",
            "lanes":{"sample":{
                "label":"Sample",
                "layer":"fixture",
                "mode":"fixture",
                "owner_surface":"owner.md",
                "purpose":"Exercise the maintained command boundary.",
                "failure_route":"owner.md",
                "does_not_own":["source meaning"],
                "command_sequence":"sample"
            }},
            "command_sequences":{"sample":[step("legacy Python", command)]}
        }));

        let issues = lanes::validate_manifest(&root.0).unwrap();
        assert!(
            issues.iter().any(|(_, detail)| {
                detail.contains("Python commands are unsupported in maintained validation lanes")
            }),
            "manifest validator accepted argv0 {:?}",
            command[0]
        );
        let error = lanes::command_sequence(&root.0, "sample").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Python commands are unsupported"),
            "command selection accepted argv0 {:?}",
            command[0]
        );
    }
}

#[test]
fn workspace_consumers_run_before_package_tests_can_relink_products() {
    let steps = lanes::command_sequence(&repository(), "rust_workspace").unwrap();
    let mut prepared = false;
    let mut package_tests_started = false;
    let mut consumers = 0;
    for (_, command) in steps {
        let argv: Vec<_> = command.iter().map(String::as_str).collect();
        if argv.contains(&"--no-run") && argv.contains(&"--message-format=json") {
            assert!(!prepared, "prepare workspace products once");
            assert!(
                argv.windows(2)
                    .any(|pair| pair == ["--exclude", "tos-access"]),
                "prepare current conformance products without unused Access test harnesses"
            );
            prepared = true;
        } else if argv.first() == Some(&"@tos-native-growth-classes")
            || (argv.first() == Some(&"cargo")
                && argv.contains(&"test")
                && argv.contains(&"--workspace"))
        {
            assert!(prepared, "prepare products before their consumers");
            assert!(
                !package_tests_started,
                "package tests can relink prepared products"
            );
            consumers += 1;
        } else if argv.first() == Some(&"cargo") && argv.contains(&"test") {
            package_tests_started = true;
        }
    }
    assert!(prepared && consumers > 0 && package_tests_started);
}

#[test]
fn browser_behavior_runs_the_complete_native_node_playwright_suite() {
    let steps = lanes::command_sequence(&repository(), "software_browser").unwrap();
    let groups: Vec<_> = steps
        .iter()
        .filter(|(label, _)| label.starts_with("browser behavior: "))
        .collect();
    assert_eq!(groups.len(), 1);
    let argv = &groups[0].1;
    assert_eq!(
        argv.iter().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "node",
            "--test",
            "--test-reporter=tap",
            "access/e2e/test_webmcp.mjs"
        ]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn actual_native_validation_runs_with_empty_path_and_stops_at_failure_or_budget() {
    use std::process::Command;
    let root = Fixture::new();
    fs::write(root.0.join("adapter.sh"),"printf '%s\\n' \"$1\" >> trace\ncase \"$1\" in fail) exit 17;; slow) /bin/sleep 5;; esac\n").unwrap();
    root.manifest(json!({"command_sequences":{
        "sample":[step("first", &["/bin/sh","adapter.sh","first"]),step("failing", &["/bin/sh","adapter.sh","fail"]),step("later", &["/bin/sh","adapter.sh","later"])],
        "rust_workspace":[{"label":"budgeted slow","command":["/bin/sh","adapter.sh","slow"],"command_timeout_ms":500},step("later", &["/bin/sh","adapter.sh","later"])]}}));
    let exe = std::env::var_os("TOS_VALIDATION_LANES_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-validation-lanes").into());
    let invoke = |args: &[&str]| {
        Command::new(&exe)
            .arg("--repo-root")
            .arg(&root.0)
            .args(args)
            .env("PATH", "")
            .output()
            .unwrap()
    };
    let output = invoke(&["--sequence", "sample", "--run", "sample"]);
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(
        fs::read_to_string(root.0.join("trace")).unwrap(),
        "first\nfail\n"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("first: /bin/sh adapter.sh first\n"));
    assert!(stdout.contains("[ok] first\n"));
    assert!(!stdout.contains("[run] later:"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed with exit code 17"));
    fs::remove_file(root.0.join("trace")).unwrap();
    let output = invoke(&["--run", "rust_workspace", "--lane-timeout-ms", "5000"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("execution wall deadline"));
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("command_timeout_ms=500 lane_wall_cap_ms=5000")
    );
    let trace = fs::read_to_string(root.0.join("trace")).unwrap_or_default();
    assert!(trace.is_empty() || trace == "slow\n");
    let unknown = invoke(&["--unknown"]);
    assert_eq!(unknown.status.code(), Some(2));
}

#[cfg(target_os = "linux")]
#[test]
fn actual_native_cargo_stream_binds_exact_products_and_rejects_escape_and_mutation() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    use tos_foundation::Digest256;
    let root = Fixture::new();
    let target = root.0.join("cargo-target");
    fs::create_dir_all(target.join("debug/deps")).unwrap();
    let access = target.join("debug/tos-access");
    let case = target.join("debug/deps/conformance-fixture");
    let write_exe = |path: &Path, bytes: &[u8]| {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    };
    write_exe(&access, b"fixture prepared consumer bytes");
    write_exe(&case, b"fixture Claim publication image");
    let cargo = root.0.join("cargo");
    write_exe(&cargo,b"#!/bin/sh\ncase \" $* \" in *' --no-run '*) /bin/cat artifacts.jsonl; [ \"$MUTATE\" = yes ] && printf changed > cargo-target/debug/tos-access; exit 0;; esac\nprintf '%s\\n%s\\n%s\\n' \"$TOS_NATIVE_PREPARED_CONSUMER_BIN\" \"$TOS_NATIVE_PREPARED_CONSUMER_SHA256\" \"$TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256\" > received\n");
    let package = "path+file:///workspace/tests/conformance/rust#tos-conformance@1.0.0";
    let row = |package: &str, kind: &str, path: &Path| json!({"reason":"compiler-artifact","package_id":package,"target":{"name":"conformance","kind":[kind]},"executable":path});
    let stream = |selected: &Path| {
        [
            row(
                "path+file:///workspace/tos-access#tos-access@1.0.0",
                "test",
                Path::new("/unrelated"),
            ),
            row(package, "bin", Path::new("/unrelated")),
            row(package, "test", selected),
        ]
        .iter()
        .map(|v| v.to_string() + "\n")
        .collect::<String>()
    };
    fs::write(root.0.join("artifacts.jsonl"), stream(&case)).unwrap();
    root.manifest(json!({"command_sequences":{"rust_workspace":[
        {"label":"prepare","command":[cargo,"test","--workspace","--no-run","--message-format=json"]},
        {"label":"consume","command":[cargo,"test","--workspace"]}
    ]}}));
    let exe = std::env::var_os("TOS_VALIDATION_LANES_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-validation-lanes").into());
    let invoke = |mutate: bool| {
        Command::new(&exe)
            .arg("--repo-root")
            .arg(&root.0)
            .args(["--run", "rust_workspace"])
            .env("CARGO_TARGET_DIR", &target)
            .env("PATH", "")
            .env("MUTATE", if mutate { "yes" } else { "no" })
            .env("TOS_NATIVE_PREPARED_CONSUMER_SHA256", "not-authority")
            .output()
            .unwrap()
    };
    let output = invoke(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.0.join("received")).unwrap(),
        format!(
            "{}\n{}\n{}\n",
            access.display(),
            Digest256::of_bytes(b"fixture prepared consumer bytes").to_hex(),
            Digest256::of_bytes(b"fixture Claim publication image").to_hex()
        )
    );
    fs::remove_file(root.0.join("received")).unwrap();
    let outside = root.0.join("outside");
    write_exe(&outside, b"outside exact Cargo target");
    fs::write(root.0.join("artifacts.jsonl"), stream(&outside)).unwrap();
    assert!(!invoke(false).status.success());
    assert!(!root.0.join("received").exists());
    fs::write(
        root.0.join("artifacts.jsonl"),
        stream(&case) + &row(package, "test", &case).to_string() + "\n",
    )
    .unwrap();
    assert!(!invoke(false).status.success());
    assert!(!root.0.join("received").exists());
    // Mutation between preparation and consumption is checked by a separate
    // authored command; successful Cargo preparation never grants future bytes.
    fs::write(root.0.join("artifacts.jsonl"), stream(&case)).unwrap();
    root.manifest(json!({"command_sequences":{"rust_workspace":[
        {"label":"prepare","command":[cargo,"test","--workspace","--no-run","--message-format=json"]},
        {"label":"mutate","command":["/bin/sh","-c","printf changed > cargo-target/debug/tos-access"]},
        {"label":"consume","command":[cargo,"test","--workspace"]}
    ]}}));
    assert!(!invoke(false).status.success());
    assert!(!root.0.join("received").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn native_verifier_consumes_installed_symlink_entries_and_preserves_command_behavior() {
    use std::{os::unix::fs::symlink, process::Command};
    let root = Fixture::new();
    let prefix = root.0.join("install");
    let bin = prefix.join("bin");
    let payload = prefix.join("software/native/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&payload).unwrap();
    for (name, executable) in [
        (
            "tos-validation-lanes",
            env!("CARGO_BIN_EXE_tos-validation-lanes"),
        ),
        ("tos-release-check", env!("CARGO_BIN_EXE_tos-release-check")),
        ("tos-software-ci", env!("CARGO_BIN_EXE_tos-software-ci")),
    ] {
        fs::copy(executable, payload.join(name)).unwrap();
        symlink(format!("../software/native/bin/{name}"), bin.join(name)).unwrap();
    }
    let out = Command::new(bin.join("tos-software-ci"))
        .arg("verify-mechanics-install")
        .arg("--repo-root")
        .arg(repository())
        .arg("--installed-prefix")
        .arg(&prefix)
        .arg("--command-entries-only")
        .arg("--lane-timeout-ms")
        .arg("30000")
        .current_dir(&root.0)
        .env("TMPDIR", &root.0)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["status"], "pass");
    assert_eq!(result["native_phase_execution"], true);
    assert_eq!(result["failed_preparation_rejected"], true);
    assert_eq!(result["products"].as_object().unwrap().len(), 3);
    let bad = Command::new(bin.join("tos-software-ci"))
        .args([
            "verify-reader-install",
            "--command-entries-only",
            "--repo-root",
        ])
        .arg(repository())
        .arg("--installed-prefix")
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
