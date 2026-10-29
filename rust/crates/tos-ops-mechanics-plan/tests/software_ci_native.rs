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
    // A prior tree result cannot suppress changed-document validation.
    fs::write(root.join("README.md"), "<<<<<<< branch\n[new](absent.md)\n").unwrap();
    let native = invoke("plan", None, false, Some(&tree));
    assert_eq!(native.status.code(), Some(1));
    let diagnostic = String::from_utf8(native.stderr).unwrap();
    assert!(diagnostic.contains("README.md: unresolved merge marker"));
    assert!(diagnostic.contains("README.md: missing repository link target absent.md"));
    let good = serde_json::json!({
        "plan":{"result":"success","outputs":{"software_mode":"reader","worker":"true","rust":"false","route_docs":"none","agent_surface":"false","documentation_family":"false"}},
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
        "plan":{"result":"success","outputs":{"software_mode":"full","worker":"true","rust":"true","route_docs":"none","agent_surface":"false","documentation_family":"false"}},
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
    assert_eq!(selection["route_docs"], "source");
    fs::write(root.join("README.md"), "[broken](absent.md)\n").unwrap();
    assert_eq!(invoke("plan", None, false, None).status.code(), Some(1));
    fs::write(root.join("README.md"), "Human guide updated.\n").unwrap();
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
fn named_step<'a>(steps: &'a [yaml_rust2::Yaml], name: &str) -> &'a yaml_rust2::Yaml {
    let found: Vec<_> = steps
        .iter()
        .filter(|step| step["name"].as_str() == Some(name))
        .collect();
    assert_eq!(found.len(), 1, "expected one named step {name}");
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
fn needs(job: &yaml_rust2::Yaml) -> std::collections::BTreeSet<String> {
    if let Some(need) = job["needs"].as_str() {
        return std::collections::BTreeSet::from([need.to_owned()]);
    }
    job["needs"]
        .as_vec()
        .unwrap()
        .iter()
        .map(|need| need.as_str().unwrap().to_owned())
        .collect()
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
        BTreeSet::from([
            "plan",
            "software_prepare",
            "software",
            "software_package",
            "worker",
            "rust",
            "postgres",
        ])
    );
    assert!(
        jobs["required_gate"]["if"]
            .as_str()
            .unwrap()
            .contains("always()")
    );
    for (job, expected) in [
        ("software_prepare", "!= 'none'"),
        ("software", "!= 'none'"),
        ("software_package", "!= 'none'"),
        ("worker", "== 'true'"),
        ("rust", "== 'true'"),
        ("postgres", "== 'true'"),
    ] {
        assert!(needs(&jobs[job]).contains("plan"), "{job} must need plan");
        assert!(jobs[job]["if"].as_str().unwrap().contains(expected));
    }
    assert_eq!(
        needs(&jobs["software_prepare"]),
        BTreeSet::from(["plan".to_owned()])
    );
    for consumer in ["software", "software_package"] {
        assert!(
            needs(&jobs[consumer]).contains("software_prepare"),
            "{consumer} must consume the prepared software stage"
        );
    }
    assert!(
        jobs["software_package"]["strategy"]["matrix"].is_badvalue(),
        "candidate outputs must come from the static package job"
    );
    assert!(
        jobs["software"]["outputs"].is_badvalue(),
        "fixture matrix has only an aggregate result, not candidate outputs"
    );
    assert_eq!(
        jobs["software"]["strategy"]["fail-fast"].as_bool(),
        Some(false)
    );
    assert_eq!(
        jobs["software"]["strategy"]["matrix"]["group"].as_str(),
        Some(
            "${{ fromJSON(needs.plan.outputs.software_mode == 'full' && '[\"access\",\"query\",\"command\"]' || '[\"single\"]') }}"
        )
    );
    assert_eq!(
        jobs["software"]["name"].as_str(),
        Some("Software fixture contracts / ${{ matrix.group }}")
    );
    assert!(jobs["software"]["continue-on-error"].is_badvalue());
    let access_cache = named_step(
        steps(jobs, "software"),
        "Configure verified Access compiler cache",
    );
    assert_eq!(
        access_cache["if"].as_str(),
        Some("${{ matrix.group == 'access' }}")
    );
    assert_eq!(access_cache["continue-on-error"].as_bool(), Some(true));
    let cache_script = access_cache["with"]["script"].as_str().unwrap();
    for contract in [
        "45f1447fbe231e3037bde351ef70677dd212216c8d62ae7ca409fecc4d6acc89",
        "core.exportVariable('CARGO_INCREMENTAL', '0')",
        "SCCACHE_CLIENT_SIDE: '1'",
        "SCCACHE_GHA_RW_MODE: context.eventName === 'pull_request' ? 'READ_ONLY' : 'READ_WRITE'",
        "SCCACHE_IGNORE_SERVER_IO_ERROR: '1'",
    ] {
        assert!(
            cache_script.contains(contract),
            "missing compiler cache contract: {contract}"
        );
    }
    assert!(
        cache_script.find("createHash('sha256')").unwrap()
            < cache_script.find("execFileSync('tar'").unwrap()
    );
    assert!(
        cache_script.find("['--start-server']").unwrap()
            < cache_script
                .find("core.exportVariable('RUSTC_WRAPPER'")
                .unwrap()
    );
    for (name, job) in jobs.as_hash().unwrap() {
        if name.as_str() != Some("software") {
            assert!(job["steps"].as_vec().unwrap().iter().all(|step| {
                step["with"]["script"]
                    .as_str()
                    .is_none_or(|script| !script.contains("core.exportVariable('RUSTC_WRAPPER'"))
            }));
        }
    }
    let cache_stats = named_step(
        steps(jobs, "software"),
        "Capture Access compiler cache diagnostics",
    );
    assert_eq!(
        cache_stats["if"].as_str(),
        Some("${{ always() && steps.access_cache.outputs.enabled == 'true' }}")
    );
    assert!(run(cache_stats).contains("trap '\"$RUSTC_WRAPPER\" --stop-server' EXIT"));
    assert!(
        named_step(
            steps(jobs, "software"),
            "Upload non-authoritative Cargo diagnostics"
        )["with"]["path"]
            .as_str()
            .unwrap()
            .contains("cargo-resource-profile/sccache.json")
    );
    let plan = steps(jobs, "plan");
    one_step(
        plan,
        "node --test .github/scripts/verified-pr-tree.test.mjs",
    );
    let proof = plan
        .iter()
        .find(|s| s["id"].as_str() == Some("main_ci"))
        .unwrap();
    assert!(
        proof["if"]
            .as_str()
            .unwrap()
            .contains("github.event_name == 'push'")
    );
    assert!(proof["if"].as_str().unwrap().contains("refs/heads/main"));
    let build = one_step(plan, "executor-manifest");
    let selection = one_step(plan, "\"$TOS_SOFTWARE_CI_EXECUTOR\" plan");
    assert!(run(selection).contains("--validated-tree"));
    assert_eq!(
        selection["env"]["VALIDATED_TREE"].as_str(),
        Some("${{ steps.main_ci.outputs.validated_tree }}")
    );
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
    for job in [
        "software_prepare",
        "software",
        "software_package",
        "rust",
        "required_gate",
    ] {
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
    for job in ["software_prepare", "software", "software_package"] {
        assert_eq!(jobs[job]["runs-on"].as_str(), Some("ubuntu-26.04"));
    }
    assert_eq!(
        sparse(jobs, "software_prepare"),
        sparse(jobs, "software"),
        "the fixture consumer must use the producer's checkout scope"
    );
    assert_eq!(
        sparse(jobs, "software_prepare"),
        sparse(jobs, "software_package"),
        "the package consumer must use the producer's checkout scope"
    );

    let software_prepare = steps(jobs, "software_prepare");
    let software = steps(jobs, "software");
    let software_package = steps(jobs, "software_package");
    let stage_manifest = one_step(software_prepare, "software-stage-manifest");
    assert_eq!(stage_manifest["id"].as_str(), Some("stage"));
    assert_eq!(
        jobs["software_prepare"]["outputs"]["software_stage_manifest_sha256"].as_str(),
        Some("${{ steps.stage.outputs.software_stage_manifest_sha256 }}")
    );
    let stage_run = run(stage_manifest);
    for input in [
        "--stage-root \"$RUNNER_TEMP/software-stage\"",
        "--fixture-messages \"$RUNNER_TEMP/software-fixture-cargo-messages.jsonl\"",
        "--web-dist \"$GITHUB_WORKSPACE/access/web/dist\"",
        "--worker-generated \"$GITHUB_WORKSPACE/access/deploy/cloudflare-worker/generated\"",
    ] {
        assert!(
            stage_run.contains(input),
            "Software stage is missing {input}"
        );
    }
    let staged = named_step(
        software_prepare,
        "Preserve unqualified native Stage 1 inputs",
    );
    assert!(
        staged["uses"]
            .as_str()
            .unwrap()
            .starts_with("actions/upload-artifact@")
    );
    assert_eq!(
        staged["with"]["name"].as_str(),
        Some("tos-native-stage1-unqualified-${{ github.run_id }}-${{ github.run_attempt }}")
    );
    assert_eq!(
        staged["with"]["path"].as_str(),
        Some("${{ runner.temp }}/software-stage/")
    );
    assert_eq!(staged["with"]["compression-level"].as_i64(), Some(1));
    assert_eq!(staged["with"]["retention-days"].as_i64(), Some(1));

    for (job, st) in [
        ("software", software),
        ("software_package", software_package),
    ] {
        let fetch = named_step(st, "Fetch same-run Software stage");
        assert!(
            fetch["uses"]
                .as_str()
                .unwrap()
                .starts_with("actions/download-artifact@")
        );
        assert_eq!(
            fetch["with"]["name"].as_str(),
            staged["with"]["name"].as_str(),
            "{job} must download the producer's same-run stage"
        );
        let bind = named_step(st, "Verify and bind exact Software stage");
        assert!(run(bind).contains("software-stage-bind"));
        assert!(run(bind).contains(
            "--manifest-sha256 \"${{ needs.software_prepare.outputs.software_stage_manifest_sha256 }}\""
        ));
        let fetch_index = st
            .iter()
            .position(|step| std::ptr::eq(step, fetch))
            .unwrap();
        let bind_index = st.iter().position(|step| std::ptr::eq(step, bind)).unwrap();
        assert!(
            fetch_index < bind_index,
            "{job} must verify staged files after download"
        );
        let first_use = st
            .iter()
            .position(|step| {
                let command = run(step);
                if job == "software" {
                    command.contains("--phase tests") || command.contains("--run software_reader")
                } else {
                    command.contains("software-limits")
                }
            })
            .unwrap();
        assert!(
            bind_index < first_use,
            "{job} must bind the stage before use"
        );
    }

    let full = one_step(software, "--phase tests");
    assert!(full["if"].as_str().unwrap().contains("== 'full'"));
    assert!(run(full).contains("--test-group \"${{ matrix.group }}\""));
    assert!(run(full).contains("--command-timeout-ms 900000 --cargo-timings"));
    assert!(full["continue-on-error"].is_badvalue());
    let reader = one_step(software, "--run software_reader");
    assert!(reader["if"].as_str().unwrap().contains("== 'reader'"));
    assert!(!run(reader).contains("--test-group"));
    let root = repository();
    use tos_ops_mechanics_plan::validation_lanes::{
        ReleasePhase, SoftwareTestGroup, release_steps, release_steps_with_test_group,
    };
    let mut grouped = Vec::new();
    for group in [
        SoftwareTestGroup::Access,
        SoftwareTestGroup::Query,
        SoftwareTestGroup::Command,
    ] {
        grouped.extend(
            release_steps_with_test_group(&root, ReleasePhase::Tests, Some(group)).unwrap(),
        );
    }
    assert_eq!(
        grouped,
        release_steps(&root, ReleasePhase::Tests).unwrap(),
        "matrix must partition the complete authored test suffix without altering argv or order"
    );
    let receipts = run(one_step(software_prepare, "software-receipts"));
    let production_target =
        one_step(software_prepare, "software-receipts")["env"]["CARGO_TARGET_DIR"]
            .as_str()
            .unwrap();
    let test_target = "software-test-cargo-target";
    let configure = one_step(software, "CARGO_TARGET_DIR=%s/software-test-cargo-target");
    assert!(run(configure).contains("\"$RUNNER_TEMP\" >> \"$GITHUB_ENV\""));
    assert!(production_target.ends_with("/software-cargo-target"));
    assert!(!production_target.contains(test_target));
    assert_eq!(
        one_step(software_package, "software-limits")["env"]["CARGO_TARGET_DIR"].as_str(),
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
    let prepare_build = run(named_step(
        software_prepare,
        "Prepare pinned native software and browser rules",
    ));
    assert!(prepare_build.contains("--features e2e-fixture"));
    assert!(prepare_build.contains("software-fixture-cargo-messages.jsonl"));
    for line in prepare_build.lines().filter(|line| {
        line.trim().starts_with("cargo build")
            && !line.contains("--release")
            && !line.contains("--bin tos-e2e-fixture")
    }) {
        assert!(
            line.contains("--message-format=json") || line.trim_end().ends_with('\\'),
            "production build lacks receipt output: {line}"
        );
    }

    let package_build = named_step(
        software_package,
        "Build and install the native software artifact without corpus data",
    );
    let package_run = run(package_build);
    for behavior in [
        "software build --root",
        "software verify --archive",
        "software install --archive",
        "TOS_E2E_FIXTURE_BIN=",
        "TOS_E2E_SITE_EXECUTABLE=",
        "cmp --silent \"$archive\" \"$repeated\"",
        "cmp --silent \"$archive.manifest.json\" \"$repeated.manifest.json\"",
    ] {
        assert!(
            package_run.contains(behavior),
            "package job lost {behavior}"
        );
    }
    let browser = named_step(
        software_package,
        "Test browser behavior with bounded native fixtures",
    );
    assert!(run(browser).contains("--run software_browser"));
    let bind_candidate = named_step(software_package, "Bind unqualified candidate digests");
    assert_eq!(bind_candidate["id"].as_str(), Some("candidate"));
    let candidate_run = run(bind_candidate);
    for file in [
        "dist/tree-of-sophia-software.zip",
        "dist/tree-of-sophia-software.zip.manifest.json",
    ] {
        assert!(candidate_run.contains(file));
    }
    assert!(candidate_run.contains("GITHUB_OUTPUT"));
    let provisional = named_step(software_package, "Preserve unqualified Software candidate");
    assert_eq!(
        provisional["with"]["name"].as_str(),
        Some("tos-software-candidate-unqualified-${{ github.run_id }}-${{ github.run_attempt }}")
    );
    assert_eq!(provisional["with"]["compression-level"].as_i64(), Some(0));
    assert_eq!(provisional["with"]["retention-days"].as_i64(), Some(1));
    let candidate_index = software_package
        .iter()
        .position(|step| std::ptr::eq(step, bind_candidate))
        .unwrap();
    let browser_index = software_package
        .iter()
        .position(|step| std::ptr::eq(step, browser))
        .unwrap();
    let provisional_index = software_package
        .iter()
        .position(|step| std::ptr::eq(step, provisional))
        .unwrap();
    assert!(browser_index < candidate_index && candidate_index < provisional_index);
    for (output, step_output) in [
        ("candidate_sha256", "candidate_sha256"),
        ("candidate_sidecar_sha256", "candidate_sidecar_sha256"),
        ("candidate_bytes", "candidate_bytes"),
        ("candidate_sidecar_bytes", "candidate_sidecar_bytes"),
    ] {
        let expected = format!("${{{{ steps.candidate.outputs.{step_output} }}}}");
        assert_eq!(
            jobs["software_package"]["outputs"][output].as_str(),
            Some(expected.as_str())
        );
    }

    let gate_steps = steps(jobs, "required_gate");
    let native_gate = one_step(gate_steps, "\"$TOS_SOFTWARE_CI_EXECUTOR\" gate");
    let fetch_candidate = named_step(gate_steps, "Fetch candidate after every selected check");
    let verify_candidate = named_step(gate_steps, "Verify candidate against package job outputs");
    let preserve_candidate = named_step(gate_steps, "Preserve validated software candidate");
    let gate_index = gate_steps
        .iter()
        .position(|step| std::ptr::eq(step, native_gate))
        .unwrap();
    let fetch_index = gate_steps
        .iter()
        .position(|step| std::ptr::eq(step, fetch_candidate))
        .unwrap();
    let verify_index = gate_steps
        .iter()
        .position(|step| std::ptr::eq(step, verify_candidate))
        .unwrap();
    let preserve_index = gate_steps
        .iter()
        .position(|step| std::ptr::eq(step, preserve_candidate))
        .unwrap();
    assert!(
        gate_index < fetch_index && fetch_index < verify_index && verify_index < preserve_index
    );
    for step in [fetch_candidate, verify_candidate, preserve_candidate] {
        assert!(step["if"].as_str().unwrap().contains("success()"));
        assert!(
            step["if"]
                .as_str()
                .unwrap()
                .contains("software_mode != 'none'")
        );
    }
    assert_eq!(
        fetch_candidate["with"]["name"].as_str(),
        provisional["with"]["name"].as_str()
    );
    let verify_run = run(verify_candidate);
    for behavior in [
        "process.env[`${prefix}_SHA256`]",
        "process.env[`${prefix}_BYTES`]",
        "lstatSync",
        "stat.size",
        "hash.digest('hex')",
    ] {
        assert!(
            verify_run.contains(behavior),
            "candidate admission lost {behavior}"
        );
    }
    for (name, output) in [
        ("CANDIDATE_SHA256", "candidate_sha256"),
        ("CANDIDATE_SIDECAR_SHA256", "candidate_sidecar_sha256"),
        ("CANDIDATE_BYTES", "candidate_bytes"),
        ("CANDIDATE_SIDECAR_BYTES", "candidate_sidecar_bytes"),
    ] {
        let expected = format!("${{{{ needs.software_package.outputs.{output} }}}}");
        assert_eq!(
            verify_candidate["env"][name].as_str(),
            Some(expected.as_str())
        );
    }
    assert_eq!(
        preserve_candidate["with"]["name"].as_str(),
        Some("tree-of-sophia-software-${{ github.sha }}")
    );
    let postgres_steps = steps(jobs, "postgres");
    let pg_configure = run(one_step(postgres_steps, "CARGO_HOME=%s/cargo-home"));
    assert!(pg_configure.contains("CARGO_TARGET_DIR=%s/cargo-target"));
    assert!(pg_configure.contains("\"$RUNNER_TEMP\" >> \"$GITHUB_ENV\""));
    let postgres = one_step(postgres_steps, "--test postgres_durable_lab");
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
    assert!(!jobs["postgres"]["services"]["postgres"].is_badvalue());
    assert!(jobs["rust"]["services"].is_badvalue());
    let rust_job = &jobs["rust"];
    assert_eq!(
        rust_job["name"].as_str(),
        Some("Rust / ${{ matrix.group }}")
    );
    assert_eq!(rust_job["needs"].as_str(), Some("plan"));
    assert_eq!(rust_job["strategy"]["fail-fast"].as_bool(), Some(false));
    assert_eq!(
        rust_job["strategy"]["matrix"]["group"]
            .as_vec()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["cold-install", "conformance", "workspace"])
    );
    let rust_steps = steps(jobs, "rust");
    let resource_profile = one_step(rust_steps, "os.availableParallelism()");
    let resource_run = run(resource_profile);
    assert_eq!(
        resource_profile["env"]["CI_CARGO_GROUP"].as_str(),
        Some("${{ matrix.group || github.job }}")
    );
    for job in ["software_prepare", "software", "software_package"] {
        let st = steps(jobs, job);
        let profile = one_step(st, "os.availableParallelism()");
        assert_eq!(
            run(profile),
            resource_run,
            "{job} must use the shared resource policy"
        );
        assert_eq!(
            profile["env"]["CI_CARGO_GROUP"].as_str(),
            Some("${{ matrix.group || github.job }}")
        );
    }
    for evidence in [
        "Cpus_allowed_list",
        "cpu.max",
        "MemAvailable",
        "memory.max",
        "cgroup_ancestor_paths",
        "cgroup_ancestor_scope_complete",
        "assumed_unlimited",
        "12 * GIB",
        "CARGO_BUILD_JOBS=${buildJobs}",
        "TOS_GROWTH_JOBS=${buildJobs}",
        "growth_jobs: buildJobs",
        "rustc-version.txt",
        "cargo-version.txt",
        "tos_ci_cargo_resource_profile_v1",
    ] {
        assert!(
            resource_run.contains(evidence),
            "missing Rust resource evidence: {evidence}"
        );
    }
    let lane_index = rust_steps
        .iter()
        .position(|step| run(step).contains("--run rust_workspace"))
        .unwrap();
    for preparation in [
        "Install pinned Rust toolchain and WASM target",
        "Prepare retained source-cut fixture",
        "Install verified wasm-bindgen CLI",
        "Prepare isolated fs-verity test filesystem",
    ] {
        let position = rust_steps
            .iter()
            .position(|step| step["name"].as_str() == Some(preparation))
            .unwrap();
        assert!(
            position < lane_index,
            "matrix lane lost preparation step: {preparation}"
        );
    }
    let rust_run = run(one_step(rust_steps, "--run rust_workspace"));
    assert!(rust_run.contains("--rust-group \"${{ matrix.group }}\""));
    assert!(rust_run.contains("--cargo-timings"));
    assert!(rust_run.contains("--growth-jobs \"$TOS_GROWTH_JOBS\""));
    assert!(rust_run.contains("--lane-timeout-ms 10800000"));
    let before_lane = rust_steps
        .iter()
        .find(|step| step["name"].as_str() == Some("Capture pre-lane cgroup counters"))
        .unwrap();
    assert!(run(before_lane).contains("cpu.stat"));
    assert!(run(before_lane).contains("memory.peak"));
    let after_lane = rust_steps
        .iter()
        .find(|step| step["name"].as_str() == Some("Capture post-lane cgroup counters"))
        .unwrap();
    assert!(run(after_lane).contains("cpu.stat"));
    assert!(run(after_lane).contains("memory.peak"));
    assert!(after_lane["if"].as_str().unwrap().contains("always()"));
    let diagnostics = named_step(rust_steps, "Upload non-authoritative Cargo diagnostics");
    assert!(diagnostics["if"].as_str().unwrap().contains("always()"));
    let artifact_name = diagnostics["with"]["name"].as_str().unwrap();
    assert!(artifact_name.contains("${{ github.run_id }}"));
    assert!(artifact_name.contains("${{ github.run_attempt }}"));
    assert!(artifact_name.contains("${{ matrix.group || 'single' }}"));
    assert!(
        diagnostics["with"]["path"]
            .as_str()
            .unwrap()
            .contains("resource-profile.json")
    );
    assert!(
        diagnostics["with"]["path"]
            .as_str()
            .unwrap()
            .contains("cargo-timings")
    );
    assert!(diagnostics["with"]["name"].as_str().unwrap().starts_with(
        "${{ github.job }}-diagnostics-${{ github.run_id }}-${{ github.run_attempt }}-"
    ));
    for job in ["software_prepare", "software", "software_package"] {
        let st = steps(jobs, job);
        let before = named_step(st, "Capture pre-lane cgroup counters");
        let after = named_step(st, "Capture post-lane cgroup counters");
        let diagnostic = named_step(st, "Upload non-authoritative Cargo diagnostics");
        assert!(run(before).contains("cpu_stat"));
        assert!(run(after).contains("memory_peak"));
        assert!(after["if"].as_str().unwrap().contains("always()"));
        assert!(diagnostic["if"].as_str().unwrap().contains("always()"));
        let name = diagnostic["with"]["name"].as_str().unwrap();
        assert!(name.contains("${{ github.job }}"));
        assert!(name.contains("${{ github.run_id }}"));
        assert!(name.contains("${{ github.run_attempt }}"));
        assert!(name.contains("${{ matrix.group || 'single' }}"));
        assert!(
            diagnostic["with"]["path"]
                .as_str()
                .unwrap()
                .contains("cargo-resource-profile/resource-profile.json")
        );
    }
    assert_eq!(sparse(jobs, "postgres"), sparse(jobs, "rust"));
    assert!(
        run(one_step(postgres_steps, "cargo build"))
            .contains("-p tos-validation --bin tos-schema-worker")
    );
    one_step(postgres_steps, "mkfs.ext4 -q -F -O verity");
    one_step(postgres_steps, "sudo umount");
    let gate = named_step(
        steps(jobs, "required_gate"),
        "Require every selected check to pass",
    );
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
    let postgres = sparse(jobs, "postgres");
    for checkout in [&rust, &software, &worker, &postgres] {
        for path in ["/ToS/source-witnesses/", "/ToS/", "/ToS/candidate-intake/"] {
            assert!(!checkout.contains(path));
        }
    }
}

#[cfg(target_os = "linux")]
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
