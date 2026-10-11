//! Bounded real-source revision exercise over private PostgreSQL and STO.
//! The selected authored bytes are copied into a private fixture before any
//! capture or mutation; the repository remains read-only.

use super::{Lab, ScratchRoot, contract_digest, database_url, limits};
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};
use tos_command::{
    DurablePgCoordinator, DurableProjectionLimits, DurableProjectionProfile,
    DurableProjectionSearch, DurableProjectionStatus,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, RelativePath, SourceRevision,
    canonical_bytes_v1, parse_json,
};
use tos_source_store::{
    CaptureGitRequest, CaptureRestoreLimits, CorpusCutReader, CorpusReader, CutReadLimits,
    GitCaptureLimits, ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
};
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};

const AGENT_DIRS: &[&str] = &["parmenides-of-elea", "justus-lipsius", "hans-hahn"];
const SHARED_SOURCE: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
];
const SOFTWARE_COMPONENTS: &[&str] = &[
    "rust/crates/tos-command/src/source_native_cli.rs",
    "rust/crates/tos-command/src/source_command.rs",
    "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
    "rust/crates/tos-command/src/source_revisions.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "rust/crates/tos-command/src/source_private_assessment_sources.rs",
    "rust/crates/tos-validation/src/assessment.rs",
    "rust/crates/tos-compiler/src/source_witness_catalog.rs",
    "rust/crates/tos-command/src/source_private_profile.rs",
    "rust/crates/tos-command/src/source_sign_native.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_corpus_index_projection.rs",
    "rust/crates/tos-compiler/src/source_bibliographic.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
    "rust/crates/tos-command/src/source_creation.rs",
    "rust/crates/tos-command/src/source_creation_store.rs",
    "rust/crates/tos-command/src/source_serialization.rs",
    "rust/crates/tos-compiler/src/knowledge_normalization.rs",
    "rust/crates/tos-command/src/source_private_claim.rs",
    "rust/crates/tos-command/src/source_private_owner_store.rs",
    "rust/crates/tos-command/src/source_work_transaction.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_versions.rs",
];
const RECORD_PATH: &str = "ToS/source-witnesses/agents/parmenides-of-elea/agent.json";
const FORMS_PATH: &str = "ToS/source-witnesses/agents/parmenides-of-elea/agent.human-forms.json";
const HISTORY_PATH: &str =
    "ToS/source-witnesses/agents/parmenides-of-elea/source-revision-history.json";
const FORM_IDS: &[&str] = &[
    "tos.form.parmenides-survival.parmenides.name-ru",
    "tos.form.parmenides-survival.parmenides.name-en",
    "tos.form.parmenides-survival.parmenides.note-ru",
];
const FORM_FIELDS: &[&str] = &[
    "metadata.preferred-name",
    "metadata.variant-name:0",
    "metadata.source-note",
];

type SourceMembers = BTreeMap<String, (Vec<u8>, u32)>;

fn software_component_paths() -> Vec<String> {
    let mut paths = SOFTWARE_COMPONENTS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    paths.extend(
        tos_command::source_claims::CLAIM_GROUNDING_RULE_INPUTS
            .iter()
            .chain(tos_command::source_claims::CLAIM_REVISION_RULE_INPUTS)
            .filter(|path| path.starts_with("rust/"))
            .map(|path| (*path).to_owned()),
    );
    paths.into_iter().collect()
}

fn canonical(value: &Value) -> Vec<u8> {
    let raw = serde_json::to_vec(value).unwrap();
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
    )
    .unwrap()
}

fn repository_root() -> PathBuf {
    match std::env::var_os("TOS_CMD2_LAB_SOURCE_ROOT") {
        Some(path) => {
            let path = PathBuf::from(path);
            assert!(
                path.is_absolute(),
                "selected lab source root must be absolute"
            );
            path.canonicalize().unwrap()
        }
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap(),
    }
}

fn copy_repository_member(
    repository: &Path,
    private_root: &Path,
    members: &mut SourceMembers,
    path: &str,
) {
    let source = repository.join(path);
    let metadata = source
        .symlink_metadata()
        .expect("authored source member exists");
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    let raw = fs::read(&source).expect("authored source member reads");
    let mode = metadata.mode() & 0o7777;
    let target = private_root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, &raw).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        fs::read(&target).unwrap(),
        raw,
        "private fixture copy is byte exact"
    );
    match members.get(path) {
        Some((prior, prior_mode)) => {
            assert_eq!(prior, &raw, "shared source member has one exact value");
            assert_eq!(*prior_mode, mode, "shared source mode is stable");
        }
        None => {
            members.insert(path.to_owned(), (raw, mode));
        }
    }
}

