//! Independent mechanics assertions retained from the package-local tests.
//! The subjects are native kernels; no Python builder/validator is invoked.
use crate::threshold_registry;
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

/// Run the retained assertions against the selected repository's authored bytes.
/// The supervisor owns time, output, cancellation and child-process limits.
pub fn run(root: &Path, home: &str) -> std::io::Result<usize> {
    let count = match home {
        "mechanics/agon/parts/threshold-registry" => {
            agon_generated_shape_and_native_builder_validator(root);
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
