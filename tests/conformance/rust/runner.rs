//! Independent, small conformance vectors for the Rust migration foundation.
//! OPS registers this as an integration test with serde_json and tempfile.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde_json::Value;
use tar::Archive;
use tempfile::TempDir;
use tos_foundation::{
    CanonicalProfile, CodePointSpan, Digest256, JsonEmissionProfile, JsonLimits, JsonMode,
    JsonNumberKind, JsonValue, RelativePath, SourceRevision, canonical_bytes_v1,
    canonical_raw_bytes_v1, emit_json_profile, emit_preserved_json, parse_json,
};
use tos_source_store::{CorpusReader, ReadLimits, Selector, StoreErrorCode};

// Frozen outputs from bounded, owner-run legacy Python conformance oracles.
// The adjacent provenance records pin their exact source and selected-input
// hashes; native tests never regenerate these expected values.
pub(crate) fn frozen_legacy_python_oracle_provenance(id: &str) -> Value {
    let known = match id {
        "assessment-whole-view"
        | "catalog-bibliographic"
        | "claim-creation"
        | "claim-revision-catalog-history"
        | "claim-revision-identity-history-v1"
        | "claim-revision-identity-history-v2"
        | "claim-revision-initial"
        | "claim-revision-selected-history"
        | "compound-source-family"
        | "corpus-composition"
        | "corpus-query-cases"
        | "object-link-artifact"
        | "object-link-work"
        | "repository-topology"
        | "whole-authored-philosophy-graph"
        | "whole-authored-philosophy-summary" => id,
        _ => panic!("unknown frozen legacy Python oracle {id}"),
    };
    let directory = fixtures().join("legacy-python-oracles-v1");
    let provenance: Value = serde_json::from_slice(
        &fs::read(directory.join(format!("{known}.provenance.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        provenance["schema_version"],
        "tos_legacy_python_oracle_capture_v1"
    );
    assert_eq!(provenance["oracle_id"], known);
    assert!(
        provenance["repository_head"]
            .as_str()
            .is_some_and(|value| value.len() == 40)
    );
    assert!(provenance["input_witness"].is_object());
    let sources = provenance["owner_sources"].as_array().unwrap();
    assert!(
        !sources.is_empty(),
        "frozen oracle pins historical owner source hashes"
    );
    assert!(sources.iter().all(|source| {
        source["path"].is_string()
            && source["bytes"].as_u64().is_some()
            && source["sha256"]
                .as_str()
                .is_some_and(|digest| digest.len() == 64)
    }));
    provenance
}

pub(crate) fn frozen_legacy_python_oracle_raw(id: &str) -> Vec<u8> {
    let directory = fixtures().join("legacy-python-oracles-v1");
    let output = fs::read(directory.join(format!("{id}.expected.json"))).unwrap();
    let provenance = frozen_legacy_python_oracle_provenance(id);
    assert_eq!(
        provenance["output_bytes"].as_u64(),
        Some(output.len() as u64)
    );
    let digest = Digest256::of_bytes(&output).to_hex();
    assert_eq!(provenance["output_sha256"].as_str(), Some(digest.as_str()));
    output
}

pub(crate) fn frozen_legacy_python_oracle(id: &str) -> Value {
    serde_json::from_slice(&frozen_legacy_python_oracle_raw(id)).unwrap()
}

// normal conformance lane never writes; source and input hashes make each
// captured historical result independently reviewable.
fn fixtures() -> PathBuf {
    if std::env::var("TOS_NATIVE_INSTALLED_SOFTWARE_SITE").as_deref() == Ok("1") {
        let source = PathBuf::from(
            std::env::var_os("TOS_NATIVE_SOFTWARE_SOURCE_ROOT")
                .expect("installed cohort requires its exact admitted fixture source"),
        );
        assert!(source.is_absolute());
        return source.join("tests/conformance/rust");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// A one-time capture preserves the complete output of a maintained Python
// fixture producer. Native tests can replay that fixture without importing or
// executing its historical producer; only root-relative JSON path values
// listed in the capture manifest are rebased into the new isolated directory.
#[derive(Clone, Debug)]
pub(crate) struct CapturedFixtureFile {
    pub bytes: u64,
    pub mode: u32,
    pub sha256: String,
    pub source_bytes: u64,
    pub source_sha256: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CapturedPythonSource {
    pub module: String,
    pub root: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub(crate) struct NativeFixtureCaptureIdentity {
    pub repository_head: String,
    pub repository_tree: String,
    pub manifest_sha256: String,
    pub source_manifest_sha256: String,
    pub conformance_sha256: String,
    pub owner_sha256: String,
    pub python_sha256: String,
    pub factory_source_path: String,
    pub factory_source_symbol: String,
    pub factory_source_sha256: String,
    pub factory_script_sha256: String,
    pub packet_sha256: String,
    pub source_packet_sha256: String,
}

#[derive(Clone, Debug)]
pub(crate) struct SoftwareIdentityMigration {
    pub historical_python_sources: Vec<CapturedPythonSource>,
    pub native_owner_paths: Vec<String>,
}

pub(crate) struct NativePythonFixture {
    pub root: PathBuf,
    pub owner: PathBuf,
    pub roots: BTreeMap<String, PathBuf>,
    pub packets: BTreeMap<String, Value>,
    pub raw_packet: Vec<u8>,
    pub files: BTreeMap<String, CapturedFixtureFile>,
    pub capture_identity: NativeFixtureCaptureIdentity,
    pub software_identity_migration: SoftwareIdentityMigration,
    _capture_temp: TempDir,
}

#[derive(Clone)]
enum FixtureArchiveMember {
    Directory {
        mode: u32,
        output: PathBuf,
    },
    File {
        bytes: u64,
        mode: Option<u32>,
        sha256: String,
        output: PathBuf,
    },
}

struct BoundedReader<R> {
    inner: R,
    read: u64,
    maximum: u64,
}

impl<R> BoundedReader<R> {
    fn new(inner: R, maximum: u64) -> Self {
        Self {
            inner,
            read: 0,
            maximum,
        }
    }
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let remaining = self.maximum.saturating_sub(self.read);
        if remaining == 0 {
            let mut extra = [0u8; 1];
            return match self.inner.read(&mut extra)? {
                0 => Ok(0),
                _ => Err(io::Error::other(
                    "fixture archive exceeds expanded-size cap",
                )),
            };
        }
        let allowed = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
        let count = self.inner.read(&mut buffer[..allowed])?;
        self.read += count as u64;
        Ok(count)
    }
}

fn native_fixture_bundle_root() -> PathBuf {
    std::env::var_os("TOS_NATIVE_PYTHON_FIXTURE_CAPTURE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| fixtures().join("fixtures/native-python-fixture-capture-v1"))
}

fn native_fixture_capture_root(id: &str) -> PathBuf {
    let bundle = native_fixture_bundle_root();
    if id == "public-text-base" {
        bundle.join("public-text-supplement")
    } else {
        bundle.join("base")
    }
}

fn capture_relative_path(value: &str) -> PathBuf {
    assert!(
        !value.is_empty() && !Path::new(value).is_absolute(),
        "capture path is relative"
    );
    assert!(
        value.len() <= 512 && Path::new(value).components().count() <= 32,
        "capture path is within the producer's declared path cap"
    );
    let path = RelativePath::parse(value)
        .unwrap_or_else(|error| panic!("unsafe capture path {value}: {error}"));
    PathBuf::from(path.as_str())
}

fn capture_mode(value: &Value) -> u32 {
    let mode = value.as_u64().expect("captured mode is an integer");
    assert!(mode <= 0o7777, "captured mode has unsupported bits");
    mode as u32
}

fn capture_pointer_tokens(pointer: &str) -> Vec<String> {
    if pointer == "/" {
        return Vec::new();
    }
    assert!(pointer.starts_with('/'), "capture JSON pointer is absolute");
    pointer[1..]
        .split('/')
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn capture_value_at_pointer<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    let mut current = value;
    for token in capture_pointer_tokens(pointer) {
        current = match current {
            Value::Object(object) => object
                .get(&token)
                .unwrap_or_else(|| panic!("missing capture pointer {pointer}")),
            Value::Array(array) => array
                .get(token.parse::<usize>().unwrap())
                .unwrap_or_else(|| panic!("missing capture pointer {pointer}")),
            _ => panic!("capture pointer crosses a scalar: {pointer}"),
        };
    }
    current
}

fn capture_scan_tree(
    root: &Path,
    relative: &Path,
    directories: &mut BTreeSet<String>,
    files: &mut BTreeSet<String>,
) {
    let current = if relative.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };
    for entry in fs::read_dir(&current)
        .unwrap_or_else(|error| panic!("scan captured tree {}: {error}", current.display()))
    {
        let entry = entry.unwrap();
        let name = entry
            .file_name()
            .into_string()
            .expect("captured tree path is UTF-8");
        let child_relative = if relative.as_os_str().is_empty() {
            PathBuf::from(&name)
        } else {
            relative.join(&name)
        };
        let child = root.join(&child_relative);
        let metadata = fs::symlink_metadata(&child).unwrap();
        let key = child_relative
            .to_str()
            .expect("captured relative path is UTF-8")
            .to_owned();
        if metadata.file_type().is_symlink() {
            panic!(
                "captured fixture tree contains a symlink: {}",
                child.display()
            );
        } else if metadata.is_dir() {
            assert!(directories.insert(key), "duplicate captured directory");
            capture_scan_tree(root, &child_relative, directories, files);
        } else {
            assert!(metadata.is_file(), "captured tree contains a non-file");
            assert!(files.insert(key), "duplicate captured file");
        }
    }
}

fn capture_value_at_pointer_mut<'a>(value: &'a mut Value, pointer: &str) -> &'a mut Value {
    let mut current = value;
    for token in capture_pointer_tokens(pointer) {
        current = match current {
            Value::Object(object) => object
                .get_mut(&token)
                .unwrap_or_else(|| panic!("missing capture pointer {pointer}")),
            Value::Array(array) => array
                .get_mut(token.parse::<usize>().unwrap())
                .unwrap_or_else(|| panic!("missing capture pointer {pointer}")),
            _ => panic!("capture pointer crosses a scalar: {pointer}"),
        };
    }
    current
}

fn capture_replace_object_key(value: &mut Value, pointer: &str, source: &str, target: &str) {
    let object = capture_value_at_pointer_mut(value, pointer)
        .as_object_mut()
        .unwrap_or_else(|| panic!("capture JSON-key pointer is not an object: {pointer}"));
    let child = object
        .remove(source)
        .unwrap_or_else(|| panic!("missing captured JSON object key {source}"));
    assert!(
        !object.contains_key(target),
        "captured JSON object key target already exists"
    );
    object.insert(target.to_owned(), child);
}

fn capture_object_has_key(value: &Value, pointer: &str, key: &str) -> bool {
    capture_value_at_pointer(value, pointer)
        .as_object()
        .is_some_and(|object| object.contains_key(key))
}

fn capture_root_relative(captured_root: &str, captured_value: &str, suffix: &str) -> bool {
    let prefix = captured_root.trim_end_matches('/');
    let expected = if suffix.is_empty() || suffix == "." {
        prefix.to_owned()
    } else {
        format!("{prefix}/{suffix}")
    };
    captured_value == expected
}

fn capture_replacement(root: &Path, suffix: &str) -> PathBuf {
    if suffix.is_empty() || suffix == "." {
        root.to_path_buf()
    } else {
        root.join(capture_relative_path(suffix))
    }
}

fn capture_archive_key(value: &str) -> String {
    let path = Path::new(value);
    assert!(
        !value.is_empty() && value.len() <= 512 && !value.contains('\\') && !value.contains('\0')
    );
    assert!(path.is_relative() && path.components().count() <= 40);
    assert!(
        path.components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    );
    path.to_str()
        .expect("captured archive path is UTF-8")
        .to_owned()
}

fn insert_archive_member(
    expected: &mut BTreeMap<String, FixtureArchiveMember>,
    path: String,
    member: FixtureArchiveMember,
) {
    assert!(
        expected.insert(path.clone(), member).is_none(),
        "duplicate captured archive path: {path}"
    );
}

fn unpack_native_fixture_archive(
    bundle_root: &Path,
    id: &str,
    entry: &Value,
    archive_row: &Value,
    caps: &Value,
) -> TempDir {
    assert_eq!(
        required(archive_row, "path"),
        format!("archives/{id}.tar.gz")
    );
    let archive_path = bundle_root.join(capture_relative_path(required(archive_row, "path")));
    let archive_metadata = fs::symlink_metadata(&archive_path).unwrap_or_else(|error| {
        panic!(
            "inspect captured fixture archive {}: {error}",
            archive_path.display()
        )
    });
    assert!(archive_metadata.is_file() && !archive_metadata.file_type().is_symlink());
    const MAX_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;
    assert!(
        archive_metadata.len() <= MAX_ARCHIVE_BYTES,
        "fixture archive exceeds compressed-size cap"
    );
    let archive_raw = fs::read(&archive_path).unwrap();
    assert!(archive_raw.len() as u64 <= MAX_ARCHIVE_BYTES);
    assert_eq!(
        archive_row["bytes"].as_u64(),
        Some(archive_raw.len() as u64),
        "captured archive byte count changed"
    );
    assert_eq!(
        required(archive_row, "sha256"),
        Digest256::of_bytes(&archive_raw).to_hex(),
        "captured archive source hash changed"
    );

    let packet_bytes = entry["packet_bytes"]
        .as_u64()
        .expect("captured packet byte count");
    assert!(packet_bytes <= caps["max_packet_bytes"].as_u64().unwrap());
    let mut expected = BTreeMap::<String, FixtureArchiveMember>::new();
    insert_archive_member(
        &mut expected,
        "packet.json".to_owned(),
        FixtureArchiveMember::File {
            bytes: packet_bytes,
            mode: None,
            sha256: required(entry, "packet_sha256").to_owned(),
            output: capture_relative_path(required(entry, "packet_path")),
        },
    );
    let roots = entry["roots"].as_array().expect("captured fixture roots");
    for (root_index, root) in roots.iter().enumerate() {
        let stored = capture_relative_path(required(root, "stored_path"));
        let archive_root = format!("roots/{root_index}");
        let directories = root["directories"]
            .as_array()
            .expect("captured directories");
        for directory in directories {
            let relative = required(directory, "path");
            let path = if relative == "." {
                archive_root.clone()
            } else {
                format!("{archive_root}/{}", capture_archive_key(relative))
            };
            let output = if relative == "." {
                stored.clone()
            } else {
                stored.join(capture_relative_path(relative))
            };
            insert_archive_member(
                &mut expected,
                path,
                FixtureArchiveMember::Directory {
                    mode: capture_mode(&directory["mode"]),
                    output,
                },
            );
        }
        for file in root["files"].as_array().expect("captured fixture files") {
            let relative = required(file, "path");
            let path = format!("{archive_root}/{}", capture_archive_key(relative));
            let bytes = file["bytes"].as_u64().expect("captured file byte count");
            assert!(bytes <= caps["max_file_bytes"].as_u64().unwrap());
            insert_archive_member(
                &mut expected,
                path,
                FixtureArchiveMember::File {
                    bytes,
                    mode: Some(capture_mode(&file["mode"])),
                    sha256: required(file, "sha256").to_owned(),
                    output: stored.join(capture_relative_path(relative)),
                },
            );
        }
    }
    assert!(
        !expected.is_empty() && expected.len() <= 32_768,
        "fixture archive member count is bounded"
    );
    let tree_bytes = roots
        .iter()
        .try_fold(0u64, |total, root| {
            total.checked_add(root["tree_bytes"].as_u64()?)
        })
        .expect("fixture archive tree byte count overflow");
    let expected_file_bytes = packet_bytes
        .checked_add(tree_bytes)
        .expect("fixture archive byte count overflow");
    let archive_expanded_cap = expected_file_bytes
        .checked_add((expected.len() as u64).checked_mul(1024).unwrap())
        .and_then(|bytes| bytes.checked_add(10_240))
        .expect("fixture archive expanded cap overflow");

    let destination = tempfile::Builder::new()
        .prefix("tos-native-python-fixture-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let mut seen = BTreeSet::new();
    let mut directories = Vec::new();
    let mut total_file_bytes = 0u64;
    {
        let decoder = GzDecoder::new(std::io::Cursor::new(archive_raw));
        let bounded = BoundedReader::new(decoder, archive_expanded_cap);
        let mut archive = Archive::new(bounded);
        let mut entry_count = 0usize;
        for result in archive.entries().unwrap() {
            let mut member = result.unwrap();
            entry_count = entry_count.checked_add(1).unwrap();
            assert!(entry_count <= expected.len());
            let relative = member.path().unwrap().into_owned();
            let relative = relative
                .to_str()
                .expect("captured archive member path is UTF-8");
            // POSIX tar directory headers conventionally end in one slash.
            // Their manifest identity is the same exact relative directory.
            let relative = if member.header().entry_type().is_dir() {
                relative.strip_suffix('/').unwrap_or(relative)
            } else {
                relative
            };
            let relative = capture_archive_key(relative);
            let expected_member = expected
                .get(&relative)
                .unwrap_or_else(|| panic!("unmanifested captured archive member {relative}"));
            assert!(
                seen.insert(relative.clone()),
                "duplicate captured archive member {relative}"
            );
            let mode = member.header().mode().unwrap();
            assert!(
                mode <= 0o777,
                "captured archive member has unsupported mode bits"
            );
            match expected_member {
                FixtureArchiveMember::Directory {
                    mode: expected_mode,
                    output: output_relative,
                } => {
                    assert!(member.header().entry_type().is_dir());
                    assert_eq!(member.header().size().unwrap(), 0);
                    assert_eq!(mode, *expected_mode);
                    let output = destination.path().join(output_relative);
                    fs::create_dir_all(&output).unwrap();
                    directories.push((output, mode));
                }
                FixtureArchiveMember::File {
                    bytes,
                    mode: expected_mode,
                    sha256,
                    output: output_relative,
                } => {
                    assert!(member.header().entry_type().is_file());
                    assert_eq!(member.header().size().unwrap(), *bytes);
                    if let Some(expected_mode) = expected_mode {
                        assert_eq!(mode, *expected_mode);
                    }
                    total_file_bytes = total_file_bytes.checked_add(*bytes).unwrap();
                    assert!(
                        total_file_bytes <= expected_file_bytes,
                        "captured archive payload exceeds manifest bytes"
                    );
                    let mut raw = Vec::with_capacity(usize::try_from(*bytes).unwrap());
                    member.read_to_end(&mut raw).unwrap();
                    assert_eq!(raw.len() as u64, *bytes);
                    assert_eq!(
                        Digest256::of_bytes(&raw).to_hex(),
                        *sha256,
                        "captured archive file hash differs: {relative}"
                    );
                    let output = destination.path().join(output_relative);
                    fs::create_dir_all(output.parent().unwrap()).unwrap();
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&output)
                        .unwrap();
                    file.write_all(&raw).unwrap();
                    drop(file);
                    fs::set_permissions(&output, fs::Permissions::from_mode(mode)).unwrap();
                }
            }
        }
        let mut bounded = archive.into_inner();
        io::copy(&mut bounded, &mut io::sink()).unwrap();
    }
    assert_eq!(
        seen.len(),
        expected.len(),
        "captured archive member set changed"
    );
    assert_eq!(
        total_file_bytes, expected_file_bytes,
        "captured archive byte total changed"
    );
    for (directory, mode) in directories.into_iter().rev() {
        fs::set_permissions(directory, fs::Permissions::from_mode(mode)).unwrap();
    }
    destination
}

/// Verifies and materializes one captured fixture. `destinations` maps each
/// manifest root name to its caller-owned isolated directory. The caller must
/// declare the native Rust owner paths that replace the captured Python owner.
pub(crate) fn native_python_fixture(
    id: &str,
    destinations: &[(&str, &Path)],
    native_owner_paths: &[&str],
) -> NativePythonFixture {
    assert!(
        !id.is_empty()
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    );
    let bundle_root = native_fixture_capture_root(id);
    let manifest_path = bundle_root.join("manifest.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).unwrap_or_else(|error| {
        panic!(
            "inspect fixture manifest {}: {error}",
            manifest_path.display()
        )
    });
    assert!(manifest_metadata.is_file() && !manifest_metadata.file_type().is_symlink());
    assert!(
        manifest_metadata.len() <= 16 * 1024 * 1024,
        "fixture manifest exceeds cap"
    );
    let manifest_raw = fs::read(&manifest_path).unwrap_or_else(|error| {
        panic!("read fixture manifest {}: {error}", manifest_path.display())
    });
    assert!(
        manifest_raw.len() <= 16 * 1024 * 1024,
        "fixture manifest exceeds cap"
    );
    let manifest: Value = serde_json::from_slice(&manifest_raw).unwrap();
    assert_eq!(
        manifest["schema_version"],
        "tos_native_python_fixture_capture_v1"
    );
    assert_eq!(
        manifest["status"], "passed",
        "fixture capture must have passed"
    );
    let manifest_sha256 = Digest256::of_bytes(&manifest_raw).to_hex();
    let source = &manifest["source"];
    let products = &manifest["products"];
    let caps = &manifest["caps"];
    if id == "public-text-base" {
        let primary_path = native_fixture_bundle_root().join("base/manifest.json");
        let primary_metadata = fs::symlink_metadata(&primary_path).unwrap();
        assert!(primary_metadata.is_file() && !primary_metadata.file_type().is_symlink());
        assert!(primary_metadata.len() <= 16 * 1024 * 1024);
        let primary_raw = fs::read(primary_path).unwrap();
        let primary: Value = serde_json::from_slice(&primary_raw).unwrap();
        assert_eq!(
            primary["schema_version"],
            "tos_native_python_fixture_capture_v1"
        );
        assert_eq!(primary["status"], "passed");
        assert_eq!(
            primary["source"]["repository_head"],
            source["repository_head"]
        );
        assert_eq!(
            primary["source"]["repository_tree"],
            source["repository_tree"]
        );
        for product in ["conformance", "owner", "python"] {
            assert_eq!(
                primary["products"][product]["sha256"], products[product]["sha256"],
                "supplemental fixture product identity matches R2"
            );
        }
    }
    assert!(
        manifest["aggregate_tree_bytes"].as_u64().unwrap()
            <= caps["max_total_tree_bytes"].as_u64().unwrap()
    );
    let entry = manifest["captured_factories"]
        .as_array()
        .expect("captured factory entries")
        .iter()
        .find(|entry| entry["fixture_id"] == id)
        .unwrap_or_else(|| panic!("unknown captured native fixture {id}"));
    assert_eq!(
        entry["exit_code"], 0,
        "captured fixture producer must succeed"
    );
    let archive_row = manifest["archives"]
        .get(id)
        .unwrap_or_else(|| panic!("captured archive identity is missing for {id}"));
    let capture_temp = unpack_native_fixture_archive(&bundle_root, id, entry, archive_row, caps);
    let capture_root = capture_temp.path().to_path_buf();

    let packet_path = required(entry, "packet_path");
    let packet_path_relative = capture_relative_path(packet_path);
    assert!(
        entry["packet_bytes"].as_u64().unwrap() <= caps["max_packet_bytes"].as_u64().unwrap(),
        "captured packet exceeds declared cap"
    );
    let stored_packet = capture_root.join(&packet_path_relative);
    let packet_metadata = fs::symlink_metadata(&stored_packet).unwrap_or_else(|error| {
        panic!(
            "inspect captured packet {}: {error}",
            stored_packet.display()
        )
    });
    assert!(packet_metadata.is_file() && !packet_metadata.file_type().is_symlink());
    assert!(packet_metadata.len() <= caps["max_packet_bytes"].as_u64().unwrap());
    let raw_packet = fs::read(&stored_packet).unwrap();
    assert_eq!(
        entry["packet_bytes"].as_u64(),
        Some(raw_packet.len() as u64)
    );
    let packet_sha256 = Digest256::of_bytes(&raw_packet).to_hex();
    assert_eq!(
        entry["packet_sha256"].as_str(),
        Some(packet_sha256.as_str())
    );
    let mut packet: Value = serde_json::from_slice(&raw_packet).unwrap();

    let captured_roots = entry["roots"].as_array().expect("captured fixture roots");
    assert_eq!(
        captured_roots.len(),
        destinations.len(),
        "destination roots match the complete captured root set"
    );
    let mut root_paths = BTreeMap::<String, PathBuf>::new();
    let mut root_sources = BTreeMap::<String, String>::new();
    let mut root_rows = BTreeMap::<String, &Value>::new();
    let mut destination_paths = BTreeSet::new();
    for row in captured_roots {
        let name = required(row, "name").to_owned();
        assert!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        );
        assert!(
            !root_paths.contains_key(&name),
            "duplicate captured root name"
        );
        let (_, target) = destinations
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .unwrap_or_else(|| panic!("destination missing for captured root {name}"));
        assert!(target.is_absolute(), "fixture destination is absolute");
        assert!(
            destination_paths.insert(target.to_path_buf()),
            "duplicate fixture destination"
        );
        assert!(
            *target != capture_root.as_path(),
            "fixture destination cannot be the capture store"
        );
        if target.exists() {
            let metadata = fs::symlink_metadata(target).unwrap();
            assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
            assert!(
                fs::read_dir(target).unwrap().next().is_none(),
                "fixture destination must be empty"
            );
        } else {
            fs::create_dir_all(target).unwrap();
        }
        fs::set_permissions(target, fs::Permissions::from_mode(0o700)).unwrap();
        root_paths.insert(name.clone(), target.to_path_buf());
        root_sources.insert(name.clone(), required(row, "captured_path").to_owned());
        root_rows.insert(name, row);
    }
    assert_eq!(
        root_paths.len(),
        destinations.len(),
        "every declared destination has a captured root"
    );
    if id == "public-text-base" {
        assert_eq!(root_paths.len(), 1);
        assert!(
            root_paths.contains_key("workspace"),
            "public Text supplement captures its complete isolated workspace"
        );
    }
    for (index, path) in destination_paths.iter().enumerate() {
        for other in destination_paths.iter().skip(index + 1) {
            assert!(
                !path.starts_with(other) && !other.starts_with(path),
                "fixture destinations cannot overlap"
            );
        }
    }

    let mut files = BTreeMap::<String, CapturedFixtureFile>::new();
    for (name, row) in &root_rows {
        let stored = capture_root.join(capture_relative_path(required(row, "stored_path")));
        let stored_metadata = fs::symlink_metadata(&stored)
            .unwrap_or_else(|error| panic!("inspect captured root {}: {error}", stored.display()));
        assert!(
            stored_metadata.is_dir() && !stored_metadata.file_type().is_symlink(),
            "captured root is a real directory"
        );
        let root_mode = capture_mode(&row["root_mode"]);
        assert!(
            row["tree_bytes"].as_u64().unwrap() <= caps["max_tree_bytes"].as_u64().unwrap(),
            "captured root exceeds declared byte cap"
        );
        assert!(
            row["file_count"].as_u64().unwrap() <= caps["max_files_per_root"].as_u64().unwrap(),
            "captured root exceeds declared file cap"
        );
        let directories = row["directories"]
            .as_array()
            .expect("captured directory modes");
        assert!(!directories.is_empty());
        assert_eq!(
            directories.len() as u64,
            row["directory_count"].as_u64().unwrap() + 1
        );
        let root_directory = directories
            .iter()
            .find(|directory| directory["path"] == ".")
            .unwrap();
        assert_eq!(capture_mode(&root_directory["mode"]), root_mode);
        assert_eq!(
            stored_metadata.permissions().mode() & 0o7777,
            root_mode,
            "captured root mode changed"
        );
        let target_root = &root_paths[name];
        let destination_mode = fs::metadata(target_root).unwrap().permissions().mode() & 0o7777;
        assert!(
            destination_mode & 0o022 == 0,
            "fixture destination is not private"
        );

        let mut made_directories = Vec::new();
        let mut expected_directories = BTreeSet::new();
        for directory in directories {
            let relative = required(directory, "path");
            let target = if relative == "." {
                target_root.to_path_buf()
            } else {
                target_root.join(capture_relative_path(relative))
            };
            if relative != "." {
                fs::create_dir_all(&target).unwrap();
                assert!(
                    !fs::symlink_metadata(&target)
                        .unwrap()
                        .file_type()
                        .is_symlink()
                );
                let relative_path = capture_relative_path(relative);
                let stored_directory = stored.join(&relative_path);
                let stored_meta = fs::symlink_metadata(&stored_directory).unwrap();
                assert!(stored_meta.is_dir() && !stored_meta.file_type().is_symlink());
                assert_eq!(
                    stored_meta.permissions().mode() & 0o7777,
                    capture_mode(&directory["mode"]),
                    "captured directory mode changed"
                );
                assert!(expected_directories.insert(relative_path.to_string_lossy().into_owned()));
            }
            made_directories.push((target, capture_mode(&directory["mode"])));
        }

        let mut root_bytes = 0u64;
        let mut root_file_count = 0usize;
        let mut expected_files = BTreeSet::new();
        for file in row["files"].as_array().expect("captured file rows") {
            let relative = required(file, "path");
            let relative_path = capture_relative_path(relative);
            let stored_file = stored.join(&relative_path);
            let stored_metadata = fs::symlink_metadata(&stored_file).unwrap_or_else(|error| {
                panic!("inspect captured file {}: {error}", stored_file.display())
            });
            assert!(
                stored_metadata.is_file() && !stored_metadata.file_type().is_symlink(),
                "captured fixture file is regular"
            );
            assert!(
                stored_metadata.len() <= caps["max_file_bytes"].as_u64().unwrap(),
                "captured file exceeds declared cap"
            );
            let raw = fs::read(&stored_file).unwrap_or_else(|error| {
                panic!("read captured file {}: {error}", stored_file.display())
            });
            let mode = capture_mode(&file["mode"]);
            assert_eq!(
                stored_metadata.permissions().mode() & 0o7777,
                mode,
                "captured file mode changed"
            );
            assert!(
                raw.len() as u64 <= caps["max_file_bytes"].as_u64().unwrap(),
                "captured file exceeds declared cap"
            );
            let sha256 = Digest256::of_bytes(&raw).to_hex();
            assert_eq!(file["bytes"].as_u64(), Some(raw.len() as u64));
            assert_eq!(file["sha256"].as_str(), Some(sha256.as_str()));
            let target = target_root.join(&relative_path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&target, &raw).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(mode)).unwrap();
            let key = format!("{name}/{relative}");
            let source_bytes = file["source_bytes"]
                .as_u64()
                .unwrap_or_else(|| file["bytes"].as_u64().unwrap());
            let source_sha256 = file
                .get("source_sha256")
                .and_then(Value::as_str)
                .unwrap_or_else(|| file["sha256"].as_str().unwrap())
                .to_owned();
            assert!(
                files
                    .insert(
                        key,
                        CapturedFixtureFile {
                            bytes: raw.len() as u64,
                            mode,
                            sha256,
                            source_bytes,
                            source_sha256
                        }
                    )
                    .is_none()
            );
            assert!(
                expected_files.insert(relative_path.to_string_lossy().into_owned()),
                "duplicate captured file path"
            );
            root_bytes += raw.len() as u64;
            root_file_count += 1;
        }
        assert_eq!(row["tree_bytes"].as_u64(), Some(root_bytes));
        assert_eq!(row["file_count"].as_u64(), Some(root_file_count as u64));
        assert!(
            root_bytes <= caps["max_tree_bytes"].as_u64().unwrap(),
            "captured root exceeds declared cap"
        );
        assert!(
            root_file_count as u64 <= caps["max_files_per_root"].as_u64().unwrap(),
            "captured root exceeds declared file-count cap"
        );
        let mut actual_directories = BTreeSet::new();
        let mut actual_files = BTreeSet::new();
        capture_scan_tree(
            &stored,
            Path::new(""),
            &mut actual_directories,
            &mut actual_files,
        );
        assert_eq!(
            actual_directories, expected_directories,
            "captured directory set differs from manifest"
        );
        assert_eq!(
            actual_files, expected_files,
            "captured file set differs from manifest"
        );
        for (directory, mode) in made_directories.into_iter().rev() {
            fs::set_permissions(directory, fs::Permissions::from_mode(mode)).unwrap();
        }
    }

    let relocations = entry["relocations"]
        .as_array()
        .expect("declared root relocations");
    let mut file_relocations =
        BTreeMap::<(String, String, String, bool), Vec<(String, String)>>::new();
    for relocation in relocations {
        let operation = required(relocation, "operation");
        let artifact = required(relocation, "artifact");
        let pointer = required(relocation, "json_pointer");
        let root_name = required(relocation, "root_name");
        let captured_value = required(relocation, "captured_value");
        let suffix = required(relocation, "relative_suffix");
        let json_key = relocation["json_key"].as_bool().unwrap_or(false);
        let replacement = match operation {
            "replace_root_prefix" => {
                let target_root_name = relocation
                    .get("target_root_name")
                    .and_then(Value::as_str)
                    .unwrap_or(root_name);
                let captured_root = root_sources
                    .get(target_root_name)
                    .unwrap_or_else(|| panic!("unknown relocated root {target_root_name}"));
                assert!(
                    capture_root_relative(captured_root, captured_value, suffix),
                    "relocation value is outside its declared root"
                );
                let target_root = root_paths
                    .get(target_root_name)
                    .unwrap_or_else(|| panic!("missing relocated target root {target_root_name}"));
                capture_replacement(target_root, suffix)
                    .to_str()
                    .expect("fixture path is UTF-8")
                    .to_owned()
            }
            "normalize_environment_path" => {
                let prefix = format!("/captured/tos-native-fixture/{id}/environment/");
                assert!(
                    captured_value.starts_with(&prefix),
                    "environment path has a stable fixture label"
                );
                let source_sha = required(relocation, "source_captured_value_sha256");
                assert_eq!(source_sha.len(), 64);
                assert!(source_sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
                captured_value.to_owned()
            }
            other => panic!("unknown relocation operation {other}"),
        };
        match artifact {
            "packet" => {
                assert_eq!(required(relocation, "path"), packet_path);
                if json_key {
                    assert!(
                        capture_object_has_key(&packet, pointer, captured_value),
                        "packet object-key relocation source changed"
                    );
                    capture_replace_object_key(&mut packet, pointer, captured_value, &replacement);
                } else {
                    let target = capture_value_at_pointer_mut(&mut packet, pointer);
                    assert_eq!(
                        target.as_str(),
                        Some(captured_value),
                        "packet relocation source changed"
                    );
                    *target = Value::String(replacement);
                }
            }
            "file" => {
                let relative = required(relocation, "path");
                let relative_path = capture_relative_path(relative);
                let file_root_name = relocation
                    .get("file_root_name")
                    .and_then(Value::as_str)
                    .unwrap_or(root_name);
                let key = format!("{file_root_name}/{relative}");
                assert!(
                    files.contains_key(&key),
                    "relocated file is absent from captured root"
                );
                file_relocations
                    .entry((
                        file_root_name.to_owned(),
                        relative_path.to_string_lossy().into_owned(),
                        captured_value.to_owned(),
                        json_key,
                    ))
                    .or_default()
                    .push((pointer.to_owned(), replacement));
            }
            other => panic!("unknown relocation artifact {other}"),
        }
    }
    for ((root_name, relative, captured_value, json_key), edits) in file_relocations {
        assert!(
            edits
                .iter()
                .all(|(_, replacement)| replacement == &edits[0].1)
        );
        let target = root_paths[&root_name].join(capture_relative_path(&relative));
        let source = fs::read(&target).unwrap();
        let parsed: Value = serde_json::from_slice(&source).expect("relocated source file is JSON");
        for (pointer, _) in &edits {
            if json_key {
                assert!(
                    capture_object_has_key(&parsed, pointer, &captured_value),
                    "file object-key relocation source changed"
                );
            } else {
                assert_eq!(
                    capture_value_at_pointer(&parsed, pointer).as_str(),
                    Some(captured_value.as_str()),
                    "file relocation source changed"
                );
            }
        }
        let old_token = serde_json::to_vec(&captured_value).unwrap();
        let new_token = serde_json::to_vec(&edits[0].1).unwrap();
        let old_count = source
            .windows(old_token.len())
            .filter(|window| *window == old_token.as_slice())
            .count();
        assert_eq!(
            old_count,
            edits.len(),
            "file relocation pointer/token count differs"
        );
        // Apply the already-counted exact string token replacements without
        // reserializing the JSON object or changing its unrelated bytes.
        let mut updated = Vec::with_capacity(source.len());
        let mut cursor = 0usize;
        while cursor < source.len() {
            if source[cursor..].starts_with(&old_token) {
                updated.extend_from_slice(&new_token);
                cursor += old_token.len();
            } else {
                updated.push(source[cursor]);
                cursor += 1;
            }
        }
        let removed = old_token
            .len()
            .checked_mul(edits.len())
            .expect("relocation byte count overflow");
        let added = new_token
            .len()
            .checked_mul(edits.len())
            .expect("relocation byte count overflow");
        let expected_len = source
            .len()
            .checked_sub(removed)
            .and_then(|length| length.checked_add(added))
            .expect("relocation length overflow");
        assert_eq!(updated.len(), expected_len);
        let relocated: Value =
            serde_json::from_slice(&updated).expect("relocated JSON remains valid");
        for (pointer, replacement) in &edits {
            if json_key {
                assert!(
                    capture_object_has_key(&relocated, pointer, replacement),
                    "file object-key relocation target changed"
                );
            } else {
                assert_eq!(
                    capture_value_at_pointer(&relocated, pointer).as_str(),
                    Some(replacement.as_str()),
                    "file relocation target changed"
                );
            }
        }
        fs::write(&target, updated).unwrap();
        let mode = files[&format!("{root_name}/{relative}")].mode;
        fs::set_permissions(&target, fs::Permissions::from_mode(mode)).unwrap();
    }

    let owner = PathBuf::from(required(&packet, "owner"));
    assert!(owner.is_absolute(), "captured owner path is absolute");
    assert!(owner.is_file(), "relocated owner file exists");
    assert!(
        root_paths.values().any(|root| owner.starts_with(root)),
        "owner is inside one of the materialized fixture roots"
    );
    let root = root_paths
        .get("root")
        .or_else(|| root_paths.get("source-root"))
        .or_else(|| root_paths.get("fixture-root"))
        .or_else(|| root_paths.get("public-root"))
        .or_else(|| root_paths.get("journal-root"))
        .or_else(|| root_paths.get("workspace"))
        .or_else(|| root_paths.values().next())
        .unwrap()
        .to_path_buf();
    let packet_sha256 = packet_sha256;
    let factory_source = &entry["factory_source"];
    let capture_identity = NativeFixtureCaptureIdentity {
        repository_head: required(source, "repository_head").to_owned(),
        repository_tree: required(source, "repository_tree").to_owned(),
        manifest_sha256,
        source_manifest_sha256: required(&manifest, "capture_source_manifest_sha256").to_owned(),
        conformance_sha256: required(&products["conformance"], "sha256").to_owned(),
        owner_sha256: required(&products["owner"], "sha256").to_owned(),
        python_sha256: required(&products["python"], "sha256").to_owned(),
        factory_source_path: required(factory_source, "path").to_owned(),
        factory_source_symbol: required(factory_source, "symbol").to_owned(),
        factory_source_sha256: required(factory_source, "file_sha256").to_owned(),
        factory_script_sha256: required(entry, "factory_script_sha256").to_owned(),
        packet_sha256,
        source_packet_sha256: required(entry, "source_packet_sha256").to_owned(),
    };
    let historical_python_sources = entry["import_sources"]
        .as_array()
        .expect("captured imported Python source identities")
        .iter()
        .map(|source| CapturedPythonSource {
            module: required(source, "module").to_owned(),
            root: required(source, "root").to_owned(),
            path: required(source, "path").to_owned(),
            bytes: source["bytes"].as_u64().expect("Python source byte count"),
            sha256: required(source, "sha256").to_owned(),
        })
        .collect();
    let software_identity_migration = SoftwareIdentityMigration {
        historical_python_sources,
        native_owner_paths: native_owner_paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
    };
    NativePythonFixture {
        root,
        owner,
        roots: root_paths,
        packets: BTreeMap::from([("factory".to_owned(), packet)]),
        raw_packet,
        files,
        capture_identity,
        software_identity_migration,
        _capture_temp: capture_temp,
    }
}

pub(crate) fn assert_native_python_fixture(
    fixture: &NativePythonFixture,
    legacy_factory_script: &str,
    native_owner_paths: &[&str],
) {
    let identity = &fixture.capture_identity;
    assert_eq!(
        Digest256::of_bytes(legacy_factory_script.as_bytes()).to_hex(),
        identity.factory_script_sha256
    );
    assert_eq!(
        Digest256::of_bytes(&fixture.raw_packet).to_hex(),
        identity.packet_sha256
    );
    assert_eq!(identity.repository_head.len(), 40);
    assert_eq!(identity.repository_tree.len(), 40);
    for digest in [
        &identity.manifest_sha256,
        &identity.source_manifest_sha256,
        &identity.conformance_sha256,
        &identity.owner_sha256,
        &identity.python_sha256,
        &identity.factory_source_sha256,
        &identity.factory_script_sha256,
        &identity.packet_sha256,
        &identity.source_packet_sha256,
    ] {
        assert_eq!(digest.len(), 64, "captured identity uses a SHA-256 digest");
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert!(!identity.factory_source_path.is_empty());
    assert!(!identity.factory_source_symbol.is_empty());
    assert!(
        !fixture
            .software_identity_migration
            .historical_python_sources
            .is_empty()
    );
    let expected_native_owner_paths = native_owner_paths
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        fixture
            .software_identity_migration
            .native_owner_paths
            .as_slice(),
        expected_native_owner_paths.as_slice()
    );
    assert!(!fixture.roots.is_empty());
    assert!(
        fixture
            .roots
            .values()
            .any(|root| root.as_path() == fixture.root.as_path())
    );
    assert!(fixture.owner.is_absolute() && fixture.owner.is_file());
    assert!(
        fixture
            .roots
            .values()
            .any(|root| fixture.owner.starts_with(root))
    );
    assert!(fixture.packets.contains_key("factory"));
    assert!(!fixture.files.is_empty());
    for file in fixture.files.values() {
        assert!(file.bytes <= 16 * 1024 * 1024);
        assert_eq!(file.sha256.len(), 64);
        assert!(file.source_bytes <= 16 * 1024 * 1024);
        assert_eq!(file.source_sha256.len(), 64);
        assert!(file.mode <= 0o7777);
    }
    for source in &fixture
        .software_identity_migration
        .historical_python_sources
    {
        assert!(!source.module.is_empty() && !source.root.is_empty() && !source.path.is_empty());
        assert_eq!(source.sha256.len(), 64);
    }
}

fn lines(name: &str) -> Vec<Value> {
    fs::read_to_string(fixtures().join(name))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn required<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing {key}: {value}"))
}

fn decode_hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn source_bytes(case: &Value) -> Vec<u8> {
    match case.get("input_hex") {
        Some(hex) => decode_hex(hex.as_str().unwrap()),
        None => required(case, "input_utf8").as_bytes().to_vec(),
    }
}

#[test]
fn independent_foundation_vectors() {
    let cases = lines("foundation.jsonl");
    assert_eq!(cases.len(), 23, "a dropped vector is a contract change");
    for case in cases {
        let id = required(&case, "case_id");
        let expected = &case["expected"];
        match required(&case, "operation") {
            "relative_path" => {
                let actual = RelativePath::parse(required(&case, "input_utf8"));
                if let Some(code) = expected.get("reject") {
                    assert_eq!(
                        actual.unwrap_err().code.as_str(),
                        code.as_str().unwrap(),
                        "{id}"
                    );
                } else {
                    assert_eq!(
                        actual.unwrap().as_str(),
                        required(expected, "accept"),
                        "{id}"
                    );
                }
            }
            "span" => {
                let text = required(&case, "input_utf8");
                assert_eq!(
                    text.chars().count() as u64,
                    expected["code_points"].as_u64().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    text.len() as u64,
                    expected["utf8_bytes"].as_u64().unwrap(),
                    "{id}"
                );
                let range = expected["code_point_range"].as_array().unwrap();
                let span = CodePointSpan::new(
                    range[0].as_u64().unwrap(),
                    range[1].as_u64().unwrap(),
                    Digest256::of_bytes(text.as_bytes()),
                    "source-exact-v1",
                )
                .unwrap();
                let bytes = span.byte_span_in(text).unwrap();
                let result = expected["corresponding_byte_range"].as_array().unwrap();
                assert_eq!(
                    [bytes.start, bytes.end],
                    [result[0].as_u64().unwrap(), result[1].as_u64().unwrap()],
                    "{id}"
                );
                assert!(
                    span.byte_span_in("AéO").is_err(),
                    "{id}: representation digest must bind offsets"
                );
            }
            operation @ ("parse" | "parse_preserve" | "canonical") => {
                let mode = match case.get("mode").and_then(Value::as_str) {
                    Some("RequestLastWins") => JsonMode::RequestLastWins,
                    _ => JsonMode::PublishedStrict,
                };
                let parsed = parse_json(&source_bytes(&case), mode, JsonLimits::default());
                if operation != "canonical" && expected.get("reject").is_some() {
                    assert_eq!(
                        parsed.unwrap_err().code.as_str(),
                        required(expected, "reject"),
                        "{id}"
                    );
                    continue;
                }
                let parsed =
                    parsed.unwrap_or_else(|error| panic!("{id}: unexpected parse error {error}"));
                if operation == "canonical" {
                    assert_eq!(required(&case, "profile"), "CorpusSnapshotV1", "{id}");
                    let canonical = canonical_bytes_v1(
                        parsed.root(),
                        CanonicalProfile::CorpusSnapshotV1,
                        JsonLimits::default(),
                    );
                    if expected.get("reject").is_some() {
                        assert_eq!(
                            canonical.unwrap_err().code.as_str(),
                            required(expected, "reject"),
                            "{id}"
                        );
                    } else {
                        let bytes = canonical.unwrap();
                        assert_eq!(
                            bytes.as_slice(),
                            required(expected, "canonical_utf8").as_bytes(),
                            "{id}"
                        );
                        assert_eq!(
                            Digest256::of_bytes(&bytes).to_hex(),
                            required(expected, "digest_hex"),
                            "{id}"
                        );
                    }
                    continue;
                }
                if let Some(order) = expected.get("member_order") {
                    let observed: Vec<&str> = parsed
                        .root()
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(key, _)| key.as_str().unwrap())
                        .collect();
                    let wanted: Vec<&str> = order
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|key| key.as_str().unwrap())
                        .collect();
                    assert_eq!(observed, wanted, "{id}");
                }
                if let Some(numbers) = expected.get("numbers").and_then(Value::as_object) {
                    for (key, wanted) in numbers {
                        let JsonValue::Number(number) = parsed.root().object_get(key).unwrap()
                        else {
                            panic!("{id}: {key} lost numeric context");
                        };
                        let kind = match number.kind {
                            JsonNumberKind::Int => "Int",
                            JsonNumberKind::Float => "Float",
                        };
                        assert_eq!(kind, required(wanted, "kind"), "{id}/{key}");
                        assert_eq!(number.lexeme, required(wanted, "lexeme"), "{id}/{key}");
                    }
                }
                if let Some(wanted) = expected.get("preserved_utf8") {
                    let actual = emit_preserved_json(&parsed, JsonLimits::default()).unwrap();
                    assert_eq!(
                        actual.as_slice(),
                        wanted.as_str().unwrap().as_bytes(),
                        "{id}"
                    );
                }
            }
            other => panic!("{id}: unhandled operation {other}"),
        }
    }
}

