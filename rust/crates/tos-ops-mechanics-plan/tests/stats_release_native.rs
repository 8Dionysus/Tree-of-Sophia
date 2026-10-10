//! Actual Rust CLI publication/status with a synthetic external owner process.
//! The fixture checks transport and custody, never real stats semantic admission.
#[cfg(target_os = "linux")]
#[test]
fn stats_publication_preserves_exact_observation_and_last_success_on_owner_failure() {
    use serde_json::{Value, json};
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::Path,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tos_ops_mechanics_plan::{
        kag_release::canonical,
        stats_release::{SOURCE_KIND, SOURCE_PATHS},
    };
    let root = std::env::temp_dir().join(format!(
        "tos-stats-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = root.join("source");
    let owner = root.join("explicit owner with spaces");
    let release = root.join("release");
    let packages = root.join("packages");
    fs::create_dir_all(owner.join("scripts")).unwrap();
    fs::create_dir_all(&packages).unwrap();
    for (name, version) in [("jsonschema", "4.0.0"), ("referencing", "0.1.0")] {
        let path = packages.join(format!("{name}-{version}.dist-info"));
        fs::create_dir(&path).unwrap();
        fs::write(
            path.join("METADATA"),
            format!("Name: {name}\nVersion: {version}\n\n"),
        )
        .unwrap();
    }
    let python = root.join("external-runtime-fixture");
    let script = r#"#!/bin/sh
case "$1" in
  --version) printf 'Python 3.14.0\n'; exit 0;;
esac
if [ "$3" = '-m' ] && [ "$4" = 'site' ]; then
  printf "sys.path = [\n    '%s',\n]\n" "$(dirname "$0")/packages"
  exit 0
fi
mode=$(cat "$(dirname "$3")/../src/aoa_stats_builder/mode.py")
case "$mode" in
  reject) printf 'synthetic owner stdout\n'; printf 'synthetic owner refused\n' >&2; exit 19;;
  mutate) printf ' ' >> "$5";;
  extra) mkfifo "$(dirname "$5")/extra.pipe";;
esac
exit 0
"#;
    fs::write(&python, script).unwrap();
    fs::set_permissions(&python, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        owner.join("scripts/validate_stats_protocol.py"),
        b"# synthetic external owner\n",
    )
    .unwrap();
    // The mode is part of the selected validator's declared Python code tree.
    fs::create_dir_all(owner.join("src/aoa_stats_builder")).unwrap();
    let mode = owner.join("src/aoa_stats_builder/mode.py");
    fs::write(&mode, "accept").unwrap();
    for name in SOURCE_PATHS {
        let path = source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"# synthetic source\n").unwrap();
    }
    let port = json!({"evidence_posture":{"live_state":"reference_only","privacy":"public","raw_content_allowed":false}});
    let packet = json!({"observation_id":"synthetic:observation:one","observed_at":"2026-09-14T00:00:00Z",
        "provenance":{"source_revision":"original-packet-source"},"posture":{"live_state":"reference"}});
    fs::write(
        source.join("stats/port.manifest.json"),
        canonical(&port).unwrap(),
    )
    .unwrap();
    fs::write(
        source.join("stats/packets/table-i-prepared-dossier-route-ratio.reference.json"),
        canonical(&packet).unwrap(),
    )
    .unwrap();
    let exe = std::env::var_os("TOS_STATS_RELEASE_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-stats-release").into());
    // The local validation adapter preserves the external owner's streams and
    // exit code, and accepts only the explicitly selected inputs.
    fs::write(&mode, "reject").unwrap();
    let validated = Command::new(&exe)
        .args(["validate", "--stats-root"])
        .arg(&owner)
        .arg("--port")
        .arg(source.join("stats/port.manifest.json"))
        .arg("--python")
        .arg(&python)
        .env("AOA_STATS_ROOT", root.join("unselected"))
        .output()
        .unwrap();
    assert_eq!(validated.status.code(), Some(19));
    assert!(String::from_utf8_lossy(&validated.stdout).contains("synthetic owner stdout"));
    assert!(String::from_utf8_lossy(&validated.stderr).contains("synthetic owner refused"));
    assert!(
        !Command::new(&exe)
            .arg("validate")
            .output()
            .unwrap()
            .status
            .success()
    );
    let linked_port = root.join("linked-port.json");
    std::os::unix::fs::symlink(source.join("stats/port.manifest.json"), &linked_port).unwrap();
    assert!(
        !Command::new(&exe)
            .args(["validate", "--stats-root"])
            .arg(&owner)
            .arg("--port")
            .arg(&linked_port)
            .arg("--python")
            .arg(&python)
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::write(&mode, "accept").unwrap();
    let build = || {
        Command::new(&exe)
            .args(["build", "--source-root"])
            .arg(&source)
            .arg("--stats-root")
            .arg(&owner)
            .arg("--release-root")
            .arg(&release)
            .arg("--python")
            .arg(&python)
            .output()
            .unwrap()
    };
    let status = |expected: &str| -> Value {
        let out = Command::new(&exe)
            .args(["status", "--release-root"])
            .arg(&release)
            .args(["--expected-revision", expected])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    assert!(status(&"0".repeat(64))["state"].is_null());
    assert!(!release.exists(), "status must not create missing state");
    let first = build();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first["source_kind"], SOURCE_KIND);
    let revision = first["source_revision"].as_str().unwrap();
    let observed = status(revision);
    assert_eq!(
        observed["observation"]["source_revision"],
        "original-packet-source"
    );
    assert_eq!(observed["observation"]["live_state"], "reference");
    let success = observed["integration_revision"].clone();
    for failure in ["reject", "mutate", "extra"] {
        fs::write(&mode, failure).unwrap();
        let failed = build();
        assert!(!failed.status.success(), "{failure}");
        let observed = status(revision);
        assert_eq!(observed["integration_revision"], success);
        assert_eq!(observed["latest_attempt"]["state"], "failed");
        assert_eq!(
            observed["observation"],
            status(&"0".repeat(64))["observation"]
        );
    }
    fs::write(&mode, "accept").unwrap();
    fs::write(
        owner.join("src/aoa_stats_builder/new.py"),
        b"# changed imported owner code\n",
    )
    .unwrap();
    let second = build();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second: Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second["source_revision"], first["source_revision"]);
    assert_ne!(
        second["integration_revision"],
        first["integration_revision"]
    );
    let artifact = release
        .join("releases")
        .join(second["integration_revision"].as_str().unwrap());
    fs::write(
        artifact.join("Tree-of-Sophia/stats/README.md"),
        b"tampered\n",
    )
    .unwrap();
    let bad = Command::new(&exe)
        .args(["status", "--release-root"])
        .arg(&release)
        .args(["--expected-revision", revision])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    // Every path here belongs to this unique synthetic fixture, including the
    // deliberately injected FIFO retained after a rejected external consumer.
    assert!(root.starts_with(std::env::temp_dir()));
    assert!(Path::new(&root).is_dir());
    fs::remove_dir_all(root).unwrap();
}
