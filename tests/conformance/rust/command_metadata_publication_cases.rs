//! The maintained initial Metadata oracle observes a real native creation.
//! Creation, profile transition and publication use the SAME protected owner C;
//! its committed database/binding then continue through the real access lanes.
use super::command_claim_publication_cases::{
    AGENT_RECORD_COMPONENTS, agent_authored, agent_authored_capture, agent_catalog,
    agent_native_call, canonical_lf, fixture_physical_bytes, read_packet, report_object_order,
    typed,
};
use super::*;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Component,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tos_command::source_claim_publication::ClaimPublicationLimits;
use tos_compiler::{
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_semantic_index::{self, SemanticMaintenanceLimits},
};
use tos_source_store::{
    CaptureGitRequest, CaptureRestoreLimits, GitCaptureLimits, ReadLimits, SoftwareCaptureReader,
    SoftwareCaptureSelectionV1, capture_git, restore_capture,
};
#[path = "claim_publication_access.rs"]
mod access;
#[path = "../../../rust/crates/tos-access/tests/support/native_child.rs"]
mod native_child;

fn private_json(path: &Path, value: &Value) {
    let raw = canonical_lf(value);
    assert!(raw.len() <= 1_048_576);
    fs::write(path, raw).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn decode_fixture_hex(raw: &str) -> Vec<u8> {
    assert!(raw.len().is_multiple_of(2));
    raw.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => panic!("invalid frozen Metadata source hex"),
            };
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect()
}
fn safe_metadata_fixture_relative(raw: &str) -> PathBuf {
    let relative = Path::new(raw);
    assert!(!relative.as_os_str().is_empty() && !relative.is_absolute());
    assert!(
        relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    );
    relative.to_owned()
}
fn collect_metadata_part(
    fixture_dir: &Path,
    role: &str,
    root_sha: &str,
    descriptor: &Value,
    expected_prefix: &str,
    entries: &BTreeMap<String, Value>,
    seen: &mut BTreeSet<String>,
    deadline: Instant,
    total_bytes: &mut u64,
) {
    assert!(Instant::now() < deadline);
    assert_eq!(required(descriptor, "prefix"), expected_prefix);
    let path = required(descriptor, "path");
    let relative = safe_metadata_fixture_relative(&format!("derived/{path}"));
    let role_prefix = format!("{role}.parts/");
    assert!(path.starts_with(role_prefix.as_str()));
    let stored_path = relative.to_string_lossy().into_owned();
    let manifest = entries
        .get(&stored_path)
        .expect("every root descriptor is frozen");
    let size = descriptor["size_bytes"].as_u64().unwrap();
    assert!(size <= 8_388_608);
    assert_eq!(manifest["bytes"].as_u64(), Some(size));
    assert_eq!(required(manifest, "sha256"), required(descriptor, "sha256"));
    let bindings = manifest["bindings"].as_array().unwrap();
    let binding = bindings
        .iter()
        .find(|entry| entry["role"].as_str() == Some(role))
        .expect("part manifest binds each root role");
    assert_eq!(
        required(binding, "namespace_path"),
        format!("derived/{role}.json").as_str()
    );
    assert_eq!(required(binding, "root_snapshot_sha256"), root_sha);
    for field in [
        "kind",
        "prefix",
        "sha256",
        "size_bytes",
        "decoded_sha256",
        "decoded_bytes",
        "count",
    ] {
        assert_eq!(
            binding[field], descriptor[field],
            "{role} {stored_path} {field}"
        );
    }
    assert!(
        seen.insert(stored_path.clone()),
        "duplicate frozen part path"
    );
    *total_bytes = total_bytes.checked_add(size).unwrap();
    assert!(*total_bytes <= 16_777_216);
    let source = fixture_dir.join(&relative);
    let metadata = fs::metadata(&source).unwrap();
    assert!(metadata.is_file() && metadata.len() == size);
    assert_eq!(
        native_child::bounded_sha_before(&source, 8_388_608, deadline).to_hex(),
        required(descriptor, "sha256")
    );
    if required(descriptor, "kind") == "index" {
        let raw = fs::read(&source).unwrap();
        assert_eq!(
            Digest256::of_bytes(&raw).to_hex(),
            required(descriptor, "sha256")
        );
        assert_eq!(
            raw.len() as u64,
            descriptor["decoded_bytes"].as_u64().unwrap()
        );
        assert_eq!(
            Digest256::of_bytes(&raw).to_hex(),
            required(descriptor, "decoded_sha256")
        );
        let index: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(
            required(&index, "schema_version"),
            "tos_projection_partition_index_v1"
        );
        assert_eq!(required(&index, "prefix"), expected_prefix);
        assert_eq!(index["count"], descriptor["count"]);
        let children = index["children"].as_object().unwrap();
        let mut child_count = 0u64;
        for (key, child) in children {
            assert_eq!(key.len(), 1);
            assert!(
                key.bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            );
            child_count = child_count
                .checked_add(child["count"].as_u64().unwrap())
                .unwrap();
            collect_metadata_part(
                fixture_dir,
                role,
                root_sha,
                child,
                &format!("{expected_prefix}{key}"),
                entries,
                seen,
                deadline,
                total_bytes,
            );
        }
        assert_eq!(child_count, descriptor["count"].as_u64().unwrap());
    } else {
        assert_eq!(required(descriptor, "kind"), "data");
    }
}
fn materialize_metadata_parts(
    fixture_dir: &Path,
    packet: &Value,
    source_root: &Path,
    deadline: Instant,
) {
    let manifest = read_packet(&fixture_dir.join("parts-manifest.json"));
    assert_eq!(
        required(&manifest, "schema_version"),
        "tos_native_metadata_fixture_parts_v1"
    );
    let files = manifest["files"].as_array().unwrap();
    assert!(!files.is_empty() && files.len() <= 512);
    let mut entries = BTreeMap::new();
    let mut total_bytes = 0u64;
    for row in files {
        assert!(Instant::now() < deadline);
        let path = required(row, "path").to_owned();
        let relative = safe_metadata_fixture_relative(&path);
        assert!(path.starts_with("derived/"));
        let bytes = row["bytes"].as_u64().unwrap();
        assert!(bytes <= 8_388_608);
        total_bytes = total_bytes.checked_add(bytes).unwrap();
        assert!(total_bytes <= 16_777_216);
        assert!(entries.insert(path, row.clone()).is_none());
        let source = fixture_dir.join(relative);
        assert_eq!(fs::metadata(&source).unwrap().len(), bytes);
        assert_eq!(
            native_child::bounded_sha_before(&source, 8_388_608, deadline).to_hex(),
            required(row, "sha256")
        );
    }
    assert_eq!(manifest["total_bytes"].as_u64(), Some(total_bytes));
    let roots = packet["source_inputs"]["roots"].as_object().unwrap();
    assert_eq!(roots.len(), 3);
    let root_rows = manifest["roots"].as_array().unwrap();
    assert_eq!(root_rows.len(), roots.len());
    let mut seen = BTreeSet::new();
    let mut descriptor_bytes = 0u64;
    for role in [
        "bibliographic-claims",
        "source-catalog",
        "source-navigation",
    ] {
        let root_item = &roots[role];
        let root_text = required(root_item, "root_json");
        let root_bytes = root_text.as_bytes();
        let root_sha = required(root_item, "snapshot_sha256");
        assert_eq!(Digest256::of_bytes(root_bytes).to_hex(), root_sha);
        let root: Value = serde_json::from_slice(root_bytes).unwrap();
        assert_eq!(
            required(&root, "schema_version"),
            "tos_partitioned_projection_v1"
        );
        let row = root_rows
            .iter()
            .find(|entry| entry["role"].as_str() == Some(role))
            .unwrap();
        assert_eq!(required(row, "snapshot_sha256"), root_sha);
        assert_eq!(
            row["root_json_bytes"].as_u64(),
            Some(root_bytes.len() as u64)
        );
        let mut role_parts = 0u64;
        for collection in root["collections"].as_object().unwrap().values() {
            collect_metadata_part(
                fixture_dir,
                role,
                root_sha,
                &collection["root"],
                "",
                &entries,
                &mut seen,
                deadline,
                &mut descriptor_bytes,
            );
            role_parts = seen
                .iter()
                .filter(|path| path.contains(&format!("/{role}.parts/")))
                .count() as u64;
        }
        assert_eq!(row["part_file_count"].as_u64(), Some(role_parts));
    }
    assert_eq!(
        seen.len(),
        entries.len(),
        "captured part set equals embedded-root closure"
    );
    assert_eq!(seen, entries.keys().cloned().collect());
    for path in seen {
        assert!(Instant::now() < deadline);
        let relative = safe_metadata_fixture_relative(&path);
        let source = fixture_dir.join(&relative);
        let target = source_root.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&source, &target).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    }
}
fn materialize_metadata_predecessor(
    fixture_dir: &Path,
    packet: &mut Value,
    workspace: &Path,
    deadline: Instant,
) -> (PathBuf, PathBuf, PathBuf) {
    const FIXTURE_ROOT: &str = "/__tos_fixture__/metadata-source";
    assert_eq!(required(packet, "source_root"), FIXTURE_ROOT);
    assert_eq!(
        required(packet, "db_path"),
        format!("{FIXTURE_ROOT}/derived/prepared.sqlite").as_str()
    );
    assert_eq!(
        required(packet, "owner_config"),
        format!("{FIXTURE_ROOT}/metadata-addition-owner.json").as_str()
    );
    let source_root = workspace.join("metadata-source");
    fs::create_dir(&source_root).unwrap();
    fs::set_permissions(&source_root, fs::Permissions::from_mode(0o700)).unwrap();
    let source_root = source_root.canonicalize().unwrap();
    let db_path = source_root.join("derived/prepared.sqlite");
    let owner_path = source_root.join("metadata-addition-owner.json");
    let frozen_source_inputs = packet["source_inputs"].clone();
    let roots = packet["source_inputs"]["roots"].as_object_mut().unwrap();
    for role in [
        "bibliographic-claims",
        "source-catalog",
        "source-navigation",
    ] {
        let captured = format!("{FIXTURE_ROOT}/derived/{role}.json");
        assert_eq!(required(&roots[role], "namespace_path"), captured.as_str());
        roots.get_mut(role).unwrap()["namespace_path"] =
            json!(source_root.join("derived").join(format!("{role}.json")));
    }
    packet["source_root"] = json!(source_root);
    packet["db_path"] = json!(db_path);
    packet["owner_config"] = json!(owner_path);
    packet["owner_config_document"]["source_root"] = json!(source_root);

    let source_files = packet["source_files"].as_object().unwrap();
    assert!(source_files.len() <= 2048);
    let mut total = 0usize;
    for (relative, encoded) in source_files {
        assert!(relative.starts_with("ToS/"));
        let relative_path = safe_metadata_fixture_relative(relative);
        let bytes = decode_fixture_hex(encoded.as_str().unwrap());
        total = total.checked_add(bytes.len()).unwrap();
        assert!(bytes.len() <= 8_388_608 && total <= 16_777_216);
        let target = source_root.join(relative_path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, bytes).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let db_parent = db_path.parent().unwrap();
    fs::create_dir_all(db_parent).unwrap();
    fs::set_permissions(db_parent, fs::Permissions::from_mode(0o700)).unwrap();
    fs::copy(fixture_dir.join("prepared-before.sqlite"), &db_path).unwrap();
    fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600)).unwrap();
    let source_inputs_raw = canonical_lf(&packet["source_inputs"]);
    assert!(source_inputs_raw.len() <= 1_048_576);
    let source_inputs_sha = Digest256::of_bytes(&source_inputs_raw).to_hex();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    // Relocate this frozen fixture before its preservation baseline. Every
    // physical source-state row must bind the same relocated input bytes.
    let old_source_sha: String = connection
        .query_row(
            "SELECT sha256 FROM prepared_source_state WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        Digest256::of_bytes(&canonical_lf(&frozen_source_inputs)).to_hex(),
        old_source_sha
    );
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
        assert_eq!(state["binding"], packet["binding"]);
        // The historical export relocated prepared_source_state to its
        // portable root but retained this known reference in both dependents.
        assert_eq!(
            state["source_inputs_sha256"],
            "f2123f65b518e6bd2dddde1e6fc0276c556aaab2ea1cfd5d92064718850b16c3"
        );
        state["source_inputs_sha256"] = json!(source_inputs_sha);
        let bytes = canonical_lf(&state);
        let changed = connection
            .execute(
                &format!("UPDATE {table} SET json=?1,sha256=?2 WHERE singleton=1"),
                rusqlite::params![
                    std::str::from_utf8(&bytes).unwrap(),
                    Digest256::of_bytes(&bytes).to_hex()
                ],
            )
            .unwrap();
        assert_eq!(changed, 1);
    }
    let changed = connection
        .execute(
            "UPDATE prepared_source_state SET inputs=?1,sha256=?2 WHERE singleton=1",
            rusqlite::params![
                std::str::from_utf8(&source_inputs_raw).unwrap(),
                source_inputs_sha
            ],
        )
        .unwrap();
    assert_eq!(changed, 1, "one frozen prepared-source row is rebased");
    connection
        .execute_batch("COMMIT; PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    materialize_metadata_parts(fixture_dir, packet, &source_root, deadline);
    let mut owner_document = packet["owner_config_document"].clone();
    owner_document["uid"] = json!(fs::metadata(&source_root).unwrap().uid());
    private_json(&owner_path, &owner_document);
    // A historical Python fixture has no native cooperating source mutex.
    // Establish it as part of this fresh synthetic owner, before any reader.
    let lock = source_root.join("ToS/source-witnesses/.historical-create.writer.lock");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o600)).unwrap();
    (source_root, db_path, owner_path)
}