#[test]
fn independent_python_canonical_profile_oracles() {
    let cases = lines("canonical-profiles-v1.jsonl");
    assert_eq!(
        cases.len(),
        17,
        "a dropped Python oracle is a contract change"
    );
    for case in cases {
        let id = required(&case, "case_id");
        let raw = required(&case, "input_utf8").as_bytes();
        let parsed = parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap_or_else(|error| {
                panic!("{id}: Python-accepted input failed Rust parse: {error}")
            });
        let expected = &case["expected"];
        for (profile, bytes_key, digest_key) in [
            (
                CanonicalProfile::CorpusSnapshotV1,
                "corpus_snapshot_v1_utf8",
                "corpus_snapshot_v1_sha256",
            ),
            (
                CanonicalProfile::SourceRecordDigestV1,
                "source_record_v1_utf8",
                "source_record_v1_sha256",
            ),
        ] {
            let bytes = canonical_bytes_v1(parsed.root(), profile, JsonLimits::default())
                .unwrap_or_else(|error| {
                    panic!("{id}/{}: canonical error {error}", profile.as_str())
                });
            assert_eq!(
                bytes,
                required(expected, bytes_key).as_bytes(),
                "{id}/{} bytes",
                profile.as_str()
            );
            assert_eq!(
                Digest256::of_bytes(&bytes).to_hex(),
                required(expected, digest_key),
                "{id}/{} digest",
                profile.as_str()
            );
            assert_eq!(
                canonical_raw_bytes_v1(raw, profile, JsonLimits::default()).unwrap(),
                bytes,
                "{id}/{} strict raw entry point",
                profile.as_str()
            );
        }
        assert_eq!(
            canonical_raw_bytes_v1(
                raw,
                CanonicalProfile::SourceCommandInputV1,
                JsonLimits::default()
            )
            .unwrap(),
            required(expected, "source_record_v1_utf8").as_bytes(),
            "{id}/command byte profile only"
        );
    }
    let duplicate = br#"{"id":1,"\u0069d":2}"#;
    assert_eq!(
        canonical_raw_bytes_v1(
            duplicate,
            CanonicalProfile::SourceCommandInputV1,
            JsonLimits::default()
        )
        .unwrap_err()
        .code
        .as_str(),
        "duplicate_member",
        "command raw bytes must reject decoded duplicate keys"
    );
}

