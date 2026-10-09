use super::*;
use serde_json::{Value, json};
use sha1::Digest as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    payload_root: PathBuf,
    manifest_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let payload_root = temp.path().join("payload-root");
        let contracts = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ToS/contracts")
            .canonicalize()
            .unwrap();
        fs::create_dir_all(root.join("ToS/contracts")).unwrap();
        for entry in fs::read_dir(contracts).unwrap() {
            let path = entry.unwrap().path();
            if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".schema.json")
            {
                fs::copy(
                    &path,
                    root.join("ToS/contracts").join(path.file_name().unwrap()),
                )
                .unwrap();
            }
        }
        fs::create_dir(&payload_root).unwrap();
        fs::set_permissions(&payload_root, fs::Permissions::from_mode(0o700)).unwrap();

        let package = json!({
            "target_slug":"fixture-sutta",
            "records":{},
            "rights":{
                "schema_version":"tos_rights_record_v1",
                "rights_id":"tos.rights.registry-fixture",
                "scope_refs":["fixture:registry"],
                "assessment_status":"not_assessed",
                "jurisdictions_reviewed":[],
                "source_refs":["fixture:registry"],
                "permissions":[],
                "restrictions":[],
                "visibility":"local_only",
                "redistribution_posture":"not_authorized",
                "derivative_posture":"local_research_only",
                "assessed_by":{"maker_type":"imported_source","agent_ref":"software:fixture"},
                "assessed_at":"2026-10-01T00:00:00+00:00",
                "rationale":"Synthetic fixture metadata only.",
                "review_status":"unreviewed",
                "record_version":1
            },
            "claims":[]
        });
        let package_ref = "ToS/source-witnesses/discovery/fixture/prepared-source-packages.jsonl";
        let package_path = root.join(package_ref);
        fs::create_dir_all(package_path.parent().unwrap()).unwrap();
        let mut package_bytes = serde_json::to_vec(&package).unwrap();
        package_bytes.push(b'\n');
        fs::write(&package_path, &package_bytes).unwrap();

        let snapshot_ref = "ToS/source-witnesses/discovery/fixture/source-registry-snapshot.json";
        let snapshot = b"{\"fixture\":\"bound normalized snapshot\"}\n";
        fs::write(root.join(snapshot_ref), snapshot).unwrap();
        let pin = "d6d54741b7f2ddfeca82f02c3f95eb3990b4e351";
        let manifest = json!({
            "schema_version":"tos_registry_first_planting_preparation_v1",
            "status":"prepared-not-acquired",
            "prepared_packages_ref":package_ref,
            "prepared_packages_sha256":sha256(&package_bytes),
            "source_registry_snapshot_ref":snapshot_ref,
            "source_registry_snapshot_sha256":sha256(snapshot),
            "provider_pins":{"bilara":pin},
            "metadata_observations":[],
            "targets":[{
                "slug":"fixture-sutta",
                "family":"pali-canon",
                "repository":"suttacentral/bilara-data",
                "pin":pin,
                "files":[]
            }],
            "totals":{"works":1,"payload_files":0,"payload_bytes":0}
        });
        let manifest_ref = "ToS/source-witnesses/discovery/fixture/manifest.json";
        let manifest_path = root.join(manifest_ref);
        fs::write(&manifest_path, json_bytes(&manifest).unwrap()).unwrap();

        Self {
            _temp: temp,
            root,
            payload_root,
            manifest_path,
        }
    }

    fn request(&self, operation: &str) -> Value {
        json!({
            "operation":operation,
            "root":self.root,
            "manifest_path":self.manifest_path,
            "payload_source_root":self.payload_root
        })
    }
}

fn tree_snapshot(root: &Path) -> Vec<(PathBuf, u32, Option<Vec<u8>>)> {
    fn visit(root: &Path, directory: &Path, output: &mut Vec<(PathBuf, u32, Option<Vec<u8>>)>) {
        let mut entries = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let metadata = fs::symlink_metadata(&path).unwrap();
            let body = if metadata.is_dir() {
                None
            } else {
                Some(fs::read(&path).unwrap())
            };
            output.push((
                path.strip_prefix(root).unwrap().to_path_buf(),
                metadata.permissions().mode() & 0o777,
                body,
            ));
            if metadata.is_dir() {
                visit(root, &path, output);
            }
        }
    }

    let mut output = Vec::new();
    visit(root, root, &mut output);
    output
}

#[test]
fn registry_preparation_verification_is_bound_and_payload_free() {
    let fixture = Fixture::new();
    let before = tree_snapshot(&fixture.root);
    let result = invoke(&fixture.request("registry.verify_preparation")).unwrap();

    assert_eq!(result["status"], "preparation-verified");
    assert_eq!(result["registry_bound"], true);
    assert_eq!(result["payloads_downloaded"], false);
    assert_eq!(result["totals"]["payload_files"], 0);
    assert_eq!(result["totals"]["payload_bytes"], 0);
    assert_eq!(tree_snapshot(&fixture.root), before);
    assert!(tree_snapshot(&fixture.payload_root).is_empty());
}

