//! Native agent surface boundaries: profile authority, crosswalk and activation.
use serde_json::{Value, json};
use tos_ops_mechanics_plan::agent_surface_validation::{
    activation_policy_issues, legacy_projection_crosswalk_issues, profile_binding_issues,
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
fn legacy_crosswalk_requires_all_source_owned_names_and_destination_kinds() {
    let mut source = manifest();
    assert!(legacy_projection_crosswalk_issues(&source).is_empty());
    source["legacy_projection_migration"]["entries"][0]["target_kind"] = json!("invented");
    assert!(
        legacy_projection_crosswalk_issues(&source)
            .iter()
            .any(|(_, m)| m == "target_kind is not a recognized destination kind")
    );
    source["legacy_projection_migration"]["entries"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        legacy_projection_crosswalk_issues(&source)
            .iter()
            .any(|(_, m)| m.starts_with("crosswalk names must be"))
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
    write(
        "scripts/build_agent_surface_currentness.py",
        include_bytes!(
            "../../../../tests/fixtures/agent_surface_oracle/build_agent_surface_currentness.py"
        ),
    );
    write(
        "scripts/validate_agent_surface.py",
        include_bytes!("../../../../tests/fixtures/agent_surface_oracle/validate_agent_surface.py"),
    );
    write(".agents/AGENTS.md", b"# Local owner\n");
    write(".agents/README.md", b"# Public entry\n");
    let mut source = manifest();
    // The frozen oracle vector exercises the legacy selected-family contract.
    source["owner_ports"]["kag_provider"]["generated_family"]["scope"] =
        json!("selected_local_family");
    for port in source["owner_ports"].as_object().unwrap().values() {
        for route in port["currentness_inputs"].as_array().unwrap() {
            write(route.as_str().unwrap(), b"bounded synthetic owned input\n");
        }
    }
    for key in ["segments", "receipt_root"] {
        fs::create_dir_all(
            root.join(
                source["owner_ports"]["kag_provider"]["generated_family"][key]
                    .as_str()
                    .unwrap(),
            ),
        )
        .unwrap();
    }
    let write_manifest = |m: &Value| {
        write(
            ".agents/agent-surface.manifest.json",
            &serde_json::to_vec_pretty(m).unwrap(),
        )
    };
    let build = || {
        let result = Command::new("/usr/bin/python3")
            .arg("-B")
            .arg(root.join("scripts/build_agent_surface_currentness.py"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    let executable = std::env::var_os("TOS_AGENT_SURFACE_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let compare = || {
        let native = Command::new(&executable)
            .arg("--repo-root")
            .arg(&root)
            .arg("--python")
            .arg("/usr/bin/python3")
            .arg("--agent-surface-validate")
            .output()
            .unwrap();
        let oracle = Command::new("/usr/bin/python3")
            .arg("-B")
            .arg(root.join("scripts/validate_agent_surface.py"))
            .arg("--check")
            .output()
            .unwrap();
        assert_eq!(
            native.status.code(),
            oracle.status.code(),
            "native {}\noracle {}",
            String::from_utf8_lossy(&native.stderr),
            String::from_utf8_lossy(&oracle.stderr)
        );
        assert_eq!(native.stdout, oracle.stdout);
        assert_eq!(
            String::from_utf8_lossy(&native.stderr),
            String::from_utf8_lossy(&oracle.stderr)
        );
        String::from_utf8(native.stderr).unwrap()
    };
    write_manifest(&source);
    build();
    let findings = compare();
    assert!(findings.contains("generated KAG family manifest is missing"));
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
fn budget_scope_truth_table_retains_relation_base_and_requested_scope_as_independent_inputs() {
    use std::sync::atomic::AtomicI32;
    use tos_ops_mechanics_plan::agent_surface_budget::{
        budget_exceedance_relation, budget_receipt_contract_issues, canonical_budget_scope,
    };
    use tos_ops_mechanics_plan::route_cards::RouteSources;
    let root =
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
            .unwrap();
    let cases = [
        (10, 10, 20, 20, None),
        (11, 10, 20, 20, Some("generated_delta")),
        (10, 10, 21, 20, Some("tracked_size")),
        (11, 10, 21, 20, Some("generated_delta_and_tracked_size")),
    ];
    for (changed, limit, tracked, tracked_limit, relation) in cases {
        assert_eq!(
            budget_exceedance_relation(
                &json!(changed),
                &json!(limit),
                &json!(tracked),
                &json!(tracked_limit)
            )
            .unwrap(),
            relation
        );
        for base_v3 in [false, true] {
            for requested in [
                "v2_to_v3_migration",
                "generated_delta",
                "tracked_size",
                "generated_delta_and_tracked_size",
            ] {
                let digest = "a".repeat(64);
                let m = json!({"schema_version":"aoa-repo-local-kag-family-manifest-v3","repo":{"name":"Tree-of-Sophia","git_ref":"git-index-source-tree"},"budgets":{"changed_generated_bytes_max":limit,"tracked_bytes_max":tracked_limit},"summary":{"tracked_bytes":tracked},"family_identity":{"content_digest":digest}});
                let receipt = json!({"schema_version":"aoa-repo-local-kag-budget-receipt-v1","repo":"Tree-of-Sophia","scope":requested,"base_ref":"b".repeat(40),"head_family_digest":digest,"changed_generated_bytes":changed,"changed_generated_files":1,"default_limit_bytes":limit,"allowed_bytes":changed,"tracked_bytes":tracked,"tracked_bytes_max":tracked_limit,"allowed_tracked_bytes":tracked,"reason":"focused budget scope control","approved_by":"test-owner","decision_ref":"aoa-kag:docs/decisions/AOA-KAG-D-0017-portable-content-addressed-repository-family.md"});
                let issues = budget_receipt_contract_issues(
                    &root,
                    &mut RouteSources::new(&root).unwrap(),
                    &m,
                    &receipt,
                    &digest,
                    "focused-budget-receipt.json",
                    Some(base_v3),
                    false,
                    false,
                    &AtomicI32::new(0),
                    None,
                )
                .unwrap();
                let accepted = relation.is_some()
                    && Some(requested) == canonical_budget_scope(relation, base_v3);
                assert_eq!(
                    issues.is_empty(),
                    accepted,
                    "base_v3={base_v3} relation={relation:?} requested={requested} issues={issues:?}"
                );
            }
        }
    }
    assert!(
        budget_exceedance_relation(&json!(true), &json!(10), &json!(20), &json!(20))
            .unwrap_err()
            .to_string()
            .contains("must be integers")
    );
    assert!(
        budget_exceedance_relation(&json!(-1), &json!(10), &json!(20), &json!(20))
            .unwrap_err()
            .to_string()
            .contains("must not be negative")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn actual_builder_byte_parity_covers_nonempty_package_and_git_selection() {
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
    write(
        "scripts/build_agent_surface_currentness.py",
        include_bytes!(
            "../../../../tests/fixtures/agent_surface_oracle/build_agent_surface_currentness.py"
        ),
    );
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
        let oracle = Command::new("/usr/bin/python3")
            .arg("-B")
            .arg(root.join("scripts/build_agent_surface_currentness.py"))
            .output()
            .unwrap();
        assert!(
            oracle.status.success(),
            "{}",
            String::from_utf8_lossy(&oracle.stderr)
        );
        let expected = fs::read(root.join(".agents/agent-surface.current.json")).unwrap();
        let native = Command::new(&executable)
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
        assert_eq!(native.stdout, oracle.stdout);
        assert_eq!(native.stderr, oracle.stderr);
        assert_eq!(
            fs::read(root.join(".agents/agent-surface.current.json")).unwrap(),
            expected
        );
        let checked = Command::new(&executable)
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
    assert!(
        untracked["packages"][0]["companion_counts"]["assets"]
            .as_u64()
            .unwrap()
            > 0
    );
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
fn external_kag_scope_requires_authored_routes_without_claiming_unselected_artifacts() {
    use std::{
        fs,
        sync::atomic::AtomicI32,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tos_ops_mechanics_plan::{
        agent_surface_budget::generated_family_issues, route_cards::RouteSources,
    };
    let root = std::env::temp_dir().join(format!(
        "tos-agent-external-{}-{}",
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
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for field in [
        "provider_template",
        "owner_route",
        "publication_route",
        "validation_route",
    ] {
        let path = port["generated_family"][field].as_str().unwrap();
        let destination = root.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, fs::read(source_root.join(path)).unwrap()).unwrap();
    }
    let cancel = AtomicI32::new(0);
    let check = |port: &Value| {
        generated_family_issues(
            &root,
            &mut RouteSources::new(&root).unwrap(),
            port,
            false,
            None,
            &cancel,
        )
        .unwrap()
    };
    assert!(
        !root
            .join(port["generated_family"]["manifest"].as_str().unwrap())
            .exists()
    );
    assert_eq!(check(&port), vec![]);
    let template = port["generated_family"]["provider_template"]
        .as_str()
        .unwrap()
        .to_owned();
    let original = fs::read(root.join(&template)).unwrap();
    fs::write(root.join(&template), b"{").unwrap();
    assert_eq!(check(&port), vec![(template.clone(), "external KAG provider template does not bind its schema, owner, record classes and publication route".into())]);
    fs::write(root.join(&template), original).unwrap();
    let owner = port["generated_family"]["owner_route"]
        .as_str()
        .unwrap()
        .to_owned();
    fs::remove_file(root.join(&owner)).unwrap();
    assert_eq!(
        check(&port),
        vec![(
            owner.clone(),
            "external KAG owner_route route is missing".into()
        )]
    );
    fs::write(root.join(&owner), b"# Owner\n").unwrap();
    fs::remove_file(root.join(&template)).unwrap();
    assert_eq!(
        check(&port),
        vec![(
            template,
            "external KAG provider_template route is missing".into()
        )]
    );
    port["generated_family"]["scope"] = json!("selected_local_family");
    let family = &port["generated_family"];
    let local = root.join(family["manifest"].as_str().unwrap());
    fs::create_dir_all(local.parent().unwrap()).unwrap();
    fs::write(
        local,
        br#"{"family_identity":{"content_digest":"corrupt"}}"#,
    )
    .unwrap();
    for field in ["segments", "receipt_root"] {
        fs::create_dir_all(root.join(family[field].as_str().unwrap())).unwrap();
    }
    let expected = vec![(
        family["manifest"].as_str().unwrap().into(),
        "generated KAG family digest must be 64 lowercase hex characters".into(),
    )];
    assert_eq!(check(&port), expected);
    port["generated_family"]
        .as_object_mut()
        .unwrap()
        .remove("scope");
    assert_eq!(check(&port), expected);
}