#[test]
fn independent_python_whole_form_set_history_bytes() {
    let cases = lines("history-v1/legacy-whole-form-set.jsonl");
    assert_eq!(
        cases.len(),
        12,
        "a dropped Python history oracle is a contract change"
    );
    let limits = JsonLimits::new(2 * 1024 * 1024, 64, 300_000, 4_300).unwrap();
    for case in cases {
        let id = required(&case, "case_id");
        let raw = match required(&case, "input_kind") {
            "inline_json" => required(&case, "input_json").as_bytes().to_vec(),
            "repeat_string" => format!(
                "{{\"{}\":\"{}\"}}",
                required(&case, "key"),
                required(&case, "unit").repeat(case["count"].as_u64().unwrap() as usize),
            )
            .into_bytes(),
            "repo_file" => {
                let path = fixtures()
                    .join("../../..")
                    .join(required(&case, "input_path"));
                let raw = fs::read(path).unwrap();
                assert_eq!(
                    Digest256::of_bytes(&raw).to_hex(),
                    required(&case, "input_sha256"),
                    "{id}"
                );
                raw
            }
            kind => panic!("{id}: unknown input kind {kind}"),
        };
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits);
        if case.get("expected_python_error").and_then(Value::as_str) == Some("ValueError") {
            assert_eq!(parsed.unwrap_err().code.as_str(), "nonfinite_float", "{id}");
            continue;
        }
        let parsed = parsed.unwrap_or_else(|error| panic!("{id}: parse failed: {error}"));
        let emitted = emit_json_profile(
            parsed.root(),
            JsonEmissionProfile::SourceFormSetPublishedV1,
            limits,
        );
        if case.get("expected_python_error").and_then(Value::as_str) == Some("UnicodeEncodeError") {
            assert_eq!(
                emitted.unwrap_err().code.as_str(),
                "invalid_unicode_scalar",
                "{id}"
            );
            continue;
        }
        if case.get("expected_error").is_some() {
            assert_eq!(
                emitted.unwrap_err().code.as_str(),
                required(&case, "expected_error"),
                "{id}"
            );
            continue;
        }
        let emitted = emitted.unwrap_or_else(|error| panic!("{id}: emit failed: {error}"));
        let expected = &case["expected"];
        assert_eq!(
            emitted.bytes.len() as u64,
            expected["size_bytes"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(
            emitted.sha256.to_hex(),
            required(expected, "sha256"),
            "{id}"
        );
        if let Some(text) = expected.get("utf8").and_then(Value::as_str) {
            assert_eq!(emitted.bytes, text.as_bytes(), "{id}");
        } else if required(&case, "input_kind") == "repo_file" {
            assert_eq!(emitted.bytes, raw, "{id}: pinned public file bytes changed");
        }
    }
}

fn read_limits() -> ReadLimits {
    ReadLimits {
        max_manifest_bytes: 8192,
        max_manifest_entries: 32,
        max_selected_object_bytes: 1024,
        json: JsonLimits::default(),
    }
}

fn revision(hex: &str) -> SourceRevision {
    SourceRevision(Digest256::from_hex(hex).unwrap())
}

fn fixture_store() -> PathBuf {
    fixtures().join("corpus-v1/store")
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let destination = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&source, &destination)?;
        } else {
            fs::copy(source, destination)?;
        }
    }
    Ok(())
}

