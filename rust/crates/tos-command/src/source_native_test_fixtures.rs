//! Frozen, provenance-linked source fixtures captured before retirement of
//! the maintained Python test factories. Only the isolated source_root changes.
use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path};
use tos_foundation::Digest256;

const FIXTURE_TREE: &str = "rust/crates/tos-command/tests/fixtures/source-native-python-v1";
const MANIFEST_SHA256: &str = "06d9dc44671c508caaf69ab58a15fb1e0757eb2bf37c26b6bcbc8f94514c2b14";
const SOURCE_HEAD: &str = "5a51a1295ecc75f57e1f5565cc0d2429eabd46e6";

fn digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

fn safe_relative(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn packet(repository: &Path, root: &Path, id: &str) -> Value {
    let manifest_path = repository.join(FIXTURE_TREE).join("manifest.json");
    let manifest_raw = fs::read(&manifest_path).expect("frozen source fixture manifest");
    assert_eq!(
        digest(&manifest_raw),
        MANIFEST_SHA256,
        "frozen source fixture manifest drift"
    );
    let manifest: Value =
        serde_json::from_slice(&manifest_raw).expect("valid frozen fixture manifest");
    assert_eq!(
        manifest["schema_version"],
        "tos_source_native_fixture_capture_v1"
    );
    assert_eq!(manifest["provenance"]["repository_head"], SOURCE_HEAD);
    assert_eq!(manifest["capture"]["captured_repo_head"], SOURCE_HEAD);
    let entry = manifest["captures"]
        .as_array()
        .expect("fixture captures")
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("requested captured fixture");
    let owner_ref = entry["owner_ref"].as_str().expect("fixture owner path");
    assert!(safe_relative(owner_ref));
    let files = entry["files"].as_array().expect("fixture file inventory");
    let mut saw_owner = false;
    for file in files {
        let relative = file["path"].as_str().expect("fixture source path");
        assert!(safe_relative(relative));
        let artifact = file["artifact_path"]
            .as_str()
            .expect("fixture artifact path");
        assert!(safe_relative(artifact));
        assert_eq!(artifact, format!("fixtures/{id}/{relative}"));
        let source = repository.join(FIXTURE_TREE).join(artifact);
        let metadata = fs::symlink_metadata(&source).expect("captured fixture file");
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        let raw = fs::read(&source).expect("read captured fixture file");
        assert_eq!(
            raw.len(),
            file["size"].as_u64().unwrap() as usize,
            "fixture size drift: {relative}"
        );
        assert_eq!(
            digest(&raw),
            file["sha256"].as_str().unwrap(),
            "fixture byte drift: {relative}"
        );
        let mode = u32::from_str_radix(file["mode"].as_str().unwrap(), 8).unwrap();
        assert!(mode & !0o777 == 0);
        let mut runtime_raw = raw;
        if relative == owner_ref {
            saw_owner = true;
            let mut config: Value =
                serde_json::from_slice(&runtime_raw).expect("captured owner config");
            assert_eq!(
                config["source_root"], "$TOS_FIXTURE_ROOT",
                "only declared fixture root rebasing is allowed"
            );
            config["source_root"] = Value::String(
                root.canonicalize()
                    .expect("isolated fixture root")
                    .to_string_lossy()
                    .into_owned(),
            );
            runtime_raw = serde_json::to_vec_pretty(&config).expect("rebased owner config");
            runtime_raw.push(b'\n');
        }
        let target = root.join(relative);
        fs::create_dir_all(target.parent().unwrap()).expect("create captured fixture parent");
        fs::write(&target, runtime_raw).expect("write captured fixture file");
        fs::set_permissions(&target, fs::Permissions::from_mode(mode))
            .expect("restore captured fixture mode");
    }
    assert!(saw_owner, "captured fixture owner must be present");
    let mut result = entry.clone();
    result["owner"] = Value::String(root.join(owner_ref).to_string_lossy().into_owned());
    result
}

pub(crate) fn record_revision(repository: &Path, root: &Path, family: usize) -> Value {
    let id = match family {
        0..=6 => format!("record-family-{family}"),
        7 | 8 => "record-family-4".to_owned(),
        9 => "record-family-0".to_owned(),
        _ => panic!("unknown captured record revision family"),
    };
    packet(repository, root, &id)
}

pub(crate) fn work_expression(repository: &Path, root: &Path) -> Value {
    packet(repository, root, "work-expression")
}