fn selected_members(repository: &Path, private_root: &Path, count: usize) -> SourceMembers {
    assert!(
        count == 1 || count == 3,
        "the bounded comparison is 1 and 3 records"
    );
    fs::create_dir_all(private_root).unwrap();
    fs::set_permissions(private_root, fs::Permissions::from_mode(0o700)).unwrap();
    let mut members = SourceMembers::new();
    for name in AGENT_DIRS.iter().take(count) {
        let relative_dir = format!("ToS/source-witnesses/agents/{name}");
        let source_dir = repository.join(&relative_dir);
        let mut entries = fs::read_dir(&source_dir)
            .expect("selected actual source package opens")
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        assert_eq!(
            entries.len(),
            6,
            "the selected Agent package remains flat v1"
        );
        for entry in entries {
            assert!(entry.file_type().unwrap().is_file());
            let relative_path = format!("{relative_dir}/{}", entry.file_name().to_str().unwrap());
            copy_repository_member(repository, private_root, &mut members, &relative_path);
        }
        let record_path = format!("{relative_dir}/agent.json");
        let record: Value = serde_json::from_slice(&members[&record_path].0).unwrap();
        let refs = record["source_refs"].as_array().unwrap();
        for reference in refs.iter().filter_map(Value::as_str) {
            if reference.starts_with("ToS/") {
                let relative = RelativePath::parse(reference)
                    .expect("authored local source reference is a safe repository path");
                copy_repository_member(repository, private_root, &mut members, relative.as_str());
            }
        }
        assert!(
            !members.contains_key(&format!("{relative_dir}/source-revision-history.json")),
            "selected records have the no-history baseline"
        );
    }
    for path in SHARED_SOURCE {
        copy_repository_member(repository, private_root, &mut members, path);
    }
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file()
            && entry
                .file_name()
                .to_str()
                .unwrap()
                .ends_with(".schema.json")
        {
            let path = format!("ToS/contracts/{}", entry.file_name().to_str().unwrap());
            copy_repository_member(repository, private_root, &mut members, &path);
        }
    }
    assert!(members.len() <= 512);
    assert!(members.values().map(|(raw, _)| raw.len()).sum::<usize>() <= 16 * 1024 * 1024);
    members
}

fn source_counters(members: &SourceMembers) -> Value {
    let contract_members = members
        .iter()
        .filter(|(path, _)| path.starts_with("ToS/contracts/") && path.ends_with(".schema.json"))
        .collect::<Vec<_>>();
    let registry_members = members
        .iter()
        .filter(|(path, _)| {
            path.as_str() == "ToS/doctrine/semantic-interchange/entity-types.v1.json"
                || path.as_str() == "ToS/doctrine/semantic-interchange/relation-types.v1.json"
        })
        .collect::<Vec<_>>();
    let package_members = members
        .iter()
        .filter(|(path, _)| {
            AGENT_DIRS
                .iter()
                .any(|name| path.starts_with(&format!("ToS/source-witnesses/agents/{name}/")))
        })
        .collect::<Vec<_>>();
    let reference_members = members
        .iter()
        .filter(|(path, _)| path.starts_with("ToS/review-ledger/"))
        .collect::<Vec<_>>();
    let bytes = |items: &[(&String, &(Vec<u8>, u32))]| {
        items
            .iter()
            .map(|(_, (raw, _))| raw.len() as u64)
            .sum::<u64>()
    };
    json!({
        "source_members":members.len(),
        "source_bytes":members.values().map(|(raw,_)|raw.len() as u64).sum::<u64>(),
        "contract_schema_files":contract_members.len(),
        "contract_schema_bytes":bytes(&contract_members),
        "registry_files":registry_members.len(),
        "registry_bytes":bytes(&registry_members),
        "agent_package_files":package_members.len(),
        "agent_package_bytes":bytes(&package_members),
        "local_review_reference_files":reference_members.len(),
        "local_review_reference_bytes":bytes(&reference_members)
    })
}

struct SourceCutFixture {
    _scratch: ScratchRoot,
    revision: SourceRevision,
    cut: CorpusCutReader,
    members: SourceMembers,
}

fn source_cut(
    repository: &Path,
    records: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCutFixture {
    let scratch = ScratchRoot::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let copy_root = scratch.0.join("source-copy");
    let members = selected_members(repository, &copy_root, records);
    let source_root = scratch.0.join("snapshot");
    fs::create_dir_all(source_root.join("objects")).unwrap();
    fs::create_dir_all(source_root.join("revisions")).unwrap();
    let files = members
        .iter()
        .map(|(path, (raw, mode))| {
            let copied = fs::read(copy_root.join(path)).unwrap();
            assert_eq!(&copied, raw);
            let sha = Digest256::of_bytes(&copied).to_hex();
            fs::write(source_root.join("objects").join(&sha), &copied).unwrap();
            json!({"path":path,"sha256":sha,"size_bytes":copied.len(),"mode":mode})
        })
        .collect::<Vec<_>>();
    let mut manifest = json!({
        "schema_version":"tos_corpus_snapshot_v1",
        "base_revision":null,
        "files":files,
        "identities":{},
        "dependencies":{},
        "retirements":[],
        "validator_sha256":Digest256::of_bytes(b"private transport fixture; not source admission").to_hex()
    });
    let revision = SourceRevision(Digest256::of_bytes(&canonical(&manifest)));
    manifest["revision"] = json!(revision.0.to_hex());
    let directory = source_root.join("revisions").join(revision.0.to_hex());
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("snapshot.json"), canonical(&manifest)).unwrap();
    let read_limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 2048,
        max_selected_object_bytes: 8_388_608,
        json: JsonLimits::default(),
    };
    let cut = CorpusReader::open_existing(&source_root, read_limits)
        .unwrap()
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 1,
                max_members: 2048,
                max_total_bytes: 33_554_432,
                max_member_bytes: 8_388_608,
            },
            deadline,
            cancelled,
        )
        .unwrap();
    let expectation = cut.stream(revision).unwrap().unwrap().expectation();
    assert_eq!(expectation.count as usize, members.len());
    SourceCutFixture {
        _scratch: scratch,
        revision,
        cut,
        members,
    }
}

struct SoftwareFixture {
    _scratch: ScratchRoot,
    capture: PathBuf,
    restored: PathBuf,
    selection: SoftwareCaptureSelectionV1,
    components: Vec<String>,
}

