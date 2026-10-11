//! Bounded real-source revision exercise over private PostgreSQL and STO.
//! The selected authored bytes are copied into a private fixture before any
//! capture or mutation; the repository remains read-only.

use super::{Lab, ScratchRoot, contract_digest, database_url, limits};
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tos_command::{
    DurablePgCoordinator, DurableProjectionExtractor, DurableProjectionLimits,
    DurableProjectionProfile, DurableProjectionSearch, DurableProjectionStatus,
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
    "rust/crates/tos-command/src/source_creation_pg.rs",
    "rust/crates/tos-command/src/source_native_pg_creation_cli.rs",
    "rust/crates/tos-command/src/source_cohort_creation.rs",
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
const CREATED_RECORD_PATH: &str = "ToS/source-witnesses/agents/pg-real-addition-fixture/agent.json";
const CREATED_RECORD_ID: &str = "tos.agent.pg-real-addition-fixture";
const CREATED_FORM_ID: &str = "tos.form.pg-real-addition-fixture.preferred-name";
const NIETZSCHE_DIR: &str = "friedrich-nietzsche";
const NIETZSCHE_RECORD_PATH: &str = "ToS/source-witnesses/agents/friedrich-nietzsche/agent.json";
const NIETZSCHE_FORMS_PATH: &str =
    "ToS/source-witnesses/agents/friedrich-nietzsche/agent.human-forms.json";
const NIETZSCHE_HISTORY_PATH: &str =
    "ToS/source-witnesses/agents/friedrich-nietzsche/source-revision-history.json";
const NIETZSCHE_FORM_IDS: &[&str] = &[
    "tos.form.metadata.ee414f73c4461ed7de9865bd361798a4fc4ad33b4c9014cd42f69b8868cf0eb3",
    "tos.form.metadata.ef612d54f6556810586d9fcbb0c77cfb0058a7f8f0dfde43a47f9d4df5102484",
    "tos.form.metadata.c2976c14db396fd6f5a82355a301899a7cf5e18fcf6455cbea0ca056146cce6d",
];
const NIETZSCHE_FORM_FIELDS: &[&str] = &[
    "metadata.preferred-name",
    "metadata.source-note",
    "metadata.variant-name:0",
];
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

#[derive(Clone, Copy)]
struct RevisionTarget {
    source_path: &'static str,
    forms_path: &'static str,
    history_path: &'static str,
    record_id: &'static str,
    command_id: &'static str,
    form_ids: &'static [&'static str],
    form_fields: &'static [&'static str],
    allowed_fields: &'static [&'static str],
    preferred_label: &'static str,
    notes: Option<&'static str>,
    resulting_version: u64,
    resulting_form_version: u64,
}

const PARMENIDES_TARGET: RevisionTarget = RevisionTarget {
    source_path: RECORD_PATH,
    forms_path: FORMS_PATH,
    history_path: HISTORY_PATH,
    record_id: "tos.agent.parmenides-of-elea",
    command_id: "test:pg-parmenides-correction",
    form_ids: FORM_IDS,
    form_fields: FORM_FIELDS,
    allowed_fields: &["preferred_label", "notes"],
    preferred_label: "Парменид Элейский Quartz",
    notes: Some(
        "Частная интеграционная фикстура PostgreSQL; семантическое признание не выполнялось.",
    ),
    resulting_version: 2,
    resulting_form_version: 2,
};

const NIETZSCHE_TARGET: RevisionTarget = RevisionTarget {
    source_path: NIETZSCHE_RECORD_PATH,
    forms_path: NIETZSCHE_FORMS_PATH,
    history_path: NIETZSCHE_HISTORY_PATH,
    record_id: "tos.agent.friedrich-nietzsche",
    command_id: "test:pg-nietzsche-shared-source-correction",
    form_ids: NIETZSCHE_FORM_IDS,
    form_fields: NIETZSCHE_FORM_FIELDS,
    allowed_fields: &["preferred_label"],
    preferred_label: "Friedrich Nietzsche Quartz",
    notes: None,
    resulting_version: 4,
    resulting_form_version: 3,
};

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

