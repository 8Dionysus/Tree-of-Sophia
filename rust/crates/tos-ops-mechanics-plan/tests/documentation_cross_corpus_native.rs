use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tos_ops_mechanics_plan::documentation_cross_corpus as guards;
fn map() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/validation/documentation_family_map.json"
    ))
    .unwrap()
}
struct Fixture {
    root: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    fn write(&self, path: &str, contents: &[u8]) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fn new() -> Self {
        let fixture = Self {
            root: std::env::temp_dir().join(format!(
                "tos-docs-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )),
        };
        fs::create_dir_all(&fixture.root).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .arg(&fixture.root)
                .status()
                .unwrap()
                .success()
        );
        fixture.write("scripts/tiny_entry_route.source.json", include_bytes!(
            "../../../../scripts/tiny_entry_route.source.json"
        ));
        let source = map();
        for family in source["families"].as_array().unwrap() {
            for path in family["currentness_inputs"].as_array().unwrap() {
                let path = path.as_str().unwrap();
                if path.starts_with("external:") {
                    continue;
                }
                if path.ends_with('/') {
                    fs::create_dir_all(fixture.root.join(path)).unwrap();
                } else {
                    fixture.write(path, b"{}\n");
                }
            }
        }
        for path in source["public_authored_surfaces"].as_array().unwrap() {
            fixture.write(path.as_str().unwrap(), b"bounded authored entry\n");
        }
        for key in ["schema_ref", "currentness_schema_ref"] {
            fixture.write(source[key].as_str().unwrap(), b"{}\n");
        }
        fixture.write(
            "docs/validation/documentation_family_map.json",
            &serde_json::to_vec_pretty(&source).unwrap(),
        );
        let lanes = source["families"]
            .as_array()
            .unwrap()
            .iter()
            .map(|family| {
                (
                    family["validation_lane"].as_str().unwrap().to_string(),
                    json!([]),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        fixture.write(
            "docs/validation/validation_lanes.json",
            &serde_json::to_vec(&json!({"lanes":lanes})).unwrap(),
        );
        fixture.write(
            ".agents/agents-route.current.json",
            b"{\"task_routes\":[{\"inherited_context_tokens\":1}]}\n",
        );
        fixture.write("docs/validation/agents_route_inventory.json",b"{\"route_card_discovery\":{\"root_cards\":[\"AGENTS.md\"],\"route_roots\":[\"docs\",\"mechanics\"]}}\n");
        fixture.write(
            "docs/validation/script_inventory.json",
            b"{\"script_surfaces\":[]}\n",
        );
        fixture.write("scripts/build_documentation_family_currentness.py",include_bytes!("../../../../tests/fixtures/documentation_cross_corpus_oracle/build_documentation_family_currentness.py"));
        fixture.write("scripts/validate_documentation_cross_corpus.py",include_bytes!("../../../../tests/fixtures/documentation_cross_corpus_oracle/validate_documentation_cross_corpus.py"));
        fixture.write("scripts/validate_mechanics_topology.py",include_bytes!("../../../../tests/fixtures/documentation_cross_corpus_oracle/validate_mechanics_topology.py"));
        fixture.write("scripts/validate_nested_agents.py",include_bytes!("../../../../tests/fixtures/documentation_cross_corpus_oracle/validate_nested_agents.py"));
        fixture.track();
        fixture.build();
        fixture
    }
    fn track(&self) {
        assert!(
            Command::new("git")
                .current_dir(&self.root)
                .args(["add", "--all"])
                .status()
                .unwrap()
                .success()
        );
    }
    fn build(&self) {
        let out = Command::new("/usr/bin/python3")
            .arg("-B")
            .arg(
                self.root
                    .join("scripts/build_documentation_family_currentness.py"),
            )
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fn native(&self) -> Vec<(String, String)> {
        let executable = std::env::var_os("TOS_DOCUMENTATION_TEST_EXECUTABLE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
        let out = Command::new(executable)
            .arg("--repo-root")
            .arg(&self.root)
            .arg("--python")
            .arg("/usr/bin/python3")
            .arg("--documentation-cross-corpus-validate")
            .output()
            .unwrap();
        assert!(
            matches!(out.status.code(), Some(0 | 1)),
            "native operation error: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).unwrap();
        let mut lines = stdout.lines();
        let header = lines.next().expect("native CLI must publish its result");
        if out.status.success() {
            assert_eq!(
                header,
                "[ok] validated cross-corpus documentation currentness and context guards"
            );
            assert!(lines.next().is_none(), "unexpected success output");
            return Vec::new();
        }
        assert_eq!(header, "Cross-corpus documentation validation failed.");
        lines
            .map(|line| {
                let diagnostic = line
                    .strip_prefix("- ")
                    .expect("malformed native diagnostic");
                let (location, message) = diagnostic
                    .split_once(": ")
                    .expect("native diagnostic lacks location");
                (location.to_string(), message.to_string())
            })
            .filter(|(location, message)| {
                // The oracle requests reuse_existing=False. Select that same unique
                // guard surface from the real CLI; owner composition has its own tests.
                ![
                    "AGENTS-route validator: ",
                    "mechanics topology validator: ",
                    "decision validator: ",
                    "public-entry validator: ",
                ]
                .iter()
                .any(|prefix| message.starts_with(prefix))
                    && !(location == "scripts/validate_agent_surface.py"
                        && message.starts_with("agent-surface validator: "))
                    && !(location == "scripts/validate_root_entry_map.py"
                        && message.starts_with("owner validator: "))
            })
            .collect()
    }
    fn oracle(&self) -> Vec<(String, String)> {
        let out=Command::new("/usr/bin/python3").arg("-B").arg("-c").arg("import sys,types,json,pathlib; root=pathlib.Path(sys.argv[1]);sys.path.insert(0,str(root/'scripts'));sys.modules['validate_decision_records']=types.ModuleType('validate_decision_records');sys.modules['validate_tiny_entry_route']=types.ModuleType('validate_tiny_entry_route');import validate_documentation_cross_corpus as validator;print(json.dumps(validator.run_validation(root,reuse_existing=False),ensure_ascii=False))").arg(&self.root).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn compare(&self) -> Vec<(String, String)> {
        let native = self.native();
        let mut oracle = self.oracle();
        assert_eq!(native.len(), oracle.len());
        for (actual, expected) in native.iter().zip(&mut oracle) {
            // Both readers reject the same missing public-entry operand. Their
            // OS error prose differs; preserve the exact location and file path.
            let operand = "ToS/derived-exports/root_entry_map.min.json";
            if actual.0 == "context_probes#public_entry"
                && expected.0 == actual.0
                && actual.1 == format!("cannot measure probe: missing operand file: {operand}")
                && expected.1
                    == format!(
                        "cannot measure probe: [Errno 2] No such file or directory: '{}'",
                        self.root.join(operand).display()
                    )
            {
                expected.1.clone_from(&actual.1);
            }
        }
        assert_eq!(native, oracle);
        native
    }
}
#[test]
fn source_authority_configuration_and_family_boundaries_remain_explicit() {
    let source = map();
    let mut issues = Vec::new();
    guards::validate_authority_declarations(&json!([{"id":"x","owner":"README.md","strength":"authored"},{"id":"x","owner":"README.md","strength":"authored"},{"id":"x","owner":"ToS/","strength":"authored"}]),&mut issues,"authority_declarations").unwrap();
    assert_eq!(
        issues.iter().map(|v| v.1.as_str()).collect::<Vec<_>>(),
        vec![
            "duplicate authority declaration: x",
            "conflicting authority declaration: x"
        ]
    );
    issues.clear();
    let mut families = source["families"].clone();
    families[0]["owner"] = json!("wrong-owner");
    guards::validate_family_authority_claims(
        &families,
        &source["authority_declarations"],
        &mut issues,
    )
    .unwrap();
    assert!(
        issues
            .iter()
            .any(|v| v.1.starts_with("family owner conflicts"))
    );
    issues.clear();
    families[0]["authority_keys"] = json!(["unknown"]);
    guards::validate_family_authority_claims(
        &families,
        &source["authority_declarations"],
        &mut issues,
    )
    .unwrap();
    assert!(
        issues
            .iter()
            .any(|v| v.1 == "authority key has no declaration: unknown")
    );
    issues.clear();
    families = source["families"].clone();
    families[2]["match"]["exclude_prefixes"] = json!([]);
    guards::validate_family_match_rules(&families, &mut issues).unwrap();
    assert!(
        issues
            .iter()
            .any(|v| v.1.contains("overlapping family prefixes require"))
    );
    issues.clear();
    let mut rules = source["surface_rules"].clone();
    rules["exclude_paths"] = json!(["../private.md"]);
    guards::validate_surface_rules(&rules, &mut issues).unwrap();
    assert!(issues.iter().any(|v| v.1.contains("unsafe path")));
    assert!(issues.iter().any(|v| v.1.contains("must stay within")));
    issues.clear();
    guards::validate_context_probe_configuration(&json!([]), &mut issues).unwrap();
    assert!(
        issues
            .iter()
            .any(|v| v.1 == "context_probes must be a non-empty list")
    );
}
#[test]
fn markdown_routes_fragments_reference_definitions_and_code_visibility_match_owner_oracle() {
    let fixture = Fixture::new();
    assert_eq!(fixture.compare(), vec![
        ("docs/validation/documentation_family_map.json#kag".into(), "family owner conflicts with declaration kag_provider_route".into()),
        ("docs/validation/documentation_family_map.json#kag".into(), "family owner conflicts with declaration kag_source_return".into()),
        ("context_probes#public_entry".into(), "cannot measure probe: missing operand file: ToS/derived-exports/root_entry_map.min.json".into()),
    ]);
    fixture.write(
        "docs/target.md",
        b"# Named target\n# Named target\n<a id=\"explicit\"></a>\n",
    );
    fixture.write("docs/routes.md",b"[good](target.md#named-target-1)\n[missing](absent.md)\n[fragment](target.md#absent)\n[unresolved][unknown]\n[resolved][named]\n[named]: target.md#explicit\n[^note]: textual footnote prose\n`[ignored](inline-missing.md)`\n    [ignored](indented-missing.md)\n```\n[ignored](fenced-missing.md)\n```\n");
    fixture.track();
    fixture.build();
    let findings = fixture.compare();
    assert!(
        findings
            .iter()
            .any(|v| v.1 == "broken local documentation route: absent.md")
    );
    assert!(
        findings
            .iter()
            .any(|v| v.1 == "broken local documentation fragment: target.md#absent")
    );
    assert!(
        findings
            .iter()
            .any(|v| v.1 == "unresolved reference-style documentation route: unknown")
    );
    assert!(
        !findings
            .iter()
            .any(|v| v.1.contains("ignored") || v.1.contains("missing.md"))
    );
}
#[test]
fn executable_markers_are_command_local_and_do_not_exempt_neighbor_commands() {
    let fixture = Fixture::new();
    fixture.write("mechanics/demo/parts/probe/scripts/run.py", b"pass\n");
    fixture.write(
        "docs/validation/script_inventory.json",
        b"{\"script_surfaces\":[{\"path\":\"mechanics/demo/parts/probe/scripts/run.py\"}]}\n",
    );
    fixture.write("docs/routes.md",b"mechanics/demo/parts/probe/scripts/run.py\nexternal owner aoa-kag scripts/external.py; scripts/local.py\nexternal owner aoa-kag scripts/external.py, scripts/comma.py\n| aoa-kag scripts/external.py | scripts/cell.py |\nexternal owner aoa-kag scripts/external.py && scripts/joint.py\n");
    fixture.track();
    fixture.build();
    let findings = fixture.compare();
    for expected in [
        "scripts/local.py",
        "scripts/comma.py",
        "scripts/cell.py",
        "scripts/joint.py",
    ] {
        assert!(
            findings
                .iter()
                .any(|v| v.1 == format!("stale executable reference: {expected}"))
        );
    }
    assert!(!findings.iter().any(|v| v.1.contains("scripts/external.py")));
}
#[test]
fn public_context_and_projection_mutations_are_checked_on_each_invocation() {
    let fixture = Fixture::new();
    fixture.write("README.md", b"/home/private OPENAI_API_KEY session_id\n");
    fixture.track();
    fixture.build();
    let findings = fixture.compare();
    for marker in ["/home/", "OPENAI_API_KEY", "session_id"] {
        assert!(
            findings
                .iter()
                .any(|v| v.1 == format!("contains forbidden public-safety marker '{marker}'"))
        );
    }
    fixture.write(
        ".agents/agents-route.current.json",
        b"{\"task_routes\":[{\"inherited_context_tokens\":99999}]}\n",
    );
    fixture.build();
    assert!(
        fixture
            .compare()
            .iter()
            .any(|v| v.1 == "context budget exceeded: 99999>2800")
    );
    fixture.write("docs/validation/documentation-family.current.json", b"{}\n");
    assert!(
        fixture
            .compare()
            .iter()
            .any(|v| v.1 == "generated documentation family projection is stale")
    );
}
#[test]
fn actual_builder_cli_matches_bytes_across_human_glob_contracts_and_binary_carriers() {
    let fixture = Fixture::new();
    fixture.write("docs/binary.json", &[0xff, 0, 1]);
    fixture.write(
        ".agents/skills/example/agents/openai.yaml",
        b"policy:\n  bounded: true\n",
    );
    fixture.write("docs/alpha.txt", b"one\ntwo\n");
    fixture.write("docs/zeta.txt", b"three\n");
    fixture.track();
    let executable = std::env::var_os("TOS_DOCUMENTATION_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    for pattern in [
        ".agents/skills/**/agents/openai.yaml",
        "docs/[a-m]*.txt",
        "docs/[!a-m]*.txt",
        "docs/[z-a]*.txt",
    ] {
        let mut source = map();
        source["atlas_method"]["human_exclusion"] = json!(pattern);
        fixture.write(
            "docs/validation/documentation_family_map.json",
            &serde_json::to_vec_pretty(&source).unwrap(),
        );
        fixture.build();
        let expected = fs::read(
            fixture
                .root
                .join("docs/validation/documentation-family.current.json"),
        )
        .unwrap();
        let result = Command::new(&executable)
            .arg("--repo-root")
            .arg(&fixture.root)
            .arg("--documentation-family-build")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read(
                fixture
                    .root
                    .join("docs/validation/documentation-family.current.json")
            )
            .unwrap(),
            expected,
            "pattern={pattern}"
        );
        let checked = Command::new(&executable)
            .arg("--repo-root")
            .arg(&fixture.root)
            .args(["--documentation-family-build", "--check"])
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
    }
    let expected = fs::read(
        fixture
            .root
            .join("docs/validation/documentation-family.current.json"),
    )
    .unwrap();
    let output = Command::new(&executable)
        .arg("--repo-root")
        .arg(&fixture.root)
        .args([
            "--documentation-family-build",
            "--output",
            "docs/../documentation-alternate.json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(fixture.root.join("documentation-alternate.json")).unwrap(),
        expected
    );
}
#[test]
fn actual_coordinator_cli_does_not_invent_an_ambient_kag_export_operation() {
    let fixture = Fixture::new();
    fixture.write("scripts/validate_local_kag_provider.py",b"from pathlib import Path\nPath('ambient-export-invoked').write_text('invalid ambient operation')\nraise SystemExit(77)\n");
    fixture.track();
    fixture.build();
    let executable = std::env::var_os("TOS_DOCUMENTATION_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let out = Command::new(&executable)
        .arg("--repo-root")
        .arg(&fixture.root)
        .arg("--python")
        .arg("/usr/bin/python3")
        .arg("--documentation-cross-corpus-validate")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .starts_with("Cross-corpus documentation validation failed.")
    );
    assert!(!fixture.root.join("ambient-export-invoked").exists());
    assert!(
        !String::from_utf8_lossy(&out.stdout)
            .contains("validate_local_kag_provider.py: owner validator")
    );
}

#[test]
fn priced_card_discovery_streams_payload_names_but_refuses_untracked_cards() {
    use std::{sync::atomic::AtomicI32, time::{Duration, Instant}};
    use tos_ops_mechanics_plan::route_cards::{self, RouteSources};
    let fixture = Fixture::new();
    let mut inventory: Value = serde_json::from_str(include_str!(
        "../../../../docs/validation/agents_route_inventory.json"
    )).unwrap();
    inventory["route_card_discovery"]["route_roots"] = json!(["docs", "mechanics"]);
    fixture.write(route_cards::INVENTORY, &serde_json::to_vec(&inventory).unwrap());
    fixture.track();
    let sources = || RouteSources::new_until_with_operation_limit(
        &fixture.root, Instant::now() + Duration::from_secs(30),
        route_cards::MAX_BUDGETED_ROUTE_OPERATIONS,
    ).unwrap();
    let before = sources().discover(&inventory).unwrap();
    for index in 0..10_001 {
        fixture.write(&format!("docs/ignored-payload/{index}"), b"");
    }
    assert!(sources().discover(&inventory).is_err());
    assert_eq!(sources().discover_cards_with_limits(
        &inventory, route_cards::MAX_SELECTED_PATH_DISCOVERY_ENTRIES,
    ).unwrap(), before);
    let issues = route_cards::run_validation_with_card_discovery_limit(
        &fixture.root, &mut sources(), &AtomicI32::new(0),
        route_cards::MAX_SELECTED_PATH_DISCOVERY_ENTRIES,
    ).unwrap();
    assert!(!issues.iter().any(|(_, message)|
        message.contains("route discovery entry bound exceeded")));
    let generated = route_cards::build_currentness(&fixture.root, &AtomicI32::new(0)).unwrap();
    let generated_cards: Vec<_> = generated["cards"].as_array().unwrap().iter()
        .map(|card| card["path"].as_str().unwrap().to_owned()).collect();
    assert_eq!(generated_cards, before);
    fixture.write("docs/ignored-payload/AGENTS.md", b"untracked card\n");
    let issues = route_cards::run_validation_with_card_discovery_limit(
        &fixture.root, &mut sources(), &AtomicI32::new(0),
        route_cards::MAX_SELECTED_PATH_DISCOVERY_ENTRIES,
    ).unwrap();
    assert!(issues.iter().any(|(path, message)|
        path == "docs/ignored-payload/AGENTS.md"
            && message == "discovered AGENTS.md is not tracked"));
}
#[test]
fn tracked_executable_route_card_cannot_disappear() {
    use tos_ops_mechanics_plan::route_cards::RouteSources;
    let fixture = Fixture::new();
    let mut sources = RouteSources::new(&fixture.root).unwrap();
    let error = guards::validate_executable_routes(
        &fixture.root, &mut sources, &["docs/AGENTS.md".into()], &mut Vec::new(),
    ).unwrap_err();
    assert!(error.to_string().contains("tracked route card is missing: docs/AGENTS.md"));
}
