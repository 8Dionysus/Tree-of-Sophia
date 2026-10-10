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
    // New source-visible errors fail together; historical missing links stay historical.
    fs::write(root.join("README.md"), "<<<<<<< branch\n[new](absent.md)\n").unwrap();
    let native = invoke("plan", None, false);
    assert_eq!(native.status.code(), Some(1));
    let diagnostic = String::from_utf8(native.stderr).unwrap();
    assert!(diagnostic.contains("README.md: unresolved merge marker"));
    assert!(diagnostic.contains("README.md: missing repository link target absent.md"));
    let good = serde_json::json!({"plan":{"result":"success","outputs":{"software_mode":"reader","worker":"true","rust":"false"}}, "software":{"result":"success"},"worker":{"result":"success"},"rust":{"result":"skipped"}});
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
    for needle in [
        "--no-default-features",
        "--bin tos-software-ci",
        "--message-format=json",
        "--github-output \"$GITHUB_OUTPUT\"",
    ] {
        assert!(run(build).contains(needle));
    }
    assert!(run(selection).contains("--repo-root \"$GITHUB_WORKSPACE\""));
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
    assert!(run(full).contains("--command-timeout-ms 900000"));
    assert!(full["if"].as_str().unwrap().contains("== 'full'"));
    let reader = one_step(software, "--run software_reader");
    assert!(reader["if"].as_str().unwrap().contains("== 'reader'"));
    let package = run(one_step(software, "software install --archive"));
    for needle in [
        "software build --root",
        "software verify --archive",
        "env -i PATH=",
        "software-limits --repo-root",
    ] {
        assert!(package.contains(needle));
    }
    assert!(!package.contains("pip install"));
    assert!(package.find("mkdir -p -- \"$root/dist\"") < package.find("software build --root"));
    let receipts = run(one_step(software, "software-receipts"));
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
    let lane = one_step(rust, "--run rust_workspace");
    assert!(run(lane).contains("--lane-timeout-ms 5400000"));
    for key in [
        "TOS_NATIVE_OWNER_COMMAND_PATH",
        "TOS_NATIVE_OWNER_COMMAND_BIN",
    ] {
        assert_eq!(
            lane["env"][key].as_str(),
            Some("${{ runner.temp }}/cargo-target/debug/tos-native-owner-command")
        );
    }
    let postgres = one_step(
        rust,
        "cargo test -p tos-command --features postgres-lab --test postgres_durable_lab --locked -- --nocapture",
    );
    assert!(postgres["env"]["TOS_CMD_POSTGRES_URL"].as_str().is_some());
    assert!(!jobs["rust"]["services"]["postgres"].is_badvalue());
    let gate = steps(jobs, "required_gate").last().unwrap();
    assert_eq!(run(gate), "\"$TOS_SOFTWARE_CI_EXECUTOR\" gate");
    assert_eq!(
        gate["env"]["CI_NEEDS"].as_str(),
        Some("${{ toJSON(needs) }}")
    );
}