fn software_fixture(
    repository: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SoftwareFixture {
    let scratch = ScratchRoot::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
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
    assert_eq!(commit.len(), 40);
    let capture = scratch.0.join("software-capture");
    let restored = scratch.0.join("software-restored");
    let components = software_component_paths();
    let include_prefixes = components.clone();
    let captured = tos_source_store::capture_git(
        CaptureGitRequest {
            repository,
            commit: &commit,
            include_prefixes: &include_prefixes,
            exclude_prefixes: &[],
            exclude_path_parts: &[],
            output: &capture,
        },
        GitCaptureLimits {
            max_members: 512,
            max_member_bytes: 2_097_152,
            max_source_bytes: 64 * 1024 * 1024,
            max_metadata_bytes: 8 * 1024 * 1024,
            max_tree_bytes: 8 * 1024 * 1024,
            max_archive_bytes: 64 * 1024 * 1024,
        },
        deadline,
        cancelled,
    )
    .unwrap();
    let manifest_raw = fs::read(capture.join("capture.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&manifest_raw).unwrap();
    assert_eq!(Digest256::of_bytes(&manifest_raw), captured.manifest_sha256);
    let source_git_tree = captured
        .manifest
        .object_get("source_git_tree")
        .and_then(|value| value.as_str())
        .expect("the exact source Git tree is captured")
        .to_owned();
    assert_eq!(
        manifest["source_git_commit"].as_str(),
        Some(commit.as_str())
    );
    assert_eq!(
        manifest["source_git_tree"].as_str(),
        Some(source_git_tree.as_str())
    );
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree,
        capture_manifest_sha256: captured.manifest_sha256,
    };
    let read_limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 2048,
        max_selected_object_bytes: 8_388_608,
        json: JsonLimits::default(),
    };
    tos_source_store::restore_capture(
        &capture,
        &restored,
        &selection,
        CaptureRestoreLimits {
            metadata: read_limits,
            max_archive_bytes: 64 * 1024 * 1024,
            max_decoded_bytes: 64 * 1024 * 1024,
            max_source_bytes: 64 * 1024 * 1024,
        },
        deadline,
        cancelled,
    )
    .unwrap();
    SoftwareFixture {
        _scratch: scratch,
        capture,
        restored,
        selection,
        components,
    }
}

fn software_reader(
    fixture: &SoftwareFixture,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (
    SoftwareCaptureReader,
    tos_source_store::SoftwareComponentSelectionV1,
) {
    let read_limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 2048,
        max_selected_object_bytes: 8_388_608,
        json: JsonLimits::default(),
    };
    let software = SoftwareCaptureReader::open(
        &fixture.capture,
        &fixture.restored,
        fixture.selection.clone(),
        read_limits,
        deadline,
        cancelled,
    )
    .unwrap();
    let paths = fixture
        .components
        .iter()
        .map(|path| RelativePath::parse(path).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    (software, components)
}

struct RevisionCase {
    _owner_scratch: ScratchRoot,
    lab: Lab,
    cohort: tos_command::ManagedSourceCohort,
    source_root: PathBuf,
    original_record: Vec<u8>,
    all_source: SourceMembers,
    initial_head: u64,
    owner_config: PathBuf,
}

fn bootstrap_case(
    repository: &Path,
    software_fixture: &SoftwareFixture,
    count: usize,
    deadline: Instant,
    cancelled: &std::sync::Arc<AtomicBool>,
) -> RevisionCase {
    let mut lab = Lab::new(&database_url());
    fs::set_permissions(&lab._root.0, fs::Permissions::from_mode(0o700)).unwrap();
    let source_root = lab._root.0.canonicalize().unwrap();
    let source = source_cut(repository, count, deadline, cancelled);
    let (software, components) = software_reader(software_fixture, deadline, cancelled);
    let worker_path = PathBuf::from(
        std::env::var_os("TOS_SCHEMA_WORKER_PATH")
            .expect("the exact selected schema worker is supplied by the laboratory"),
    );
    assert!(worker_path.is_absolute());
    let worker_identity = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    let mut worker = CutWorkerSchemaExecutor::from_cut(
        &source.cut,
        tos_validation::FormatProfile::LegacyPythonObserved20260923,
        worker_identity,
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 262_144,
        },
        deadline,
        cancelled,
    )
    .unwrap();
    let owner_scratch = ScratchRoot::new();
    fs::set_permissions(&owner_scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let owner_config = owner_scratch
        .0
        .canonicalize()
        .unwrap()
        .join("revision-owner.json");
    let record_raw = source.members[RECORD_PATH].0.clone();
    let record: Value = serde_json::from_slice(&record_raw).unwrap();
    let uid = rustix::process::getuid().as_raw();
    let config = json!({
        "schema_version":"tos_local_corpus_revision_owner_v1",
        "uid":uid,
        "principal_id":"test:private-pg-source-revision",
        "source_root":source_root,
        "source_path":RECORD_PATH,
        "record_id":record["record_id"],
        "authority_ref":"test:bounded-pg-source-revision",
        "allowed_form_ids":FORM_IDS,
        "allowed_fields":["preferred_label","notes"],
        "allowed_operations":["record.revise"],
        "expires_at":"2099-01-01T00:00:00Z",
        "record_type":"agent"
    });
    let owner_raw = canonical(&config);
    fs::write(&owner_config, &owner_raw).unwrap();
    fs::set_permissions(&owner_config, fs::Permissions::from_mode(0o600)).unwrap();
    let request = json!({"schema_version":"tos_local_source_command_v1","operation":"describe"});
    let context = tos_command::source_command::CommandContext {
        base_revision: source.revision,
        configuration_raw: owner_raw,
        request_raw: canonical(&request),
        recorded_at: "2026-10-11T00:00:00+00:00".to_owned(),
        effective_uid: uid.into(),
        files: components
            .members()
            .map(|member| tos_command::source_command::SourceFile {
                path: member.path.clone(),
                raw: software
                    .read_selected_component(
                        &components,
                        &member.path,
                        2_097_152,
                        deadline,
                        cancelled,
                    )
                    .unwrap(),
            })
            .collect(),
    };
    let initial = lab
        .db
        .bootstrap_source_cohort(
            &lab.store,
            &lab.domain,
            &source.cut,
            source.revision,
            source
                .cut
                .stream(source.revision)
                .unwrap()
                .unwrap()
                .expectation(),
            &context,
            &software,
            &components,
            &mut worker,
            contract_digest(),
            deadline,
            cancelled,
        )
        .expect("the exact copied source cut bootstraps through the PG/STO owner");
    worker.finish(deadline, cancelled).unwrap();
    let initial_head = lab.head_seq();
    assert!(initial_head > 0 && initial_head <= 64);
    assert_eq!(initial.files.get(RECORD_PATH), Some(&record_raw));
    RevisionCase {
        _owner_scratch: owner_scratch,
        lab,
        cohort: initial.cohort,
        source_root,
        original_record: record_raw,
        all_source: source.members,
        initial_head,
        owner_config,
    }
}

fn native_binary() -> PathBuf {
    std::env::var_os("TOS_CMD2_NATIVE_OWNER_COMMAND")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_tos-native-owner-command")))
        .canonicalize()
        .expect("the selected native owner executable exists")
}

fn invocation(
    case: &RevisionCase,
    software: &SoftwareFixture,
    binary: &Path,
    worker_path: &Path,
    invocation_path: &Path,
) {
    let binary_digest = Digest256::of_bytes(&fs::read(binary).unwrap()).to_prefixed();
    let worker_digest = Digest256::of_bytes(&fs::read(worker_path).unwrap()).to_prefixed();
    let value = json!({
        "schema_version":"tos_local_pg_record_revision_invocation_v1",
        "owner_config":case.owner_config,
        "native_executable":binary,
        "native_executable_sha256":binary_digest,
        "source_selection":{
            "domain":case.lab.domain,
            "source_contract_sha256":contract_digest().to_prefixed(),
            "store_root":case.source_root,
            "job_id":"private-job",
            "job_fence":1,
            "store_limits":{
                "max_segment_bytes":limits().max_segment_bytes,
                "max_frame_bytes":limits().max_frame_bytes,
                "max_frames":limits().max_frames,
                "max_journal_bytes":limits().max_journal_bytes
            }
        },
        "software_capture":software.capture,
        "software_restored_root":software.restored,
        "software_selection":{
            "source_git_commit":software.selection.source_git_commit,
            "source_git_tree":software.selection.source_git_tree,
            "capture_manifest_sha256":software.selection.capture_manifest_sha256.to_prefixed()
        },
        "software_components":software.components,
        "schema_worker":{"absolute_path":worker_path,"sha256":worker_digest},
        "budgets":{
            "max_members":512,
            "max_total_bytes":8*1024*1024,
            "max_member_bytes":1_048_576,
            "max_schema_receipts":128,
            "max_schema_receipt_bytes":262_144,
            "worker_cpu_seconds":3,
            "worker_address_space_bytes":1_073_741_824
        }
    });
    assert!(invocation_path.is_absolute());
    fs::write(invocation_path, canonical(&value)).unwrap();
    fs::set_permissions(invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
}

struct NativeInvocation {
    result: Value,
    resources: Value,
}

#[derive(Clone, Copy)]
struct SelfUsage {
    user_cpu_micros: u64,
    system_cpu_micros: u64,
    max_rss_bytes: u64,
    input_block_operations: u64,
    output_block_operations: u64,
    proc_io: Option<(u64, u64)>,
}

fn proc_io_counters(pid: u32) -> Option<(u64, u64)> {
    let raw = fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
    let counter = |name: &str| {
        raw.lines().find_map(|line| {
            line.strip_prefix(&format!("{name}: "))
                .and_then(|value| value.parse::<u64>().ok())
        })
    };
    Some((counter("read_bytes")?, counter("write_bytes")?))
}

fn drain_bounded(mut stream: impl Read, cap: usize) -> std::io::Result<(Vec<u8>, u64)> {
    let mut output = Vec::with_capacity(cap.min(8192));
    let mut total = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok((output, total));
        }
        total = total.saturating_add(read as u64);
        let retained = cap.saturating_sub(output.len()).min(read);
        output.extend_from_slice(&buffer[..retained]);
    }
}

fn self_usage() -> SelfUsage {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) },
        0,
        "test process resource snapshot is available"
    );
    let timeval_micros = |value: libc::timeval| {
        u64::try_from(value.tv_sec)
            .unwrap()
            .saturating_mul(1_000_000)
            .saturating_add(u64::try_from(value.tv_usec).unwrap())
    };
    SelfUsage {
        user_cpu_micros: timeval_micros(usage.ru_utime),
        system_cpu_micros: timeval_micros(usage.ru_stime),
        max_rss_bytes: u64::try_from(usage.ru_maxrss).unwrap().saturating_mul(1024),
        input_block_operations: u64::try_from(usage.ru_inblock).unwrap(),
        output_block_operations: u64::try_from(usage.ru_oublock).unwrap(),
        proc_io: proc_io_counters(std::process::id()),
    }
}

