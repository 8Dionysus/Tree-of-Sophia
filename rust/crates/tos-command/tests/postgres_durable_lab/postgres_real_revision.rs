//! Bounded real-source revision exercise over private PostgreSQL and STO.
//! The selected authored bytes are copied into a private fixture before any
//! capture or mutation; the repository remains read-only.

use super::{
    Lab, ScratchRoot, contract_digest, database_url, limits, source_inventory_projection_bytes,
    source_inventory_projection_root, source_inventory_projection_rows,
};
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
    DurablePgCoordinator, DurableProjectionLimits, DurableProjectionProfile,
    DurableProjectionSearch, DurableProjectionStatus,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, RelativePath,
    SourceRevision, canonical_bytes_v1, emit_json_profile, parse_json,
};
use tos_query::source_text_search::{
    SourceTextCandidateSet, SourceTextDocument, SourceTextSearchBudget, SourceTextSearchRequest,
    SourceTextSearchSelection, execute_source_text_search,
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
static REVISION_SCHEMA_PROVISION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
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
    owner_schema: &'static str,
    source_path: &'static str,
    forms_path: &'static str,
    history_path: &'static str,
    record_id: &'static str,
    record_type: &'static str,
    command_id: &'static str,
    form_ids: &'static [&'static str],
    form_fields: &'static [&'static str],
    allowed_fields: &'static [&'static str],
    record_id_field: &'static str,
    record_schema_version: Option<&'static str>,
    preferred_label: Option<&'static str>,
    notes: Option<&'static str>,
    native_field_patch: Option<(&'static str, &'static str)>,
    resulting_version: u64,
    resulting_form_version: u64,
}

const PARMENIDES_TARGET: RevisionTarget = RevisionTarget {
    owner_schema: "tos_local_corpus_revision_owner_v1",
    source_path: RECORD_PATH,
    forms_path: FORMS_PATH,
    history_path: HISTORY_PATH,
    record_id: "tos.agent.parmenides-of-elea",
    record_type: "agent",
    command_id: "test:pg-parmenides-correction",
    form_ids: FORM_IDS,
    form_fields: FORM_FIELDS,
    allowed_fields: &["preferred_label", "notes"],
    record_id_field: "record_id",
    record_schema_version: None,
    preferred_label: Some("Парменид Элейский Quartz"),
    notes: Some(
        "Частная интеграционная фикстура PostgreSQL; семантическое признание не выполнялось.",
    ),
    native_field_patch: None,
    resulting_version: 2,
    resulting_form_version: 2,
};

const NIETZSCHE_TARGET: RevisionTarget = RevisionTarget {
    owner_schema: "tos_local_corpus_revision_owner_v1",
    source_path: NIETZSCHE_RECORD_PATH,
    forms_path: NIETZSCHE_FORMS_PATH,
    history_path: NIETZSCHE_HISTORY_PATH,
    record_id: "tos.agent.friedrich-nietzsche",
    record_type: "agent",
    command_id: "test:pg-nietzsche-shared-source-correction",
    form_ids: NIETZSCHE_FORM_IDS,
    form_fields: NIETZSCHE_FORM_FIELDS,
    allowed_fields: &["preferred_label"],
    record_id_field: "record_id",
    record_schema_version: None,
    preferred_label: Some("Friedrich Nietzsche Quartz"),
    notes: None,
    native_field_patch: None,
    resulting_version: 4,
    resulting_form_version: 3,
};

const WORK_SOURCE_PATH: &str = "ToS/source-witnesses/works/justus-lipsius/de-constantia/work.json";
const WORK_FORMS_PATH: &str =
    "ToS/source-witnesses/works/justus-lipsius/de-constantia/work.human-forms.json";
const WORK_HISTORY_PATH: &str =
    "ToS/source-witnesses/works/justus-lipsius/de-constantia/source-revision-history.json";
const WORK_FORM_IDS: &[&str] = &[
    "tos.form.work.justus-lipsius.de-constantia.name-ru",
    "tos.form.work.justus-lipsius.de-constantia.name-alt",
    "tos.form.work.justus-lipsius.de-constantia.note-ru",
];
const WORK_FORM_FIELDS: &[&str] = &[
    "metadata.preferred-name",
    "metadata.variant-name:0",
    "metadata.source-note",
];
const WORK_TARGET: RevisionTarget = RevisionTarget {
    owner_schema: "tos_local_corpus_revision_owner_v3",
    source_path: WORK_SOURCE_PATH,
    forms_path: WORK_FORMS_PATH,
    history_path: WORK_HISTORY_PATH,
    record_id: "tos.work.justus-lipsius.de-constantia",
    record_type: "work",
    command_id: "test:pg-work-metadata-revision",
    form_ids: WORK_FORM_IDS,
    form_fields: WORK_FORM_FIELDS,
    allowed_fields: &["preferred_label"],
    record_id_field: "record_id",
    record_schema_version: None,
    preferred_label: Some("О постоянстве (De constantia) Quartz"),
    notes: None,
    native_field_patch: None,
    resulting_version: 3,
    resulting_form_version: 3,
};

const ARTIFACT_SOURCE_PATH: &str =
    "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.json";
const ARTIFACT_FORMS_PATH: &str =
    "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.human-forms.json";
const ARTIFACT_HISTORY_PATH: &str =
    "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/source-revision-history.json";
