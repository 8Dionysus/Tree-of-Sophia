//! Independent mechanics assertions retained from the package-local tests.
//! The subjects are native kernels; no Python builder/validator is invoked.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tos_ops_mechanics_plan::{questbook, threshold_registry};

#[path = "mechanics_contracts/experience.rs"]
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
    ($fixture:expr, $path:literal) => {
        $fixture.write($path, include_bytes!(concat!("../../../../", $path)))
    };
}

#[test]
fn agon_generated_shape_and_native_builder_validator() {
    let f = Fixture::new("agon");
    authored!(
        f,
        "mechanics/agon/parts/threshold-registry/config/tos_agon_threshold_intakes.config.json"
    );
    authored!(
        f,
        "mechanics/agon/parts/threshold-registry/generated/tos_agon_threshold_intake_registry.min.json"
    );
    authored!(
        f,
        "mechanics/agon/parts/threshold-registry/schemas/tos-agon-threshold-intake-registry.schema.json"
    );
    authored!(
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
fn quest_fixture() -> Fixture {
    let f = Fixture::new("questbook");
    authored!(f, "QUESTBOOK.md");
    authored!(
        f,
        "mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md"
    );
    authored!(
        f,
        "mechanics/questbook/parts/dispatch-contracts/schemas/quest.schema.json"
    );
    authored!(
        f,
        "mechanics/questbook/parts/dispatch-contracts/schemas/quest_dispatch.schema.json"
    );
    authored!(
        f,
        "mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json"
    );
    authored!(
        f,
        "mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json"
    );
    authored!(f, "quests/TOS-Q-0001.yaml");
    authored!(f, "quests/TOS-Q-0002.yaml");
    authored!(f, "quests/TOS-Q-0003.yaml");
    authored!(f, "quests/TOS-Q-0004.yaml");
    f
}
fn refused(root: &Path, tokens: &[&str]) {
    let message = questbook::validate_surface(root).unwrap_err().to_string();
    for token in tokens {
        assert!(message.contains(token), "{message}");
    }
}

#[test]
fn questbook_valid_surface_and_all_retained_mutation_boundaries() {
    let valid = quest_fixture();
    questbook::validate_surface(&valid.0).unwrap();
    drop(valid);
    for (path, token) in [
        (INTEGRATION, INTEGRATION),
        ("quests/TOS-Q-0003.yaml", "TOS-Q-0003.yaml"),
    ] {
        let f = quest_fixture();
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
        let f = quest_fixture();
        f.replace(path, before, after);
        refused(&f.0, &tokens);
    }
}