fn isolated_pg_cgroup_snapshot() -> Value {
    let Some(path) = std::env::var_os("TOS_CMD2_PG_CGROUP").map(PathBuf::from) else {
        return json!({
            "available":false,
            "scope":"PostgreSQL cgroup",
            "reason":"TOS_CMD2_PG_CGROUP is not configured"
        });
    };
    if !path.is_absolute() {
        return json!({
            "available":false,
            "scope":"PostgreSQL cgroup",
            "reason":"TOS_CMD2_PG_CGROUP is not absolute"
        });
    }
    let Ok(path) = path.canonicalize() else {
        return json!({
            "available":false,
            "scope":"PostgreSQL cgroup",
            "reason":"TOS_CMD2_PG_CGROUP could not be resolved"
        });
    };
    if !path.starts_with("/sys/fs/cgroup") || !path.is_dir() {
        return json!({
            "available":false,
            "scope":"PostgreSQL cgroup",
            "reason":"TOS_CMD2_PG_CGROUP is not a directory under /sys/fs/cgroup"
        });
    }
    let read = |name: &str| fs::read_to_string(path.join(name)).ok();
    json!({
        "available":true,
        "scope":"exact path selected by TOS_CMD2_PG_CGROUP",
        "io.stat":read("io.stat"),
        "cpu.stat":read("cpu.stat"),
        "memory.current":read("memory.current"),
        "memory.peak":read("memory.peak")
    })
}