fn verify_frozen_metadata_evidence(fixture_dir: &Path, deadline: Instant) {
    let provenance = read_packet(&fixture_dir.join("PROVENANCE.json"));
    assert_eq!(
        required(&provenance, "schema_version"),
        "tos_legacy_python_metadata_oracle_evidence_v1"
    );
    assert!(required(&provenance, "classification").contains("oracle-evidence-only"));
    assert!(required(&provenance, "classification").contains("not source-runtime acceptance"));
    let artifacts = provenance["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 8);
    for artifact in artifacts {
        assert!(Instant::now() < deadline);
        let relative = Path::new(required(artifact, "path"));
        assert!(
            relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        );
        let path = fixture_dir.join(relative);
        assert!(path.starts_with(fixture_dir));
        let cap = match relative.file_name().unwrap().to_str().unwrap() {
            "prepared-before.sqlite" => 629_145_600,
            "prepared-before.packet.json" | "full-union-oracle.json" => 16_777_216,
            "owner-before.json" | "source-create-receipt.json" => 1_048_576,
            "export-capture-manifest.json"
            | "oracle-capture-manifest.json"
            | "parts-manifest.json" => 1_048_576,
            _ => panic!("unexpected frozen Metadata evidence file"),
        };
        assert_eq!(
            native_child::bounded_sha_before(&path, cap, deadline).to_hex(),
            required(artifact, "sha256")
        );
    }
}

#[test]
fn maintained_initial_metadata_whole_transaction_and_access() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let temporary = tempfile::tempdir().unwrap();
    let workspace = temporary.path();
    let cancelled = Arc::new(AtomicBool::new(false));
    let repository = super::validation_cut_cases::repository();
    let consumer = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("protected native owner C"),
    );
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let e = std::env::current_exe().expect("current conformance executable E");
    for (path, cap) in [
        (&e, 512u64 * 1024 * 1024),
        (&consumer, 512 * 1024 * 1024),
        (&worker_path, 128 * 1024 * 1024),
    ] {
        assert!(fs::metadata(path).unwrap().len() <= cap);
    }
    let e_sha = native_child::bounded_sha_before(&e, 512 * 1024 * 1024, deadline);
    let c_sha = native_child::bounded_sha_before(&consumer, 512 * 1024 * 1024, deadline);
    let w_sha = native_child::bounded_sha_before(&worker_path, 128 * 1024 * 1024, deadline);
    let fixture_dir = repository.join("tests/conformance/rust/legacy-python-oracles-v1/metadata");
    verify_frozen_metadata_evidence(&fixture_dir, deadline);
    let mut packet = read_packet(&fixture_dir.join("prepared-before.packet.json"));
    let (root, db_path, owner) =
        materialize_metadata_predecessor(&fixture_dir, &mut packet, workspace, deadline);
    let root = root.canonicalize().unwrap();
    assert!(root.starts_with(workspace.canonicalize().unwrap()));
    assert!(owner.starts_with(&root) && db_path.starts_with(&root));
    // The native DB fence requires its immediate parent to be fixture-private.
    let db_parent = db_path.parent().unwrap();
    assert_eq!(db_parent.canonicalize().unwrap(), db_parent);
    assert_eq!(
        fs::metadata(db_parent).unwrap().uid(),
        fs::metadata(&root).unwrap().uid()
    );
    fs::set_permissions(db_parent, fs::Permissions::from_mode(0o700)).unwrap();
    // Only this synthetic fixture is inventoried. No authored host scan or
    // estimation run; reserve the bounded static increments below before DB work.
    let (baseline, _) = fixture_physical_bytes(&[workspace.to_owned()], deadline);
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(!PathBuf::from(format!("{}{suffix}", db_path.display())).exists());
    }
    let baseline_db = fs::metadata(&db_path)
        .unwrap()
        .blocks()
        .checked_mul(512)
        .unwrap();
    // Replace the DB component of F with the full DB/WAL reserve; no baseline
    // database double counting or second auxiliary database copy.
    assert!(baseline.checked_sub(baseline_db).unwrap() + 480 * 1024 * 1024 <= 1024 * 1024 * 1024);
    fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600)).unwrap();
    let (original_files, original_modes) = agent_authored_capture(&root, deadline, 33_554_432);
    let original_store = workspace.join("original-cut");
    let original_revision = super::validation_cut_cases::write_cut_store_with_modes(
        &original_files,
        &original_store,
        &original_modes,
    );
    let source_catalog: Value = serde_json::from_str(required(
        &packet["source_inputs"]["roots"]["source-catalog"],
        "root_json",
    ))
    .unwrap();
    let mut names: BTreeSet<String> = AGENT_RECORD_COMPONENTS
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    names.extend(
        tos_command::source_claims::CLAIM_GROUNDING_RULE_INPUTS
            .iter()
            .map(|name| (*name).to_owned()),
    );
    // Fixed initial-creation rule inputs not carried by the catalog execution profile.
    names.insert("rust/crates/tos-command/src/source_private_claim.rs".to_owned());
    names.insert("rust/crates/tos-command/src/source_private_owner_store.rs".to_owned());
    names.extend([
        "rust/crates/tos-command/src/source_legacy_historical_claim.rs".to_owned(),
        "rust/crates/tos-command/src/source_corpus_index_projection.rs".to_owned(),
        "rust/crates/tos-compiler/src/source_bibliographic.rs".to_owned(),
        "rust/crates/tos-compiler/src/source_bibliographic_render.rs".to_owned(),
    ]);
    let mut software_files: BTreeMap<String, Vec<u8>> = names
        .iter()
        .map(|name| (name.clone(), fs::read(repository.join(name)).unwrap()))
        .collect();
    let provenance = read_packet(&fixture_dir.join("PROVENANCE.json"));
    let historical_commit = required(&provenance["capture"], "source_commit");
    assert!(
        historical_commit.len() == 40 && historical_commit.bytes().all(|b| b.is_ascii_hexdigit())
    );
    for (reference, expected) in source_catalog["header"]["profile_bindings"]["execution"]
        .as_object()
        .unwrap()
    {
        if reference.starts_with("ToS/") {
            continue;
        }
        safe_metadata_fixture_relative(reference);
        let mut show = Command::new("git");
        show.arg("-C")
            .arg(&repository)
            .arg("show")
            .arg(format!("{historical_commit}:{reference}"));
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                show.env_remove(name);
            }
        }
        show.env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        let output = native_child::bounded_output_before(&mut show, 2_097_152, deadline);
        assert!(output.status.success());
        assert_eq!(expected["bytes"].as_u64(), Some(output.stdout.len() as u64));
        assert_eq!(
            expected["sha256"],
            Digest256::of_bytes(&output.stdout).to_hex()
        );
        assert!(
            software_files
                .insert(reference.clone(), output.stdout)
                .is_none()
        );
        names.insert(reference.clone());
    }
    assert!(names.len() <= 64);
    // This capture is explicitly synthetic: current native rule source plus
    // immutable historical software evidence. No historical file is executed.
    let software_repository = workspace.join("metadata-software-evidence-repository");
    fs::create_dir(&software_repository).unwrap();
    let mut software_bytes = 0usize;
    for (name, raw) in &software_files {
        assert!(Instant::now() < deadline && raw.len() <= 2_097_152);
        software_bytes = software_bytes.checked_add(raw.len()).unwrap();
        assert!(software_bytes <= 33_554_432);
        for base in [&root, &software_repository] {
            let target = base.join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, raw).unwrap();
        }
    }
    let git = |args: &[&str]| {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&software_repository)
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "user.name=ToS synthetic fixture",
                "-c",
                "user.email=fixture@invalid",
            ])
            .args(args);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
        command
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        let output = native_child::bounded_output_before(&mut command, 4096, deadline);
        assert!(
            output.status.success(),
            "synthetic software capture Git operation"
        );
        output.stdout
    };
    git(&["init", "--quiet"]);
    git(&["add", "--all"]);
    git(&[
        "commit",
        "--quiet",
        "-m",
        "synthetic current and historical metadata software evidence",
    ]);
    let capture = workspace.join("software-capture");
    let restored = workspace.join("software-restored");
    let commit = String::from_utf8(git(&["rev-parse", "HEAD^{commit}"]))
        .unwrap()
        .trim()
        .to_owned();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    let include_prefixes = names.iter().cloned().collect::<Vec<_>>();
    let captured = capture_git(
        CaptureGitRequest {
            repository: &software_repository,
            commit: &commit,
            include_prefixes: &include_prefixes,
            exclude_prefixes: &[],
            exclude_path_parts: &[],
            output: &capture,
        },
        GitCaptureLimits {
            max_members: 512,
            max_member_bytes: 2_097_152,
            max_source_bytes: 33_554_432,
            max_metadata_bytes: 4_194_304,
            max_tree_bytes: 4_194_304,
            max_archive_bytes: 33_554_432,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let capture_raw = fs::read(capture.join("capture.json")).unwrap();
    assert!(capture_raw.len() <= 1_048_576);
    assert_eq!(Digest256::of_bytes(&capture_raw), captured.manifest_sha256);
    let manifest: Value = serde_json::from_slice(&capture_raw).unwrap();
    let source_git_tree = captured
        .manifest
        .object_get("source_git_tree")
        .and_then(|value| value.as_str())
        .expect("native capture returns its exact Git tree")
        .to_owned();
    assert_eq!(manifest["source_git_commit"].as_str().unwrap(), commit);
    assert_eq!(
        manifest["source_git_tree"].as_str().unwrap(),
        source_git_tree
    );
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree,
        capture_manifest_sha256: captured.manifest_sha256,
    };
    let read_limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 512,
        max_selected_object_bytes: 33_554_432,
        json: JsonLimits::default(),
    };
    restore_capture(
        &capture,
        &restored,
        &selection,
        CaptureRestoreLimits {
            metadata: read_limits,
            max_archive_bytes: 33_554_432,
            max_decoded_bytes: 33_554_432,
            max_source_bytes: 33_554_432,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
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
        &cancelled,
    )
    .unwrap();
    let paths = names
        .iter()
        .map(|p| RelativePath::parse(p).unwrap())
        .collect::<Vec<_>>();
    software.select_components(&paths).unwrap(); // Selected capture validates the concrete component union.
    let invocation_path = workspace.join("metadata-native-invocation.json");
    let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,"owner_context":null,"assessment_schema_worker":null,"native_executable":consumer,"native_executable_sha256":c_sha.to_prefixed(),"corpus_store":original_store,"source_revision":original_revision.0.to_prefixed(),"original_source_revision":original_revision.0.to_prefixed(),"software_capture":capture,"software_restored_root":restored,"software_selection":{"source_git_commit":selection.source_git_commit,"source_git_tree":selection.source_git_tree,"capture_manifest_sha256":selection.capture_manifest_sha256.to_prefixed()},"software_components":names,"schema_worker":{"absolute_path":worker_path,"sha256":w_sha.to_prefixed()},"budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    private_json(&invocation_path, &invocation);
    let mut request = packet["creation_request"].clone();
    request["operation"] = json!("prepare-create");
    for key in [
        "command_id",
        "expected_configuration",
        "expected_dependencies",
        "expected_revision",
        "expected_source",
    ] {
        request.as_object_mut().unwrap().remove(key);
    }
    let preview =
        agent_native_call(&repository, &owner, &invocation_path, &request, deadline)["result"]
            .clone();
    request["operation"] = json!("source.create");
    request["command_id"] = json!("synthetic:actual-native-Metadata-addition");
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    request["expected_revision"] = Value::Null;
    request["expected_source"] = Value::Null;
    let created =
        agent_native_call(&repository, &owner, &invocation_path, &request, deadline)["result"]
            .clone();
    assert_eq!(created["replayed"], false);
    let (current_files, current_modes) = agent_authored_capture(&root, deadline, 33_554_432);
    // One corpus store retains the original authenticated revision while the
    // new current revision is appended; the fixed CLI opens BOTH through it.
    let current_store = original_store.clone();
    let current_revision = super::validation_cut_cases::write_cut_store_with_optional_modes(
        &current_files,
        &current_store,
        Some(original_revision),
        Some(&current_modes),
    );
    // Genuine native creation is observed by the exact maintained full oracle
    // BEFORE any native prepared profile transition changes Python identities.
    let oracle = read_packet(&fixture_dir.join("full-union-oracle.json"));
    assert_eq!(agent_authored(&root, deadline), current_files);
    let publication = PublicationLimits {
        max_bytes: 67_108_864,
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
    let operation = ClaimPublicationLimits::default();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.busy_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA page_size", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        4096
    );
    connection
        .execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF; PRAGMA max_page_count=16384")
        .unwrap();
    let initial_binding = typed(&packet["binding"]);
    let initial_catalog = agent_catalog(&packet, &packet["header"]);
    {
        // Fresh native auxiliary index from the SAME immutable normalized
        // predecessor. No Python projector digest is relabelled.
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
        // Import the maintained Python catalog through the same native bootstrap
        // used by Claim publication. Reproduce its full catalog/header before
        // binding the native auxiliary index; never relabel its projector.
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
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    let companions = root.join("metadata-prepared-companions");
    fs::create_dir(&companions).unwrap();
    fs::set_permissions(&companions, fs::Permissions::from_mode(0o700)).unwrap();
    let binding_path = companions.join("binding.json");
    let source_path = companions.join("source.json");
    let catalog_path = companions.join("catalog.json");
    let descriptor_path = companions.join("descriptor.json");
    private_json(&binding_path, &packet["binding"]);
    private_json(&source_path, &packet["source_inputs"]);
    private_json(
        &catalog_path,
        &json!({"header":packet["header"],"entity_registry":packet["entities"],"relation_registry":packet["relations"],"lenses":packet["lenses"],"source_order_profile":"source-graph-id-v1"}),
    );
    private_json(&descriptor_path, &packet["descriptor"]);
    invocation["schema_version"] = json!("tos_local_native_metadata_publication_invocation_v1");
    invocation["corpus_store"] = json!(current_store);
    invocation["source_revision"] = json!(current_revision.0.to_prefixed());
    invocation["prepared_database"] = json!(db_path);
    invocation["expected_binding_path"] = json!(binding_path);
    invocation["source_inputs_path"] = json!(source_path);
    invocation["source_inputs_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&source_path).unwrap()).to_prefixed());
    invocation["catalog_path"] = json!(catalog_path);
    invocation["catalog_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&catalog_path).unwrap()).to_prefixed());
    invocation["descriptor_path"] = json!(descriptor_path);
    invocation["descriptor_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&descriptor_path).unwrap()).to_prefixed());
    invocation["publication_limits"] = json!({"operation":{"max_nodes":operation.max_nodes,"max_relations":operation.max_relations,"max_claims":operation.max_claims,"max_bytes":operation.max_bytes,"max_row_bytes":operation.max_row_bytes,"max_contexts":operation.max_contexts,"max_vm_steps":operation.max_vm_steps,"cow_target_bytes":operation.cow_target_bytes},"publication":publication,"catalog":catalog_limits,"semantic":semantic_limits,"bibliographic":{"max_claim_cohort_rows":16,"max_claim_cohort_bytes":16_777_216,"max_output_rows":4096,"max_output_bytes":16_777_216}});
    invocation["reviewed_execution_transition"] = Value::Null;
    invocation["reviewed_metadata_transition"] = Value::Null;
    invocation["expected_creation_receipt_sha256"] = Value::Null;
    invocation["expected_creation_request_digest"] = Value::Null;
    private_json(&invocation_path, &invocation);
    let identity = agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &json!({"action":"describe-metadata-execution"}),
        deadline,
    )["result"]
        .clone();
    assert_eq!(identity["processor"], c_sha.to_hex());
    let deps = &packet["source_inputs"]["dependencies"];
    let mut after_normalization = packet["header"]["normalization_binding"].clone();
    after_normalization["processor_digest"] = identity["processor"].clone();
    invocation["reviewed_execution_transition"] = json!({"dependency_implementation_before":packet["dependency_implementation_before"],"declaration_before":deps["declaration-profile"],"agent_publication_before":deps["agent-publication-profile"],"claim_publication_before":deps.get("claim-publication-profile").unwrap_or(&Value::Null),"normalization_before":packet["header"]["normalization_binding"],"normalization_after":after_normalization,"native_normalization_processor_sha256":identity["processor"],"review_ref":"test:Metadata-native-predecessor-execution-review","reviewed_after_agent_sha256":identity["agent_publication"]});
    private_json(&invocation_path, &invocation);
    let bootstrap = agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &json!({"action":"reviewed-metadata-execution-bootstrap"}),
        deadline,
    )["result"]
        .clone();
    private_json(&binding_path, &bootstrap["binding"]);
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    let source_raw: String = connection
        .query_row(
            "SELECT inputs FROM prepared_source_state WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(source_raw.len() <= 1_048_576);
    let selected_source: Value = serde_json::from_str(&source_raw).unwrap();
    private_json(&source_path, &selected_source);
    private_json(
        &catalog_path,
        &json!({"header":bootstrap["source_header"],"entity_registry":packet["entities"],"relation_registry":packet["relations"],"lenses":packet["lenses"],"source_order_profile":"source-graph-id-v1"}),
    );
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    let predecessor_binding = companions.join("predecessor-binding.json");
    private_json(&predecessor_binding, &bootstrap["binding"]);
    invocation["source_inputs_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&source_path).unwrap()).to_prefixed());
    invocation["catalog_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&catalog_path).unwrap()).to_prefixed());
    invocation["reviewed_execution_transition"] = Value::Null;
    let receipt_path = root
        .join(required(&packet, "source_path"))
        .with_file_name("source-create-receipt.json");
    invocation["expected_creation_receipt_sha256"] =
        json!(native_child::bounded_sha_before(&receipt_path, 1_048_576, deadline).to_prefixed());
    invocation["expected_creation_request_digest"] = created["receipt"]["request_digest"].clone();
    if let Some(before) =
        selected_source["dependencies"].get("metadata-addition-publication-profile")
    {
        invocation["reviewed_metadata_transition"] = json!({"before_sha256":before,"after_sha256":identity["metadata_publication"],"review_ref":"test:Metadata-specific-compatible-execution-review"});
    }
    private_json(&invocation_path, &invocation);
    let published = agent_native_call(&repository,&owner,&invocation_path,&json!({"action":"publish-initial-metadata","recorded_at":created["receipt"]["recorded_at"],"creation_request":request}),deadline)["result"].clone();
    assert_eq!(published["prepared_committed"], true);
    assert_eq!(published["changed_nodes"], 3);
    assert_eq!(published["changed_relations"], 2);
    assert_eq!(agent_authored(&root, deadline), current_files);
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    for kind in ["node", "relation"] {
        let mut statement = connection
            .prepare(&format!(
                "SELECT id,json FROM knowledge_{kind}s ORDER BY id LIMIT 16385"
            ))
            .unwrap();
        let actual: BTreeMap<String, Value> = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|r| {
                let (id, raw) = r.unwrap();
                assert!(raw.len() <= 1_048_576);
                (id, serde_json::from_str(&raw).unwrap())
            })
            .collect();
        let expected: BTreeMap<String, Value> = oracle["expected"][format!("{kind}s")]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (required(r, "id").to_owned(), r.clone()))
            .collect();
        assert!(actual.len() <= 16384);
        assert_eq!(actual, expected);
    }
    assert_eq!(
        published["source_header"]["counts"]["semantic_validation"],
        oracle["expected"]["counts"]["semantic_validation"]
    );
    drop(connection);
    private_json(&binding_path, &published["binding"]);
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    let selected_source_raw: String = connection
        .query_row(
            "SELECT inputs FROM prepared_source_state WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(selected_source_raw.len() <= 1_048_576);
    let selected_source: Value = serde_json::from_str(&selected_source_raw).unwrap();
    private_json(&source_path, &selected_source);
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
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
    access::verify_published_access_until(
        &db_path,
        &binding_path,
        required(&oracle, "new_node_id"),
        required(&oracle, "new_relation_id"),
        deadline,
    );
    let (physical, _) = fixture_physical_bytes(&[workspace.to_owned()], deadline);
    assert!(physical <= 1024 * 1024 * 1024);
    for (path, sha, cap) in [
        (&e, e_sha, 512 * 1024 * 1024),
        (&consumer, c_sha, 512 * 1024 * 1024),
        (&worker_path, w_sha, 128 * 1024 * 1024),
    ] {
        assert_eq!(native_child::bounded_sha_before(path, cap, deadline), sha);
    }
    assert!(Instant::now() < deadline);
}

struct LoadReadinessMcp {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: std::io::BufReader<std::process::ChildStdout>,
}

impl LoadReadinessMcp {
    fn call(&mut self, value: &Value) -> (Value, Vec<u8>) {
        use std::io::{BufRead, Write};
        let mut raw = serde_json::to_vec(value).unwrap();
        raw.push(b'\n');
        self.input.write_all(&raw).unwrap();
        self.input.flush().unwrap();
        let mut response = Vec::new();
        self.output.read_until(b'\n', &mut response).unwrap();
        assert!(response.len() > 1 && response.len() <= 4_194_304);
        assert_eq!(response.last(), Some(&b'\n'));
        let parsed: Value = serde_json::from_slice(&response[..response.len() - 1]).unwrap();
        (parsed, response[..response.len() - 1].to_vec())
    }
}

impl Drop for LoadReadinessMcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn copy_load_fixture_tree(source: &Path, destination: &Path) {
    let metadata = fs::symlink_metadata(source).unwrap();
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        fs::create_dir(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            copy_load_fixture_tree(&entry.path(), &destination.join(entry.file_name()));
        }
    } else {
        assert!(metadata.is_file() && metadata.len() <= 33_554_432);
        fs::copy(source, destination).unwrap();
    }
    fs::set_permissions(
        destination,
        fs::Permissions::from_mode(metadata.mode() & 0o777),
    )
    .unwrap();
}

fn load_readiness_native_call(binary: &Path, invocation: &Path, request: &Value) -> Value {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(binary)
        .args(["source-commands", "--invocation"])
        .arg(invocation)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start exact native source-command binary");
    let mut input = child.stdin.take().unwrap();
    serde_json::to_writer(&mut input, request).unwrap();
    input.write_all(b"\n").unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "native source fixture command refused: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 4_194_304 && output.stderr.len() <= 65_536);
    serde_json::from_slice(&output.stdout).unwrap()
}

fn load_readiness_mcp_process(
    binary: &Path,
    root: &Path,
    inputs: &Path,
    model: &Path,
    binding: &Path,
    working: &Path,
) -> LoadReadinessMcp {
    use std::process::Stdio;
    let mut child = Command::new(binary)
        .arg("--root")
        .arg(root)
        .arg("--source-inputs")
        .arg(inputs)
        .arg("--prepared-read-model")
        .arg(model)
        .arg("--prepared-binding")
        .arg(binding)
        .args(["mcp", "--transport", "stdio"])
        .current_dir(working)
        .env_remove("TOS_RELEASE_ROOT")
        .env_remove("TOS_DATA_ROOT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start selected native MCP query product");
    let input = child.stdin.take().unwrap();
    let output = std::io::BufReader::new(child.stdout.take().unwrap());
    LoadReadinessMcp {
        child,
        input,
        output,
    }
}

fn load_readiness_tool_value(response: &Value) -> Value {
    if let Some(value) = response["result"].get("structuredContent") {
        return value.clone();
    }
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("native MCP tool text result");
    serde_json::from_str(text).unwrap_or_else(|error| {
        panic!("native MCP structured result text: {error}; response={response}")
    })
}

fn fixture_path(path: &Path) -> String {
    path.canonicalize().unwrap().to_str().unwrap().to_owned()
}

#[test]
fn export_protected_native_load_readiness_fixture_when_selected() {
    use std::io::Read;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use tos_foundation::Digest256;

    let Some(output_value) = std::env::var_os("TOS_LOAD_READINESS_EXPORT_DIR") else {
        return;
    };
    let output = PathBuf::from(output_value);
    assert!(
        output.is_absolute(),
        "load-readiness export path is absolute"
    );
    assert!(
        fs::symlink_metadata(&output).is_err(),
        "load-readiness export directory must be new"
    );
    let mut output_builder = fs::DirBuilder::new();
    output_builder.mode(0o700);
    output_builder.create(&output).unwrap();
    let output = output.canonicalize().unwrap();
    let output_metadata = fs::metadata(&output).unwrap();
    assert_eq!(
        output_metadata.uid(),
        fs::metadata("/proc/self").unwrap().uid()
    );
    assert_eq!(output_metadata.mode() & 0o077, 0);
    let baseline = output.join("owner-stage-baseline");
    assert!(!baseline.exists(), "fixture baseline must be fresh");
    fs::create_dir(&baseline).unwrap();
    fs::set_permissions(&baseline, fs::Permissions::from_mode(0o700)).unwrap();

    let query_binary = PathBuf::from(
        std::env::var_os("TOS_LOAD_READINESS_EXPORT_QUERY_BINARY")
            .expect("selected native tos binary for exact MCP response capture"),
    );
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("selected native source-owner command binary"),
    );
    assert!(query_binary.is_absolute() && native.is_absolute());
    assert!(fs::metadata(&query_binary).unwrap().mode() & 0o111 != 0);
    assert!(fs::metadata(&native).unwrap().mode() & 0o111 != 0);

    let deadline = Instant::now() + Duration::from_secs(600);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let (source_root, model, binding_path, source_inputs_path, source_record_id) =
        super::command_claim_publication_cases::publish_agent_load_fixture(&baseline, deadline);

    let forms = baseline.join("forms");
    fs::create_dir(&forms).unwrap();
    fs::set_permissions(&forms, fs::Permissions::from_mode(0o700)).unwrap();
    let source_root_forms = forms.join("source-root");
    fs::create_dir(&source_root_forms).unwrap();
    fs::set_permissions(&source_root_forms, fs::Permissions::from_mode(0o700)).unwrap();
    let (mut source_files, source_owner_bytes, _) =
        super::command_form_cases::fixture_files("composite-v1");
    let mut owner: Value = serde_json::from_slice(&source_owner_bytes).unwrap();
    owner["source_root"] = json!(fixture_path(&source_root_forms));
    owner["uid"] = json!(fs::metadata(&source_root_forms).unwrap().uid());
    for (relative, raw) in &source_files {
        assert!(relative.starts_with("ToS/"));
        let target = source_root_forms.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let owner_path = forms.join("owner.json");
    private_json(&owner_path, &owner);
    let store = forms.join("corpus-store");
    let source_revision = super::validation_cut_cases::write_cut_store(&source_files, &store);

    let component = "rust/crates/tos-command/src/source_forms.rs";
    let mut software_files = BTreeMap::new();
    software_files.insert(
        component.to_owned(),
        fs::read(repository.join(component)).unwrap(),
    );
    let (capture, _software, selected_components) =
        super::command_record_cases::captured_components(
            &software_files,
            deadline,
            &AtomicBool::new(false),
        );
    let capture_selection = capture.selection.clone();
    let capture_path = forms.join("software-capture");
    let restored_path = forms.join("software-restored");
    copy_load_fixture_tree(&capture.capture, &capture_path);
    copy_load_fixture_tree(&capture.restored, &restored_path);
    drop(capture);
    drop(_software);

    let worker = super::validation_cut_cases::selected_worker_path();
    let native_digest = super::command_text_cases::alignment_image_digest(&native);
    let worker_digest = super::command_text_cases::alignment_image_digest(&worker);
    let invocation = json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":fixture_path(&owner_path),
        "owner_context":null,
        "assessment_schema_worker":null,
        "native_executable":fixture_path(&native),
        "native_executable_sha256":native_digest.to_prefixed(),
        "corpus_store":fixture_path(&store),
        "source_revision":source_revision.0.to_prefixed(),
        "original_source_revision":source_revision.0.to_prefixed(),
        "software_capture":fixture_path(&capture_path),
        "software_restored_root":fixture_path(&restored_path),
        "software_selection":{
            "source_git_commit":capture_selection.source_git_commit,
            "source_git_tree":capture_selection.source_git_tree,
            "capture_manifest_sha256":capture_selection.capture_manifest_sha256.to_prefixed()
        },
        "software_components":selected_components.members().map(|member| member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":fixture_path(&worker),"sha256":worker_digest.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
            "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}
    });
    let invocation_path = forms.join("invocation.json");
    private_json(&invocation_path, &invocation);
    let describe_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"describe"
    });
    let sdk_describe = load_readiness_native_call(&native, &invocation_path, &describe_request);
    assert_eq!(sdk_describe["grants_admission"], false);
    let described = sdk_describe["result"].clone();
    let apply_fixture: Value = serde_json::from_slice(
        &fs::read(repository.join("rust/crates/tos-command/tests/fixtures/source_forms_shadow/composite-v1/apply.request.json")).unwrap(),
    )
    .unwrap();

    // Preflight the HTTP semantic response on a separate clone. The exported
    // baseline remains pristine and every load stage can reset by a fresh copy.
    let preflight = output.join("preflight-forms");
    copy_load_fixture_tree(&forms, &preflight);
    let preflight_owner_path = preflight.join("owner.json");
    let preflight_invocation_path = preflight.join("invocation.json");
    let mut preflight_owner: Value =
        serde_json::from_slice(&fs::read(&preflight_owner_path).unwrap()).unwrap();
    preflight_owner["source_root"] = json!(fixture_path(&preflight.join("source-root")));
    private_json(&preflight_owner_path, &preflight_owner);
    let mut preflight_invocation: Value =
        serde_json::from_slice(&fs::read(&preflight_invocation_path).unwrap()).unwrap();
    preflight_invocation["owner_config"] = json!(fixture_path(&preflight_owner_path));
    preflight_invocation["corpus_store"] = json!(fixture_path(&preflight.join("corpus-store")));
    preflight_invocation["software_capture"] =
        json!(fixture_path(&preflight.join("software-capture")));
    preflight_invocation["software_restored_root"] =
        json!(fixture_path(&preflight.join("software-restored")));
    private_json(&preflight_invocation_path, &preflight_invocation);
    let preflight_description = load_readiness_native_call(
        &native,
        &preflight_invocation_path,
        &describe_request,
    )["result"]
        .clone();
    let make_apply = |command_id: String, description: &Value| {
        json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"apply",
            "command_id":command_id,
            "expected_source":description["source"],
            "expected_revision":description["revision"],
            "expected_configuration":description["owner_configuration"],
            "changes":apply_fixture["changes"]
        })
    };
    let preflight_apply = make_apply(
        "tos-load-readiness:preflight".into(),
        &preflight_description,
    );
    let first_apply =
        load_readiness_native_call(&native, &preflight_invocation_path, &preflight_apply);
    assert_eq!(first_apply["result"]["replayed"], false);
    let replayed_apply =
        load_readiness_native_call(&native, &preflight_invocation_path, &preflight_apply);
    assert_eq!(replayed_apply["result"]["replayed"], true);
    fs::remove_dir_all(&preflight).unwrap();
    source_files.clear();

    // Capture exact MCP JSON-RPC response bytes from the selected product. The
    // installed streamable-HTTP route uses the same response serializer.
    let mut mcp = load_readiness_mcp_process(
        &query_binary,
        &source_root,
        &source_inputs_path,
        &model,
        &binding_path,
        &output,
    );
    let (initialized, _) = mcp.call(&json!({
        "jsonrpc":"2.0","id":1,"method":"initialize",
        "params":{"protocolVersion":"2025-11-25","capabilities":{},
            "clientInfo":{"name":"tos-load-readiness-fixture-export","version":"1"}}
    }));
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    use std::io::Write;
    mcp.input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .unwrap();
    mcp.input.flush().unwrap();
    let search_request = json!({
        "jsonrpc":"2.0","id":101,"method":"tools/call",
        "params":{"name":"tos_knowledge_search","arguments":{"mode":"compressed","query":"revision-fixture","limit":64,"sources":["source-navigation"]}}
    });
    let discover_request = json!({
        "jsonrpc":"2.0","id":102,"method":"tools/call",
        "params":{"name":"tos_source_handle_discover","arguments":{"selector":{
            "layer":"metadata_record","record_type":"agent","record_id":source_record_id
        }}}
    });
    let (search_response, search_bytes) = mcp.call(&search_request);
    assert!(search_response.get("result").is_some());
    let search_result = load_readiness_tool_value(&search_response);
    assert!(
        search_result["nodes"]
            .as_array()
            .is_some_and(|nodes| !nodes.is_empty()),
        "selected prepared compressed search returns a real synthetic metadata result"
    );
    // Capture the complete bounded fixture page: paginated cursor expiry is
    // intentionally time-dependent and cannot be a cross-run byte oracle.
    assert!(search_result["page"]["next_cursor"].is_null());
    let (discover_response, discover_bytes) = mcp.call(&discover_request);
    assert!(discover_response.get("result").is_some());
    let handle = load_readiness_tool_value(&discover_response)["handle"].clone();
    assert!(
        handle.is_object(),
        "fixture discovery issues the selected source handle"
    );
    let read_request = json!({
        "jsonrpc":"2.0","id":103,"method":"tools/call",
        "params":{"name":"tos_source_read","arguments":{"handle":handle,"representation":"record"}}
    });
    let (read_response, read_bytes) = mcp.call(&read_request);
    assert!(read_response.get("result").is_some());
    assert_eq!(
        load_readiness_tool_value(&read_response)["status"],
        "available"
    );

    let mut secret = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut secret)
        .unwrap();
    let token_text = secret
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let token_path = baseline.join("synthetic-http-token");
    fs::write(&token_path, format!("{token_text}\n")).unwrap();
    fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();

    let ignored_receipt_paths = vec![
        // The harness checks this digest and receipt against the exact published
        // form-set bytes in its isolated owner stage.
        "/result/revision".to_owned(),
        "/result/owner_configuration".to_owned(),
        "/result/receipt/owner_configuration".to_owned(),
        "/result/receipt/recorded_at".to_owned(),
        "/result/receipt/command_id".to_owned(),
        "/result/receipt/request_digest".to_owned(),
        "/result/receipt/files/source-create-provenance.jsonl/sha256".to_owned(),
    ];
    let conflict_body = json!({"status":"error","code":"owner-conflict","outcome":"unconfirmed"});
    let conflict_sha = Digest256::of_bytes(&serde_json::to_vec(&conflict_body).unwrap()).to_hex();
    let mut sessions = Vec::with_capacity(256);
    for index in 0..256usize {
        let command_id = format!("tos-load-readiness:apply:{index:03}");
        let operations = vec![
            json!({
                "channel":"mcp","label":"indexed-search","request":search_request,
                "expected_status":200,"expected_sha256":Digest256::of_bytes(&search_bytes).to_hex()
            }),
            json!({
                "channel":"mcp","label":"source-handle-discovery","request":discover_request,
                "expected_status":200,"expected_sha256":Digest256::of_bytes(&discover_bytes).to_hex(),
                "reconnect_before":index==0
            }),
            json!({
                "channel":"mcp","label":"exact-source-return","request":read_request,
                "expected_status":200,"expected_sha256":Digest256::of_bytes(&read_bytes).to_hex()
            }),
            json!({
                "channel":"sdk","label":"native-owner-describe","request":describe_request,
                "expected_status":0,"expected_body":sdk_describe,
                "ignored_json_paths":["/result/owner_configuration"],
                "sdk_argv":[fixture_path(&native),"source-commands","--invocation",fixture_path(&invocation_path)]
            }),
            json!({
                "channel":"owner_http","label":"shared-object-apply-race",
                "request":make_apply(command_id, &described),
                "expected_status":200,"expected_body":replayed_apply,
                "ignored_json_paths":ignored_receipt_paths,
                "alternate_outcomes":[{"status":409,"body":conflict_body,"sha256":conflict_sha}],
                "retry_same_command_id":true,"conflict_group":"synthetic-form-apply-v1",
                "bind_expected_configuration_from_owner":true
            }),
        ];
        sessions.push(json!({"id":format!("load-session-{index:03}"),"operations":operations}));
    }
    let schedule = json!({"schema":"tos_protocol_load_schedule_v1","sessions":sessions});
    let schedule_path = output.join("load-readiness-schedule.json");
    // Exact MCP response digests already protect every response byte. Keeping
    // their repeated bodies would exceed the maintained reader's 16 MiB cap.
    let schedule_bytes = serde_json::to_vec(&schedule).unwrap();
    assert!(schedule_bytes.len() <= 16_777_216);
    fs::write(&schedule_path, schedule_bytes).unwrap();
    fs::set_permissions(&schedule_path, fs::Permissions::from_mode(0o600)).unwrap();
    let manifest = json!({
        "schema_version":"tos_load_readiness_fixture_export_v1",
        "classification":"synthetic-conformance-data; no production owner authority or rights",
        "baseline":"owner-stage-baseline",
        "schedule":"load-readiness-schedule.json",
        "sessions":256,
        "operations_per_session":5,
        "stages":[2,8,16,32,64,128,256],
        "query_binary":fixture_path(&query_binary),
        "owner_binary":fixture_path(&native),
        "model":fixture_path(&model),
        "binding":fixture_path(&binding_path),
        "source_root":fixture_path(&source_root),
        "source_inputs":fixture_path(&source_inputs_path),
        "owner_config":fixture_path(&owner_path),
        "invocation":fixture_path(&invocation_path),
        "token_file":fixture_path(&token_path),
        "source_record_id":source_record_id,
        "source_representation":"record",
        "conflict_group":"synthetic-form-apply-v1",
        "retry":"same command id and body; transport creates a fresh nonce",
        "baseline_reset":"tos-load-readiness copies owner-stage-baseline into a fresh output for every invocation"
    });
    let manifest_path = output.join("load-readiness-fixture.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(Instant::now() < deadline);
}