fn selected_shared_source_members(
    repository: &Path,
    private_root: &Path,
    count: usize,
) -> SourceMembers {
    assert!(count == 1 || count == 3);
    fs::create_dir_all(private_root).unwrap();
    fs::set_permissions(private_root, fs::Permissions::from_mode(0o700)).unwrap();
    let selected = if count == 1 {
        vec![NIETZSCHE_DIR]
    } else {
        vec![NIETZSCHE_DIR, "parmenides-of-elea", "justus-lipsius"]
    };
    let mut members = SourceMembers::new();
    for name in selected {
        let relative_dir = format!("ToS/source-witnesses/agents/{name}");
        let source_dir = repository.join(&relative_dir);
        let mut entries = fs::read_dir(&source_dir)
            .expect("selected actual Agent package opens")
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        assert!(
            entries.len() == 3 || entries.len() == 6,
            "selected package has its exact authored flat inventory"
        );
        for entry in entries {
            assert!(entry.file_type().unwrap().is_file());
            let relative_path = format!("{relative_dir}/{}", entry.file_name().to_str().unwrap());
            copy_repository_member(repository, private_root, &mut members, &relative_path);
        }
        let record_path = format!("{relative_dir}/agent.json");
        let record: Value = serde_json::from_slice(&members[&record_path].0).unwrap();
        for reference in record["source_refs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
        {
            if reference.starts_with("ToS/") {
                let relative = RelativePath::parse(reference)
                    .expect("authored local source reference is a safe repository path");
                copy_repository_member(repository, private_root, &mut members, relative.as_str());
            }
        }
        if name == NIETZSCHE_DIR {
            let history_path = format!("{relative_dir}/source-revision-history.json");
            let history: Value = serde_json::from_slice(&members[&history_path].0).unwrap();
            let receipts = history["receipts"]
                .as_array()
                .filter(|receipts| !receipts.is_empty())
                .expect("the current Nietzsche history has retained prior revisions");
            for receipt in receipts {
                let archive_path = RelativePath::parse(
                    receipt["archive_path"]
                        .as_str()
                        .expect("history names its exact retained archive"),
                )
                .expect("the retained archive path is safe");
                let manifest_path = format!("{}/manifest.json", archive_path.as_str());
                copy_repository_member(repository, private_root, &mut members, &manifest_path);
                let manifest: Value = serde_json::from_slice(&members[&manifest_path].0).unwrap();
                let archived_files = manifest["files"]
                    .as_object()
                    .expect("the selected retained archive has its file map");
                assert_eq!(manifest["schema_version"], "tos_source_package_archive_v2");
                assert_eq!(
                    archived_files
                        .keys()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from(["agent.json", "agent.human-forms.json"]),
                    "the retained Nietzsche revision has its exact record and HumanForms members"
                );
                for binding in archived_files.values() {
                    let blob = binding["blob"]
                        .as_str()
                        .expect("archive manifest names exact blob bytes");
                    let blob_path =
                        RelativePath::parse(&format!("{}/{}", archive_path.as_str(), blob))
                            .expect("retained archive blob path is safe");
                    copy_repository_member(
                        repository,
                        private_root,
                        &mut members,
                        blob_path.as_str(),
                    );
                    let (raw, _) = &members[blob_path.as_str()];
                    assert_eq!(raw.len() as u64, binding["bytes"].as_u64().unwrap());
                    let expected_sha256 = format!("sha256:{}", Digest256::of_bytes(raw).to_hex());
                    assert_eq!(
                        binding["sha256"].as_str(),
                        Some(expected_sha256.as_str()),
                        "the selected archive blob matches its exact manifest digest"
                    );
                }
            }
        }
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
                .chain(std::iter::once(&NIETZSCHE_DIR))
                .any(|name| path.starts_with(&format!("ToS/source-witnesses/agents/{name}/")))
                || path.starts_with("ToS/source-witnesses/.record-revisions/")
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
    source_cut_from_members(scratch, copy_root, members, deadline, cancelled)
}

fn shared_source_cut(
    repository: &Path,
    records: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCutFixture {
    let scratch = ScratchRoot::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let copy_root = scratch.0.join("source-copy");
    let members = selected_shared_source_members(repository, &copy_root, records);
    source_cut_from_members(scratch, copy_root, members, deadline, cancelled)
}

fn source_cut_from_members(
    scratch: ScratchRoot,
    copy_root: PathBuf,
    members: SourceMembers,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCutFixture {
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
    target: RevisionTarget,
}

fn bootstrap_case(
    repository: &Path,
    software_fixture: &SoftwareFixture,
    count: usize,
    deadline: Instant,
    cancelled: &std::sync::Arc<AtomicBool>,
) -> RevisionCase {
    let source = source_cut(repository, count, deadline, cancelled);
    bootstrap_case_from_cut(
        software_fixture,
        source,
        PARMENIDES_TARGET,
        deadline,
        cancelled,
    )
}

fn bootstrap_shared_case(
    repository: &Path,
    software_fixture: &SoftwareFixture,
    count: usize,
    deadline: Instant,
    cancelled: &std::sync::Arc<AtomicBool>,
) -> RevisionCase {
    let source = shared_source_cut(repository, count, deadline, cancelled);
    bootstrap_case_from_cut(
        software_fixture,
        source,
        NIETZSCHE_TARGET,
        deadline,
        cancelled,
    )
}

fn bootstrap_case_from_cut(
    software_fixture: &SoftwareFixture,
    source: SourceCutFixture,
    target: RevisionTarget,
    deadline: Instant,
    cancelled: &std::sync::Arc<AtomicBool>,
) -> RevisionCase {
    let mut lab = Lab::new(&database_url());
    fs::set_permissions(&lab._root.0, fs::Permissions::from_mode(0o700)).unwrap();
    let source_root = lab._root.0.canonicalize().unwrap();
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
    let record_raw = source.members[target.source_path].0.clone();
    let record: Value = serde_json::from_slice(&record_raw).unwrap();
    let uid = rustix::process::getuid().as_raw();
    let config = json!({
        "schema_version":"tos_local_corpus_revision_owner_v1",
        "uid":uid,
        "principal_id":"test:private-pg-source-revision",
        "source_root":source_root,
        "source_path":target.source_path,
        "record_id":record["record_id"],
        "authority_ref":"test:bounded-pg-source-revision",
        "allowed_form_ids":target.form_ids,
        "allowed_fields":target.allowed_fields,
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
    assert_eq!(initial.files.get(target.source_path), Some(&record_raw));
    RevisionCase {
        _owner_scratch: owner_scratch,
        lab,
        cohort: initial.cohort,
        source_root,
        original_record: record_raw,
        all_source: source.members,
        initial_head,
        owner_config,
        target,
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
    owner_config: &Path,
    schema_version: &str,
) {
    let binary_digest = Digest256::of_bytes(&fs::read(binary).unwrap()).to_prefixed();
    let worker_digest = Digest256::of_bytes(&fs::read(worker_path).unwrap()).to_prefixed();
    let value = json!({
        "schema_version":schema_version,
        "owner_config":owner_config,
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
    dropped_source_read_work: Option<Value>,
    resources: Value,
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

struct KillAfterFirstExtract {
    marker: PathBuf,
    calls: AtomicUsize,
}

impl tos_command::DurableProjectionExtractor for KillAfterFirstExtract {
    fn extract(
        &self,
        source_path: &str,
        revision: u64,
        exact_source_bytes: &[u8],
    ) -> tos_command::DurableResult<Vec<tos_command::DurableProjectionDocument>> {
        let documents = tos_command::source_projection_cli::SourceTextExtractor.extract(
            source_path,
            revision,
            exact_source_bytes,
        )?;
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            use std::io::Write;
            let pending_marker = self.marker.with_extension("pending");
            let mut marker = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&pending_marker)
                .expect("private projection interruption marker is created once");
            marker
                .write_all(b"production source extraction reached during event staging")
                .unwrap();
            marker.sync_all().unwrap();
            drop(marker);
            fs::rename(pending_marker, &self.marker)
                .expect("ready marker appears only after its contents are durable");
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        Ok(documents)
    }
}

/// Invoked only by the parent integration test after it has committed one
/// real source event. The parent kills this process after the production
/// extractor has run once, while `apply_projection_batch` stages that event.
#[test]
fn pg_real_projection_interrupt_child_entry() {
    let Some(marker) = std::env::var_os("TOS_PG_REAL_PROJECTION_MARKER") else {
        return;
    };
    let domain = std::env::var("TOS_PG_REAL_PROJECTION_DOMAIN").unwrap();
    let consumer = std::env::var("TOS_PG_REAL_PROJECTION_CONSUMER").unwrap();
    let store_root = PathBuf::from(std::env::var_os("TOS_PG_REAL_PROJECTION_STORE").unwrap());
    let url = super::database_url();
    let mut db = tos_command::DurablePgCoordinator::connect(&url).unwrap();
    let store =
        tos_segment_store::SegmentStore::open_existing(&store_root, super::limits()).unwrap();
    let profile = tos_command::DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
        version: 1,
        source_contract: super::contract_digest(),
    };
    let extractor = KillAfterFirstExtract {
        marker: PathBuf::from(marker),
        calls: AtomicUsize::new(0),
    };
    db.apply_projection_batch(
        &store,
        &domain,
        &consumer,
        &profile,
        &extractor,
        tos_command::DurableProjectionLimits {
            max_events: 1,
            ..tos_command::DurableProjectionLimits::default()
        },
    )
    .expect("the child is killed while one real outbox event is in flight");
    panic!("projection child unexpectedly completed before its parent killed it");
}

/// Opens the committed database through a new process after the extraction
/// process was killed, then resumes from the still-committed projection cursor.
#[test]
fn pg_real_projection_resume_child_entry() {
    if std::env::var_os("TOS_PG_REAL_PROJECTION_RESUME").is_none() {
        return;
    }
    let domain = std::env::var("TOS_PG_REAL_PROJECTION_DOMAIN").unwrap();
    let consumer = std::env::var("TOS_PG_REAL_PROJECTION_CONSUMER").unwrap();
    let store_root = PathBuf::from(std::env::var_os("TOS_PG_REAL_PROJECTION_STORE").unwrap());
    let mut db = tos_command::DurablePgCoordinator::connect(&super::database_url()).unwrap();
    let store =
        tos_segment_store::SegmentStore::open_existing(&store_root, super::limits()).unwrap();
    let profile = tos_command::DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
        version: 1,
        source_contract: super::contract_digest(),
    };
    let progress = db
        .apply_projection_batch(
            &store,
            &domain,
            &consumer,
            &profile,
            &tos_command::source_projection_cli::SourceTextExtractor,
            tos_command::DurableProjectionLimits {
                max_events: 1,
                ..tos_command::DurableProjectionLimits::default()
            },
        )
        .expect("fresh process resumes the committed source event");
    assert!(matches!(
        progress.status,
        tos_command::DurableProjectionStatus::Current { .. }
    ));
    let work = serde_json::to_value(&progress.work).unwrap();
    println!(
        "TOS_PG_REAL_PROJECTION_RESUME_RESULT {}",
        serde_json::to_string(&work).unwrap()
    );
}

fn resume_projection_in_fresh_process(case: &RevisionCase, consumer: &str) -> (Value, Value) {
    let started = Instant::now();
    let pg_cgroup_before = isolated_pg_cgroup_snapshot();
    let executable = std::env::current_exe().expect("integration test executable is available");
    let mut command = Command::new(executable);
    command.process_group(0);
    let mut child = command
        .arg("pg_real_projection_resume_child_entry")
        .arg("--nocapture")
        .env("TOS_CMD_POSTGRES_URL", &case.lab.url)
        .env("TOS_SOURCE_PG_URL", &case.lab.url)
        .env("TOS_PG_REAL_PROJECTION_RESUME", "1")
        .env("TOS_PG_REAL_PROJECTION_DOMAIN", &case.lab.domain)
        .env("TOS_PG_REAL_PROJECTION_CONSUMER", consumer)
        .env("TOS_PG_REAL_PROJECTION_STORE", &case.lab._root.0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("fresh projection process starts");
    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, 1_048_576).unwrap());
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, 1_048_576).unwrap());
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
                "fresh projection process wait reaches exit"
            );
        }
        if Instant::now() >= deadline {
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
                    "timed-out fresh projection process is stopped"
                );
            }
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let proc_io = proc_io_counters(pid);
    let mut status = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let reaped = unsafe { libc::wait4(pid as libc::pid_t, &mut status, 0, &mut usage) };
    assert_eq!(
        reaped, pid as libc::pid_t,
        "fresh projection process is reaped"
    );
    let (stdout, stdout_bytes) = stdout_reader.join().unwrap();
    let (stderr, stderr_bytes) = stderr_reader.join().unwrap();
    assert!(stdout_bytes <= 1_048_576 && stderr_bytes <= 1_048_576);
    assert!(
        exited && status == 0,
        "fresh projection process completes within 30 seconds: status={}, stdout={}, stderr={}",
        ExitStatus::from_raw(status),
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    let prefix = b"TOS_PG_REAL_PROJECTION_RESUME_RESULT ";
    let work = stdout
        .split(|byte| *byte == b'\n')
        .find_map(|line| line.strip_prefix(prefix))
        .map(|line| serde_json::from_slice(line).expect("child work report is JSON"))
        .expect("fresh projection process reports its logical work");
    let pg_cgroup_after = isolated_pg_cgroup_snapshot();
    let resources = json!({
        "scope":"fresh projection child direct-process wait4 rusage and /proc PID I/O; excludes PostgreSQL server",
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
    });
    (work, resources)
}

fn kill_projection_child(case: &RevisionCase, consumer: &str) -> Value {
    let marker = case._owner_scratch.0.join("projection-process-kill-ready");
    let started = Instant::now();
    let pg_cgroup_before = isolated_pg_cgroup_snapshot();
    let executable = std::env::current_exe().expect("integration test executable is available");
    let mut command = Command::new(executable);
    command.process_group(0);
    let mut child = command
        .arg("pg_real_projection_interrupt_child_entry")
        .arg("--nocapture")
        .env("TOS_CMD_POSTGRES_URL", &case.lab.url)
        .env("TOS_SOURCE_PG_URL", &case.lab.url)
        .env("TOS_PG_REAL_PROJECTION_MARKER", &marker)
        .env("TOS_PG_REAL_PROJECTION_DOMAIN", &case.lab.domain)
        .env("TOS_PG_REAL_PROJECTION_CONSUMER", consumer)
        .env("TOS_PG_REAL_PROJECTION_STORE", &case.lab._root.0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("isolated test process starts the real projection apply child");
    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, 1_048_576).unwrap());
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, 1_048_576).unwrap());
    let deadline = started + Duration::from_secs(30);
    loop {
        if marker.exists() {
            break;
        }
        if let Some(status) = child
            .try_wait()
            .expect("projection child status is readable")
        {
            let (stdout, _) = stdout_reader.join().unwrap();
            let (stderr, _) = stderr_reader.join().unwrap();
            panic!(
                "projection child exited before its extraction barrier: status={status}, stdout={}, stderr={}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
        if Instant::now() >= deadline {
            unsafe {
                libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.kill();
            let status = child.wait().expect("timed-out projection child is reaped");
            let (stdout, _) = stdout_reader.join().unwrap();
            let (stderr, _) = stderr_reader.join().unwrap();
            panic!(
                "projection child missed its 30 second extraction barrier: status={status}, stdout={}, stderr={}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
    let marker_bytes = fs::read(&marker).unwrap();
    assert_eq!(
        marker_bytes,
        b"production source extraction reached during event staging"
    );
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
    let mut status = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let reaped = unsafe { libc::wait4(pid as libc::pid_t, &mut status, 0, &mut usage) };
    assert_eq!(
        reaped, pid as libc::pid_t,
        "killed projection child is reaped"
    );
    let (stdout, stdout_bytes) = stdout_reader.join().unwrap();
    let (stderr, stderr_bytes) = stderr_reader.join().unwrap();
    assert!(stdout_bytes <= 1_048_576 && stderr_bytes <= 1_048_576);
    assert_eq!(
        ExitStatus::from_raw(status).signal(),
        Some(libc::SIGKILL),
        "the projection process ended from the parent's SIGKILL"
    );
    let pg_cgroup_after = isolated_pg_cgroup_snapshot();
    json!({
        "scope":"real test-executable process running production apply_projection_batch with SourceTextExtractor; parent SIGKILL during event staging after first extract",
        "wall_micros":u64::try_from(started.elapsed().as_micros()).unwrap(),
        "wait4_user_cpu_micros":timeval_micros(usage.ru_utime),
        "wait4_system_cpu_micros":timeval_micros(usage.ru_stime),
        "wait4_max_rss_bytes":u64::try_from(usage.ru_maxrss).unwrap().saturating_mul(1024),
        "stdout_bytes":stdout_bytes,
        "stderr_bytes":stderr_bytes,
        "postgres_cgroup":{"before":pg_cgroup_before,"after":pg_cgroup_after}
    })
}

fn timeval_micros(value: libc::timeval) -> u64 {
    u64::try_from(value.tv_sec)
        .unwrap()
        .saturating_mul(1_000_000)
        .saturating_add(u64::try_from(value.tv_usec).unwrap())
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
    invoke_with_expected_exit(
        binary,
        invocation_path,
        url,
        request,
        discard_reply,
        true,
        "tos_local_pg_record_revision_result_v1",
    )
}

fn invoke_creation(
    binary: &Path,
    invocation_path: &Path,
    url: &str,
    request: &Value,
    discard_reply: bool,
) -> NativeInvocation {
    invoke_with_expected_exit(
        binary,
        invocation_path,
        url,
        request,
        discard_reply,
        true,
        "tos_local_pg_agent_creation_result_v1",
    )
}

fn invoke_refusal(
    binary: &Path,
    invocation_path: &Path,
    url: &str,
    request: &Value,
) -> NativeInvocation {
    invoke_with_expected_exit(
        binary,
        invocation_path,
        url,
        request,
        false,
        false,
        "tos_local_pg_record_revision_result_v1",
    )
}

fn invoke_with_expected_exit(
    binary: &Path,
    invocation_path: &Path,
    url: &str,
    request: &Value,
    discard_reply: bool,
    expect_success: bool,
    expected_envelope: &str,
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
    assert!(
        stdin_result.is_ok(),
        "native invocation accepted the bounded request"
    );
    let mut dropped_source_read_work = None;
    let result = if expect_success {
        assert_eq!(
            status,
            0,
            "native invocation refused: {}",
            String::from_utf8_lossy(&stderr)
        );
        if discard_reply {
            let response: Value = serde_json::from_slice(&stdout).unwrap();
            assert_eq!(response["schema_version"], expected_envelope);
            assert_eq!(response["grants_admission"], false);
            dropped_source_read_work = response["result"].get("source_read_work").cloned();
            Value::Null
        } else {
            let response: Value = serde_json::from_slice(&stdout).unwrap();
            assert_eq!(response["schema_version"], expected_envelope);
            assert_eq!(response["grants_admission"], false);
            response["result"].clone()
        }
    } else {
        json!({
            "refused":status != 0,
            "exit_status":status,
            "stdout":String::from_utf8_lossy(&stdout).into_owned(),
            "stderr":String::from_utf8_lossy(&stderr).into_owned()
        })
    };
    let timeval_micros = |value: libc::timeval| {
        u64::try_from(value.tv_sec)
            .unwrap()
            .saturating_mul(1_000_000)
            .saturating_add(u64::try_from(value.tv_usec).unwrap())
    };
    NativeInvocation {
        result,
        dropped_source_read_work,
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

fn projection_dependencies(
    case: &RevisionCase,
    consumer: &str,
) -> BTreeMap<(String, String), BTreeSet<String>> {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    let rows = db
        .query(
            "SELECT source_path,document_id,dependency_path
             FROM cmd2_projection_dependency
             WHERE domain=$1 AND consumer_id=$2 AND dependency_kind='source'
             ORDER BY source_path COLLATE \"C\",document_id COLLATE \"C\",dependency_path COLLATE \"C\"",
            &[&case.lab.domain, &consumer],
        )
        .unwrap();
    let mut dependencies = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for row in rows {
        let key = (row.get::<_, String>(0), row.get::<_, String>(1));
        let path = row.get::<_, String>(2);
        assert!(
            dependencies.entry(key).or_default().insert(path),
            "one source dependency edge exists per document/path pair"
        );
    }
    dependencies
}

fn assert_bounded_committed_member_rows(work: &Value, scope: &str) {
    let exact_members = work["exact_source_members_read"]
        .as_u64()
        .unwrap_or_else(|| panic!("{scope} exact-source member count is reported"));
    assert!(
        work["custody_metadata_bytes_read"].as_u64().is_some(),
        "{scope} custody metadata bytes are reported independently"
    );
    assert!(
        work["segment_bytes_read"].as_u64().is_some(),
        "{scope} selected STO payload bytes are reported independently"
    );
    let examined = work["committed_member_rows_examined"]
        .as_u64()
        .unwrap_or_else(|| panic!("{scope} committed-member row count is reported"));
    assert!(
        examined <= exact_members.saturating_mul(65),
        "{scope} examines at most 65 committed rows per exact source member: {examined} <= 65 * {exact_members}"
    );
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
    let target = case.target;
    let record_path = target.source_path.to_owned();
    let original_authored_record_version = serde_json::from_slice::<Value>(&case.original_record)
        .unwrap()["record_version"]
        .as_u64()
        .expect("authored record version is present");
    invocation(
        case,
        software,
        binary,
        &worker_path,
        &invocation_path,
        &case.owner_config,
        "tos_local_pg_record_revision_invocation_v1",
    );
    let source_before_drift = source_member(case, target.source_path);
    let before_fault_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    let db = Client::connect(&case.lab.url, NoTls).unwrap();
    let selected_row = db
        .query_one(
            "SELECT revision,source_mode,content_digest,source_dependencies
             FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &record_path],
        )
        .expect("selected current source row exists");
    let original_custody_revision = u64::try_from(selected_row.get::<_, i64>("revision"))
        .expect("the selected prior durable revision is positive");
    assert_eq!(
        original_custody_revision, 1,
        "each isolated fixture begins at durable custody revision one, independently of the authored record_version"
    );
    if target.record_id == NIETZSCHE_TARGET.record_id {
        assert_eq!(original_authored_record_version, 3);
        assert_ne!(original_custody_revision, original_authored_record_version);
    }
    let original_source_mode = selected_row.get::<_, i32>("source_mode");
    let original_content_digest = selected_row.get::<_, String>("content_digest");
    let original_dependencies = selected_row.get::<_, Option<Vec<String>>>("source_dependencies");
    drop(db);
    let drifted_source_mode = original_source_mode ^ 1;
    assert!((0..=0o7777).contains(&drifted_source_mode));
    assert_ne!(original_source_mode, drifted_source_mode);
    let db = Client::connect(&case.lab.url, NoTls).unwrap();
    assert_eq!(
        db.execute(
            "UPDATE cmd2_current SET source_mode=$3 WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &record_path, &drifted_source_mode],
        )
        .unwrap(),
        1,
        "the isolated fault changes only the selected current mode"
    );
    drop(db);
    let refused_call = invoke_refusal(
        binary,
        &invocation_path,
        &case.lab.url,
        &json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
    );
    let refusal_text = format!(
        "{}{}",
        refused_call.result["stdout"].as_str().unwrap(),
        refused_call.result["stderr"].as_str().unwrap()
    );
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    let drifted_row = db
        .query_one(
            "SELECT revision,source_mode,content_digest,source_dependencies
             FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &record_path],
        )
        .expect("selected current source remains available after refusal");
    let observed_drifted_source_mode = drifted_row.get::<_, i32>("source_mode");
    let observed_drifted_custody_revision = drifted_row.get::<_, i64>("revision");
    let observed_drifted_content_digest = drifted_row.get::<_, String>("content_digest");
    let observed_drifted_dependencies =
        drifted_row.get::<_, Option<Vec<String>>>("source_dependencies");
    assert_eq!(
        db.execute(
            "UPDATE cmd2_current SET source_mode=$3 WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &record_path, &original_source_mode],
        )
        .unwrap(),
        1,
        "the source mode is restored exactly"
    );
    let restored_row = db
        .query_one(
            "SELECT revision,source_mode,content_digest,source_dependencies
             FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &record_path],
        )
        .expect("selected current source is restored");
    let restored_source_mode = restored_row.get::<_, i32>("source_mode");
    let restored_custody_revision = restored_row.get::<_, i64>("revision");
    assert_eq!(restored_source_mode, original_source_mode);
    assert_eq!(restored_custody_revision, original_custody_revision as i64);
    assert_eq!(
        restored_row.get::<_, String>("content_digest"),
        original_content_digest
    );
    assert_eq!(
        restored_row.get::<_, Option<Vec<String>>>("source_dependencies"),
        original_dependencies
    );
    drop(db);
    assert_eq!(
        refused_call.result["refused"], true,
        "native describe refuses the corrupted current metadata"
    );
    assert!(
        refusal_text.contains("PostgreSQL source revision metadata"),
        "native describe refuses the current/history source-mode mismatch: {refusal_text}"
    );
    assert_eq!(observed_drifted_source_mode, drifted_source_mode);
    assert_eq!(
        observed_drifted_custody_revision,
        original_custody_revision as i64
    );
    assert_eq!(observed_drifted_content_digest, original_content_digest);
    assert_eq!(observed_drifted_dependencies, original_dependencies);
    assert_eq!(source_member(case, target.source_path), source_before_drift);
    assert_eq!(
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table)),
        before_fault_counts,
        "the isolated metadata drift and refusal add no durable command records"
    );
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
        target.record_id
    );
    let mut fields = json!({"preferred_label":target.preferred_label});
    if let Some(notes) = target.notes {
        fields["notes"] = json!(notes);
    }
    let preview_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-revise",
        "fields":fields,
        "forms":target.form_ids.iter().zip(target.form_fields).map(|(form_id,field_id)| json!({"form_id":form_id,"field_id":field_id})).collect::<Vec<_>>(),
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
    request["command_id"] = json!(target.command_id);
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
    let current_record = source_member(case, target.source_path);
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
    assert_eq!(source_member(case, target.source_path), current_record);
    let after: Value = serde_json::from_slice(&current_record).unwrap();
    let mut expected: Value = serde_json::from_slice(&case.original_record).unwrap();
    expected["preferred_label"] = json!(target.preferred_label);
    if let Some(notes) = target.notes {
        expected["notes"] = json!(notes);
    }
    expected["record_version"] = json!(target.resulting_version);
    assert_eq!(
        after, expected,
        "the PG current source matches the prepared correction"
    );
    assert_ne!(before_counts, committed_counts);
    let paths = history_source_paths(case, committed_head);
    for required in [target.source_path, target.forms_path, target.history_path] {
        assert!(
            paths.iter().any(|path| path == required),
            "revision records {required}"
        );
    }
    let forms: Value = serde_json::from_slice(&source_member(case, target.forms_path)).unwrap();
    assert_eq!(forms["forms"].as_array().unwrap().len(), 3);
    assert!(
        forms["forms"]
            .as_array()
            .unwrap()
            .iter()
            .all(|form| form["form_version"] == target.resulting_form_version)
    );
    let retained = case
        .lab
        .db
        .cold_recover_exact(
            &case.lab.store,
            &case.lab.domain,
            target.source_path,
            original_custody_revision,
        )
        .expect("the original record locator remains retained");
    assert_eq!(
        case.lab
            .db
            .warm_read_selected(&case.lab.store, &retained, 1_048_576)
            .unwrap(),
        case.original_record,
        "the previous source bytes are exactly recoverable"
    );
    let mut updated = BTreeSet::from([
        target.source_path.to_owned(),
        target.forms_path.to_owned(),
        target.history_path.to_owned(),
    ]);
    let archive_path = {
        let history: Value =
            serde_json::from_slice(&source_member(case, target.history_path)).unwrap();
        history["receipts"].as_array().unwrap().last().unwrap()["archive_path"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert!(
        paths.iter().all(|path| {
            path == target.source_path
                || path == target.forms_path
                || path == target.history_path
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
        "record_id":target.record_id,
        "source_path":target.source_path,
        "previous_custody_revision":original_custody_revision,
        "previous_authored_record_version":original_authored_record_version,
        "before_head":before_head,
        "committed_head":committed_head,
        "before_counts":before_counts,
        "committed_counts":committed_counts,
        "changed_source_paths":paths,
        "native_invocations":{
            "describe":describe_resources,
            "source_mode_drift_refusal":{
                "source_mode_before":original_source_mode,
                "source_mode_drifted":drifted_source_mode,
                "source_mode_restored":restored_source_mode,
                "native_describe_refused":true,
                "error":refused_call.result,
                "resources":refused_call.resources
            },
            "prepare_revise":prepare_resources,
            "record_revise_reply_dropped":commit_resources,
            "same_command_retry":retry_resources
        },
        "source_bytes":case.all_source.values().map(|(raw,_)|raw.len() as u64).sum::<u64>(),
        "current_record_bytes":current_record.len(),
        "current_record_sha256":Digest256::of_bytes(&current_record).to_hex(),
        "retained_original_bytes":case.original_record.len(),
        "retained_original_sha256":Digest256::of_bytes(&case.original_record).to_hex(),
        "source_read_work":retry["source_read_work"].clone(),
        "source_read_work_scope":"logical source-store work; segment_bytes_read counts selected STO frame and envelope bytes read, while physical device I/O is separate; software capture/context is excluded",
        "replayed":retry["replayed"],
        "archive_manifest_path":archive_manifest_path
    })
}

fn create_agent(case: &mut RevisionCase, software: &SoftwareFixture, binary: &Path) -> Value {
    let worker_path = PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").unwrap());
    let invocation_path = case.source_root.join("creation-invocation.json");
    let owner_config = case._owner_scratch.0.join("creation-owner.json");
    let uid = rustix::process::getuid().as_raw();
    let owner = json!({
        "schema_version":"tos_local_corpus_create_owner_v2",
        "uid":uid,
        "principal_id":"software:test-private-pg-real-addition",
        "maker_type":"software",
        "source_root":case.source_root,
        "source_path":CREATED_RECORD_PATH,
        "record_type":"agent",
        "record_id":CREATED_RECORD_ID,
        "authority_ref":"test:private-pg-real-addition:no-admission",
        "allowed_form_ids":[CREATED_FORM_ID],
        "allowed_operations":["source.create"],
        "expires_at":"2099-01-01T00:00:00Z",
        "provenance_event_id":"tos.event.pg-real-addition-fixture"
    });
    fs::write(&owner_config, canonical(&owner)).unwrap();
    fs::set_permissions(&owner_config, fs::Permissions::from_mode(0o600)).unwrap();
    invocation(
        case,
        software,
        binary,
        &worker_path,
        &invocation_path,
        &owner_config,
        "tos_local_pg_agent_creation_invocation_v1",
    );

    let record = json!({
        "schema_version":"tos_corpus_record_v1",
        "record_type":"agent",
        "record_id":CREATED_RECORD_ID,
        "record_version":1,
        "preferred_label":"Sundial PostgreSQL addition fixture",
        "variant_labels":[],
        "identity_status":"provisional",
        "source_refs":["test:private-pg-real-addition:no-admission"],
        "external_identifiers":[],
        "same_as_posture":"no_equivalence_claim",
        "notes":"Private PostgreSQL mechanics fixture; no semantic, source, rights, or canon admission.",
        "supersedes_ref":null,
        "field_languages":{
            "preferred_label":{"language":"en","script":"Latn"},
            "notes":{"language":"en","script":"Latn"}
        }
    });
    let prepare_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-create",
        "record":record,
        "forms":[{"form_id":CREATED_FORM_ID,"field_id":"metadata.preferred-name"}]
    });
    let prepare_call = invoke_creation(
        binary,
        &invocation_path,
        &case.lab.url,
        &prepare_request,
        false,
    );
    let preview = prepare_call.result;
    assert_eq!(preview["record_id"], CREATED_RECORD_ID);
    let mut request = prepare_request.clone();
    request["operation"] = json!("source.create");
    request["command_id"] = json!("test:pg-real-addition-fixture");
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    let before_head = case.lab.head_seq();
    let before_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    let commit_call = invoke_creation(binary, &invocation_path, &case.lab.url, &request, true);
    let commit_resources = commit_call.resources;
    let initial_source_read_work = commit_call
        .dropped_source_read_work
        .expect("the dropped initial creation reply contains bounded source-read counters");
    let committed_head = case.lab.head_seq();
    assert_eq!(
        committed_head,
        before_head + 1,
        "one Agent addition is one durable event"
    );
    let committed_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    assert_eq!(committed_counts[4], before_counts[4] + 1);

    let retry_call = invoke_creation(binary, &invocation_path, &case.lab.url, &request, false);
    let retry = retry_call.result;
    assert_eq!(retry["operation"], "source.create");
    assert_eq!(retry["record_id"], CREATED_RECORD_ID);
    assert_eq!(retry["source_path"], CREATED_RECORD_PATH);
    assert_eq!(
        retry["replayed"], true,
        "the exact command recovers its lost reply"
    );
    assert_eq!(retry["durable_commit"]["replayed"], true);
    assert_eq!(retry["durable_commit"]["commit_seq"], committed_head);
    assert_eq!(case.lab.head_seq(), committed_head);
    assert_eq!(
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table)),
        committed_counts,
        "exact retry leaves every durable counter unchanged"
    );
    let expected_paths = BTreeSet::from([
        CREATED_RECORD_PATH.to_owned(),
        "ToS/source-witnesses/agents/pg-real-addition-fixture/agent.human-forms.json".to_owned(),
        "ToS/source-witnesses/agents/pg-real-addition-fixture/source-create-provenance.jsonl"
            .to_owned(),
    ]);
    let actual_paths = retry["created_source_paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path.as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_paths, expected_paths);
    for path in &actual_paths {
        let raw = source_member(case, path);
        assert!(
            !raw.is_empty(),
            "created source member is exactly readable: {path}"
        );
    }
    let created_record = source_member(case, CREATED_RECORD_PATH);
    assert_eq!(created_record, canonical(&record));
    assert_eq!(
        retry["source_read_work"]["exact_source_members_read"], 0,
        "durable replay does not repeat ordinary source-selection reads"
    );
    let reopened_store =
        tos_segment_store::SegmentStore::open_existing(&case.source_root, limits())
            .expect("the isolated STO root reopens after native initial Agent creation");
    let mut reopened_db = DurablePgCoordinator::connect(&case.lab.url).unwrap();
    let _cold_cut = reopened_db
        .cold_verify_cut(&reopened_store, &case.lab.domain)
        .expect("the created PG/STO generation passes the existing cold cut verifier");
    let cold_record = reopened_db
        .cold_recover_exact(&reopened_store, &case.lab.domain, CREATED_RECORD_PATH, 1)
        .expect("the new Agent is discoverable by its cold exact historical locator");
    assert_eq!(
        reopened_db
            .warm_read_selected(&reopened_store, &cold_record, 1_048_576)
            .unwrap(),
        created_record,
        "cold reopen returns the exact created Agent bytes"
    );
    json!({
        "record_id":CREATED_RECORD_ID,
        "source_path":CREATED_RECORD_PATH,
        "committed_head":committed_head,
        "created_source_paths":actual_paths,
        "record_sha256":Digest256::of_bytes(&created_record).to_hex(),
        "cold_cut_verified":true,
        "initial_source_read_work":initial_source_read_work,
        "retry_source_read_work":retry["source_read_work"].clone(),
        "native_invocations":{
            "prepare_create":prepare_call.resources,
            "source_create_reply_dropped":commit_resources,
            "same_command_retry":retry_call.resources
        }
    })
}

fn project_case(
    case: &mut RevisionCase,
    software: &SoftwareFixture,
    records: usize,
    binary: &Path,
) -> Value {
    let consumer = format!("private-pg-real-revision-{records}");
    let creation = create_agent(case, software, binary);
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
    assert!(
        before_rows
            .iter()
            .any(|row| row.contains(CREATED_RECORD_ID)),
        "the initial Agent addition is published in the selected-text projection"
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
    let created_record = source_member(case, CREATED_RECORD_PATH);
    assert_eq!(
        Digest256::of_bytes(&created_record).to_hex(),
        creation["record_sha256"],
        "the added Agent remains byte-exact after the neighboring revision"
    );
    let revision_head = case.lab.head_seq();
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, &consumer, &profile, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == initial_head && head_seq == revision_head
    ));
    let interrupted_process_resources = kill_projection_child(case, &consumer);
    assert_eq!(
        projection_rows(case, &consumer),
        before_rows,
        "killing a real projector during extraction leaves the committed projection rows unchanged"
    );
    // The killed process left no in-memory resume state. Reopen through a new
    // coordinator and prove the durable cursor still points before the event.
    case.lab.db = DurablePgCoordinator::connect(&case.lab.url).unwrap();
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, &consumer, &profile, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == initial_head && head_seq == revision_head
    ));
    let (revision_work, resume_process_resources) =
        resume_projection_in_fresh_process(case, &consumer);
    let revision_apply_resources = json!({
        "scope":"fresh-process single revision apply_projection_batch; direct-child wait4 rusage and /proc PID I/O exclude PostgreSQL server",
        "resume_process":resume_process_resources,
        "interrupted_process":interrupted_process_resources
    });
    case.lab.db = DurablePgCoordinator::connect(&case.lab.url).unwrap();
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, &consumer, &profile, Duration::from_secs(15))
            .unwrap(),
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
    assert_eq!(
        after_documents[&(CREATED_RECORD_PATH.to_owned(), CREATED_RECORD_ID.to_owned())],
        before_documents[&(CREATED_RECORD_PATH.to_owned(), CREATED_RECORD_ID.to_owned())],
        "the newly added Agent stays unchanged during the neighboring correction"
    );
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
    let creation_search = case
        .lab
        .db
        .search_projection(
            &case.lab.domain,
            &consumer,
            &profile,
            "Sundial",
            10,
            Duration::from_secs(15),
        )
        .unwrap();
    assert!(
        matches!(
            creation_search,
            DurableProjectionSearch::Current { ref hits, through_seq }
                if through_seq == case.lab.head_seq()
                    && hits.iter().any(|hit| hit.document_id == CREATED_RECORD_ID)
        ),
        "the created Agent is searchable after projection publication"
    );
    json!({
        "consumer":consumer,
        "initial_documents":before_rows.len(),
        "final_documents":after_rows.len(),
        "projection_cursor":case.lab.head_seq(),
        "new_text_search":"Quartz",
        "projection_work":{
            "scope":"logical projection work; segment_bytes_read counts selected STO frame and envelope bytes read, while physical device I/O is separate; publication counters cover work applied by this consumer",
            "bootstrap":initial_work,
            "revision":revision_work
        },
        "revision_apply_resources":revision_apply_resources,
        "initial_agent_creation":creation,
        "revision":receipt
    })
}

