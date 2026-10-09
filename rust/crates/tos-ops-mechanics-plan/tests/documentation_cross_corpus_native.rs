use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tos_ops_mechanics_plan::{documentation_cross_corpus as guards, documentation_family as family, route_cards::RouteSources};
use std::sync::atomic::AtomicI32;
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
        fixture.write(
            "scripts/tiny_entry_route.source.json",
            include_bytes!("../../../../scripts/tiny_entry_route.source.json"),
        );
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
        for key in ["builder", "validator"] {
            fixture.write(source[key].as_str().unwrap(), b"// Native owner fixture\n");
        }
        fixture.write("ToS/derived-exports/root_entry_map.min.json", b"{\"context_summary\":{}}\n");
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
        let out = self.command().arg("--documentation-family-build").output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fn command(&self) -> Command {
        let executable = std::env::var_os("TOS_DOCUMENTATION_TEST_EXECUTABLE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
        let mut command = Command::new(executable);
        // Git is the declared tracked-file platform bridge. No Python fallback.
        command.env_clear().env("PATH", "/usr/bin")
            .arg("--repo-root").arg(&self.root)
            .arg("--python").arg("/no-python-executable");
        command
    }
    fn native(&self) -> Vec<(String, String)> {
        let out = self.command().arg("--documentation-cross-corpus-validate")
            .output().unwrap();
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
                // Select this guard surface from the real coordinating CLI.
                // Composed owner validators have their own bounded tests.
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
                    && !(location == "rust/crates/tos-ops-mechanics-plan/src/root_entry_map.rs"
                        && message.starts_with("owner validator: "))
            })
            .collect()
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
fn markdown_routes_fragments_reference_definitions_and_code_visibility_remain_checked() {
    let fixture = Fixture::new();
    assert_eq!(fixture.native(), vec![]);
    fixture.write(
        "docs/target.md",
        b"# Named target\n# Named target\n<a id=\"explicit\"></a>\n",
    );
    fixture.write("docs/routes.md",b"[good](target.md#named-target-1)\n[missing](absent.md)\n[fragment](target.md#absent)\n[unresolved][unknown]\n[resolved][named]\n[named]: target.md#explicit\n[^note]: textual footnote prose\n`[ignored](inline-missing.md)`\n    [ignored](indented-missing.md)\n```\n[ignored](fenced-missing.md)\n```\n");
    fixture.track();
    fixture.build();
    let findings = fixture.native();
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
    let findings = fixture.native();
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
    let findings = fixture.native();
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
            .native()
            .iter()
            .any(|v| v.1 == "context budget exceeded: 99999>2800")
    );
    fixture.write("docs/validation/documentation-family.current.json", b"{}\n");
    assert!(
        fixture
            .native()
            .iter()
            .any(|v| v.1 == "generated documentation family projection is stale")
    );
}
#[test]
fn actual_builder_cli_preserves_glob_counts_binary_fixity_order_and_output_contracts() {
    let fixture = Fixture::new();
    fixture.write("docs/binary.json", &[0xff, 0, 1]);
    fixture.write(
        ".agents/skills/example/agents/openai.yaml",
        b"policy:\n  bounded: true\n",
    );
    fixture.write("docs/alpha.txt", b"one\ntwo\n");
    fixture.write("docs/zeta.txt", b"three\n");
    fixture.track();
    for (pattern, excluded) in [
        (".agents/skills/**/agents/openai.yaml", 1),
        ("docs/[a-m]*.txt", 1),
        ("docs/[!a-m]*.txt", 1),
        ("docs/[z-a]*.txt", 0),
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
        let current: Value = serde_json::from_slice(&expected).unwrap();
        assert_eq!(current["coverage"]["human_scope"]["excluded_skill_launch_metadata"], excluded);
        assert_eq!(current["tracked_source"], "git ls-files -z");
        let records = current["tracked_surfaces"].as_array().unwrap();
        let binary = records.iter().find(|r| r["path"] == "docs/binary.json").unwrap();
        assert_eq!(binary["bytes"], 3);
        assert_eq!(binary["sha256"], "942e1e2a66a427b6551732f758bc314f22b9cdec9365a3425c9184de299392b5");
        assert_eq!(binary["surface_kind"], "structured");
        assert!(records.windows(2).all(|p| p[0]["path"].as_str() < p[1]["path"].as_str()));
        assert_eq!(current["coverage"]["unhandled_family_count"], 0);
        assert_eq!(current["family_summaries"].as_array().unwrap().len(), source["families"].as_array().unwrap().len());
        let result = fixture.command()
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
        let checked = fixture.command()
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
    let output = fixture.command()
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
    let out = fixture.command()
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
    use std::{
        time::{Duration, Instant},
    };
    use tos_ops_mechanics_plan::route_cards::{self, RouteSources};
    let fixture = Fixture::new();
    let mut inventory: Value = serde_json::from_str(include_str!(
        "../../../../docs/validation/agents_route_inventory.json"
    ))
    .unwrap();
    inventory["route_card_discovery"]["route_roots"] = json!(["docs", "mechanics"]);
    fixture.write(
        route_cards::INVENTORY,
        &serde_json::to_vec(&inventory).unwrap(),
    );
    fixture.track();
    let sources = || {
        RouteSources::new_until_with_operation_limit(
            &fixture.root,
            Instant::now() + Duration::from_secs(30),
            route_cards::MAX_BUDGETED_ROUTE_OPERATIONS,
        )
        .unwrap()
    };
    let before = sources().discover(&inventory).unwrap();
    for index in 0..10_001 {
        fixture.write(&format!("docs/ignored-payload/{index}"), b"");
    }
    assert!(sources().discover(&inventory).is_err());
    assert_eq!(
        sources()
            .discover_cards_with_limits(
                &inventory,
                route_cards::MAX_SELECTED_PATH_DISCOVERY_ENTRIES,
            )
            .unwrap(),
        before
    );
    // Git-backed coordination needs its own process, as the maintained CLI has.
    // The Rust test harness has a worker thread even with --test-threads=1.
    let out = fixture.command().arg("--nested-agents-validate").output().unwrap();
    let diagnostics = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(matches!(out.status.code(), Some(0 | 1)) && !diagnostics.contains("route discovery entry bound exceeded"), "{diagnostics}");
    let out = fixture.command().arg("--agents-route-currentness-build").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let generated: Value = serde_json::from_slice(&fs::read(fixture.root.join(".agents/agents-route.current.json")).unwrap()).unwrap();
    let generated_cards: Vec<_> = generated["cards"].as_array().unwrap().iter()
        .map(|card| card["path"].as_str().unwrap().to_owned()).collect();
    assert_eq!(generated_cards, before);
    fixture.write("docs/ignored-payload/AGENTS.md", b"untracked card\n");
    let out = fixture.command().arg("--nested-agents-validate").output().unwrap();
    let diagnostics = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(
        !out.status.success() && diagnostics.contains("docs/ignored-payload/AGENTS.md")
            && diagnostics.contains("discovered AGENTS.md is not tracked"), "{diagnostics}"
    );
}
#[test]
fn tracked_executable_route_card_cannot_disappear() {
    use tos_ops_mechanics_plan::route_cards::RouteSources;
    let fixture = Fixture::new();
    fs::remove_file(fixture.root.join("docs/AGENTS.md")).unwrap();
    let mut sources = RouteSources::new(&fixture.root).unwrap();
    let error = guards::validate_executable_routes(
        &fixture.root,
        &mut sources,
        &["docs/AGENTS.md".into()],
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("tracked route card is missing: docs/AGENTS.md")
    );
}

#[test]
fn source_map_binds_lanes_routes_public_requirements_and_executable_inventory() {
    let fixture = Fixture::new();
    let source = map();
    let mut issues = Vec::new();
    guards::validate_source_map(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &source, &mut issues).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    for (pointer, replacement, expected) in [
        ("/families/0/validation_lane", json!("missing-lane"), "not in command authority"),
        ("/generated_currentness", json!("README.md"), "canonical route"),
        ("/public_authored_surfaces", json!([]), "public_authored_surfaces"),
        ("/public_forbidden_markers", json!([]), "public_forbidden_markers"),
        ("/atlas_method/structured_carrier_extensions", json!([".toml"]), "extension groups must exactly cover"),
        ("/atlas_method/tracked_source", json!("find tracked -print0"), "tracked_source must match the builder operation"),
    ] {
        let mut changed = source.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        let mut issues = Vec::new();
        guards::validate_source_map(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &changed, &mut issues).unwrap();
        assert!(issues.iter().any(|v| v.1.contains(expected)), "{pointer}: {issues:?}");
    }
    let mut changed_probes = source["context_probes"].clone();
    changed_probes[0]["max_tokens"] = json!(9999);
    let mut issues = Vec::new();
    guards::validate_context_probe_configuration(&changed_probes, &mut issues).unwrap();
    assert!(issues.iter().any(|v| v.1.contains("canonical context contract")), "{issues:?}");
    for (field, required) in [("public_authored_surfaces", "README.md"), ("public_forbidden_markers", "BEGIN PRIVATE KEY")] {
        let mut changed = source.clone();
        changed[field].as_array_mut().unwrap().retain(|v| v != required);
        let mut issues = Vec::new();
        guards::validate_source_map(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &changed, &mut issues).unwrap();
        assert!(issues.iter().any(|v| v.1.contains("missing required entries")), "{issues:?}");
    }
    let mut invalid = source;
    invalid["atlas_method"]["tracked_source"] = json!("find tracked -print0");
    let err = family::build_currentness_with_tracked(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &invalid, &[], &AtomicI32::new(0)).unwrap_err();
    assert!(err.to_string().contains("tracked_source must match the builder operation"));
}

#[test]
fn authority_and_surface_configuration_cannot_silently_drop_coverage() {
    let source = map();
    let mut harmless = Vec::new();
    guards::validate_authority_declarations(&json!([{"id":"claim","owner":"README.md","strength":"authored"}]), &mut harmless, "authority_declarations").unwrap();
    assert!(harmless.is_empty());
    for missing_all in [true, false] {
        let mut families = source["families"].clone();
        if missing_all { families[0]["authority_keys"] = json!([]); }
        else { families[0]["authority_keys"].as_array_mut().unwrap().retain(|k| k != "repository_identity"); }
        let mut issues = Vec::new();
        guards::validate_family_authority_claims(&families, &source["authority_declarations"], &mut issues).unwrap();
        let expected = if missing_all { "non-empty list" } else { "not referenced by authority_keys" };
        assert!(issues.iter().any(|v| v.1.contains(expected)), "{issues:?}");
    }
    for (field, replacement, expected) in [
        ("include_extensions", json!([]), "include_extensions"),
        ("exclude_prefixes", json!(["kag/indexes"]), "must end with '/'"),
        ("exclude_prefixes", json!(["ToS/"]), "generated_carrier_prefixes"),
        ("generated_carrier_prefixes", json!(["ToS/"]), "canonical generated carriers"),
    ] {
        let mut rules = source["surface_rules"].clone();
        rules[field] = replacement;
        let mut issues = Vec::new();
        guards::validate_surface_rules(&rules, &mut issues).unwrap();
        assert!(issues.iter().any(|v| v.1.contains(expected)), "{field}: {issues:?}");
    }
    let mut families = source["families"].clone();
    families[0]["id"] = json!("unknown-family");
    let mut issues = Vec::new();
    guards::validate_family_match_rules(&families, &mut issues).unwrap();
    assert!(issues.iter().any(|v| v.1.contains("canonical")), "{issues:?}");
    let fixture = Fixture::new();
    fixture.write("unowned-new-family/file.md", b"new tracked source\n");
    fixture.track();
    fixture.build();
    let issues = fixture.native();
    assert!(issues.iter().any(|v| v.1.contains("unhandled family count is 1:") && v.1.contains("unowned-new-family/file.md")), "{issues:?}");
}

#[test]
fn context_measures_use_declared_surfaces_and_preserve_limits() {
    let fixture = Fixture::new();
    fixture.write("one.md", b"one two three\n");
    fixture.write("two.md", b"four five\n");
    fixture.write("summary.json", b"{\"context_summary\":{\"one\":\"two\"}}\n");
    fixture.write("wrong.json", b"{\"not_a_summary\":{}}\n");
    fixture.write("nonobject.json", b"{\"context_summary\":[]}\n");
    fixture.write("first.json", b"{\"task_routes\":[{\"inherited_context_tokens\":2}]}\n");
    fixture.write("second.json", b"{\"task_routes\":[{\"inherited_context_tokens\":5}]}\n");
    fixture.write("empty.json", b"{\"task_routes\":[]}\n");
    for (measure, surfaces, maximum, expected) in [
        ("sum", json!(["one.md", "two.md"]), 4, "context budget exceeded: 5>4"),
        ("max", json!(["one.md", "two.md"]), 2, "context budget exceeded: 3>2"),
        ("typo", json!(["one.md"]), 4, "unsupported context measure"),
        ("sum", json!([]), 4, "surfaces must be a non-empty list"),
        ("generated_summary", json!(["missing.json"]), 20, "cannot measure probe"),
        ("generated_summary", json!(["wrong.json"]), 20, "lacks context_summary"),
        ("generated_summary", json!(["nonobject.json"]), 20, "lacks context_summary"),
        ("generated_summary", json!(["summary.json", "wrong.json"]), 20, "requires exactly one configured surface"),
        ("agents_route_max_inherited", json!(["first.json", "second.json"]), 4, "context budget exceeded: 5>4"),
        ("agents_route_max_inherited", json!(["empty.json"]), 4, "produced no inherited measurements"),
    ] {
        let probe = json!({"context_probes":[{"id":"test","surfaces":surfaces,"measure":measure,"max_tokens":maximum}]});
        let mut issues = Vec::new();
        guards::validate_context_probes(&mut RouteSources::new(&fixture.root).unwrap(), &probe, &mut issues).unwrap();
        assert!(issues.iter().any(|v| v.1.contains(expected)), "{measure}: {issues:?}");
    }
    let probe = json!({"context_probes":[{"id":"summary","surfaces":["summary.json"],"measure":"generated_summary","max_tokens":2}]});
    let mut issues = Vec::new();
    let measured = guards::validate_context_probes(&mut RouteSources::new(&fixture.root).unwrap(), &probe, &mut issues).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(measured["summary"].to_string(), "2");
}

#[test]
fn markdown_definition_and_executable_presence_are_rechecked_after_mutation() {
    let fixture = Fixture::new();
    fixture.write("docs/target.md", b"# Target\n");
    fixture.write("docs/route.md", b"[guide][guide]\n[guide]: target.md\n");
    let tracked = vec!["docs/route.md".into(), "docs/target.md".into()];
    let mut issues = Vec::new();
    guards::validate_markdown_routes(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &tracked, &mut issues).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    fixture.write("docs/route.md", b"[guide][guide]\n[guide]: missing.md\n");
    guards::validate_markdown_routes(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &tracked, &mut issues).unwrap();
    assert!(issues.iter().any(|v| v.1 == "broken local documentation route: missing.md"));
    let target = "access/deploy/cloudflare-worker/scripts/build_runtime.sh";
    fixture.write(target, b"# declared platform bridge\n");
    fixture.write("docs/route.md", format!("Run `{target}`.\n").as_bytes());
    fixture.write("docs/validation/script_inventory.json", &serde_json::to_vec(&json!({"script_surfaces":[{"path":target}]})).unwrap());
    issues.clear();
    guards::validate_executable_routes(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &tracked, &mut issues).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    fs::remove_file(fixture.root.join(target)).unwrap();
    fixture.write("evals/AGENTS.md", b"Run `scripts/not-present.sh`.\n");
    fixture.write("docs/decisions/TOS-D-9999-history.md", b"Decision ID: TOS-D-9999\nRun scripts/retired-historical.py\n");
    fixture.write("docs/decisions/TOS-D-9998-wrong-identity.md", b"Decision ID: TOS-D-9997\nRun scripts/unaccepted-historical.py\n");
    let tracked = ["docs/route.md".into(), "evals/AGENTS.md".into(), "docs/decisions/TOS-D-9999-history.md".into(), "docs/decisions/TOS-D-9998-wrong-identity.md".into()];
    guards::validate_executable_routes(&fixture.root, &mut RouteSources::new(&fixture.root).unwrap(), &tracked, &mut issues).unwrap();
    for expected in [target, "scripts/not-present.sh", "scripts/unaccepted-historical.py"] {
        assert!(issues.iter().any(|v| v.1 == format!("stale executable reference: {expected}")), "{issues:?}");
    }
    assert!(!issues.iter().any(|v| v.1.contains("scripts/retired-historical.py")));
}

#[test]
fn public_guard_allows_domain_terminology_and_rejects_host_receipt_markers() {
    let fixture = Fixture::new();
    let payload = json!({"public_authored_surfaces":["public.md"],"public_forbidden_markers":["/srv/","holder_pid"]});
    fixture.write("public.md", b"The provider route is public-safe.\n");
    let mut issues = Vec::new();
    guards::validate_public_safety(&mut RouteSources::new(&fixture.root).unwrap(), &payload, &mut issues).unwrap();
    assert!(issues.is_empty());
    fixture.write("public.md", b"provider route /srv/private holder_pid\n");
    guards::validate_public_safety(&mut RouteSources::new(&fixture.root).unwrap(), &payload, &mut issues).unwrap();
    assert_eq!(issues.len(), 2);
}
