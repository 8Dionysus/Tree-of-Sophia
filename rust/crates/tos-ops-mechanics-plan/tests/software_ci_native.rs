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
    let invoke = |mode: &str, needs: Option<&str>, full: bool| -> Output {
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
        let native = invoke("plan", None, full);
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
                    "software_mode=full\nworker=true\nrust=true\nroute_docs=none\nagent_surface=false\ndocumentation_family=false\n"
                } else {
                    "software_mode=reader\nworker=true\nrust=false\nroute_docs=none\nagent_surface=false\ndocumentation_family=false\n"
                })
        );
        assert!(
            String::from_utf8(native.stdout)
                .unwrap()
                .contains("\\u0437")
        );
    }
    // New source-visible errors fail together; historical missing links stay historical.
    fs::write(root.join("README.md"), "<<<<<<< branch\n[new](absent.md)\n").unwrap();
    let native = invoke("plan", None, false);
    assert_eq!(native.status.code(), Some(1));
    let diagnostic = String::from_utf8(native.stderr).unwrap();
    assert!(diagnostic.contains("README.md: unresolved merge marker"));
    assert!(diagnostic.contains("README.md: missing repository link target absent.md"));
    let good = serde_json::json!({"plan":{"result":"success","outputs":{"software_mode":"reader","worker":"true","rust":"false","route_docs":"none","agent_surface":"false","documentation_family":"false"}}, "software":{"result":"success"},"worker":{"result":"success"},"rust":{"result":"skipped"}});
    let native = invoke("gate", Some(&good.to_string()), false);
    assert_eq!(native.status.code(), Some(0));
    assert!(
        String::from_utf8(native.stdout)
            .unwrap()
            .contains("All selected checks succeeded")
    );
    for job in ["plan", "software", "worker", "rust"] {
        for result in [Some("failure"), Some("cancelled"), None] {
            let mut bad = good.clone();
            if let Some(result) = result {
                bad[job]["result"] = result.into();
            } else {
                bad.as_object_mut().unwrap().remove(job);
            }
            assert_eq!(
                invoke("gate", Some(&bad.to_string()), false).status.code(),
                Some(1)
            );
        }
    }
    assert!(
        Command::new(root.join("missing-native"))
            .arg("gate")
            .env("PATH", "")
            .output()
            .is_err()
    );
    // Real source history still validates changed Markdown when package jobs
    // are omitted. A route card selects its source owner checks explicitly.
    git(&["reset", "--hard", &base]);
    fs::write(root.join("README.md"), "Human guide updated.\n").unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "# AGENTS.md\nThis card applies to the fixture.\n",
    )
    .unwrap();
    git(&["add", "README.md", "AGENTS.md"]);
    git(&["commit", "-m", "documentation only"]);
    let native = invoke("plan", None, false);
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    let selection: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
    assert_eq!(selection["software_mode"], "none");
    assert_eq!(selection["rust"], false);
    assert_eq!(selection["worker"], false);
    assert_eq!(selection["route_docs"], "source");
    fs::write(root.join("README.md"), "[broken](absent.md)\n").unwrap();
    assert_eq!(invoke("plan", None, false).status.code(), Some(1));
    fs::write(root.join("README.md"), "Human guide updated.\n").unwrap();
    fs::remove_file(root.join(integration)).unwrap();
    git(&["add", "-A", "--", integration]);
    git(&["commit", "-m", "delete required Questbook input"]);
    let native = invoke("plan", None, false);
    assert!(native.status.success());
    let selection: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
    assert_eq!(selection["software_mode"], "none");
    assert_eq!(selection["worker"], false);
    assert_eq!(selection["rust"], true);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn software_ci_large_diff_keeps_late_job_and_documentation_coverage() {
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
    // The original base and all changed docs remain checked even after the
    // former4096 threshold and when the caller explicitly requests full jobs.
    fs::write(
        root.join("zz-late.md"),
        "<<<<<<< branch\n[new](absent.md)\n",
    )
    .unwrap();
    let before = fs::read(root.join("native-output")).unwrap();
    for full in [false, true] {
        let output = invoke(full);
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains("zz-late.md: unresolved merge marker"),
            "{error}"
        );
        assert!(
            error.contains("zz-late.md: missing repository link target absent.md"),
            "{error}"
        );
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(root.join("native-output")).unwrap(), before);
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
fn workflow(name: &str) -> yaml_rust2::Yaml {
    let raw = std::fs::read_to_string(repository().join(".github/workflows").join(name)).unwrap();
    let mut docs = yaml_rust2::YamlLoader::load_from_str(&raw).unwrap();
    assert_eq!(docs.len(), 1);
    docs.remove(0)
}
fn steps<'a>(jobs: &'a yaml_rust2::Yaml, job: &str) -> &'a [yaml_rust2::Yaml] {
    jobs[job]["steps"].as_vec().unwrap()
}
fn run(step: &yaml_rust2::Yaml) -> &str {
    step["run"].as_str().unwrap_or("")
}
fn one_step<'a>(steps: &'a [yaml_rust2::Yaml], needle: &str) -> &'a yaml_rust2::Yaml {
    let found: Vec<_> = steps.iter().filter(|s| run(s).contains(needle)).collect();
    assert_eq!(found.len(), 1, "expected one step containing {needle}");
    found[0]
}
fn sparse(jobs: &yaml_rust2::Yaml, job: &str) -> std::collections::BTreeSet<String> {
    let found: Vec<_> = steps(jobs, job)
        .iter()
        .filter_map(|s| s["with"]["sparse-checkout"].as_str())
        .collect();
    assert_eq!(found.len(), 1);
    found[0].lines().map(str::to_owned).collect()
}

