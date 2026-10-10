//! One retained historical Claim owner lifecycle over a frozen fixture packet
//! and direct native CLI calls. Synthetic inputs grant no admission.
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_source_store::{ReadLimits, SoftwareCaptureReader};

#[derive(Default)]
struct ProcessLedger {
    git_spawns: usize,
    native_cli_spawns: usize,
}

fn canonical(value: &Value) -> Vec<u8> {
    tos_foundation::canonical_raw_bytes_v1(
        &serde_json::to_vec(value).unwrap(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
    )
    .unwrap()
}

fn bounded_process(
    command: &mut Command,
    deadline: Instant,
    output_cap: u64,
    error_cap: u64,
    ledger: &mut ProcessLedger,
) -> (std::process::ExitStatus, Vec<u8>, Vec<u8>) {
    assert!(Instant::now() < deadline);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") || key == "PYTHONPATH" || key == "PYTHONHOME" {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        // Bound trusted Git mappings after foreign GIT_* inputs are removed.
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "core.packedGitWindowSize")
        .env("GIT_CONFIG_VALUE_0", "16m")
        .env("GIT_CONFIG_KEY_1", "core.packedGitLimit")
        .env("GIT_CONFIG_VALUE_1", "64m");
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(errors.try_clone().unwrap()))
        .spawn()
        .unwrap();
    let step_deadline = deadline.min(Instant::now() + Duration::from_secs(75));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step_deadline
            || output.metadata().unwrap().len() > output_cap
            || errors.metadata().unwrap().len() > error_cap
        {
            let _ = Command::new("/usr/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            panic!("bounded Legacy Claim subprocess refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step_deadline);
    assert!(output.metadata().unwrap().len() <= output_cap);
    assert!(errors.metadata().unwrap().len() <= error_cap);
    output.seek(SeekFrom::Start(0)).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut output_bytes = Vec::new();
    let mut error_bytes = Vec::new();
    output.read_to_end(&mut output_bytes).unwrap();
    errors.read_to_end(&mut error_bytes).unwrap();
    (status, output_bytes, error_bytes)
}

fn source_members(root: &Path, deadline: Instant) -> (BTreeMap<String, Vec<u8>>, u64) {
    let mut directories = vec![root.join("ToS")];
    let mut entries = 0usize;
    let mut directories_seen = 0usize;
    let mut count = 0usize;
    let mut bytes = 0u64;
    while let Some(directory) = directories.pop() {
        assert!(Instant::now() < deadline);
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            entries = entries.checked_add(1).unwrap();
            assert!(entries <= 4352);
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            let path = entry.path();
            let reference = path.strip_prefix(root).unwrap().to_str().unwrap();
            assert!(reference.len() <= 512 && reference.split('/').count() <= 16);
            if kind.is_dir() {
                if tos_source_store::has_authored_source_descendants_v1(reference) {
                    directories_seen = directories_seen.checked_add(1).unwrap();
                    assert!(directories_seen <= 4096);
                    directories.push(path);
                }
            } else if tos_source_store::is_authored_source_path_v1(reference)
                && reference != "ToS/source-witnesses/.historical-create.writer.lock"
            {
                assert!(kind.is_file());
                let size = entry.metadata().unwrap().len();
                assert!(size <= 8_388_608);
                count = count.checked_add(1).unwrap();
                bytes = bytes.checked_add(size).unwrap();
                assert!(count <= 256 && bytes <= 8_388_608);
            }
        }
    }
    let mut files = super::command_text_cases::authored_text_files(root);
    files.remove("ToS/source-witnesses/.historical-create.writer.lock");
    assert_eq!(files.len(), count);
    assert_eq!(
        files.values().map(|raw| raw.len() as u64).sum::<u64>(),
        bytes
    );
    (files, bytes)
}

fn charge_software(names: &[String], repository: &Path, total: u64, files: usize) -> (u64, usize) {
    let mut total = total;
    let mut files = files;
    for name in names {
        assert!(name.len() <= 512 && name.split('/').count() <= 16);
        let metadata = fs::symlink_metadata(repository.join(name)).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        let size = metadata.len();
        assert!(size <= 8_388_608);
        total = total.checked_add(size).unwrap();
        files = files.checked_add(1).unwrap();
        assert!(files <= 256 && total <= 8_388_608);
    }
    (total, files)
}

