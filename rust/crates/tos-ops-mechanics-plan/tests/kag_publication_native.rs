//! Real native CLI publication with a controlled foreign-owner subprocess.
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::{fs, io};
use tos_ops_mechanics_plan::{
    kag_corpus_export::{PRIMARY, SOURCE_PATHS},
    kag_release::canonical,
};
fn digest(raw: &[u8]) -> String {
    let mut h = tos_foundation::Digest256Hasher::new();
    h.update(raw);
    h.finalize().to_hex()
}
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
mod fixtures {
    use super::*;
    include!("support/kag_fixture.rs");
}
use fixtures::{fixture, repo};
// The selected foreign owner is a controlled subprocess in these tests.
// Its CLI protocol is exercised without importing or executing ToS Python.
fn selected_owner(base: &Path, source_hash: &str) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let kag = base.join("selected-kag");
    for path in tos_ops_mechanics_plan::kag_release::PROGRAM_PATHS {
        write_new(&kag.join(path), b"controlled foreign owner fixture\n").unwrap();
    }
    write_new(&kag.join("mode"), b"ok\n").unwrap();
    write_new(
        &kag.join("probe.json"),
        &canonical(&json!({
            "primary_source": {"identity": {"path": PRIMARY, "content_hash": source_hash},
                "owner_return_route": {"repo": "Tree-of-Sophia", "surface": PRIMARY}},
            "distribution_identity": {"corpus": "controlled-fixture"}
        }))
        .unwrap(),
    )
    .unwrap();
    let interpreter = base.join("foreign-owner-runner");
    write_new(&interpreter, br##"#!/bin/sh
set -eu
program=$1
shift
test "$1" = --repo-root
provider=$2
test "$3" = --artifact-root
artifacts=$4
shift 4
mode=$(cat mode)
case "$program" in
  */scripts/build_repo_local_kag_release.py)
    test "$#" = 0
    case "$mode" in
      fail) echo 'controlled producer failure' >&2; exit 7;;
      mutate) printf changed >> "$provider/ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json";;
      control-mutate) printf changed >> "$provider/kag/README.md";;
      program-mutate) printf changed >> scripts/query_repo_local_kag.py;;
    esac
    mkdir -p "$provider/kag/indexes"
    printf '{"fixture":true}' > "$provider/kag/indexes/hot.json"
    printf '{"fixture":true}' > "$artifacts/config.json"
    ;;
  */scripts/validate_repo_local_kag_family.py)
    test "$#" = 3
    test "$1" = --no-shadow-git
    test "$2" = --probe-source
    test "$3" = ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json
    test "$mode" != control-fail || { echo 'provider-home source reference refused' >&2; exit 9; }
    cat probe.json
    ;;
  *) exit 11;;
esac
"##).unwrap();
    fs::set_permissions(&interpreter, fs::Permissions::from_mode(0o700)).unwrap();
    (kag, interpreter)
}

fn publish(
    store: &Path,
    revision: &str,
    kag: &Path,
    release: &Path,
    runner: &Path,
) -> io::Result<Value> {
    let executable = std::env::var_os("TOS_KAG_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-kag-release").into());
    let result = std::process::Command::new(executable)
        .arg("build")
        .arg("--repo-root")
        .arg(repo())
        .arg("--store")
        .arg(store)
        .arg("--revision")
        .arg(revision)
        .arg("--kag-root")
        .arg(kag)
        .arg("--release-root")
        .arg(release)
        .arg("--python")
        .arg(runner)
        .output()?;
    if !result.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&result.stderr).into_owned(),
        ));
    }
    serde_json::from_slice(&result.stdout).map_err(io::Error::other)
}