const ARTIFACT_FORM_IDS: &[&str] = &[
    "tos.form.oim-a00645-name",
    "tos.form.oim-a00645-source-note",
];
const ARTIFACT_FORM_FIELDS: &[&str] = &["metadata.preferred-name", "metadata.source-note"];
const ARTIFACT_PHYSICAL_DESCRIPTION: &str = r#"{"height_mm":null,"material":"Reported clay-tablet component; private PG fixture correction.","source_ref":"https://cdli.earth/artifacts/222243","thickness_mm":null,"width_mm":null}"#;
const ARTIFACT_TARGET: RevisionTarget = RevisionTarget {
    owner_schema: "tos_local_native_metadata_revision_owner_v1",
    source_path: ARTIFACT_SOURCE_PATH,
    forms_path: ARTIFACT_FORMS_PATH,
    history_path: ARTIFACT_HISTORY_PATH,
    record_id: "tos.artifact.sumerian.adab.oim-a00645",
    record_type: "artifact",
    command_id: "test:pg-native-artifact-metadata-revision",
    form_ids: ARTIFACT_FORM_IDS,
    form_fields: ARTIFACT_FORM_FIELDS,
    allowed_fields: &["physical_description"],
    record_id_field: "artifact_id",
    record_schema_version: Some("tos_artifact_source_witness_v2"),
    preferred_label: None,
    notes: None,
    native_field_patch: Some(("physical_description", ARTIFACT_PHYSICAL_DESCRIPTION)),
    resulting_version: 2,
    resulting_form_version: 2,
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

fn published_source_bytes(value: &Value) -> Vec<u8> {
    let normalized = canonical(value);
    let parsed = parse_json(
        &normalized,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    emit_json_profile(
        parsed.root(),
        JsonEmissionProfile::SourceFormSetPublishedV1,
        JsonLimits::default(),
    )
    .unwrap()
    .bytes
}

fn owner_configuration_digest(path: &Path) -> String {
    let raw = fs::read(path).unwrap();
    let owner = parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let canonical = canonical_bytes_v1(
        owner.root(),
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits::default(),
    )
    .unwrap();
    Digest256::of_bytes(&canonical).to_prefixed()
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
    let retained_history = super::retained_history_capture::retained_history(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap(),
    );
    let retained_root = retained_history.path().join("source");
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
                copy_repository_member(&retained_root, private_root, &mut members, &manifest_path);
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
                        &retained_root,
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

fn selected_work_source_members(repository: &Path, private_root: &Path) -> SourceMembers {
    fs::create_dir_all(private_root).unwrap();
    fs::set_permissions(private_root, fs::Permissions::from_mode(0o700)).unwrap();
    let mut members = SourceMembers::new();
    let retained_history = super::retained_history_capture::retained_history(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap(),
    );
    let retained_root = retained_history.path().join("source");
    let work_directory = Path::new(WORK_SOURCE_PATH).parent().unwrap();
    let source_directory = repository.join(work_directory);
    let mut entries = fs::read_dir(&source_directory)
        .expect("selected actual Work package opens")
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        assert!(entry.file_type().unwrap().is_file());
        let relative = format!(
            "{}/{}",
            work_directory.display(),
            entry.file_name().to_str().unwrap()
        );
        copy_repository_member(repository, private_root, &mut members, &relative);
    }

    let record: Value = serde_json::from_slice(&members[WORK_SOURCE_PATH].0).unwrap();
    for reference in record["source_refs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
    {
        if reference.starts_with("ToS/") {
            let relative = RelativePath::parse(reference).unwrap();
            copy_repository_member(repository, private_root, &mut members, relative.as_str());
        }
    }

    // The selected revision verifier reconstructs the retained prefix of the
    // Work history. Copy exactly the referenced manifests and their blobs.
    let history: Value = serde_json::from_slice(&members[WORK_HISTORY_PATH].0).unwrap();
    for receipt in history["receipts"].as_array().unwrap() {
        let archive = RelativePath::parse(receipt["archive_path"].as_str().unwrap()).unwrap();
        let manifest_path = format!("{}/manifest.json", archive.as_str());
        copy_repository_member(&retained_root, private_root, &mut members, &manifest_path);
        let manifest: Value = serde_json::from_slice(&members[&manifest_path].0).unwrap();
        for binding in manifest["files"].as_object().unwrap().values() {
            let blob = binding["blob"].as_str().unwrap();
            let blob_path = RelativePath::parse(&format!("{}/{}", archive.as_str(), blob)).unwrap();
            copy_repository_member(
                &retained_root,
                private_root,
                &mut members,
                blob_path.as_str(),
            );
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

fn selected_native_source_members(
    repository: &Path,
    private_root: &Path,
    target: RevisionTarget,
) -> SourceMembers {
    fs::create_dir_all(private_root).unwrap();
    fs::set_permissions(private_root, fs::Permissions::from_mode(0o700)).unwrap();
    let mut members = SourceMembers::new();
    for path in [target.source_path, target.forms_path] {
        copy_repository_member(repository, private_root, &mut members, path);
    }
    if repository.join(target.history_path).exists() {
        copy_repository_member(repository, private_root, &mut members, target.history_path);
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

fn native_source_cut(
    repository: &Path,
    target: RevisionTarget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCutFixture {
    let scratch = ScratchRoot::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let copy_root = scratch.0.join("source-copy");
    let members = selected_native_source_members(repository, &copy_root, target);
    source_cut_from_members(scratch, copy_root, members, deadline, cancelled)
}

fn work_source_cut(
    repository: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCutFixture {
    let scratch = ScratchRoot::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let copy_root = scratch.0.join("source-copy");
    let members = selected_work_source_members(repository, &copy_root);
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
    let expectation = cut.stream(revision).unwrap().expectation();
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
    cohort: tos_command::source_cohort::ManagedSourceCohort,
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

fn bootstrap_work_case(
    repository: &Path,
    software_fixture: &SoftwareFixture,
    deadline: Instant,
    cancelled: &std::sync::Arc<AtomicBool>,
) -> RevisionCase {
    bootstrap_case_from_cut(
        software_fixture,
        work_source_cut(repository, deadline, cancelled),
        WORK_TARGET,
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
    bootstrap_case_on_lab(
        Lab::new(&database_url()),
        software_fixture,
        source,
        target,
        deadline,
        cancelled,
    )
}

fn bootstrap_case_on_lab(
    mut lab: Lab,
    software_fixture: &SoftwareFixture,
    source: SourceCutFixture,
    target: RevisionTarget,
    deadline: Instant,
    cancelled: &std::sync::Arc<AtomicBool>,
) -> RevisionCase {
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
    let mut allowed_operations = vec!["record.revise"];
    if matches!(
        target.owner_schema,
        "tos_local_corpus_revision_owner_v2"
            | "tos_local_corpus_revision_owner_v3"
            | "tos_local_native_metadata_revision_owner_v1"
    ) {
        allowed_operations.push("record.revision-schema.provision");
    }
    if target.record_type == "work" {
        allowed_operations.push("work.revision-schema.provision");
    }
    let mut config = json!({
        "schema_version":target.owner_schema,
        "uid":uid,
        "principal_id":"test:private-pg-source-revision",
        "source_root":source_root,
        "source_path":target.source_path,
        "record_id":record[target.record_id_field],
        "authority_ref":"test:bounded-pg-source-revision",
        "allowed_form_ids":target.form_ids,
        "allowed_fields":target.allowed_fields,
        "allowed_operations":allowed_operations,
        "expires_at":"2099-01-01T00:00:00Z",
        "record_type":target.record_type
    });
    if let Some(schema_version) = target.record_schema_version {
        config["record_schema_version"] = json!(schema_version);
    }
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
    let bootstrap_owner_check = || -> tos_command::DurableResult<()> { Ok(()) };
    let initial = lab
        .db
        .bootstrap_source_cohort(
            &lab.store,
            &lab.domain,
            &source.cut,
            source.revision,
            source.cut.stream(source.revision).unwrap().expectation(),
            &context,
            &software,
            &components,
            &mut worker,
            contract_digest(),
            "private-job",
            1,
            tos_command::source_cohort::BootstrapJobMode::RequireExisting,
            &bootstrap_owner_check,
            deadline,
            cancelled,
        )
        .expect("the exact copied source cut bootstraps through the PG/STO owner");
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

fn local_rights_invocation(
    case: &RevisionCase,
    binary: &Path,
    invocation_path: &Path,
    lock_timeout_ms: u64,
) -> String {
    let owner_path = case._owner_scratch.0.join("local-rights-owner.json");
    let owner = json!({
        "schema_version":"tos_local_pg_rights_revoke_owner_v1",
        "uid":rustix::process::getuid().as_raw(),
        "source_root":case.source_root,
        "domain":case.lab.domain,
        "principal_id":"test:private-pg-local-rights",
        "allowed_operations":["rights.revoke"],
        "expires_at":"2099-01-01T00:00:00Z"
    });
    fs::write(&owner_path, canonical(&owner)).unwrap();
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let configuration_digest = owner_configuration_digest(&owner_path);
    let invocation = json!({
        "schema_version":"tos_local_pg_rights_revoke_invocation_v1",
        "owner_config":owner_path,
        "private_source_root":case.source_root,
        "native_executable":binary,
        "native_executable_sha256":Digest256::of_bytes(&fs::read(binary).unwrap()).to_prefixed(),
        "budgets":{"lock_timeout_ms":lock_timeout_ms}
    });
    assert!(invocation_path.is_absolute());
    fs::write(invocation_path, canonical(&invocation)).unwrap();
    fs::set_permissions(invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    configuration_digest
}

fn invoke_local_rights_command(
    binary: &Path,
    invocation_path: &Path,
    url: &str,
    request: &Value,
    expect_success: bool,
    discard_reply: bool,
) -> NativeInvocation {
    invoke_with_expected_exit(
        binary,
        invocation_path,
        url,
        request,
        discard_reply,
        expect_success,
        "tos_local_pg_rights_revoke_result_v1",
        None,
        false,
    )
}

fn provision_work_revision_extension(
    case: &RevisionCase,
    software: &SoftwareFixture,
    binary: &Path,
) {
    let _guard = REVISION_SCHEMA_PROVISION_LOCK.lock().unwrap();
    let worker_path = PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").unwrap());
    let invocation_path = case
        ._owner_scratch
        .0
        .join("work-revision-schema-provision-invocation.json");
    invocation(
        case,
        software,
        binary,
        &worker_path,
        &invocation_path,
        &case.owner_config,
        "tos_local_pg_record_revision_invocation_v1",
    );
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    let was_present: bool = db
        .query_one(
            "SELECT to_regclass(current_schema() || '.cmd2_work_revision_history_v1') IS NOT NULL",
            &[],
        )
        .unwrap()
        .get(0);
    if !was_present {
        let absent_read = invoke_refusal(
            binary,
            &invocation_path,
            &case.lab.url,
            &json!({
                "schema_version":"tos_local_source_command_v1",
                "operation":"describe"
            }),
        );
        assert_eq!(absent_read.result["refused"], true);
        assert!(
            absent_read.result["stderr"]
                .as_str()
                .unwrap()
                .contains("PG Work revision extension absent or unknown")
        );
        let extension_still_absent: bool = db
            .query_one(
                "SELECT to_regclass(current_schema() || '.cmd2_work_revision_history_v1') IS NOT NULL",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(
            !extension_still_absent,
            "ordinary Work reads do not provision DDL"
        );
    }
    drop(db);
    let result = invoke(
        binary,
        &invocation_path,
        &case.lab.url,
        &json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"work.revision-schema.provision"
        }),
        false,
    )
    .result;
    assert_eq!(
        result["schema_version"],
        "tos_local_pg_work_revision_schema_provision_result_v1"
    );
    assert_eq!(result["created"], !was_present);
    assert_eq!(result["verified_schema_v1"], true);
    assert_eq!(result["grants_admission"], false);
}

fn provision_selected_revision_extension(
    case: &mut RevisionCase,
    software: &SoftwareFixture,
    binary: &Path,
    invocation_path: &Path,
    refusal_request: Option<&Value>,
) {
    let _guard = REVISION_SCHEMA_PROVISION_LOCK.lock().unwrap();
    let worker_path = PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").unwrap());
    invocation(
        case,
        software,
        binary,
        &worker_path,
        invocation_path,
        &case.owner_config,
        "tos_local_pg_record_revision_invocation_v1",
    );
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    let was_present: bool = db
        .query_one(
            "SELECT to_regclass(current_schema() || '.cmd2_work_revision_history_v1') IS NOT NULL",
            &[],
        )
        .unwrap()
        .get(0);
    drop(db);
    if !was_present && let Some(request) = refusal_request {
        let before_head = case.lab.head_seq();
        let before_counts =
            ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
        let source_path = case.target.source_path;
        let before_source = source_member(case, source_path);
        let refused = invoke_refusal(binary, invocation_path, &case.lab.url, request);
        assert_eq!(refused.result["refused"], true);
        assert!(
            refused.result["stderr"]
                .as_str()
                .unwrap()
                .contains("PG selected record revision extension absent"),
            "selected-null mutation reports the absent retained-replay extension"
        );
        assert_eq!(case.lab.head_seq(), before_head);
        assert_eq!(
            ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table)),
            before_counts,
            "refusal before extension provisioning leaves all durable counters unchanged"
        );
        assert_eq!(
            source_member(case, source_path),
            before_source,
            "refusal before extension provisioning leaves selected source bytes unchanged"
        );
        let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
        let still_absent: bool = db
            .query_one(
                "SELECT to_regclass(current_schema() || '.cmd2_work_revision_history_v1') IS NULL",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(still_absent, "a refused revision does not provision DDL");
    }
    let result = invoke(
        binary,
        invocation_path,
        &case.lab.url,
        &json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"record.revision-schema.provision"
        }),
        false,
    )
    .result;
    assert_eq!(
        result["schema_version"],
        "tos_local_pg_selected_revision_schema_provision_result_v1"
    );
    assert_eq!(result["extension"], "cmd2_work_revision_history_v1");
    assert_eq!(result["created"], !was_present);
    assert_eq!(result["verified_schema_v1"], true);
    assert_eq!(result["grants_admission"], false);
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

/// Applies one pending committed source event in a fresh process using the
/// explicitly selected bounded v1 or v2 source projection profile.
#[test]
fn pg_real_projection_apply_child_entry() {
    if std::env::var_os("TOS_PG_REAL_PROJECTION_APPLY").is_none() {
        return;
    }
    let domain = std::env::var("TOS_PG_REAL_PROJECTION_DOMAIN").unwrap();
    let consumer = std::env::var("TOS_PG_REAL_PROJECTION_CONSUMER").unwrap();
    let profile_id = std::env::var("TOS_PG_REAL_PROJECTION_PROFILE").unwrap();
    let store_root = PathBuf::from(std::env::var_os("TOS_PG_REAL_PROJECTION_STORE").unwrap());
    let mut db = tos_command::DurablePgCoordinator::connect(&super::database_url()).unwrap();
    let store =
        tos_segment_store::SegmentStore::open_existing(&store_root, super::limits()).unwrap();
    let version = if profile_id == tos_command::source_search_projection::SOURCE_SEARCH_PROFILE {
        1
    } else if profile_id == tos_command::source_search_projection::SOURCE_SEARCH_PROFILE_V2 {
        2
    } else {
        panic!("unsupported bounded projection profile: {profile_id}");
    };
    let profile = tos_command::DurableProjectionProfile {
        id: profile_id.clone(),
        version,
        source_contract: super::contract_digest(),
    };
    let limits = tos_command::DurableProjectionLimits {
        max_events: 1,
        max_dependencies: if version == 2 { 2 } else { 1 },
        ..tos_command::DurableProjectionLimits::default()
    };
    let progress = if version == 1 {
        db.apply_projection_batch(
            &store,
            &domain,
            &consumer,
            &profile,
            &tos_command::source_projection_cli::SourceTextExtractor,
            limits,
        )
    } else {
        db.apply_projection_batch(
            &store,
            &domain,
            &consumer,
            &profile,
            &tos_command::source_projection_cli::SourceTextExtractorV2,
            limits,
        )
    }
    .expect("fresh process applies the committed source event");
    assert!(matches!(
        progress.status,
        tos_command::DurableProjectionStatus::Current { .. }
    ));
    let work = serde_json::to_value(&progress.work).unwrap();
    println!(
        "TOS_PG_REAL_PROJECTION_APPLY_RESULT {}",
        serde_json::to_string(&json!({"profile":profile_id,"work":work})).unwrap()
    );
}

/// Keeps the typed current-page disclosure guard open until a parent process
/// probes the exact PostgreSQL rights row for the expected conflicting lock.
#[test]
fn pg_real_source_text_disclosure_guard_child_entry() {
    let Some(selection_path) = std::env::var_os("TOS_PG_REAL_SOURCE_TEXT_SELECTION") else {
        return;
    };
    let ready = PathBuf::from(std::env::var_os("TOS_PG_REAL_SOURCE_TEXT_READY").unwrap());
    let release = PathBuf::from(std::env::var_os("TOS_PG_REAL_SOURCE_TEXT_RELEASE").unwrap());
    let request = SourceTextSearchRequest {
        query: "Sundial".to_owned(),
        limit: 1,
        document_types: vec!["corpus_record/agent".to_owned()],
        source_paths: vec![CREATED_RECORD_PATH.to_owned()],
        cursor: None,
    };
    let mut disclosure = match tos_command::source_projection_cli::run_source_text_search(
        Path::new(&selection_path),
        request,
        Instant::now() + Duration::from_secs(30),
        &NeverAbort,
    )
    .expect("typed source-text search returns its current-page disclosure lease")
    {
        tos_command::source_projection_cli::SourceTextSearchResult::Current {
            mut disclosure,
            ..
        } => {
            disclosure
                .recheck()
                .expect("initial disclosure binding rechecks");
            disclosure
        }
        _ => panic!("the selected source-text projection is current in the child"),
    };
    fs::write(&ready, b"current page and disclosure lease held").unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !release.exists() {
        assert!(Instant::now() < deadline, "parent releases the held lease");
        thread::sleep(Duration::from_millis(10));
    }
    disclosure
        .recheck()
        .expect("the failed concurrent revoke left the held page binding current");
}

fn apply_projection_in_fresh_process(
    case: &RevisionCase,
    consumer: &str,
    profile_id: &str,
) -> (Value, Value) {
    assert!(
        profile_id == tos_command::source_search_projection::SOURCE_SEARCH_PROFILE
            || profile_id == tos_command::source_search_projection::SOURCE_SEARCH_PROFILE_V2
    );
    let started = Instant::now();
    let pg_cgroup_before = isolated_pg_cgroup_snapshot();
    let executable = std::env::current_exe().expect("integration test executable is available");
    let mut command = Command::new(executable);
    command.process_group(0);
    let mut child = command
        .arg("pg_real_projection_apply_child_entry")
        .arg("--nocapture")
        .env("TOS_CMD_POSTGRES_URL", &case.lab.url)
        .env("TOS_SOURCE_PG_URL", &case.lab.url)
        .env("TOS_PG_REAL_PROJECTION_APPLY", "1")
        .env("TOS_PG_REAL_PROJECTION_DOMAIN", &case.lab.domain)
        .env("TOS_PG_REAL_PROJECTION_CONSUMER", consumer)
        .env("TOS_PG_REAL_PROJECTION_PROFILE", profile_id)
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
    let prefix = b"TOS_PG_REAL_PROJECTION_APPLY_RESULT ";
    let report: Value = stdout
        .split(|byte| *byte == b'\n')
        .find_map(|line| line.strip_prefix(prefix))
        .map(|line| serde_json::from_slice(line).expect("child work report is JSON"))
        .expect("fresh projection process reports its logical work");
    assert_eq!(report["profile"].as_str(), Some(profile_id));
    let work = report["work"].clone();
    let pg_cgroup_after = isolated_pg_cgroup_snapshot();
    let resources = json!({
        "scope":"fresh projection child direct-process wait4 rusage and /proc PID I/O; excludes PostgreSQL server",
        "profile":profile_id,
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
    let (_, stdout_bytes) = stdout_reader.join().unwrap();
    let (_, stderr_bytes) = stderr_reader.join().unwrap();
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
        None,
        true,
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
        None,
        true,
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
        None,
        true,
    )
}

fn invoke_source_bootstrap(
    binary: &Path,
    invocation_path: &Path,
    url: &str,
    request: &Value,
    expect_success: bool,
) -> NativeInvocation {
    invoke_with_expected_exit(
        binary,
        invocation_path,
        url,
        request,
        false,
        expect_success,
        "tos_local_pg_source_bootstrap_result_v1",
        Some("source-commands"),
        false,
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
    command_prefix: Option<&str>,
    nested_result: bool,
) -> NativeInvocation {
    let input = canonical(request);
    assert!(input.len() <= 1_048_576);
    let started = Instant::now();
    let pg_cgroup_before = isolated_pg_cgroup_snapshot();
    let mut command = Command::new(binary);
    command.process_group(0);
    if let Some(prefix) = command_prefix {
        command.arg(prefix);
    }
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
            if nested_result {
                dropped_source_read_work = response["result"].get("source_read_work").cloned();
            }
            Value::Null
        } else {
            let response: Value = serde_json::from_slice(&stdout).unwrap();
            assert_eq!(response["schema_version"], expected_envelope);
            assert_eq!(response["grants_admission"], false);
            if nested_result {
                response["result"].clone()
            } else {
                response
            }
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

struct NeverAbort;

impl tos_query::AbortProbe for NeverAbort {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        None
    }
}

fn source_text_selection_path(case: &RevisionCase, consumer: &str, profile: &str) -> PathBuf {
    let path = case
        ._owner_scratch
        .0
        .join(format!("source-text-selection-{consumer}.json"));
    let selection = json!({
        "schema_version":"tos_source_projection_selection_v1",
        "domain":case.lab.domain,
        "source_contract_sha256":contract_digest().to_hex(),
        "consumer_id":consumer,
        "profile":profile,
        "store_root":case.source_root,
        "store_limits":{
            "max_segment_bytes":limits().max_segment_bytes,
            "max_frame_bytes":limits().max_frame_bytes,
            "max_frames":limits().max_frames,
            "max_journal_bytes":limits().max_journal_bytes
        }
    });
    fs::write(&path, canonical(&selection)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

fn invoke_source_text_search(
    binary: &Path,
    selection_path: &Path,
    url: &str,
    request: &Value,
) -> std::process::Output {
    use std::io::Write;

    let mut child = Command::new(binary)
        .arg("source-projection")
        .arg("--selection")
        .arg(selection_path)
        .env("TOS_SOURCE_PG_URL", url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("selected source-text search owner starts");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&canonical(request))
        .expect("source-text search request is delivered");
    let output = child
        .wait_with_output()
        .expect("selected source-text search owner exits");
    assert!(output.stdout.len() <= 1_048_576);
    assert!(output.stderr.len() <= 65_536);
    output
}

fn source_text_search_cli(
    case: &RevisionCase,
    binary: &Path,
    consumer: &str,
    profile: &str,
    request: &Value,
) -> Value {
    let selection_path = source_text_selection_path(case, consumer, profile);
    let output = invoke_source_text_search(binary, &selection_path, &case.lab.url, request);
    assert!(
        output.status.success(),
        "source-text search refused: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["schema_version"], "tos_source_text_search_v1");
    assert_eq!(response["operation"], "search-text");
    response
}

fn projection_text_documents(
    case: &RevisionCase,
    consumer: &str,
    selection_seq: u64,
) -> Vec<SourceTextDocument> {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    db.query(
        "SELECT source_path,document_id,source_revision,title,body,aliases,
                identity,document_type
         FROM cmd2_projection_document
         WHERE domain=$1 AND consumer_id=$2
         ORDER BY source_path COLLATE \"C\",document_id COLLATE \"C\"",
        &[&case.lab.domain, &consumer],
    )
    .unwrap()
    .into_iter()
    .map(|row| SourceTextDocument {
        source_path: row.get(0),
        document_id: row.get(1),
        custody_revision: row.get::<_, i64>(2).to_string(),
        title: row.get(3),
        body: row.get(4),
        aliases: row.get(5),
        identity: row.get(6),
        document_type: row.get(7),
        selection_seq,
    })
    .collect()
}

fn source_text_qry_selection(consumer: &str, selection_seq: u64) -> SourceTextSearchSelection {
    let profile = tos_command::source_search_projection::SOURCE_SEARCH_PROFILE;
    let source_contract = contract_digest();
    let descriptor = format!(
        "{}\0{}\0{}\0{}\0{}",
        tos_query::source_text_search::SOURCE_TEXT_SEARCH_OPERATION,
        profile,
        1,
        source_contract.to_hex(),
        1
    );
    SourceTextSearchSelection {
        consumer_id: consumer.to_owned(),
        domain: "private-source-text-selection".to_owned(),
        consumer_cut: selection_seq.to_string(),
        profile_id: profile.to_owned(),
        profile_sha256: Digest256::of_bytes(descriptor.as_bytes()),
        selection_seq,
    }
}

fn assert_source_text_cli_matches_qry(
    case: &RevisionCase,
    consumer: &str,
    through_seq: u64,
    request: &SourceTextSearchRequest,
    cli_page: &Value,
) {
    let documents = projection_text_documents(case, consumer, through_seq);
    let selection = SourceTextSearchSelection {
        domain: case.lab.domain.clone(),
        ..source_text_qry_selection(consumer, through_seq)
    };
    let budget = SourceTextSearchBudget {
        max_candidates: documents.len().max(1),
        max_text_code_points: 16_000_000,
        max_text_bytes: 64 * 1024 * 1024,
        max_excerpt_code_points: 512,
        max_response_bytes: 1024 * 1024,
        max_work_steps: 100_000_000,
    };
    let page = execute_source_text_search(
        request,
        &selection,
        SourceTextCandidateSet {
            complete: true,
            documents: &documents,
        },
        budget,
        &NeverAbort,
    )
    .expect("QRY ranks the same complete selected projection candidates");
    let encoded = page
        .render_json(1024 * 1024, &NeverAbort)
        .expect("QRY page renders within the owner bound");
    let qry_page: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(&qry_page, cli_page, "the PG adapter and shared QRY agree");
}

fn run_source_text_search_scenario(
    case: &RevisionCase,
    binary: &Path,
    consumer: &str,
    through_seq: u64,
) -> Value {
    let profile = tos_command::source_search_projection::SOURCE_SEARCH_PROFILE;
    let paths = vec![RECORD_PATH.to_owned(), CREATED_RECORD_PATH.to_owned()];
    let filters = vec!["corpus_record/agent".to_owned()];
    let cyrillic = SourceTextSearchRequest {
        query: "а".to_owned(),
        limit: 10,
        document_types: filters.clone(),
        source_paths: paths.clone(),
        cursor: None,
    };
    let cyrillic_response = source_text_search_cli(
        case,
        binary,
        consumer,
        profile,
        &json!({
            "operation":"search-text",
            "query":cyrillic.query,
            "limit":cyrillic.limit,
            "document_types":cyrillic.document_types,
            "source_paths":cyrillic.source_paths
        }),
    );
    assert_eq!(cyrillic_response["status"], "current");
    assert!(
        cyrillic_response["result"]["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit["document_id"] == "tos.agent.parmenides-of-elea")
    );
    assert!(
        cyrillic_response["result"]["hits"]
            .as_array()
            .unwrap()
            .iter()
            .all(|hit| hit["document_type"] == "corpus_record/agent")
    );
    assert_source_text_cli_matches_qry(
        case,
        consumer,
        through_seq,
        &cyrillic,
        &cyrillic_response["result"],
    );

    // The raw query is over the old 4096-byte owner ceiling while remaining
    // within QRY's 4096-code-point input bound; FND strip/lower reduces it to
    // the same one-character Cyrillic query as the ordinary request.
    let unicode_whitespace_query =
        format!("{}А{}", "\u{3000}".repeat(1366), "\u{3000}".repeat(1366));
    assert!(unicode_whitespace_query.len() > 4096);
    let unicode_whitespace = SourceTextSearchRequest {
        query: unicode_whitespace_query,
        ..cyrillic.clone()
    };
    let unicode_whitespace_response = source_text_search_cli(
        case,
        binary,
        consumer,
        profile,
        &json!({
            "operation":"search-text",
            "query":unicode_whitespace.query,
            "limit":unicode_whitespace.limit,
            "document_types":unicode_whitespace.document_types,
            "source_paths":unicode_whitespace.source_paths
        }),
    );
    assert_eq!(unicode_whitespace_response["status"], "current");
    assert!(
        unicode_whitespace_response["result"]["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit["document_id"] == "tos.agent.parmenides-of-elea")
    );
    assert_source_text_cli_matches_qry(
        case,
        consumer,
        through_seq,
        &unicode_whitespace,
        &unicode_whitespace_response["result"],
    );

    let mut wildcard_searches = serde_json::Map::new();
    for query in ["%", "_"] {
        let request = SourceTextSearchRequest {
            query: query.to_owned(),
            limit: 10,
            document_types: filters.clone(),
            source_paths: vec![CREATED_RECORD_PATH.to_owned()],
            cursor: None,
        };
        let response = source_text_search_cli(
            case,
            binary,
            consumer,
            profile,
            &json!({
                "operation":"search-text",
                "query":request.query,
                "limit":request.limit,
                "document_types":request.document_types,
                "source_paths":request.source_paths
            }),
        );
        assert_eq!(response["status"], "current");
        let hits = response["result"]["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 1, "wildcard metacharacter is matched literally");
        assert_eq!(hits[0]["document_id"], CREATED_RECORD_ID);
        assert_source_text_cli_matches_qry(
            case,
            consumer,
            through_seq,
            &request,
            &response["result"],
        );
        wildcard_searches.insert(query.to_owned(), response["result"].clone());
    }

    let exact_filter = SourceTextSearchRequest {
        query: "Sundial".to_owned(),
        limit: 10,
        document_types: filters.clone(),
        source_paths: vec![CREATED_RECORD_PATH.to_owned()],
        cursor: None,
    };
    let exact_filter_response = source_text_search_cli(
        case,
        binary,
        consumer,
        profile,
        &json!({
            "operation":"search-text",
            "query":exact_filter.query,
            "limit":exact_filter.limit,
            "document_types":exact_filter.document_types,
            "source_paths":exact_filter.source_paths
        }),
    );
    let exact_hits = exact_filter_response["result"]["hits"].as_array().unwrap();
    assert_eq!(exact_hits.len(), 1);
    assert_eq!(exact_hits[0]["source_path"], CREATED_RECORD_PATH);
    assert_source_text_cli_matches_qry(
        case,
        consumer,
        through_seq,
        &exact_filter,
        &exact_filter_response["result"],
    );
    let absent_path = SourceTextSearchRequest {
        source_paths: vec!["ToS/source-witnesses/agents/not-selected/agent.json".to_owned()],
        ..exact_filter.clone()
    };
    let absent_path_response = source_text_search_cli(
        case,
        binary,
        consumer,
        profile,
        &json!({
            "operation":"search-text",
            "query":absent_path.query,
            "limit":absent_path.limit,
            "document_types":absent_path.document_types,
            "source_paths":absent_path.source_paths
        }),
    );
    assert_eq!(absent_path_response["status"], "current");
    assert!(
        absent_path_response["result"]["hits"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_source_text_cli_matches_qry(
        case,
        consumer,
        through_seq,
        &absent_path,
        &absent_path_response["result"],
    );

    let first_page_request = SourceTextSearchRequest {
        query: "e".to_owned(),
        limit: 1,
        document_types: filters.clone(),
        source_paths: paths.clone(),
        cursor: None,
    };
    let first_page = source_text_search_cli(
        case,
        binary,
        consumer,
        profile,
        &json!({
            "operation":"search-text",
            "query":first_page_request.query,
            "limit":first_page_request.limit,
            "document_types":first_page_request.document_types,
            "source_paths":first_page_request.source_paths
        }),
    );
    assert_eq!(first_page["status"], "current");
    assert_eq!(first_page["result"]["hits"].as_array().unwrap().len(), 1);
    assert_eq!(first_page["result"]["has_more"], true);
    assert_source_text_cli_matches_qry(
        case,
        consumer,
        through_seq,
        &first_page_request,
        &first_page["result"],
    );
    let first_hit = first_page["result"]["hits"][0].clone();
    let cursor = first_page["result"]["next_cursor"]
        .as_str()
        .expect("a second matching document produces a continuation cursor")
        .to_owned();
    let second_page_request = SourceTextSearchRequest {
        cursor: Some(cursor.clone()),
        ..first_page_request.clone()
    };
    let second_page = source_text_search_cli(
        case,
        binary,
        consumer,
        profile,
        &json!({
            "operation":"search-text",
            "query":second_page_request.query,
            "limit":second_page_request.limit,
            "document_types":second_page_request.document_types,
            "source_paths":second_page_request.source_paths,
            "cursor":second_page_request.cursor
        }),
    );
    assert_eq!(second_page["status"], "current");
    assert_eq!(second_page["result"]["hits"].as_array().unwrap().len(), 1);
    assert_ne!(second_page["result"]["hits"][0], first_hit);
    assert_eq!(second_page["result"]["has_more"], false);
    assert_source_text_cli_matches_qry(
        case,
        consumer,
        through_seq,
        &second_page_request,
        &second_page["result"],
    );
    let full_order_request = SourceTextSearchRequest {
        limit: 10,
        cursor: None,
        ..first_page_request.clone()
    };
    let full_order = source_text_search_cli(
        case,
        binary,
        consumer,
        profile,
        &json!({
            "operation":"search-text",
            "query":full_order_request.query,
            "limit":full_order_request.limit,
            "document_types":full_order_request.document_types,
            "source_paths":full_order_request.source_paths
        }),
    );
    let ordered_hits = full_order["result"]["hits"].as_array().unwrap();
    assert!(
        ordered_hits.len() >= 2,
        "the selected text has multiple matches"
    );
    assert_eq!(ordered_hits[0], first_hit);
    assert_eq!(ordered_hits[1], second_page["result"]["hits"][0]);
    assert!(
        ordered_hits
            .windows(2)
            .all(|pair| { pair[0]["rank"].as_u64().unwrap() <= pair[1]["rank"].as_u64().unwrap() })
    );
    assert_source_text_cli_matches_qry(
        case,
        consumer,
        through_seq,
        &full_order_request,
        &full_order["result"],
    );

    let docs = projection_text_documents(case, consumer, through_seq);
    let selection = SourceTextSearchSelection {
        domain: case.lab.domain.clone(),
        ..source_text_qry_selection(consumer, through_seq)
    };
    let budget = SourceTextSearchBudget {
        max_candidates: docs.len().max(1),
        max_text_code_points: 16_000_000,
        max_text_bytes: 64 * 1024 * 1024,
        max_excerpt_code_points: 512,
        max_response_bytes: 1024 * 1024,
        max_work_steps: 100_000_000,
    };
    let stale_filter = SourceTextSearchRequest {
        source_paths: vec![CREATED_RECORD_PATH.to_owned()],
        ..second_page_request.clone()
    };
    let stale_filter_error = execute_source_text_search(
        &stale_filter,
        &selection,
        SourceTextCandidateSet {
            complete: true,
            documents: &docs,
        },
        budget,
        &NeverAbort,
    )
    .expect_err("a continuation cannot cross a source-path filter change");
    assert_eq!(
        stale_filter_error.code,
        tos_query::source_text_search::SourceTextSearchErrorCode::StaleCursor
    );
    let wrong_consumer = SourceTextSearchSelection {
        consumer_id: format!("{consumer}-wrong"),
        ..selection.clone()
    };
    let stale_consumer_error = execute_source_text_search(
        &second_page_request,
        &wrong_consumer,
        SourceTextCandidateSet {
            complete: true,
            documents: &docs,
        },
        budget,
        &NeverAbort,
    )
    .expect_err("a continuation cannot cross a selected consumer change");
    assert_eq!(
        stale_consumer_error.code,
        tos_query::source_text_search::SourceTextSearchErrorCode::StaleCursor
    );

    let stale_response = source_text_search_cli(
        case,
        binary,
        &format!("{consumer}-not-initialized"),
        profile,
        &json!({"operation":"search-text","query":"Sundial","limit":1}),
    );
    assert_eq!(stale_response["status"], "stale");

    json!({
        "projection_seq":through_seq,
        "unicode_cyrillic_one_character":cyrillic_response["result"],
        "literal_wildcards":wildcard_searches,
        "exact_filter":exact_filter_response["result"],
        "absent_exact_filter_hits":absent_path_response["result"]["hits"].as_array().unwrap().len(),
        "cursor_pages":[first_page["result"],second_page["result"]],
        "stale_cursor_wrong_filter":stale_filter_error.code.as_str(),
        "stale_cursor_wrong_consumer":stale_consumer_error.code.as_str(),
        "stale_owner_consumer":stale_response["status"]
    })
}

fn assert_source_text_disclosure_lease_blocks_revoke(
    case: &RevisionCase,
    consumer: &str,
    binary: &Path,
) -> Value {
    let selection_path = source_text_selection_path(
        case,
        consumer,
        tos_command::source_search_projection::SOURCE_SEARCH_PROFILE,
    );
    let ready = case
        ._owner_scratch
        .0
        .join(format!("source-text-guard-{consumer}-ready"));
    let release = case
        ._owner_scratch
        .0
        .join(format!("source-text-guard-{consumer}-release"));
    let rights_invocation_path = case._owner_scratch.0.join(format!(
        "source-text-guard-{consumer}-rights-invocation.json"
    ));
    let configuration_digest = local_rights_invocation(case, binary, &rights_invocation_path, 150);
    let describe_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"describe",
        "expected_configuration":configuration_digest
    });
    let before_revoke = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &describe_request,
        true,
        false,
    );
    assert!(before_revoke.result["rights"]["rights_allowed"] == true);
    let revoke_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"rights.revoke",
        "command_id":"test:source-text-lease-lock-timeout",
        "expected_configuration":configuration_digest,
        "expected_rights_version":before_revoke.result["rights"]["rights_version"],
        "reason":"bounded disclosure-lease lock probe"
    });
    let executable = std::env::current_exe().expect("integration test executable is available");
    let mut command = Command::new(executable);
    command.process_group(0);
    let mut child = command
        .arg("pg_real_source_text_disclosure_guard_child_entry")
        .arg("--nocapture")
        .env("TOS_CMD_POSTGRES_URL", &case.lab.url)
        .env("TOS_SOURCE_PG_URL", &case.lab.url)
        .env("TOS_PG_REAL_SOURCE_TEXT_SELECTION", &selection_path)
        .env("TOS_PG_REAL_SOURCE_TEXT_READY", &ready)
        .env("TOS_PG_REAL_SOURCE_TEXT_RELEASE", &release)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the typed disclosure guard child starts");
    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, 65_536).unwrap());
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, 65_536).unwrap());
    let ready_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if ready.exists() {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            let stdout = stdout_reader.join().unwrap().0;
            let stderr = stderr_reader.join().unwrap().0;
            panic!(
                "typed source-text search child exited before holding the guard: status={status}, stdout={}, stderr={}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
        if Instant::now() >= ready_deadline {
            unsafe {
                libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            panic!("typed source-text guard child missed its ready barrier");
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read(&ready).unwrap(),
        b"current page and disclosure lease held"
    );
    let revoke_attempt = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &revoke_request,
        false,
        false,
    );
    assert!(revoke_attempt.result["refused"].as_bool().unwrap());
    assert!(
        revoke_attempt.result["stderr"]
            .as_str()
            .unwrap()
            .contains("CMD local rights state is locked")
    );
    fs::write(&release, b"release").unwrap();
    let exit_deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= exit_deadline {
            unsafe {
                libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            panic!("typed source-text guard child did not release its page");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (stdout, stdout_bytes) = stdout_reader.join().unwrap();
    let (stderr, stderr_bytes) = stderr_reader.join().unwrap();
    assert_eq!(status.code(), Some(0));
    let after_release = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &describe_request,
        true,
        false,
    );
    assert!(after_release.result["rights"]["rights_allowed"] == true);
    assert_eq!(
        after_release.result["rights"]["rights_version"],
        before_revoke.result["rights"]["rights_version"]
    );
    json!({
        "typed_current_page_guard":"held across concurrent rights update attempt",
        "concurrent_revoke_refused":revoke_attempt.result["refused"],
        "rights_after_timeout":after_release.result["rights"],
        "child_exit_status":status.code(),
        "child_stdout_bytes":stdout_bytes,
        "child_stderr_bytes":stderr_bytes,
        "child_stdout":String::from_utf8_lossy(&stdout),
        "child_stderr":String::from_utf8_lossy(&stderr)
    })
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

fn assert_initial_creation_receipt_rows(case: &RevisionCase, work: &Value) {
    // Independent creation-schema contract; not taken from reported work.
    const REQUIRED: &[&str] = &[
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/knowledge-assessment.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
    ];
    // The fixture's exact ordered snapshot has no dependency edges. Bootstrap
    // commits it in protocol batches of at most 64 members. Each selected read
    // authenticates that entire receipt plus its one selected member row.
    let expected = REQUIRED.iter().fold(0u64, |sum, path| {
        let ordinal = case
            .all_source
            .keys()
            .position(|candidate| candidate == path)
            .expect("required creation schema belongs to the independent fixture");
        let batch_start = (ordinal / 64) * 64;
        let members = case.all_source.len().checked_sub(batch_start).unwrap().min(64);
        sum.checked_add(u64::try_from(members).unwrap().checked_add(1).unwrap())
            .unwrap()
    });
    assert_eq!(
        work["exact_source_members_read"].as_u64(),
        Some(u64::try_from(REQUIRED.len()).unwrap()),
        "initial Agent creation reads exactly its independent schema inputs"
    );
    assert_eq!(
        work["committed_member_rows_examined"].as_u64(),
        Some(expected),
        "initial creation authenticates the selected bootstrap receipt batches"
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
    let invocation_path = case._owner_scratch.0.join("revision-invocation.json");
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
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
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
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
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
        refusal_text.contains("bounded source revision read metadata"),
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
    let selected_owner = matches!(
        target.owner_schema,
        "tos_local_corpus_revision_owner_v2"
            | "tos_local_corpus_revision_owner_v3"
            | "tos_local_native_metadata_revision_owner_v1"
    );
    if selected_owner {
        assert!(
            described["command_operations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|operation| operation == "record.revision-schema.provision"),
            "PG discovery reports the explicit selected revision schema route"
        );
    }
    assert_eq!(described["source"]["id"], target.record_id);
    assert_eq!(
        described["owner_configuration"],
        owner_configuration_digest(&case.owner_config)
    );
    let mut fields = json!({});
    if let Some(preferred_label) = target.preferred_label {
        fields["preferred_label"] = json!(preferred_label);
    }
    if let Some(notes) = target.notes {
        fields["notes"] = json!(notes);
    }
    if let Some((field, raw_value)) = target.native_field_patch {
        fields[field] = serde_json::from_str(raw_value).unwrap();
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
    if target.record_type == "work" {
        request["expected_publication"] = Value::Null;
    } else if matches!(
        target.owner_schema,
        "tos_local_corpus_revision_owner_v2"
            | "tos_local_corpus_revision_owner_v3"
            | "tos_local_native_metadata_revision_owner_v1"
    ) {
        request["expected_publication"] = preview["publication_snapshot"].clone();
    }
    if selected_owner && request["expected_publication"].is_null() {
        provision_selected_revision_extension(
            case,
            software,
            binary,
            &invocation_path,
            Some(&request),
        );
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
    if let Some(preferred_label) = target.preferred_label {
        expected["preferred_label"] = json!(preferred_label);
    }
    if let Some(notes) = target.notes {
        expected["notes"] = json!(notes);
    }
    if let Some((field, raw_value)) = target.native_field_patch {
        expected[field] = serde_json::from_str(raw_value).unwrap();
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
    assert_eq!(
        forms["forms"].as_array().unwrap().len(),
        target.form_ids.len()
    );
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
    case.lab
        .db
        .cold_verify_cut(&case.lab.store, &case.lab.domain)
        .expect("the Work revision is part of a cold-verified source generation");
    let current_custody_revision: i64 = Client::connect(&case.lab.url, NoTls)
        .unwrap()
        .query_one(
            "SELECT revision FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &record_path],
        )
        .unwrap()
        .get(0);
    let cold_current = case
        .lab
        .db
        .cold_recover_exact(
            &case.lab.store,
            &case.lab.domain,
            target.source_path,
            current_custody_revision.try_into().unwrap(),
        )
        .expect("the current Work revision has an exact cold locator");
    assert_eq!(
        case.lab
            .db
            .warm_read_selected(&case.lab.store, &cold_current, 1_048_576)
            .unwrap(),
        current_record,
        "cold reopen returns the exact revised Work bytes"
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
    let record_basename = Path::new(target.source_path)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("selected source record has a UTF-8 basename");
    let binding = &archive_manifest["files"][record_basename];
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
    let invocation_path = case._owner_scratch.0.join("creation-invocation.json");
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
        "preferred_label":"Sundial % _ PostgreSQL addition fixture",
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
    request["expected_source"] = Value::Null;
    request["expected_revision"] = Value::Null;
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
        "ToS/source-witnesses/agents/pg-real-addition-fixture/source-create-request.json"
            .to_owned(),
        "ToS/source-witnesses/agents/pg-real-addition-fixture/source-create-environment.json"
            .to_owned(),
        "ToS/source-witnesses/agents/pg-real-addition-fixture/source-create-provenance.jsonl"
            .to_owned(),
        "ToS/source-witnesses/agents/pg-real-addition-fixture/source-create-receipt.json"
            .to_owned(),
    ]);
    let actual_paths = retry["created_source_paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path.as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_paths, expected_paths);
    let mut created_source_bytes = 0_u64;
    for path in &actual_paths {
        let raw = source_member(case, path);
        assert!(
            !raw.is_empty(),
            "created source member is exactly readable: {path}"
        );
        created_source_bytes += raw.len() as u64;
    }
    let created_record = source_member(case, CREATED_RECORD_PATH);
    assert_eq!(created_record, published_source_bytes(&record));
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
        "created_source_bytes":created_source_bytes,
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
    let profile = DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
        version: 1,
        source_contract: contract_digest(),
    };
    case.lab.db.init_projection_schema().unwrap();
    let extractor = tos_command::source_projection_cli::SourceTextExtractor;
    let baseline_head = case.lab.head_seq();
    let baseline_limit = usize::try_from(baseline_head).unwrap();
    assert!((1..=256).contains(&baseline_limit));
    let baseline = case
        .lab
        .db
        .apply_projection_batch(
            &case.lab.store,
            &case.lab.domain,
            &consumer,
            &profile,
            &extractor,
            DurableProjectionLimits {
                max_events: baseline_limit,
                ..DurableProjectionLimits::default()
            },
        )
        .expect("the bounded baseline projects from committed STO bytes before the addition");
    let baseline_work = serde_json::to_value(&baseline.work).unwrap();
    assert!(matches!(
        baseline.status,
        DurableProjectionStatus::Current { through_seq } if through_seq == baseline_head
    ));
    let baseline_rows = projection_rows(case, &consumer);
    assert!(
        baseline_rows
            .iter()
            .any(|row| row.contains("tos.agent.parmenides-of-elea"))
    );
    assert!(
        baseline_rows
            .iter()
            .all(|row| !row.contains(CREATED_RECORD_ID)),
        "the new Agent is absent from the projection baseline"
    );
    if records == 3 {
        assert!(
            baseline_rows
                .iter()
                .any(|row| row.contains("tos.agent.justus-lipsius"))
        );
        assert!(
            baseline_rows
                .iter()
                .any(|row| row.contains("tos.agent.hans-hahn"))
        );
    }

    let creation = create_agent(case, software, binary);
    let addition_head = case.lab.head_seq();
    assert_eq!(addition_head, baseline_head + 1);
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, &consumer, &profile, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == baseline_head && head_seq == addition_head
    ));
    let (addition_work, addition_apply_resources) = apply_projection_in_fresh_process(
        case,
        &consumer,
        tos_command::source_search_projection::SOURCE_SEARCH_PROFILE,
    );
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, &consumer, &profile, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Current { through_seq } if through_seq == addition_head
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
        "the new Agent event publishes its selected-text document"
    );
    let baseline_documents = projection_documents(&baseline_rows);
    let before_documents = projection_documents(&before_rows);
    let created_key = (CREATED_RECORD_PATH.to_owned(), CREATED_RECORD_ID.to_owned());
    let mut expected_documents = baseline_documents.keys().cloned().collect::<BTreeSet<_>>();
    assert!(expected_documents.insert(created_key.clone()));
    assert_eq!(
        before_documents.keys().cloned().collect::<BTreeSet<_>>(),
        expected_documents,
        "the addition publishes exactly one new document"
    );
    for (key, document) in &baseline_documents {
        assert_eq!(
            before_documents.get(key),
            Some(document),
            "the addition leaves baseline projection document {key:?} unchanged"
        );
    }
    assert_eq!(addition_work["extracted_documents"], 1);
    assert_eq!(addition_work["published_documents"], 1);
    let addition_search = case
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
    assert!(matches!(
        addition_search,
        DurableProjectionSearch::Current { ref hits, through_seq }
            if through_seq == addition_head
                && hits.iter().any(|hit| hit.document_id == CREATED_RECORD_ID)
    ));
    let addition_text_search =
        run_source_text_search_scenario(case, binary, &consumer, addition_head);
    let disclosure_lease =
        assert_source_text_disclosure_lease_blocks_revoke(case, &consumer, binary);
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
            if through_seq == addition_head && head_seq == revision_head
    ));
    let pending_text_search = source_text_search_cli(
        case,
        binary,
        &consumer,
        tos_command::source_search_projection::SOURCE_SEARCH_PROFILE,
        &json!({
            "operation":"search-text",
            "query":"Quartz",
            "limit":1,
            "document_types":["corpus_record/agent"],
            "source_paths":[RECORD_PATH]
        }),
    );
    assert_eq!(pending_text_search["status"], "pending");
    assert_eq!(pending_text_search["through_seq"], addition_head);
    assert_eq!(pending_text_search["head_seq"], revision_head);
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
            if through_seq == addition_head && head_seq == revision_head
    ));
    let (revision_work, resume_process_resources) = apply_projection_in_fresh_process(
        case,
        &consumer,
        tos_command::source_search_projection::SOURCE_SEARCH_PROFILE,
    );
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
    let selection_path = source_text_selection_path(
        case,
        &consumer,
        tos_command::source_search_projection::SOURCE_SEARCH_PROFILE,
    );
    let rights_invocation_path = case
        ._owner_scratch
        .0
        .join("source-text-projection-rights-invocation.json");
    let configuration_digest =
        local_rights_invocation(case, binary, &rights_invocation_path, 5_000);
    let describe_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"describe",
        "expected_configuration":configuration_digest
    });
    let before_revoke = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &describe_request,
        true,
        false,
    );
    assert!(before_revoke.result["rights"]["rights_allowed"] == true);
    let revoke_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"rights.revoke",
        "command_id":"test:source-text-projection-revoke",
        "expected_configuration":configuration_digest,
        "expected_rights_version":before_revoke.result["rights"]["rights_version"],
        "reason":"withdraw current private CMD projection use"
    });
    let dropped_reply = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &revoke_request,
        true,
        true,
    );
    assert!(dropped_reply.result.is_null());
    let replay = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &revoke_request,
        true,
        false,
    );
    assert!(replay.result["receipt"]["replayed"] == true);
    assert!(
        !replay
            .result
            .to_string()
            .contains("withdraw current private CMD projection use"),
        "the private revoke reason is not returned"
    );
    assert!(replay.result["current_rights"]["rights_allowed"] == false);
    assert_eq!(
        replay.result["current_rights"]["rights_version"],
        replay.result["receipt"]["rights_version"]
    );
    let mut changed_replay = revoke_request.clone();
    changed_replay["expected_rights_version"] =
        replay.result["current_rights"]["rights_version"].clone();
    let command_reuse = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &changed_replay,
        false,
        false,
    );
    assert!(command_reuse.result["refused"].as_bool().unwrap());
    let command_reuse_response: Value =
        serde_json::from_str(command_reuse.result["stdout"].as_str().unwrap())
            .expect("command ID collision returns a typed owner error");
    assert_eq!(command_reuse_response["error"], "JournalConflict");
    let new_revoke = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"rights.revoke",
        "command_id":"test:source-text-projection-second-revoke",
        "expected_configuration":configuration_digest,
        "expected_rights_version":replay.result["current_rights"]["rights_version"],
        "reason":"a new command cannot revoke an already revoked domain"
    });
    let new_revoke_refusal = invoke_local_rights_command(
        binary,
        &rights_invocation_path,
        &case.lab.url,
        &new_revoke,
        false,
        false,
    );
    assert!(new_revoke_refusal.result["refused"].as_bool().unwrap());
    let revoked_search = invoke_source_text_search(
        binary,
        &selection_path,
        &case.lab.url,
        &json!({"operation":"search-text","query":"Sundial","limit":1}),
    );
    assert!(!revoked_search.status.success());
    assert!(
        String::from_utf8_lossy(&revoked_search.stderr).contains("source-text search refused"),
        "the native source-text owner refuses a rights-revoked projection"
    );
    json!({
        "consumer":consumer,
        "baseline_documents":baseline_rows.len(),
        "documents_after_addition":before_rows.len(),
        "final_documents":after_rows.len(),
        "baseline_head":baseline_head,
        "addition_head":addition_head,
        "projection_cursor":case.lab.head_seq(),
        "new_text_search":"Quartz",
        "projection_work":{
            "scope":"logical projection work; segment_bytes_read counts selected STO frame and envelope bytes read, while physical device I/O is separate; publication counters cover work applied by this consumer",
            "bootstrap":baseline_work,
            "addition":addition_work,
            "revision":revision_work
        },
        "addition_projection_head":addition_head,
        "addition_apply_resources":addition_apply_resources,
        "revision_apply_resources":revision_apply_resources,
        "addition_source_text_search":addition_text_search,
        "source_text_disclosure_lease":disclosure_lease,
        "pending_source_text_search":pending_text_search,
        "local_cmd_rights_revoke":{
            "lost_reply_recovered":replay.result["receipt"]["replayed"],
            "command_id_reuse_refused":command_reuse.result["refused"],
            "new_revoke_after_revocation_refused":new_revoke_refusal.result["refused"],
            "current_rights":replay.result["current_rights"]
        },
        "rights_revocation_refuses_source_text":{
            "exit_status":revoked_search.status.code(),
            "stderr":String::from_utf8_lossy(&revoked_search.stderr)
        },
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
    let (revision_work, revision_apply_resources) = apply_projection_in_fresh_process(
        case,
        &consumer,
        tos_command::source_search_projection::SOURCE_SEARCH_PROFILE_V2,
    );
    assert_eq!(revision_work["extracted_documents"], 4);
    assert_eq!(revision_work["published_documents"], 4);
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, &consumer, &profile, Duration::from_secs(15))
            .unwrap(),
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
    assert_eq!(
        one.original_record, three.original_record,
        "two private CMD domains select the same exact source bytes"
    );
    assert_ne!(one.lab.domain, three.lab.domain);
    let one_projection = project_case(&mut one, &software, 1, &binary);
    let revoked_domain = one
        .lab
        .db
        .local_rights_snapshot(&one.lab.domain)
        .expect("the revoked domain retains its CMD-local rights snapshot");
    let independent_domain = three
        .lab
        .db
        .local_rights_snapshot(&three.lab.domain)
        .expect("the same source bytes retain their independent domain rights");
    assert!(!revoked_domain.rights_allowed);
    assert!(independent_domain.rights_allowed);
    let three_projection = project_case(&mut three, &software, 3, &binary);
    for (records, projection) in [(1, &one_projection), (3, &three_projection)] {
        assert_bounded_committed_member_rows(
            &projection["projection_work"]["bootstrap"],
            &format!("{records}-record bootstrap projection"),
        );
        assert_bounded_committed_member_rows(
            &projection["projection_work"]["addition"],
            &format!("{records}-record new Agent projection"),
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
        assert_initial_creation_receipt_rows(
            if records == 1 { &one } else { &three },
            &projection["initial_agent_creation"]["initial_source_read_work"],
        );
        assert_eq!(
            projection["projection_work"]["addition"]["exact_source_bytes_read"],
            projection["initial_agent_creation"]["created_source_bytes"],
            "addition reads exactly its six committed members, including measured provenance"
        );
    }
    for counter in [
        "examined_event_members",
        "exact_source_members_read",
        "committed_member_rows_examined",
        "extracted_documents",
        "published_source_paths",
        "published_documents",
    ] {
        assert_eq!(
            one_projection["projection_work"]["addition"][counter],
            three_projection["projection_work"]["addition"][counter],
            "the same new Agent event has the same {counter} projection work at 1 and 3 records"
        );
    }
    // Independent captures retain real timing values. Their serialized byte
    // lengths can differ even when the selected members and work are the same.
    // Account for their exact payloads above, then compare framing work.
    let segment_overhead = |projection: &Value| {
        let work = &projection["projection_work"]["addition"];
        work["segment_bytes_read"]
            .as_u64()
            .unwrap()
            .checked_sub(work["exact_source_bytes_read"].as_u64().unwrap())
            .expect("segment reads include the exact selected member bytes")
    };
    assert_eq!(
        segment_overhead(&one_projection),
        segment_overhead(&three_projection)
    );
    for counter in [
        "examined_event_members",
        "exact_source_members_read",
        "exact_source_bytes_read",
        "segment_bytes_read",
        "committed_member_rows_examined",
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
fn real_pg_selected_null_revision_requires_schema_and_replays_exactly() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let mut target = PARMENIDES_TARGET;
    target.owner_schema = "tos_local_corpus_revision_owner_v2";
    target.command_id = "test:pg-selected-null-revision";
    let source = source_cut(&repository, 1, deadline, &cancelled);
    let mut case = bootstrap_case_from_cut(&software, source, target, deadline, &cancelled);
    let binary = native_binary();
    let worker_path = PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").unwrap());
    let invocation_path = case
        ._owner_scratch
        .0
        .join("selected-null-revision-invocation.json");
    invocation(
        &case,
        &software,
        &binary,
        &worker_path,
        &invocation_path,
        &case.owner_config,
        "tos_local_pg_record_revision_invocation_v1",
    );

    let described = invoke(
        &binary,
        &invocation_path,
        &case.lab.url,
        &json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
        false,
    )
    .result;
    assert!(
        described["command_operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|operation| operation == "record.revision-schema.provision")
    );
    let mut fields = json!({});
    if let Some(preferred_label) = target.preferred_label {
        fields["preferred_label"] = json!(preferred_label);
    }
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
    let preview = invoke(
        &binary,
        &invocation_path,
        &case.lab.url,
        &preview_request,
        false,
    )
    .result;
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
    request["expected_publication"] = Value::Null;

    provision_selected_revision_extension(
        &mut case,
        &software,
        &binary,
        &invocation_path,
        Some(&request),
    );
    let before_head = case.lab.head_seq();
    let discarded = invoke(&binary, &invocation_path, &case.lab.url, &request, true);
    let committed_head = case.lab.head_seq();
    assert_eq!(committed_head, before_head + 1);
    let committed_history = source_member(&mut case, target.history_path);
    let retry = invoke(&binary, &invocation_path, &case.lab.url, &request, false).result;
    assert!(discarded.result.is_null());
    assert_eq!(retry["replayed"], true);
    assert_eq!(case.lab.head_seq(), committed_head);
    assert_eq!(
        source_member(&mut case, target.history_path),
        committed_history,
        "the exact retry retains the original revision timestamp and receipt"
    );
    let rows = Client::connect(&case.lab.url, NoTls)
        .unwrap()
        .query(
            "SELECT generation,command_id,commit_seq
             FROM cmd2_work_revision_history_v1
             WHERE domain=$1 AND source_path=$2 AND record_id=$3",
            &[&case.lab.domain, &target.source_path, &target.record_id],
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, i64>(0), 1);
    assert_eq!(rows[0].get::<_, String>(1), target.command_id);
    assert_eq!(rows[0].get::<_, i64>(2), committed_head as i64);
}

#[test]
fn real_pg_selected_corpus_v2_and_v3_non_work_revisions_replay() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let binary = native_binary();
    for (owner_schema, command_id) in [
        (
            "tos_local_corpus_revision_owner_v2",
            "test:pg-selected-corpus-v2-agent-correction",
        ),
        (
            "tos_local_corpus_revision_owner_v3",
            "test:pg-selected-corpus-v3-agent-correction",
        ),
    ] {
        let mut target = PARMENIDES_TARGET;
        target.owner_schema = owner_schema;
        target.command_id = command_id;
        let source = source_cut(&repository, 1, deadline, &cancelled);
        let mut case = bootstrap_case_from_cut(&software, source, target, deadline, &cancelled);
        let result = record_correction(&mut case, &software, &binary);
        assert_eq!(result["record_id"], target.record_id);
        assert_eq!(result["source_path"], target.source_path);
        assert_eq!(result["replayed"], true);
        assert_eq!(case.lab.head_seq(), result["committed_head"]);
    }
}

#[test]
fn real_pg_native_artifact_revision_replays_and_refuses_revoked_rights() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let source = native_source_cut(&repository, ARTIFACT_TARGET, deadline, &cancelled);
    let mut case =
        bootstrap_case_from_cut(&software, source, ARTIFACT_TARGET, deadline, &cancelled);
    let binary = native_binary();
    let revision = record_correction(&mut case, &software, &binary);
    assert_eq!(revision["record_id"], ARTIFACT_TARGET.record_id);
    assert_eq!(revision["source_path"], ARTIFACT_TARGET.source_path);
    assert_eq!(revision["replayed"], true);
    assert_eq!(case.lab.head_seq(), revision["committed_head"]);

    let source_before_revoke = source_member(&mut case, ARTIFACT_TARGET.source_path);
    assert_eq!(
        Digest256::of_bytes(&source_before_revoke).to_hex(),
        revision["current_record_sha256"]
    );
    // The source commitment can be inspected without disclosing revoked bytes.
    let source_commitment = |case: &RevisionCase| {
        let row = Client::connect(&case.lab.url, NoTls)
            .unwrap()
            .query_one(
                "SELECT source_revision,source_epoch,source_generation
                 FROM cmd2_domain WHERE domain=$1",
                &[&case.lab.domain],
            )
            .unwrap();
        (
            row.get::<_, String>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
        )
    };
    let source_commitment_before = source_commitment(&case);
    let rights_invocation_path = case
        ._owner_scratch
        .0
        .join("artifact-rights-revoke-invocation.json");
    let rights_configuration =
        local_rights_invocation(&case, &binary, &rights_invocation_path, 5_000);
    let rights_describe = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"describe",
        "expected_configuration":rights_configuration
    });
    let before_revoke = invoke_local_rights_command(
        &binary,
        &rights_invocation_path,
        &case.lab.url,
        &rights_describe,
        true,
        false,
    );
    assert_eq!(before_revoke.result["rights"]["rights_allowed"], true);
    let revoke = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"rights.revoke",
        "command_id":"test:pg-native-artifact-rights-revoke",
        "expected_configuration":rights_configuration,
        "expected_rights_version":before_revoke.result["rights"]["rights_version"],
        "reason":"Refuse later native Artifact use after local rights revocation."
    });
    let revoked = invoke_local_rights_command(
        &binary,
        &rights_invocation_path,
        &case.lab.url,
        &revoke,
        true,
        false,
    );
    assert_eq!(revoked.result["current_rights"]["rights_allowed"], false);

    let counts_before_refusal =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    let pg_invocation_path = case._owner_scratch.0.join("revision-invocation.json");
    let refused = invoke_refusal(
        &binary,
        &pg_invocation_path,
        &case.lab.url,
        &json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
    );
    assert_eq!(refused.result["refused"], true);
    assert_eq!(
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table)),
        counts_before_refusal,
        "rights-refused native describe adds no revision records"
    );
    assert_eq!(
        source_commitment(&case),
        source_commitment_before,
        "revocation and refused describe preserve the committed source version"
    );
    assert!(
        matches!(
            case.lab.db.read_current_source_member(
                &case.lab.store,
                &case.cohort,
                &RelativePath::parse(ARTIFACT_TARGET.source_path).unwrap(),
                1_048_576,
                Instant::now() + Duration::from_secs(15),
                &AtomicBool::new(false),
            ),
            Err(tos_command::DurableError::Refused(
                "current source rights revoked"
            ))
        ),
        "revoked source bytes remain unreadable through the coordinator"
    );
    println!(
        "TOS_PG_REAL_NATIVE_ARTIFACT_REVISION {}",
        serde_json::to_string(&json!({
            "record_id":ARTIFACT_TARGET.record_id,
            "source_path":ARTIFACT_TARGET.source_path,
            "owner_family":ARTIFACT_TARGET.owner_schema,
            "revision":revision,
            "rights_revoke":revoked.result,
            "rights_refusal":refused.result
        }))
        .unwrap()
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

#[test]
fn real_pg_selected_work_revision_replays_and_cold_reads_exact_history() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let mut case = bootstrap_work_case(&repository, &software, deadline, &cancelled);
    let binary = native_binary();
    provision_work_revision_extension(&case, &software, &binary);
    let result = record_correction(&mut case, &software, &binary);
    assert_eq!(result["record_id"], WORK_TARGET.record_id);
    assert_eq!(result["source_path"], WORK_TARGET.source_path);
    assert_eq!(result["replayed"], true);
    assert_eq!(
        result["source_read_work"]["historical_source_members_read"], 6,
        "exact Work retry reopens three predecessor and three successor source members"
    );
    assert!(
        result["source_read_work"]["historical_source_bytes_read"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0),
        "Work retry reports bytes read from exact immutable source versions"
    );
    assert_eq!(case.lab.head_seq(), result["committed_head"]);
    let extension_rows = Client::connect(&case.lab.url, NoTls)
        .unwrap()
        .query(
            "SELECT generation,command_id,commit_seq,receipt_digest::text
             FROM cmd2_work_revision_history_v1 WHERE domain=$1
             ORDER BY generation",
            &[&case.lab.domain],
        )
        .unwrap();
    assert_eq!(extension_rows.len(), 1);
    assert_eq!(extension_rows[0].get::<_, i64>(0), 1);
    assert_eq!(
        extension_rows[0].get::<_, String>(1),
        WORK_TARGET.command_id
    );
    assert_eq!(
        extension_rows[0].get::<_, i64>(2),
        result["committed_head"].as_i64().unwrap()
    );
    assert_eq!(extension_rows[0].get::<_, String>(3).len(), 64);
    println!(
        "TOS_PG_REAL_WORK_REVISION {}",
        serde_json::to_string(&json!({
            "record_id":WORK_TARGET.record_id,
            "source_path":WORK_TARGET.source_path,
            "owner_family":"tos_local_corpus_revision_owner_v3",
            "revision":result
        }))
        .unwrap()
    );
}

