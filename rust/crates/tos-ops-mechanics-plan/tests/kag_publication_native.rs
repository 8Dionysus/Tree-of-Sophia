//! Actual standalone source-export CLI, with no consumer repository or runtime.
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::{fs, io};
use tos_ops_mechanics_plan::{
    kag_corpus_export::{PRIMARY, SOURCE_PATHS},
    release_files::canonical,
};
fn digest(raw: &[u8]) -> String {
    tos_foundation::Digest256::of_bytes(raw).to_hex()
}
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}
mod fixtures {
    use super::*;
    include!("support/kag_fixture.rs");
}
use fixtures::{fixture, repo};
#[test]
fn standalone_export_preserves_exact_source_and_refuses_tampering_and_foreign_options() {
    let (base, store, revision) = fixture();
    let output = base.0.join("export");
    let binary = env!("CARGO_BIN_EXE_tos-kag-release");
    let built = std::process::Command::new(binary)
        .args(["export-build", "--repo-root"])
        .arg(repo())
        .arg("--store")
        .arg(&store)
        .args(["--revision", &revision, "--output"])
        .arg(&output)
        .env("PATH", base.0.join("no-tools"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let manifest: Value = serde_json::from_slice(&built.stdout).unwrap();
    assert_eq!(manifest["corpus_revision"], revision);
    let verified = || {
        std::process::Command::new(binary)
            .args(["export-verify", "--release"])
            .arg(&output)
            .env("PATH", base.0.join("no-tools"))
            .output()
            .unwrap()
    };
    let checked = verified();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&checked.stdout).unwrap(),
        manifest
    );
    for option in ["--python", "--kag-root"] {
        assert!(
            !std::process::Command::new(binary)
                .args(["export-verify", "--release"])
                .arg(&output)
                .arg(option)
                .arg("/unselected/foreign-runtime")
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let subject = output.join("Tree-of-Sophia").join(PRIMARY);
    let original = fs::read(&subject).unwrap();
    fs::write(&subject, b"changed").unwrap();
    assert!(!verified().status.success());
    fs::write(&subject, original).unwrap();
    assert!(verified().status.success());
    fs::write(output.join("extra.json"), b"{}").unwrap();
    assert!(!verified().status.success());
}

#[test]
fn root_map_materialization_uses_verified_export_and_refuses_absence_staleness_and_tampering() {
    use std::collections::BTreeSet;
    use std::sync::atomic::AtomicI32;
    use tos_ops_mechanics_plan::{kag_corpus_export, root_entry_map, route_cards::RouteSources};

    let (base, store, revision) = fixture();
    let export = base.0.join("root-map-export");
    kag_corpus_export::build_export(&repo(), &store, &revision, &export).unwrap();
    kag_corpus_export::verify_export(&export).unwrap();
    let root = base.0.join("standalone-consumer");
    fs::create_dir(&root).unwrap();
    // Use only the verified synthetic source closure. Do not materialize the
    // capsule in the consumer root or accept an authored JSON copy as an export.
    for path in SOURCE_PATHS {
        write_new(
            &root.join(path),
            &fs::read(export.join("Tree-of-Sophia").join(path)).unwrap(),
        )
        .unwrap();
    }
    let source: Value = serde_json::from_str(include_str!(
        "../../../../scripts/root_entry_map.source.json"
    ))
    .unwrap();
    let mut refs = BTreeSet::new();
    for key in [
        "schema_ref",
        "authority_ref",
        "public_root_ref",
        "current_tiny_entry_ref",
        "export_ref",
    ] {
        refs.insert(source[key].as_str().unwrap());
    }
    for reference in source["validation_refs"].as_array().unwrap() {
        refs.insert(reference.as_str().unwrap());
    }
    for route in source["routes"].as_array().unwrap() {
        refs.insert(route["surface_ref"].as_str().unwrap());
        for reference in route["verification_refs"].as_array().unwrap() {
            refs.insert(reference.as_str().unwrap());
        }
    }
    for key in ["authority_ref", "contract_version"] {
        refs.insert(source["artifact_identity"][key].as_str().unwrap());
    }
    for reference in refs {
        let path = reference.split('#').next().unwrap();
        if path != kag_corpus_export::CAPSULE
            && path != root_entry_map::SOURCE
            && !root.join(path).exists()
        {
            write_new(&root.join(path), b"bounded synthetic owner route\n").unwrap();
        }
    }
    fs::write(
        root.join("ToS/contracts/root-entry-map.schema.json"),
        include_bytes!("../../../../ToS/contracts/root-entry-map.schema.json"),
    )
    .unwrap();
    write_new(
        &root.join(root_entry_map::SOURCE),
        include_bytes!("../../../../scripts/root_entry_map.source.json"),
    )
    .unwrap();
    let cancel = AtomicI32::new(0);
    let missing = root_entry_map::validate_with_export(
        &root,
        &mut RouteSources::new(&root).unwrap(),
        Some(&export),
        &cancel,
    )
    .unwrap();
    assert!(
        missing
            .iter()
            .any(|(_, issue)| issue.contains("not materialized"))
    );
    assert!(!root.join(root_entry_map::OUTPUT).exists());
    assert!(
        root_entry_map::build_with_export(
            &root,
            &mut RouteSources::new(&root).unwrap(),
            Some(&export),
            &cancel,
            false
        )
        .unwrap()
    );
    let output = root.join(root_entry_map::OUTPUT);
    let mut materialized: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(materialized["schema_version"], "tos_root_entry_map_v1");
    assert!(
        root_entry_map::validate_with_export(
            &root,
            &mut RouteSources::new(&root).unwrap(),
            Some(&export),
            &cancel
        )
        .unwrap()
        .is_empty()
    );
    assert!(!root.join(kag_corpus_export::CAPSULE).exists());
    materialized["routes"][0]["need"] = json!("stale synthetic need");
    fs::write(&output, root_entry_map::render(&materialized).unwrap()).unwrap();
    let stale = root_entry_map::validate_with_export(
        &root,
        &mut RouteSources::new(&root).unwrap(),
        Some(&export),
        &cancel,
    )
    .unwrap();
    assert!(
        stale
            .iter()
            .any(|(_, issue)| issue.contains("canonical rebuild"))
    );
    let before = fs::read(&output).unwrap();
    fs::write(
        export
            .join("Tree-of-Sophia")
            .join(kag_corpus_export::CAPSULE),
        b"tampered export",
    )
    .unwrap();
    assert!(
        root_entry_map::build_with_export(
            &root,
            &mut RouteSources::new(&root).unwrap(),
            Some(&export),
            &cancel,
            false
        )
        .is_err()
    );
    assert_eq!(fs::read(&output).unwrap(), before);
}