fn source_hash(store: &Path, revision: &str) -> String {
    let snapshot: Value = serde_json::from_slice(
        &fs::read(store.join("revisions").join(revision).join("snapshot.json")).unwrap(),
    )
    .unwrap();
    snapshot["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["path"] == PRIMARY)
        .unwrap()["sha256"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn publication_is_deterministic_preserves_prior_success_and_binds_owner_programs() {
    let (base, store, revision) = fixture();
    let (kag, runner) = selected_owner(&base.0, &source_hash(&store, &revision));
    let release = base.0.join("release");
    let first = publish(&store, &revision, &kag, &release, &runner).unwrap();
    assert_eq!(first["programs"].as_array().unwrap().len(), 5);
    assert_eq!(
        publish(&store, &revision, &kag, &release, &runner).unwrap(),
        first
    );
    let other = base.0.join("other-release");
    assert_eq!(
        publish(&store, &revision, &kag, &other, &runner).unwrap(),
        first
    );
    let status = tos_ops_mechanics_plan::kag_release::status_release(&release, &revision).unwrap();
    assert_eq!(status["freshness"], "current");
    assert_eq!(
        status["integration_revision"],
        first["integration_revision"]
    );
    assert_eq!(status["primary_source"], first["primary_source"]);
    let old = release
        .join("releases")
        .join(first["integration_revision"].as_str().unwrap());
    fs::write(kag.join("mode"), b"fail\n").unwrap();
    assert!(publish(&store, &revision, &kag, &release, &runner).is_err());
    assert_eq!(
        tos_ops_mechanics_plan::kag_release::verify_integration(&old, &revision).unwrap(),
        first
    );
    let status =
        tos_ops_mechanics_plan::kag_downstream_status::Status::new(&release.join("status"), "kag")
            .unwrap()
            .status(&revision)
            .unwrap();
    assert_eq!(status["freshness"], "current");
    assert_eq!(status["state"]["latest"]["state"], "failed");
    fs::write(kag.join("mode"), b"ok\n").unwrap();
    fs::write(
        kag.join("scripts/query_repo_local_kag.py"),
        b"changed foreign owner\n",
    )
    .unwrap();
    let second = publish(&store, &revision, &kag, &release, &runner).unwrap();
    assert_ne!(
        second["integration_revision"],
        first["integration_revision"]
    );
    assert_eq!(
        tos_ops_mechanics_plan::kag_release::verify_integration(&old, &revision).unwrap(),
        first
    );
}

#[test]
fn owner_failure_mutation_and_invalid_probe_never_publish() {
    for mode in [
        "fail",
        "mutate",
        "control-mutate",
        "program-mutate",
        "control-fail",
        "mismatch",
        "duplicate",
    ] {
        let (base, store, revision) = fixture();
        let (kag, runner) = selected_owner(&base.0, &source_hash(&store, &revision));
        fs::write(kag.join("mode"), mode).unwrap();
        if mode == "mismatch" {
            let mut probe: Value =
                serde_json::from_slice(&fs::read(kag.join("probe.json")).unwrap()).unwrap();
            probe["primary_source"]["identity"]["content_hash"] = json!("f".repeat(64));
            fs::write(kag.join("probe.json"), canonical(&probe).unwrap()).unwrap();
        } else if mode == "duplicate" {
            fs::write(
                kag.join("probe.json"),
                b"{\"primary_source\":{},\"primary_source\":{}}",
            )
            .unwrap();
        }
        let release = base.0.join("release");
        assert!(
            publish(&store, &revision, &kag, &release, &runner).is_err(),
            "accepted {mode}"
        );
        assert_eq!(fs::read_dir(release.join("releases")).unwrap().count(), 0);
        let status = tos_ops_mechanics_plan::kag_downstream_status::Status::new(
            &release.join("status"),
            "kag",
        )
        .unwrap()
        .status(&revision)
        .unwrap();
        assert_eq!(status["freshness"], "missing");
        assert_eq!(status["state"]["latest"]["state"], "failed");
    }
}

fn rewrite_integration(path: &Path, value: &mut Value) {
    value
        .as_object_mut()
        .unwrap()
        .remove("integration_revision");
    value["integration_revision"] = json!(digest(&canonical(value).unwrap()));
    fs::write(path.join("integration.json"), canonical(value).unwrap()).unwrap();
}

#[test]
fn complete_membership_tamper_and_historical_program_binding_are_checked() {
    for case in [
        "extra",
        "member",
        "manifest",
        "primary",
        "historical",
        "unknown-program",
    ] {
        let (base, store, revision) = fixture();
        let (kag, runner) = selected_owner(&base.0, &source_hash(&store, &revision));
        let release = base.0.join("release");
        let mut integration = publish(&store, &revision, &kag, &release, &runner).unwrap();
        let path = release
            .join("releases")
            .join(integration["integration_revision"].as_str().unwrap());
        match case {
            "extra" => fs::write(path.join("unexpected.bin"), b"extra").unwrap(),
            "member" => fs::write(path.join("artifacts/config.json"), b"changed").unwrap(),
            "manifest" => fs::write(path.join("integration.json"), b"invalid JSON").unwrap(),
            "primary" => {
                integration["primary_source"]["identity"]["content_hash"] = json!("f".repeat(64));
                rewrite_integration(&path, &mut integration);
            }
            "historical" => {
                integration["programs"].as_array_mut().unwrap().remove(2);
                rewrite_integration(&path, &mut integration);
            }
            "unknown-program" => {
                integration["programs"][2]["path"] = json!("scripts/unrecognized.py");
                rewrite_integration(&path, &mut integration);
            }
            _ => unreachable!(),
        }
        let observed = tos_ops_mechanics_plan::kag_release::verify_integration(&path, &revision);
        if case == "historical" {
            assert_eq!(observed.unwrap(), integration);
        } else {
            assert!(observed.is_err(), "accepted {case}");
            assert!(
                tos_ops_mechanics_plan::kag_release::status_release(&release, &revision).is_err()
            );
            if case == "extra" {
                assert!(publish(&store, &revision, &kag, &release, &runner).is_err());
                assert!(path.join("unexpected.bin").exists());
            }
        }
    }
    let (base, _, revision) = fixture();
    let missing = base.0.join("missing");
    let status = tos_ops_mechanics_plan::kag_release::status_release(&missing, &revision).unwrap();
    assert_eq!(status["freshness"], "missing");
    assert!(status["integration_revision"].is_null());
    assert!(!missing.exists());
}