#[test]
fn real_pg_selected_work_revision_recovers_ready_attempt_and_cold_reads_exact_history() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let mut case = bootstrap_work_case(&repository, &software, deadline, &cancelled);
    let binary = native_binary();
    provision_work_revision_extension(&case, &software, &binary);

    let worker_path = PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").unwrap());
    let invocation_path = case._owner_scratch.0.join("ready-revision-invocation.json");
    invocation(
        &case,
        &software,
        &binary,
        &worker_path,
        &invocation_path,
        &case.owner_config,
        "tos_local_pg_record_revision_invocation_v1",
    );
    let described = invoke(
        &binary,
        &invocation_path,
        &case.lab.url,
        &json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
        false,
    )
    .result;
    let mut fields = json!({"preferred_label":WORK_TARGET.preferred_label.unwrap()});
    if let Some(notes) = WORK_TARGET.notes {
        fields["notes"] = json!(notes);
    }
    let preview_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-revise",
        "fields":fields,
        "forms":WORK_TARGET.form_ids.iter().zip(WORK_TARGET.form_fields).map(|(form_id,field_id)| json!({"form_id":form_id,"field_id":field_id})).collect::<Vec<_>>(),
        "reason":"Isolated PG mechanics fixture only; no source, semantic, rights, or canon admission."
    });
    let preview = invoke(
        &binary,
        &invocation_path,
        &case.lab.url,
        &preview_request,
        false,
    )
    .result;
    assert_eq!(
        preview["owner_configuration"],
        described["owner_configuration"]
    );
    let mut request = preview_request;
    request["operation"] = json!("record.revise");
    request["command_id"] = json!(WORK_TARGET.command_id);
    for (field, prepared) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_dependencies", "expected_dependencies"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
    ] {
        request[field] = preview[prepared].clone();
    }
    request["expected_publication"] = Value::Null;
    let before_head = case.lab.head_seq();

    // Force the first PG apply transaction to fail after the source successor
    // is durably sealed and the attempt reaches `ready`. A retry must recover
    // that exact package and reuse its recorded_at before ordinary preparation.
    let digest = Digest256::of_bytes(case.lab.domain.as_bytes()).to_hex();
    let suffix = &digest[..24];
    let function = format!("tos_test_fail_work_apply_{suffix}");
    let trigger = format!("tos_test_fail_work_apply_{suffix}");
    let sequence = format!("tos_test_fail_work_apply_{suffix}_fired");
    let domain_literal = case.lab.domain.replace('\'', "''");
    let mut interrupt = Client::connect(&case.lab.url, NoTls).unwrap();
    interrupt
        .batch_execute(&format!(
            "CREATE SEQUENCE public.{sequence};
             CREATE FUNCTION public.{function}() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN
               IF NEW.domain = '{domain_literal}' THEN
                 PERFORM nextval('public.{sequence}'::regclass);
                 RAISE EXCEPTION 'intentional ready-work interrupt';
               END IF;
               RETURN NEW;
             END $$;
             CREATE TRIGGER {trigger} BEFORE INSERT OR UPDATE ON cmd2_current
             FOR EACH ROW EXECUTE FUNCTION public.{function}();"
        ))
        .expect("the isolated fixture can interrupt only its own domain apply");
    drop(interrupt);
    let interrupted = invoke_with_expected_exit(
        &binary,
        &invocation_path,
        &case.lab.url,
        &request,
        false,
        false,
        "tos_local_pg_record_revision_result_v1",
        None,
        true,
    );
    let mut cleanup = Client::connect(&case.lab.url, NoTls).unwrap();
    cleanup
        .batch_execute(&format!(
            "DROP TRIGGER {trigger} ON cmd2_current; DROP FUNCTION public.{function}();"
        ))
        .expect("the one-shot fixture interrupt is removed");
    let apply_trigger_fired: bool = cleanup
        .query_one(&format!("SELECT is_called FROM public.{sequence}"), &[])
        .expect("the one-shot fixture witness survives the failed PG transaction")
        .get(0);
    cleanup
        .batch_execute(&format!("DROP SEQUENCE public.{sequence};"))
        .expect("the one-shot fixture witness is removed before retry");
    drop(cleanup);
    assert!(interrupted.result["refused"].as_bool().unwrap());
    assert!(
        apply_trigger_fired,
        "the injected PG apply trigger ran before the command was refused"
    );
    assert_eq!(case.lab.head_seq(), before_head);
    let state: String = Client::connect(&case.lab.url, NoTls)
        .unwrap()
        .query_one(
            "SELECT state FROM cmd2_attempt WHERE domain=$1 AND command_id=$2",
            &[&case.lab.domain, &WORK_TARGET.command_id],
        )
        .expect("the interrupted exact attempt is retained")
        .get(0);
    assert_eq!(state, "ready");

    let retry = invoke(&binary, &invocation_path, &case.lab.url, &request, false).result;
    assert_eq!(retry["replayed"], false);
    let committed_head = case.lab.head_seq();
    assert_eq!(committed_head, before_head + 1);
    assert_eq!(retry["source"]["id"], WORK_TARGET.record_id);
    assert_eq!(retry["source_path"], WORK_TARGET.source_path);

    let committed_replay = invoke(&binary, &invocation_path, &case.lab.url, &request, false).result;
    assert_eq!(committed_replay["replayed"], true);
    assert_eq!(case.lab.head_seq(), committed_head);
    let original_cold = case
        .lab
        .db
        .cold_recover_exact(
            &case.lab.store,
            &case.lab.domain,
            WORK_TARGET.source_path,
            1,
        )
        .expect("the original Work revision remains cold-recoverable");
    assert_eq!(
        case.lab
            .db
            .warm_read_selected(&case.lab.store, &original_cold, 1_048_576)
            .unwrap(),
        case.original_record
    );
    let current_revision: i64 = Client::connect(&case.lab.url, NoTls)
        .unwrap()
        .query_one(
            "SELECT revision FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &WORK_TARGET.source_path],
        )
        .unwrap()
        .get(0);
    let current_cold = case
        .lab
        .db
        .cold_recover_exact(
            &case.lab.store,
            &case.lab.domain,
            WORK_TARGET.source_path,
            current_revision.try_into().unwrap(),
        )
        .expect("the recovered Work successor has an exact cold locator");
    assert_eq!(
        case.lab
            .db
            .warm_read_selected(&case.lab.store, &current_cold, 1_048_576)
            .unwrap(),
        source_member(&mut case, WORK_TARGET.source_path)
    );
    case.lab
        .db
        .cold_verify_cut(&case.lab.store, &case.lab.domain)
        .expect("the recovered ready revision is part of a cold-verified source generation");
}