#[test]
fn workflow_requires_authenticated_native_selection_and_all_selected_jobs() {
    use std::collections::BTreeSet;
    let w = workflow("repo-validation.yml");
    let jobs = &w["jobs"];
    let mut cached_jobs = BTreeSet::new();
    for (name, job) in jobs.as_hash().unwrap() {
        if let Some(env) = job["env"].as_hash() {
            assert!(
                env.values().all(|value| !value
                    .as_str()
                    .is_some_and(|s| s.contains("${{") && s.contains("runner."))),
                "job env cannot reference the runner context"
            );
        }
        let st = job["steps"].as_vec().unwrap();
        let Some(restore) = st.iter().find(|s| s["id"].as_str() == Some("cargo_cache")) else {
            continue;
        };
        cached_jobs.insert(name.as_str().unwrap());
        assert!(
            restore["uses"]
                .as_str()
                .unwrap()
                .starts_with("actions/cache@")
        );
        assert_eq!(restore["continue-on-error"].as_bool(), Some(true));
        let key = restore["with"]["key"].as_str().unwrap();
        assert!(key.starts_with("tos-cargo-lib-v2-"));
        assert!(
            !key.contains("github.sha"),
            "identical trees must share a cache key"
        );
        let suffix = "-${{ steps.cargo_platform.outputs.source_tree }}";
        let prefix = key.strip_suffix(suffix).unwrap();
        assert_eq!(
            restore["with"]["restore-keys"].as_str().unwrap().trim(),
            format!("{prefix}-")
        );
        assert!(run(one_step(st, "source_tree=%s")).contains("git rev-parse 'HEAD^{tree}'"));
        for path in restore["with"]["path"].as_str().unwrap().lines() {
            assert!(
                path.ends_with("/registry")
                    || path.ends_with("/**/.fingerprint")
                    || path.ends_with("/**/build")
                    || ["rlib", "rmeta", "so", "d"]
                        .iter()
                        .any(|extension| path.ends_with(&format!("/**/deps/*.{extension}"))),
                "cache only compiler libraries/metadata, not full targets: {path}"
            );
        }
    }
    assert_eq!(cached_jobs, BTreeSet::from(["plan", "worker"]));
    assert!(!w["on"]["workflow_dispatch"].is_badvalue());
    assert_eq!(
        jobs["required_gate"]["needs"]
            .as_vec()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["plan", "software", "worker", "rust"])
    );
    assert!(
        jobs["required_gate"]["if"]
            .as_str()
            .unwrap()
            .contains("always()")
    );
    for (job, expected) in [
        ("software", "!= 'none'"),
        ("worker", "== 'true'"),
        ("rust", "== 'true'"),
    ] {
        assert_eq!(jobs[job]["needs"].as_str(), Some("plan"));
        assert!(jobs[job]["if"].as_str().unwrap().contains(expected));
    }
    let plan = steps(jobs, "plan");
    let build = one_step(plan, "executor-manifest");
    let selection = one_step(plan, "\"$TOS_SOFTWARE_CI_EXECUTOR\" plan");
    assert!(
        plan.iter().position(|s| std::ptr::eq(s, build))
            < plan.iter().position(|s| std::ptr::eq(s, selection))
    );
    let documentation = one_step(plan, "--documentation-family-build");
    assert!(
        documentation["if"]
            .as_str()
            .unwrap()
            .contains("steps.select.outputs.route_docs")
    );
    assert!(run(documentation).contains("operation=validate-source"));
    assert!(run(documentation).contains("operation=validate;"));
    assert!(run(documentation).contains("tos-agents-route-harness"));
    assert!(documentation["continue-on-error"].is_badvalue());
    for job in ["software", "rust", "required_gate"] {
        let st = steps(jobs, job);
        let binding = one_step(st, "executor-bind --repo-root");
        let first_call = st
            .iter()
            .position(|s| {
                run(s).contains("\"$TOS_RELEASE_CHECK_EXECUTOR\"")
                    || run(s).contains("\"$TOS_VALIDATION_LANES_EXECUTOR\"")
                    || run(s).contains("\"$TOS_SOFTWARE_CI_EXECUTOR\" gate")
            })
            .unwrap();
        assert!(st.iter().position(|s| std::ptr::eq(s, binding)).unwrap() < first_call);
        assert_eq!(
            binding["env"]["TOS_CI_EXECUTOR_SHA256"].as_str(),
            Some("${{ needs.plan.outputs.executor_sha256 }}")
        );
        assert_eq!(
            binding["env"]["TOS_CI_MANIFEST_SHA256"].as_str(),
            Some("${{ needs.plan.outputs.executor_manifest_sha256 }}")
        );
        assert!(
            run(binding).find("sha256sum --check --status").unwrap()
                < run(binding)
                    .find("\"$root/tos-software-ci\" executor-bind")
                    .unwrap()
        );
    }
    let software = steps(jobs, "software");
    let full = one_step(software, "--phase tests");
    assert!(full["if"].as_str().unwrap().contains("== 'full'"));
    let reader = one_step(software, "--run software_reader");
    assert!(reader["if"].as_str().unwrap().contains("== 'reader'"));
    let receipts = run(one_step(software, "software-receipts"));
    let production_target = one_step(software, "software-receipts")["env"]["CARGO_TARGET_DIR"]
        .as_str()
        .unwrap();
    let test_target = "software-test-cargo-target";
    let configure = one_step(software, "CARGO_TARGET_DIR=%s/software-test-cargo-target");
    assert!(run(configure).contains("\"$RUNNER_TEMP\" >> \"$GITHUB_ENV\""));
    assert!(production_target.ends_with("/software-cargo-target"));
    assert!(!production_target.contains(test_target));
    assert_eq!(
        one_step(software, "software-limits")["env"]["CARGO_TARGET_DIR"].as_str(),
        Some(production_target),
    );
    assert!(receipts.contains("\"$TOS_SOFTWARE_CI_EXECUTOR\" software-receipts"));
    assert!(receipts.contains("--executor-root \"$RUNNER_TEMP/tos-ci-executors\""));
    for name in [
        "tos-release-check",
        "tos-validation-lanes",
        "tos-software-ci",
    ] {
        assert!(!receipts.contains(&format!("--bin {name}")));
    }
    for line in receipts.lines().filter(|line| {
        line.trim().starts_with("cargo build")
            && !line.contains("--release")
            && !line.contains("--bin tos-e2e-fixture")
    }) {
        assert!(
            line.contains("--message-format=json") || line.trim_end().ends_with('\\'),
            "production build lacks receipt output: {line}"
        );
    }
    let rust = steps(jobs, "rust");
    let postgres = one_step(rust, "--test postgres_durable_lab");
    assert!(postgres["env"]["TOS_CMD_POSTGRES_URL"].as_str().is_some());
    let current_restore = postgres["env"]["TOS_CMD2_V2_RESTORE_PG_URL"]
        .as_str()
        .expect("addressed current-writer restore target is configured");
    let v1_wire_restore = postgres["env"]["TOS_CMD2_V2_V1_WIRE_RESTORE_PG_URL"]
        .as_str()
        .expect("current-ABI V1-wire restore target is configured");
    assert_ne!(current_restore, v1_wire_restore);
    assert!(current_restore.contains("${{ github.run_id }}_${{ github.run_attempt }}"));
    assert!(v1_wire_restore.contains("${{ github.run_id }}_${{ github.run_attempt }}"));
    assert!(
        postgres["env"]["TOS_CMD2_V2_RESTORE_DATABASE"]
            .as_str()
            .is_some()
    );
    assert!(
        postgres["env"]["TOS_CMD2_V2_V1_WIRE_RESTORE_DATABASE"]
            .as_str()
            .is_some()
    );
    assert!(!jobs["rust"]["services"]["postgres"].is_badvalue());
    let gate = steps(jobs, "required_gate").last().unwrap();
    assert_eq!(run(gate), "\"$TOS_SOFTWARE_CI_EXECUTOR\" gate");
    assert_eq!(
        gate["env"]["CI_NEEDS"].as_str(),
        Some("${{ toJSON(needs) }}")
    );
}

#[test]
fn sparse_checkout_excludes_whole_corpus_and_private_payloads() {
    let w = workflow("repo-validation.yml");
    let jobs = &w["jobs"];
    let rust = sparse(jobs, "rust");
    let software = sparse(jobs, "software");
    let worker = sparse(jobs, "worker");
    assert!(rust.contains("!/ToS/source-witnesses/**/payload/"));
    for checkout in [&rust, &software, &worker] {
        for path in ["/ToS/source-witnesses/", "/ToS/", "/ToS/candidate-intake/"] {
            assert!(!checkout.contains(path));
        }
    }
}

#[path = "support/validation_lanes.rs"]
mod validation_lanes_cases;