fn connected_browser_mcp_process(
    binary: &Path,
    root: &Path,
    inputs: &Path,
    model: &Path,
    binding: &Path,
    local_text: Option<&Path>,
    working: &Path,
) -> LoadReadinessMcp {
    use std::process::Stdio;
    let mut command = Command::new(binary);
    command
        .arg("--root")
        .arg(root)
        .arg("--source-inputs")
        .arg(inputs)
        .arg("--prepared-read-model")
        .arg(model)
        .arg("--prepared-binding")
        .arg(binding);
    if let Some(path) = local_text {
        command.arg("--source-local-text-selection").arg(path);
    }
    command
        .args(["mcp", "--transport", "stdio"])
        .current_dir(working)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("TOS_") {
            command.env_remove(name);
        }
    }
    let mut child = command.spawn().expect("start source-bound native MCP");
    let input = child.stdin.take().unwrap();
    let output = std::io::BufReader::new(child.stdout.take().unwrap());
    LoadReadinessMcp {
        child,
        input,
        output,
    }
}

fn connected_browser_initialize(mcp: &mut LoadReadinessMcp) {
    use std::io::Write;
    let (initialized, _) = mcp.call(&json!({
        "jsonrpc":"2.0","id":1,"method":"initialize",
        "params":{"protocolVersion":"2025-11-25","capabilities":{},
            "clientInfo":{"name":"tos-connected-source-browser-export","version":"1"}}
    }));
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    mcp.input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .unwrap();
    mcp.input.flush().unwrap();
}