#[test]
fn real_source_bootstrap_initializes_absent_schema_cold_selects_and_refuses_reinit() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let repository = repository_root();
    let source = source_cut(&repository, 1, deadline, &cancelled);
    let membership = source.cut.stream(source.revision).unwrap().expectation();
    let software = software_fixture(&repository, deadline, &cancelled);
    let binary = native_binary();
    let worker_path = PathBuf::from(
        std::env::var_os("TOS_SCHEMA_WORKER_PATH")
            .expect("the exact selected schema worker is supplied by the laboratory"),
    )
    .canonicalize()
    .expect("the selected schema worker exists");
    assert!(worker_path.is_absolute());

    let base_url = database_url();
    let schema_nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let pg_schema = format!("tos_bootstrap_{}_{}", std::process::id(), schema_nonce);
    let mut admin = Client::connect(&base_url, NoTls).expect("selected private PostgreSQL opens");
    admin
        .batch_execute(&format!("CREATE SCHEMA \"{pg_schema}\""))
        .expect("unique private test schema is created");
    let separator = if base_url.contains('?') { "&" } else { "?" };
    let selected_url =
        if base_url.starts_with("postgres://") || base_url.starts_with("postgresql://") {
            format!("{base_url}{separator}options=-csearch_path%3D{pg_schema}%2Cpublic")
        } else {
            format!("{base_url} options='-csearch_path={pg_schema},public'")
        };
    {
        let mut selected =
            Client::connect(&selected_url, NoTls).expect("unique private test schema is selected");
        let current_schema: String = selected
            .query_one("SELECT current_schema()", &[])
            .unwrap()
            .get(0);
        assert_eq!(current_schema, pg_schema);
        let absent: bool = selected
            .query_one(
                "SELECT to_regclass(format('%I.cmd2_domain', $1::text)) IS NULL",
                &[&pg_schema],
            )
            .unwrap()
            .get(0);
        assert!(absent, "the fresh schema has no CMD.2 domain");
    }

    let store_scratch = ScratchRoot::new();
    fs::set_permissions(&store_scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let store_root = store_scratch.0.canonicalize().unwrap();
    let owner_scratch = ScratchRoot::new();
    fs::set_permissions(&owner_scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let owner_root = owner_scratch.0.canonicalize().unwrap();
    let owner_config = owner_root.join("bootstrap-owner.json");
    let invocation_path = owner_root.join("bootstrap-invocation.json");
    let domain = format!("tos-bootstrap-{}-{schema_nonce}", std::process::id());
    let job_id = "private-bootstrap-job";
    let contract = contract_digest();
    let uid = rustix::process::getuid().as_raw();
    let owner = json!({
        "schema_version":"tos_local_pg_source_bootstrap_owner_v1",
        "uid":uid,
        "expires_at":"2099-01-01T00:00:00Z",
        "source_root":store_root,
        "principal_id":"test:private-native-source-bootstrap",
        "authority_ref":"test:one-record-bootstrap-proof",
        "allowed_operations":["source.bootstrap"],
        "domain":domain,
        "source_contract_sha256":contract.to_prefixed(),
        "source_revision_sha256":source.revision.0.to_prefixed(),
        "source_membership_sha256":membership.digest.to_prefixed(),
        "source_membership_count":membership.count,
        "initialization_mode":"initialize",
        "job_id":job_id,
        "job_fence":1,
        "initialize_schema":true
    });
    fs::write(&owner_config, canonical(&owner)).unwrap();
    fs::set_permissions(&owner_config, fs::Permissions::from_mode(0o600)).unwrap();

    let store_limits = limits();
    let invocation = json!({
        "schema_version":"tos_local_pg_source_bootstrap_invocation_v1",
        "owner_config":owner_config,
        "native_executable":binary,
        "native_executable_sha256":Digest256::of_bytes(&fs::read(&binary).unwrap()).to_prefixed(),
        "source_selection":{
            "domain":domain,
            "source_contract_sha256":contract.to_prefixed(),
            "store_root":store_root,
            "store_limits":{
                "max_segment_bytes":store_limits.max_segment_bytes,
                "max_frame_bytes":store_limits.max_frame_bytes,
                "max_frames":store_limits.max_frames,
                "max_journal_bytes":store_limits.max_journal_bytes
            },
            "job_id":job_id,
            "job_fence":1,
            "initialization_mode":"initialize",
            "source_revision_sha256":source.revision.0.to_prefixed(),
            "source_membership_sha256":membership.digest.to_prefixed(),
            "source_membership_count":membership.count,
            "initialize_schema":true
        },
        "corpus_store":source._scratch.0.join("snapshot").canonicalize().unwrap(),
        "source_revision":source.revision.0.to_prefixed(),
        "software_capture":software.capture.canonicalize().unwrap(),
        "software_restored_root":software.restored.canonicalize().unwrap(),
        "software_selection":{
            "source_git_commit":software.selection.source_git_commit,
            "source_git_tree":software.selection.source_git_tree,
            "capture_manifest_sha256":software.selection.capture_manifest_sha256.to_prefixed()
        },
        "software_components":software.components,
        "schema_worker":{
            "absolute_path":worker_path,
            "sha256":Digest256::of_bytes(&fs::read(&worker_path).unwrap()).to_prefixed()
        },
        "budgets":{
            "max_revisions":1,
            "max_members":512,
            "max_total_bytes":8*1024*1024,
            "max_member_bytes":1_048_576,
            "max_schema_receipts":128,
            "max_schema_receipt_bytes":262_144,
            "worker_cpu_seconds":3,
            "worker_address_space_bytes":1_073_741_824
        }
    });
    fs::write(&invocation_path, canonical(&invocation)).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"source.bootstrap"
    });

    let completed =
        invoke_source_bootstrap(&binary, &invocation_path, &selected_url, &request, true).result;
    assert_eq!(completed["source_complete"], true);
    assert_eq!(completed["schema_initialized"], true);
    assert_eq!(completed["grants_admission"], false);
    assert_eq!(completed["grants_publication"], false);
    assert_eq!(
        completed["source_revision_sha256"],
        source.revision.0.to_prefixed()
    );
    assert_eq!(
        completed["source_membership_sha256"],
        membership.digest.to_prefixed()
    );
    assert_eq!(
        completed["current_membership_sha256"],
        membership.digest.to_prefixed()
    );
    assert_eq!(completed["source_membership_count"], membership.count);
    assert_eq!(completed["current_membership_count"], membership.count);
    assert!(completed["selected_head_seq"].as_u64().unwrap() > 0);
    assert!(completed["selected_generation_sha256"].as_str().is_some());
    assert!(completed["selected_state_sha256"].as_str().is_some());

    let state_snapshot = || {
        let mut db = DurablePgCoordinator::connect(&selected_url).unwrap();
        let store =
            tos_segment_store::SegmentStore::open_existing(&store_root, store_limits).unwrap();
        let selected = db
            .cold_open_selected_generation(&store, &domain, deadline, &cancelled)
            .unwrap();
        let mut pg = Client::connect(&selected_url, NoTls).unwrap();
        let row = pg
            .query_one(
                "SELECT d.head_seq, d.source_complete,
                        d.selected_generation_digest::text, d.source_revision::text,
                        d.source_membership_digest::text, d.source_membership_count,
                        (SELECT count(*) FROM cmd2_current c WHERE c.domain=d.domain),
                        (SELECT count(*) FROM cmd2_member m WHERE m.domain=d.domain),
                        (SELECT count(*) FROM cmd2_log l WHERE l.domain=d.domain),
                        (SELECT count(*) FROM cmd2_outbox o WHERE o.domain=d.domain)
                 FROM cmd2_domain d WHERE d.domain=$1",
                &[&domain],
            )
            .unwrap();
        json!({
            "head_seq": row.get::<_, i64>(0),
            "source_complete": row.get::<_, bool>(1),
            "selected_generation_digest": row.get::<_, Option<String>>(2),
            "source_revision": row.get::<_, Option<String>>(3),
            "source_membership_digest": row.get::<_, Option<String>>(4),
            "source_membership_count": row.get::<_, Option<i64>>(5),
            "current_rows": row.get::<_, i64>(6),
            "member_rows": row.get::<_, i64>(7),
            "log_rows": row.get::<_, i64>(8),
            "outbox_rows": row.get::<_, i64>(9),
            "cold_generation_digest":selected.digest().to_prefixed(),
            "cold_state_digest": selected.cut().state_digest().to_prefixed(),
            "cold_head_seq": selected.cut().through_commit_seq()
        })
    };
    let valid_state = state_snapshot();
    let selected_generation_hex = completed["selected_generation_sha256"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .expect("bootstrap returns its cold selected generation identity");
    assert_eq!(valid_state["source_complete"], true);
    assert_eq!(
        valid_state["selected_generation_digest"],
        selected_generation_hex
    );
    assert_eq!(
        valid_state["cold_generation_digest"],
        completed["selected_generation_sha256"]
    );
    assert_eq!(
        valid_state["cold_state_digest"],
        completed["selected_state_sha256"]
    );
    assert_eq!(valid_state["cold_head_seq"], completed["selected_head_seq"]);

    let refused =
        invoke_source_bootstrap(&binary, &invocation_path, &selected_url, &request, false);
    assert_eq!(refused.result["refused"], true);
    assert_eq!(state_snapshot(), valid_state);
    println!(
        "TOS_PG_SOURCE_BOOTSTRAP {}",
        serde_json::to_string(&json!({
            "source_revision_sha256": source.revision.0.to_prefixed(),
            "source_membership_sha256": membership.digest.to_prefixed(),
            "source_membership_count": membership.count,
            "cold_generation_sha256": completed["selected_generation_sha256"],
            "cold_state_sha256": completed["selected_state_sha256"],
            "cold_head_seq": completed["selected_head_seq"],
            "existing_schema_reinitialization_refused": true,
            "state_unchanged_after_refusal": true,
            "grants_admission": false,
            "grants_publication": false
        }))
        .unwrap()
    );

    admin
        .batch_execute(&format!("DROP SCHEMA \"{pg_schema}\" CASCADE"))
        .expect("private bootstrap test schema is removed");
}