fn invoke(
    binary: &Path,
    invocation_path: &Path,
    url: &str,
    request: &Value,
    discard_reply: bool,
) -> NativeInvocation {
    let input = canonical(request);
    assert!(input.len() <= 1_048_576);
    let started = Instant::now();
    let pg_cgroup_before = isolated_pg_cgroup_snapshot();
    let mut command = Command::new(binary);
    command.process_group(0);
    let mut child = command
        .arg("--invocation")
        .arg(invocation_path)
        .env("TOS_SOURCE_PG_URL", url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ordinary native owner command starts");
    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, 1_048_576).unwrap());
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, 1_048_576).unwrap());
    let mut stdin = child.stdin.take().unwrap();
    let stdin_writer = thread::spawn(move || {
        use std::io::Write;
        stdin.write_all(&input)
    });
    let deadline = started + Duration::from_secs(30);
    let mut exited = false;
    loop {
        let mut siginfo: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let waited = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut siginfo,
                libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
            )
        };
        if waited == 0 {
            if unsafe { siginfo.si_pid() } != 0 {
                exited = true;
                break;
            }
        } else {
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EINTR),
                "native owner waitid reaches process exit"
            );
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    if !exited {
        // The native child owns a private process group so a stuck helper and
        // its descendants cannot keep the concurrently drained pipes open.
        unsafe {
            libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
        }
        let _ = child.kill();
        let mut siginfo: libc::siginfo_t = unsafe { std::mem::zeroed() };
        loop {
            let waited = unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid as libc::id_t,
                    &mut siginfo,
                    libc::WEXITED | libc::WNOWAIT,
                )
            };
            if waited == 0 {
                break;
            }
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EINTR),
                "timed-out native owner is reaped after termination"
            );
        }
    } else {
        // The entry point should reap its own helpers. If it leaves any child
        // in its private group, terminate it so it cannot keep our pipes open.
        unsafe {
            libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
        }
    }
    let proc_io = proc_io_counters(pid);
    let mut status = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let reaped = unsafe { libc::wait4(pid as libc::pid_t, &mut status, 0, &mut usage) };
    assert_eq!(reaped, pid as libc::pid_t, "native owner process is reaped");
    let (stdout, stdout_bytes) = stdout_reader.join().unwrap();
    let (stderr, stderr_bytes) = stderr_reader.join().unwrap();
    let stdin_result = stdin_writer.join().unwrap();
    let pg_cgroup_after = isolated_pg_cgroup_snapshot();
    assert!(
        stdout_bytes <= 1_048_576 && stderr_bytes <= 1_048_576,
        "native invocation output remains bounded"
    );
    assert!(
        exited,
        "native invocation completed within its 30 second deadline: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert_eq!(
        status,
        0,
        "native invocation refused: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        stdin_result.is_ok(),
        "native invocation accepted the bounded request"
    );
    let result = if discard_reply {
        Value::Null
    } else {
        let response: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(
            response["schema_version"],
            "tos_local_pg_record_revision_result_v1"
        );
        assert_eq!(response["grants_admission"], false);
        response["result"].clone()
    };
    let timeval_micros = |value: libc::timeval| {
        u64::try_from(value.tv_sec)
            .unwrap()
            .saturating_mul(1_000_000)
            .saturating_add(u64::try_from(value.tv_usec).unwrap())
    };
    NativeInvocation {
        result,
        resources: json!({
            "scope":"native subprocess direct-child wait4 rusage and /proc PID I/O; excludes PostgreSQL server and is not whole-transaction physical I/O",
            "wall_micros":u64::try_from(started.elapsed().as_micros()).unwrap(),
            "wait4_user_cpu_micros":timeval_micros(usage.ru_utime),
            "wait4_system_cpu_micros":timeval_micros(usage.ru_stime),
            "wait4_max_rss_bytes":u64::try_from(usage.ru_maxrss).unwrap().saturating_mul(1024),
            "wait4_input_block_operations":u64::try_from(usage.ru_inblock).unwrap(),
            "wait4_output_block_operations":u64::try_from(usage.ru_oublock).unwrap(),
            "proc_read_bytes":proc_io.map(|(read, _)| read),
            "proc_write_bytes":proc_io.map(|(_, write)| write),
            "postgres_cgroup":{"before":pg_cgroup_before,"after":pg_cgroup_after},
            "stdout_bytes":stdout_bytes,
            "stderr_bytes":stderr_bytes
        }),
    }
}

