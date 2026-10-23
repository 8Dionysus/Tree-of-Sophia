//! Native agent surface boundaries: profile authority and activation.
use serde_json::{Value, json};
use tos_ops_mechanics_plan::agent_surface_validation::{
    activation_policy_issues, profile_binding_issues,
};
fn manifest() -> Value {
    serde_json::from_str(include_str!(
        "../../../../.agents/agent-surface.manifest.json"
    ))
    .unwrap()
}
#[test]
fn immutable_profile_owner_revision_and_duplicate_selection_remain_admission_inputs() {
    let source = manifest();
    let (issues, selected) = profile_binding_issues(&source);
    assert!(issues.is_empty(), "{issues:?}");
    assert!(!selected.is_empty());
    let mut changed = source.clone();
    changed["profile_binding"]["source_ref"] = json!("aoa-skills@unaccepted");
    assert!(
        profile_binding_issues(&changed)
            .0
            .iter()
            .any(|(_, m)| m.contains("source_ref must be"))
    );
    let skill = source["profile_binding"]["sources"][0]["skills"][0].clone();
    changed = source.clone();
    changed["profile_binding"]["sources"][0]["skills"]
        .as_array_mut()
        .unwrap()
        .push(skill);
    let (issues, _) = profile_binding_issues(&changed);
    assert!(
        issues
            .iter()
            .any(|(_, m)| m == "profile selection contains duplicate skills")
    );
    changed = source;
    changed["profile_binding"]["sources"][0]["root"] = json!("../owner");
    assert!(
        profile_binding_issues(&changed)
            .0
            .iter()
            .any(|(_, m)| m == "root must be a safe relative owner root")
    );
}
#[test]
fn activation_preserves_manual_invoke_and_checkpoint_suggest_contracts() {
    assert!(
        activation_policy_issues(Some("explicit-only"), Some("manual"), Some(false), None)
            .is_empty()
    );
    assert!(
        activation_policy_issues(Some("explicit-preferred"), Some("invoke"), Some(true), None)
            .is_empty()
    );
    assert!(
        activation_policy_issues(
            Some("explicit-preferred"),
            Some("suggest"),
            Some(false),
            Some("aoa-checkpoint-closeout-bridge")
        )
        .is_empty()
    );
    let issues = activation_policy_issues(Some("explicit-only"), Some("invoke"), Some(true), None);
    assert_eq!(
        issues,
        vec![
            "implicit_activation_policy='invoke', expected 'manual'",
            "allow_implicit_invocation=True, expected False"
        ]
    );
    assert_eq!(
        activation_policy_issues(None, None, None, None),
        vec!["unknown aoa_invocation_mode None"]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn actual_cli_preserves_probe_owner_currentness_and_public_safety_diagnostics() {
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};
    let root = std::env::temp_dir().join(format!(
        "tos-agent-surface-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    assert!(
        Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let write = |relative: &str, payload: &[u8]| {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, payload).unwrap();
    };

    write(".agents/AGENTS.md", b"# Local owner\n");
    write(".agents/README.md", b"# Public entry\n");
    let source = manifest();
    for port in source["owner_ports"].as_object().unwrap().values() {
        for route in port["currentness_inputs"].as_array().unwrap() {
            write(route.as_str().unwrap(), b"bounded synthetic owned input\n");
        }
    }
    let write_manifest = |m: &Value| {
        write(
            ".agents/agent-surface.manifest.json",
            &serde_json::to_vec_pretty(m).unwrap(),
        )
    };
    let executable = std::env::var_os("TOS_AGENT_SURFACE_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let command = || {
        let mut c = Command::new(&executable);
        c.env_clear()
            .env("PATH", "/usr/bin")
            .arg("--repo-root")
            .arg(&root)
            .arg("--python")
            .arg("/no-python-executable");
        c
    };
    let build = || {
        let out = command().arg("--agent-surface-build").output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    let compare = || {
        let native = command().arg("--agent-surface-validate").output().unwrap();
        assert_eq!(
            native.status.code(),
            Some(1),
            "{}{}",
            String::from_utf8_lossy(&native.stdout),
            String::from_utf8_lossy(&native.stderr)
        );
        String::from_utf8(native.stderr).unwrap()
    };
    write_manifest(&source);
    build();
    let findings = compare();
    assert!(!findings.contains("generated KAG family manifest is missing"));
    assert!(!findings.contains("generated currentness is stale"));
    let mut changed = source.clone();
    let probe = changed["task_probes"][0].clone();
    let probe_id = probe["id"].as_str().unwrap().to_string();
    changed["task_probes"].as_array_mut().unwrap().push(probe);
    write_manifest(&changed);
    build();
    assert!(compare().contains(&format!("duplicate task probe {probe_id}")));
    changed = source.clone();
    let port = &mut changed["owner_ports"]["eval_port"];
    let route = port["manifest"].as_str().unwrap().to_string();
    port["currentness_inputs"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v.as_str() != Some(&route));
    write_manifest(&changed);
    build();
    assert!(compare().contains("eval_port manifest must be a currentness input"));
    changed = source.clone();
    let port = &mut changed["owner_ports"]["eval_port"];
    let route = port["local_owner"].as_str().unwrap().to_string();
    port["currentness_inputs"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v.as_str() != Some(&route));
    write_manifest(&changed);
    build();
    assert!(compare().contains("eval_port local_owner must be a currentness input"));
    write_manifest(&source);
    build();
    write("evals/AGENTS.md", b"changed owner input\n");
    assert!(compare().contains("generated currentness is stale"));
    build();
    write(".agents/README.md", b"/home/private OPENAI_API_KEY\n");
    let findings = compare();
    assert!(findings.contains("contains forbidden public-safety marker '/home/'"));
    assert!(findings.contains("contains forbidden public-safety marker 'OPENAI_API_KEY'"));
    fs::create_dir_all(root.join(".agents/skills/orphan")).unwrap();
    assert!(compare().contains("stale repository-local projection remains"));
}

#[test]
fn actual_builder_preserves_nonempty_package_fixity_and_git_selection() {
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};
    let root = std::env::temp_dir().join(format!(
        "tos-agent-package-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    assert!(
        Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let write = |relative: &str, payload: &[u8]| {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, payload).unwrap();
    };

    let mut source = manifest();
    source["owner_ports"] = json!({});
    write(
        ".agents/agent-surface.manifest.json",
        &serde_json::to_vec_pretty(&source).unwrap(),
    );
    write(".agents/skills/example/SKILL.md",b"---\nname: example\ndescription: bounded source description\nlicense: Apache-2.0\ncompatibility: codex\nmetadata:\n  aoa_scope: repo\n  aoa_status: candidate\n  aoa_invocation_mode: explicit-only\n  aoa_source_skill_path: skills/example\n  aoa_source_repo: 8Dionysus/aoa-skills\n  aoa_portable_profile: local\n---\nBody with references/guide.md and assets/icon.svg.\n");
    write(".agents/skills/example/agents/openai.yaml",b"policy:\n  implicit_activation_policy: manual\n  allow_implicit_invocation: false\ndisplay:\n  label: bounded example\n");
    write(
        ".agents/skills/example/references/guide.md",
        "source companion\r\nРусский\n".as_bytes(),
    );
    write(".agents/skills/example/assets/icon.svg", b"<svg/>\n");
    write(
        ".agents/skills/example/assets/binary.bin",
        &[0xff, 0, 1, 0x80],
    );
    write(".agents/skills/example/checks/probe.txt", b"check\n");
    write(".agents/skills/example/scripts/helper.txt", b"helper\n");
    write(".agents/skills/example/examples/sample.txt", b"example\n");
    write(
        ".agents/skills/example/__pycache__/ignored.pyc",
        b"runtime cache",
    );
    write(
        ".agents/skills/example/.deps/ignored.txt",
        b"runtime dependency",
    );
    let executable = std::env::var_os("TOS_AGENT_SURFACE_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let compare = || -> Value {
        let native = Command::new(&executable)
            .env_clear()
            .env("PATH", "/usr/bin")
            .arg("--repo-root")
            .arg(&root)
            .arg("--agent-surface-build")
            .output()
            .unwrap();
        assert_eq!(
            native.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert!(native.stderr.is_empty());
        let expected = fs::read(root.join(".agents/agent-surface.current.json")).unwrap();
        let checked = Command::new(&executable)
            .env_clear()
            .env("PATH", "/usr/bin")
            .arg("--repo-root")
            .arg(&root)
            .arg("--agent-surface-build")
            .arg("--check")
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        serde_json::from_slice(&expected).unwrap()
    };
    let untracked = compare();
    assert_eq!(untracked["packages"][0]["companion_counts"]["assets"], 2);
    assert_eq!(untracked["package_inventory"]["skill_entrypoints"], 1);
    let before_ignored = untracked["packages"][0]["package_sha256"].clone();
    write(
        ".agents/skills/example/__pycache__/ignored.pyc",
        b"changed runtime cache",
    );
    write(
        ".agents/skills/example/.deps/ignored.txt",
        b"changed runtime dependency",
    );
    assert_eq!(compare()["packages"][0]["package_sha256"], before_ignored);
    assert!(
        Command::new("git")
            .current_dir(&root)
            .args([
                "add",
                "--",
                ".agents/skills/example/SKILL.md",
                ".agents/skills/example/agents/openai.yaml",
                ".agents/skills/example/references/guide.md"
            ])
            .status()
            .unwrap()
            .success()
    );
    let tracked = compare();
    assert_eq!(
        tracked["packages"][0]["companion_counts"]["assets"],
        json!(0)
    );
    assert_ne!(
        untracked["packages"][0]["package_sha256"],
        tracked["packages"][0]["package_sha256"]
    );
    write(
        ".agents/skills/example/references/guide.md",
        b"changed selected companion\n",
    );
    let changed = compare();
    assert_ne!(
        tracked["packages"][0]["package_sha256"],
        changed["packages"][0]["package_sha256"]
    );
}

#[test]
fn source_export_port_checks_only_its_tos_routes() {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tos_ops_mechanics_plan::{
        agent_surface_exports::source_export_issues, route_cards::RouteSources,
    };
    let root = std::env::temp_dir().join(format!(
        "tos-agent-export-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let mut port = manifest()["owner_ports"]["kag_provider"].clone();
    for field in [
        "source_contract",
        "owner_route",
        "builder",
        "validation_route",
    ] {
        let path = port["generated_family"][field].as_str().unwrap();
        let dest = root.join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, b"owned route\n").unwrap();
    }
    let check =
        |port: &Value| source_export_issues(&mut RouteSources::new(&root).unwrap(), port).unwrap();
    assert_eq!(check(&port), vec![]);
    for external_owner in ["aoa-kag", "Tree-of-Sophia and aoa-kag"] {
        let mut changed = port.clone();
        changed["canonical_owner"] = json!(external_owner);
        assert_eq!(
            check(&changed),
            vec![(
                "kag_provider".into(),
                "ToS source export must remain owned by Tree-of-Sophia".into()
            )]
        );
    }
    let path = port["generated_family"]["builder"]
        .as_str()
        .unwrap()
        .to_owned();
    fs::remove_file(root.join(&path)).unwrap();
    assert_eq!(
        check(&port),
        vec![(path, "source export builder route is missing".into())]
    );
    port["generated_family"]["scope"] = json!("external_integration_release");
    assert!(!check(&port).is_empty());
}

#[test]
fn standalone_export_routes_do_not_require_a_consumer_checkout_or_import() {
    let source = manifest();
    let port = &source["owner_ports"]["kag_provider"];
    assert_eq!(port["canonical_owner"], "Tree-of-Sophia");
    assert_eq!(port["manifest"], "kag/README.md");
    let probe = source["task_probes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|probe| probe["id"] == "kag_currentness")
        .unwrap();
    for step in probe["required_chain"].as_array().unwrap() {
        let step = step.as_str().unwrap();
        assert!(!step.contains("aoa-kag"));
        assert!(!step.contains("consumer-owned import"));
    }
    let workflow = include_str!("../../../../.github/workflows/repo-validation.yml");
    assert!(!workflow.contains("aoa-kag"));
    assert!(!workflow.contains("provider_pin.json"));
}

#[test]
fn activation_and_description_parsing_keep_mapping_and_single_line_boundaries() {
    use tos_ops_mechanics_plan::agent_surface::{
        boolean_scalar, frontmatter_scalars, policy_scalars, quoted_scalar_is_open,
    };
    assert!(
        boolean_scalar(
            Some("definitely-not-a-boolean"),
            "allow_implicit_invocation",
            "agents/openai.yaml"
        )
        .unwrap_err()
        .to_string()
        .contains("boolean literal")
    );
    for input in [
        "implicit_activation_policy: manual\npolicy:\n  implicit_activation_policy: invoke\n  allow_implicit_invocation: true\n",
        "policy:\n  implicit_activation_policy: invoke\n  allow_implicit_invocation: true\ndisplay:\n  implicit_activation_policy: manual\n  allow_implicit_invocation: false\n",
    ] {
        assert!(
            policy_scalars(input, "agents/openai.yaml")
                .unwrap_err()
                .to_string()
                .contains("immediate child of policy")
        );
    }
    assert!(frontmatter_scalars("metadata:\n  aoa_invocation_mode: explicit-only\nname: example\naoa_invocation_mode: explicit-preferred\n","SKILL.md").unwrap_err().to_string().contains("immediate child of metadata"));
    assert!(!quoted_scalar_is_open("'Nietzsche''s description'"));
    assert!(quoted_scalar_is_open("'Nietzsche''s description"));
    for input in [
        "name: example\ndescription: >\n  This description is folded by YAML.\n",
        "description: 'The line ends with an escaped apostrophe ''\n  and continues physically.\n",
        "name: example\ndescription: \"first line\n  continuation line\n",
    ] {
        assert!(
            frontmatter_scalars(input, "SKILL.md")
                .unwrap_err()
                .to_string()
                .contains("multiline description")
        );
    }
}