fn selected_software(
    repository: &Path,
    names: &[String],
    deadline: Instant,
    ledger: &mut ProcessLedger,
) -> (
    super::source_cut_cases::SoftwareCaptureFixture,
    SoftwareCaptureReader,
    tos_source_store::SoftwareComponentSelectionV1,
) {
    let output = {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(repository)
            .args(["rev-parse", "HEAD^{commit}"]);
        ledger.git_spawns += 1;
        bounded_process(&mut command, deadline, 1_048_576, 1_048_576, ledger)
    };
    assert!(output.0.success());
    let commit = String::from_utf8(output.1).unwrap().trim().to_owned();
    assert!(
        commit.len() == 40
            && commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );

    let temporary = tempfile::tempdir().unwrap();
    let capture = temporary.path().join("software-capture");
    let restored = temporary.path().join("software-restored");
    let prefixes = names.iter().map(String::as_str).collect::<Vec<_>>();
    let cancelled = AtomicBool::new(false);
    let selection = super::source_cut_cases::capture_software_archive(
        repository, &commit, &prefixes, &capture, deadline, &cancelled,
    );
    super::source_cut_cases::restore_software_archive(
        &capture, &restored, &selection, deadline, &cancelled,
    );
    let fixture = super::source_cut_cases::SoftwareCaptureFixture {
        temporary,
        capture,
        restored,
        selection,
    };
    let software = SoftwareCaptureReader::open(
        &fixture.capture,
        &fixture.restored,
        fixture.selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let paths = names
        .iter()
        .map(|name| tos_foundation::RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    assert_eq!(components.members().count(), names.len());
    for name in names {
        let path = tos_foundation::RelativePath::parse(name).unwrap();
        let captured = software
            .read_selected_component(&components, &path, 8_388_608, deadline, &cancelled)
            .unwrap();
        assert_eq!(
            captured,
            fs::read(repository.join(name)).unwrap(),
            "selected source {name}"
        );
    }
    (fixture, software, components)
}

fn comparable(value: &Value, ignore_revision: bool) -> Value {
    let mut normalized = value.clone();
    if let Some(object) = normalized.as_object_mut() {
        // Actual configuration digests are independently checked against the
        // relocated protected owner (and selected contracts for form owners).
        object.remove("owner_configuration");
        if ignore_revision {
            object.remove("revision");
        }
        if let Some(receipt) = object.get_mut("receipt").and_then(Value::as_object_mut) {
            receipt.remove("recorded_at");
            receipt.remove("owner_configuration");
        }
    }
    normalized
}

fn request_digest(value: &Value) -> String {
    let raw = tos_foundation::canonical_raw_bytes_v1(
        &serde_json::to_vec(value).unwrap(),
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits::default(),
    )
    .unwrap();
    tos_foundation::Digest256::of_bytes(&raw).to_prefixed()
}

fn archive_reference(record_id: &str, revision: &str) -> String {
    let revision = revision.strip_prefix("sha256:").unwrap();
    assert_eq!(revision.len(), 64);
    format!(
        "ToS/source-witnesses/.record-revisions/{}-{revision}",
        tos_foundation::Digest256::of_bytes(record_id.as_bytes()).to_hex()
    )
}

fn assert_oracle(actual: &Value, expected: &Value, ignore_revision: bool, label: &str) {
    assert_eq!(
        comparable(actual, ignore_revision),
        comparable(expected, ignore_revision),
        "native Legacy Claim {label} differs from maintained oracle"
    );
}

fn assert_form_mutation(actual: &Value, expected: &Value, request: &Value, label: &str) {
    let digest = request_digest(request);
    let expected_revision = &request["expected_revision"];
    assert_eq!(actual["receipt"]["request_digest"], digest);
    assert_eq!(actual["receipt"]["previous_revision"], *expected_revision);
    assert_eq!(
        actual["receipt"]["owner_configuration"],
        request["expected_configuration"]
    );
    let mut expected = expected.clone();
    expected["receipt"]["request_digest"] = Value::String(digest);
    expected["receipt"]["previous_revision"] = expected_revision.clone();
    assert_oracle(actual, &expected, true, label);
}

fn assert_revision_mutation(
    actual: &Value,
    expected: &Value,
    request: &Value,
    record_id: &str,
    label: &str,
) -> String {
    let digest = request_digest(request);
    let previous_revision = request["expected_revision"].as_str().unwrap();
    let archive = archive_reference(record_id, previous_revision);
    assert_eq!(actual["receipt"]["request"], *request);
    assert_eq!(actual["receipt"]["request_digest"], digest);
    assert_eq!(actual["receipt"]["previous_revision"], previous_revision);
    assert_eq!(actual["receipt"]["archive_path"], archive);
    assert_eq!(
        actual["receipt"]["owner_configuration"],
        request["expected_configuration"]
    );
    assert_eq!(
        actual["receipt"]["dependencies"],
        request["expected_dependencies"]
    );
    let mut expected = expected.clone();
    expected["receipt"]["dependencies"] = request["expected_dependencies"].clone();
    expected["receipt"]["request"] = request.clone();
    expected["receipt"]["request_digest"] = Value::String(digest);
    expected["receipt"]["previous_revision"] = Value::String(previous_revision.to_owned());
    expected["receipt"]["archive_path"] = Value::String(archive.clone());
    assert_oracle(actual, &expected, true, label);
    archive
}

fn assert_inspection(actual: &Value, expected: &Value, archive: &str, label: &str) {
    let mut expected = expected.clone();
    for (name, expected_location) in expected["files"].as_object_mut().unwrap() {
        let digest = expected_location["sha256"].as_str().unwrap();
        let blob = format!("{}.blob", digest.strip_prefix("sha256:").unwrap());
        let reference = format!("{archive}/{blob}");
        assert_eq!(actual["files"][name]["archive_path"], reference);
        expected_location["archive_path"] = Value::String(reference);
    }
    assert_oracle(actual, &expected, true, label);
}

fn comparable_form_payload(value: &Value, request: &Value, verify_actual: bool) -> Value {
    let mut normalized = value.clone();
    let command_id = request["command_id"].as_str().unwrap();
    let digest = request_digest(request);
    let previous_revision = request["expected_revision"].clone();
    let receipts = normalized["growth_history"].as_array_mut().unwrap();
    let mut found = false;
    for receipt in receipts {
        if receipt["command_id"].as_str() != Some(command_id) {
            continue;
        }
        assert!(!found, "form command receipt is duplicated");
        found = true;
        if verify_actual {
            assert_eq!(receipt["request_digest"], digest);
            assert_eq!(receipt["previous_revision"], previous_revision);
            assert_eq!(
                receipt["owner_configuration"],
                request["expected_configuration"]
            );
        }
        let receipt = receipt.as_object_mut().unwrap();
        receipt.insert("request_digest".into(), Value::String(digest.clone()));
        receipt.insert("previous_revision".into(), previous_revision.clone());
        receipt.remove("recorded_at");
        receipt.insert(
            "owner_configuration".into(),
            request["expected_configuration"].clone(),
        );
    }
    assert!(found, "form command receipt is absent");
    normalized
}

fn native_call(
    owner: &Path,
    invocation_path: &Path,
    invocation: &Value,
    request: &Value,
    deadline: Instant,
    ledger: &mut ProcessLedger,
) -> Value {
    assert!(Instant::now() < deadline);
    assert_eq!(
        invocation["owner_config"].as_str(),
        Some(owner.to_string_lossy().as_ref()),
        "direct native CLI invocation selects the expected owner"
    );
    fs::write(invocation_path, canonical(invocation)).unwrap();
    fs::set_permissions(invocation_path, fs::Permissions::from_mode(0o600)).unwrap();

    let executable = PathBuf::from(
        invocation["native_executable"]
            .as_str()
            .expect("selected native owner executable"),
    );
    assert!(executable.is_absolute());
    let mut input = tempfile::tempfile().unwrap();
    input.write_all(&canonical(request)).unwrap();
    input.seek(SeekFrom::Start(0)).unwrap();
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut command = Command::new(executable);
    command
        .args(["source-commands", "--invocation"])
        .arg(invocation_path);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(errors.try_clone().unwrap()));
    let mut child = command.process_group(0).spawn().unwrap();
    ledger.native_cli_spawns = ledger.native_cli_spawns.checked_add(1).unwrap();
    let step_deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step_deadline
            || output.metadata().unwrap().len() > 4_194_304
            || errors.metadata().unwrap().len() > 1_048_576
        {
            let _ = Command::new("/usr/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            panic!("bounded native Legacy Claim command refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step_deadline);
    assert!(output.metadata().unwrap().len() <= 4_194_304);
    assert!(errors.metadata().unwrap().len() <= 1_048_576);
    output.seek(SeekFrom::Start(0)).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut output_bytes = Vec::new();
    let mut error_bytes = Vec::new();
    output.read_to_end(&mut output_bytes).unwrap();
    errors.read_to_end(&mut error_bytes).unwrap();
    assert!(
        status.success(),
        "native CLI output={} stderr={}",
        String::from_utf8_lossy(&output_bytes),
        String::from_utf8_lossy(&error_bytes)
    );
    let response: Value = serde_json::from_slice(&output_bytes).unwrap();
    assert_eq!(
        response["schema_version"],
        "tos_local_native_source_result_v1"
    );
    assert_eq!(response["grants_admission"], false);
    let config: Value = serde_json::from_slice(&fs::read(owner).unwrap()).unwrap();
    let configuration = if config["schema_version"] == "tos_local_historical_claim_form_owner_v1" {
        let root = Path::new(config["source_root"].as_str().unwrap());
        let mut contracts = serde_json::Map::new();
        for relative in [
            "ToS/contracts/historical-claim.schema.json",
            "ToS/contracts/claim-packet.schema.json",
            "ToS/contracts/historical-record.schema.json",
            "ToS/contracts/corpus-record.schema.json",
            "ToS/contracts/knowledge-assessment.schema.json",
            "ToS/contracts/claim-display-fields.schema.json",
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        ] {
            contracts.insert(
                relative.to_owned(),
                Value::String(
                    tos_foundation::Digest256::of_bytes(&fs::read(root.join(relative)).unwrap())
                        .to_prefixed(),
                ),
            );
        }
        request_digest(&serde_json::json!({"configuration":config,"source_contracts":contracts}))
    } else {
        assert_eq!(
            config["schema_version"],
            "tos_local_historical_claim_revision_owner_v1"
        );
        request_digest(&config)
    };
    assert_eq!(response["result"]["owner_configuration"], configuration);
    response["result"].clone()
}

#[test]
fn native_legacy_historical_claim_revision_forms_and_cold_lineage_match_oracle() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let mut ledger = ProcessLedger::default();
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("historical-source-root");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let native_owner_paths = [
        "rust/crates/tos-command/src/bin/tos-native-owner-command.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_native_legacy_claim_cli.rs",
        "rust/crates/tos-command/src/source_native_claim_cli.rs",
        "rust/crates/tos-command/src/source_native_forms_cli.rs",
        "rust/crates/tos-command/src/source_native_revisions_cli.rs",
        "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
        "rust/crates/tos-command/src/source_legacy_claim_store.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_command.rs",
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_revisions.rs",
        "rust/crates/tos-command/src/source_operation.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
    ];
    let captured = super::native_python_fixture(
        "legacy-claim",
        &[("fixture-root", &root)],
        &native_owner_paths,
    );
    assert_eq!(
        captured.capture_identity.factory_script_sha256,
        "ed7495e1ae3a2d534090c3fb78fef2e5540553d657b5c85d22b505212b93fb8a",
        "historical Python fixture identity is pinned by the captured manifest"
    );
    assert_eq!(
        captured.capture_identity.factory_source_path,
        "tests/conformance/rust/command_legacy_claim_cases.rs"
    );
    assert_eq!(captured.capture_identity.factory_source_symbol, "FIXTURE");
    assert_eq!(
        captured.capture_identity.packet_sha256,
        "ec936f4ca270f21041d0e5b82b69eded498b5936bc94f2a5384600fce86ac6b5"
    );
    let fixture = &captured.packets["factory"];
    let root = captured.root;
    let revision_owner = PathBuf::from(fixture["revision_owner"].as_str().unwrap());
    let form_owner = PathBuf::from(fixture["form_owner"].as_str().unwrap());
    fs::set_permissions(&revision_owner, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&form_owner, fs::Permissions::from_mode(0o600)).unwrap();

    let (authored, authored_bytes) = source_members(&root, deadline);
    let generated_prefix = fixture["generated_prefix"].as_str().unwrap();
    let original = authored
        .iter()
        .filter(|(name, _)| !name.starts_with(generated_prefix))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    assert!(!original.is_empty() && original.len() < authored.len());
    assert!(
        authored
            .keys()
            .any(|name| name.starts_with(generated_prefix))
    );
    let initial_stream = fs::read(root.join(fixture["stream_path"].as_str().unwrap())).unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&initial_stream).to_hex(),
        fixture["initial_stream_sha256"].as_str().unwrap()
    );

    let mut software_names = vec![
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/agents/friedrich-nietzsche/agent.json",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/places/chemnitz/place.json",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json",
        "rust/crates/tos-command/src/bin/tos-native-owner-command.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_native_legacy_claim_cli.rs",
        "rust/crates/tos-command/src/source_native_claim_cli.rs",
        "rust/crates/tos-command/src/source_native_forms_cli.rs",
        "rust/crates/tos-command/src/source_native_revisions_cli.rs",
        "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
        "rust/crates/tos-command/src/source_legacy_claim_store.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_command.rs",
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_revisions.rs",
        "rust/crates/tos-command/src/source_operation.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let mut native_rules = BTreeMap::new();
    super::command_record_cases::native_metadata_rule_files(&repository, &mut native_rules, &[]);
    software_names.extend(
        native_rules
            .keys()
            .filter(|name| !name.starts_with("ToS/"))
            .cloned(),
    );
    super::command_record_cases::materialize_native_fixture_software(&root, &native_rules);
    software_names.sort();
    software_names.dedup();
    let (fixture_bytes, fixture_files) =
        charge_software(&software_names, &repository, authored_bytes, authored.len());
    let software_bytes = fixture_bytes - authored_bytes;
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must select the protected native owner command image"),
    );
    assert!(native.is_absolute());
    let worker = super::validation_cut_cases::selected_worker_path();
    let native_bytes = fs::metadata(&native).unwrap().len();
    let worker_bytes = fs::metadata(&worker).unwrap().len();
    let consumer_bytes = fs::metadata(std::env::current_exe().unwrap())
        .unwrap()
        .len();
    assert!(native_bytes <= 536_870_912 && worker_bytes <= 536_870_912);
    assert!(consumer_bytes <= 536_870_912);
    let selected_image_bytes = native_bytes
        .checked_add(consumer_bytes)
        .and_then(|sum| sum.checked_add(worker_bytes))
        .unwrap();
    assert!(Instant::now() < deadline);
    let (capture, _software, components) =
        selected_software(&repository, &software_names, deadline, &mut ledger);

    let store = temporary.path().join("legacy-cut-store");
    let original_revision = super::validation_cut_cases::write_cut_store(&original, &store);
    let current_revision = super::validation_cut_cases::write_cut_store_on_base(
        &authored,
        &store,
        Some(original_revision),
    );
    assert_ne!(original_revision, current_revision);
    let cancelled = AtomicBool::new(false);
    let _original_cut =
        super::command_form_cases::open_cut(&store, original_revision, deadline, &cancelled);
    let _current_cut =
        super::command_form_cases::open_cut(&store, current_revision, deadline, &cancelled);

    let invocation_path = temporary.path().join("legacy-claim-invocation.json");
    let mut invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_context":null,
        "owner_config":revision_owner,
        "assessment_schema_worker":null,
        "native_executable":native,
        "native_executable_sha256":super::command_text_cases::alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,
        "source_revision":current_revision.0.to_prefixed(),
        "original_source_revision":original_revision.0.to_prefixed(),
        "software_capture":capture.capture,
        "software_restored_root":capture.restored,
        "software_selection":{
            "source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed(),
        },
        "software_components":components.members().map(|member| member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{
            "absolute_path":worker,
            "sha256":super::command_text_cases::alignment_image_digest(&worker).to_prefixed(),
        },
        "budgets":{
            "max_revisions":4,"max_members":256,"max_total_bytes":8388608,
            "max_member_bytes":8388608,"max_schema_receipts":128,
            "max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,
            "worker_address_space_bytes":1073741824
        }
    });

    let expected = &fixture["expected"];
    let revision_preview_request = &fixture["revision_preview_request"];
    let preview = native_call(
        &revision_owner,
        &invocation_path,
        &invocation,
        revision_preview_request,
        deadline,
        &mut ledger,
    );
    // Code migration changes the dependency digest; the same bound source
    // objects and evidence below must still match the frozen reference.
    assert!(
        tos_foundation::Digest256::from_prefixed(
            preview["expected_dependencies"].as_str().unwrap()
        )
        .is_ok()
    );
    let mut expected_preview = expected["revision_preview"].clone();
    expected_preview["expected_dependencies"] = preview["expected_dependencies"].clone();
    assert_oracle(&preview, &expected_preview, false, "prepare-revise");
    let mut revision_request = fixture["revision_request"].clone();
    revision_request["expected_source"] = preview["source"].clone();
    revision_request["expected_revision"] = preview["revision"].clone();
    revision_request["expected_configuration"] = preview["owner_configuration"].clone();
    revision_request["expected_dependencies"] = preview["expected_dependencies"].clone();
    revision_request["expected_inputs"] = preview["source_bindings"].clone();
    let before_refusal = source_members(&root, deadline);
    let mut stale_request = revision_request.clone();
    stale_request["expected_dependencies"] = Value::String(format!("sha256:{}", "0".repeat(64)));
    let (status, raw, error) = super::command_text_cases::native_owner_cli_observation(
        &repository,
        &revision_owner,
        &invocation_path,
        &stale_request,
        deadline,
    );
    ledger.native_cli_spawns += 1;
    assert!(
        !status.success(),
        "stale dependency accepted: {} {}",
        String::from_utf8_lossy(&raw),
        String::from_utf8_lossy(&error)
    );
    assert_eq!(source_members(&root, deadline), before_refusal);
    let revised = native_call(
        &revision_owner,
        &invocation_path,
        &invocation,
        &revision_request,
        deadline,
        &mut ledger,
    );
    let archive_ref = assert_revision_mutation(
        &revised,
        &expected["revision_result"],
        &revision_request,
        fixture["claim_id"].as_str().unwrap(),
        "claim.revise",
    );
    assert_eq!(revised["replayed"], false);
    assert_eq!(revised["grants_admission"], false);
    let revised_stream = fs::read(root.join(fixture["stream_path"].as_str().unwrap())).unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&revised_stream).to_hex(),
        fixture["revised_stream_sha256"].as_str().unwrap()
    );
    assert_ne!(revised_stream, initial_stream);
    let archive = root.join(&archive_ref);
    assert!(archive.is_dir() && !archive.is_symlink());
    assert!(archive.join("manifest.json").is_file());

    let (revised_authored, revised_bytes) = source_members(&root, deadline);
    let max_authored_bytes = authored_bytes.max(revised_bytes);
    let max_fixture_bytes = software_bytes.checked_add(max_authored_bytes).unwrap();
    let max_fixture_files = software_names.len() + authored.len().max(revised_authored.len());
    assert!(max_fixture_bytes <= 8_388_608 && max_fixture_files <= 256);
    let revised_revision =
        super::command_form_cases::successor(&revised_authored, &store, current_revision);
    invocation["source_revision"] = Value::String(revised_revision.0.to_prefixed());
    let replay = native_call(
        &revision_owner,
        &invocation_path,
        &invocation,
        &revision_request,
        deadline,
        &mut ledger,
    );
    let replay_archive = assert_revision_mutation(
        &replay,
        &expected["revision_replay"],
        &revision_request,
        fixture["claim_id"].as_str().unwrap(),
        "cold claim replay",
    );
    assert_eq!(replay_archive, archive_ref);
    assert_eq!(replay["replayed"], true);
    let inspected = native_call(
        &revision_owner,
        &invocation_path,
        &invocation,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1",
            "operation":"inspect-version","source":preview["source"]}),
        deadline,
        &mut ledger,
    );
    assert_inspection(
        &inspected,
        &expected["inspect"],
        &archive_ref,
        "inspect-version",
    );
    assert_eq!(inspected["record"]["claim_id"], fixture["claim_id"]);

    invocation["owner_config"] = Value::String(form_owner.to_string_lossy().into_owned());
    let form_preview = native_call(
        &form_owner,
        &invocation_path,
        &invocation,
        &fixture["form_prepare_request"],
        deadline,
        &mut ledger,
    );
    assert_oracle(
        &form_preview,
        &expected["form_preview"],
        false,
        "forms prepare",
    );
    let mut form_request = fixture["form_request"].clone();
    form_request["expected_source"] = form_preview["source"].clone();
    form_request["expected_revision"] = form_preview["revision"].clone();
    form_request["expected_configuration"] = form_preview["owner_configuration"].clone();
    form_request["changes"] = serde_json::json!([form_preview["prepared_change"].clone()]);
    let form_result = native_call(
        &form_owner,
        &invocation_path,
        &invocation,
        &form_request,
        deadline,
        &mut ledger,
    );
    assert_form_mutation(
        &form_result,
        &expected["form_result"],
        &form_request,
        "forms apply",
    );
    assert_eq!(form_result["replayed"], false);
    assert_eq!(form_result["grants_admission"], false);
    let final_form = fs::read(root.join(fixture["claim_form_path"].as_str().unwrap())).unwrap();
    let actual_form: Value = serde_json::from_slice(&final_form).unwrap();
    assert_eq!(
        comparable_form_payload(&actual_form, &form_request, true),
        comparable_form_payload(&fixture["final_form_payload"], &form_request, false),
        "forms preserve the exact legacy Claim set apart from runtime receipt fields"
    );
    assert_eq!(
        tos_foundation::Digest256::of_bytes(
            &fs::read(root.join(fixture["stream_path"].as_str().unwrap())).unwrap()
        )
        .to_hex(),
        fixture["revised_stream_sha256"].as_str().unwrap(),
        "separate form writer preserves the revised Claim bytes"
    );

    let (final_authored, final_bytes) = source_members(&root, deadline);
    let max_authored_bytes = max_authored_bytes.max(final_bytes);
    let max_fixture_bytes = software_bytes.checked_add(max_authored_bytes).unwrap();
    let max_fixture_files = software_names.len()
        + authored
            .len()
            .max(revised_authored.len())
            .max(final_authored.len());
    assert!(max_fixture_bytes <= 8_388_608 && max_fixture_files <= 256);
    let final_revision =
        super::command_form_cases::successor(&final_authored, &store, revised_revision);
    invocation["source_revision"] = Value::String(final_revision.0.to_prefixed());
    let form_replay = native_call(
        &form_owner,
        &invocation_path,
        &invocation,
        &form_request,
        deadline,
        &mut ledger,
    );
    assert_form_mutation(
        &form_replay,
        &expected["form_replay"],
        &form_request,
        "cold forms replay",
    );
    assert_eq!(form_replay["replayed"], true);
    assert_eq!(form_replay["grants_admission"], false);
    assert!(Instant::now() < deadline);
    assert_eq!(ledger.git_spawns, 1);
    assert_eq!(ledger.native_cli_spawns, 8);

    eprintln!(
        "Legacy Claim F={max_fixture_bytes} bytes/{max_fixture_files} files (max authored state {max_authored_bytes} bytes; initial closure {fixture_bytes} bytes/{fixture_files} files), E={native_bytes}, C={consumer_bytes}, W={worker_bytes}, E+C+W={selected_image_bytes}; native_cli_spawns={}",
        ledger.native_cli_spawns
    );
}