fn source_member(case: &mut RevisionCase, path: &str) -> Vec<u8> {
    case.lab
        .db
        .read_current_source_member(
            &case.lab.store,
            &case.cohort,
            &RelativePath::parse(path).unwrap(),
            1_048_576,
            Instant::now() + Duration::from_secs(15),
            &AtomicBool::new(false),
        )
        .unwrap()
        .raw
}

fn projection_rows(case: &RevisionCase, consumer: &str) -> Vec<String> {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    db.query(
        "SELECT row_to_json(p)::text FROM (
             SELECT source_path,document_id,source_revision,title,body,aliases,
                    identity,document_type
             FROM cmd2_projection_document
             WHERE domain=$1 AND consumer_id=$2
             ORDER BY source_path,document_id
         ) p",
        &[&case.lab.domain, &consumer],
    )
    .unwrap()
    .into_iter()
    .map(|row| row.get::<_, String>(0))
    .collect()
}

fn projection_documents(rows: &[String]) -> BTreeMap<(String, String), Value> {
    rows.iter()
        .map(|row| {
            let value: Value = serde_json::from_str(row).unwrap();
            let key = (
                value["source_path"].as_str().unwrap().to_owned(),
                value["document_id"].as_str().unwrap().to_owned(),
            );
            (key, value)
        })
        .collect()
}

fn history_source_paths(case: &RevisionCase, seq: u64) -> Vec<String> {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    db.query(
        "SELECT subject FROM cmd2_history
         WHERE domain=$1 AND commit_seq=$2 AND source_mode IS NOT NULL
         ORDER BY subject COLLATE \"C\"",
        &[&case.lab.domain, &(seq as i64)],
    )
    .unwrap()
    .into_iter()
    .map(|row| row.get(0))
    .collect()
}