fn recovery_database_state(url: &str) -> (String, i64) {
    let config = url
        .parse::<postgres::Config>()
        .expect("private PG URL parses");
    assert_eq!(
        config.get_hosts().len(),
        1,
        "one explicitly selected PG host"
    );
    assert!(config.get_ports().len() <= 1);
    assert!(config.get_hostaddrs().is_empty());
    assert_eq!(
        config.get_target_session_attrs(),
        postgres::config::TargetSessionAttrs::Any
    );
    assert!(matches!(
        config.get_ssl_mode(),
        postgres::config::SslMode::Disable | postgres::config::SslMode::Prefer
    ));
    assert!(matches!(
        &config.get_hosts()[0],
        postgres::config::Host::Tcp(host) if host == "127.0.0.1" || host == "::1"
    ));
    assert!(config.get_user().is_some());
    assert!(config.get_dbname().is_some_and(|name| !name.is_empty()));
    let mut db = Client::connect(url, NoTls).expect("dedicated recovery database connects");
    let server_version = db
        .query_one("SHOW server_version_num", &[])
        .unwrap()
        .get::<_, String>(0)
        .parse::<u32>()
        .expect("PostgreSQL reports its numeric server version");
    assert_eq!(
        server_version / 10_000,
        16,
        "recovery is pinned to PostgreSQL 16"
    );
    let database = db
        .query_one("SELECT current_database()", &[])
        .unwrap()
        .get::<_, String>(0);
    let public_tables = db
        .query_one(
            "SELECT count(*) FROM pg_catalog.pg_tables WHERE schemaname='public'",
            &[],
        )
        .unwrap()
        .get::<_, i64>(0);
    (database, public_tables)
}

