//! Actual native CI selection, source history and required-job gate.
#[cfg(target_os = "linux")]
#[test]
fn software_ci_actual_history_and_required_gate_are_native() {
    use std::{
        fs,
        process::{Command, Output},
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::env::temp_dir().join(format!(
        "tos-software-ci-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("access/src/tos_access")).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    git(&["init"]);
    git(&["config", "user.name", "Software CI fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["config", "commit.gpgsign", "false"]);
    git(&["config", "core.hooksPath", "/dev/null"]);
    fs::write(root.join("README.md"), "[old missing](missing-old.md)\n").unwrap();
    fs::write(root.join("access/src/tos_access/reader.py"), "# source\n").unwrap();
    let integration =
        "mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md";
    fs::create_dir_all(root.join(integration).parent().unwrap()).unwrap();
    fs::write(root.join(integration), "Required Questbook source input.\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "baseline"]);
    let base = git(&["rev-parse", "HEAD"]);
    fs::remove_file(root.join("access/src/tos_access/reader.py")).unwrap();
    fs::write(
        root.join("access/заметка😀.md"),
        "[reference]\n[reference]: ../target.md\n",
    )
    .unwrap();
    fs::write(root.join("target.md"), "exists\n").unwrap();
    fs::write(root.join("README.md"), "[old missing](missing-old.md)\n```md\n[literal](fake.md)\n```\n[new](target.md#part)\n[external](https://example.invalid/no-fetch)\n[root](/hosted-site)\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-m", "reader removal plus Unicode documentation"]);
    let executable = std::env::var_os("TOS_SOFTWARE_CI_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-software-ci").into());
    let invoke = |mode: &str, needs: Option<&str>, full: bool, tree: Option<&str>| -> Output {
        let mut command = Command::new(&executable);
        command.arg(mode);
        if mode == "plan" {
            command
                .arg("--repo-root")
                .arg(&root)
                .arg("--base")
                .arg(&base);
            if full {
                command.arg("--full");
            }
            if let Some(tree) = tree {
                command.arg("--validated-tree").arg(tree);
            }
            command.env("GITHUB_OUTPUT", root.join("native-output"));
        } else {
            command
                .env("CI_NEEDS", needs.unwrap())
                .env_remove("GITHUB_OUTPUT")
                .env("PATH", "");
        }
        command.current_dir(&root).output().unwrap()
    };
    for full in [false, true] {
        let native = invoke("plan", None, full, None);
        assert_eq!(
            native.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert!(native.stderr.is_empty());
        let selection: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
        assert_eq!(
            selection["software_mode"],
            if full { "full" } else { "reader" }
        );
        assert_eq!(
            selection["changed_paths"],
            serde_json::json!([
                "README.md",
                "access/src/tos_access/reader.py",
                "access/заметка😀.md",
                "target.md"
            ])
        );
        assert_eq!(selection["worker"], true);
        assert_eq!(selection["rust"], full);
        assert_eq!(selection["forced_full"], full);
        assert!(
            fs::read_to_string(root.join("native-output"))
                .unwrap()
                .ends_with(if full {
                    "software_mode=full\nworker=true\nrust=true\n"
                } else {
                    "software_mode=reader\nworker=true\nrust=false\n"
                })
        );
        assert!(
            String::from_utf8(native.stdout)
                .unwrap()
                .contains("\\u0437")
        );
    }
    let tree = git(&["rev-parse", "HEAD^{tree}"]);
    let other_tree = "0".repeat(40);
    for (candidate, full, expected) in [
        (tree.as_str(), false, "none"),
        (other_tree.as_str(), false, "reader"),
        (tree.as_str(), true, "full"),
    ] {
        let native = invoke("plan", None, full, Some(candidate));
        assert!(
            native.status.success(),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        let selected: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
        assert_eq!(selected["software_mode"], expected);
        assert_eq!(
            selected["validated_tree"].as_str(),
            (expected == "none").then_some(tree.as_str())
        );
        if expected == "none" {
            assert_eq!(selected["worker"], false);
            assert_eq!(selected["rust"], false);
        }
    }
    assert!(
        !invoke("plan", None, false, Some("invalid"))
            .status
            .success()
    );
    let good = serde_json::json!({
        "plan":{"result":"success","outputs":{"software_mode":"reader","worker":"true","rust":"false"}},
        "software_prepare":{"result":"success"},
        "software":{"result":"success"},
        "software_package":{"result":"success"},
        "worker":{"result":"success"},
        "rust":{"result":"skipped"},
        "postgres":{"result":"skipped"}
    });
    let native = invoke("gate", Some(&good.to_string()), false, None);
    assert_eq!(native.status.code(), Some(0));
    assert!(
        String::from_utf8(native.stdout)
            .unwrap()
            .contains("All selected checks succeeded")
    );
    let mut no_software = good.clone();
    no_software["plan"]["outputs"]["software_mode"] = "none".into();
    no_software["plan"]["outputs"]["worker"] = "false".into();
    for job in ["software_prepare", "software", "software_package", "worker"] {
        no_software[job]["result"] = "skipped".into();
    }
    assert_eq!(
        invoke("gate", Some(&no_software.to_string()), false, None)
            .status
            .code(),
        Some(0),
        "unselected software producer and consumers must be skipped"
    );
    for job in ["software_prepare", "software", "software_package"] {
        let mut unexpectedly_run = no_software.clone();
        unexpectedly_run[job]["result"] = "success".into();
        assert_eq!(
            invoke("gate", Some(&unexpectedly_run.to_string()), false, None)
                .status
                .code(),
            Some(1),
            "unselected software job {job} must not run"
        );
    }
    for job in [
        "plan",
        "software_prepare",
        "software",
        "software_package",
        "worker",
        "rust",
        "postgres",
    ] {
        for result in [Some("failure"), Some("cancelled"), None] {
            let mut bad = good.clone();
            if let Some(result) = result {
                bad[job]["result"] = result.into();
            } else {
                bad.as_object_mut().unwrap().remove(job);
            }
            assert_eq!(
                invoke("gate", Some(&bad.to_string()), false, None)
                    .status
                    .code(),
                Some(1)
            );
        }
    }
    let selected_rust = serde_json::json!({
        "plan":{"result":"success","outputs":{"software_mode":"full","worker":"true","rust":"true"}},
        "software_prepare":{"result":"success"},
        "software":{"result":"success"},
        "software_package":{"result":"success"},
        "worker":{"result":"success"},
        "rust":{"result":"success"},
        "postgres":{"result":"success"}
    });
    assert_eq!(
        invoke("gate", Some(&selected_rust.to_string()), false, None)
            .status
            .code(),
        Some(0),
        "the selected Rust matrix aggregate and PostgreSQL result must succeed"
    );
    for result in [Some("failure"), Some("cancelled"), None] {
        let mut bad = selected_rust.clone();
        if let Some(result) = result {
            bad["rust"]["result"] = result.into();
        } else {
            bad.as_object_mut().unwrap().remove("rust");
        }
        assert_eq!(
            invoke("gate", Some(&bad.to_string()), false, None)
                .status
                .code(),
            Some(1),
            "the aggregate result for all Rust matrix children must fail closed"
        );
    }
    assert!(
        Command::new(root.join("missing-native"))
            .arg("gate")
            .env("PATH", "")
            .output()
            .is_err()
    );
    // Documentation paths retain their changed-path selection.
    git(&["reset", "--hard", &base]);
    fs::write(root.join("README.md"), "Human guide updated.\n").unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "# AGENTS.md\nThis card applies to the fixture.\n",
    )
    .unwrap();
    git(&["add", "README.md", "AGENTS.md"]);
    git(&["commit", "-m", "documentation only"]);
    let native = invoke("plan", None, false, None);
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    let selection: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
    assert_eq!(selection["software_mode"], "none");
    assert_eq!(selection["rust"], false);
    assert_eq!(selection["worker"], false);
    fs::remove_file(root.join(integration)).unwrap();
    git(&["add", "-A", "--", integration]);
    git(&["commit", "-m", "delete required Questbook input"]);
    let native = invoke("plan", None, false, None);
    assert!(native.status.success());
    let selection: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
    assert_eq!(selection["software_mode"], "none");
    assert_eq!(selection["worker"], false);
    assert_eq!(selection["rust"], true);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn software_ci_large_diff_keeps_late_job_selection() {
    use std::{
        fs,
        io::Write,
        path::Path,
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };
    fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> String {
        let mut command = Command::new("git");
        command
            .args(args)
            .current_dir(root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if input.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().unwrap();
        if let Some(bytes) = input {
            child.stdin.take().unwrap().write_all(bytes).unwrap();
        }
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    }
    let root = std::env::temp_dir().join(format!(
        "tos-ci-large-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet"], None);
    git(&root, &["config", "user.name", "Software CI fixture"], None);
    git(
        &root,
        &["config", "user.email", "fixture@example.invalid"],
        None,
    );
    git(&root, &["config", "commit.gpgsign", "false"], None);
    git(&root, &["config", "core.hooksPath", "/dev/null"], None);
    fs::write(root.join("target.md"), "synthetic link target\n").unwrap();
    git(&root, &["add", "target.md"], None);
    git(&root, &["commit", "--quiet", "-m", "baseline"], None);
    let base = git(&root, &["rev-parse", "HEAD"], None);
    let blob = git(
        &root,
        &["hash-object", "-w", "--stdin"],
        Some(b"synthetic source\n"),
    );
    // Index-only source rows avoid thousands of irrelevant worktree files.
    // HEAD still contains the complete real Git tree and exact changed paths.
    let mut entries = (0..4097)
        .map(|n| format!("100644 {blob}\taccess/web/fixture-{n:05}.ts\n"))
        .collect::<String>();
    entries.push_str(&format!("100644 {blob}\tzz-shared-contract.json\n"));
    git(
        &root,
        &["update-index", "--index-info"],
        Some(entries.as_bytes()),
    );
    fs::write(root.join("zz-late.md"), "[new](target.md)\n").unwrap();
    git(&root, &["add", "zz-late.md"], None);
    git(
        &root,
        &["commit", "--quiet", "-m", "large bounded migration"],
        None,
    );
    let executable = std::env::var_os("TOS_SOFTWARE_CI_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-software-ci").into());
    let invoke = |full: bool| {
        let mut command = Command::new(&executable);
        command
            .args(["plan", "--repo-root"])
            .arg(&root)
            .arg("--base")
            .arg(&base)
            .env("GITHUB_OUTPUT", root.join("native-output"))
            .current_dir(&root);
        if full {
            command.arg("--full");
        }
        command.output().unwrap()
    };
    for full in [false, true] {
        let output = invoke(full);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let selection: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(selection["changed_paths"].as_array().unwrap().len(), 4099);
        assert_eq!(selection["changed_paths"][4097], "zz-late.md");
        assert_eq!(selection["changed_paths"][4098], "zz-shared-contract.json");
        assert_eq!(selection["software_mode"], "full");
        assert_eq!(selection["worker"], true);
        assert_eq!(selection["rust"], true);
        assert_eq!(selection["forced_full"], full);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rust_product_changes_require_the_native_package_consumer() {
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "rust/crates/tos-access/src/software_archive.rs",
        "tests/conformance/rust/source-profile.json",
    ] {
        let selected =
            tos_ops_mechanics_plan::software_ci::select(vec![path.to_owned()], false).unwrap();
        assert_eq!(selected.software_mode, "browser", "{path}");
        assert!(selected.rust, "{path}");
        assert!(!selected.worker, "{path}");
    }
}

fn repository() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf()
}
#[test]
fn software_stage_cli_authenticates_then_binds_synthetic_products() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command, Output},
        time::{SystemTime, UNIX_EPOCH},
    };
    use tos_foundation::{
        Digest256Hasher,
        native_software_roles::{self, COMMANDS},
    };

    let temp = std::env::temp_dir().join(format!(
        "tos-software-stage-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let producer = temp.join("producer");
    fs::create_dir_all(&producer).unwrap();
    let source =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")).unwrap();
    let git = |root: &Path, args: &[&str]| {
        let output = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    for package in [
        "tos-access",
        "tos-command",
        "tos-validation",
        "tos-compiler",
        "tos-reader",
        "tos-ops-mechanics-plan",
    ] {
        let relative = format!("rust/crates/{package}/Cargo.toml");
        let target = producer.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(source.join(&relative), target).unwrap();
    }
    for name in ["Cargo.lock", "rust-toolchain.toml"] {
        fs::copy(source.join(name), producer.join(name)).unwrap();
    }
    git(&producer, &["init", "--quiet"]);
    git(
        &producer,
        &["config", "user.name", "Software stage fixture"],
    );
    git(
        &producer,
        &["config", "user.email", "fixture@example.invalid"],
    );
    git(&producer, &["add", "."]);
    git(
        &producer,
        &["commit", "--quiet", "-m", "synthetic stage source"],
    );
    let consumer = temp.join("consumer");
    let cloned = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "clone",
            "--quiet",
            "--no-hardlinks",
        ])
        .arg(&producer)
        .arg(&consumer)
        .output()
        .unwrap();
    assert!(
        cloned.status.success(),
        "{}",
        String::from_utf8_lossy(&cloned.stderr)
    );

    let target = "x86_64-unknown-linux-gnu";
    let images = producer
        .join("software-cargo-target")
        .join(target)
        .join("debug");
    let receipts = producer.join("receipts");
    fs::create_dir_all(&images).unwrap();
    fs::create_dir_all(&receipts).unwrap();
    let lock = fs::read(producer.join("Cargo.lock")).unwrap();
    let mut hash = Digest256Hasher::new();
    hash.update(&lock);
    let identity = serde_json::json!({
        "source_commit": git(&producer, &["rev-parse", "HEAD"]),
        "source_tree": git(&producer, &["rev-parse", "HEAD^{tree}"]),
        "lock_sha256": hash.finalize().to_hex(), "toolchain": "1.98.1",
        "target": target, "profile": "debug", "features": []
    });
    let product_names = std::iter::once("tos-access").chain(COMMANDS);
    let mut descriptor = serde_json::Map::new();
    for (index, name) in product_names.enumerate() {
        let bytes = vec![index as u8 + 1; 64];
        let image = images.join(name);
        fs::write(&image, &bytes).unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o755)).unwrap();
        let mut product = identity.clone();
        let object = product.as_object_mut().unwrap();
        if name == "tos-access" {
            object.remove("features");
            object.insert("opt_level".into(), serde_json::json!("1"));
            object.insert(
                "schema_version".into(),
                serde_json::json!("tos_native_access_build_v2"),
            );
        } else {
            object.insert(
                "features".into(),
                serde_json::json!(native_software_roles::features(name).unwrap_or(&[])),
            );
            object.insert(
                "schema_version".into(),
                serde_json::json!("tos_native_software_command_build_v1"),
            );
            descriptor.insert(
                name.into(),
                serde_json::json!({
                    "binary": image.display().to_string(),
                    "receipt": receipts.join(format!("{name}-build.json")).display().to_string()
                }),
            );
        }
        object.insert("sha256".into(), serde_json::json!(hash_bytes(&bytes)));
        object.insert("size_bytes".into(), serde_json::json!(bytes.len()));
        let file = if name == "tos-access" {
            "tos-native-build.json".into()
        } else {
            format!("{name}-build.json")
        };
        fs::write(receipts.join(file), serde_json::to_vec(&product).unwrap()).unwrap();
    }
    fs::write(
        receipts.join("tos-native-command-products.json"),
        serde_json::to_vec(&descriptor).unwrap(),
    )
    .unwrap();
    let fixture = images.join("tos-e2e-fixture");
    let fixture_bytes = vec![0xfe; 64];
    fs::write(&fixture, &fixture_bytes).unwrap();
    fs::set_permissions(&fixture, fs::Permissions::from_mode(0o755)).unwrap();
    let fixture_messages = producer.join("fixture-cargo.jsonl");
    let event = serde_json::json!({
        "reason":"compiler-artifact",
        "package_id":format!("path+file://{}#tos-access@0.6.0", producer.join("rust/crates/tos-access").display()),
        "manifest_path":producer.join("rust/crates/tos-access/Cargo.toml"),
        "target":{"name":"tos-e2e-fixture","kind":["bin"],"required-features":["e2e-fixture"]},
        "features":["default","e2e-fixture","source-owner","source-tools"],
        "profile":{"opt_level":"0","debuginfo":0,"debug_assertions":true,"test":false},
        "executable":fixture
    });
    let mut messages = serde_json::to_vec(&event).unwrap();
    messages.extend_from_slice(b"\n{\"reason\":\"build-finished\",\"success\":true}\n");
    fs::write(&fixture_messages, &messages).unwrap();
    let rustc = producer.join("rustc-version");
    fs::write(&rustc, b"rustc 1.98.1 (synthetic)\n").unwrap();
    let web = producer.join("access/web/dist");
    let generated = producer.join("access/deploy/cloudflare-worker/generated");
    for (path, bytes) in [
        (web.join("assets/tos-graph.js"), b"graph-js".as_slice()),
        (web.join("assets/tos-graph.css"), b"graph-css".as_slice()),
        (generated.join("tos_web_rules.js"), b"rules-js".as_slice()),
        (
            generated.join("tos_web_rules_bg.wasm"),
            b"rules-wasm".as_slice(),
        ),
    ] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let stage = temp.join("stage");
    let output = temp.join("github-output");
    fs::write(&output, b"").unwrap();
    let invoke = |mode: &str, flags: &[(&str, String)]| -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tos-software-ci"));
        command.arg(mode);
        for (name, value) in flags {
            command.arg(name).arg(value);
        }
        command.output().unwrap()
    };
    let producer_args = [
        ("--repo-root", producer.display().to_string()),
        ("--stage-root", stage.display().to_string()),
        ("--image-root", images.display().to_string()),
        ("--receipt-root", receipts.display().to_string()),
        ("--fixture-messages", fixture_messages.display().to_string()),
        ("--rustc-version", rustc.display().to_string()),
        ("--web-dist", web.display().to_string()),
        ("--worker-generated", generated.display().to_string()),
        ("--github-output", output.display().to_string()),
    ];
    let produced = invoke("software-stage-manifest", &producer_args);
    assert!(
        produced.status.success(),
        "{}",
        String::from_utf8_lossy(&produced.stderr)
    );
    let digest = fs::read_to_string(&output)
        .unwrap()
        .trim()
        .strip_prefix("software_stage_manifest_sha256=")
        .unwrap()
        .to_owned();

    let consumer_images = consumer
        .join("software-cargo-target")
        .join(target)
        .join("debug");
    let consumer_receipts = consumer.join("receipts");
    fs::create_dir_all(&consumer_receipts).unwrap();
    let env = temp.join("github-env");
    let path = temp.join("github-path");
    fs::write(&env, b"").unwrap();
    fs::write(&path, b"").unwrap();
    let bind_args = |digest: String| {
        [
            ("--repo-root", consumer.display().to_string()),
            ("--stage-root", stage.display().to_string()),
            ("--manifest-sha256", digest),
            ("--image-root", consumer_images.display().to_string()),
            ("--receipt-root", consumer_receipts.display().to_string()),
            ("--github-env", env.display().to_string()),
            ("--github-path", path.display().to_string()),
        ]
    };
    let wrong = "0".repeat(64);
    assert_ne!(wrong, digest);
    assert!(
        !invoke("software-stage-bind", &bind_args(wrong))
            .status
            .success()
    );
    assert!(!consumer.join("access").exists() && !consumer_images.exists());
    assert!(fs::read_dir(&consumer_receipts).unwrap().next().is_none());
    assert_eq!(fs::read(&env).unwrap(), b"");
    assert_eq!(fs::read(&path).unwrap(), b"");
    let bound = invoke("software-stage-bind", &bind_args(digest));
    assert!(
        bound.status.success(),
        "{}",
        String::from_utf8_lossy(&bound.stderr)
    );
    for (index, name) in std::iter::once("tos-access").chain(COMMANDS).enumerate() {
        let file = consumer_images.join(name);
        assert_eq!(fs::read(&file).unwrap(), vec![index as u8 + 1; 64]);
        assert_eq!(
            fs::metadata(file).unwrap().permissions().mode() & 0o111,
            0o111
        );
    }
    assert_eq!(
        fs::read(consumer_images.join("tos-e2e-fixture")).unwrap(),
        fixture_bytes
    );
    let rebound: serde_json::Value = serde_json::from_slice(
        &fs::read(consumer_receipts.join("tos-native-command-products.json")).unwrap(),
    )
    .unwrap();
    for name in COMMANDS {
        assert_eq!(
            rebound[name]["binary"],
            consumer_images.join(name).display().to_string()
        );
        assert_eq!(
            rebound[name]["receipt"],
            consumer_receipts
                .join(format!("{name}-build.json"))
                .display()
                .to_string()
        );
    }
    let exported = fs::read_to_string(&env).unwrap();
    for key in [
        "TOS_NATIVE_SOURCE_PREPARE_LIMITS={\"max_input_bytes\":1048576,\"max_capture_bytes\":8388608,\"max_stage_bytes\":8388608,\"max_temp_bytes\":8388608}",
        "TOS_KAG_RELEASE_BIN=",
        "TOS_SCHEMA_WORKER_PATH=",
        "TOS_NATIVE_OWNER_COMMAND_PATH=",
        "TOS_E2E_FIXTURE_BIN=",
    ] {
        assert!(exported.contains(key));
    }
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        format!("{}\n", consumer_images.display())
    );
    for (relative, bytes) in [
        (
            "access/web/dist/assets/tos-graph.js",
            b"graph-js".as_slice(),
        ),
        (
            "access/web/dist/assets/tos-graph.css",
            b"graph-css".as_slice(),
        ),
        (
            "access/deploy/cloudflare-worker/generated/tos_web_rules.js",
            b"rules-js".as_slice(),
        ),
        (
            "access/deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm",
            b"rules-wasm".as_slice(),
        ),
    ] {
        assert_eq!(fs::read(consumer.join(relative)).unwrap(), bytes);
    }
    assert_eq!(
        fs::read(consumer_receipts.join("software-fixture-cargo-messages.jsonl")).unwrap(),
        messages
    );
    fs::remove_dir_all(temp).unwrap();
}

fn hash_bytes(raw: &[u8]) -> String {
    let mut digest = tos_foundation::Digest256Hasher::new();
    digest.update(raw);
    digest.finalize().to_hex()
}

#[path = "support/validation_lanes.rs"]
mod validation_lanes_cases;
