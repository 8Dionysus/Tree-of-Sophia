//! Independent mechanics assertions retained from the package-local tests.
//! The subjects are native kernels; no Python builder/validator is invoked.
use crate::{questbook, threshold_registry};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "experience.rs"]
mod experience;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tos-mechanics-contract-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // A foreign/pre-existing path is never adopted or removed.
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn write(&self, relative: &str, raw: &[u8]) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, raw).unwrap();
    }
    fn replace(&self, relative: &str, before: &str, after: &str) {
        let path = self.0.join(relative);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(before), "fixture mutation must hit {relative}");
        fs::write(path, raw.replacen(before, after, 1)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

macro_rules! authored {
    ($root:expr, $fixture:expr, $path:literal) => {
        $fixture.write($path, &fs::read($root.join($path)).unwrap())
    };
}

pub fn agon_generated_shape_and_native_builder_validator(root: &Path) {
    let f = Fixture::new("agon");
    authored!(
        root,
        f,
        "mechanics/agon/parts/threshold-registry/config/tos_agon_threshold_intakes.config.json"
    );
    authored!(
        root,
        f,
        "mechanics/agon/parts/threshold-registry/generated/tos_agon_threshold_intake_registry.min.json"
    );
    authored!(
        root,
        f,
        "mechanics/agon/parts/threshold-registry/schemas/tos-agon-threshold-intake-registry.schema.json"
    );
    authored!(
        root,
        f,
        "mechanics/agon/parts/threshold-intake/schemas/tos-agon-threshold-intake.schema.json"
    );
    let source: serde_json::Value = serde_json::from_slice(
        &fs::read(f.0.join(
            "mechanics/agon/parts/threshold-registry/config/tos_agon_threshold_intakes.config.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let generated: serde_json::Value = serde_json::from_slice(&fs::read(f.0.join(
        "mechanics/agon/parts/threshold-registry/generated/tos_agon_threshold_intake_registry.min.json"
    )).unwrap()).unwrap();
    let count = source["threshold_intakes"].as_array().unwrap().len();
    assert!(count > 0);
    assert_eq!(generated["review_phase_order"], "XVIII");
    assert_eq!(generated["count"].as_u64(), Some(count as u64));
    let items = generated["threshold_intakes"].as_array().unwrap();
    assert_eq!(items.len(), count);
    assert!(items.iter().all(|item| item["live_protocol"] == false));
    assert!(
        items
            .iter()
            .all(|item| item["review_status"] == "candidate_only")
    );
    threshold_registry::build(&f.0, true).unwrap();
    threshold_registry::validate(&f.0).unwrap();
}

const INTEGRATION: &str =
    "mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md";
const DISPATCH: &str =
    "mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json";
fn quest_fixture(root: &Path) -> Fixture {
    let f = Fixture::new("questbook");
    authored!(root, f, "QUESTBOOK.md");
    authored!(
        root,
        f,
        "mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md"
    );
    authored!(
        root,
        f,
        "mechanics/questbook/parts/dispatch-contracts/schemas/quest.schema.json"
    );
    authored!(
        root,
        f,
        "mechanics/questbook/parts/dispatch-contracts/schemas/quest_dispatch.schema.json"
    );
    authored!(
        root,
        f,
        "mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json"
    );
    authored!(
        root,
        f,
        "mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json"
    );
    authored!(root, f, "quests/TOS-Q-0001.yaml");
    authored!(root, f, "quests/TOS-Q-0002.yaml");
    authored!(root, f, "quests/TOS-Q-0003.yaml");
    authored!(root, f, "quests/TOS-Q-0004.yaml");
    authored!(
        root,
        f,
        "quests/TOS-Q-AGON-0001-sophian-threshold-intake.md"
    );
    for path in [
        "ToS/AGENTS.md",
        "ToS/candidate-intake/AGENTS.md",
        "ToS/derived-exports/AGENTS.md",
        "mechanics/questbook/AGENTS.md",
        "mechanics/agon/AGENTS.md",
        "mechanics/boundary-bridge/AGENTS.md",
    ] {
        f.write(path, &fs::read(root.join(path)).unwrap());
    }
    f
}
fn refused(root: &Path, tokens: &[&str]) {
    let message = questbook::validate_surface(root).unwrap_err().to_string();
    for token in tokens {
        assert!(message.contains(token), "{message}");
    }
}

pub fn questbook_valid_surface_and_all_retained_mutation_boundaries(root: &Path) {
    // The direct-helper missing-mode assertion is independent of full schema
    // validation: no projection field access may obscure this diagnostic.
    let error = questbook::dispatch_entry("TOS-Q-0001", &serde_json::json!({"activation": {}}))
        .unwrap_err();
    assert!(error.to_string().contains("activation.mode"));
    let valid = quest_fixture(root);
    questbook::validate_surface(&valid.0).unwrap();
    let rows: serde_json::Value =
        serde_json::from_slice(&fs::read(valid.0.join(DISPATCH)).unwrap()).unwrap();
    let mut synthetic = rows[0].clone();
    synthetic["kind"] = serde_json::json!("guardrail");
    synthetic["activation"] = serde_json::json!({"mode": "manual"});
    let projected = questbook::dispatch_entry("TOS-Q-SYNTHETIC", &synthetic).unwrap();
    assert_eq!(
        projected["requires_artifacts"],
        serde_json::json!(["bounded_plan", "guardrail_check", "verification_result"])
    );
    synthetic["kind"] = serde_json::json!("seam");
    let projected = questbook::dispatch_entry("TOS-Q-0002", &synthetic).unwrap();
    assert_eq!(
        projected["requires_artifacts"],
        serde_json::json!(["bounded_plan", "work_result", "verification_result"])
    );
    drop(valid);
    for (path, token) in [
        (INTEGRATION, INTEGRATION),
        ("quests/TOS-Q-0003.yaml", "TOS-Q-0003.yaml"),
    ] {
        let f = quest_fixture(root);
        fs::remove_file(f.0.join(path)).unwrap();
        refused(&f.0, &[token]);
    }
    for (path, before, after, tokens) in [
        (
            "quests/TOS-Q-0002.yaml",
            "repo: Tree-of-Sophia",
            "repo: aoa-kag",
            vec!["repo must equal 'Tree-of-Sophia'"],
        ),
        (
            "quests/TOS-Q-0004.yaml",
            "id: TOS-Q-0004",
            "id: TOS-Q-9999",
            vec!["id must equal 'TOS-Q-0004'"],
        ),
        (
            "quests/TOS-Q-0001.yaml",
            "public_safe: true",
            "public_safe: false",
            vec!["public_safe must be true"],
        ),
        (
            "QUESTBOOK.md",
            "\"id\": \"TOS-Q-0004\"",
            "\"id\": \"TOS-Q-0003\"",
            vec!["duplicate obligation id"],
        ),
        (
            "QUESTBOOK.md",
            "\"runtime_execution\": false",
            "\"runtime_execution\": true",
            vec!["runtime_execution must remain false"],
        ),
        (
            DISPATCH,
            "\"source_path\": \"quests/TOS-Q-0004.yaml\"",
            "\"source_path\": \"quests/TOS-Q-9999.yaml\"",
            vec!["quest_dispatch.min.example.json"],
        ),
        (
            "quests/TOS-Q-0001.yaml",
            "activation:\n  mode: immediate\n",
            "activation: {}\n",
            vec!["activation", "mode"],
        ),
        (
            DISPATCH,
            "\"activation_mode\": \"immediate\"",
            "\"activation_mode\": \"not-a-valid-mode\"",
            vec!["quest_dispatch.schema.json"],
        ),
    ] {
        let f = quest_fixture(root);
        f.replace(path, before, after);
        refused(&f.0, &tokens);
    }
    // A complete core register needs neither the demonstration quartet nor
    // legacy projection files. Integration prose is not a phrase password.
    let f = Fixture::new("empty-register");
    f.write(
        INTEGRATION,
        b"Source meaning and activation stay with their owners.\n",
    );
    f.write("QUESTBOOK.md", b"# Operational obligations\n\n```tos-obligations-v1\n{\"schema_version\":\"tos_obligations_v1\",\"runtime_execution\":false,\"canon_admission\":false,\"records\":[]}\n```\n");
    questbook::validate_surface(&f.0).unwrap();

    // State, owner, history, return condition and canon refusal remain checked.
    for (before, after, token) in [
        (
            "\"canon_admission\": false",
            "\"canon_admission\": true",
            "canon_admission",
        ),
        (
            "\"owner_ref\": \"mechanics/questbook/AGENTS.md\"",
            "\"owner_ref\": \"missing.md\"",
            "owner_ref",
        ),
        (
            "\"source_return\": \"ToS/AGENTS.md\"",
            "\"source_return\": \"mechanics/questbook/AGENTS.md\"",
            "source_return",
        ),
        (
            "\"state\": \"done\"",
            "\"state\": \"invented\"",
            "obligation state",
        ),
        (
            "\"record_kind\": \"authored_note\"",
            "\"record_kind\": \"automatic_dispatch\"",
            "record kind",
        ),
        (
            "https://github.com/8Dionysus/Tree-of-Sophia/blob/",
            "https://example.invalid/blob/",
            "immutable record history",
        ),
    ] {
        let f = quest_fixture(root);
        f.replace("QUESTBOOK.md", before, after);
        refused(&f.0, &[token]);
    }
    let f = quest_fixture(root);
    let text = fs::read_to_string(f.0.join("QUESTBOOK.md")).unwrap();
    let marker = "```tos-obligations-v1\n";
    let (before, tail) = text.split_once(marker).unwrap();
    let (raw, after) = tail.split_once("\n```").unwrap();
    let mut register: serde_json::Value = serde_json::from_str(raw).unwrap();
    register["records"][2]["return_when"] = serde_json::json!("");
    f.write(
        "QUESTBOOK.md",
        format!(
            "{before}{marker}{}{after_prefix}{after}",
            register,
            after_prefix = "\n```"
        )
        .as_bytes(),
    );
    refused(&f.0, &["return condition"]);
    register["records"][2]["return_when"] = serde_json::json!("Manual source review");
    register["records"].as_array_mut().unwrap().pop();
    f.write(
        "QUESTBOOK.md",
        format!("{before}{marker}{}\n```{after}", register).as_bytes(),
    );
    refused(&f.0, &["unregistered retained obligation"]);
}

/// Full native topology validation accepts a complete compact package with no
/// PARTS/PROVENANCE/ROADMAP files, and rejects loss of its actual route carriers.
pub fn compact_topology_preserves_owner_history_and_source_return(_root: &Path) {
    use serde_json::json;
    let f = Fixture::new("compact-topology");
    for (path, body) in [
        ("mechanics/AGENTS.md", "# Mechanics owner\n"),
        ("mechanics/VALIDATION.md", "# Mechanical checks\n"),
        ("ToS/AGENTS.md", "# Authored source owner\n"),
        (
            "mechanics/README.md",
            "## Task-to-owner map\n## Progressive disclosure\ntopology.json validation_lanes.json docs/decisions/README.md\n[Agon](agon/README.md)\n",
        ),
        ("mechanics/agon/AGENTS.md", "# Agon owner\n"),
        (
            "mechanics/agon/parts/intake/README.md",
            "# Intake contract\n",
        ),
        (
            "mechanics/agon/README.md",
            "## Operations\n[Intake](parts/intake/README.md)\n## History\n[Prior source](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/agon/PROVENANCE.md)\n",
        ),
        (
            "docs/validation/script_inventory.json",
            "{\"script_surfaces\":[]}\n",
        ),
        ("tests/test_inventory.json", "{\"tests\":[]}\n"),
    ] {
        f.write(path, body.as_bytes());
    }
    let topology = json!({
        "schema_version": "tos_mechanics_topology_v2", "owner_repo": "Tree-of-Sophia", "root": "mechanics/",
        "legacy_policy": "pinned-git-history-no-active-legacy",
        "always_on_context_budget": {"surface": "mechanics/README.md", "metric": "whitespace_tokens_v1", "max_tokens": 1000},
        "packages": [{"slug": "agon", "class": "local", "status": "active", "active_parts": ["intake"], "legacy_required": false,
            "compact_routes": {"owner": "mechanics/agon/AGENTS.md", "selection": "mechanics/agon/README.md#operations",
                "history": "mechanics/agon/README.md#history", "source_return": "ToS/AGENTS.md", "validation": "mechanics/VALIDATION.md",
                "history_revision": "b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d", "history_path": "mechanics/agon/PROVENANCE.md"}}],
        "moved_path_accounting": {}, "moved_path_targets": {}
    });
    f.write("mechanics/topology.json", topology.to_string().as_bytes());
    for args in [&["init", "--quiet"][..], &["add", "--", "."][..]] {
        assert!(
            std::process::Command::new("/usr/bin/git")
                .args(args)
                .current_dir(&f.0)
                .status()
                .unwrap()
                .success()
        );
    }
    assert!(
        crate::mechanics_topology::validate(&f.0)
            .unwrap()
            .is_empty()
    );
    for field in [
        "owner",
        "history",
        "source_return",
        "validation",
        "selection",
        "history_revision",
    ] {
        let mut broken = topology.clone();
        broken["packages"][0]["compact_routes"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        f.write("mechanics/topology.json", broken.to_string().as_bytes());
        let issues = crate::mechanics_topology::validate(&f.0).unwrap();
        assert!(!issues.is_empty(), "missing {field} must fail");
    }
    f.write("mechanics/topology.json", topology.to_string().as_bytes());
    for path in ["mechanics/agon/AGENTS.md", "ToS/AGENTS.md"] {
        let bytes = fs::read(f.0.join(path)).unwrap();
        fs::remove_file(f.0.join(path)).unwrap();
        assert!(
            !crate::mechanics_topology::validate(&f.0)
                .unwrap()
                .is_empty()
        );
        f.write(path, &bytes);
    }
    f.replace(
        "mechanics/agon/README.md",
        "[Prior source]",
        "<!-- [Prior source]",
    );
    let path = f.0.join("mechanics/agon/README.md");
    let mut body = fs::read_to_string(&path).unwrap();
    body.push_str("-->\n");
    fs::write(path, body).unwrap();
    assert!(
        !crate::mechanics_topology::validate(&f.0)
            .unwrap()
            .is_empty()
    );
}

/// Run the retained assertions against the selected repository's authored bytes.
/// The supervisor owns time, output, cancellation and child-process limits.
pub fn run(root: &Path, home: &str) -> std::io::Result<usize> {
    let count = match home {
        "mechanics/agon/parts/threshold-registry" => {
            agon_generated_shape_and_native_builder_validator(root);
            compact_topology_preserves_owner_history_and_source_return(root);
            1
        }
        "mechanics/questbook" => {
            questbook_valid_surface_and_all_retained_mutation_boundaries(root);
            1
        }
        "mechanics/experience" => {
            experience::experience_candidate_all_retained_schema_mutations(root);
            experience::experience_governance_all_retained_schema_mutations(root);
            experience::experience_installation_all_retained_schema_mutations(root);
            3
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported native mechanics assertion home",
            ));
        }
    };
    Ok(count)
}

pub fn experience_candidate(root: &Path) {
    experience::experience_candidate_all_retained_schema_mutations(root);
}

pub fn experience_governance(root: &Path) {
    experience::experience_governance_all_retained_schema_mutations(root);
}

pub fn experience_installation(root: &Path) {
    experience::experience_installation_all_retained_schema_mutations(root);
}