#[test]
fn edge_hosts_build_and_select_the_exact_native_product_before_checks() {
    let w = workflow("repo-validation.yml");
    let jobs = &w["jobs"];
    assert!(sparse(jobs, "software").contains("/.github/workflows/cloudflare-edge.yml"));
    let worker = steps(jobs, "worker");
    assert!(
        run(one_step(
            worker,
            "cargo +1.98.1 build --locked -p tos-access"
        ))
        .contains("--bin tos-access --target x86_64-unknown-linux-gnu")
    );
    let test = one_step(worker, "test -x \"$TOS_ACCESS_BIN\"");
    assert_eq!(
        test["env"]["TOS_ACCESS_BIN"].as_str(),
        Some("${{ runner.temp }}/worker-cargo-target/x86_64-unknown-linux-gnu/debug/tos-access")
    );
    let w = workflow("cloudflare-edge.yml");
    let demand = &w["on"]["workflow_dispatch"]["inputs"]["build_seconds"];
    assert_eq!(demand["required"].as_bool(), Some(true));
    assert_eq!(demand["type"].as_str(), Some("number"));
    assert!(demand["default"].is_badvalue());
    let job = &w["jobs"]["contract"];
    assert_eq!(job["timeout-minutes"].as_i64(), Some(120));
    let st = job["steps"].as_vec().unwrap();
    let native = one_step(st, "cargo +1.98.1 build --locked -p tos-access");
    let wasm = one_step(st, "--out-name tos_web_rules --out-dir generated");
    let check = one_step(st, "npm run check");
    for needle in ["TOS_ACCESS_BIN=", "TOS_BUILD_MAX_SECONDS="] {
        assert!(run(native).contains(needle));
    }
    for needle in [
        "rustup toolchain install 1.98.1 --profile minimal --target wasm32-unknown-unknown",
        "cargo +1.98.1 build --locked --release -p tos-web-rules --features wasm --target wasm32-unknown-unknown",
        "b51f0208fdff83515a787bd8ab9ac5865ed84dabb66d0c709957bb59793c645f",
    ] {
        assert!(run(wasm).contains(needle));
    }
    let pos = |a| st.iter().position(|s| std::ptr::eq(s, a)).unwrap();
    assert!(pos(native) < pos(wasm) && pos(wasm) < pos(check));
    assert!(run(st.last().unwrap()).contains("npx wrangler deploy --dry-run"));
}

fn sparse_contains(patterns: &std::collections::BTreeSet<String>, path: &str) -> bool {
    patterns.iter().any(|pattern| {
        let included = pattern.strip_prefix('/').unwrap_or(pattern);
        included == path || included.ends_with('/') && path.starts_with(included)
    })
}

