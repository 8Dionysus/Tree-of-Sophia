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
            "`TOS-Q-0004`",
            "`TOS-Q-XXXX`",
            vec!["TOS-Q-0004"],
        ),
        (
            INTEGRATION,
            "## Core boundary",
            "## Convenience boundary",
            vec!["## Core boundary"],
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
}

/// Run the retained assertions against the selected repository's authored bytes.
/// The supervisor owns time, output, cancellation and child-process limits.
pub fn run(root: &Path, home: &str) -> std::io::Result<usize> {
    let count = match home {
        "mechanics/agon/parts/threshold-registry" => {
            agon_generated_shape_and_native_builder_validator(root);
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
