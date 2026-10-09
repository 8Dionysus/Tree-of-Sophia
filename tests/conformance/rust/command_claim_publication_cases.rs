//! Frozen maintained Claim/Agent fixtures enter the native whole callers before BEGIN.
//! Their exact captured source, packet, prepared rows and independent full-graph oracle remain offline test data.
use super::*;
use flate2::read::GzDecoder;
use serde_json::json;
use std::io::{Cursor, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{
    collections::BTreeMap,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tar::Archive;
use tos_command::source_claim_publication::{
    ClaimAdditionPublication, ClaimPublicationLimits, ClaimPublicationProgress,
    ReviewedClaimProfileTransition,
};
use tos_compiler::{
    QueryVocabulary,
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::{CatalogInputs, SourceOrderProfile},
    prepared_semantic_index::{self, SemanticMaintenanceLimits},
    prepared_source_binding::PreparedSourceInputs,
    source_bibliographic::BibliographicLimits,
    source_witness_catalog::SourceCatalogLimits,
};
#[path = "claim_publication_access.rs"]
mod claim_publication_access;
#[path = "../../../rust/crates/tos-access/tests/support/native_child.rs"]
mod native_child;
pub(super) fn typed(value: &Value) -> JsonValue {
    parse_json(
        &canonical_json(value),
        JsonMode::PublishedStrict,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap()
    .root()
    .clone()
}
// Compare the full semantic report independently of object encounter order.
// Preserve every scalar (including number kind/lexeme) and array position.
pub(super) fn report_object_order(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(entries) => {
            let mut entries = entries
                .iter()
                .map(|(key, value)| (key.clone(), report_object_order(value)))
                .collect::<Vec<_>>();
            entries.sort_by(|a, b| a.0.units().cmp(b.0.units()));
            JsonValue::Object(entries)
        }
        JsonValue::Array(entries) => {
            JsonValue::Array(entries.iter().map(report_object_order).collect())
        }
        scalar => scalar.clone(),
    }
}
pub(super) fn canonical_lf(value: &Value) -> Vec<u8> {
    canonical_raw_bytes_v1(
        &serde_json::to_vec(value).unwrap(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap()
}
pub(super) fn read_packet(path: &Path) -> Value {
    assert!(fs::metadata(path).unwrap().len() <= 16_777_216);
    let raw = fs::read(path).unwrap();
    parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap();
    serde_json::from_slice(&raw).unwrap()
}
pub(super) fn fixture_physical_bytes(roots: &[PathBuf], deadline: Instant) -> (u64, usize) {
    use std::os::unix::fs::MetadataExt;
    let mut pending: Vec<_> = roots.iter().cloned().map(|p| (p, 0usize)).collect();
    let mut entries = 0usize;
    let mut physical = 0u64;
    while let Some((path, depth)) = pending.pop() {
        assert!(Instant::now() < deadline);
        entries += 1;
        assert!(
            entries <= 16_384 && depth <= 64,
            "finite synthetic fixture inventory"
        );
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "synthetic fixture symlink"
        );
        assert!(
            metadata.is_file() || metadata.is_dir(),
            "synthetic fixture member type"
        );
        physical = physical
            .checked_add(metadata.blocks().checked_mul(512).unwrap())
            .unwrap();
        assert!(
            physical <= 512 * 1024 * 1024,
            "synthetic baseline physical budget"
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                assert!(pending.len() + entries < 16_384);
                pending.push((entry.unwrap().path(), depth + 1));
            }
        }
    }
    (physical, entries)
}

// Same local failure-custody pattern as Item/PublicText; no source copy.
// Only the explicit narrow launcher opt-in retains a failed fixture.
struct PublicationFailureFixture {
    directory: tempfile::TempDir,
    label: &'static str,
    retain_failure: bool,
}
impl PublicationFailureFixture {
    fn new(label: &'static str) -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
            label,
            retain_failure: std::env::var_os("TOS_NATIVE_CLAIM_PUBLICATION_RETAIN_FIXTURE")
                .as_deref()
                == Some(std::ffi::OsStr::new("1")),
        }
    }
    fn path(&self) -> &Path {
        self.directory.path()
    }
}
impl Drop for PublicationFailureFixture {
    fn drop(&mut self) {
        if self.retain_failure && std::thread::panicking() {
            self.directory.disable_cleanup(true);
            eprintln!(
                "{} failed fixture retained at {}",
                self.label,
                self.path().display()
            );
        }
    }
}

const CLAIM_FIXTURE_BUNDLE_SCHEMA: &str = "tos_claim_publication_fixture_bundle_v1";
const CLAIM_FIXTURE_SOURCE_HEAD: &str = "1d33e8f3dcc360c7e39a3b72db13d97a0d188f23";
const CLAIM_FIXTURE_SOURCE_SHA256: &str =
    "93b293020a9179f691599434c365336aaa3998b7f6f8deffd5b87f149619cc2d";
const CLAIM_FIXTURE_PROVENANCE_SHA256: &str =
    "e0c59bafd8de24f3bfc42bee4f025f51d86161eca190c5d77dbe65fea9b42f03";
const CLAIM_FIXTURE_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024;
const CLAIM_FIXTURE_MEMBER_BYTES: u64 = 16 * 1024 * 1024;
const CLAIM_FIXTURE_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

fn claim_fixture_bundle_dir() -> PathBuf {
    fixtures().join("fixtures/source-claim-publication-v1")
}

fn claim_fixture_provenance() -> Value {
    let raw = fs::read(claim_fixture_bundle_dir().join("provenance.json")).unwrap();
    assert!(raw.len() <= 4 * 1024 * 1024);
    assert_eq!(
        Digest256::of_bytes(&raw).to_hex(),
        CLAIM_FIXTURE_PROVENANCE_SHA256,
        "frozen Claim/Agent fixture provenance changed"
    );
    let provenance: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(provenance["schema_version"], CLAIM_FIXTURE_BUNDLE_SCHEMA);
    assert_eq!(provenance["repository_head"], CLAIM_FIXTURE_SOURCE_HEAD);
    assert_eq!(
        provenance["fixture_source_sha256"],
        CLAIM_FIXTURE_SOURCE_SHA256
    );
    provenance
}

fn safe_fixture_relative_path(raw: &str) -> PathBuf {
    let path = Path::new(raw);
    assert!(!raw.is_empty() && !raw.contains('\\') && !raw.contains('\0'));
    assert!(path.is_relative());
    assert!(
        path.components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    );
    path.to_owned()
}

fn unpack_claim_fixture_capture(provenance: &Value, capture_name: &str, destination: &Path) {
    let capture = &provenance["captures"][capture_name];
    let archive_name = required(capture, "archive");
    assert!(matches!(
        archive_name,
        "claim-pre.tar.gz" | "agent-pre.tar.gz"
    ));
    let archive_raw = fs::read(claim_fixture_bundle_dir().join(archive_name)).unwrap();
    assert!(archive_raw.len() as u64 <= CLAIM_FIXTURE_ARCHIVE_BYTES);
    assert_eq!(
        archive_raw.len() as u64,
        capture["archive_bytes"].as_u64().unwrap()
    );
    assert_eq!(
        Digest256::of_bytes(&archive_raw).to_hex(),
        required(capture, "archive_sha256")
    );
    assert!(destination.is_absolute() && !destination.exists());
    fs::create_dir(destination).unwrap();
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700)).unwrap();

    let expected_entries = capture["entries"].as_array().unwrap();
    assert!(!expected_entries.is_empty() && expected_entries.len() <= 16_384);
    let expected = expected_entries
        .iter()
        .map(|entry| {
            let path = required(entry, "path").to_owned();
            safe_fixture_relative_path(&path);
            assert!(entry["mode"].as_u64().unwrap() <= 0o777);
            (path, entry.clone())
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        expected.len(),
        expected_entries.len(),
        "duplicate fixture manifest path"
    );

    let decoder = GzDecoder::new(Cursor::new(archive_raw));
    let mut archive = Archive::new(decoder);
    let mut seen = std::collections::BTreeSet::new();
    let mut directories = Vec::new();
    let mut total_file_bytes = 0u64;
    let mut entry_count = 0usize;
    for member in archive.entries().unwrap() {
        let mut member = member.unwrap();
        entry_count = entry_count.checked_add(1).unwrap();
        assert!(entry_count <= 16_384);
        let relative = member.path().unwrap().into_owned();
        assert!(relative.is_relative());
        assert!(
            relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        );
        // tar preserves the directory marker; the manifest records path components.
        let relative: PathBuf = relative.components().collect();
        let relative_text = relative.to_str().expect("captured UTF-8 member path");
        let expected_member = expected
            .get(relative_text)
            .expect("unmanifested fixture member");
        assert!(
            seen.insert(relative_text.to_owned()),
            "duplicate tar member"
        );
        let mode = member.header().mode().unwrap();
        assert_eq!(u64::from(mode), expected_member["mode"].as_u64().unwrap());
        let output = destination.join(&relative);
        match expected_member["kind"].as_str().unwrap() {
            "directory" => {
                assert!(member.header().entry_type().is_dir());
                assert_eq!(member.header().size().unwrap(), 0);
                fs::create_dir_all(&output).unwrap();
                directories.push((output, mode));
            }
            "file" => {
                assert!(member.header().entry_type().is_file());
                let expected_bytes = expected_member["bytes"].as_u64().unwrap();
                assert!(expected_bytes <= CLAIM_FIXTURE_MEMBER_BYTES);
                assert_eq!(member.header().size().unwrap(), expected_bytes);
                total_file_bytes = total_file_bytes.checked_add(expected_bytes).unwrap();
                assert!(total_file_bytes <= CLAIM_FIXTURE_TOTAL_BYTES);
                let mut raw = Vec::with_capacity(usize::try_from(expected_bytes).unwrap());
                member.read_to_end(&mut raw).unwrap();
                assert_eq!(raw.len() as u64, expected_bytes);
                assert_eq!(
                    Digest256::of_bytes(&raw).to_hex(),
                    required(expected_member, "sha256"),
                    "frozen fixture member changed: {relative_text}"
                );
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
            kind => panic!("unsupported fixture member kind {kind}"),
        }
    }
    assert_eq!(
        seen.len(),
        expected.len(),
        "fixture capture member set changed"
    );
    assert_eq!(
        total_file_bytes,
        capture["aggregate_file_bytes"].as_u64().unwrap()
    );
    for (directory, mode) in directories.into_iter().rev() {
        fs::set_permissions(directory, fs::Permissions::from_mode(mode)).unwrap();
    }
}

fn source_command_canonical_bytes(value: &Value) -> Vec<u8> {
    let raw = serde_json::to_vec(value).unwrap();
    let limits = JsonLimits::new(8_388_608, 128, 2_000_000, 4096).unwrap();
    let document = parse_json(&raw, JsonMode::PublishedStrict, limits).unwrap();
    canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceCommandInputV1,
        limits,
    )
    .unwrap()
}

