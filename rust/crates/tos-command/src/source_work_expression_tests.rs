//! Deterministic pending edge of the real private Work plan and mover. The
//! public conformance case covers the uninterrupted create/sibling path.
use super::*;
use crate::source_command::SourceFile;
use crate::source_creation_store::IsolatedCreationRoot;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, SourceRevision, canonical_bytes_v1, parse_json,
};
use tos_source_store::{
    CaptureGitRequest, CaptureRestoreLimits, CorpusReader, CutReadLimits, GitCaptureLimits,
    ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1, capture_git, restore_capture,
};
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::CutWorkerLimits;

pub(crate) fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

pub(crate) fn authored(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut directories = vec![root.join("ToS")];
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() {
                directories.push(entry.path());
                continue;
            }
            assert!(kind.is_file());
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
            let raw = fs::read(entry.path()).unwrap();
            total = total.checked_add(raw.len()).unwrap();
            assert!(raw.len() <= 8_388_608 && total <= 33_554_432);
            assert!(files.insert(path, raw).is_none());
            assert!(files.len() <= 4096);
        }
    }
    files
}

pub(crate) fn cut(
    files: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (SourceRevision, CorpusCutReader) {
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("revisions")).unwrap();
    let members = files
        .iter()
        .map(|(path, raw)| {
            let sha = Digest256::of_bytes(raw).to_hex();
            fs::write(root.join("objects").join(&sha), raw).unwrap();
            serde_json::json!({"path":path,"sha256":sha,"size_bytes":raw.len(),"mode":420})
        })
        .collect::<Vec<_>>();
    let mut manifest = serde_json::json!({
        "schema_version":"tos_corpus_snapshot_v1", "base_revision":null,
        "files":members,"identities":{},"dependencies":{},"retirements":[],
        "validator_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    });
    let body = canonical_corpus_manifest(&manifest);
    let revision = SourceRevision(Digest256::of_bytes(&body));
    manifest["revision"] = serde_json::Value::String(revision.0.to_hex());
    let home = root.join("revisions").join(revision.0.to_hex());
    fs::create_dir(&home).unwrap();
    let raw = canonical_corpus_manifest(&manifest);
    fs::write(home.join("snapshot.json"), raw).unwrap();
    let reader = CorpusReader::open_existing(
        root,
        ReadLimits {
            max_manifest_bytes: 4_194_304,
            max_manifest_entries: 2048,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
    )
    .unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 2048,
                max_total_bytes: 33_554_432,
                max_member_bytes: 8_388_608,
            },
            deadline,
            cancelled,
        )
        .unwrap();
    (revision, cut)
}

fn canonical_corpus_manifest(value: &serde_json::Value) -> Vec<u8> {
    let serialized = serde_json::to_vec(value).unwrap();
    let parsed = parse_json(
        &serialized,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
    )
    .unwrap()
}

pub(crate) fn software(
    repository: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    scratch: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (SoftwareCaptureReader, SoftwareComponentSelectionV1) {
    let commit_output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .unwrap();
    assert!(commit_output.status.success());
    let commit = String::from_utf8(commit_output.stdout)
        .unwrap()
        .trim()
        .to_owned();
    assert!(commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let capture = scratch.join("software-capture");
    let restored = scratch.join("software-restored");
    let include_prefixes = files
        .keys()
        .filter(|path| !path.starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    assert!(!include_prefixes.is_empty());
    let exclude_prefixes = Vec::new();
    let exclude_path_parts = Vec::new();
    let captured = capture_git(
        CaptureGitRequest {
            repository,
            commit: &commit,
            include_prefixes: &include_prefixes,
            exclude_prefixes: &exclude_prefixes,
            exclude_path_parts: &exclude_path_parts,
            output: &capture,
        },
        GitCaptureLimits {
            max_members: 16_384,
            max_member_bytes: 8_388_608,
            max_source_bytes: 536_870_912,
            max_metadata_bytes: 67_108_864,
            max_tree_bytes: 67_108_864,
            max_archive_bytes: 536_870_912,
        },
        deadline,
        cancelled,
    )
    .unwrap();
    let manifest_raw = fs::read(capture.join("capture.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_raw).unwrap();
    assert_eq!(Digest256::of_bytes(&manifest_raw), captured.manifest_sha256);
    assert_eq!(manifest["source_git_commit"].as_str().unwrap(), commit);
    let source_git_tree = captured
        .manifest
        .object_get("source_git_tree")
        .and_then(|value| value.as_str())
        .expect("native capture returns its exact Git tree")
        .to_owned();
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
        max_selected_object_bytes: 2_097_152,
        json: JsonLimits::default(),
    };
    restore_capture(
        &capture,
        &restored,
        &selection,
        CaptureRestoreLimits {
            metadata: read_limits,
            max_archive_bytes: 536_870_912,
            max_decoded_bytes: 536_870_912,
            max_source_bytes: 536_870_912,
        },
        deadline,
        cancelled,
    )
    .unwrap();
    let software = SoftwareCaptureReader::open(
        &capture,
        &restored,
        selection,
        read_limits,
        deadline,
        cancelled,
    )
    .unwrap();
    let paths = include_prefixes
        .iter()
        .map(|name| RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    (software, components)
}

pub(crate) fn worker(
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> CutWorkerSchemaExecutor {
    let path = std::env::var_os("TOS_SCHEMA_WORKER_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap())
                .join("debug/tos-schema-worker")
        });
    assert!(path.is_absolute() && path.is_file());
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    CutWorkerSchemaExecutor::from_cut(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            sha256: Digest256::of_bytes(&fs::read(&path).unwrap()),
            absolute_path: path,
        },
        budget,
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 262_144,
        },
        deadline,
        cancelled,
    )
    .unwrap()
}

type Side = Option<(usize, Digest256)>;

struct SelectedWitness {
    path: String,
    before: Side,
    after: Side,
}

fn side(raw: Option<&[u8]>) -> Side {
    raw.map(|raw| (raw.len(), Digest256::of_bytes(raw)))
}

fn current_side(root: &Path, reference: &str) -> Side {
    match fs::read(root.join(reference)) {
        Ok(raw) => side(Some(&raw)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("selected Work member read: {error}"),
    }
}

fn mixed_selected(root: &Path, selected: &[SelectedWitness]) -> bool {
    let mut before = false;
    let mut after = false;
    for member in selected {
        let actual = current_side(root, &member.path);
        assert!(
            actual == member.before || actual == member.after,
            "selected Work member entered a third state: {}",
            member.path
        );
        if member.before == member.after {
            continue;
        }
        before |= actual == member.before;
        after |= actual == member.after;
    }
    before && after
}

#[test]
fn real_pending_refuses_changed_dependency_then_resumes_or_rolls_back() {
    let repository = repository();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    for decision in [WorkRecoveryDecision::Resume, WorkRecoveryDecision::Rollback] {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let fixture = crate::source_creation_store::source_native_test_fixtures::work_expression(
            &repository,
            isolated.path(),
        );
        let owner = isolated.path().join("compound-owner.json");
        let owner_raw = fs::read(&owner).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let authored = authored(isolated.path());
        let mut files = authored.clone();
        for name in IMPLEMENTATIONS {
            let raw = fs::read(repository.join(name)).unwrap();
            let target = isolated.path().join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, &raw).unwrap();
            assert!(files.insert((*name).to_owned(), raw).is_none());
        }
        let (software, components) =
            software(&repository, &files, temporary.path(), deadline, &cancelled);
        let (revision, selected) = cut(
            &authored,
            &temporary.path().join("cut"),
            deadline,
            &cancelled,
        );
        let config = cmd::parse(&owner_raw).unwrap();
        let mut request = fixture["request"].clone();
        request["expected_configuration"] =
            serde_json::json!(cmd::record_digest(&config).unwrap().to_prefixed());
        let context = CommandContext {
            base_revision: revision,
            configuration_raw: owner_raw,
            request_raw: serde_json::to_vec(&request).unwrap(),
            recorded_at: "2026-01-01T12:34:56+00:00".to_owned(),
            effective_uid: fs::metadata(isolated.path()).unwrap().uid().into(),
            files: files
                .iter()
                .map(|(path, raw)| SourceFile {
                    path: RelativePath::parse(path).unwrap(),
                    raw: raw.clone(),
                })
                .collect(),
        };
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled).unwrap();
        let limits = ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline,
        };
        let mut schema = worker(&selected, deadline, &cancelled);
        let prepared = prepare_work_application(
            &filesystem,
            &context,
            &selected,
            &software,
            &components,
            &mut schema,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(schema);
        let dependency = isolated
            .path()
            .join("ToS/contracts/corpus-record.schema.json");
        let original = fs::read(&dependency).unwrap();
        let changed = [original.as_slice(), b"\n"].concat();
        let PreparedWorkApplication { plan, guard, .. } = prepared;
        let selected_witnesses = plan
            .files
            .iter()
            .map(|file| SelectedWitness {
                path: file.path.as_str().to_owned(),
                before: side(file.before.as_deref()),
                after: side(file.after.as_deref()),
            })
            .collect::<Vec<_>>();
        assert!(selected_witnesses.len() > 1);
        let fence =
            work_transaction::WorkCorpusFence::hold(&filesystem, deadline, &cancelled).unwrap();
        let mut switched = false;
        let result = fence.apply(
            plan,
            &guard.snapshot,
            |summary, extent| {
                if extent.pending_state.is_some()
                    && !switched
                    && mixed_selected(isolated.path(), &selected_witnesses)
                {
                    fs::write(&dependency, &changed).unwrap();
                    switched = true;
                }
                guard.check(
                    &filesystem,
                    &context,
                    &selected,
                    summary,
                    extent,
                    limits,
                    &cancelled,
                )
            },
            deadline,
            &cancelled,
        );
        drop(fence);
        assert!(switched);
        assert!(matches!(result, Err(SourceCommandError::Conflict(_))));
        assert_eq!(fs::read(&dependency).unwrap(), changed);
        assert!(mixed_selected(isolated.path(), &selected_witnesses));
        let control = isolated
            .path()
            .join("ToS/source-witnesses/.metadata-publication.json");
        let pending: serde_json::Value =
            serde_json::from_slice(&fs::read(&control).unwrap()).unwrap();
        assert_eq!(pending["phase"], "pending");
        fs::write(&dependency, original).unwrap();
        let mut recovery_worker = worker(&selected, deadline, &cancelled);
        let recovered = recover_isolated_work_expression_from_captures(
            &filesystem,
            &context,
            &selected,
            &software,
            &components,
            &mut recovery_worker,
            decision,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(recovery_worker);
        assert_eq!(
            recovered.transaction_id(),
            pending["transaction_id"].as_str().unwrap()
        );
        for member in &selected_witnesses {
            assert_eq!(
                current_side(isolated.path(), &member.path),
                if matches!(decision, WorkRecoveryDecision::Resume) {
                    member.after
                } else {
                    member.before
                },
                "selected Work member was not restored: {}",
                member.path,
            );
        }
        let terminal: serde_json::Value =
            serde_json::from_slice(&fs::read(&control).unwrap()).unwrap();
        assert_eq!(terminal["phase"], "ready");
        assert_eq!(terminal["transaction_id"], pending["transaction_id"]);
        assert_eq!(terminal["manifest_sha256"], pending["manifest_sha256"]);
        assert_eq!(
            terminal["outcome"],
            if matches!(decision, WorkRecoveryDecision::Resume) {
                "committed"
            } else {
                "rolled-back"
            }
        );
        if matches!(decision, WorkRecoveryDecision::Resume) {
            assert!(!recovered.replayed());
            assert!(
                isolated
                    .path()
                    .join(fixture["expression_ref"].as_str().unwrap())
                    .exists()
            );
        } else {
            assert_eq!(recovered.receipt(), &JsonValue::Null);
            assert!(
                !isolated
                    .path()
                    .join(fixture["expression_ref"].as_str().unwrap())
                    .exists()
            );
        }
    }
}

fn cli37_image(path: &Path) -> (Digest256, (u64, u64, u64)) {
    use std::io::Read;
    use tos_foundation::Digest256Hasher;
    assert!(path.is_absolute());
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file() && metadata.permissions().mode() & 0o022 == 0);
    assert!(metadata.len() <= 536_870_912);
    let mut reader = fs::File::open(path).unwrap();
    let mut hash = Digest256Hasher::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let count = reader.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    (
        hash.finalize(),
        (metadata.dev(), metadata.ino(), metadata.len()),
    )
}

fn cli37_observe(
    native: &Path,
    invocation: &Path,
    request: &serde_json::Value,
    deadline: Instant,
) -> (bool, serde_json::Value) {
    use std::io::{Read, Seek, SeekFrom, Write};
    let raw = serde_json::to_vec(request).unwrap();
    assert!(raw.len() <= 1_048_576);
    let mut input = tempfile::tempfile().unwrap();
    input.write_all(&raw).unwrap();
    input.seek(SeekFrom::Start(0)).unwrap();
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut child = Command::new(native)
        .arg("--invocation")
        .arg(invocation)
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(errors.try_clone().unwrap()))
        .spawn()
        .unwrap();
    let step = deadline.min(Instant::now() + Duration::from_secs(60));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step
            || output.metadata().unwrap().len() > 1_048_576
            || errors.metadata().unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded Work37 actual native caller refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step);
    assert!(
        output.metadata().unwrap().len() <= 1_048_576
            && errors.metadata().unwrap().len() <= 1_048_576
    );
    output.seek(SeekFrom::Start(0)).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut raw = Vec::new();
    output.read_to_end(&mut raw).unwrap();
    let mut diagnostic = Vec::new();
    errors.read_to_end(&mut diagnostic).unwrap();
    if !status.success() {
        return (
            false,
            serde_json::json!({"diagnostic":String::from_utf8_lossy(&diagnostic)}),
        );
    }
    let response: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(
        response["schema_version"],
        "tos_local_native_source_result_v1"
    );
    assert_eq!(response["grants_admission"], false);
    assert_eq!(response["result"]["grants_admission"], false);
    (true, response["result"].clone())
}

#[test]
fn native_work37_cli_creates_process_cold_replays_and_recovers_exact_pending() {
    let repository = repository();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS selects immutable Work37 CLI"),
    );
    let worker_path = PathBuf::from(
        std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("OPS selects immutable Work37 worker"),
    );
    let native_guard = cli37_image(&native);
    let worker_guard = cli37_image(&worker_path);
    let consumer = std::env::current_exe().unwrap();
    let consumer_guard = cli37_image(&consumer);
    // Three sequential fixture roots bound coexistence to one whole Work packet.
    // Cold below means a fresh actual caller process, retaining the same exact
    // root/config bytes. It does not claim archive restoration or relocation.
    for mode in 0..3 {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let fixture = crate::source_creation_store::source_native_test_fixtures::work_expression(
            &repository,
            isolated.path(),
        );
        let owner = isolated.path().join("compound-owner.json");
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let owner_raw = fs::read(&owner).unwrap();
        let initial = authored(isolated.path());
        let mut files = initial.clone();
        for name in IMPLEMENTATIONS {
            let raw = fs::read(repository.join(name)).unwrap();
            assert!(raw.len() <= 8_388_608);
            let target = isolated.path().join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, &raw).unwrap();
            assert!(files.insert((*name).to_owned(), raw).is_none());
        }
        let fixture_bytes = files
            .values()
            .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
            .unwrap();
        assert!(fixture_bytes <= 33_554_432 && files.len() <= 2048);
        eprintln!(
            "Work37 mode={mode} F={fixture_bytes} E={} C={} W={}",
            native_guard.1.2, consumer_guard.1.2, worker_guard.1.2
        );
        let (software, components) =
            software(&repository, &files, temporary.path(), deadline, &cancelled);
        let store = temporary.path().join("cut");
        let (original_revision, selected) = cut(&initial, &store, deadline, &cancelled);
        let selection = software.selection();
        let invocation_path = temporary.path().join("native-work37-invocation.json");
        let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
            "owner_config":owner,"owner_context":null,"assessment_schema_worker":null,
            "native_executable":native,"native_executable_sha256":native_guard.0.to_prefixed(),
            "corpus_store":store,"source_revision":original_revision.0.to_prefixed(),"original_source_revision":original_revision.0.to_prefixed(),
            "software_capture":temporary.path().join("capture"),"software_restored_root":temporary.path().join("restored"),
            "software_selection":{"source_git_commit":selection.source_git_commit,"source_git_tree":selection.source_git_tree,
                "capture_manifest_sha256":selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":worker_path,"sha256":worker_guard.0.to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
                "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        let freeze = |value: &serde_json::Value| {
            let raw = serde_json::to_vec(value).unwrap();
            assert!(raw.len() <= 1_048_576);
            fs::write(&invocation_path, raw).unwrap();
            fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
        };
        freeze(&invocation);
        if mode == 0 {
            let (ok, describe) = cli37_observe(
                &native,
                &invocation_path,
                &serde_json::json!({"schema_version":"tos_local_work_expression_command_v1","operation":"describe"}),
                deadline,
            );
            assert!(ok, "{describe}");
            assert!(describe["source_fields"].is_array());
            assert!(describe["materializations"].is_null());
        }
        let (ok, preview) =
            cli37_observe(&native, &invocation_path, &fixture["proposal"], deadline);
        assert!(ok, "{preview}");
        assert!(preview["prepared_materializations"].is_object());
        // The maintained Python fixture prepared these exact parent fields
        // independently; native dependency/publication identities remain owned
        // by the actual native preview and are never copied from its oracle.
        assert_eq!(preview["prepared_fields"], fixture["request"]["fields"]);
        assert_eq!(preview["source"], fixture["request"]["expected_source"]);
        let mut request = fixture["request"].clone();
        for (field, prepared) in [
            ("fields", "prepared_fields"),
            ("expected_source", "source"),
            ("expected_revision", "revision"),
            ("expected_configuration", "owner_configuration"),
            ("expected_dependencies", "expected_dependencies"),
            ("expected_publication", "expected_publication"),
        ] {
            request[field] = preview[prepared].clone();
        }
        if mode == 0 {
            let (ok, created) = cli37_observe(&native, &invocation_path, &request, deadline);
            assert!(ok, "{created}");
            assert_eq!(created["replayed"], false);
            assert!(!created["receipt"].is_null());
            assert_eq!(
                created["materializations"],
                preview["prepared_materializations"]
            );
            let expression: serde_json::Value = serde_json::from_slice(
                &fs::read(
                    isolated
                        .path()
                        .join(fixture["expression_ref"].as_str().unwrap()),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(expression, fixture["proposal"]["record"]);
            let work: serde_json::Value = serde_json::from_slice(
                &fs::read(isolated.path().join(fixture["work_ref"].as_str().unwrap())).unwrap(),
            )
            .unwrap();
            let mut expected_work: serde_json::Value =
                serde_json::from_slice(&initial[fixture["work_ref"].as_str().unwrap()]).unwrap();
            expected_work["record_version"] =
                serde_json::json!(preview["source"]["version"].as_u64().unwrap() + 1);
            expected_work["expression_claim_refs"] =
                serde_json::json!([fixture["proposal"]["claim"]["claim_id"]]);
            assert_eq!(work, expected_work);
            let untouched = fixture["untouched_ref"].as_str().unwrap();
            assert_eq!(
                fs::read(isolated.path().join(untouched)).unwrap(),
                initial[untouched]
            );
            let after = authored(isolated.path());
            let (current_revision, _) = cut(&after, &store, deadline, &cancelled);
            invocation["source_revision"] = serde_json::json!(current_revision.0.to_prefixed());
            freeze(&invocation);
            let (ok, replay) = cli37_observe(&native, &invocation_path, &request, deadline);
            assert!(ok, "{replay}");
            assert_eq!(replay["replayed"], true);
            assert_eq!(replay["receipt"], created["receipt"]);
            assert_eq!(replay["materializations"], created["materializations"]);
            assert_eq!(authored(isolated.path()), after);
        } else {
            let filesystem =
                CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled)
                    .unwrap();
            let context = CommandContext {
                base_revision: original_revision,
                configuration_raw: owner_raw.clone(),
                request_raw: serde_json::to_vec(&request).unwrap(),
                recorded_at: "2026-01-01T12:34:56+00:00".into(),
                effective_uid: fs::metadata(isolated.path()).unwrap().uid().into(),
                files: files
                    .iter()
                    .map(|(path, raw)| SourceFile {
                        path: RelativePath::parse(path).unwrap(),
                        raw: raw.clone(),
                    })
                    .collect(),
            };
            let limits = ItemLimits {
                max_member_bytes: 2_097_152,
                max_total_bytes: 33_554_432,
                max_state_bytes: 33_554_432,
                max_issues: 256,
                deadline,
            };
            let mut schema = worker(&selected, deadline, &cancelled);
            let prepared = prepare_work_application(
                &filesystem,
                &context,
                &selected,
                &software,
                &components,
                &mut schema,
                limits,
                &cancelled,
            )
            .unwrap();
            drop(schema);
            let dependency = isolated
                .path()
                .join("ToS/contracts/corpus-record.schema.json");
            let before_dependency = fs::read(&dependency).unwrap();
            let changed = [before_dependency.as_slice(), b"\n"].concat();
            let PreparedWorkApplication { plan, guard, .. } = prepared;
            let witnesses = plan
                .files
                .iter()
                .map(|file| SelectedWitness {
                    path: file.path.as_str().into(),
                    before: side(file.before.as_deref()),
                    after: side(file.after.as_deref()),
                })
                .collect::<Vec<_>>();
            let fence =
                work_transaction::WorkCorpusFence::hold(&filesystem, deadline, &cancelled).unwrap();
            let mut switched = false;
            let failed = fence.apply(
                plan,
                &guard.snapshot,
                |summary, extent| {
                    if extent.pending_state.is_some()
                        && !switched
                        && mixed_selected(isolated.path(), &witnesses)
                    {
                        fs::write(&dependency, &changed).unwrap();
                        switched = true;
                    }
                    guard.check(
                        &filesystem,
                        &context,
                        &selected,
                        summary,
                        extent,
                        limits,
                        &cancelled,
                    )
                },
                deadline,
                &cancelled,
            );
            drop(fence);
            assert!(switched && matches!(failed, Err(SourceCommandError::Conflict(_))));
            let control = isolated
                .path()
                .join("ToS/source-witnesses/.metadata-publication.json");
            let pending_raw = fs::read(&control).unwrap();
            let pending: serde_json::Value = serde_json::from_slice(&pending_raw).unwrap();
            let recovery = serde_json::json!({"schema_version":"tos_local_work_expression_command_v1",
                "operation":"work.expression.recover","transaction_id":pending["transaction_id"],
                "decision":if mode==1 {"resume"} else {"rollback"},"expected_configuration":preview["owner_configuration"]});
            let refuse_unchanged = |request: &serde_json::Value| {
                let before = witnesses
                    .iter()
                    .map(|file| current_side(isolated.path(), &file.path))
                    .collect::<Vec<_>>();
                assert!(!cli37_observe(&native, &invocation_path, request, deadline).0);
                assert_eq!(fs::read(&control).unwrap(), pending_raw);
                let after = witnesses
                    .iter()
                    .map(|file| current_side(isolated.path(), &file.path))
                    .collect::<Vec<_>>();
                assert_eq!(
                    after, before,
                    "refused Work37 CLI changed selected pending bytes"
                );
            };
            refuse_unchanged(&recovery);
            fs::write(&dependency, &before_dependency).unwrap();
            let mut wrong = recovery.clone();
            wrong["transaction_id"] = serde_json::json!(format!("sha256:{}", "0".repeat(64)));
            refuse_unchanged(&wrong);
            let mut revoked: serde_json::Value = serde_json::from_slice(&owner_raw).unwrap();
            revoked["allowed_operations"] = serde_json::json!([]);
            fs::write(&owner, serde_json::to_vec(&revoked).unwrap()).unwrap();
            refuse_unchanged(&recovery);
            fs::write(&owner, &owner_raw).unwrap();
            let source = isolated.path().join(fixture["work_ref"].as_str().unwrap());
            let source_raw = fs::read(&source).unwrap();
            fs::write(&source, [source_raw.as_slice(), b"\n"].concat()).unwrap();
            refuse_unchanged(&recovery);
            fs::write(&source, &source_raw).unwrap();
            let (ok, recovered) = cli37_observe(&native, &invocation_path, &recovery, deadline);
            assert!(ok, "{recovered}");
            for file in &witnesses {
                assert_eq!(
                    current_side(isolated.path(), &file.path),
                    if mode == 1 { file.after } else { file.before }
                );
            }
            let terminal: serde_json::Value =
                serde_json::from_slice(&fs::read(&control).unwrap()).unwrap();
            assert_eq!(terminal["phase"], "ready");
            assert_eq!(terminal["transaction_id"], pending["transaction_id"]);
            assert_eq!(terminal["manifest_sha256"], pending["manifest_sha256"]);
            assert_eq!(
                terminal["outcome"],
                if mode == 1 {
                    "committed"
                } else {
                    "rolled-back"
                }
            );
            if mode == 2 {
                assert!(recovered["receipt"].is_null());
            }
        }
        assert_eq!(cli37_image(&native), native_guard);
        assert_eq!(cli37_image(&worker_path), worker_guard);
        assert_eq!(cli37_image(&consumer), consumer_guard);
        drop(selected);
        drop(components);
        drop(software);
        drop(isolated);
        temporary.close().unwrap();
    }
}
