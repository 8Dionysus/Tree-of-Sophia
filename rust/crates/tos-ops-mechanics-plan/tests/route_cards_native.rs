//! Native route-card CLI contracts with explicit positive and refusal cases.
#[cfg(target_os = "linux")]
#[test]
fn route_currentness_and_nested_consumers_preserve_owned_contracts() {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
            "tos-route-cards-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    let root = &fixture.0;
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write(
        "rust/crates/tos-ops-mechanics-plan/src/route_cards.rs",
        "// Native owner fixture\n",
    );
    write("scripts/harness.py", "# fixture route\n");
    let clean = "# AGENTS.md\r\n## Role\r\nSource route.\r\n## Read Before Editing\r\nRead README.md when public navigation is relevant.\r\n## Boundary Law\r\nREADME.md VALIDATION.md ROADMAP.md BOUNDARIES.md ToS/ mechanics/\r\n";
    write("AGENTS.md", clean);
    write(
        ".github/AGENTS.md",
        "# AGENTS.md\nThis card applies to GitHub support.\n",
    );
    write(
        "ToS/AGENTS.md",
        "# AGENTS.md\nThis card applies to source.\nToS/source_home.manifest.json ToS/doctrine/KNOWLEDGE_MODEL.md ToS/doctrine/NODE_CONTRACT.md\n",
    );
    write(
        "ToS/branch/AGENTS.md",
        "# AGENTS.md\nThis card applies to branch metadata.\n",
    );
    write("ToS/branch/target.md", "source\n");
    write("docs/owner.md", "owner\u{1c}handoff\r\nauthority\n");
    for path in [
        "README.md",
        "VALIDATION.md",
        "ROADMAP.md",
        "BOUNDARIES.md",
        "ToS/source_home.manifest.json",
        "ToS/doctrine/KNOWLEDGE_MODEL.md",
        "ToS/doctrine/NODE_CONTRACT.md",
    ] {
        write(path, "source\n");
    }
    let inventory = serde_json::json!({"schema_version":"tos_agents_route_inventory_v1","owner_repo":"Tree-of-Sophia","owner_surface":"AGENTS.md","validator":"rust/crates/tos-ops-mechanics-plan/src/route_cards.rs","currentness":".agents/agents-route.current.json","harness":"scripts/harness.py","route_card_discovery":{"root_cards":["AGENTS.md"],"route_roots":[".github","ToS","docs"],"preserved_non_cards":[]},"influencing_surfaces":[{"path":"README.md","role":"orientation"}],"context_budget":{"limit":1000.0,"tiny":1e-7},"scope_duplication_policy":{"byte_identical_nested_cards":"error"},"task_routes":[{"id":"handoff","target":"ToS/branch/target.md","owner_route":"docs/owner.md","on_demand_surfaces":["README.md","docs/owner.md"],"validation_paths":["VALIDATION.md"],"completion_evidence":["ToS/branch/target.md"],"handoff_routes":["aoa-memo"]}]});
    write(
        "docs/validation/agents_route_inventory.json",
        &serde_json::to_string_pretty(&inventory).unwrap(),
    );
    let invoke = |program: &Path, args: &[&str]| -> Output {
        let result = Command::new("/usr/bin/timeout")
            .args(["--kill-after=2", "40"])
            .arg(program)
            .args(args)
            .current_dir(root)
            .env_clear()
            .env("PATH", "/usr/bin")
            .output()
            .unwrap();
        assert!(
            result.stdout.len() + result.stderr.len() < 2 * 1024 * 1024,
            "fixture output ceiling"
        );
        assert_ne!(result.status.code(), Some(124), "fixture deadline");
        result
    };
    assert!(
        invoke(Path::new("/usr/bin/git"), &["init", "--quiet"])
            .status
            .success()
    );
    assert!(
        invoke(Path::new("/usr/bin/git"), &["add", "--", "."])
            .status
            .success()
    );
    let executable = std::env::var_os("TOS_ROUTE_CARDS_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-route-cards").into());
    let executable = PathBuf::from(executable);
    let native = |args: &[&str]| -> Output {
        let root_text = root.to_str().unwrap();
        let mut full = vec!["--repo-root", root_text];
        full.extend_from_slice(args);
        invoke(&executable, &full)
    };
    let rebuild = || {
        let a = native(&["build", "--output", "candidate/route.json"]);
        assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
        fs::copy(
            root.join("candidate/route.json"),
            root.join(".agents/agents-route.current.json"),
        )
        .unwrap();
    };
    fs::create_dir_all(root.join(".agents")).unwrap();
    rebuild();
    let compare = |expected: i32| {
        let a = native(&["validate"]);
        assert_eq!(
            a.status.code(),
            Some(expected),
            "{}{}",
            String::from_utf8_lossy(&a.stdout),
            String::from_utf8_lossy(&a.stderr)
        );
        assert!(
            a.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&a.stderr)
        );
        String::from_utf8(a.stdout).unwrap()
    };
    compare(0);
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("candidate/route.json")).unwrap()).unwrap();
    assert_eq!(
        value["task_routes"][0]["inheritance_stack"],
        serde_json::json!(["AGENTS.md", "ToS/AGENTS.md", "ToS/branch/AGENTS.md"])
    );
    assert_eq!(value["task_routes"][0]["owner_in_inheritance_stack"], false);
    assert_eq!(value["task_routes"][0]["owner_handoff"]["lines"], 3);
    let source = fs::read(root.join("AGENTS.md")).unwrap();
    let card = value["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["path"] == "AGENTS.md")
        .unwrap();
    assert_eq!(card["bytes"], source.len());
    assert_eq!(
        card["sha256"],
        tos_ops_mechanics_plan::route_cards::sha256_bytes(&source)
    );
    assert!(source.contains(&b'\r'));
    let check_native = native(&["build", "--check", "--output", "candidate/route.json"]);
    assert!(check_native.status.success());
    let external = root.with_extension("external.json");
    assert!(
        native(&["build", "--output", external.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(
        fs::read(&external).unwrap(),
        fs::read(root.join("candidate/route.json")).unwrap()
    );
    fs::remove_file(&external).unwrap();
    write("candidate/route.json", "stale\n");
    let check_native = native(&["build", "--check", "--output", "candidate/route.json"]);
    assert_eq!(check_native.status.code(), Some(1));
    write("README.md", "changed source\n");
    assert!(compare(1).contains("generated AGENTS route currentness is stale"));
    rebuild();
    compare(0);
    for (body, refusal_count) in [
        (
            "Read README.md for setup.\nReview README.md before editing.\n",
            2,
        ),
        ("Open README.md when public navigation changes.\n", 0),
        (
            "Open README.md only when its human explanation is relevant.\n",
            0,
        ),
        (
            "## What lives here\nThis child retains only its class-local semantic delta.\n## Boundary\nInherit the nearest validation route.\n",
            0,
        ),
    ] {
        write(
            "ToS/branch/AGENTS.md",
            &format!("# AGENTS.md\nThis card applies to branch metadata.\n{body}"),
        );
        rebuild();
        let findings = compare(if refusal_count == 0 { 0 } else { 1 });
        assert_eq!(
            findings.matches("unconditional README").count(),
            refusal_count,
            "{findings}"
        );
    }
    write(
        "ToS/branch/AGENTS.md",
        "# AGENTS.md\nThis card applies to branch metadata.\n## Validation\nRun:\n## Boundary\n- First:\n- Second:\n",
    );
    fs::remove_file(root.join("VALIDATION.md")).unwrap();
    write(
        "AGENTS.md",
        &clean.replace("VALIDATION.md", "validation owner"),
    );
    rebuild();
    let findings = compare(1);
    assert!(findings.contains("orphan extraction lead-in"), "{findings}");
    assert!(
        findings.contains("missing nearest validation route"),
        "{findings}"
    );
    write("VALIDATION.md", "source\n");
    write("AGENTS.md", clean);
    let bad = "# AGENTS.md\nThis card applies to branch.\n## Role\nRead README.md first.\nUse `git` and `python scripts/missing.py` for routing.\nFOO=bar python scripts/missing.py\n```bash\npython scripts/missing.py\n```\n## Verify\n## Boundary\n- First:\n- Second:\n## Role\n## Operating Card\n| input | source |\n## Heading\u{a0}inside\nBody.\n## Heading\u{a0}inside\nBody.\n## Heading\u{200d}inside\nBody.\n## Heading\u{200d}inside\nBody.\n## Heading\u{e000}inside\nBody.\n## Heading\u{e000}inside\nBody.\nread README\u{301}\nTail:\n";
    write("ToS/branch/AGENTS.md", bad);
    rebuild();
    let findings = compare(1);
    for marker in [
        "inline runnable command",
        "environment-assignment command",
        "runnable command line",
        "fenced procedure/example",
        "empty procedural section",
        "same-level bullet",
        "stacked colon",
        "before EOF",
        "repeated heading 'Role'",
        "unconditional README inventory",
        "local executable reference points to missing path",
        "Operating Card missing field: owner",
        "Operating Card needs check, validation, or tools field",
        "repeated heading 'Heading\\xa0inside'",
        "repeated heading 'Heading\\u200dinside'",
        "repeated heading 'Heading\\ue000inside'",
    ] {
        assert!(findings.contains(marker), "missing {marker}: {findings}");
    }
    write("ToS/branch/AGENTS.md", clean);
    rebuild();
    assert!(compare(1).contains("byte-identical nested card duplicates AGENTS.md"));
    write(
        "ToS/branch/AGENTS.md",
        "# AGENTS.md\nThis card applies to branch metadata.\n",
    );
    write(
        "outside/AGENTS.md",
        "# AGENTS.md\nThis card applies outside.\n",
    );
    assert!(
        invoke(
            Path::new("/usr/bin/git"),
            &["add", "--", "outside/AGENTS.md"]
        )
        .status
        .success()
    );
    write(
        "ToS/untracked/AGENTS.md",
        "# AGENTS.md\nThis card applies to untracked source.\n",
    );
    rebuild();
    let findings = compare(1);
    assert!(findings.contains("outside route-card inventory roots"));
    assert!(findings.contains("discovered AGENTS.md is not tracked"));
    use std::os::unix::fs::symlink;
    symlink(root.join("README.md"), root.join("docs/symlink.md")).unwrap();
    let refusal = native(&["build", "--output", "candidate/refused.json"]);
    assert!(!refusal.status.success());
    assert!(String::from_utf8_lossy(&refusal.stderr).contains("symlink"));
    fs::remove_file(root.join("docs/symlink.md")).unwrap();
    symlink(root.join("candidate"), root.join("linked-parent")).unwrap();
    assert!(
        !native(&["build", "--output", "linked-parent/refused.json"])
            .status
            .success()
    );
    assert!(!root.join("candidate/refused.json").exists());
    let fifo = root.join("candidate/fifo");
    let name = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(
        !native(&["build", "--output", "candidate/fifo"])
            .status
            .success()
    );
    write("docs/validation/agents_route_inventory.json", "not-json\n");
    let findings = compare(1);
    assert!(findings.contains("route inventory is unreadable or malformed"));
    fs::remove_file(root.join("docs/validation/agents_route_inventory.json")).unwrap();
    assert!(compare(1).contains("route inventory is missing"));
}