fn recovery_pg_endpoint(url: &str) -> (String, u16) {
    let config = url
        .parse::<postgres::Config>()
        .expect("private PG URL parses");
    let postgres::config::Host::Tcp(host) = &config.get_hosts()[0] else {
        panic!("private recovery database uses a loopback TCP host");
    };
    (
        host.clone(),
        config.get_ports().first().copied().unwrap_or(5432),
    )
}

fn new_real_backup_source_lab(url: &str) -> Lab {
    let mut db = DurablePgCoordinator::connect(url).expect("fresh source database connects");
    db.init_lab_schema()
        .expect("the explicitly selected fresh source database initializes CMD.2");
    let domain = super::unique_domain();
    db.create_domain(&domain, contract_digest())
        .expect("private recovery domain is created");
    db.set_job_epoch(&domain, "private-job", 1)
        .expect("private recovery job fence is initialized");
    let root = ScratchRoot::new();
    fs::set_permissions(&root.0, fs::Permissions::from_mode(0o700)).unwrap();
    let store =
        tos_segment_store::SegmentStore::initialize_empty(&root.0, domain.as_bytes(), limits())
            .expect("private recovery source store initializes");
    Lab {
        db,
        store,
        _root: root,
        domain,
        url: url.to_owned(),
        attempt_fences: std::collections::HashMap::new(),
    }
}

fn json_rows(rows: Vec<postgres::Row>) -> Vec<Value> {
    rows.into_iter()
        .map(|row| serde_json::from_str(&row.get::<_, String>(0)).unwrap())
        .collect()
}