fn connected_browser_tool(
    mcp: &mut LoadReadinessMcp,
    id: u64,
    name: &str,
    arguments: Value,
) -> Value {
    let (response, _) = mcp.call(&json!({
        "jsonrpc":"2.0","id":id,"method":"tools/call",
        "params":{"name":name,"arguments":arguments}
    }));
    assert!(
        response.get("result").is_some(),
        "selected source tool returned a result"
    );
    load_readiness_tool_value(&response)
}

fn connected_browser_selector(value: &Value) {
    assert!(value.is_object());
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ["layer", "record_id", "record_type"].into_iter().collect()
    );
    assert_eq!(value["layer"], "metadata_record");
    assert!(value["record_type"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(value["record_id"].as_str().is_some_and(|s| !s.is_empty()));
}

fn connected_browser_bare_sha(value: &Value) -> String {
    let raw = value.as_str().expect("source owner digest string");
    let bare = raw.strip_prefix("sha256:").unwrap_or(raw);
    assert!(
        bare.len() == 64
            && bare
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
    bare.to_owned()
}

fn connected_browser_graph_node(model: &Path, record_id: &str, version: u64) -> Value {
    let connection =
        rusqlite::Connection::open_with_flags(model, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open selected prepared model read-only");
    let mut statement = connection
        .prepare("SELECT json FROM knowledge_nodes ORDER BY id LIMIT 16385")
        .expect("read maintained graph nodes");
    let mut found = Vec::new();
    for row in statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
    {
        let raw = row.unwrap();
        assert!(raw.len() <= 1_048_576);
        let node: Value = serde_json::from_str(&raw).unwrap();
        if node["source_graph"] == "source-navigation"
            && node["attributes"]["source_record"]["record_id"] == record_id
            && node["attributes"]["source_record"]["record_version"] == version
        {
            found.push(node);
            assert!(
                found.len() <= 1,
                "source record has one graph node for its exact version"
            );
        }
    }
    assert_eq!(
        found.len(),
        1,
        "selected source record has one current owner graph node"
    );
    found.pop().unwrap()
}

fn connected_browser_git_revision(repository: &Path, revision: &str, deadline: Instant) -> String {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", revision]);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0");
    let output = native_child::bounded_output_before(&mut command, 1024, deadline);
    assert!(
        output.status.success(),
        "resolve bounded source Git identity"
    );
    let value = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    assert!(value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()));
    value
}

fn connected_browser_source_members_match_head(
    repository: &Path,
    paths: &BTreeSet<String>,
    deadline: Instant,
) {
    assert!(!paths.is_empty());
    let mut command = Command::new("git");
    command.arg("-C").arg(repository).args([
        "diff",
        "--quiet",
        "--no-ext-diff",
        "--no-textconv",
        "HEAD",
        "--",
    ]);
    for path in paths {
        safe_metadata_fixture_relative(path);
        command.arg(path);
    }
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0");
    let output = native_child::bounded_output_before(&mut command, 4096, deadline);
    assert!(
        output.status.success(),
        "current selected Rust source inputs match repository HEAD"
    );
}

fn connected_browser_capture_synthetic_software(
    workspace: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (
    super::source_cut_cases::SoftwareCaptureFixture,
    SoftwareCaptureReader,
    tos_source_store::SoftwareComponentSelectionV1,
) {
    use tos_source_store::ReadLimits;

    let repository = workspace.join("occurrence-software-evidence-repository");
    assert!(fs::symlink_metadata(&repository).is_err());
    fs::create_dir(&repository).unwrap();
    let mut total_bytes = 0usize;
    for (name, raw) in files {
        safe_metadata_fixture_relative(name);
        assert!(!name.starts_with("ToS/"));
        assert!(raw.len() <= 2_097_152);
        total_bytes = total_bytes.checked_add(raw.len()).unwrap();
        assert!(total_bytes <= 33_554_432);
        let target = repository.join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }

    let git = |arguments: &[&str]| {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&repository)
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "user.name=ToS synthetic fixture",
                "-c",
                "user.email=fixture@invalid",
            ])
            .args(arguments);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
        command
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        let output = native_child::bounded_output_before(&mut command, 4096, deadline);
        assert!(
            output.status.success(),
            "synthetic occurrence software Git operation"
        );
        output.stdout
    };
    git(&["init", "--quiet"]);
    git(&["add", "--all"]);
    git(&[
        "commit",
        "--quiet",
        "-m",
        "synthetic mixed-origin current Rust and historical profile evidence",
    ]);
    let commit = String::from_utf8(git(&["rev-parse", "HEAD^{commit}"]))
        .unwrap()
        .trim()
        .to_owned();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    let prefixes = files.keys().map(String::as_str).collect::<Vec<_>>();
    let capture =
        super::source_cut_cases::captured_software_fixture(&repository, &commit, &prefixes);
    let software = SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 33_554_432,
            json: tos_foundation::JsonLimits::default(),
        },
        deadline,
        cancelled,
    )
    .unwrap();
    let paths = prefixes
        .iter()
        .map(|name| tos_foundation::RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    (capture, software, components)
}