#[test]
fn registry_preparation_rejects_a_payload_url_outside_its_pinned_provider() {
    let fixture = Fixture::new();
    let mut manifest = read_json(&fixture.manifest_path).unwrap();
    manifest["targets"][0]["paths"] = json!({
        "item_root":"ToS/source-witnesses/works/pali-canon/fixture-sutta/items/root"
    });
    manifest["targets"][0]["files"] = json!([{
        "upstream_path":"root/pli/ms/wrong.json",
        "basename":"wrong.json",
        "byte_size":1,
        "git_blob_sha1":"0000000000000000000000000000000000000000",
        "media_type":"application/json",
        "url":"https://example.invalid/unbound-source"
    }]);
    manifest["totals"]["payload_files"] = json!(1);
    manifest["totals"]["payload_bytes"] = json!(1);
    fs::write(&fixture.manifest_path, json_bytes(&manifest).unwrap()).unwrap();
    let before = tree_snapshot(&fixture.root);

    let error = invoke(&fixture.request("registry.verify_preparation")).unwrap_err();
    assert!(error.contains("unbound source URL"));
    assert_eq!(tree_snapshot(&fixture.root), before);
    assert!(tree_snapshot(&fixture.payload_root).is_empty());
}

#[test]
fn registry_acquisition_rejects_invalid_selection_before_any_write() {
    let fixture = Fixture::new();
    let repository_before = tree_snapshot(&fixture.root);
    let payloads_before = tree_snapshot(&fixture.payload_root);

    let mut malformed = fixture.request("registry.acquire");
    malformed["target_slugs"] = json!("fixture-sutta");
    assert!(
        invoke(&malformed)
            .unwrap_err()
            .contains("target_slugs must be an array of strings")
    );

    let mut unknown = fixture.request("registry.acquire");
    unknown["target_slugs"] = json!(["unknown-target"]);
    assert!(
        invoke(&unknown)
            .unwrap_err()
            .contains("unknown requested acquisition target")
    );

    let mut malformed_root = fixture.request("registry.acquire");
    malformed_root["payload_source_root"] = json!({"path":fixture.payload_root});
    assert!(
        invoke(&malformed_root)
            .unwrap_err()
            .contains("payload_source_root must be a string")
    );

    assert_eq!(tree_snapshot(&fixture.root), repository_before);
    assert_eq!(tree_snapshot(&fixture.payload_root), payloads_before);
}

#[test]
fn registry_transfer_keeps_exact_pinned_bytes_and_a_completed_receipt() {
    let fixture = Fixture::new();
    let target = json!({
        "slug":"fixture-sutta",
        "paths":{"item_root":"ToS/source-witnesses/works/pali-canon/fixture-sutta/items/root"}
    });
    let item_root = target["paths"]["item_root"].as_str().unwrap();
    let basename = "fixture_root-pli-ms.json";
    let body = b"{\"fixture:1\":\"Exact fixture source text\"}\n".to_vec();
    let mut blob = sha1::Sha1::new();
    blob.update(format!("blob {}\0", body.len()).as_bytes());
    blob.update(&body);
    let git_blob_sha1 = format!("{:x}", blob.finalize());
    let url = "https://raw.githubusercontent.com/suttacentral/bilara-data/d6d54741b7f2ddfeca82f02c3f95eb3990b4e351/root/pli/ms/fixture_root-pli-ms.json";
    let entry = json!({
        "upstream_path":format!("root/pli/ms/{basename}"),
        "basename":basename,
        "byte_size":body.len(),
        "git_blob_sha1":git_blob_sha1,
        "media_type":"application/json",
        "url":url
    });
    let relative = format!("{item_root}/payload/{basename}");
    fs::write(fixture.root.join(".gitignore"), format!("/{relative}\n")).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&fixture.root)
            .status()
            .unwrap()
            .success()
    );

    let expected_body = body.clone();
    let expected_blob = entry["git_blob_sha1"].as_str().unwrap().to_owned();
    TEST_FETCH_CALLBACK.with(|callback| {
        *callback.borrow_mut() = Some(Box::new(move |request| {
            assert_eq!(request["url"], url);
            assert_eq!(request["target_slug"], "fixture-sutta");
            assert_eq!(request["expected_byte_size"], expected_body.len());
            assert_eq!(
                request["expected_git_blob_sha1"].as_str(),
                Some(expected_blob.as_str())
            );
            Ok(expected_body.clone())
        }));
    });

    let log = fixture
        .root
        .join("ToS/source-witnesses/discovery/fixture/acquisition-transfers.jsonl");
    let (retained, receipt) = transfer(
        &fixture.root,
        &target,
        &entry,
        &log,
        Some(&fixture.payload_root),
        true,
    )
    .unwrap();
    TEST_FETCH_CALLBACK.with(|callback| *callback.borrow_mut() = None);

    assert_eq!(retained, body);
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["http_status"], 200);
    assert_eq!(receipt["final_url"], url);
    assert_eq!(receipt["sha256"], sha256(&retained));
    assert_eq!(receipt["expected_git_blob_sha1"], entry["git_blob_sha1"]);
    let payload = fixture
        .payload_root
        .join(item_root.strip_prefix("ToS/source-witnesses/").unwrap())
        .join("payload")
        .join(basename);
    assert_eq!(fs::read(&payload).unwrap(), retained);
    assert_eq!(
        fs::metadata(&payload).unwrap().permissions().mode() & 0o777,
        0o444
    );
    let rows = fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| strict_json(line.as_bytes()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["status"], "started");
    assert_eq!(rows[1], receipt);
    assert!(!fixture.root.join(&relative).exists());
    assert!(
        !fixture
            .root
            .join(format!("{item_root}/item.manifest.json"))
            .exists()
    );
}