fn real_recovery_metadata_snapshot(case: &RevisionCase) -> Value {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    db.batch_execute("SET TIME ZONE 'UTC'")
        .expect("recovery snapshot timestamps use the backup census timezone");
    let domain = db
        .query_one(
            "SELECT row_to_json(d)::text FROM cmd2_domain d WHERE domain=$1",
            &[&case.lab.domain],
        )
        .unwrap()
        .get::<_, String>(0);
    let current = db
        .query_one(
            "SELECT row_to_json(c)::text FROM cmd2_current c
             WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &case.target.source_path],
        )
        .unwrap()
        .get::<_, String>(0);
    let history = json_rows(
        db.query(
            "SELECT row_to_json(h)::text FROM cmd2_history h
             WHERE domain=$1 AND subject=$2 ORDER BY revision",
            &[&case.lab.domain, &case.target.source_path],
        )
        .unwrap(),
    );
    let receipts = json_rows(
        db.query(
            "SELECT row_to_json(r)::text FROM cmd2_receipt r
             WHERE domain=$1 ORDER BY commit_seq",
            &[&case.lab.domain],
        )
        .unwrap(),
    );
    let log = json_rows(
        db.query(
            "SELECT row_to_json(l)::text FROM cmd2_log l
             WHERE domain=$1 ORDER BY commit_seq",
            &[&case.lab.domain],
        )
        .unwrap(),
    );
    let outbox = json_rows(
        db.query(
            "SELECT row_to_json(o)::text FROM cmd2_outbox o
             WHERE domain=$1 ORDER BY commit_seq",
            &[&case.lab.domain],
        )
        .unwrap(),
    );
    let navigation_policy_current = json_rows(
        db.query(
            "SELECT row_to_json(p)::text FROM cmd2_source_navigation_policy_current p
             WHERE domain=$1 ORDER BY consumer_id COLLATE \"C\"",
            &[&case.lab.domain],
        )
        .unwrap(),
    );
    let navigation_policy_history = json_rows(
        db.query(
            "SELECT row_to_json(p)::text FROM cmd2_source_navigation_policy_history p
             WHERE domain=$1 ORDER BY consumer_id COLLATE \"C\",policy_version",
            &[&case.lab.domain],
        )
        .unwrap(),
    );
    json!({
        "domain":serde_json::from_str::<Value>(&domain).unwrap(),
        "current_record":serde_json::from_str::<Value>(&current).unwrap(),
        "record_history":history,
        "receipts":receipts,
        "log":log,
        "outbox":outbox,
        "source_navigation_policy_current":navigation_policy_current,
        "source_navigation_policy_history":navigation_policy_history
    })
}

fn real_projection_snapshot(case: &RevisionCase, consumer: &str) -> Value {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    let consumer_row = db
        .query_opt(
            "SELECT row_to_json(c)::text FROM cmd2_projection_consumer c
             WHERE domain=$1 AND consumer_id=$2",
            &[&case.lab.domain, &consumer],
        )
        .unwrap()
        .expect("the projection consumer exists before backup")
        .get::<_, String>(0);
    let document_rows = db
        .query(
            "SELECT row_to_json(p)::text FROM cmd2_projection_document p
             WHERE domain=$1 AND consumer_id=$2
             ORDER BY source_path COLLATE \"C\",document_id COLLATE \"C\"",
            &[&case.lab.domain, &consumer],
        )
        .unwrap()
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    let documents = projection_documents(&document_rows)
        .into_values()
        .collect::<Vec<_>>();
    let dependencies = projection_dependencies(case, consumer)
        .into_iter()
        .flat_map(|((source_path, document_id), paths)| {
            paths.into_iter().map(move |dependency_path| {
                json!({
                    "source_path":source_path,
                    "document_id":document_id,
                    "dependency_kind":"source",
                    "dependency_path":dependency_path
                })
            })
        })
        .collect::<Vec<_>>();
    json!({
        "consumer":serde_json::from_str::<Value>(&consumer_row).unwrap(),
        "documents":documents,
        "dependencies":dependencies
    })
}

fn project_real_source_to_current(case: &mut RevisionCase, consumer: &str) -> Value {
    let profile = DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
        version: 1,
        source_contract: contract_digest(),
    };
    case.lab.db.init_projection_schema().unwrap();
    let head = case.lab.head_seq();
    let max_events = usize::try_from(head).expect("small private source event head");
    assert!((1..=256).contains(&max_events));
    let progress = case
        .lab
        .db
        .apply_projection_batch(
            &case.lab.store,
            &case.lab.domain,
            consumer,
            &profile,
            &tos_command::source_projection_cli::SourceTextExtractor,
            DurableProjectionLimits {
                max_events,
                ..DurableProjectionLimits::default()
            },
        )
        .expect("the real bounded source history projects through its current head");
    assert!(matches!(
        &progress.status,
        DurableProjectionStatus::Current { through_seq } if *through_seq == head
    ));
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, consumer, &profile, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Current { through_seq } if through_seq == head
    ));
    serde_json::to_value(progress).unwrap()
}

fn real_projection_search(case: &mut RevisionCase, consumer: &str) -> Value {
    let profile = DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
        version: 1,
        source_contract: contract_digest(),
    };
    let result = case
        .lab
        .db
        .search_projection(
            &case.lab.domain,
            consumer,
            &profile,
            "Quartz",
            10,
            Duration::from_secs(15),
        )
        .expect("restored source-text projection search succeeds");
    let DurableProjectionSearch::Current { hits, through_seq } = result else {
        panic!("the projected source search is current through the requested event");
    };
    assert_eq!(through_seq, case.lab.head_seq());
    assert!(hits.iter().any(|hit| {
        hit.source_path == case.target.source_path && hit.document_id == case.target.record_id
    }));
    serde_json::to_value(json!({"through_seq":through_seq,"hits":hits})).unwrap()
}

fn current_custody_revision(case: &RevisionCase, source_path: &str) -> u64 {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    u64::try_from(
        db.query_one(
            "SELECT revision FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&case.lab.domain, &source_path],
        )
        .unwrap()
        .get::<_, i64>(0),
    )
    .unwrap()
}

fn cold_exact_source_bytes(case: &mut RevisionCase, source_path: &str, revision: u64) -> Vec<u8> {
    let retained = case
        .lab
        .db
        .cold_recover_exact(&case.lab.store, &case.lab.domain, source_path, revision)
        .expect("exact selected source custody revision remains cold-recoverable");
    case.lab
        .db
        .warm_read_selected(&case.lab.store, &retained, 1_048_576)
        .unwrap()
}

fn cold_recovery_cut(case: &mut RevisionCase) -> Value {
    let cut = case
        .lab
        .db
        .cold_verify_cut(&case.lab.store, &case.lab.domain)
        .expect(
            "restored or continued PG/STO domain passes cold metadata and custody verification",
        );
    json!({
        "log_digest":cut.log_digest().to_hex(),
        "through_commit_seq":cut.through_commit_seq(),
        "historical_members":cut.historical_members()
    })
}

fn run_recovery_cli(binary: &Path, operation: &str, args: &[String], pg_url: &str) -> Value {
    let output = Command::new(binary)
        .arg(operation)
        .args(args)
        .env("TOS_BACKUP_PG_URL", pg_url)
        .output()
        .expect("the pinned native backup/restore owner command starts");
    assert!(
        output.stdout.len() <= 1_048_576 && output.stderr.len() <= 1_048_576,
        "native recovery owner output remains bounded"
    );
    assert!(
        output.status.success(),
        "native {operation} refuses: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .expect("the native recovery owner returns one JSON result")
}

fn recovery_tool_pin(path_name: &str, digest_name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(
        std::env::var_os(path_name).unwrap_or_else(|| panic!("{path_name} is required")),
    );
    assert!(
        path.is_absolute(),
        "{path_name} is an absolute executable path"
    );
    let digest = std::env::var(digest_name).unwrap_or_else(|_| panic!("{digest_name} is required"));
    let observed = Digest256::of_bytes(&fs::read(&path).expect("selected PG tool reads")).to_hex();
    assert_eq!(
        digest, observed,
        "{digest_name} pins the exact owner-selected PG16 tool bytes"
    );
    (path, digest)
}

fn base_recovery_args(
    operation: &str,
    case: &RevisionCase,
    backup_root: &Path,
    store_root: &Path,
    tool_path: &Path,
    tool_digest: &str,
) -> Vec<String> {
    let store_limits = limits();
    let mut args = vec![
        "--domain".to_owned(),
        case.lab.domain.clone(),
        "--store-root".to_owned(),
        store_root.display().to_string(),
        "--backup-root".to_owned(),
        backup_root.display().to_string(),
        "--pg-tool".to_owned(),
        tool_path.display().to_string(),
        "--pg-tool-sha256".to_owned(),
        tool_digest.to_owned(),
        "--max-segment-bytes".to_owned(),
        store_limits.max_segment_bytes.to_string(),
        "--max-frame-bytes".to_owned(),
        store_limits.max_frame_bytes.to_string(),
        "--max-frames".to_owned(),
        store_limits.max_frames.to_string(),
        "--max-journal-bytes".to_owned(),
        store_limits.max_journal_bytes.to_string(),
        "--max-seconds".to_owned(),
        "240".to_owned(),
    ];
    if operation == "backup" {
        args.extend(["--confirm-quiescent-owner".to_owned(), "yes".to_owned()]);
    } else {
        args.extend(["--confirm-fresh-target-owner".to_owned(), "yes".to_owned()]);
    }
    args
}

fn retarget_revision_owner(case: &mut RevisionCase, store_root: &Path) {
    let mut owner: Value = serde_json::from_slice(&fs::read(&case.owner_config).unwrap()).unwrap();
    assert_eq!(
        owner["schema_version"],
        "tos_local_corpus_revision_owner_v1"
    );
    let original_source_root = case.source_root.to_string_lossy().into_owned();
    assert_eq!(owner["source_root"], original_source_root);
    let mut expected = owner.clone();
    let restored_source_root = store_root.canonicalize().unwrap();
    expected["source_root"] = json!(restored_source_root);
    owner["source_root"] = expected["source_root"].clone();
    assert_eq!(
        owner, expected,
        "restoration changes only the bound private store path"
    );
    fs::write(&case.owner_config, canonical(&owner)).unwrap();
    fs::set_permissions(&case.owner_config, fs::Permissions::from_mode(0o600)).unwrap();
    let metadata = fs::metadata(&case.owner_config).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o600);
    case.source_root = restored_source_root;
}

fn continue_native_revision_after_restore(
    case: &mut RevisionCase,
    software: &SoftwareFixture,
    binary: &Path,
    previous_current: &[u8],
    prior_projection_head: u64,
    prior_domain: &Value,
    prior_projection: &Value,
) -> Value {
    let target = case.target;
    let worker_path = PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").unwrap());
    let invocation_path = case
        ._owner_scratch
        .0
        .join("restored-revision-invocation.json");
    invocation(
        case,
        software,
        binary,
        &worker_path,
        &invocation_path,
        &case.owner_config,
        "tos_local_pg_record_revision_invocation_v1",
    );
    let described = invoke(
        binary,
        &invocation_path,
        &case.lab.url,
        &json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
        false,
    )
    .result;
    assert_eq!(described["source"]["id"], target.record_id);
    assert_eq!(
        described["owner_configuration"],
        owner_configuration_digest(&case.owner_config)
    );
    let label = "Парменид Элейский Restored";
    let preview_request = json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-revise",
        "fields":{"preferred_label":label},
        "forms":target.form_ids.iter().zip(target.form_fields)
            .map(|(form_id,field_id)|json!({"form_id":form_id,"field_id":field_id}))
            .collect::<Vec<_>>(),
        "reason":"Bounded native continuation after private PG/STO restoration; no source, semantic, rights, or canon admission."
    });
    let preview = invoke(
        binary,
        &invocation_path,
        &case.lab.url,
        &preview_request,
        false,
    )
    .result;
    assert_eq!(
        preview["owner_configuration"],
        described["owner_configuration"]
    );
    let mut request = preview_request;
    request["operation"] = json!("record.revise");
    request["command_id"] = json!(format!(
        "test:pg-parmenides-post-restore-correction:{}",
        case.lab.domain
    ));
    for (field, prepared) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_dependencies", "expected_dependencies"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
    ] {
        request[field] = preview[prepared].clone();
    }
    let restored_head = case.lab.head_seq();
    assert_eq!(restored_head, prior_projection_head);
    let previous_custody_revision = current_custody_revision(case, target.source_path);
    assert_eq!(previous_custody_revision, 2);
    let current_before = source_member(case, target.source_path);
    assert_eq!(current_before, previous_current);
    let before_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    let committed = invoke(binary, &invocation_path, &case.lab.url, &request, true);
    let committed_head = case.lab.head_seq();
    assert_eq!(committed_head, restored_head + 1);
    let committed_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table));
    assert_eq!(committed_counts[4], before_counts[4] + 1);
    let retry = invoke(binary, &invocation_path, &case.lab.url, &request, false);
    assert_eq!(retry.result["replayed"], true);
    assert_eq!(case.lab.head_seq(), committed_head);
    assert_eq!(
        ["current", "history", "receipt", "log", "outbox"].map(|table| case.lab.count(table)),
        committed_counts,
        "the exact post-restore retry adds no second commit"
    );
    let current = source_member(case, target.source_path);
    let mut expected: Value = serde_json::from_slice(&current_before).unwrap();
    expected["preferred_label"] = json!(label);
    expected["record_version"] = json!(3);
    assert_eq!(
        serde_json::from_slice::<Value>(&current).unwrap(),
        expected,
        "the second owner-checked native revision continues authored record version three"
    );
    assert_eq!(
        current_custody_revision(case, target.source_path),
        previous_custody_revision + 1
    );
    let forms: Value = serde_json::from_slice(&source_member(case, target.forms_path)).unwrap();
    assert_eq!(forms["forms"].as_array().unwrap().len(), 3);
    assert!(
        forms["forms"]
            .as_array()
            .unwrap()
            .iter()
            .all(|form| form["form_version"] == 3)
    );
    let changed_paths = history_source_paths(case, committed_head);
    for path in [target.source_path, target.forms_path, target.history_path] {
        assert!(changed_paths.iter().any(|changed| changed == path));
    }
    let new_cut = cold_recovery_cut(case);
    assert_eq!(new_cut["through_commit_seq"], committed_head);
    assert_ne!(new_cut["through_commit_seq"], prior_projection_head);
    assert_eq!(
        cold_exact_source_bytes(case, target.source_path, previous_custody_revision),
        current_before
    );
    assert_eq!(
        cold_exact_source_bytes(case, target.source_path, 1),
        case.original_record
    );
    assert_eq!(
        cold_exact_source_bytes(case, target.source_path, previous_custody_revision + 1),
        current
    );
    let domain_after = real_recovery_metadata_snapshot(case)["domain"].clone();
    for key in [
        "rights_version",
        "rights_allowed",
        "rule_version",
        "contract_digest",
        "source_revision",
        "source_membership_digest",
        "source_membership_count",
        "source_epoch",
        "source_complete",
    ] {
        assert_eq!(
            domain_after[key], prior_domain[key],
            "restored source/policy field {key} remains stable during continuation"
        );
    }
    assert_eq!(
        domain_after["source_generation"], committed_head,
        "the native revision advances the selected source generation to its commit sequence"
    );
    for key in [
        "selected_generation_digest",
        "complete_cut_digest",
        "complete_cut_generation",
        "source_projection_digest",
    ] {
        assert_eq!(
            domain_after[key],
            Value::Null,
            "the earlier selected generation is invalidated by source revision: {key}"
        );
    }
    let projection_profile = DurableProjectionProfile {
        id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
        version: 1,
        source_contract: contract_digest(),
    };
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, "private-pg-real-backup-restore", &projection_profile, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == prior_projection_head && head_seq == committed_head
    ));
    assert_eq!(
        real_projection_snapshot(case, "private-pg-real-backup-restore"),
        *prior_projection,
        "backup projection rows remain at their verified pre-continuation cursor"
    );
    json!({
        "before_head":restored_head,
        "committed_head":committed_head,
        "before_counts":before_counts,
        "committed_counts":committed_counts,
        "source_path":target.source_path,
        "previous_custody_revision":previous_custody_revision,
        "current_custody_revision":previous_custody_revision+1,
        "current_record_sha256":Digest256::of_bytes(&current).to_hex(),
        "retained_previous_record_sha256":Digest256::of_bytes(&current_before).to_hex(),
        "native_invocations":{
            "describe":described,
            "reply_dropped":committed.resources,
            "same_command_retry":retry.resources
        },
        "replayed":retry.result["replayed"],
        "projection_status_after_commit":"pending at the restored pre-write cursor",
        "cold_cut_after_commit":new_cut
    })
}