fn record_correction(case: &mut RevisionCase, software: &SoftwareFixture, binary: &Path) -> Value {
    let worker_path = PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").unwrap());
    let invocation_path = case.source_root.join("revision-invocation.json");
    invocation(case, software, binary, &worker_path, &invocation_path);
    let described_call = invoke(
        binary,
        &invocation_path,
        &case.lab.url,
        &json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
        false,
    );
    let described = described_call.result;
    let describe_resources = described_call.resources;
    assert_eq!(
        described["owner_configuration"]["record_id"],
        "tos.agent.parmenides-of-elea"
    );
    let preview_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-revise",
        "fields":{
            "preferred_label":"Парменид Элейский Quartz",
            "notes":"Частная интеграционная фикстура PostgreSQL; семантическое признание не выполнялось."
        },
        "forms":FORM_IDS.iter().zip(FORM_FIELDS).map(|(form_id,field_id)| json!({"form_id":form_id,"field_id":field_id})).collect::<Vec<_>>(),
        "reason":"Isolated PG mechanics fixture only; no source, semantic, rights, or canon admission."
    });
    let preview_call = invoke(
        binary,
        &invocation_path,
        &case.lab.url,
        &preview_request,
        false,
    );
    let preview = preview_call.result;
    let prepare_resources = preview_call.resources;
    assert_eq!(
        preview["owner_configuration"],
        described["owner_configuration"]
    );
    let mut request = preview_request;
    request["operation"] = json!("record.revise");
    request["command_id"] = json!("test:pg-parmenides-correction");
    for (field, prepared) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_dependencies", "expected_dependencies"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
    ] {
        request[field] = preview[prepared].clone();
    }
    let before_head = case.lab.head_seq();
    let before_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    assert!(before_head >= case.initial_head);
    // The first process commits successfully; dropping its reply exercises the
    // exact same-command retry path without changing any repository source.
    let committed_call = invoke(binary, &invocation_path, &case.lab.url, &request, true);
    let commit_resources = committed_call.resources;
    let committed_head = case.lab.head_seq();
    assert_eq!(
        committed_head,
        before_head + 1,
        "one revision is one durable event"
    );
    let committed_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    assert_eq!(
        committed_counts[4],
        before_counts[4] + 1,
        "one outbox event is committed"
    );
    let current_record = source_member(case, RECORD_PATH);
    let retry_call = invoke(binary, &invocation_path, &case.lab.url, &request, false);
    let retry = retry_call.result;
    let retry_resources = retry_call.resources;
    assert_eq!(
        retry["replayed"], true,
        "same exact command recovers its lost reply"
    );
    assert_eq!(case.lab.head_seq(), committed_head);
    assert_eq!(
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table)),
        committed_counts,
        "exact retry leaves every durable counter unchanged"
    );
    assert_eq!(source_member(case, RECORD_PATH), current_record);
    let after: Value = serde_json::from_slice(&current_record).unwrap();
    let mut expected: Value = serde_json::from_slice(&case.original_record).unwrap();
    expected["preferred_label"] = json!("Парменид Элейский Quartz");
    expected["notes"] = json!(
        "Частная интеграционная фикстура PostgreSQL; семантическое признание не выполнялось."
    );
    expected["record_version"] = json!(2);
    assert_eq!(
        after, expected,
        "the PG current source matches the prepared correction"
    );
    assert_ne!(before_counts, committed_counts);
    let paths = history_source_paths(case, committed_head);
    for required in [RECORD_PATH, FORMS_PATH, HISTORY_PATH] {
        assert!(
            paths.iter().any(|path| path == required),
            "revision records {required}"
        );
    }
    let forms: Value = serde_json::from_slice(&source_member(case, FORMS_PATH)).unwrap();
    assert_eq!(forms["forms"].as_array().unwrap().len(), 3);
    assert!(
        forms["forms"]
            .as_array()
            .unwrap()
            .iter()
            .all(|form| form["form_version"] == 2)
    );
    let retained = case
        .lab
        .db
        .cold_recover_exact(&case.lab.store, &case.lab.domain, RECORD_PATH, 1)
        .expect("the original Parmenides record locator remains retained");
    assert_eq!(
        case.lab
            .db
            .warm_read_selected(&case.lab.store, &retained, 1_048_576)
            .unwrap(),
        case.original_record,
        "the previous source bytes are exactly recoverable"
    );
    let mut updated = BTreeSet::from([
        RECORD_PATH.to_owned(),
        FORMS_PATH.to_owned(),
        HISTORY_PATH.to_owned(),
    ]);
    let archive_path = {
        let history: Value = serde_json::from_slice(&source_member(case, HISTORY_PATH)).unwrap();
        history["receipts"].as_array().unwrap().last().unwrap()["archive_path"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert!(
        paths.iter().all(|path| {
            path == RECORD_PATH
                || path == FORMS_PATH
                || path == HISTORY_PATH
                || path.starts_with(&format!("{archive_path}/"))
        }),
        "the durable event is limited to the selected package and retained archive"
    );
    let archive_manifest_path = format!("{archive_path}/manifest.json");
    updated.insert(archive_manifest_path.clone());
    // All pre-existing source paths outside the selected record, forms,
    // history, and its retained archive remain byte-identical.
    let baseline = case
        .all_source
        .iter()
        .map(|(path, (raw, _))| (path.clone(), raw.clone()))
        .collect::<Vec<_>>();
    for (path, raw) in baseline {
        if updated.contains(&path) {
            continue;
        }
        assert_eq!(
            source_member(case, &path),
            raw,
            "unaffected source path {path}"
        );
    }
    let archive_manifest: Value =
        serde_json::from_slice(&source_member(case, &archive_manifest_path)).unwrap();
    let binding = &archive_manifest["files"]["agent.json"];
    let blob_path = format!("{archive_path}/{}", binding["blob"].as_str().unwrap());
    assert_eq!(source_member(case, &blob_path), case.original_record);
    json!({
        "before_head":before_head,
        "committed_head":committed_head,
        "before_counts":before_counts,
        "committed_counts":committed_counts,
        "changed_source_paths":paths,
        "native_invocations":{
            "describe":describe_resources,
            "prepare_revise":prepare_resources,
            "record_revise_reply_dropped":commit_resources,
            "same_command_retry":retry_resources
        },
        "source_bytes":case.all_source.values().map(|(raw,_)|raw.len() as u64).sum::<u64>(),
        "current_record_bytes":current_record.len(),
        "current_record_sha256":Digest256::of_bytes(&current_record).to_hex(),
        "retained_original_bytes":case.original_record.len(),
        "retained_original_sha256":Digest256::of_bytes(&case.original_record).to_hex(),
        "replayed":retry["replayed"],
        "archive_manifest_path":archive_manifest_path
    })
}

fn project_case(
    case: &mut RevisionCase,
    software: &SoftwareFixture,
    records: usize,
    binary: &Path,
) -> Value {
    let consumer = format!("private-pg-real-revision-{records}");
    let profile = DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
        version: 1,
        source_contract: contract_digest(),
    };
    case.lab.db.init_projection_schema().unwrap();
    let extractor = tos_command::source_projection_cli::SourceTextExtractor;
    let initial_head = case.lab.head_seq();
    let initial_limit = usize::try_from(initial_head).unwrap();
    assert!((1..=256).contains(&initial_limit));
    let initial = case
        .lab
        .db
        .apply_projection_batch(
            &case.lab.store,
            &case.lab.domain,
            &consumer,
            &profile,
            &extractor,
            DurableProjectionLimits {
                max_events: initial_limit,
                ..DurableProjectionLimits::default()
            },
        )
        .expect("all bounded bootstrap events project from committed STO bytes");
    let initial_work = serde_json::to_value(&initial.work).unwrap();
    assert!(matches!(
        initial.status,
        DurableProjectionStatus::Current { through_seq } if through_seq == initial_head
    ));
    let before_rows = projection_rows(case, &consumer);
    assert!(
        before_rows
            .iter()
            .any(|row| row.contains("tos.agent.parmenides-of-elea"))
    );
    if records == 3 {
        assert!(
            before_rows
                .iter()
                .any(|row| row.contains("tos.agent.justus-lipsius"))
        );
        assert!(
            before_rows
                .iter()
                .any(|row| row.contains("tos.agent.hans-hahn"))
        );
    }
    let receipt = record_correction(case, software, binary);
    let revision_apply_started = Instant::now();
    let pg_cgroup_before_apply = isolated_pg_cgroup_snapshot();
    let self_before_apply = self_usage();
    let one = case
        .lab
        .db
        .apply_projection_batch(
            &case.lab.store,
            &case.lab.domain,
            &consumer,
            &profile,
            &extractor,
            DurableProjectionLimits {
                max_events: 1,
                ..DurableProjectionLimits::default()
            },
        )
        .expect("one committed source correction projects atomically");
    let revision_apply_wall_micros =
        u64::try_from(revision_apply_started.elapsed().as_micros()).unwrap();
    let self_after_apply = self_usage();
    let pg_cgroup_after_apply = isolated_pg_cgroup_snapshot();
    let revision_work = serde_json::to_value(&one.work).unwrap();
    let self_proc_io_delta = match (self_before_apply.proc_io, self_after_apply.proc_io) {
        (Some((read_before, write_before)), Some((read_after, write_after))) => json!({
            "available":true,
            "read_bytes":read_after.saturating_sub(read_before),
            "write_bytes":write_after.saturating_sub(write_before)
        }),
        _ => json!({"available":false,"reason":"/proc/self/io was unavailable"}),
    };
    let revision_apply_resources = json!({
        "scope":"single revision apply_projection_batch call; test-process rusage and /proc/self/io deltas exclude PostgreSQL server",
        "wall_micros":revision_apply_wall_micros,
        "test_process":{
            "user_cpu_delta_micros":self_after_apply.user_cpu_micros.saturating_sub(self_before_apply.user_cpu_micros),
            "system_cpu_delta_micros":self_after_apply.system_cpu_micros.saturating_sub(self_before_apply.system_cpu_micros),
            "max_rss_after_bytes":self_after_apply.max_rss_bytes,
            "input_block_operation_delta":self_after_apply.input_block_operations.saturating_sub(self_before_apply.input_block_operations),
            "output_block_operation_delta":self_after_apply.output_block_operations.saturating_sub(self_before_apply.output_block_operations),
            "proc_io_delta":self_proc_io_delta
        },
        "postgres_cgroup":{"before":pg_cgroup_before_apply,"after":pg_cgroup_after_apply}
    });
    assert!(matches!(
        one.status,
        DurableProjectionStatus::Current { through_seq } if through_seq == case.lab.head_seq()
    ));
    let after_rows = projection_rows(case, &consumer);
    assert_eq!(
        before_rows.len(),
        after_rows.len(),
        "one record correction keeps document cardinality"
    );
    let before_documents = projection_documents(&before_rows);
    let after_documents = projection_documents(&after_rows);
    assert_eq!(
        before_documents.keys().collect::<Vec<_>>(),
        after_documents.keys().collect::<Vec<_>>()
    );
    let changed_documents = before_documents
        .iter()
        .filter(|(key, before)| after_documents.get(*key) != Some(*before))
        .map(|(key, _)| key)
        .collect::<Vec<_>>();
    assert_eq!(
        changed_documents.len(),
        1,
        "only the selected text document changes"
    );
    assert_eq!(changed_documents[0].0, RECORD_PATH);
    assert_eq!(changed_documents[0].1, "tos.agent.parmenides-of-elea");
    assert!(
        !serde_json::to_string(&before_documents[changed_documents[0]])
            .unwrap()
            .contains("Quartz")
    );
    assert!(
        serde_json::to_string(&after_documents[changed_documents[0]])
            .unwrap()
            .contains("Quartz")
    );
    let changed: BTreeSet<String> = history_source_paths(case, case.lab.head_seq())
        .into_iter()
        .collect();
    let unaffected = |rows: &[String]| {
        rows.iter()
            .filter(|row| {
                let value: Value = serde_json::from_str(row).unwrap();
                !changed.contains(value["source_path"].as_str().unwrap())
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(unaffected(&before_rows), unaffected(&after_rows));
    let search = case
        .lab
        .db
        .search_projection(
            &case.lab.domain,
            &consumer,
            &profile,
            "Quartz",
            10,
            Duration::from_secs(15),
        )
        .unwrap();
    assert!(matches!(
        search,
        DurableProjectionSearch::Current { ref hits, through_seq }
            if through_seq == case.lab.head_seq()
                && hits.iter().any(|hit| hit.document_id == "tos.agent.parmenides-of-elea")
    ));
    json!({
        "consumer":consumer,
        "initial_documents":before_rows.len(),
        "final_documents":after_rows.len(),
        "projection_cursor":case.lab.head_seq(),
        "new_text_search":"Quartz",
        "projection_work":{"bootstrap":initial_work,"revision":revision_work},
        "revision_apply_resources":revision_apply_resources,
        "revision":receipt
    })
}

#[test]
fn real_parmenides_revision_retries_projects_and_preserves_growth_unaffected_paths() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let mut one = bootstrap_case(&repository, &software, 1, deadline, &cancelled);
    let mut three = bootstrap_case(&repository, &software, 3, deadline, &cancelled);
    let binary = native_binary();
    let one_projection = project_case(&mut one, &software, 1, &binary);
    let three_projection = project_case(&mut three, &software, 3, &binary);
    for counter in [
        "examined_event_members",
        "exact_source_members_read",
        "exact_source_bytes_read",
        "extracted_documents",
        "published_source_paths",
        "published_documents",
    ] {
        assert_eq!(
            one_projection["projection_work"]["revision"][counter],
            three_projection["projection_work"]["revision"][counter],
            "the one-record correction operation has the same {counter} work at 1 and 3 records"
        );
    }
    let output = json!({
        "schema_version":"tos_cmd2_real_source_revision_test_v1",
        "source_record":"tos.agent.parmenides-of-elea",
        "bootstrap_cases":[
            {"record_count":1,"source":source_counters(&one.all_source),"projection":one_projection},
            {"record_count":3,"source":source_counters(&three.all_source),"projection":three_projection}
        ],
        "authored_repository_mutated":false,
        "semantic_admission":false
    });
    println!(
        "TOS_PG_REAL_REVISION {}",
        serde_json::to_string(&output).unwrap()
    );
}