fn project_nietzsche_shared_case(
    case: &mut RevisionCase,
    software: &SoftwareFixture,
    records: usize,
    binary: &Path,
) -> Value {
    let consumer = format!("private-pg-real-shared-source-{records}");
    let profile = DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE_V2.to_owned(),
        version: 2,
        source_contract: contract_digest(),
    };
    case.lab.db.init_projection_schema().unwrap();
    let extractor = tos_command::source_projection_cli::SourceTextExtractorV2;
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
                max_dependencies: 2,
                ..DurableProjectionLimits::default()
            },
        )
        .expect("the v2 consumer selects paired source bytes at each committed event cut");
    let initial_work = serde_json::to_value(&initial.work).unwrap();
    assert!(matches!(
        initial.status,
        DurableProjectionStatus::Current { through_seq } if through_seq == initial_head
    ));
    let before_rows = projection_rows(case, &consumer);
    let before_documents = projection_documents(&before_rows);
    let metadata_key = (
        NIETZSCHE_RECORD_PATH.to_owned(),
        NIETZSCHE_TARGET.record_id.to_owned(),
    );
    assert!(before_documents.contains_key(&metadata_key));
    let form_keys = NIETZSCHE_FORM_IDS
        .iter()
        .map(|form_id| (NIETZSCHE_FORMS_PATH.to_owned(), (*form_id).to_owned()))
        .collect::<BTreeSet<_>>();
    assert!(
        form_keys
            .iter()
            .all(|key| before_documents.contains_key(key))
    );
    let expected_form_dependencies = BTreeSet::from([
        NIETZSCHE_FORMS_PATH.to_owned(),
        NIETZSCHE_RECORD_PATH.to_owned(),
    ]);
    let before_dependencies = projection_dependencies(case, &consumer);
    for key in &form_keys {
        assert_eq!(
            before_dependencies.get(key),
            Some(&expected_form_dependencies),
            "each current source-copy form is attributed to its HumanForms and exact Agent metadata"
        );
    }
    assert_eq!(
        before_dependencies.get(&metadata_key),
        Some(&BTreeSet::from([NIETZSCHE_RECORD_PATH.to_owned()])),
        "the record text document keeps its exact one-source dependency"
    );

    let receipt = record_correction(case, software, binary);
    let revision_head = case.lab.head_seq();
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, &consumer, &profile, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == initial_head && head_seq == revision_head
    ));
    let applied = case
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
                max_dependencies: 2,
                ..DurableProjectionLimits::default()
            },
        )
        .expect("the joined projection applies the native shared-source revision");
    let revision_work = serde_json::to_value(&applied.work).unwrap();
    assert_eq!(revision_work["extracted_documents"], 4);
    assert_eq!(revision_work["published_documents"], 4);
    assert!(matches!(
        applied.status,
        DurableProjectionStatus::Current { through_seq } if through_seq == revision_head
    ));
    let after_rows = projection_rows(case, &consumer);
    let after_documents = projection_documents(&after_rows);
    assert_eq!(before_documents.len(), after_documents.len());
    assert_eq!(
        before_documents.keys().collect::<Vec<_>>(),
        after_documents.keys().collect::<Vec<_>>(),
        "the correction replaces existing source documents without changing cardinality"
    );
    let changed_documents = before_documents
        .iter()
        .filter(|(key, before)| after_documents.get(*key) != Some(*before))
        .map(|(key, _)| (*key).clone())
        .collect::<BTreeSet<_>>();
    let expected_changed =
        BTreeSet::from_iter(std::iter::once(metadata_key.clone()).chain(form_keys.iter().cloned()));
    assert_eq!(
        changed_documents, expected_changed,
        "one metadata revision rebinds exactly its record and three source-copy form documents"
    );
    let after_dependencies = projection_dependencies(case, &consumer);
    for (form_id, role) in NIETZSCHE_FORM_IDS.iter().zip(["name", "hover", "name"]) {
        let key = (NIETZSCHE_FORMS_PATH.to_owned(), (*form_id).to_owned());
        assert_eq!(
            after_dependencies.get(&key),
            Some(&expected_form_dependencies),
            "every published source-copy form retains its exact shared metadata dependency"
        );
        let document = &after_documents[&key];
        assert_eq!(document["identity"].as_str(), Some(*form_id));
        assert_eq!(
            document["document_type"].as_str(),
            Some(if role == "hover" {
                "human_form/hover"
            } else {
                "human_form/name"
            })
        );
    }
    assert_eq!(
        after_dependencies.get(&metadata_key),
        Some(&BTreeSet::from([NIETZSCHE_RECORD_PATH.to_owned()])),
        "the revised main-source document remains directly attributed"
    );
    let unaffected = |docs: &BTreeMap<(String, String), Value>| {
        docs.iter()
            .filter(|(key, _)| !expected_changed.contains(*key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(
        unaffected(&before_documents),
        unaffected(&after_documents),
        "no unrelated source document is rewritten"
    );
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
    let search_ids = match search {
        DurableProjectionSearch::Current { hits, through_seq } => {
            assert_eq!(through_seq, revision_head);
            hits.into_iter()
                .map(|hit| hit.document_id)
                .collect::<BTreeSet<_>>()
        }
        state => panic!("revised source-copy projection is searchable: {state:?}"),
    };
    assert!(search_ids.contains(NIETZSCHE_TARGET.record_id));
    assert!(search_ids.contains(NIETZSCHE_FORM_IDS[0]));
    json!({
        "consumer":consumer,
        "profile":profile.id,
        "initial_documents":before_rows.len(),
        "final_documents":after_rows.len(),
        "projection_cursor":revision_head,
        "changed_documents":changed_documents,
        "source_copy_form_ids":NIETZSCHE_FORM_IDS,
        "source_copy_dependencies":expected_form_dependencies,
        "search_query":"Quartz",
        "search_hits":search_ids,
        "projection_work":{"bootstrap":initial_work,"revision":revision_work},
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
    for (records, projection) in [(1, &one_projection), (3, &three_projection)] {
        assert_bounded_committed_member_rows(
            &projection["projection_work"]["bootstrap"],
            &format!("{records}-record bootstrap projection"),
        );
        assert_bounded_committed_member_rows(
            &projection["projection_work"]["revision"],
            &format!("{records}-record revision projection"),
        );
        assert_bounded_committed_member_rows(
            &projection["revision"]["source_read_work"],
            &format!("{records}-record native source revision"),
        );
        assert_bounded_committed_member_rows(
            &projection["initial_agent_creation"]["initial_source_read_work"],
            &format!("{records}-record native initial Agent creation"),
        );
    }
    for counter in [
        "examined_event_members",
        "exact_source_members_read",
        "exact_source_bytes_read",
        "segment_bytes_read",
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
    let one_creation_work = &one_projection["initial_agent_creation"]["initial_source_read_work"];
    let three_creation_work =
        &three_projection["initial_agent_creation"]["initial_source_read_work"];
    for counter in [
        "exact_source_members_read",
        "exact_source_bytes_read",
        "segment_bytes_read",
        "committed_member_rows_examined",
        "metadata_member_lookups",
        "directory_index_rows_examined",
        "unique_index_lookups",
    ] {
        assert!(one_creation_work[counter].as_u64().is_some());
        assert_eq!(
            one_creation_work[counter], three_creation_work[counter],
            "the same ordinary Agent addition has the same {counter} source-selection work at 1 and 3 records"
        );
    }
    let one_source_read_work = &one_projection["revision"]["source_read_work"];
    let three_source_read_work = &three_projection["revision"]["source_read_work"];
    assert!(one_source_read_work.is_object());
    assert!(three_source_read_work.is_object());
    for counter in [
        "exact_source_members_read",
        "exact_source_bytes_read",
        "segment_bytes_read",
        "metadata_member_lookups",
        "directory_index_rows_examined",
        "unique_index_lookups",
    ] {
        assert!(
            one_source_read_work[counter].as_u64().is_some(),
            "native source-read work counter {counter} is numeric"
        );
        assert_eq!(
            one_source_read_work[counter], three_source_read_work[counter],
            "the one-record correction has the same {counter} logical source-read work at 1 and 3 records"
        );
    }
    let output = json!({
        "schema_version":"tos_cmd2_real_source_revision_test_v1",
        "source_record":"tos.agent.parmenides-of-elea",
        "initial_added_record":CREATED_RECORD_ID,
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

#[test]
fn real_nietzsche_shared_revision_reprojects_all_three_forms_at_1_and_3_records() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let mut one = bootstrap_shared_case(&repository, &software, 1, deadline, &cancelled);
    let mut three = bootstrap_shared_case(&repository, &software, 3, deadline, &cancelled);
    let binary = native_binary();
    let one_projection = project_nietzsche_shared_case(&mut one, &software, 1, &binary);
    let three_projection = project_nietzsche_shared_case(&mut three, &software, 3, &binary);
    for (records, projection) in [(1, &one_projection), (3, &three_projection)] {
        assert_bounded_committed_member_rows(
            &projection["projection_work"]["bootstrap"],
            &format!("{records}-record v2 bootstrap projection"),
        );
        assert_bounded_committed_member_rows(
            &projection["projection_work"]["revision"],
            &format!("{records}-record joined revision projection"),
        );
        assert_bounded_committed_member_rows(
            &projection["revision"]["source_read_work"],
            &format!("{records}-record native Nietzsche revision"),
        );
    }
    for counter in [
        "examined_event_members",
        "exact_source_members_read",
        "exact_source_bytes_read",
        "segment_bytes_read",
        "examined_related_roots",
        "examined_dependency_paths",
        "extracted_documents",
        "published_source_paths",
        "published_documents",
    ] {
        assert_eq!(
            one_projection["projection_work"]["revision"][counter],
            three_projection["projection_work"]["revision"][counter],
            "the same Nietzsche shared-source revision has the same {counter} projection work at 1 and 3 records"
        );
    }
    let one_source_read_work = &one_projection["revision"]["source_read_work"];
    let three_source_read_work = &three_projection["revision"]["source_read_work"];
    for counter in [
        "exact_source_members_read",
        "exact_source_bytes_read",
        "segment_bytes_read",
        "metadata_member_lookups",
        "directory_index_rows_examined",
        "unique_index_lookups",
    ] {
        assert_eq!(
            one_source_read_work[counter], three_source_read_work[counter],
            "the same native Nietzsche correction has the same {counter} source-selection work at 1 and 3 records"
        );
    }
    let output = json!({
        "schema_version":"tos_cmd2_real_shared_source_projection_test_v1",
        "source_record":NIETZSCHE_TARGET.record_id,
        "independent_growth_records":["tos.agent.parmenides-of-elea","tos.agent.justus-lipsius"],
        "form_ids":NIETZSCHE_FORM_IDS,
        "bootstrap_cases":[
            {"record_count":1,"source":source_counters(&one.all_source),"projection":one_projection},
            {"record_count":3,"source":source_counters(&three.all_source),"projection":three_projection}
        ],
        "authored_repository_mutated":false,
        "semantic_admission":false
    });
    println!(
        "TOS_PG_REAL_SHARED_PROJECTION {}",
        serde_json::to_string(&output).unwrap()
    );
}
