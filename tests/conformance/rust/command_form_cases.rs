//! One composed native form lifecycle over existing independent Python oracle
//! bytes, actual anchored source cuts and the selected disposable schema worker.
//! Carrier/schema green still ends in the explicit full-admission refusal.
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_command::{CommandContext, SourceCommandError, SourceFile};
use tos_command::source_forms::run_form_command;
use tos_command::source_operation::{SourceOperationError, bind_selected_candidate};
use tos_source_store::{CorpusCutReader, CutReadLimits};
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::operation::OperationLimits;
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};

fn fixture_files(profile: &str) -> (BTreeMap<String, Vec<u8>>, Vec<u8>, String) {
    let repository = super::validation_cut_cases::repository();
    let packet = repository
        .join("rust/crates/tos-command/tests/fixtures/source_forms_shadow")
        .join(profile);
    let config = fs::read(packet.join("owner.synthetic.json")).unwrap();
    let owner: Value = serde_json::from_slice(&config).unwrap();
    let path = owner["source_path"].as_str().unwrap().to_owned();
    let target = format!("{}.human-forms.json", path.strip_suffix(".json").unwrap());
    let mut files = BTreeMap::new();
    // Existing exact selected schema resources; no production corpus traversal.
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            files.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    // Native oracle pins its exact source schema, independently of current
    // schema evolution. The fixture worker receives these selected bytes.
    if !profile.is_empty() {
        let source: Value =
            serde_json::from_slice(&fs::read(packet.join("source.initial.json")).unwrap()).unwrap();
        let schema = match source["schema_version"].as_str().unwrap() {
            "tos_artifact_source_witness_v1" => "artifact-source-witness.schema.json",
            "tos_artifact_source_witness_v2" => "artifact-source-witness-v2.schema.json",
            "tos_scholarly_composite_witness_v1" => "scholarly-composite-witness.schema.json",
            _ => panic!("unknown selected oracle source"),
        };
        files.insert(
            format!("ToS/contracts/{schema}"),
            fs::read(packet.join("source-schema.initial.json")).unwrap(),
        );
    }
    files.insert(path, fs::read(packet.join("source.initial.json")).unwrap());
    files.insert(
        target.clone(),
        fs::read(packet.join("form-set.initial.json")).unwrap(),
    );
    (files, config, target)
}
pub(super) fn open_cut(
    root: &Path,
    revision: SourceRevision,
    deadline: Instant,
    cancel: &AtomicBool,
) -> CorpusCutReader {
    CorpusReader::open_existing(
        root,
        ReadLimits {
            max_manifest_bytes: 4_194_304,
            max_manifest_entries: 2048,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
    )
    .unwrap()
    .open_source_cut(
        revision,
        CutReadLimits {
            max_revisions: 4,
            max_members: 2048,
            max_total_bytes: 33_554_432,
            max_member_bytes: 8_388_608,
        },
        deadline,
        cancel,
    )
    .unwrap()
}
pub(super) fn schemas(
    cut: &CorpusCutReader,
    deadline: Instant,
    cancel: &AtomicBool,
) -> CutWorkerSchemaExecutor {
    let absolute_path = super::validation_cut_cases::selected_worker_path();
    let sha256 = Digest256::of_bytes(&fs::read(&absolute_path).unwrap());
    CutWorkerSchemaExecutor::from_cut(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            absolute_path,
            sha256,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 262_144,
        },
        deadline,
        cancel,
    )
    .unwrap()
}
pub(super) fn context(
    files: &BTreeMap<String, Vec<u8>>,
    configuration_raw: Vec<u8>,
    request_raw: Vec<u8>,
    base_revision: SourceRevision,
) -> CommandContext {
    CommandContext {
        base_revision,
        configuration_raw,
        request_raw,
        recorded_at: "2026-01-01T12:34:56+00:00".into(),
        effective_uid: 1000,
        files: files
            .iter()
            .map(|(path, raw)| SourceFile {
                path: RelativePath::parse(path).unwrap(),
                raw: raw.clone(),
            })
            .collect(),
    }
}
pub(super) fn successor(
    files: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    base: SourceRevision,
) -> SourceRevision {
    let unlinked = super::validation_cut_cases::write_cut_store(files, root);
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(
            root.join("revisions")
                .join(unlinked.0.to_hex())
                .join("snapshot.json"),
        )
        .unwrap(),
    )
    .unwrap();
    manifest["base_revision"] = Value::String(base.0.to_hex());
    manifest.as_object_mut().unwrap().remove("revision");
    let revision = SourceRevision(Digest256::of_bytes(&canonical_json(&manifest)));
    manifest["revision"] = Value::String(revision.0.to_hex());
    let directory = root.join("revisions").join(revision.0.to_hex());
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("snapshot.json"), canonical_json(&manifest)).unwrap();
    revision
}