#[test]
fn sparse_checkout_preserves_exact_fixtures_without_whole_corpus() {
    use std::collections::BTreeSet;
    let root = repository();
    let w = workflow("repo-validation.yml");
    let jobs = &w["jobs"];
    let rust = sparse(jobs, "rust");
    let software = sparse(jobs, "software");
    let worker = sparse(jobs, "worker");
    let required_sources: BTreeSet<&str> = BTreeSet::from([
        "QUESTBOOK.md",
        "ToS/candidate-intake/AGENTS.md",
        "ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/mode-b/edges.csv",
        "ToS/candidate-intake/zarathustra/concept-workbench-v1/english-translation-candidate.v1.schema.json",
        "ToS/candidate-intake/zarathustra/concept-workbench-v1/plan.v1.json",
        "ToS/candidate-intake/zarathustra/concept-workbench-v1/word-analysis-task.v1.schema.json",
        "ToS/canon/**/node.human-forms.json",
        "ToS/canon/**/node.json",
        "ToS/canon/AGENTS.md",
        "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv",
        "ToS/derived-exports/AGENTS.md",
        "ToS/doctrine/AGENTS.md",
        "ToS/philosophy/AGENTS.md",
        "ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json",
        "ToS/philosophy/philosophy.manifest.json",
        "ToS/public-compatibility/AGENTS.md",
        "ToS/public-compatibility/source_node.example.json",
        "ToS/research-packets/AGENTS.md",
        "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md",
        "ToS/research-packets/foundation-laboratory-2026-07/ZARATHUSTRA_PARTS_2_3_PROVISION_IDENTITY_RESEARCH.md",
        "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-a-occurrences-only.json",
        "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json",
        "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-c-invalid-model-promotion.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/lab.manifest.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-a.anchor.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-b-unicode.txt",
        "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-b.anchor.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-c.anchor.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/editorial-policy.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-a-raw-ocr.txt",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-a.layer.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-b-diplomatic.txt",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-b.layer.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-c.layer.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-a-source-layout-observation.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-b-competing-segmentations.json",
        "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-c-invalid-acceptance.json",
        "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-a-one-to-one-proposal.json",
        "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-b-competing-mappings.json",
        "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-c-invalid-acceptance.json",
        "ToS/review-ledger/2026-09-10-mysl-collection-order-source-reading.md",
        "ToS/review-ledger/AGENTS.md",
        "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/",
        "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/",
        "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-d34e996729a4bb5b04a3a6cc486f2e1ef7e020747fc18afcf10e5ab7628feb73/",
        "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-fafe3ef84b65b29968511c6a06ff018e46571d11c604ec14fc5ef5018f70c1b2/",
        "ToS/source-witnesses/.record-revisions/2c4c3a4f5cb2cbf1713ebdaa0b27dfcb0729cf980f33591a6e1e2ea6296b8d25-f63f2f0562a6a662be9c5340ddde5686a6de35a8991ad2ad7b53e3a8fd134eba/",
        "ToS/source-witnesses/.record-revisions/3b6ca195bb9bb9fb57cc1e0d9bece8b18011ef12c3d99d614aac5fa3760ad712-8d38fda8bf756906f8ed3543a8cc069082d39b04b188db5050d76a2ad663a497/",
        "ToS/source-witnesses/.record-revisions/709df7fb307a1331d25fa253f7159a3ee27b3862898ddf8db74c6cbfc6965438-75afe571bb0254a738c20c0d5d09fac8b11ce0f299068b534ef652ef09575422/",
        "ToS/source-witnesses/AGENTS.md",
        "ToS/source-witnesses/agents/constantin-georg-naumann/",
        "ToS/source-witnesses/agents/erasmus-of-rotterdam/agent.json",
        "ToS/source-witnesses/agents/friedrich-nietzsche/agent.json",
        "ToS/source-witnesses/artifacts/old-babylonian/susa/hammurabi-stele-sb-8/artifact-witness.json",
        "ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/artifact-witness.json",
        "ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/rights.json",
        "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645-plus-a00649a-i/artifact-witness.json",
        "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.human-forms.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/membership-claims.jsonl",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/responsibility-claims.jsonl",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/source-revision-history.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/anchors.jsonl",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/work-boundary-map.json",
        "ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md",
        "ToS/source-witnesses/discovery/provenance.jsonl",
        "ToS/source-witnesses/discovery/runs/old-babylonian-gilgamesh-cbs7771.2026-08-22.v1.json",
        "ToS/source-witnesses/discovery/runs/zarathustra-parts-2-3-provision-identity.2026-08-01.v1.json",
        "ToS/source-witnesses/documents/friedrich-nietzsche/naumann-letter-705/",
        "ToS/source-witnesses/links/cdli/cdlb-2006-1/article/link.json",
        "ToS/source-witnesses/links/internet-archive/onfoursongsconta00good/landing/link.human-forms.json",
        "ToS/source-witnesses/links/internet-archive/onfoursongsconta00good/landing/link.json",
        "ToS/source-witnesses/places/chemnitz/place.json",
        "ToS/source-witnesses/relations/mysl-1996-volume-2-member-order/source-claims.jsonl",
        "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/",
        "ToS/source-witnesses/relations/oim-a00645-physical-composition/source-claims.jsonl",
        "ToS/source-witnesses/research-corpora/foundation-source-routes/research-corpus.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/rights.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-2/editions/chemnitz-schmeitzner-1883-part-2/items/dta-sbb-corrected-tei-p5/source-metadata-snapshot.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/editions/saint-petersburg-prometey-1911-fourth/items/rsl-neb-scan-pdf/rights.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/expression.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/responsibility-claims.jsonl",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/edition-reading-admission.dta-ekgwb.za-i-vorrede-1.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/initial-sign-packet.v5.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/provenance.opening-sentence-alignment.za-i-vorrede-1.v2.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-layer.za-i-vorrede-1-p1.antonovsky-1911-embedded.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-layer.za-i-vorrede-1-p1.dta-machine.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.antonovsky-1911-layout.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.antonovsky-1911-sentence-proposal.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-layout.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-sentence-proposal.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-samples.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-target-anchors.v1.jsonl",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/translation-alignment.za-i-vorrede-1-opening-sentence.dta-1883-antonovsky-1911.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-opening-sentence-alignment.plan.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/work.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/der-fall-wagner/work.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/ecce-homo/work.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/goetzen-daemmerung/work.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/numbered-unit-label-correspondence.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/transfer-candidate-page-crosswalk.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/numbered-unit-page-map.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.human-forms.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/work.json",
        "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/expression.json",
        "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-claims.jsonl",
        "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/work.json",
        "ToS/source_home.manifest.json",
        "ToS/zarathustra/AGENTS.md",
        "access/tests/fixtures/knowledge-contract/ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.human-forms.json",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/agents/friedrich-nietzsche/agent.json",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/places/chemnitz/place.json",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json",
        "mechanics/agon/parts/threshold-intake/schemas/tos-agon-threshold-intake.schema.json",
        "mechanics/agon/parts/threshold-registry/config/tos_agon_threshold_intakes.config.json",
        "mechanics/agon/parts/threshold-registry/generated/tos_agon_threshold_intake_registry.min.json",
        "mechanics/agon/parts/threshold-registry/schemas/tos-agon-threshold-intake-registry.schema.json",
        "mechanics/experience/parts/adoption-boundary/examples/tos_adoption_boundary_dossier.example.json",
        "mechanics/experience/parts/adoption-boundary/examples/tos_no_runtime_adoption_guard.example.json",
        "mechanics/experience/parts/adoption-boundary/schemas/tos_adoption_boundary_dossier_v1.json",
        "mechanics/experience/parts/adoption-boundary/schemas/tos_no_runtime_adoption_guard_v1.json",
        "mechanics/experience/parts/candidate-review/examples/aoa_experience_candidate_dossier.example.json",
        "mechanics/experience/parts/candidate-review/examples/tos_intake_boundary_decision.example.json",
        "mechanics/experience/parts/candidate-review/schemas/aoa_experience_candidate_dossier_v1.json",
        "mechanics/experience/parts/candidate-review/schemas/tos_intake_boundary_decision_v1.json",
        "mechanics/experience/parts/governance-boundary/examples/tos_governance_dossier_boundary_v1.example.json",
        "mechanics/experience/parts/governance-boundary/examples/tos_governance_review_note.example.json",
        "mechanics/experience/parts/governance-boundary/schemas/tos_governance_dossier_boundary_v1.json",
        "mechanics/experience/parts/governance-boundary/schemas/tos_governance_review_note_v1.json",
        "mechanics/experience/parts/installation-boundary/examples/tos_installation_dossier_boundary_v1.example.json",
        "mechanics/experience/parts/installation-boundary/schemas/tos_installation_dossier_boundary_v1.json",
        "mechanics/experience/parts/pattern-review/examples/tos_pattern_review_note.example.json",
        "mechanics/experience/parts/pattern-review/schemas/tos_pattern_review_note_v1.json",
        "mechanics/experience/parts/service-office-boundary/examples/tos_no_runtime_office_write_guard_v1.example.json",
        "mechanics/experience/parts/service-office-boundary/examples/tos_service_dossier_boundary_v1.example.json",
        "mechanics/experience/parts/service-office-boundary/schemas/tos_no_runtime_office_write_guard_v1.json",
        "mechanics/experience/parts/service-office-boundary/schemas/tos_service_dossier_boundary_v1.json",
        "mechanics/experience/parts/write-guards/examples/tos_no_direct_write_guard.example.json",
        "mechanics/experience/parts/write-guards/schemas/tos_no_direct_write_guard_v1.json",
        "mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json",
        "mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json",
        "mechanics/questbook/parts/dispatch-contracts/schemas/quest.schema.json",
        "mechanics/questbook/parts/dispatch-contracts/schemas/quest_dispatch.schema.json",
        "mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md",
        "quests/TOS-Q-0001.yaml",
        "quests/TOS-Q-0002.yaml",
        "quests/TOS-Q-0003.yaml",
        "quests/TOS-Q-0004.yaml",
    ]);
    let required_fixtures: BTreeSet<&str> = BTreeSet::from([
        "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/item.json",
        "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/item.manifest.json",
        "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/provenance.jsonl",
        "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/rights.json",
        "rust/crates/tos-command/src/source_payload_custody.rs",
    ]);
    let software_schemas: BTreeSet<&str> = BTreeSet::from([
        "ToS/candidate-intake/zarathustra/concept-workbench-v1/english-translation-candidate.v1.schema.json",
        "ToS/candidate-intake/zarathustra/concept-workbench-v1/word-analysis-task.v1.schema.json",
    ]);
    let shared_compiled_source_inputs: BTreeSet<&str> =
        BTreeSet::from(["ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json"]);
    for path in &required_sources {
        assert!(
            sparse_contains(&rust, path),
            "Rust sparse input missing: {path}"
        );
    }
    let lanes: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("docs/validation/validation_lanes.json")).unwrap(),
    )
    .unwrap();
    let release_tests: Vec<_> = lanes["command_sequences"]["release_check"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["label"].as_str().unwrap().starts_with("run tests:"))
        .map(|s| {
            s["command"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        release_tests,
        vec![
            vec!["cargo", "test", "--locked", "-p", "tos-access"],
            vec!["cargo", "test", "--locked", "-p", "tos-query"],
            vec![
                "cargo",
                "test",
                "--locked",
                "-p",
                "tos-command",
                "-p",
                "tos-source-store"
            ],
        ]
    );
    for path in &required_fixtures {
        assert!(
            sparse_contains(&software, path),
            "software sparse input missing: {path}"
        );
    }
    let mut worker_schemas = software_schemas.clone();
    worker_schemas.extend(["ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-search-result.v1.schema.json","ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-request.v2.schema.json","ToS/candidate-intake/zarathustra/reading-workbench-v1/reading-search-result.v1.schema.json","ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"]);
    for path in &software_schemas {
        assert!(root.join(path).is_file());
        assert!(sparse_contains(&software, path));
    }
    for path in &worker_schemas {
        assert!(root.join(path).is_file());
        assert!(worker.contains(&format!("/{path}")));
    }
    let fixture_root = "ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/";
    let include = regex::Regex::new(r#"include_(?:bytes|str)!\s*\(\s*"([^"]+)"\s*\)"#).unwrap();
    let mut inputs = BTreeSet::new();
    for relative in [
        "rust/crates/tos-compiler/tests/generic_xml_uxlc_lab.rs",
        "rust/crates/tos-compiler/tests/generic_xml_uxlc_lab/inputs.rs",
    ] {
        let source = root.join(relative);
        let text = std::fs::read_to_string(&source).unwrap();
        for capture in include.captures_iter(&text) {
            if !capture[1].contains("generic-xml-resource-inventory-uxlc-abc-v1") {
                continue;
            }
            let target = source
                .parent()
                .unwrap()
                .join(&capture[1])
                .canonicalize()
                .unwrap();
            assert!(target.is_file());
            let path = target
                .strip_prefix(&root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            assert!(path.starts_with(fixture_root));
            inputs.insert(path);
        }
    }
    assert_eq!(inputs.len(), 58);
    for path in inputs {
        for checkout in [&rust, &software] {
            assert!(
                checkout.contains(&format!("/{path}")),
                "embedded fixture missing: {path}"
            );
        }
    }
    assert!(rust.contains("!/ToS/source-witnesses/**/payload/"));
    for checkout in [&rust, &software, &worker] {
        for path in &shared_compiled_source_inputs {
            assert!(root.join(path).is_file());
            assert!(checkout.contains(&format!("/{path}")));
        }
        for path in ["/ToS/source-witnesses/", "/ToS/", "/ToS/candidate-intake/"] {
            assert!(!checkout.contains(path));
        }
    }
    for checkout in [&rust, &software] {
        assert!(!checkout.contains(&format!("/{fixture_root}")));
    }
}

#[path = "support/validation_lanes.rs"]
mod validation_lanes_cases;