fn connected_browser_create_occurrence(
    repository: &Path,
    workspace: &Path,
    source_root: &Path,
    cut_revision: tos_foundation::SourceRevision,
    occurrence: &Value,
    deadline: Instant,
) -> (Value, Value, Value) {
    use std::os::unix::fs::MetadataExt;

    let cancel = AtomicBool::new(false);
    let owner_command = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("selected native owner"),
    );
    let worker = super::validation_cut_cases::selected_worker_path();
    let create_owner_path = workspace.join("occurrence-owner.json");
    let create_invocation_path = workspace.join("occurrence-create-invocation.json");
    let record_id = occurrence["record_id"].as_str().unwrap();
    let source_path = "ToS/source-witnesses/occurrences/connected-reader/occurrence.json";
    let source_home = source_root
        .join(source_path)
        .parent()
        .unwrap()
        .to_path_buf();
    let source_parent = source_home.parent().unwrap();
    fs::create_dir_all(source_parent).unwrap();
    assert!(
        !source_home.exists(),
        "fresh source.create target home must be absent before owner preview"
    );
    let uid = fs::metadata(source_root).unwrap().uid();
    let owner = json!({
        "schema_version":"tos_local_profile_create_owner_v1",
        "uid":uid,
        "principal_id":"software:connected-reader-fixture",
        "maker_type":"software",
        "source_root":source_root,
        "source_path":source_path,
        "record_id":record_id,
        "authority_ref":"fixture:public-synthetic-native-occurrence",
        "allowed_form_ids":["tos.form.creation.fixture-name"],
        "allowed_operations":["source.create"],
        "expires_at":"2099-01-01T00:00:00Z",
        "provenance_event_id":"tos.event.fixture.connected-reader-occurrence",
        "profile_type_id":"tos.entity.occurrence"
    });
    private_json(&create_owner_path, &owner);

    let mut files = BTreeMap::new();
    for name in super::command_claim_publication_cases::AGENT_RECORD_COMPONENTS {
        files.insert((*name).to_owned(), fs::read(repository.join(name)).unwrap());
    }
    super::command_record_cases::native_metadata_rule_files(
        repository,
        &mut files,
        &[
            "rust/crates/tos-command/src/source_creation.rs",
            "rust/crates/tos-command/src/source_creation_store.rs",
            "rust/crates/tos-command/src/source_serialization.rs",
            "rust/crates/tos-command/src/source_native_creation_cli.rs",
            "rust/crates/tos-command/src/source_creation_cli_selection.rs",
            "rust/crates/tos-command/src/source_forms_publication.rs",
        ],
    );
    let current_source_commit =
        connected_browser_git_revision(repository, "HEAD^{commit}", deadline);
    let current_source_tree = connected_browser_git_revision(repository, "HEAD^{tree}", deadline);
    // The native rule constant contains authored ToS contract files. Keep
    // those bytes in the selected source-item lane, not in the synthetic
    // software capture. The full input set is still verified against HEAD.
    let current_names = files.keys().cloned().collect::<BTreeSet<_>>();
    connected_browser_source_members_match_head(repository, &current_names, deadline);
    let current_source_item_files: BTreeMap<String, Vec<u8>> = files
        .iter()
        .filter(|(path, _)| path.starts_with("ToS/"))
        .map(|(path, raw)| (path.clone(), raw.clone()))
        .collect();
    let current_source_item_members = current_source_item_files
        .iter()
        .map(|(path, raw)| {
            json!({
                "path":path,
                "bytes":raw.len(),
                "sha256":Digest256::of_bytes(raw).to_hex()
            })
        })
        .collect::<Vec<_>>();
    for (path, current_input) in &current_source_item_files {
        let selected_source_item = fs::read(source_root.join(path)).unwrap();
        assert_eq!(
            &selected_source_item, current_input,
            "current authored ToS rule input is present in the selected source cut"
        );
    }
    let current_source_members = files
        .iter()
        .filter(|(path, _)| !path.starts_with("ToS/"))
        .map(|(path, raw)| {
            json!({
                "path":path,
                "bytes":raw.len(),
                "sha256":Digest256::of_bytes(raw).to_hex()
            })
        })
        .collect::<Vec<_>>();
    let current_source_item_bytes = current_source_item_files
        .values()
        .map(Vec::len)
        .sum::<usize>();
    assert!(current_source_item_files.len() <= 512);
    assert!(current_source_item_bytes <= 33_554_432);
    for path in current_source_item_files.keys() {
        assert!(
            files.remove(path).is_some(),
            "split authored ToS source item from software members"
        );
    }
    // Metadata assembly re-reads both source and execution profiles bound by
    // the predecessor catalog. Preserve source-item bindings in the selected
    // source cut and add only missing non-ToS historical bytes to the
    // synthetic capture. Historical evidence is never attributed to, or
    // executed as, current Rust source.
    let selected_source = read_packet(&source_root.join("agent-selected-source.json"));
    let source_catalog: Value = serde_json::from_str(required(
        &selected_source["roots"]["source-catalog"],
        "root_json",
    ))
    .unwrap();
    let fixture_dir = repository.join("tests/conformance/rust/legacy-python-oracles-v1/metadata");
    let provenance = read_packet(&fixture_dir.join("PROVENANCE.json"));
    let historical_commit = required(&provenance["capture"], "source_commit");
    assert!(
        historical_commit.len() == 40 && historical_commit.bytes().all(|b| b.is_ascii_hexdigit())
    );
    let historical_tree = connected_browser_git_revision(
        repository,
        &format!("{historical_commit}^{{tree}}"),
        deadline,
    );
    assert_ne!(current_source_commit, historical_commit);
    assert_ne!(current_source_tree, historical_tree);
    let mut historical_profile_members = Vec::new();
    let mut source_item_bindings = Vec::new();
    let current_member_paths = files.keys().cloned().collect::<BTreeSet<_>>();
    let mut historical_profile_requirements = BTreeMap::<String, (u64, String)>::new();
    let mut historical_profile_paths = BTreeSet::new();
    let mut namespace_bindings = BTreeMap::<String, Vec<Value>>::new();
    for namespace in ["source", "execution"] {
        let bindings = source_catalog["header"]["profile_bindings"][namespace]
            .as_object()
            .unwrap();
        let mut admitted_bindings = Vec::new();
        for (reference, expected) in bindings {
            let relative = safe_metadata_fixture_relative(reference);
            let expected_bytes = expected["bytes"].as_u64().unwrap();
            let expected_sha256 = expected["sha256"].as_str().unwrap();
            assert_eq!(expected_sha256.len(), 64);
            assert!(expected_sha256.bytes().all(|b| b.is_ascii_hexdigit()));
            if reference.starts_with("ToS/") {
                // Source profiles remain source-item bindings in the selected
                // source cut; they are not copied into software_capture.
                assert!(
                    !files.contains_key(reference),
                    "ToS source-item profile does not become a software member"
                );
                let source_item_path = source_root.join(relative);
                let source_item = fs::read(&source_item_path).unwrap();
                assert_eq!(source_item.len() as u64, expected_bytes);
                let source_item_sha256 = Digest256::of_bytes(&source_item).to_hex();
                assert_eq!(source_item_sha256, expected_sha256);
                let binding = json!({
                    "namespace":namespace,
                    "path":reference,
                    "bytes":expected_bytes,
                    "sha256":expected_sha256,
                    "software_member":false,
                    "origin":"selected_source_item"
                });
                source_item_bindings.push(binding.clone());
                admitted_bindings.push(binding);
                continue;
            }
            safe_metadata_fixture_relative(reference);
            if current_member_paths.contains(reference) {
                let current = files.get(reference).unwrap();
                assert_eq!(current.len() as u64, expected_bytes);
                let current_sha256 = Digest256::of_bytes(current).to_hex();
                assert_eq!(
                    current_sha256, expected_sha256,
                    "profile requirement cannot collide with a different current owner member"
                );
                admitted_bindings.push(json!({
                    "namespace":namespace,
                    "path":reference,
                    "bytes":expected_bytes,
                    "sha256":expected_sha256,
                    "software_member":true,
                    "origin":"current_rust_source"
                }));
                continue;
            }
            if let Some((prior_bytes, prior_sha256)) =
                historical_profile_requirements.get(reference)
            {
                assert_eq!(*prior_bytes, expected_bytes);
                assert_eq!(prior_sha256, expected_sha256);
            } else {
                historical_profile_requirements.insert(
                    reference.clone(),
                    (expected_bytes, expected_sha256.to_owned()),
                );
            }
            if !historical_profile_paths.contains(reference) {
                let mut show = Command::new("git");
                show.arg("-C")
                    .arg(repository)
                    .arg("show")
                    .arg(format!("{historical_commit}:{reference}"));
                for (name, _) in std::env::vars_os() {
                    if name.to_string_lossy().starts_with("GIT_") {
                        show.env_remove(name);
                    }
                }
                show.env("GIT_NO_REPLACE_OBJECTS", "1")
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", "/dev/null");
                let output = native_child::bounded_output_before(&mut show, 2_097_152, deadline);
                assert!(output.status.success());
                assert_eq!(output.stdout.len() as u64, expected_bytes);
                let historical_sha256 = Digest256::of_bytes(&output.stdout).to_hex();
                assert_eq!(historical_sha256, expected_sha256);
                historical_profile_members.push(json!({
                    "path":reference,
                    "bytes":output.stdout.len(),
                    "sha256":historical_sha256
                }));
                assert!(historical_profile_paths.insert(reference.clone()));
                assert!(files.insert(reference.clone(), output.stdout).is_none());
            } else {
                let historical = files.get(reference).unwrap();
                assert_eq!(historical.len() as u64, expected_bytes);
                let historical_sha256 = Digest256::of_bytes(historical).to_hex();
                assert_eq!(historical_sha256, expected_sha256);
            }
            admitted_bindings.push(json!({
                "namespace":namespace,
                "path":reference,
                "bytes":expected_bytes,
                "sha256":expected_sha256,
                "software_member":true,
                "origin":"historical_profile_source"
            }));
        }
        namespace_bindings.insert(namespace.to_owned(), admitted_bindings);
    }
    assert_eq!(
        namespace_bindings.get("source").unwrap().len(),
        10,
        "frozen predecessor source bindings"
    );
    assert_eq!(
        namespace_bindings.get("execution").unwrap().len(),
        14,
        "frozen predecessor execution bindings"
    );
    assert_eq!(
        source_item_bindings.len(),
        11,
        "predecessor ToS source-item bindings"
    );
    assert_eq!(
        historical_profile_members.len(),
        13,
        "unique historical non-ToS profile members"
    );
    assert_eq!(
        historical_profile_paths.len(),
        historical_profile_members.len()
    );
    assert!(files.len() <= 512);
    assert!(files.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    super::command_record_cases::materialize_native_fixture_software(source_root, &files);
    let (capture, _software, components) =
        connected_browser_capture_synthetic_software(workspace, &files, deadline, &cancel);
    assert_ne!(capture.selection.source_git_commit, current_source_commit);
    assert_ne!(capture.selection.source_git_commit, historical_commit);
    assert_ne!(capture.selection.source_git_tree, current_source_tree);
    assert_ne!(capture.selection.source_git_tree, historical_tree);
    assert_eq!(
        current_source_members.len() + historical_profile_members.len(),
        files.len(),
        "synthetic capture keeps its two source origins disjoint"
    );
    private_json(
        &workspace.join("occurrence-software-capture-origin-map.json"),
        &json!({
            "schema_version":"tos_connected_browser_mixed_software_capture_origin_map_v1",
            "synthetic_capture":{
                "identity_kind":"synthetic_mixed_origin_composition",
                "source_git_commit":capture.selection.source_git_commit.clone(),
                "source_git_tree":capture.selection.source_git_tree.clone(),
                "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed(),
                "member_count":files.len()
            },
            "current_rust_source":{
                "working_tree_members_match_git_head":true,
                "git_commit":current_source_commit,
                "git_tree":current_source_tree,
                "members":current_source_members
            },
            "current_authored_source_items":{
                "git_commit":current_source_commit,
                "git_tree":current_source_tree,
                "members":current_source_item_members,
                "origin":"selected_current_source_root",
                "selected_cut_bytes_verified":true,
                "software_members":false
            },
            "historical_profile_source":{
                "content_role":"read_only_bound_evidence",
                "git_commit":historical_commit,
                "git_tree":historical_tree,
                "members":historical_profile_members
            },
            "profile_bindings_by_namespace":namespace_bindings,
            "source_item_bindings":source_item_bindings,
            "historical_profile_files_executed":false
        }),
    );
    let capture_path = workspace.join("occurrence-software-capture");
    let restored_path = workspace.join("occurrence-software-restored");
    copy_load_fixture_tree(&capture.capture, &capture_path);
    copy_load_fixture_tree(&capture.restored, &restored_path);

    let mut invocation: Value =
        serde_json::from_slice(&fs::read(workspace.join("agent-native-invocation.json")).unwrap())
            .unwrap();
    let worker_sha = super::command_text_cases::alignment_image_digest(&worker);
    let owner_sha = super::command_text_cases::alignment_image_digest(&owner_command);
    invocation["schema_version"] = json!("tos_local_native_source_invocation_v1");
    invocation["owner_config"] = json!(create_owner_path);
    invocation["native_executable"] = json!(owner_command);
    invocation["native_executable_sha256"] = json!(owner_sha.to_prefixed());
    invocation["source_revision"] = json!(cut_revision.0.to_prefixed());
    // The occurrence binding and its native source closure are added to this
    // exact cut; initial creation must read and preview from that same cut.
    invocation["original_source_revision"] = json!(cut_revision.0.to_prefixed());
    invocation["software_capture"] = json!(capture_path);
    invocation["software_restored_root"] = json!(restored_path);
    invocation["software_selection"] = json!({
        "source_git_commit":capture.selection.source_git_commit,
        "source_git_tree":capture.selection.source_git_tree,
        "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()
    });
    invocation["software_components"] = json!(
        components
            .members()
            .map(|member| member.path.as_str())
            .collect::<Vec<_>>()
    );
    invocation["schema_worker"] = json!({"absolute_path":worker,"sha256":worker_sha.to_prefixed()});
    invocation["assessment_schema_worker"] = Value::Null;
    invocation["budgets"] = json!({
        "max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
        "max_member_bytes":8388608,"max_schema_receipts":128,
        "max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,
        "worker_address_space_bytes":1073741824
    });
    private_json(&create_invocation_path, &invocation);

    assert_eq!(occurrence["visibility"], "public_metadata_only");
    // Occurrence visibility stays within the authored metadata schema. Native
    // text access is governed separately by the conditional-rights selection.
    let record = occurrence.clone();
    let mut request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-create",
        "record":record,
        "forms":[{"form_id":"tos.form.creation.fixture-name","field_id":"metadata.preferred-name"}]
    });
    let preview = super::command_claim_publication_cases::agent_native_call(
        repository,
        &create_owner_path,
        &create_invocation_path,
        &request,
        deadline,
    )["result"]
        .clone();
    assert!(
        !source_home.exists(),
        "prepare-create must not publish an initial source package"
    );
    request["operation"] = json!("source.create");
    request["command_id"] = json!("fixture:connected-reader-occurrence");
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    request["expected_source"] = Value::Null;
    request["expected_revision"] = Value::Null;
    let created = super::command_claim_publication_cases::agent_native_call(
        repository,
        &create_owner_path,
        &create_invocation_path,
        &request,
        deadline,
    )["result"]
        .clone();
    assert_eq!(created["replayed"], false);
    assert_eq!(created["grants_admission"], false);
    assert_eq!(created["record_id"], record_id);
    (request, created, invocation)
}