#[test]
fn maintained_forms_propose_exact_bytes_bind_real_cut_and_refuse_unissued_admission() {
    for profile in ["", "artifact-v1", "artifact-v2", "composite-v1"] {
        let (mut files, config, target) = fixture_files(profile);
        let packet = super::validation_cut_cases::repository()
            .join("rust/crates/tos-command/tests/fixtures/source_forms_shadow")
            .join(profile);
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("store");
        let base = super::validation_cut_cases::write_cut_store(&files, &root);
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(120);
        let cut = open_cut(&root, base, deadline, &cancel);
        let mut worker = schemas(&cut, deadline, &cancel);
        let request = fs::read(packet.join("apply.request.json")).unwrap();
        let ctx = context(&files, config.clone(), request.clone(), base);
        let prepared = run_form_command(&ctx, &mut worker, deadline, &cancel).unwrap();
        assert_eq!(prepared.changes.len(), 1, "{profile}");
        let raw = prepared.changes[0].after.as_ref().unwrap();
        assert_eq!(
            *raw,
            fs::read(packet.join("form-set.published.json")).unwrap(),
            "{profile}: maintained exact Python publication bytes"
        );
        assert_eq!(
            prepared.commit(),
            Err(SourceCommandError::MissingProductionAdmission)
        );
        files.insert(target, raw.clone());
        let candidate = successor(&files, &root, base);
        let candidate_cut = open_cut(&root, candidate, deadline, &cancel);
        let bound = bind_selected_candidate(
            &ctx,
            prepared,
            &candidate_cut,
            RelativePath::parse("protected-owner/form-command.json").unwrap(),
            OperationLimits {
                max_member_bytes: 8_388_608,
                max_total_bytes: 33_554_432,
                max_state_bytes: 33_554_432,
                max_reads: 8192,
                max_changes: 64,
                deadline,
            },
            &cancel,
        )
        .unwrap();
        assert_eq!(bound.binding().base_revision(), base);
        assert_eq!(bound.binding().candidate_revision(), candidate);
        assert!(matches!(
            bound.commit(),
            Err(SourceOperationError::MissingFullSourceAdmission)
        ));
        let replay_ctx = context(&files, config.clone(), request.clone(), candidate);
        let mut worker = schemas(&candidate_cut, deadline, &cancel);
        let replay = run_form_command(&replay_ctx, &mut worker, deadline, &cancel).unwrap();
        assert!(replay.replayed);
        assert!(replay.changes.is_empty());
        // Current revocation must apply before historical receipt replay.
        let mut revoked: Value = serde_json::from_slice(&config).unwrap();
        revoked["allowed_operations"] = serde_json::json!([]);
        let revoked_ctx = context(
            &files,
            serde_json::to_vec(&revoked).unwrap(),
            request,
            candidate,
        );
        assert!(matches!(
            run_form_command(&revoked_ctx, &mut worker, deadline, &cancel),
            Err(SourceCommandError::Denied(_))
        ));
    }
}