fn relocate_frozen_owner_configuration(path: &Path, source_root: &Path) -> Digest256 {
    let mut configuration: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    configuration["source_root"] = json!(source_root.to_string_lossy());
    configuration["uid"] = json!(fs::metadata(source_root).unwrap().uid());
    let digest = Digest256::of_bytes(&source_command_canonical_bytes(&configuration));
    fs::write(path, canonical_lf(&configuration)).unwrap();
    digest
}

fn relocate_prepared_source_paths(
    packet: &mut Value,
    old_source_root: &str,
    source_root: &Path,
    db_path: &Path,
) {
    let original_inputs_sha = Digest256::of_bytes(&canonical_lf(&packet["source_inputs"])).to_hex();
    let roots = packet["source_inputs"]["roots"].as_object_mut().unwrap();
    assert!(!roots.is_empty() && roots.len() <= 16);
    for root in roots.values_mut() {
        let old_path = Path::new(required(root, "namespace_path"));
        let relative = old_path
            .strip_prefix(old_source_root)
            .expect("captured namespace root belongs to captured fixture root");
        assert!(
            relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        );
        root["namespace_path"] = json!(source_root.join(relative).to_string_lossy());
    }
    let inputs_raw = canonical_lf(&packet["source_inputs"]);
    assert!(inputs_raw.len() <= 1_048_576);
    let inputs_sha = Digest256::of_bytes(&inputs_raw).to_hex();
    let connection = rusqlite::Connection::open(db_path).unwrap();
    let original_sha: String = connection
        .query_row(
            "SELECT sha256 FROM prepared_source_state WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(original_sha, original_inputs_sha);
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    for table in ["source_dependency_state", "agent_context_state"] {
        let (raw, sha): (String, String) = connection
            .query_row(
                &format!("SELECT json,sha256 FROM {table} WHERE singleton=1"),
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(raw.len() <= 1_048_576);
        assert_eq!(Digest256::of_bytes(raw.as_bytes()).to_hex(), sha);
        let mut state: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(canonical_lf(&state), raw.as_bytes());
        assert_eq!(state["source_inputs_sha256"], original_sha);
        assert_eq!(state["binding"], packet["binding"]);
        state["source_inputs_sha256"] = json!(inputs_sha);
        let raw = canonical_lf(&state);
        assert_eq!(
            connection
                .execute(
                    &format!("UPDATE {table} SET json=?1,sha256=?2 WHERE singleton=1"),
                    rusqlite::params![
                        std::str::from_utf8(&raw).unwrap(),
                        Digest256::of_bytes(&raw).to_hex()
                    ]
                )
                .unwrap(),
            1
        );
    }
    let changed = connection
        .execute(
            "UPDATE prepared_source_state SET inputs=?1,sha256=?2 WHERE singleton=1",
            rusqlite::params![std::str::from_utf8(&inputs_raw).unwrap(), inputs_sha],
        )
        .unwrap();
    assert_eq!(changed, 1, "one captured prepared-source row is relocated");
    connection
        .execute_batch("COMMIT; PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
}

fn capture_path(capture: &Value, name: &str) -> PathBuf {
    safe_fixture_relative_path(required(&capture["relocated_paths"], name))
}

fn load_frozen_claim_fixture(workspace: &Path) -> PathBuf {
    let provenance = claim_fixture_provenance();
    let capture = &provenance["captures"]["claim_pre"];
    let destination = workspace.join("claim-pre");
    unpack_claim_fixture_capture(&provenance, "claim_pre", &destination);
    let packet_path = destination.join(capture_path(capture, "packet"));
    let source_root = destination.join(capture_path(capture, "source_root"));
    let owner_path = destination.join(capture_path(capture, "owner_config"));
    let db_path = destination.join(capture_path(capture, "db_path"));
    assert!(source_root.is_dir() && owner_path.is_file() && db_path.is_file());
    let old_configuration: Value = serde_json::from_slice(&fs::read(&owner_path).unwrap()).unwrap();
    let old_source_root = required(&old_configuration, "source_root").to_owned();
    let legacy_owner = source_root.join("owner.json");
    relocate_frozen_owner_configuration(&legacy_owner, &source_root);
    let configuration_digest = relocate_frozen_owner_configuration(&owner_path, &source_root);
    let package = source_root.join("ToS/source-witnesses/relations/synthetic-publication-addition");
    let request_path = package.join("source-create-request.json");
    let receipt_path = package.join("source-create-receipt.json");
    let mut request: Value = serde_json::from_slice(&fs::read(&request_path).unwrap()).unwrap();
    request["expected_configuration"] = json!(configuration_digest.to_prefixed());
    let request_raw = canonical_lf(&request);
    let request_digest = Digest256::of_bytes(&source_command_canonical_bytes(&request));
    fs::write(&request_path, &request_raw).unwrap();

    let mut receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
    receipt["owner_configuration"] = json!(configuration_digest.to_prefixed());
    receipt["request_digest"] = json!(request_digest.to_prefixed());
    receipt["files"]["source-create-request.json"] = json!({
        "bytes": request_raw.len(),
        "sha256": Digest256::of_bytes(&request_raw).to_prefixed()
    });
    let receipt_raw = canonical_lf(&receipt);
    fs::write(&receipt_path, &receipt_raw).unwrap();

    let mut packet: Value = serde_json::from_slice(&fs::read(&packet_path).unwrap()).unwrap();
    packet["db_path"] = json!(db_path.to_string_lossy());
    packet["owner_config"] = json!(owner_path.to_string_lossy());
    packet["receipt_path"] = json!(
        destination
            .join("claim-fixture.receipt.json")
            .to_string_lossy()
    );
    packet["binding_path"] = json!(
        destination
            .join("claim-fixture.binding.json")
            .to_string_lossy()
    );
    packet["expected_receipt_sha256"] = json!(Digest256::of_bytes(&receipt_raw).to_hex());
    packet["expected_request_digest"] = json!(request_digest.to_prefixed());
    relocate_prepared_source_paths(&mut packet, &old_source_root, &source_root, &db_path);
    fs::write(&packet_path, canonical_lf(&packet)).unwrap();
    packet_path
}

fn load_frozen_agent_fixture(workspace: &Path) -> PathBuf {
    let provenance = claim_fixture_provenance();
    let capture = &provenance["captures"]["agent_pre"];
    let destination = workspace.join("agent-pre");
    unpack_claim_fixture_capture(&provenance, "agent_pre", &destination);
    let packet_path = destination.join(capture_path(capture, "packet"));
    let source_root = destination.join(capture_path(capture, "source_root"));
    let owner_path = destination.join(capture_path(capture, "owner_config"));
    let db_path = destination.join(capture_path(capture, "db_path"));
    let catalog_namespace_path = destination.join(capture_path(capture, "catalog_namespace_path"));
    assert!(source_root.is_dir() && owner_path.is_file() && db_path.is_file());
    // The frozen prepared input carries the root JSON inline. Its namespace
    // locates the authenticated partition files; no root file was archived.
    assert!(!catalog_namespace_path.exists());
    assert!(catalog_namespace_path.parent().unwrap().is_dir());
    let old_configuration: Value = serde_json::from_slice(&fs::read(&owner_path).unwrap()).unwrap();
    let old_source_root = required(&old_configuration, "source_root").to_owned();
    let owner_digest = relocate_frozen_owner_configuration(&owner_path, &source_root);
    let mut packet: Value = serde_json::from_slice(&fs::read(&packet_path).unwrap()).unwrap();
    let catalog_root = &packet["source_inputs"]["roots"]["source-catalog"];
    assert_eq!(
        Digest256::of_bytes(required(catalog_root, "root_json").as_bytes()).to_hex(),
        required(catalog_root, "snapshot_sha256")
    );
    packet["source_root"] = json!(source_root.to_string_lossy());
    packet["owner_config"] = json!(owner_path.to_string_lossy());
    packet["db_path"] = json!(db_path.to_string_lossy());
    packet["catalog_namespace_path"] = json!(catalog_namespace_path.to_string_lossy());
    packet["python_preview"]["owner_configuration"] = json!(owner_digest.to_prefixed());
    relocate_prepared_source_paths(&mut packet, &old_source_root, &source_root, &db_path);
    fs::write(&packet_path, canonical_lf(&packet)).unwrap();
    let lock = source_root.join("ToS/source-witnesses/.historical-create.writer.lock");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o600)).unwrap();
    packet_path
}

fn frozen_agent_full_graph() -> Value {
    let provenance = claim_fixture_provenance();
    let capture = &provenance["captures"]["agent_post_oracle"];
    assert_eq!(
        capture["transaction_json_pointers"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let raw = fs::read(claim_fixture_bundle_dir().join(required(capture, "path"))).unwrap();
    assert_eq!(raw.len() as u64, capture["bytes"].as_u64().unwrap());
    assert_eq!(
        Digest256::of_bytes(&raw).to_hex(),
        required(capture, "sha256")
    );
    serde_json::from_slice(&raw).unwrap()
}

#[test]
fn maintained_claim_addition_whole_transaction_and_access() {
    let deadline = Instant::now() + Duration::from_secs(240);
    // Observe exact products before fixture or material writes. These reads
    // do not copy binaries into the disposable workspace.
    let executable_path = std::env::current_exe().expect("actual protected conformance ELF path");
    let executable = executable_path.as_path();
    let consumer = PathBuf::from(std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN").unwrap());
    assert!(consumer.is_absolute());
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let executable_bytes = fs::metadata(executable).unwrap().len();
    let consumer_bytes = fs::metadata(&consumer).unwrap().len();
    let worker_bytes = fs::metadata(&worker_path).unwrap().len();
    assert!(executable_bytes <= 512 * 1024 * 1024);
    assert!(consumer_bytes <= 512 * 1024 * 1024);
    assert!(worker_bytes <= 128 * 1024 * 1024);
    let executable_sha = native_child::bounded_sha_before(executable, 512 * 1024 * 1024, deadline);
    let consumer_sha = native_child::bounded_sha_before(&consumer, 512 * 1024 * 1024, deadline);
    let worker_sha = native_child::bounded_sha_before(&worker_path, 128 * 1024 * 1024, deadline);
    assert_eq!(
        executable_sha.to_hex(),
        std::env::var("TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256").unwrap()
    );
    assert_eq!(
        consumer_sha.to_hex(),
        std::env::var("TOS_NATIVE_PREPARED_CONSUMER_SHA256").unwrap()
    );
    eprintln!(
        "Claim whole preflight E={executable_bytes}:{} C={consumer_bytes}:{} W={worker_bytes}:{}",
        executable_sha.to_hex(),
        consumer_sha.to_hex(),
        worker_sha.to_hex()
    );
    assert!(Instant::now() < deadline);
    let workspace = PublicationFailureFixture::new("Claim addition");
    let packet_path = load_frozen_claim_fixture(workspace.path());
    assert!(packet_path.is_absolute());
    let packet = read_packet(&packet_path);
    let db_path = PathBuf::from(required(&packet, "db_path"))
        .canonicalize()
        .unwrap();
    let owner_config = PathBuf::from(required(&packet, "owner_config"))
        .canonicalize()
        .unwrap();
    assert!(fs::metadata(&owner_config).unwrap().len() <= 1_048_576);
    let config_raw = fs::read(&owner_config).unwrap();
    let config: Value = serde_json::from_slice(&config_raw).unwrap();
    let fixture_root = PathBuf::from(required(&config, "source_root"));
    assert!(fixture_root.is_absolute());
    let temporary_base = std::env::temp_dir().canonicalize().unwrap();
    assert!(
        !fs::symlink_metadata(&fixture_root)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let fixture_root = fixture_root.canonicalize().unwrap();
    let workspace_root = workspace.path().canonicalize().unwrap();
    assert!(fixture_root.starts_with(&temporary_base) && fixture_root != temporary_base);
    assert!(owner_config.starts_with(&fixture_root) && db_path.starts_with(&fixture_root));
    let inventory_roots = if fixture_root.starts_with(&workspace_root) {
        vec![workspace_root]
    } else {
        vec![workspace_root, fixture_root.clone()]
    };
    let selected_packet = packet_path.canonicalize().unwrap();
    assert!(
        inventory_roots
            .iter()
            .any(|root| selected_packet.starts_with(root))
    );
    for output in ["binding_path", "receipt_path"] {
        let parent = Path::new(required(&packet, output))
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        assert!(inventory_roots.iter().any(|root| parent.starts_with(root)));
    }
    let (fixture_bytes, fixture_entries) = fixture_physical_bytes(&inventory_roots, deadline);
    eprintln!("Claim whole preflight F={fixture_bytes} physical bytes entries={fixture_entries}");
    fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600)).unwrap();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.busy_timeout(Duration::from_secs(2)).unwrap();
    assert!(connection.is_autocommit());
    let cancel = Arc::new(AtomicBool::new(false));
    let progress =
        ClaimPublicationProgress::install(&connection, cancel.clone(), deadline, 100_000_000)
            .unwrap();
    connection
        .execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF")
        .unwrap();
    // Finite maintained consumer envelope; library defaults remain portable.
    let publication_limits = PublicationLimits {
        max_mutations: 100_000,
        ..PublicationLimits::default()
    };
    let catalog_limits = CatalogMaintenanceLimits {
        max_delta_bytes: 16_777_216,
        max_catalog_entries: 16_384,
        max_aggregate_bytes: 16_777_216,
        max_catalog_bytes: 8_388_608,
        max_index_bytes: 67_108_864,
        ..CatalogMaintenanceLimits::default()
    };
    let semantic_limits = SemanticMaintenanceLimits {
        max_rows: 100_000,
        max_queries: 100_000,
        max_writes: 100_000,
        max_read_bytes: 33_554_432,
        max_input_bytes: 16_777_216,
        max_input_values: 1_000_000,
        max_output_items: 16_384,
        max_bytes: 67_108_864,
        ..SemanticMaintenanceLimits::default()
    };
    let binding = typed(&packet["binding"]);
    let old_binding_path = workspace.path().join("old-binding.json");
    fs::write(&old_binding_path, canonical_lf(&packet["binding"])).unwrap();
    let catalog = CatalogInputs {
        header: typed(&packet["header"]),
        entity_registry: typed(&packet["entities"]),
        relation_registry: typed(&packet["relations"]),
        lenses: packet["lenses"]
            .as_array()
            .unwrap()
            .iter()
            .map(typed)
            .collect(),
        source_order_profile: SourceOrderProfile::SourceGraphId,
    };
    let source =
        PreparedSourceInputs::parse(&canonical_lf(&packet["source_inputs"]), publication_limits)
            .unwrap();
    // Explicit retained pre-publication source cut continuation; default fixture unchanged.
    let retained_source = if std::env::var_os("TOS_NATIVE_CLAIM_RETAINED_SOURCE_CUT").is_some() {
        let mut files = BTreeMap::new();
        let mut total = 0usize;
        for (path, hex) in packet["source_files"].as_object().unwrap() {
            let raw = decode_hex(hex.as_str().unwrap());
            total = total.checked_add(raw.len()).unwrap();
            assert!(total <= 16_777_216 && files.len() < 2048);
            files.insert(path.clone(), raw);
        }
        // The source packet retains weak catalog/control carriers for its caller.
        // The selected restore cut follows the same existing authored membership law.
        files.retain(|path, _| {
            path != "ToS/source-witnesses/.historical-create.writer.lock"
                && tos_source_store::is_authored_source_path_v1(path)
        });
        let manifest_upper = files
            .keys()
            .try_fold(512usize, |total, path| {
                total.checked_add(serde_json::to_vec(path).unwrap().len() + 256)
            })
            .unwrap();
        assert!(manifest_upper <= 4_194_304);
        let (captured, modes) = agent_authored_capture(&fixture_root, deadline, 16_777_216);
        assert_authored_bytes_equal(&captured, &files, deadline, "actual authored FS vs packet");
        let cut_root = fixture_root.join("e4-source-cut");
        // A stopped pre-publication attempt may resume with its exact retained cut.
        // Selection is explicit and proves every manifest mode and object byte;
        // the receipt supplies only the candidate revision, never source authority.
        let retained_cut = std::env::var_os("TOS_NATIVE_CLAIM_RETAINED_SOURCE_CUT");
        assert!(retained_cut.is_some());
        let revision = if let Some(selected) = retained_cut.as_ref() {
            let selected = PathBuf::from(selected);
            assert_eq!(selected, fixture_root.join("e4-source-cut-receipt.json"));
            let metadata = fs::symlink_metadata(&selected).unwrap();
            assert!(metadata.is_file() && metadata.len() <= 4_194_304);
            let receipt = read_packet(&selected);
            assert_eq!(
                receipt["store"].as_str().unwrap(),
                cut_root.to_str().unwrap()
            );
            let revision = tos_foundation::SourceRevision(
                Digest256::from_hex(receipt["revision"].as_str().unwrap()).unwrap(),
            );
            let selected_cut =
                super::command_form_cases::open_cut(&cut_root, revision, deadline, &cancel);
            assert_eq!(selected_cut.current().member_count(), captured.len());
            for member in selected_cut.current().members() {
                let path = member.path.as_str();
                assert_eq!(member.mode, modes[path]);
                let selected = selected_cut
                    .read_member(revision, &member.path, 8_388_608, deadline, &cancel)
                    .unwrap();
                assert!(
                    selected.raw == captured[path],
                    "selected authored member differs: path={path:?} bytes={} sha256={}",
                    selected.raw.len(),
                    Digest256::of_bytes(&selected.raw).to_prefixed()
                );
            }
            revision
        } else {
            assert!(!cut_root.exists());
            super::validation_cut_cases::write_cut_store_with_modes(&captured, &cut_root, &modes)
        };
        let source_bytes: usize = captured.values().map(Vec::len).sum();
        let history_members = captured
            .keys()
            .filter(|path| path.starts_with("ToS/source-witnesses/.record-revisions/"))
            .count();
        let cut_receipt = json!({"store":cut_root,"revision":revision.0.to_hex(),
        "selected_moment":"source Claim committed; before native prepared bootstrap/profile/publication",
        "selection":"actual authored source membership; operational writer lock excluded",
        "source_bytes":source_bytes,"members":captured.len(),"history_members":history_members,
        "manifest_upper_bytes":manifest_upper,"modes":modes,
        "derived_database_is_source_member":false});
        let cut_receipt_raw = canonical_lf(&cut_receipt);
        assert!(cut_receipt_raw.len() <= 4_194_304);
        if retained_cut.is_some() {
            assert_eq!(
                fs::read(fixture_root.join("e4-source-cut-receipt.json")).unwrap(),
                cut_receipt_raw
            );
        } else {
            fs::write(
                fixture_root.join("e4-source-cut-receipt.json"),
                cut_receipt_raw,
            )
            .unwrap();
        }
        Some((captured, modes, cut_root, revision, source_bytes))
    } else {
        None
    };
    // Explicit native auxiliary bootstrap over the SAME immutable predecessor.
    // Preserve the captured independent semantic rows in a bounded reference
    // packet; no executable hash is relabeled on the imported index.
    let reference = workspace.path().join("python-semantic-reference.json");
    let mut semantic_reference = BTreeMap::new();
    let mut reference_bytes = 0usize;
    for table in [
        "semantic_state",
        "semantic_rows",
        "semantic_edges",
        "semantic_deps",
        "semantic_cardinality",
        "semantic_cardinality_errors",
        "semantic_diagnostics",
        "semantic_pending",
    ] {
        let mut stmt = connection
            .prepare(&format!("SELECT * FROM {table} LIMIT 16385"))
            .unwrap();
        let count = stmt.column_count();
        let rows: Vec<Vec<Value>> = stmt
            .query_map([], |r| {
                let mut values = Vec::new();
                for i in 0..count {
                    let value = match r.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(n) => Value::from(n),
                        rusqlite::types::ValueRef::Text(v) => {
                            Value::String(std::str::from_utf8(v).unwrap().to_owned())
                        }
                        _ => panic!("semantic fixture scalar ABI"),
                    };
                    reference_bytes += match &value {
                        Value::String(s) => s.len(),
                        _ => 32,
                    };
                    assert!(reference_bytes <= 16_777_216);
                    values.push(value);
                }
                Ok(values)
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(rows.len() <= 16384);
        semantic_reference.insert(table, rows);
    }
    let raw = serde_json::to_vec(&semantic_reference).unwrap();
    assert!(raw.len() <= 16_777_216);
    fs::write(reference, raw).unwrap();
    drop(semantic_reference);
    {
        let tx = connection.unchecked_transaction().unwrap();
        for table in [
            "semantic_pending",
            "semantic_diagnostics",
            "semantic_cardinality_errors",
            "semantic_cardinality",
            "semantic_deps",
            "semantic_edges",
            "semantic_rows",
            "semantic_state",
        ] {
            tx.execute(&format!("DROP TABLE {table}"), []).unwrap();
        }
        let report = prepared_semantic_index::bootstrap_semantic_index_transaction(
            &tx,
            &binding,
            &catalog.entity_registry,
            &catalog.relation_registry,
            None,
            required(
                &packet["header"]["normalization_binding"],
                "processor_digest",
            ),
            semantic_limits,
        )
        .unwrap();
        assert_eq!(
            report_object_order(&typed(&packet["baseline_semantic_report"])),
            report_object_order(&report)
        );
        // Import the frozen independent catalog through the genuine native
        // bootstrap, which reproduces its full catalog/header before binding
        // the native auxiliary index. Never relabel the captured projector.
        for table in [
            "catalog_heads",
            "catalog_occurrences",
            "catalog_contributors",
            "catalog_totals",
            "catalog_atoms",
            "catalog_state",
        ] {
            tx.execute(&format!("DROP TABLE {table}"), []).unwrap();
        }
        let catalog_receipt =
            tos_compiler::prepared_maintenance::bootstrap_prepared_catalog_transaction(
                &tx,
                &binding,
                &catalog,
                publication_limits,
                catalog_limits,
            )
            .unwrap();
        assert_eq!(catalog_receipt.binding, binding);
        assert!(!catalog_receipt.publication_changed && !catalog_receipt.consumer_switched);
        tx.commit().unwrap();
    }
    // Real bounded schema worker over an actual sealed synthetic source cut.
    let mut files = BTreeMap::new();
    let mut bytes = 0usize;
    for (path, hex) in packet["source_files"].as_object().unwrap() {
        let raw = decode_hex(hex.as_str().unwrap());
        bytes += raw.len();
        assert!(bytes <= 16_777_216);
        files.insert(path.clone(), raw);
    }
    assert!(files.len() <= 2048);
    // Existing helper emits one fixed record per member. Bound its exact
    // escaped path contribution plus conservative256-byte framing BEFORE write.
    let manifest_upper = files
        .keys()
        .try_fold(512usize, |total, path| {
            total.checked_add(serde_json::to_vec(path).unwrap().len() + 256)
        })
        .unwrap();
    assert!(manifest_upper <= 4_194_304);
    let (cut_root, revision) = if let Some((_, _, cut_root, revision, _)) = &retained_source {
        (cut_root.clone(), *revision)
    } else {
        let cut_root = workspace.path().join("schema-cut");
        let revision = super::validation_cut_cases::write_cut_store(&files, &cut_root);
        (cut_root, revision)
    };
    drop(files);
    let cut = super::command_form_cases::open_cut(&cut_root, revision, deadline, &cancel);
    let mut worker = super::command_form_cases::schemas(&cut, deadline, &cancel);
    let descriptor = serde_json::to_vec(&packet["descriptor"]).unwrap();
    let vocabulary =
        QueryVocabulary::parse(&descriptor, tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES)
            .unwrap();
    let artifact = std::env::var("TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256")
        .expect("OPS exact current conformance artifact SHA required");
    Digest256::from_hex(&artifact).unwrap();
    let before = packet["header"]["normalization_binding"].clone();
    let mut after = before.clone();
    after["processor_digest"] = Value::String(artifact.clone());
    let dependencies = &packet["source_inputs"]["dependencies"];
    let review = ReviewedClaimProfileTransition {
        dependency_implementation_before: required(&packet, "dependency_implementation_before")
            .to_owned(),
        declaration_before: required(dependencies, "declaration-profile").to_owned(),
        agent_publication_before: required(dependencies, "agent-publication-profile").to_owned(),
        claim_publication_before: dependencies
            .get("claim-publication-profile")
            .and_then(Value::as_str)
            .map(str::to_owned),
        normalization_before: before,
        normalization_after: after,
        native_normalization_processor_sha256: artifact,
        review_ref: "test:maintained-claim-whole-source-compatibility-review".to_owned(),
    };
    let bibliographic = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 2048,
            max_rows: 4096,
            max_file_bytes: 16_777_216,
            max_row_bytes: 1_048_576,
            max_contract_bytes: 4_194_304,
            max_output_row_bytes: 1_048_576,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16_777_216,
        max_output_rows: 4096,
        max_output_bytes: 16_777_216,
        deadline,
    };
    let mut operation = ClaimAdditionPublication::prepare(
        Path::new(required(&packet, "owner_config")),
        source,
        binding,
        catalog,
        Digest256::from_hex(required(&packet, "expected_receipt_sha256")).unwrap(),
        Digest256::from_prefixed(required(&packet, "expected_request_digest")).unwrap(),
        review,
        vocabulary,
        descriptor,
        ClaimPublicationLimits::default(),
        bibliographic,
        &mut worker,
        cancel.clone(),
    )
    .unwrap();
    let tx =
        rusqlite::Transaction::new_unchecked(&connection, rusqlite::TransactionBehavior::Immediate)
            .unwrap();
    let result = operation
        .apply_transaction(
            &tx,
            &progress,
            publication_limits,
            catalog_limits,
            semantic_limits,
        )
        .unwrap();
    assert_eq!(result["prepared_committed"], false);
    let result = operation.commit_transaction(tx, &progress).unwrap();
    drop(worker);
    drop(cut);
    assert_eq!(result["prepared_committed"], true);
    let expected = &packet["expected"];
    for kind in ["node", "relation"] {
        let mut stmt = connection
            .prepare(&format!(
                "SELECT id,json FROM knowledge_{kind}s ORDER BY id LIMIT 16385"
            ))
            .unwrap();
        let actual: BTreeMap<String, Value> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|r| {
                let (id, raw) = r.unwrap();
                (id, serde_json::from_str(&raw).unwrap())
            })
            .collect();
        assert!(actual.len() <= 16384);
        let expected: BTreeMap<String, Value> = expected[format!("{kind}s")]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (required(r, "id").to_owned(), r.clone()))
            .collect();
        assert!(
            actual == expected,
            "maintained full union {kind} oracle mismatch"
        );
    }
    assert_eq!(
        result["semantic_report"],
        expected["counts"]["semantic_validation"]
    );
    let binding_path = Path::new(required(&packet, "binding_path"));
    fs::write(binding_path, canonical_lf(&result["binding"])).unwrap();
    fs::write(required(&packet, "receipt_path"), canonical_lf(&result)).unwrap();
    drop(progress);
    drop(connection);
    let old_executor = tos_access::prepared_local::PreparedLocalExecutor::open(
        db_path.clone(),
        old_binding_path,
        None,
    )
    .unwrap();
    let stale = tos_access::http::handle_get(
        &old_executor,
        "GET",
        "/api/knowledge/catalog",
        tos_access::prepared_local::profile().with_query_timeout(
            Duration::from_secs(5).min(deadline.checked_duration_since(Instant::now()).unwrap()),
        ),
    );
    assert_ne!(
        stale.status, 200,
        "old selected binding must refuse the successor"
    );
    drop(old_executor);
    if let Some((captured, modes, cut_root, revision, source_bytes)) = retained_source {
        // Restore the genuine selected pre-publication authored cut through the
        // existing protected native entry. The prepared DB is a separate retained
        // artifact; neither its bytes nor writer authority enter this source cut.
        let restored_root = fixture_root.join("e4-source-restored");
        assert!(!restored_root.exists());
        let restore_deadline = deadline.min(Instant::now() + Duration::from_secs(30));
        let output = native_child::bounded_output_before(
            Command::new(&consumer)
                .arg("restore-source-cut")
                .arg("--corpus-store")
                .arg(&cut_root)
                .arg("--source-revision")
                .arg(revision.0.to_prefixed())
                .arg("--output")
                .arg(&restored_root)
                .args([
                    "--max-revisions",
                    "1",
                    "--max-directories",
                    "4096",
                    "--max-members",
                    "2048",
                    "--max-member-bytes",
                    "8388608",
                    "--max-total-bytes",
                    "16777216",
                    "--max-metadata-bytes",
                    "4194304",
                    "--max-seconds",
                    "20",
                ]),
            65_536,
            restore_deadline,
        );
        assert!(
            output.status.success(),
            "E4 source restore refused: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let restored: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(restored["restored"], true);
        assert_eq!(restored["writer_grant_transferred"], false);
        assert_eq!(restored["source_revision"], revision.0.to_prefixed());
        assert_eq!(restored["member_count"], captured.len());
        assert_eq!(restored["source_bytes"], source_bytes);
        let (restored_files, restored_modes) =
            agent_authored_capture(&restored_root, deadline, 16_777_216);
        assert_authored_bytes_equal(
            &restored_files,
            &captured,
            deadline,
            "cold source cut bytes",
        );
        assert_eq!(restored_modes, modes, "cold source cut modes differ");
        fs::write(
            fixture_root.join("e4-source-restore-receipt.json"),
            canonical_lf(&restored),
        )
        .unwrap();
        drop(restored_files);
        drop(restored_modes);

        drop(modes);
    }
    claim_publication_access::verify_published_access_until(
        &db_path,
        binding_path,
        required(&packet, "new_node_id"),
        required(&packet, "new_relation_id"),
        deadline,
    );
}

pub(super) const AGENT_RECORD_COMPONENTS: &[&str] = &[
    "rust/crates/tos-command/src/source_native_cli.rs",
    "rust/crates/tos-command/src/source_command.rs",
    "rust/crates/tos-command/src/source_revisions.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "rust/crates/tos-validation/src/assessment.rs",
    "rust/crates/tos-command/src/source_private_profile.rs",
    "rust/crates/tos-command/src/source_sign_native.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_work_transaction.rs",
    "rust/crates/tos-command/src/source_private_claim.rs",
    "rust/crates/tos-command/src/source_private_owner_store.rs",
    "rust/crates/tos-command/src/source_creation_store.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_versions.rs",
];
pub(super) fn agent_authored(root: &Path, deadline: Instant) -> BTreeMap<String, Vec<u8>> {
    agent_authored_capture(root, deadline, 33_554_432).0
}
fn assert_authored_bytes_equal(
    left: &BTreeMap<String, Vec<u8>>,
    right: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    context: &str,
) {
    assert!(left.len() <= 2048 && right.len() <= 2048 && Instant::now() < deadline);
    if left == right {
        return;
    }
    let mut changed = 0usize;
    let mut details = Vec::new();
    let keys: std::collections::BTreeSet<_> = left.keys().chain(right.keys()).collect();
    for path in keys {
        assert!(Instant::now() < deadline);
        if left.get(path) == right.get(path) {
            continue;
        }
        changed += 1;
        if details.len() < 8 {
            let describe = |bytes: Option<&Vec<u8>>| {
                bytes.map(|raw| (raw.len(), Digest256::of_bytes(raw).to_prefixed()))
            };
            details.push(format!(
                "path={:?} path_sha256={} actual={:?} expected={:?}",
                path.chars().take(256).collect::<String>(),
                Digest256::of_bytes(path.as_bytes()).to_prefixed(),
                describe(left.get(path)),
                describe(right.get(path))
            ));
        }
    }
    panic!(
        "{context}: members actual={} expected={} differing={changed}; first8={details:?}",
        left.len(),
        right.len()
    );
}

pub(super) fn agent_authored_capture(
    root: &Path,
    deadline: Instant,
    max_bytes: usize,
) -> (BTreeMap<String, Vec<u8>>, BTreeMap<String, u32>) {
    assert!(max_bytes <= 33_554_432);
    let mut pending = vec![root.join("ToS")];
    let mut files = BTreeMap::new();
    let mut visited = 0usize;
    let mut total = 0usize;
    let mut modes = BTreeMap::new();
    while let Some(directory) = pending.pop() {
        assert!(Instant::now() < deadline);
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            visited += 1;
            assert!(visited <= 4096 && Instant::now() < deadline);
            let metadata = entry.file_type().unwrap();
            assert!(!metadata.is_symlink());
            if metadata.is_dir() {
                pending.push(entry.path());
                continue;
            }
            assert!(metadata.is_file());
            let path = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            if path == "ToS/source-witnesses/.historical-create.writer.lock"
                || !tos_source_store::is_authored_source_path_v1(&path)
            {
                continue;
            }
            let before = fs::symlink_metadata(entry.path()).unwrap();
            assert!(before.is_file() && before.len() <= 8_388_608 && files.len() < 2048);
            total = total
                .checked_add(usize::try_from(before.len()).unwrap())
                .unwrap();
            assert!(total <= max_bytes);
            let stamp = |metadata: &fs::Metadata| {
                (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.len(),
                    metadata.mode(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                )
            };
            let raw = fs::read(entry.path()).unwrap();
            let after = fs::symlink_metadata(entry.path()).unwrap();
            assert!(after.is_file() && stamp(&before) == stamp(&after));
            assert_eq!(raw.len() as u64, before.len());
            let mode = before.permissions().mode() & 0o7777;
            assert_eq!(mode & 0o7000, 0, "source fixture special mode refused");
            modes.insert(path.clone(), mode);
            files.insert(path, raw);
        }
    }
    (files, modes)
}
pub(super) fn agent_catalog(packet: &Value, header: &Value) -> CatalogInputs {
    CatalogInputs {
        header: typed(header),
        entity_registry: typed(&packet["entities"]),
        relation_registry: typed(&packet["relations"]),
        lenses: packet["lenses"]
            .as_array()
            .unwrap()
            .iter()
            .map(typed)
            .collect(),
        source_order_profile: SourceOrderProfile::SourceGraphId,
    }
}
pub(super) fn agent_native_call(
    _repository: &Path,
    owner: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> Value {
    use std::io::Write;
    let mut input = tempfile::NamedTempFile::new().unwrap();
    let input_raw = serde_json::to_vec(request).unwrap();
    assert!(input_raw.len() <= 1_048_576);
    input.write_all(&input_raw).unwrap();
    let native_command = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("exact native source owner command required"),
    );
    assert!(native_command.is_absolute());
    let mut command = Command::new(native_command);
    command
        .arg("--invocation")
        .arg(invocation)
        .stdin(std::process::Stdio::from(input.reopen().unwrap()));
    let output = native_child::bounded_output_before(
        &mut command,
        1_048_576,
        deadline.min(Instant::now() + Duration::from_secs(60)),
    );
    assert!(
        output.status.success(),
        "actual Agent Record caller action={} operation={}: {}",
        request["action"],
        request["operation"],
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn agent_publication_cli_output(
    executable: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> std::process::Output {
    use std::io::Write;
    let mut input = tempfile::NamedTempFile::new().unwrap();
    let raw = serde_json::to_vec(request).unwrap();
    assert!(raw.len() <= 1_048_576);
    input.write_all(&raw).unwrap();
    let mut command = Command::new(executable);
    command
        .arg("--invocation")
        .arg(invocation)
        .stdin(std::process::Stdio::from(input.reopen().unwrap()));
    native_child::bounded_output_before(
        &mut command,
        1_048_576,
        deadline.min(Instant::now() + Duration::from_secs(60)),
    )
}
fn agent_publication_cli(
    executable: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> Value {
    let output = agent_publication_cli_output(executable, invocation, request, deadline);
    assert!(
        output.status.success(),
        "actual Agent publication CLI action={} status={}: {}",
        request["action"]
            .as_str()
            .expect("fixed Agent publication action"),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["grants_admission"], false);
    envelope["result"].clone()
}

fn agent_sql_snapshot(
    connection: &rusqlite::Connection,
    deadline: Instant,
) -> BTreeMap<String, Vec<Vec<rusqlite::types::Value>>> {
    let mut names=connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 129").unwrap();
    let names = names
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert!(names.len() <= 128);
    let mut result = BTreeMap::new();
    let mut bytes = 0usize;
    for name in names {
        assert!(
            Instant::now() < deadline
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
        let probe = connection
            .prepare(&format!("SELECT * FROM \"{name}\" LIMIT 0"))
            .unwrap();
        let columns = probe.column_count();
        assert!((1..=64).contains(&columns));
        drop(probe);
        let order = (1..=columns)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut statement = connection
            .prepare(&format!("SELECT * FROM \"{name}\" ORDER BY {order}"))
            .unwrap();
        let mut rows = statement.query([]).unwrap();
        let mut values = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            assert!(Instant::now() < deadline);
            // Charge owned cell state and outer Vec growth before retaining a row.
            // Four row slots cover Vec's initial allocation and later doubling.
            let mut row_bytes = 4 * std::mem::size_of::<Vec<rusqlite::types::Value>>()
                + columns * std::mem::size_of::<rusqlite::types::Value>();
            for column in 0..columns {
                let size = match row.get_ref(column).unwrap() {
                    rusqlite::types::ValueRef::Text(v) | rusqlite::types::ValueRef::Blob(v) => {
                        v.len()
                    }
                    _ => 16,
                };
                assert!(size <= 16_777_216);
                row_bytes = row_bytes.checked_add(size).unwrap();
            }
            bytes = bytes.checked_add(row_bytes).unwrap();
            assert!(bytes <= 33_554_432);
            let mut owned = Vec::with_capacity(columns);
            for column in 0..columns {
                owned.push(row.get::<_, rusqlite::types::Value>(column).unwrap());
            }
            values.push(owned);
        }
        result.insert(name, values);
    }
    result
}

fn agent_physical(workspace: &Path, deadline: Instant) {
    let (bytes, _) = fixture_physical_bytes(&[workspace.to_owned()], deadline);
    // Reserve4MiB for the one sequential anonymous CLI input/stdout/stderr
    // framing, including allocated-block rounding; all named copies are here.
    assert!(bytes <= 508 * 1024 * 1024);
}

pub(super) fn publish_agent_load_fixture(
    workspace: &Path,
    deadline: Instant,
) -> (PathBuf, PathBuf, PathBuf, PathBuf, Value) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_compiler::prepared_source_binding::read_prepared_source_inputs_transaction;
    use tos_source_store::{ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1};
    let cancel = Arc::new(AtomicBool::new(false));
    let repository = super::validation_cut_cases::repository();
    let owner_command = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("exact native owner CLI required"),
    );
    let prepared_consumer = PathBuf::from(
        std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
            .expect("exact native prepared-access CLI required"),
    );
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let image_paths = [
        (
            std::env::current_exe().expect("actual protected conformance ELF path"),
            512 * 1024 * 1024,
        ),
        (owner_command.clone(), 512 * 1024 * 1024),
        (worker_path.clone(), 128 * 1024 * 1024),
        (prepared_consumer, 512 * 1024 * 1024),
    ];
    for (path, maximum) in &image_paths {
        assert!(path.is_absolute());
        assert!(fs::metadata(path).unwrap().len() <= *maximum as u64);
    }
    let images = image_paths
        .iter()
        .map(|(p, maximum)| native_child::bounded_sha_before(p, *maximum, deadline))
        .collect::<Vec<_>>();
    let packet_path = load_frozen_agent_fixture(workspace);
    let packet = read_packet(&packet_path);
    let root = PathBuf::from(required(&packet, "source_root"))
        .canonicalize()
        .unwrap();
    assert!(root.starts_with(workspace.canonicalize().unwrap()));
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let owner = PathBuf::from(required(&packet, "owner_config"));
    let config_raw = fs::read(&owner).unwrap();
    assert_eq!(
        fs::metadata(&owner).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let db_path = PathBuf::from(required(&packet, "db_path"));
    assert!(owner.starts_with(&root) && db_path.starts_with(&root));
    // The native DB fence requires its immediate parent to be fixture-private.
    let db_parent = db_path.parent().unwrap();
    assert_eq!(db_parent.canonicalize().unwrap(), db_parent);
    assert_eq!(
        fs::metadata(db_parent).unwrap().uid(),
        fs::metadata(&root).unwrap().uid()
    );
    fs::set_permissions(db_parent, fs::Permissions::from_mode(0o700)).unwrap();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.busy_timeout(Duration::from_secs(2)).unwrap();
    connection
        .execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF")
        .unwrap();
    let publication = PublicationLimits {
        max_mutations: 100_000,
        ..PublicationLimits::default()
    };
    let catalog_limits = CatalogMaintenanceLimits {
        max_delta_bytes: 16_777_216,
        max_catalog_entries: 16_384,
        max_aggregate_bytes: 16_777_216,
        max_catalog_bytes: 8_388_608,
        max_index_bytes: 67_108_864,
        ..CatalogMaintenanceLimits::default()
    };
    let semantic_limits = SemanticMaintenanceLimits {
        max_rows: 100_000,
        max_queries: 100_000,
        max_writes: 100_000,
        max_read_bytes: 33_554_432,
        max_input_bytes: 16_777_216,
        max_input_values: 1_000_000,
        max_output_items: 16_384,
        max_bytes: 67_108_864,
        ..SemanticMaintenanceLimits::default()
    };
    let operation_limits = ClaimPublicationLimits::default();
    let initial_binding = typed(&packet["binding"]);
    let initial_catalog = agent_catalog(&packet, &packet["header"]);
    // Rebuild native auxiliary indexes from this fixture's exact prepared rows.
    // Keep the frozen independent catalog, header, and semantic report unchanged.
    {
        let tx = connection.unchecked_transaction().unwrap();
        for table in [
            "semantic_pending",
            "semantic_diagnostics",
            "semantic_cardinality_errors",
            "semantic_cardinality",
            "semantic_deps",
            "semantic_edges",
            "semantic_rows",
            "semantic_state",
        ] {
            tx.execute(&format!("DROP TABLE {table}"), []).unwrap();
        }
        let report = prepared_semantic_index::bootstrap_semantic_index_transaction(
            &tx,
            &initial_binding,
            &initial_catalog.entity_registry,
            &initial_catalog.relation_registry,
            None,
            required(
                &packet["header"]["normalization_binding"],
                "processor_digest",
            ),
            semantic_limits,
        )
        .unwrap();
        assert_eq!(
            report_object_order(&report),
            report_object_order(&typed(&packet["baseline_semantic_report"]))
        );
        for table in [
            "catalog_heads",
            "catalog_occurrences",
            "catalog_contributors",
            "catalog_totals",
            "catalog_atoms",
            "catalog_state",
        ] {
            tx.execute(&format!("DROP TABLE {table}"), []).unwrap();
        }
        let catalog_receipt =
            tos_compiler::prepared_maintenance::bootstrap_prepared_catalog_transaction(
                &tx,
                &initial_binding,
                &initial_catalog,
                publication,
                catalog_limits,
            )
            .unwrap();
        assert_eq!(catalog_receipt.binding, initial_binding);
        assert!(!catalog_receipt.publication_changed && !catalog_receipt.consumer_switched);
        tx.commit().unwrap();
    }
    let original_files = agent_authored(&root, deadline);
    let original_store = workspace.join("original-cut");
    let original_revision =
        super::validation_cut_cases::write_cut_store(&original_files, &original_store);
    let _original =
        super::command_form_cases::open_cut(&original_store, original_revision, deadline, &cancel);
    let mut combined = original_files.clone();
    for name in AGENT_RECORD_COMPONENTS {
        let raw = fs::read(repository.join(name)).unwrap();
        assert!(raw.len() <= 2_097_152);
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &raw).unwrap();
        combined.insert((*name).to_owned(), raw);
    }
    assert!(combined.values().map(Vec::len).sum::<usize>() <= 33_554_432 && combined.len() <= 2048);
    let capture = workspace.join("software-capture");
    let restored = workspace.join("software-restored");
    let mut commit_command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            commit_command.env_remove(name);
        }
    }
    commit_command
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    commit_command
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"]);
    let commit_output = native_child::bounded_output_before(&mut commit_command, 4096, deadline);
    assert!(commit_output.status.success());
    let commit = String::from_utf8(commit_output.stdout)
        .unwrap()
        .trim()
        .to_owned();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    let prefixes = AGENT_RECORD_COMPONENTS.to_vec();
    let selection = super::source_cut_cases::capture_software_archive(
        &repository,
        &commit,
        &prefixes,
        &capture,
        deadline,
        &cancel,
    );
    super::source_cut_cases::restore_software_archive(
        &capture, &restored, &selection, deadline, &cancel,
    );
    let capture_raw = fs::read(capture.join("capture.json")).unwrap();
    assert!(capture_raw.len() <= 1_048_576);
    let capture_manifest: Value = serde_json::from_slice(&capture_raw).unwrap();
    assert_eq!(required(&capture_manifest, "source_git_commit"), commit);
    assert_eq!(
        Digest256::of_bytes(&capture_raw),
        selection.capture_manifest_sha256
    );
    let software = SoftwareCaptureReader::open(
        &capture,
        &restored,
        selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 33_554_432,
            json: JsonLimits::default(),
        },
        deadline,
        &cancel,
    )
    .unwrap();
    let component_paths = AGENT_RECORD_COMPONENTS
        .iter()
        .map(|p| RelativePath::parse(p).unwrap())
        .collect::<Vec<_>>();
    let _components = software.select_components(&component_paths).unwrap();
    let invocation_path = workspace.join("agent-native-invocation.json");
    let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,"owner_context":null,"assessment_schema_worker":null,"native_executable":owner_command,"native_executable_sha256":images[1].to_prefixed(),"corpus_store":original_store,"source_revision":original_revision.0.to_prefixed(),"original_source_revision":original_revision.0.to_prefixed(),"software_capture":capture,"software_restored_root":restored,"software_selection":{"source_git_commit":selection.source_git_commit,"source_git_tree":selection.source_git_tree,"capture_manifest_sha256":selection.capture_manifest_sha256.to_prefixed()},"software_components":AGENT_RECORD_COMPONENTS,"schema_worker":{"absolute_path":worker_path,"sha256":images[2].to_prefixed()},"budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let agent_invocation_path = root.join("agent-publication-invocation.json");
    let binding_companion = root.join("agent-selected-binding.json");
    let source_companion = root.join("agent-selected-source.json");
    let catalog_companion = root.join("agent-selected-catalog.json");
    let descriptor_companion = root.join("agent-selected-descriptor.json");
    let write_protected = |path: &Path, value: &Value| {
        fs::write(path, canonical_lf(value)).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    };
    let selected_catalog_value = |header: &Value| serde_json::json!({"header":header,"entity_registry":packet["entities"],"relation_registry":packet["relations"],"lenses":packet["lenses"],"source_order_profile":"source-graph-id-v1"});
    write_protected(&binding_companion, &packet["binding"]);
    write_protected(&source_companion, &packet["source_inputs"]);
    write_protected(
        &catalog_companion,
        &selected_catalog_value(&packet["header"]),
    );
    write_protected(&descriptor_companion, &packet["descriptor"]);
    let file_sha = |path: &Path| Digest256::of_bytes(&fs::read(path).unwrap()).to_prefixed();
    let mut agent_invocation = invocation.clone();
    agent_invocation["schema_version"] = json!("tos_local_native_agent_publication_invocation_v1");
    agent_invocation["prepared_database"] = json!(db_path);
    agent_invocation["expected_binding_path"] = json!(binding_companion);
    agent_invocation["source_inputs_path"] = json!(source_companion);
    agent_invocation["source_inputs_sha256"] = json!(file_sha(&source_companion));
    agent_invocation["catalog_path"] = json!(catalog_companion);
    agent_invocation["catalog_sha256"] = json!(file_sha(&catalog_companion));
    agent_invocation["descriptor_path"] = json!(descriptor_companion);
    agent_invocation["descriptor_sha256"] = json!(file_sha(&descriptor_companion));
    agent_invocation["reviewed_execution_transition"] = Value::Null;
    agent_invocation["publication_limits"] = json!({
        "operation":{"max_nodes":operation_limits.max_nodes,"max_relations":operation_limits.max_relations,"max_claims":operation_limits.max_claims,"max_bytes":operation_limits.max_bytes,"max_row_bytes":operation_limits.max_row_bytes,"max_contexts":operation_limits.max_contexts,"max_vm_steps":operation_limits.max_vm_steps,"cow_target_bytes":operation_limits.cow_target_bytes},
        "publication":publication,"catalog":catalog_limits,"semantic":semantic_limits,
        "bibliographic":{"max_claim_cohort_rows":256,"max_claim_cohort_bytes":16777216,"max_output_rows":16384,"max_output_bytes":16777216}
    });
    write_protected(&agent_invocation_path, &agent_invocation);
    let execution = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &json!({"action":"describe-agent-execution"}),
        deadline,
    );
    let mut after_normalization = packet["header"]["normalization_binding"].clone();
    after_normalization["processor_digest"] = execution["processor"].clone();
    let deps = &packet["source_inputs"]["dependencies"];
    agent_invocation["reviewed_execution_transition"] = json!({
        "dependency_implementation_before":packet["dependency_implementation_before"],"declaration_before":deps["declaration-profile"],"agent_publication_before":deps["agent-publication-profile"],"claim_publication_before":deps.get("claim-publication-profile").cloned().unwrap_or(Value::Null),
        "normalization_before":packet["header"]["normalization_binding"],"normalization_after":after_normalization,"native_normalization_processor_sha256":execution["processor"],"review_ref":"test:real-Agent-whole-predecessor-profile-review","reviewed_after_agent_sha256":execution["agent_publication"]
    });
    write_protected(&agent_invocation_path, &agent_invocation);
    let profile_result = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &json!({"action":"reviewed-agent-execution-bootstrap"}),
        deadline,
    );
    let binding = typed(&profile_result["binding"]);
    let catalog = agent_catalog(&packet, &profile_result["source_header"]);
    let source = {
        let tx = connection.unchecked_transaction().unwrap();
        let source =
            read_prepared_source_inputs_transaction(&tx, &binding, &catalog, publication).unwrap();
        tx.rollback().unwrap();
        source
    };
    let predecessor_binding = workspace.join("Agent-predecessor-binding.json");
    fs::write(
        &predecessor_binding,
        canonical_lf(&profile_result["binding"]),
    )
    .unwrap();
    write_protected(&binding_companion, &profile_result["binding"]);
    write_protected(
        &source_companion,
        &serde_json::from_slice::<Value>(source.raw()).unwrap(),
    );
    write_protected(
        &catalog_companion,
        &selected_catalog_value(&profile_result["source_header"]),
    );
    agent_invocation["source_inputs_sha256"] = json!(file_sha(&source_companion));
    agent_invocation["catalog_sha256"] = json!(file_sha(&catalog_companion));
    agent_invocation["reviewed_execution_transition"] = Value::Null;
    write_protected(&agent_invocation_path, &agent_invocation);
    // Agent's selected Record owner returns a native transport envelope around
    // the source_revisions selected-family response. Other callers keep the
    // outer envelope, so unwrap only these two Agent observations.
    let checked_agent_record_result = |mut envelope: Value| {
        assert!(envelope.is_object(), "Agent native source envelope object");
        assert_eq!(envelope.as_object().unwrap().len(), 4);
        assert_eq!(
            envelope["schema_version"],
            "tos_local_native_source_result_v1"
        );
        assert_eq!(envelope["authentication"], "local-unix-account");
        assert_eq!(envelope["grants_admission"], false);
        let result = envelope.as_object_mut().unwrap().remove("result").unwrap();
        assert!(result.is_object(), "Agent selected Record result object");
        assert_eq!(
            result["schema_version"],
            "tos_local_source_revision_result_v2"
        );
        assert_eq!(result["authentication"], "local-unix-account");
        assert_eq!(result["grants_admission"], false);
        result
    };
    let preview = checked_agent_record_result(agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &packet["proposal"],
        deadline,
    ));
    for field in ["owner_configuration", "source", "revision"] {
        assert_eq!(preview[field], packet["python_preview"][field]);
    }
    // Software dependencies bind the selected current native components; the
    // frozen Python digest belongs to its historical implementation. Source
    // identity and revision above still match the independent frozen oracle.
    assert!(Digest256::from_prefixed(required(&preview, "expected_dependencies")).is_ok());
    let mut request = packet["proposal"].clone();
    request["operation"] = Value::String("record.revise".into());
    request["command_id"] = Value::String("synthetic:actual-native-Agent-correction".into());
    for (field, prepared) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
        ("expected_dependencies", "expected_dependencies"),
        ("expected_publication", "expected_publication"),
    ] {
        request[field] = preview[prepared].clone();
    }
    agent_physical(workspace, deadline);
    let source_result = checked_agent_record_result(agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &request,
        deadline,
    ));
    assert_eq!(source_result["replayed"], false);
    assert_eq!(
        source_result["receipt"]["dependencies"],
        preview["expected_dependencies"]
    );
    let source_receipt = workspace.join("native-Record-receipt.json");
    fs::write(&source_receipt, canonical_lf(&source_result)).unwrap();
    let current_files = agent_authored(&root, deadline);
    let current_store = original_store.clone();
    let current_revision = super::validation_cut_cases::write_cut_store_on_base(
        &current_files,
        &current_store,
        Some(original_revision),
    );
    let _current =
        super::command_form_cases::open_cut(&current_store, current_revision, deadline, &cancel);
    agent_invocation["source_revision"] = json!(current_revision.0.to_prefixed());
    write_protected(&agent_invocation_path, &agent_invocation);
    let publication_request = json!({"action":"publish-agent-correction","recorded_at":source_result["receipt"]["recorded_at"],"record_request":request});
    let result = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &publication_request,
        deadline,
    );
    assert_eq!(result["prepared_committed"], true);
    // Actual protected production CLI stale predecessor refusal preserves all SQL/source bytes.
    let prepared_before = agent_sql_snapshot(&connection, deadline);
    let refusal = agent_publication_cli_output(
        &owner_command,
        &agent_invocation_path,
        &publication_request,
        deadline,
    );
    assert!(!refusal.status.success());
    assert_eq!(prepared_before, agent_sql_snapshot(&connection, deadline));
    assert_eq!(agent_authored(&root, deadline), current_files);
    // Discard publication stdout as an authority: recover only from the persisted
    // paired state under the authenticated source observation and read-only DB.
    let recovered = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &json!({"action":"inspect-agent-publication","recorded_at":source_result["receipt"]["recorded_at"],"record_request":request}),
        deadline,
    );
    assert_eq!(recovered["binding"], result["binding"]);
    assert_eq!(
        recovered["transaction"]["transaction_id"],
        result["transaction_id"]
    );
    assert_eq!(prepared_before, agent_sql_snapshot(&connection, deadline));
    assert_eq!(agent_authored(&root, deadline), current_files);
    let expected = frozen_agent_full_graph();
    assert_eq!(agent_authored(&root, deadline), current_files);
    for kind in ["node", "relation"] {
        let actual: BTreeMap<String, Value> = connection
            .prepare(&format!(
                "SELECT id,json FROM knowledge_{kind}s ORDER BY id LIMIT 16385"
            ))
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|r| {
                let (id, raw) = r.unwrap();
                (id, serde_json::from_str(&raw).unwrap())
            })
            .collect();
        let expected: BTreeMap<String, Value> = expected[format!("{kind}s")]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (required(r, "id").to_owned(), r.clone()))
            .collect();
        assert!(actual.len() <= 16384);
        assert_eq!(actual, expected);
    }
    assert_eq!(
        result["semantic_report"],
        expected["counts"]["semantic_validation"]
    );
    let binding_path = workspace.join("Agent-binding.json");
    write_protected(&binding_path, &result["binding"]);
    let source_raw: String = connection
        .query_row(
            "SELECT inputs FROM prepared_source_state WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(source_raw.len() <= 1_048_576);
    let source_inputs: Value = serde_json::from_str(&source_raw).unwrap();
    assert!(
        source_inputs["source_publication"].as_str().is_some(),
        "real native Record transaction published a selected source token"
    );
    write_protected(&source_companion, &source_inputs);
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    if std::env::var_os("TOS_NATIVE_INSTALLED_SOFTWARE_SITE").as_deref()
        == Some(std::ffi::OsStr::new("1"))
    {
        let installed_access = &image_paths[3].0;
        assert_eq!(
            native_child::bounded_sha_before(installed_access, 512 * 1024 * 1024, deadline),
            images[3]
        );
        let stale = native_child::bounded_output_before(
            Command::new(installed_access)
                .arg("--prepared-read-model")
                .arg(&db_path)
                .arg("--prepared-binding")
                .arg(&predecessor_binding)
                .args(["knowledge", "catalog"]),
            tos_access::prepared_local::PREPARED_RESPONSE_BYTES,
            deadline.min(Instant::now() + Duration::from_secs(5)),
        );
        assert!(
            !stale.status.success(),
            "installed access accepted stale Agent predecessor"
        );
        assert!(
            stale.stdout.is_empty(),
            "stale installed query disclosed rows"
        );
        assert!(
            String::from_utf8_lossy(&stale.stderr).contains("stale_selection"),
            "unexpected installed stale refusal: {}",
            String::from_utf8_lossy(&stale.stderr)
        );
        assert_eq!(
            native_child::bounded_sha_before(installed_access, 512 * 1024 * 1024, deadline),
            images[3]
        );
    }
    let old = tos_access::prepared_local::PreparedLocalExecutor::open(
        db_path.clone(),
        predecessor_binding,
        None,
    )
    .unwrap();
    let stale = tos_access::http::handle_get(
        &old,
        "GET",
        "/api/knowledge/catalog",
        tos_access::prepared_local::profile().with_query_timeout(
            Duration::from_secs(5).min(deadline.saturating_duration_since(Instant::now())),
        ),
    );
    assert_ne!(stale.status, 200);
    drop(old);
    let agent_id = required(&packet, "record_id");
    let node = expected["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["source_graph"] == "source-navigation"
                && r["attributes"]["source_record"]["record_id"] == agent_id
                && r["attributes"]["source_record"]["record_version"] == 2
        })
        .expect("independent current Agent node");
    let relation = expected["relations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source_graph"] == "semantic-interchange" && r["to_id"] == node["id"])
        .expect("maintained Agent related claim relation");
    claim_publication_access::verify_published_access_until(
        &db_path,
        &binding_path,
        required(node, "id"),
        required(relation, "id"),
        deadline,
    );
    agent_physical(workspace, deadline);
    for ((path, maximum), expected) in image_paths.iter().zip(images) {
        assert_eq!(
            native_child::bounded_sha_before(path, *maximum, deadline),
            expected
        );
    }
    assert!(Instant::now() < deadline);
    (
        root,
        db_path,
        binding_path,
        source_companion,
        packet["record_id"].clone(),
    )
}

#[test]
fn maintained_agent_record_correction_whole_transaction_and_access() {
    let deadline = Instant::now() + Duration::from_secs(600);
    let workspace = PublicationFailureFixture::new("Agent correction");
    publish_agent_load_fixture(workspace.path(), deadline);
}
