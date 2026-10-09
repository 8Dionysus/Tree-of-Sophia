//! Frozen, provenance-linked source fixtures captured before retirement of
//! the maintained Python test factories. Only the isolated source_root changes.
use flate2::read::GzDecoder;
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
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
            config["uid"] = Value::from(fs::metadata(root).unwrap().uid());
            runtime_raw = serde_json::to_vec_pretty(&config).expect("rebased owner config");
            runtime_raw.push(b'\n');
        }
        let target = root.join(relative);
        fs::create_dir_all(target.parent().unwrap()).expect("create captured fixture parent");
        let mut directory = target.parent().unwrap();
        while directory != root {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).unwrap();
            directory = directory.parent().unwrap();
        }
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

/// A single frozen Item fixture for owner-internal retained-orphan recovery.
/// The captured root is preserved; only the four explicitly selected external
/// locations in its protected grant are rebased into the isolated test root.
pub(crate) fn item_orphan_recovery(_repository: &Path, root: &Path) -> Value {
    const MANIFEST_SHA256: &str =
        "6d024a3a1bca79603e5d6b1afb172f2ab774db6bd27eb08d64d7df6b032af9ad";
    const ARCHIVE_SHA256: &str = "bc472cbb1c7f05010f9d96993efa478778d8e11c78a8a34b4f9c633e07899ef6";
    const ARCHIVE: &[u8] =
        include_bytes!("../tests/fixtures/native-item-recovery/item-base.tar.gz");
    let manifest_raw = include_bytes!("../tests/fixtures/native-item-recovery/manifest.json");
    assert_eq!(digest(manifest_raw), MANIFEST_SHA256);
    let manifest: Value = serde_json::from_slice(manifest_raw).expect("Item fixture manifest");
    assert_eq!(
        manifest["schema_version"],
        "tos_native_item_recovery_fixture_v1"
    );
    assert_eq!(
        manifest["source"]["capture_run"],
        "tos-combined-factory-capture-r2"
    );
    assert_eq!(manifest["source"]["factory_id"], "item-base");
    assert_eq!(manifest["archive"]["sha256"], ARCHIVE_SHA256);
    assert_eq!(digest(ARCHIVE), ARCHIVE_SHA256);

    let mut decoded = Vec::new();
    GzDecoder::new(ARCHIVE)
        .read_to_end(&mut decoded)
        .expect("decompress captured Item source fixture");
    let mut offset = 0usize;
    let mut packet_raw = None;
    while offset + 512 <= decoded.len() {
        let header = &decoded[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        let field = |range: std::ops::Range<usize>| {
            let raw = &header[range];
            let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
            std::str::from_utf8(&raw[..end]).expect("USTAR header text")
        };
        let name = field(0..100);
        let prefix = field(345..500);
        let relative = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        let size_text = field(124..136).trim();
        let size = if size_text.is_empty() {
            0usize
        } else {
            usize::from_str_radix(size_text, 8).expect("USTAR member size")
        };
        let data_start = offset + 512;
        let data_end = data_start.checked_add(size).expect("USTAR size overflow");
        assert!(data_end <= decoded.len(), "truncated Item fixture archive");
        let kind = header[156];
        if relative == "packet.json" {
            assert_eq!(kind, b'0');
            packet_raw = Some(decoded[data_start..data_end].to_vec());
        } else {
            assert!(safe_relative(&relative), "unsafe Item fixture path");
            let target = root.join(&relative);
            if kind == b'5' {
                fs::create_dir_all(&target).expect("create captured Item directory");
                let mode_text = field(100..108).trim();
                if !mode_text.is_empty() {
                    let mode = u32::from_str_radix(mode_text, 8).expect("USTAR directory mode");
                    fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))
                        .expect("restore captured Item directory mode");
                }
            } else {
                assert_eq!(kind, b'0', "only regular Item fixture files are accepted");
                fs::create_dir_all(target.parent().unwrap()).expect("create Item source parent");
                fs::write(&target, &decoded[data_start..data_end])
                    .expect("materialize captured Item source");
                let mode_text = field(100..108).trim();
                let mode = u32::from_str_radix(mode_text, 8).expect("USTAR file mode");
                fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))
                    .expect("restore captured Item file mode");
            }
        }
        offset = data_start + size.div_ceil(512) * 512;
    }
    let packet_raw = packet_raw.expect("captured Item fixture packet");
    assert_eq!(digest(&packet_raw), manifest["archive"]["packet_sha256"]);
    let mut packet: Value = serde_json::from_slice(&packet_raw).expect("captured Item packet");
    let root = root.canonicalize().expect("isolated Item root");
    let payload_root = root.join("canonical-payload-root");
    fs::create_dir_all(&payload_root).expect("create selected Item payload root");
    fs::set_permissions(&payload_root, fs::Permissions::from_mode(0o700))
        .expect("protect selected Item payload root");
    let recovery_root = root
        .parent()
        .expect("isolated Item root parent")
        .join("item-private-recovery");
    fs::create_dir_all(&recovery_root).expect("create selected Item recovery root");
    fs::set_permissions(&recovery_root, fs::Permissions::from_mode(0o700))
        .expect("protect selected Item recovery root");
    let owner_path = root.join("item-owner.json");
    let input_path = root.join("already-acquired.epub");
    let config = &mut packet["config"];
    config["source_root"] = Value::String(root.to_string_lossy().into_owned());
    config["uid"] = Value::from(fs::metadata(root).unwrap().uid());
    config["payload_root"] = Value::String(payload_root.to_string_lossy().into_owned());
    config["input_path"] = Value::String(input_path.to_string_lossy().into_owned());
    config["recovery_root"] = Value::String(recovery_root.to_string_lossy().into_owned());
    let mut owner_raw = serde_json::to_vec_pretty(config).expect("rebase Item owner grant");
    owner_raw.push(b'\n');
    fs::write(&owner_path, owner_raw).expect("write isolated Item owner grant");
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600))
        .expect("protect isolated Item owner grant");
    packet["owner"] = Value::String(owner_path.to_string_lossy().into_owned());
    packet["input"] = Value::String(input_path.to_string_lossy().into_owned());
    packet
}
