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