fn connected_browser_publish_initial_metadata(
    repository: &Path,
    workspace: &Path,
    source_root: &Path,
    db_path: &Path,
    binding_path: &Path,
    source_inputs_path: &Path,
    created_request: &Value,
    created_result: &Value,
    create_invocation: &Value,
    source_revision: tos_foundation::SourceRevision,
    deadline: Instant,
) -> Value {
    use tos_command::source_claim_publication::ClaimPublicationLimits;
    use tos_compiler::{
        local_prepared::PublicationLimits, prepared_catalog_index::CatalogMaintenanceLimits,
        prepared_semantic_index::SemanticMaintenanceLimits,
    };
    use tos_foundation::Digest256;

    let owner = PathBuf::from(create_invocation["owner_config"].as_str().unwrap());
    let invocation_path = workspace.join("occurrence-metadata-invocation.json");
    let binding_companion_path = source_root.join("agent-selected-binding.json");
    let catalog_path = source_root.join("agent-selected-catalog.json");
    let descriptor_path = source_root.join("agent-selected-descriptor.json");
    let mut invocation = create_invocation.clone();
    invocation["schema_version"] = json!("tos_local_native_metadata_publication_invocation_v1");
    invocation["source_revision"] = json!(source_revision.0.to_prefixed());
    invocation["prepared_database"] = json!(db_path);
    invocation["expected_binding_path"] = json!(binding_companion_path);
    invocation["source_inputs_path"] = json!(source_inputs_path);
    invocation["source_inputs_sha256"] =
        json!(Digest256::of_bytes(&fs::read(source_inputs_path).unwrap()).to_prefixed());
    invocation["catalog_path"] = json!(catalog_path);
    invocation["catalog_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&catalog_path).unwrap()).to_prefixed());
    invocation["descriptor_path"] = json!(descriptor_path);
    invocation["descriptor_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&descriptor_path).unwrap()).to_prefixed());
    let operation = ClaimPublicationLimits::default();
    let publication = PublicationLimits {
        max_bytes: 67_108_864,
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
    invocation["publication_limits"] = json!({
        "operation":{
            "max_nodes":operation.max_nodes,"max_relations":operation.max_relations,
            "max_claims":operation.max_claims,"max_bytes":operation.max_bytes,
            "max_row_bytes":operation.max_row_bytes,"max_contexts":operation.max_contexts,
            "max_vm_steps":operation.max_vm_steps,"cow_target_bytes":operation.cow_target_bytes
        },
        "publication":publication,"catalog":catalog_limits,"semantic":semantic_limits,
        "bibliographic":{
            "max_claim_cohort_rows":16,"max_claim_cohort_bytes":16_777_216,
            "max_output_rows":16_384,"max_output_bytes":16_777_216
        }
    });
    invocation["reviewed_execution_transition"] = Value::Null;
    invocation["reviewed_metadata_transition"] = Value::Null;
    let source_path =
        serde_json::from_slice::<Value>(&fs::read(&owner).unwrap()).unwrap()["source_path"]
            .as_str()
            .unwrap()
            .to_owned();
    let receipt_path = source_root
        .join(source_path)
        .with_file_name("source-create-receipt.json");
    invocation["expected_creation_receipt_sha256"] =
        json!(Digest256::of_bytes(&fs::read(receipt_path).unwrap()).to_prefixed());
    invocation["expected_creation_request_digest"] =
        created_result["receipt"]["request_digest"].clone();
    private_json(&invocation_path, &invocation);

    let publication = super::command_claim_publication_cases::agent_native_call(
        repository,
        &owner,
        &invocation_path,
        &json!({
            "action":"publish-initial-metadata",
            "recorded_at":created_result["receipt"]["recorded_at"],
            "creation_request":created_request
        }),
        deadline,
    );
    assert_eq!(publication["grants_admission"], false);
    let published = publication["result"].clone();
    assert_eq!(published["prepared_committed"], true);
    private_json(binding_path, &published["binding"]);
    private_json(&binding_companion_path, &published["binding"]);

    let connection = rusqlite::Connection::open(db_path).unwrap();
    let selected_raw: String = connection
        .query_row(
            "SELECT inputs FROM prepared_source_state WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(selected_raw.len() <= 1_048_576);
    private_json(
        source_inputs_path,
        &serde_json::from_str::<Value>(&selected_raw).unwrap(),
    );
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    let mut catalog: Value = serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    catalog["header"] = published["source_header"].clone();
    private_json(&catalog_path, &catalog);
    assert_eq!(
        invocation["source_revision"],
        json!(source_revision.0.to_prefixed())
    );
    published
}

fn connected_browser_merge_sign_fixture(
    repository: &Path,
    sign_root: &Path,
    source_root: &Path,
    deadline: Instant,
) {
    let (files, modes) = agent_authored_capture(sign_root, deadline, 33_554_432);
    for (name, raw) in files {
        if name == "ToS/source-witnesses/relations/flame-continuity/source-claims.jsonl"
            || name == "ToS/source-witnesses/occurrences/synthetic-flame-1/occurrence.json"
            || name == "ToS/source-witnesses/occurrences/synthetic-flame-2/occurrence.json"
        {
            continue;
        }
        let target = source_root.join(&name);
        if target.exists() {
            assert_eq!(
                fs::read(&target).unwrap(),
                raw,
                "fixture merge never replaces a different owner source file"
            );
            continue;
        }
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, &raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(modes[&name])).unwrap();
    }
    for name in ["LICENSE", "README.md"] {
        let source = repository.join(name);
        let target = source_root.join(name);
        let raw = fs::read(&source).unwrap();
        if target.exists() {
            assert_eq!(fs::read(&target).unwrap(), raw);
        } else {
            fs::write(&target, &raw).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        }
    }
}

fn connected_browser_write_local_selection(
    source_root: &Path,
    workspace: &Path,
    occurrence: &Value,
) -> PathBuf {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_foundation::{CanonicalProfile, JsonLimits, JsonMode};

    let instant = |offset: &str| {
        let output = Command::new("/usr/bin/date")
            .args(["--utc", "--date", offset, "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .unwrap();
        assert!(output.status.success() && output.stdout.len() <= 32);
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    let binding = &occurrence["native_text_binding"];
    let typed_binding = tos_foundation::parse_json(
        &serde_json::to_vec(binding).unwrap(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    let binding_sha256 = Digest256::of_bytes(
        &tos_foundation::canonical_bytes_v1(
            &typed_binding,
            CanonicalProfile::SourceCommandInputV1,
            JsonLimits::default(),
        )
        .unwrap(),
    )
    .to_hex();
    let layer_ref = binding["text_layer"]["record_ref"].as_str().unwrap();
    let layer: Value =
        serde_json::from_slice(&fs::read(source_root.join(layer_ref)).unwrap()).unwrap();
    let rights_record_refs = layer["representation"]["rights_record_refs"].clone();
    let notices = [
        ("LICENSE", "license"),
        ("README.md", "attribution"),
    ]
    .into_iter()
    .map(|(reference, role)| {
        json!({
            "ref":reference,
            "sha256":Digest256::of_bytes(&fs::read(source_root.join(reference)).unwrap()).to_hex(),
            "role":role
        })
    })
    .collect::<Vec<_>>();
    let mandate_path = workspace.join("connected-reader-local-read-mandate.md");
    let mandate = b"Fixture-only local read selection for exact synthetic public text. No external publication or redistribution permission is asserted.\n";
    fs::write(&mandate_path, mandate).unwrap();
    fs::set_permissions(&mandate_path, fs::Permissions::from_mode(0o600)).unwrap();
    let selection_path = workspace.join("source-local-text-selection.json");
    let selection = json!({
        "schema_version":"tos_native_local_text_read_v1",
        "source_root":source_root,
        "owner_uid":fs::metadata(source_root).unwrap().uid(),
        "issuer":"software:connected-reader-fixture",
        "mandate":{
            "path":mandate_path,
            "sha256":Digest256::of_bytes(mandate).to_hex(),
            "scope":"Exact conditional-rights synthetic unit read for conformance only"
        },
        "issued_at":instant("1 minute ago"),
        "expires_at":instant("1 hour"),
        "external_publication_authorized":false,
        "selections":[{
            "binding_sha256":binding_sha256,
            "rights_record_refs":rights_record_refs,
            "condition_review":"Synthetic fixture rights are authorized_with_conditions; this selection permits only owner-local exact-unit reading and does not authorize external publication.",
            "notices":notices
        }]
    });
    private_json(&selection_path, &selection);
    selection_path
}

fn connected_browser_produce_pair(
    repository: &Path,
    output: &Path,
    query_binary: &Path,
    deadline: Instant,
) -> (Value, Value, String, Value) {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use tos_foundation::{Digest256, SourceRevision};

    let stage = output.parent().unwrap().join(format!(
        "{}.owner-stage",
        output.file_name().unwrap().to_string_lossy()
    ));
    assert!(
        fs::symlink_metadata(&stage).is_err(),
        "owner fixture stage must be new"
    );
    let mut stage_builder = fs::DirBuilder::new();
    stage_builder.mode(0o700);
    stage_builder.create(&stage).unwrap();
    let baseline = stage.join("owner-stage-baseline");
    fs::create_dir(&baseline).unwrap();
    fs::set_permissions(&baseline, fs::Permissions::from_mode(0o700)).unwrap();

    let (source_root, db_path, binding_path, source_inputs_path, stale_record_id) =
        super::command_claim_publication_cases::publish_agent_load_fixture(&baseline, deadline);
    let source_root = source_root.canonicalize().unwrap();
    let db_path = db_path.canonicalize().unwrap();
    let binding_path = binding_path.canonicalize().unwrap();
    let source_inputs_path = source_inputs_path.canonicalize().unwrap();
    assert!(source_root.starts_with(&stage));
    assert!(db_path.starts_with(&source_root));
    assert!(
        binding_path.starts_with(&baseline) && !binding_path.starts_with(&source_root),
        "prepared binding is a private owner-stage input outside the authored source root"
    );
    assert!(source_inputs_path.starts_with(&source_root));

    let stale_id = stale_record_id.as_str().unwrap();
    let stale_selector = json!({
        "layer":"metadata_record","record_type":"agent","record_id":stale_id
    });
    let mut previous = connected_browser_mcp_process(
        query_binary,
        &source_root,
        &source_inputs_path,
        &db_path,
        &binding_path,
        None,
        &stage,
    );
    connected_browser_initialize(&mut previous);
    let old_discovery = connected_browser_tool(
        &mut previous,
        2,
        "tos_source_handle_discover",
        json!({"selector":stale_selector}),
    );
    assert_eq!(old_discovery["status"], "available");
    let stale_handle = old_discovery["handle"].clone();
    assert!(stale_handle.is_object());
    let previous_revision = connected_browser_bare_sha(&stale_handle["epoch"]["source_revision"]);
    assert_eq!(
        previous_revision,
        connected_browser_bare_sha(&old_discovery["source_revision"])
    );
    drop(previous);

    let epoch_a = stage.join("epoch-a");
    fs::create_dir(&epoch_a).unwrap();
    fs::set_permissions(&epoch_a, fs::Permissions::from_mode(0o700)).unwrap();
    let epoch_a_source_root = epoch_a.join("source-root");
    let epoch_a_inputs = epoch_a.join("source-inputs.json");
    let epoch_a_model = epoch_a.join("prepared-read-model.sqlite");
    let epoch_a_binding = epoch_a.join("prepared-binding.json");
    copy_load_fixture_tree(&source_root, &epoch_a_source_root);
    fs::copy(&source_inputs_path, &epoch_a_inputs).unwrap();
    assert!(fs::metadata(&db_path).unwrap().len() <= 512 * 1024 * 1024);
    fs::copy(&db_path, &epoch_a_model).unwrap();
    fs::copy(&binding_path, &epoch_a_binding).unwrap();
    fs::set_permissions(&epoch_a_inputs, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&epoch_a_model, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&epoch_a_binding, fs::Permissions::from_mode(0o600)).unwrap();
    let source_inputs_a_sha = Digest256::of_bytes(&fs::read(&epoch_a_inputs).unwrap()).to_hex();
    let binding_a_sha = Digest256::of_bytes(&fs::read(&epoch_a_binding).unwrap()).to_hex();
    let model_a_sha =
        native_child::bounded_sha_before(&epoch_a_model, 512 * 1024 * 1024, deadline).to_hex();
    assert_eq!(
        model_a_sha,
        native_child::bounded_sha_before(&db_path, 512 * 1024 * 1024, deadline).to_hex(),
        "epoch-A prepared model snapshot is byte-exact"
    );
    let source_inventory_a_sha = {
        let (files, modes) = agent_authored_capture(&epoch_a_source_root, deadline, 33_554_432);
        let rows = files.iter().map(|(path, raw)| json!({
            "path":path,"sha256":Digest256::of_bytes(raw).to_hex(),"bytes":raw.len(),"mode":modes[path]
        })).collect::<Vec<_>>();
        Digest256::of_bytes(&canonical_lf(&json!(rows))).to_hex()
    };
    let agent_publication_invocation: Value = serde_json::from_slice(
        &fs::read(source_root.join("agent-publication-invocation.json")).unwrap(),
    )
    .unwrap();
    let cut_a = agent_publication_invocation["source_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let corpus_store = PathBuf::from(
        serde_json::from_slice::<Value>(
            &fs::read(baseline.join("agent-native-invocation.json")).unwrap(),
        )
        .unwrap()["corpus_store"]
            .as_str()
            .unwrap(),
    );
    let epoch_a_manifest = json!({
        "source_revision":previous_revision,
        "source_root_path_at_issue":fixture_path(&source_root),
        "source_root_snapshot":fixture_path(&epoch_a_source_root),
        "source_inputs_snapshot":{"path":fixture_path(&epoch_a_inputs),"sha256":source_inputs_a_sha},
        "prepared_binding_snapshot":{"path":fixture_path(&epoch_a_binding),"sha256":binding_a_sha},
        "prepared_read_model_snapshot":{"path":fixture_path(&epoch_a_model),"sha256":model_a_sha},
        "source_inventory_sha256":source_inventory_a_sha,
        "corpus_cut_revision":cut_a,
        "corpus_store":fixture_path(&corpus_store)
    });

    let sign_workspace = stage.join("public-sign-fixture");
    let sign = super::native_public_assessment_fixture::native_public_sign_fixture(
        repository,
        &sign_workspace,
    )
    .unwrap();
    // Epoch A retains the exact older contract and its issued handles. Epoch B
    // adds sign claims using the current schema's provenance fields, so record
    // that one explicit contract transition before capturing the successor cut.
    let index_contract = "ToS/contracts/tos-corpus-index.schema.json";
    assert_eq!(
        fs::read(source_root.join(index_contract)).unwrap(),
        fs::read(epoch_a_source_root.join(index_contract)).unwrap(),
    );
    let current_contract = fs::read(repository.join(index_contract)).unwrap();
    assert_eq!(
        fs::read(sign.public_root.join(index_contract)).unwrap(),
        current_contract,
    );
    fs::write(source_root.join(index_contract), current_contract).unwrap();
    connected_browser_merge_sign_fixture(repository, &sign.public_root, &source_root, deadline);
    let occurrence_path = "ToS/source-witnesses/occurrences/synthetic-flame-1/occurrence.json";
    let occurrence: Value =
        serde_json::from_slice(&fs::read(sign.public_root.join(occurrence_path)).unwrap()).unwrap();
    assert_eq!(occurrence["record_id"], sign.occurrence_ids[0]);

    let (current_files, current_modes) = agent_authored_capture(&source_root, deadline, 33_554_432);
    let cut_base = SourceRevision(Digest256::from_prefixed(&cut_a).unwrap());
    let cut_with_sign = super::validation_cut_cases::write_cut_store_with_optional_modes(
        &current_files,
        &corpus_store,
        Some(cut_base),
        Some(&current_modes),
    );

    let (create_request, created, create_invocation) = connected_browser_create_occurrence(
        repository,
        &baseline,
        &source_root,
        cut_with_sign,
        &occurrence,
        deadline,
    );
    let (after_create_files, after_create_modes) =
        agent_authored_capture(&source_root, deadline, 33_554_432);
    let cut_after_create = super::validation_cut_cases::write_cut_store_with_optional_modes(
        &after_create_files,
        &corpus_store,
        Some(cut_with_sign),
        Some(&after_create_modes),
    );
    let local_selection =
        connected_browser_write_local_selection(&source_root, &stage, &occurrence);
    connected_browser_publish_initial_metadata(
        repository,
        &baseline,
        &source_root,
        &db_path,
        &binding_path,
        &source_inputs_path,
        &create_request,
        &created,
        &create_invocation,
        cut_after_create,
        deadline,
    );

    let pair = |root: &Path, inputs: &Path, model: &Path, binding: &Path, local: Option<&Path>| {
        json!({
            "source_root":fixture_path(root),
            "source_inputs":fixture_path(inputs),
            "model":fixture_path(model),
            "binding":fixture_path(binding),
            "local_text_selection":local.map(fixture_path)
        })
    };
    let query_inputs = json!({
        "schema_version":"tos_connected_source_browser_export_request_v1",
        "query_binary":fixture_path(query_binary),
        "current":pair(&source_root,&source_inputs_path,&db_path,&binding_path,Some(&local_selection)),
        "previous":pair(&epoch_a_source_root,&epoch_a_inputs,&epoch_a_model,&epoch_a_binding,None),
        "metadata_selector":{"layer":"metadata_record","record_type":"occurrence","record_id":occurrence["record_id"]},
        "allowed_native_selector":{"layer":"metadata_record","record_type":"occurrence","record_id":occurrence["record_id"]},
        "allowed_native_representation":"native_local_unit",
        "restricted_native_selector":{"layer":"metadata_record","record_type":"occurrence","record_id":occurrence["record_id"]},
        "stale_selector":stale_selector
    });
    (
        query_inputs,
        stale_handle,
        previous_revision,
        epoch_a_manifest,
    )
}

#[test]
fn export_connected_source_browser_inputs_when_selected() {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use tos_foundation::Digest256;

    let Some(output_value) = std::env::var_os("TOS_CONNECTED_SOURCE_BROWSER_EXPORT_DIR") else {
        return;
    };
    let output = PathBuf::from(output_value);
    assert!(output.is_absolute());
    assert!(fs::symlink_metadata(&output).is_err());
    let mut output_builder = fs::DirBuilder::new();
    output_builder.mode(0o700);
    output_builder.create(&output).unwrap();
    let output = output.canonicalize().unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().uid(),
        fs::metadata("/proc/self").unwrap().uid()
    );
    assert_eq!(fs::metadata(&output).unwrap().mode() & 0o077, 0);

    let query_binary = PathBuf::from(
        std::env::var_os("TOS_LOAD_READINESS_EXPORT_QUERY_BINARY")
            .expect("selected installed source reader product"),
    );
    assert!(query_binary.is_absolute());
    assert!(fs::metadata(&query_binary).unwrap().mode() & 0o111 != 0);
    let owner_command = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("selected native source owner"),
    );
    let prepared_consumer = PathBuf::from(
        std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
            .expect("selected prepared source consumer"),
    );
    assert!(owner_command.is_absolute() && prepared_consumer.is_absolute());
    for path in [&owner_command, &prepared_consumer] {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert!(metadata.mode() & 0o111 != 0);
        assert_eq!(path.canonicalize().unwrap(), *path);
    }
    let deadline = Instant::now() + Duration::from_secs(900);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let (request, stale_handle, previous_revision, previous_epoch) =
        connected_browser_produce_pair(&repository, &output, &query_binary, deadline);
    assert_eq!(
        request["schema_version"],
        "tos_connected_source_browser_export_request_v1"
    );
    assert_eq!(
        request
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        [
            "schema_version",
            "query_binary",
            "current",
            "previous",
            "metadata_selector",
            "allowed_native_selector",
            "allowed_native_representation",
            "restricted_native_selector",
            "stale_selector"
        ]
        .into_iter()
        .collect()
    );
    for key in [
        "metadata_selector",
        "allowed_native_selector",
        "restricted_native_selector",
        "stale_selector",
    ] {
        connected_browser_selector(&request[key]);
    }
    let allowed_representation = request["allowed_native_representation"].as_str().unwrap();
    assert!(["native_public_unit", "native_local_unit"].contains(&allowed_representation));

    let pair_paths = |name: &str| -> (PathBuf, PathBuf, PathBuf, PathBuf, Option<PathBuf>) {
        let pair = &request[name];
        assert_eq!(
            pair.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            [
                "source_root",
                "source_inputs",
                "model",
                "binding",
                "local_text_selection"
            ]
            .into_iter()
            .collect()
        );
        let file = |field: &str| {
            let path = PathBuf::from(pair[field].as_str().expect("absolute selected path"));
            assert!(path.is_absolute(), "{name}.{field} must be absolute");
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            assert_eq!(
                path.canonicalize().unwrap(),
                path,
                "{name}.{field} is canonical"
            );
            path
        };
        let root = PathBuf::from(pair["source_root"].as_str().unwrap());
        assert!(root.is_absolute());
        let root_meta = fs::symlink_metadata(&root).unwrap();
        assert!(root_meta.is_dir() && !root_meta.file_type().is_symlink());
        assert_eq!(
            root.canonicalize().unwrap(),
            root,
            "{name}.source_root is canonical"
        );
        let local = pair["local_text_selection"].as_str().map(|value| {
            let path = PathBuf::from(value);
            assert!(path.is_absolute());
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            assert_eq!(
                path.canonicalize().unwrap(),
                path,
                "local text selection is canonical"
            );
            path
        });
        (
            root,
            file("source_inputs"),
            file("model"),
            file("binding"),
            local,
        )
    };
    let (current_root, current_inputs, current_model, current_binding, current_local) =
        pair_paths("current");
    let (previous_root, previous_inputs, previous_model, previous_binding, previous_local) =
        pair_paths("previous");
    if allowed_representation == "native_local_unit" {
        assert!(
            current_local.is_some(),
            "local native unit requires exact owner local selection"
        );
    }

    let binary = PathBuf::from(request["query_binary"].as_str().unwrap());
    assert!(binary.is_absolute());
    let binary_metadata = fs::symlink_metadata(&binary).unwrap();
    assert!(binary_metadata.is_file() && !binary_metadata.file_type().is_symlink());
    assert!(binary_metadata.mode() & 0o111 != 0);
    let schema_worker = super::validation_cut_cases::selected_worker_path();

    for root in [&current_root, &previous_root] {
        assert!(
            !output.starts_with(root) && !root.starts_with(&output),
            "browser export output is disjoint from selected source roots"
        );
    }

    let mut current = connected_browser_mcp_process(
        &binary,
        &current_root,
        &current_inputs,
        &current_model,
        &current_binding,
        current_local.as_deref(),
        &output,
    );
    connected_browser_initialize(&mut current);
    let metadata_discovery = connected_browser_tool(
        &mut current,
        2,
        "tos_source_handle_discover",
        json!({"selector":request["metadata_selector"]}),
    );
    assert_eq!(metadata_discovery["status"], "available");
    let current_revision = connected_browser_bare_sha(&metadata_discovery["source_revision"]);
    assert_ne!(
        current_revision, previous_revision,
        "older source pair has a distinct source epoch"
    );
    let metadata_handle = metadata_discovery["handle"].clone();
    let metadata_result = connected_browser_tool(
        &mut current,
        3,
        "tos_source_read",
        json!({"handle":metadata_handle,"representation":"record"}),
    );
    assert_eq!(metadata_result["status"], "available");
    assert_eq!(metadata_result["grants_current_use"], false);
    assert_eq!(metadata_result["writes_to_source"], false);

    let mut readcases = vec![json!({
        "name":"current-metadata-record","selector":request["metadata_selector"],"representation":"record",
        "expected_discovery_status":"available","expected_read_status":"available"
    })];
    let mut browser_map_cases = Vec::new();
    for (id, name, selector, representation, expected_status) in [
        (
            4u64,
            "allowed-native-unit",
            &request["allowed_native_selector"],
            allowed_representation,
            "available",
        ),
        (
            6u64,
            "restricted-public-unit",
            &request["restricted_native_selector"],
            "native_public_unit",
            "access-restricted",
        ),
    ] {
        let discovery = connected_browser_tool(
            &mut current,
            id,
            "tos_source_handle_discover",
            json!({"selector":selector}),
        );
        assert_eq!(
            discovery["status"], "available",
            "{name} selected current source metadata"
        );
        assert_eq!(
            connected_browser_bare_sha(&discovery["source_revision"]),
            current_revision
        );
        let handle = discovery["handle"].clone();
        let read_result = connected_browser_tool(
            &mut current,
            id + 1,
            "tos_source_read",
            json!({"handle":handle,"representation":representation}),
        );
        assert_eq!(
            read_result["status"], expected_status,
            "{name} has owner-selected read status"
        );
        assert_eq!(read_result["grants_current_use"], false);
        assert_eq!(read_result["writes_to_source"], false);
        if expected_status == "access-restricted" {
            assert!(read_result["native_unit"].is_null() && read_result["text_access"].is_null());
        } else {
            assert!(
                read_result["native_unit"].is_object(),
                "allowed native read returns exact owner spans"
            );
        }
        let target = &handle["target"];
        let reference = &target["record_ref"];
        let record_id = reference["id"].as_str().expect("exact owner record target");
        let version = reference["version"]
            .as_u64()
            .expect("exact owner record version");
        assert_eq!(record_id, selector["record_id"]);
        let node = connected_browser_graph_node(&current_model, record_id, version);
        browser_map_cases.push(json!({
            "case_name":name,
            "selection":{"kind":"node","id":node["id"],"source_revision":current_revision,
                "content_revision":connected_browser_bare_sha(&node["content_revision"])}
        }));
        readcases.push(json!({
            "name":name,"selector":selector,"representation":representation,
            "expected_discovery_status":"available","expected_read_status":expected_status
        }));
    }

    let stale_result = connected_browser_tool(
        &mut current,
        9,
        "tos_source_read",
        json!({"handle":stale_handle,"representation":"record"}),
    );
    assert_eq!(
        stale_result["status"], "stale",
        "the handle issued on epoch A is refused on the same owner's epoch B"
    );
    assert_eq!(stale_result["grants_current_use"], false);
    assert_eq!(stale_result["writes_to_source"], false);
    assert!(stale_result["record"].is_null());
    drop(current);

    let readcases_value = json!({"cases":readcases,"stale_reads":[{
        "name":"previous-epoch-owner-handle","handle":stale_handle,"representation":"record","expected_status":"stale"
    }]});
    let browser_map = json!({"schema_version":"tos_connected_source_browser_cases_v1",
        "source_revision":current_revision,"cases":browser_map_cases});
    let write = |name: &str, value: &Value| {
        let path = output.join(name);
        private_json(&path, value);
        (
            fixture_path(&path),
            Digest256::of_bytes(&fs::read(path).unwrap()).to_hex(),
        )
    };
    let readcases_pin = write("source-read-cases.json", &readcases_value);
    let browser_map_pin = write("browser-map.json", &browser_map);
    let hash_file =
        |path: &Path, cap: u64| native_child::bounded_sha_before(path, cap, deadline).to_hex();
    let manifest = json!({
        "schema_version":"tos_connected_source_browser_export_v1",
        "classification":"owner-selected local conformance inputs; no admission, current-use grant or external publication",
        "query_binary":{"path":fixture_path(&binary),"sha256":hash_file(&binary,536_870_912)},
        "native_source_owner":{"path":fixture_path(&owner_command),"sha256":hash_file(&owner_command,536_870_912)},
        "prepared_source_consumer":{"path":fixture_path(&prepared_consumer),"sha256":hash_file(&prepared_consumer,536_870_912)},
        "schema_worker":{"path":fixture_path(&schema_worker),"sha256":hash_file(&schema_worker,134_217_728)},
        "previous_source_epoch":previous_epoch,
        "current_source_epoch":{"source_revision":current_revision,
            "source_root":fixture_path(&current_root),
            "source_inputs":fixture_path(&current_inputs),
            "source_inputs_sha256":hash_file(&current_inputs,1_073_741_824),
            "prepared_read_model":fixture_path(&current_model),
            "prepared_read_model_sha256":hash_file(&current_model,4_294_967_296),
            "prepared_binding":fixture_path(&current_binding),
            "prepared_binding_sha256":hash_file(&current_binding,1_048_576)},
        "local_text_selection":current_local.as_deref().map(|path| json!({
            "path":fixture_path(path),"sha256":hash_file(path,262_144)
        })),
        "readcases":{"path":readcases_pin.0,"sha256":readcases_pin.1},
        "browser_map":{"path":browser_map_pin.0,"sha256":browser_map_pin.1},
        "cases":{"metadata_record_id":request["metadata_selector"]["record_id"],
            "allowed_native_record_id":request["allowed_native_selector"]["record_id"],
            "allowed_native_representation":allowed_representation,
            "restricted_native_record_id":request["restricted_native_selector"]["record_id"],
            "stale_record_id":request["stale_selector"]["record_id"]}
    });
    private_json(&output.join("export-manifest.json"), &manifest);
    assert!(Instant::now() < deadline);
}
