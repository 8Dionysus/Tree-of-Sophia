//! Native command-authority coverage retained from the retired Python loader.
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
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
fn authored() -> Value {
    serde_json::from_slice(
        &fs::read(repository().join("docs/validation/validation_lanes.json")).unwrap(),
    )
    .unwrap()
}
fn step(label: &str, command: &[&str]) -> Value {
    json!({"label":label,"command":command})
}

#[test]
fn sequence_preserves_order_and_arguments_and_refuses_invalid_phase_boundaries() {
    let root = Fixture::new();
    root.manifest(json!({"command_sequences":{"sample":[step("first", &["native", "a b", "{repo_root}"]),step("second", &["tool", "--check"])]}}));
    assert_eq!(
        lanes::command_sequence(&root.0, "sample", "").unwrap(),
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
    assert!(lanes::command_sequence(&root.0, "missing", "").is_err());
    for invalid in [
        json!([]),
        json!([null]),
        json!([{"label":"bad","command":[]}]),
        json!([{"label":"bad","command":["tool",null]}]),
    ] {
        root.manifest(json!({"command_sequences":{"sample":invalid}}));
        assert!(lanes::command_sequence(&root.0, "sample", "").is_err());
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
        let mut combined = lanes::release_steps(&root.0, "", ReleasePhase::Checks).unwrap();
        combined.extend(lanes::release_steps(&root.0, "", ReleasePhase::Tests).unwrap());
        assert_eq!(
            combined,
            lanes::release_steps(&root.0, "", ReleasePhase::All).unwrap()
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
        assert!(lanes::release_steps(&root.0, "", ReleasePhase::Checks).is_err());
        assert!(lanes::release_steps(&root.0, "", ReleasePhase::Tests).is_err());
    }
}

#[test]
fn release_software_native_test_packages_are_selected_once() {
    let root = repository();
    let sequence = lanes::command_sequence(&root, "release_check", "selected-interpreter").unwrap();
    let tests = lanes::release_steps(&root, "selected-interpreter", ReleasePhase::Tests).unwrap();
    let expected = [
        (
            "run tests: native access package".to_owned(),
            vec!["cargo", "test", "--locked", "-p", "tos-access"],
        ),
        (
            "run tests: native query package".to_owned(),
            vec!["cargo", "test", "--locked", "-p", "tos-query"],
        ),
        (
            "run tests: native source and command packages".to_owned(),
            vec![
                "cargo",
                "test",
                "--locked",
                "-p",
                "tos-command",
                "-p",
                "tos-source-store",
            ],
        ),
    ];
    assert_eq!(tests.len(), expected.len());
    for ((label, argv), (expected_label, expected_argv)) in tests.iter().zip(expected) {
        assert_eq!(label, &expected_label);
        assert_eq!(
            argv,
            &expected_argv
                .iter()
                .map(|arg| (*arg).to_owned())
                .collect::<Vec<_>>()
        );
    }
    let mut combined =
        lanes::release_steps(&root, "selected-interpreter", ReleasePhase::Checks).unwrap();
    combined.extend(tests);
    assert_eq!(combined, sequence);
    assert_eq!(
        lanes::command_sequence(&root, "software_reader", "selected-interpreter").unwrap(),
        vec![(
            "native selected access wires".into(),
            vec![
                "cargo".into(),
                "test".into(),
                "--locked".into(),
                "-p".into(),
                "tos-access".into(),
                "--test".into(),
                "selected_wires".into(),
            ],
        )]
    );
}

#[test]
fn rust_workspace_preserves_isolated_source_cases_and_exact_preparation_order() {
    let root = repository();
    let value = authored();
    let steps = value["command_sequences"]["rust_workspace"]
        .as_array()
        .unwrap();
    let labels: Vec<_> = steps.iter().map(|s| s["label"].as_str().unwrap()).collect();
    assert_eq!(
        labels[..5],
        [
            "check Rust formatting",
            "build exact native schema worker for source-cut fixtures",
            "build exact native owner CLI for process-cold fixtures",
            "build exact native prepared consumer for conformance fixtures",
            "compile exact Rust conformance test image"
        ]
    );
    let command =
        |label: &str| steps.iter().find(|s| s["label"] == label).unwrap()["command"].clone();
    assert_eq!(
        command(labels[3]),
        json!([
            "cargo",
            "build",
            "--locked",
            "-p",
            "tos-access",
            "--bin",
            "tos-access"
        ])
    );
    assert_eq!(
        command(labels[4]),
        json!([
            "cargo",
            "test",
            "--no-run",
            "--workspace",
            "--locked",
            "--message-format=json"
        ])
    );
    let workspace =
        "test Rust workspace remainder excluding conformance and isolated process-cold fixtures";
    let source = "test Rust conformance root and source families";
    let classes = "test source-owned native Growth assertion classes";
    assert_eq!(
        command(workspace),
        json!([
            "cargo",
            "test",
            "--workspace",
            "--locked",
            "--no-fail-fast",
            "--exclude",
            "tos-conformance",
            "--",
            "--nocapture",
            "@tos-native-growth-exclusions"
        ])
    );
    assert_eq!(
        command(source),
        json!([
            "cargo",
            "test",
            "--workspace",
            "--locked",
            "--test",
            "conformance",
            "--",
            "--nocapture",
            "@tos-native-growth-exclusions"
        ])
    );
    assert_eq!(command(classes), json!(["@tos-native-growth-classes"]));
    let segment = "test Rust segment conformance target";
    assert_eq!(
        command(segment),
        json!([
            "cargo",
            "test",
            "--workspace",
            "--locked",
            "--test",
            "segment-conformance",
            "--",
            "--nocapture"
        ])
    );
    let cases = [
        (
            "test isolated process-cold source revision fixture",
            "source_creation_store::revision_publication::tests::native_record_revisions_cover_fixed_handlers_process_cold_and_exact_recovery",
            "rust/crates/tos-command/src/source_record_revision_tests.rs",
        ),
        (
            "test isolated process-cold Work37 fixture",
            "source_creation_store::work_expression::tests::native_work37_cli_creates_process_cold_replays_and_recovers_exact_pending",
            "rust/crates/tos-command/src/source_work_expression_tests.rs",
        ),
        (
            "test isolated process-cold Work recovery fixture",
            "source_creation_store::work_expression::tests::real_pending_refuses_changed_dependency_then_resumes_or_rolls_back",
            "rust/crates/tos-command/src/source_work_expression_tests.rs",
        ),
        (
            "test isolated conformance Artifact fixture",
            "command_artifact_cases::native_artifact_cli_describes_prepares_creates_and_cold_replays_exact_bytes",
            "tests/conformance/rust/command_artifact_cases.rs",
        ),
        (
            "test isolated conformance Claim successor fixture",
            "command_claim_cases::claim_successor_retains_bytes_replays_current_scope_and_refuses_unissued_admission",
            "tests/conformance/rust/command_claim_cases.rs",
        ),
        (
            "test isolated conformance initial Claim fixture",
            "command_claim_cases::initial_claim_creation_publishes_five_native_files_and_cold_replays",
            "tests/conformance/rust/command_claim_cases.rs",
        ),
        (
            "test isolated conformance Collection order fixture",
            "command_claim_cases::initial_collection_order_binds_retained_version_and_cold_replays",
            "tests/conformance/rust/command_claim_cases.rs",
        ),
    ];
    let mut expected = vec![workspace, source, classes, segment];
    for (i, (label, name, path)) in cases.iter().enumerate() {
        expected.push(label);
        let argv = if i < 3 {
            json!([
                "cargo",
                "test",
                "--locked",
                "-p",
                "tos-command",
                "--lib",
                name,
                "--",
                "--exact"
            ])
        } else {
            json!([
                "cargo",
                "test",
                "--workspace",
                "--locked",
                "--test",
                "conformance",
                name,
                "--",
                "--exact",
                "--nocapture"
            ])
        };
        assert_eq!(command(label), argv);
        let source = fs::read_to_string(root.join(path)).unwrap();
        assert_eq!(
            source
                .matches(&format!(
                    "#[test]\nfn {}(",
                    name.rsplit("::").next().unwrap()
                ))
                .count(),
            1
        );
    }
    assert_eq!(
        labels
            .iter()
            .filter(|l| expected.contains(l))
            .copied()
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.starts_with("test isolated "))
            .count(),
        cases.len()
    );
    assert_eq!(
        steps
            .iter()
            .filter_map(|s| s
                .get("command_timeout_ms")
                .map(|b| (s["label"].as_str().unwrap(), b.as_u64().unwrap())))
            .collect::<Vec<_>>(),
        [
            (workspace, 900000),
            (source, 900000),
            (classes, 900000),
            (cases[0].0, 1020000)
        ]
    );
}

#[test]
fn browser_behavior_groups_cover_all_declared_top_level_scenarios_once() {
    let value = authored();
    let steps = value["command_sequences"]["software_browser"]
        .as_array()
        .unwrap();
    let groups: Vec<_> = steps
        .iter()
        .filter(|s| {
            s["label"]
                .as_str()
                .unwrap()
                .starts_with("browser behavior: ")
        })
        .collect();
    assert_eq!(groups.len(), 5);
    let source = fs::read_to_string(repository().join("access/e2e/test_webmcp.py")).unwrap();
    let expected: Vec<_> = source
        .lines()
        .filter_map(|line| {
            line.strip_prefix("def test_")
                .or_else(|| line.strip_prefix("async def test_"))
        })
        .map(|name| {
            format!(
                "access/e2e/test_webmcp.py::test_{}",
                name.split('(').next().unwrap()
            )
        })
        .collect();
    assert!(!expected.is_empty());
    let mut selected = Vec::new();
    let mut labels = BTreeSet::new();
    for group in groups {
        assert!(labels.insert(group["label"].as_str().unwrap()));
        let argv = group["command"].as_array().unwrap();
        assert_eq!(argv[1..4], [json!("-m"), json!("pytest"), json!("-q")]);
        selected.extend(argv[4..].iter().map(|v| v.as_str().unwrap().to_owned()));
    }
    assert_eq!(selected, expected);
    assert_eq!(
        selected.len(),
        selected.iter().collect::<BTreeSet<_>>().len()
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
        .args(["verify-web-host", "--repo-root"])
        .arg(repository())
        .arg("--installed-prefix")
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

#[test]
fn authored_source_foundation_lane_selects_the_native_full_audit_route() {
    let manifest = authored();
    let steps = manifest["command_sequences"]["source_witness_foundation"]
        .as_array()
        .unwrap();
    let foundation = steps
        .iter()
        .find(|step| {
            let command = step["command"].as_array();
            command.is_some_and(|command| {
                command.first().and_then(Value::as_str) == Some("tos-native-owner-command")
                    && command.get(1).and_then(Value::as_str) == Some("foundation")
            })
        })
        .unwrap();
    let command = foundation["command"].as_array().unwrap();
    assert_eq!(command[2], "--repo-root");
    assert_eq!(command[3], "{repo_root}");
    assert_eq!(command[4], "--invocation");
    assert_eq!(command[5], "{foundation_invocation}");
    assert!(!steps.iter().any(|step| {
        step["command"]
            .as_array()
            .is_some_and(|command| {
                command.iter().any(|part| {
                    part.as_str().is_some_and(|part| {
                        part == "scripts/build_source_witness_catalog.py"
                            || part == "scripts/validate_source_witness_foundation.py"
                    })
                })
            })
    }));
}