fn working_store() -> (TempDir, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    copy_tree(&fixture_store(), &root).unwrap();
    (temporary, root)
}

fn fixture_manifest() -> Value {
    serde_json::from_slice(&fs::read(fixtures().join("corpus-v1/fixture.json")).unwrap()).unwrap()
}

fn selected_object(root: &Path, sha256: &str) -> PathBuf {
    root.join("objects").join(sha256)
}

fn canonical_json(value: &Value) -> Vec<u8> {
    // This setup encodes only the ASCII v1 manifest; its expected digest and
    // original bytes come from the checked-in independent fixture.
    // Workspace feature unification may enable serde_json/preserve_order.
    // Canonical fixture bytes must not depend on the map's insertion order.
    let mut sorted = value.clone();
    sorted.sort_all_objects();
    let mut raw = serde_json::to_vec(&sorted).unwrap();
    raw.push(b'\n');
    raw
}

#[test]
fn independent_corpus_v1_positive_and_missing() {
    let fixture = fixture_manifest();
    let reader = CorpusReader::open_existing(&fixture_store(), read_limits()).unwrap();
    assert_eq!(
        reader.select_current().unwrap().unwrap().0.to_hex(),
        required(&fixture, "revision_current")
    );
    for case in fixture["selected_cases"].as_array().unwrap() {
        let id = required(case, "case_id");
        let snapshot = reader
            .load_exact(revision(required(case, "revision")))
            .unwrap();
        let result = if let Some(source_id) = case.get("source_id").and_then(Value::as_str) {
            reader.resolve(&snapshot, Selector::SourceId(source_id))
        } else {
            let path = RelativePath::parse(required(case, "path")).unwrap();
            reader.resolve(&snapshot, Selector::Path(&path))
        };
        if case.get("expected_error").is_some() {
            assert_eq!(
                format!("{:?}", result.unwrap_err().code),
                required(case, "expected_error"),
                "{id}"
            );
            continue;
        }
        let actual = result.unwrap();
        let expected = &case["expected_descriptor"];
        assert_eq!(
            actual.revision.0.to_hex(),
            required(expected, "revision"),
            "{id}"
        );
        assert_eq!(actual.path.as_str(), required(expected, "path"), "{id}");
        assert_eq!(actual.sha256.to_hex(), required(expected, "sha256"), "{id}");
        assert_eq!(
            actual.size_bytes,
            expected["size_bytes"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(
            actual.mode as u64,
            expected["mode"].as_u64().unwrap(),
            "{id}"
        );
        let mut selected = Vec::new();
        let count = reader
            .read_selected(&snapshot, &actual, 1024, &mut selected)
            .unwrap();
        assert_eq!(count as usize, selected.len(), "{id}");
        assert_eq!(
            selected,
            decode_hex(required(case, "expected_bytes_hex")),
            "{id}"
        );
    }
}

#[test]
fn independent_corpus_v1_negative_boundaries() {
    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");
    let old_sha = required(
        &fixture["selected_cases"][0]["expected_descriptor"],
        "sha256",
    );
    for case in fixture["negative_cases"].as_array().unwrap() {
        let id = required(case, "case_id");
        if matches!(id, "traversal" | "git-component") {
            assert_eq!(
                RelativePath::parse(required(case, "selector_path"))
                    .unwrap_err()
                    .code
                    .as_str(),
                "unsafe_path",
                "{id}"
            );
            continue;
        }
        let (_temporary, root) = working_store();
        let old_object = selected_object(&root, old_sha);
        let old_snapshot = root.join("revisions").join(old).join("snapshot.json");
        match id {
            "wrong-selected-digest" => fs::write(&old_object, b"other").unwrap(),
            "wrong-selected-size" => fs::write(&old_object, b"tiny").unwrap(),
            "unrelated-corrupt-object" => {
                let current = required(&fixture, "revision_current");
                let manifest: Value = serde_json::from_slice(
                    &fs::read(root.join("revisions").join(current).join("snapshot.json")).unwrap(),
                )
                .unwrap();
                let beta_sha = manifest["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|entry| entry["path"] == "records/beta.txt")
                    .unwrap()["sha256"]
                    .as_str()
                    .unwrap();
                fs::write(
                    selected_object(&root, beta_sha),
                    b"damaged unrelated object",
                )
                .unwrap();
            }
            "symlink-selected-object" => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::symlink;
                    let outside = root.parent().unwrap().join("outside");
                    fs::write(&outside, b"other").unwrap();
                    fs::remove_file(&old_object).unwrap();
                    symlink(outside, &old_object).unwrap();
                }
                #[cfg(not(unix))]
                {
                    continue;
                }
            }
            "corrupt-ancestor-manifest" => {
                let mut raw = fs::read(&old_snapshot).unwrap();
                assert_eq!(raw[0], b'{');
                raw.insert(1, b' ');
                fs::write(&old_snapshot, raw).unwrap();
            }
            "duplicate-identity-json-field" => {
                let original = fs::read_to_string(&old_snapshot).unwrap();
                let one = "\"identities\":{\"tos.work.synthetic.alpha\":\"records/alpha.txt\"}";
                let two = "\"identities\":{\"tos.work.synthetic.alpha\":\"records/alpha.txt\",\"tos.work.synthetic.alpha\":\"records/alpha.txt\"}";
                assert!(original.contains(one));
                fs::write(&old_snapshot, original.replacen(one, two, 1)).unwrap();
            }
            "canonical-manifest-revision-mismatch" => {
                let mut manifest: Value =
                    serde_json::from_slice(&fs::read(&old_snapshot).unwrap()).unwrap();
                manifest["validator_sha256"] = Value::String("b".repeat(64));
                fs::write(&old_snapshot, canonical_json(&manifest)).unwrap();
            }
            "duplicate-member-path" => {
                let mut manifest: Value =
                    serde_json::from_slice(&fs::read(&old_snapshot).unwrap()).unwrap();
                let duplicated = manifest["files"][0].clone();
                manifest["files"].as_array_mut().unwrap().push(duplicated);
                let mut body = manifest.clone();
                body.as_object_mut().unwrap().remove("revision");
                let new_revision = Digest256::of_bytes(&canonical_json(&body)).to_hex();
                manifest["revision"] = Value::String(new_revision.clone());
                let new_home = root.join("revisions").join(&new_revision);
                fs::create_dir(&new_home).unwrap();
                fs::write(new_home.join("snapshot.json"), canonical_json(&manifest)).unwrap();
                let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
                assert_eq!(
                    reader.load_exact(revision(&new_revision)).unwrap_err().code,
                    StoreErrorCode::InvalidMemberIndex,
                    "{id}"
                );
                continue;
            }
            "missing-old-revision"
            | "bounded-read"
            | "declared-selected-size-over-cap"
            | "actual-object-growth-after-resolve"
            | "manifest-byte-cap"
            | "aggregate-index-entry-cap" => {}
            other => panic!("unhandled corpus negative {other}"),
        }
        let mut limits = read_limits();
        if id == "declared-selected-size-over-cap" {
            limits.max_selected_object_bytes = 4;
        }
        if id == "manifest-byte-cap" {
            limits.max_manifest_bytes = 488;
        }
        if id == "aggregate-index-entry-cap" {
            limits.max_manifest_entries = 2;
        }
        let reader = CorpusReader::open_existing(&root, limits).unwrap();
        let result_code = if id == "missing-old-revision" {
            reader
                .load_exact(revision(required(case, "selected_revision")))
                .unwrap_err()
                .code
        } else if id == "manifest-byte-cap"
            || id == "aggregate-index-entry-cap"
            || id == "corrupt-ancestor-manifest"
            || id == "duplicate-identity-json-field"
            || id == "canonical-manifest-revision-mismatch"
        {
            reader.load_exact(revision(old)).unwrap_err().code
        } else {
            let snapshot = reader.load_exact(revision(old)).unwrap();
            if id == "actual-object-growth-after-resolve" {
                let descriptor = reader
                    .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
                    .unwrap();
                fs::write(&old_object, b"larger").unwrap();
                let mut stage = Vec::new();
                let error = reader
                    .read_selected(&snapshot, &descriptor, 5, &mut stage)
                    .unwrap_err();
                assert!(
                    stage.is_empty(),
                    "{id}: preflight refusal must not expose bytes"
                );
                error.code
            } else if id == "bounded-read" {
                let descriptor = reader
                    .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
                    .unwrap();
                let mut stage = Vec::new();
                let error = reader
                    .read_selected(&snapshot, &descriptor, 4, &mut stage)
                    .unwrap_err();
                assert!(
                    stage.is_empty(),
                    "{id}: declared size refusal must not expose bytes"
                );
                error.code
            } else {
                let result =
                    reader.resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"));
                if id == "unrelated-corrupt-object" {
                    assert!(result.is_ok(), "{id}: unrelated object must not be hashed");
                    continue;
                }
                result.unwrap_err().code
            }
        };
        assert_eq!(
            format!("{result_code:?}"),
            required(case, "expected_error"),
            "{id}"
        );
    }
}

#[cfg(target_os = "linux")]
fn old_alpha_bytes(reader: &CorpusReader, old: &str) -> Vec<u8> {
    let snapshot = reader.load_exact(revision(old)).unwrap();
    let descriptor = reader
        .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
        .unwrap();
    let mut bytes = Vec::new();
    reader
        .read_selected(&snapshot, &descriptor, 1024, &mut bytes)
        .unwrap();
    bytes
}

#[cfg(target_os = "linux")]
#[test]
fn corpus_reader_keeps_opened_root_and_object_directory() {
    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");
    let old_sha = required(
        &fixture["selected_cases"][0]["expected_descriptor"],
        "sha256",
    );
    let expected = decode_hex(required(
        &fixture["selected_cases"][0],
        "expected_bytes_hex",
    ));

    // Rename the opened root and put a valid but corrupt decoy at its former
    // pathname. A reader rooted in path strings would follow the decoy.
    let (_temporary, root) = working_store();
    let decoy = root.parent().unwrap().join("decoy-store");
    copy_tree(&root, &decoy).unwrap();
    fs::write(selected_object(&decoy, old_sha), b"other").unwrap();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    fs::rename(&root, root.parent().unwrap().join("original-store")).unwrap();
    fs::rename(&decoy, &root).unwrap();
    assert_eq!(
        old_alpha_bytes(&reader, old),
        expected,
        "root path replacement"
    );

    // Replacing the objects directory inside an otherwise stable root must
    // likewise not redirect an already-open reader to new object bytes.
    let (_temporary, root) = working_store();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    fs::rename(root.join("objects"), root.join("objects-original")).unwrap();
    fs::create_dir(root.join("objects")).unwrap();
    fs::write(selected_object(&root, old_sha), b"other").unwrap();
    assert_eq!(
        old_alpha_bytes(&reader, old),
        expected,
        "objects directory replacement"
    );

    // A replaced revisions directory must not change which historical
    // manifest this already-open reader validates.
    let (_temporary, root) = working_store();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    fs::rename(root.join("revisions"), root.join("revisions-original")).unwrap();
    copy_tree(&root.join("revisions-original"), &root.join("revisions")).unwrap();
    fs::write(
        root.join("revisions").join(old).join("snapshot.json"),
        b"{}",
    )
    .unwrap();
    assert_eq!(
        old_alpha_bytes(&reader, old),
        expected,
        "revisions directory replacement"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn corpus_reader_refuses_symlinked_revision() {
    use std::os::unix::fs::symlink;

    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");

    let (_temporary, root) = working_store();
    let linked_parent = root.parent().unwrap().join("linked-parent");
    symlink(root.parent().unwrap(), &linked_parent).unwrap();
    assert_eq!(
        CorpusReader::open_existing(&linked_parent.join("store"), read_limits())
            .unwrap_err()
            .code,
        StoreErrorCode::InvalidRoot,
        "symlink in the absolute root path"
    );

    let (_temporary, root) = working_store();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    let revision_dir = root.join("revisions").join(old);
    let real_dir = root.join("revisions").join("saved-revision");
    fs::rename(&revision_dir, &real_dir).unwrap();
    symlink(&real_dir, &revision_dir).unwrap();
    assert_eq!(
        reader.load_exact(revision(old)).unwrap_err().code,
        StoreErrorCode::UnsafePath,
        "symlinked intermediate revision"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn corpus_reader_refuses_fifo_object() {
    use std::process::Command;
    use std::time::{Duration, Instant};

    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");
    if let Some(root) = std::env::var_os("TOS_ASS_FIFO_PROBE_ROOT") {
        fs::write(Path::new(&root).join(".fifo-probe-ran"), b"started").unwrap();
        let reader = CorpusReader::open_existing(Path::new(&root), read_limits()).unwrap();
        let snapshot = reader.load_exact(revision(old)).unwrap();
        assert_eq!(
            reader
                .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
                .unwrap_err()
                .code,
            StoreErrorCode::UnsafePath,
            "opened FIFO must be refused as a non-regular object"
        );
        return;
    }

    let (_temporary, root) = working_store();
    let old_sha = required(
        &fixture["selected_cases"][0]["expected_descriptor"],
        "sha256",
    );
    let object = selected_object(&root, old_sha);
    fs::remove_file(&object).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(&object)
            .status()
            .unwrap()
            .success()
    );
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("corpus_reader_refuses_fifo_object")
        .env("TOS_ASS_FIFO_PROBE_ROOT", &root)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("FIFO object probe blocked past its 10-second watchdog");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "FIFO object probe failed");
    assert_eq!(
        fs::read(root.join(".fifo-probe-ran")).unwrap(),
        b"started",
        "child test filter did not execute the FIFO probe"
    );
}
#[path = "source_cut_cases.rs"]
mod source_cut_cases;

#[path = "validation_cut_cases.rs"]
mod validation_cut_cases;

#[path = "retirement_cut_cases.rs"]
mod retirement_cut_cases;

#[path = "command_form_cases.rs"]
mod command_form_cases;

#[path = "command_record_cases.rs"]
mod command_record_cases;

#[path = "command_claim_cases.rs"]
mod command_claim_cases;

#[path = "command_collection_cases.rs"]
mod command_collection_cases;
#[path = "command_item_cases.rs"]
mod command_item_cases;
#[path = "command_work_cases.rs"]
mod command_work_cases;

#[path = "command_text_cases.rs"]
mod command_text_cases;

#[path = "compiler_source_cases.rs"]
mod compiler_source_cases;

mod command_claim_publication_cases;
mod command_public_text_cases;

mod command_responsibility_cases;

mod command_edition_cases;

mod command_artifact_cases;
mod command_object_link_cases;

#[path = "command_legacy_claim_cases.rs"]
mod command_legacy_claim_cases;
#[path = "command_metadata_publication_cases.rs"]
mod command_metadata_publication_cases;
mod command_owner_text_cases;
#[path = "command_private_claim_cases.rs"]
mod command_private_claim_cases;
#[path = "command_private_profile_cases.rs"]
mod command_private_profile_cases;
mod native_public_assessment_fixture;