fn private_package_schema_guard_catalog_truncation_roundtrip(
    db: &mut DurablePgCoordinator,
    url: &str,
) -> Value {
    const CATALOG_NAME: &str = "cmd2_private_refuse_cmd2_private_package_archive_member_truncat";
    const GENERATED_NAME: &str = "cmd2_private_refuse_cmd2_private_package_archive_member_truncate";
    assert_eq!(CATALOG_NAME.len(), 63);
    assert_eq!(GENERATED_NAME.len(), 64);
    db.require_compatible_private_package_schema()
        .expect("fresh private package schema accepts the PostgreSQL catalog spelling");

    let mut catalog =
        Client::connect(url, NoTls).expect("private package catalog connection opens");
    let trigger_names: Vec<String> = catalog
        .query(
            "SELECT tgname::text FROM pg_trigger WHERE tgrelid = 'cmd2_private_package_archive_member'::regclass AND NOT tgisinternal",
            &[],
        )
        .expect("private archive member triggers are readable")
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    assert!(trigger_names.iter().any(|name| name == CATALOG_NAME));
    assert!(!trigger_names.iter().any(|name| name == GENERATED_NAME));

    catalog
        .batch_execute(
            "DROP TRIGGER cmd2_private_refuse_cmd2_private_package_archive_member_truncat ON cmd2_private_package_archive_member",
        )
        .expect("the exact archive-member truncate guard is removed");
    assert!(
        db.require_compatible_private_package_schema().is_err(),
        "schema compatibility refuses when the exact archive-member truncate guard is absent"
    );

    // This function runs only on the recovery test's dedicated fresh source database.
    db.init_lab_schema()
        .expect("the existing schema installer restores the guard after the refusal check");
    db.require_compatible_private_package_schema()
        .expect("the installed guard is restored before backup");
    json!({
        "catalog_trigger_name":CATALOG_NAME,
        "generated_trigger_name_bytes":GENERATED_NAME.len(),
        "catalog_trigger_name_bytes":CATALOG_NAME.len(),
        "fresh_schema_accepted":true,
        "missing_guard_refused":true,
        "guard_restored_before_backup":true
    })
}

/// Runs the recovery check on dedicated explicit databases so the shared
/// postgres_durable_lab URL and its test fixtures remain untouched.
pub(super) fn run_parmenides_backup_restore_regression() -> Value {
    let source_url = std::env::var("TOS_CMD2_REAL_SOURCE_BACKUP_PG_URL")
        .expect("TOS_CMD2_REAL_SOURCE_BACKUP_PG_URL selects a fresh dedicated PG16 database");
    let restore_url = std::env::var("TOS_CMD2_V2_RESTORE_PG_URL")
        .expect("TOS_CMD2_V2_RESTORE_PG_URL selects a distinct fresh restore database");
    let (source_database, source_tables) = recovery_database_state(&source_url);
    let (restore_database, restore_tables) = recovery_database_state(&restore_url);
    let shared_url = database_url();
    let (shared_database, _) = recovery_database_state(&shared_url);
    assert_ne!(
        source_database, restore_database,
        "backup and restore use distinct database names"
    );
    assert_ne!(
        (recovery_pg_endpoint(&source_url), source_database.clone()),
        (recovery_pg_endpoint(&shared_url), shared_database.clone()),
        "the backup source never reuses the shared postgres_durable_lab database"
    );
    assert_ne!(
        (recovery_pg_endpoint(&restore_url), restore_database.clone()),
        (recovery_pg_endpoint(&shared_url), shared_database),
        "the restore target never reuses the shared postgres_durable_lab database"
    );
    assert_eq!(
        source_tables, 0,
        "the dedicated source database is empty before CMD.2 initialization"
    );
    assert_eq!(
        restore_tables, 0,
        "the restore target has no public tables before restore"
    );
    let (dump_path, dump_sha) =
        recovery_tool_pin("TOS_CMD2_PG_DUMP_PATH", "TOS_CMD2_PG_DUMP_SHA256");
    let (restore_path, restore_sha) =
        recovery_tool_pin("TOS_CMD2_PG_RESTORE_PATH", "TOS_CMD2_PG_RESTORE_SHA256");

    let deadline = Instant::now() + Duration::from_secs(600);
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let repository = repository_root();
    let software = software_fixture(&repository, deadline, &cancelled);
    let source = source_cut(&repository, 1, deadline, &cancelled);
    let mut source_lab = new_real_backup_source_lab(&source_url);
    let private_package_schema_guard =
        private_package_schema_guard_catalog_truncation_roundtrip(&mut source_lab.db, &source_url);
    let mut case = bootstrap_case_on_lab(
        source_lab,
        &software,
        source,
        PARMENIDES_TARGET,
        deadline,
        &cancelled,
    );
    let binary = native_binary();
    let consumer = "private-pg-real-backup-restore";
    let projection_work = project_real_source_to_current(&mut case, consumer);
    let projection_before_revision = real_projection_snapshot(&case, consumer);
    let projection_head = case.lab.head_seq();
    let first_revision = record_correction(&mut case, &software, &binary);
    let pending_head = case.lab.head_seq();
    assert_eq!(first_revision["committed_head"], json!(pending_head));
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, consumer, &DurableProjectionProfile {
                id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
                version: 1,
                source_contract: contract_digest(),
            }, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == projection_head && head_seq == pending_head
    ));
    assert_eq!(
        real_projection_snapshot(&case, consumer),
        projection_before_revision,
        "the committed correction is pending while the previous projection rows and cursor remain exact"
    );
    let projection_rows_before_interrupt = projection_rows(&case, consumer);
    let interrupted_projection_process = kill_projection_child(&case, consumer);
    assert_eq!(
        projection_rows(&case, consumer),
        projection_rows_before_interrupt,
        "killing the real projector during event staging leaves rows unchanged before backup"
    );
    case.lab.db = DurablePgCoordinator::connect(&case.lab.url)
        .expect("source database reopens after the interrupted projection process");
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, consumer, &DurableProjectionProfile {
                id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
                version: 1,
                source_contract: contract_digest(),
            }, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == projection_head && head_seq == pending_head
    ));
    assert_eq!(
        real_projection_snapshot(&case, consumer),
        projection_before_revision
    );
    let source_path = case.target.source_path;
    let source_current = source_member(&mut case, source_path);
    let current_custody = current_custody_revision(&case, source_path);
    assert_eq!(current_custody, 2);
    let retained_v1 = cold_exact_source_bytes(&mut case, source_path, 1);
    assert_eq!(retained_v1, case.original_record);
    let source_cut_before = cold_recovery_cut(&mut case);
    assert_eq!(source_cut_before["through_commit_seq"], case.lab.head_seq());
    let metadata_before = real_recovery_metadata_snapshot(&case);
    // The ordinary revision path need not create a temporal overlay. Reuse
    // the explicitly labeled storage fixture derived from exact current,
    // history, index, definition, and head-log projection bytes. This covers
    // V9 backup/restore preservation, not overlay-producer behavior.
    let source_inventory_before =
        super::ensure_source_inventory_projection_fixture(&case.lab.url, &case.lab.domain);

    let backup_scratch = ScratchRoot::new();
    fs::set_permissions(&backup_scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let backup_root = backup_scratch.0.canonicalize().unwrap();
    let backup_args = base_recovery_args(
        "backup",
        &case,
        &backup_root,
        &case.source_root,
        &dump_path,
        &dump_sha,
    );
    let backup = run_recovery_cli(&binary, "backup", &backup_args, &source_url);
    assert_eq!(backup["status"], "backup_complete");
    assert_eq!(backup["receipt"]["schema"], "tos_cmd2_quiescent_backup_v11");
    assert_eq!(backup["receipt"]["source_admission"], false);
    assert_eq!(backup["receipt"]["production_quiescence_verified"], false);
    let inventory_census = &backup["receipt"]["metadata"]["source_inventory_projection_v1"];
    assert_eq!(
        inventory_census["schema"],
        "tos_backup_source_inventory_projection_v1"
    );
    assert_eq!(
        backup["receipt"]["metadata"]["counts"]["cmd2_source_inventory_projection"],
        json!(source_inventory_before.len())
    );
    assert_eq!(
        inventory_census["rows"],
        json!(source_inventory_before.len())
    );
    assert_eq!(
        inventory_census["bytes"],
        json!(source_inventory_projection_bytes(&source_inventory_before))
    );
    assert!(
        inventory_census["row_root"]
            .as_str()
            .and_then(|root| Digest256::from_hex(root).ok())
            .is_some(),
        "the V9 backup records a digest for all retained inventory overlays"
    );
    assert_eq!(
        inventory_census["row_root"],
        source_inventory_projection_root(&source_inventory_before),
        "the receipt root commits the complete deterministically ordered source row set"
    );
    let receipt_path = backup_root.join("receipt.json");
    let receipt_raw = fs::read(&receipt_path).unwrap();
    let receipt_sha = Digest256::of_bytes(&receipt_raw).to_hex();
    assert_eq!(backup["receipt_sha256"], receipt_sha);

    let restored_scratch = ScratchRoot::new();
    fs::set_permissions(&restored_scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let restored_root = restored_scratch.0.canonicalize().unwrap();
    let mut restore_args = base_recovery_args(
        "restore",
        &case,
        &backup_root,
        &restored_root,
        &restore_path,
        &restore_sha,
    );
    restore_args.extend(["--receipt-sha256".to_owned(), receipt_sha.clone()]);
    let restored = run_recovery_cli(&binary, "restore", &restore_args, &restore_url);
    assert_eq!(restored["status"], "restore_verified");
    assert_eq!(restored["domain"], case.lab.domain);
    assert_eq!(restored["source_admission"], false);
    assert_eq!(restored["source_currentness_granted"], false);
    assert_eq!(restored["cut"], backup["receipt"]["cut"]);

    let restored_db = DurablePgCoordinator::connect(&restore_url)
        .expect("verified restore target database reopens");
    let restored_store = tos_segment_store::SegmentStore::open_existing(&restored_root, limits())
        .expect("verified restored STO root reopens");
    let restored_domain = case.lab.domain.clone();
    case.lab = Lab {
        db: restored_db,
        store: restored_store,
        _root: restored_scratch,
        domain: restored_domain,
        url: restore_url,
        attempt_fences: std::collections::HashMap::new(),
    };
    retarget_revision_owner(&mut case, &restored_root);

    let restored_cut = cold_recovery_cut(&mut case);
    assert_eq!(restored_cut, source_cut_before);
    assert_eq!(real_recovery_metadata_snapshot(&case), metadata_before);
    assert_eq!(
        source_inventory_projection_rows(&case.lab.url, &case.lab.domain),
        source_inventory_before,
        "restore preserves every retained inventory overlay row committed by the V9 census"
    );
    assert_eq!(case.lab.head_seq(), pending_head);
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, consumer, &DurableProjectionProfile {
                id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
                version: 1,
                source_contract: contract_digest(),
            }, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Pending { through_seq, head_seq }
            if through_seq == projection_head && head_seq == pending_head
    ));
    assert_eq!(
        real_projection_snapshot(&case, consumer),
        projection_before_revision,
        "restore preserves the exact pending projection cursor and rows"
    );
    let (projection_resume_work, projection_resume_process) = apply_projection_in_fresh_process(
        &case,
        consumer,
        tos_command::source_search_projection::SOURCE_SEARCH_PROFILE,
    );
    case.lab.db = DurablePgCoordinator::connect(&case.lab.url)
        .expect("restored target reopens after projector resume");
    assert!(matches!(
        case.lab
            .db
            .projection_status(&case.lab.domain, consumer, &DurableProjectionProfile {
                id: tos_command::source_search_projection::SOURCE_SEARCH_PROFILE.to_owned(),
                version: 1,
                source_contract: contract_digest(),
            }, Duration::from_secs(15))
            .unwrap(),
        DurableProjectionStatus::Current { through_seq } if through_seq == pending_head
    ));
    let projection_after_resume = real_projection_snapshot(&case, consumer);
    let projection_search_after_resume = real_projection_search(&mut case, consumer);
    assert_eq!(
        projection_search_after_resume["through_seq"],
        json!(pending_head),
        "the restored source-text search reaches the resumed durable head"
    );
    assert_eq!(
        current_custody_revision(&case, source_path),
        current_custody
    );
    assert_eq!(source_member(&mut case, source_path), source_current);
    assert_eq!(
        cold_exact_source_bytes(&mut case, source_path, 1),
        retained_v1
    );
    assert_eq!(
        cold_exact_source_bytes(&mut case, source_path, current_custody),
        source_current
    );
    let domain_before_continuation = metadata_before["domain"].clone();
    let continuation = continue_native_revision_after_restore(
        &mut case,
        &software,
        &binary,
        &source_current,
        pending_head,
        &domain_before_continuation,
        &projection_after_resume,
    );
    json!({
        "private_package_schema_guard":private_package_schema_guard,
        "source_inventory_projection_fixture":{
            "kind":"verified_noop_storage_fixture",
            "producer_coverage":false,
            "rows":source_inventory_before.len(),
            "bytes":source_inventory_projection_bytes(&source_inventory_before),
            "row_root":source_inventory_projection_root(&source_inventory_before)
        },
        "domain":case.lab.domain,
        "source_path":case.target.source_path,
        "source_current_sha256":Digest256::of_bytes(&source_current).to_hex(),
        "retained_v1_sha256":Digest256::of_bytes(&retained_v1).to_hex(),
        "initial_native_revision":first_revision,
        "projection_work_before_revision":projection_work,
        "projection_before_revision":projection_before_revision,
        "projection_interruption":{
            "cursor_before_interruption":projection_head,
            "pending_head":pending_head,
            "rows_unchanged_after_kill":true,
            "status_after_kill":"pending",
            "process":interrupted_projection_process
        },
        "projection_restore_resume":{
            "restored_status_before_resume":"pending",
            "restored_cursor_before_resume":projection_head,
            "restored_head_before_resume":pending_head,
            "resume_work":projection_resume_work,
            "resume_process":projection_resume_process,
            "status_after_resume":"current",
            "resumed_projection":projection_after_resume,
            "search_after_resume":projection_search_after_resume
        },
        "backup":{
            "status":backup["status"],
            "schema":backup["receipt"]["schema"],
            "receipt_sha256":receipt_sha,
            "cut":backup["receipt"]["cut"],
            "source_admission":backup["receipt"]["source_admission"],
            "production_quiescence_verified":backup["receipt"]["production_quiescence_verified"]
        },
        "restore":{
            "status":restored["status"],
            "cut":restored["cut"],
            "source_admission":restored["source_admission"],
            "source_currentness_granted":restored["source_currentness_granted"],
            "metadata_history_receipts_log_and_outbox_exact":true,
            "navigation_policy_exact":true,
            "projection_pending_cursor_and_rows_exact":true,
            "cold_current_and_retained_source_bytes_exact":true
        },
        "continuation":continuation,
        "owner_config_path_retargeted_only":true,
        "claims":{"source_admission":false,"source_currentness_granted":false}
    })
}
