//! One isolated Work→Expression owner path. The maintained Python fixture
//! authors only the original Work, Claim proposal and catalog; Rust performs
//! the selected native mutation and later observes its retained transaction.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_command::SourceCommandError;
use tos_command::source_creation_store::{
    CreationFilesystem, IsolatedCreationRoot, execute_isolated_work_expression_from_captures,
    prepare_isolated_work_expression_from_proposal, replay_isolated_work_expression_from_captures,
};
use tos_validation::FormatProfile;
use tos_validation::executor::ExecutorBudget;
use tos_validation::item_rules::ItemLimits;

fn authored_work_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut directories = vec![root.join("ToS")];
    let mut files = BTreeMap::new();
    let mut bytes = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "synthetic Work member is a link");
            if kind.is_dir() {
                directories.push(entry.path());
            } else {
                assert!(kind.is_file());
                let reference = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                // This exact historical-create sidecar is live lock state,
                // not a portable authored member of the selected cut.
                if reference != "ToS/source-witnesses/.historical-create.writer.lock"
                    && tos_source_store::is_authored_source_path_v1(&reference)
                {
                    let raw = fs::read(entry.path()).unwrap();
                    assert!(raw.len() <= 8_388_608);
                    bytes = bytes.checked_add(raw.len()).unwrap();
                    assert!(bytes <= 33_554_432);
                    assert!(files.insert(reference, raw).is_none());
                    assert!(files.len() <= 4096);
                }
            }
        }
    }
    files
}

fn work_worker(
    cut: &tos_source_store::CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_validation::source_cut::CutWorkerSchemaExecutor {
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!budget.execution_wall.is_zero());
    super::command_form_cases::schemas_for_profile_with_budget(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        budget,
        deadline,
        cancelled,
    )
}

#[test]
fn native_work_expression_publishes_replays_and_prepares_next_sibling() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    // The protected writer publishes the first child. Its committed native
    // lineage is then consumed by a fresh preview and a second publication.
    {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let factory = r#"
import json,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_expression_commands as fixture
class ExistingRoot:
    def __init__(self,*args,**kwargs): self.name=str(root)
    def cleanup(self): pass
original=fixture.tempfile.TemporaryDirectory
fixture.tempfile.TemporaryDirectory=ExistingRoot
try:
    case=fixture.NativeExpressionTests(methodName='runTest')
    case.setUp()
finally:
    fixture.tempfile.TemporaryDirectory=original
request=case.request()
first_config=case.config.copy()
case.select_child('second')
second_proposal=case.proposal()
second_config=case.config.copy()
case.config=first_config
case.owner.write_text(json.dumps(first_config))
print(json.dumps({'request':request,'second_proposal':second_proposal,'second_config':second_config,
    'implementations':sorted(fixture.compound.IMPLEMENTATIONS),
    'work_ref':case.work_ref,'expression_ref':case.config['expression_source_path']},
    ensure_ascii=False,separators=(',',':')))
"#;
        let stdout = temporary.path().join("work-fixture.stdout");
        let stderr = temporary.path().join("work-fixture.stderr");
        let mut producer = Command::new("python3")
            .args(["-c", factory])
            .arg(&repository)
            .arg(isolated.path())
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()))
            .spawn()
            .unwrap();
        let status = loop {
            if let Some(status) = producer.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline
                || fs::metadata(&stdout).unwrap().len() > 1_048_576
                || fs::metadata(&stderr).unwrap().len() > 1_048_576
            {
                producer.kill().unwrap();
                producer.wait().unwrap();
                panic!("bounded maintained Work fixture refused");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(Instant::now() < deadline);
        assert!(fs::metadata(&stdout).unwrap().len() <= 1_048_576);
        assert!(fs::metadata(&stderr).unwrap().len() <= 1_048_576);
        assert!(
            status.success(),
            "maintained Work fixture: {}",
            String::from_utf8_lossy(&fs::read(&stderr).unwrap())
        );
        let fixture: Value = serde_json::from_slice(&fs::read(&stdout).unwrap()).unwrap();
        let owner = isolated.path().join("compound-owner.json");
        let owner_raw = fs::read(&owner).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let configuration: Value = serde_json::from_slice(&owner_raw).unwrap();
        let mut files = authored_work_files(isolated.path());
        let authored = files.clone();
        assert!(authored.contains_key(fixture["work_ref"].as_str().unwrap()));
        for reference in fixture["implementations"].as_array().unwrap() {
            let reference = reference.as_str().unwrap();
            let raw = fs::read(repository.join(reference)).unwrap();
            let target = isolated.path().join(reference);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, &raw).unwrap();
            assert!(files.insert(reference.to_owned(), raw).is_none());
        }
        let (_capture, software, components) =
            super::command_record_cases::captured_components(&files, deadline, &cancelled);
        let store = temporary.path().join("original-cut");
        let original = super::validation_cut_cases::write_cut_store(&authored, &store);
        let original_cut =
            super::command_form_cases::open_cut(&store, original, deadline, &cancelled);
        let mut context = super::command_form_cases::context(
            &files,
            owner_raw,
            serde_json::to_vec(&fixture["request"]).unwrap(),
            original,
        );
        context.effective_uid = fs::metadata(isolated.path()).unwrap().uid().into();
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled).unwrap();
        let limits = ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline,
        };
        let control = isolated
            .path()
            .join("ToS/source-witnesses/.metadata-publication.json");
        let mut worker = work_worker(&original_cut, deadline, &cancelled);
        let created_first = execute_isolated_work_expression_from_captures(
            &filesystem,
            &context,
            &original_cut,
            &software,
            &components,
            &mut worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(worker);
        assert!(!created_first.replayed());
        assert_ne!(
            fs::read(isolated.path().join(fixture["work_ref"].as_str().unwrap())).unwrap(),
            authored[fixture["work_ref"].as_str().unwrap()]
        );
        assert!(
            isolated
                .path()
                .join(fixture["expression_ref"].as_str().unwrap())
                .exists()
        );
        assert_eq!(
            created_first.receipt().object_get("grants_admission"),
            Some(&JsonValue::Bool(false))
        );
        let current_files = authored_work_files(isolated.path());
        let current_store = temporary.path().join("current-cut");
        let current = super::validation_cut_cases::write_cut_store(&current_files, &current_store);
        let current_cut =
            super::command_form_cases::open_cut(&current_store, current, deadline, &cancelled);
        let mut replay_worker = work_worker(&current_cut, deadline, &cancelled);
        let replay = replay_isolated_work_expression_from_captures(
            &filesystem,
            &context,
            &original_cut,
            &current_cut,
            &software,
            &components,
            &mut replay_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(replay_worker);
        assert!(replay.replayed());
        assert_eq!(replay.transaction_id(), created_first.transaction_id());
        assert_eq!(replay.manifest_sha256(), created_first.manifest_sha256());
        assert_eq!(replay.receipt(), created_first.receipt());
        let committed_control = fs::read(&control).unwrap();
        // Changed required implementation bytes cannot pass a retained
        // current-source/software fence, even when the original cut survives.
        let implementation = fixture["implementations"][0].as_str().unwrap();
        let implementation_path = isolated.path().join(implementation);
        let original_implementation = fs::read(&implementation_path).unwrap();
        fs::write(
            &implementation_path,
            [original_implementation.as_slice(), b"\n"].concat(),
        )
        .unwrap();
        let mut changed_worker = work_worker(&current_cut, deadline, &cancelled);
        assert!(matches!(
            replay_isolated_work_expression_from_captures(
                &filesystem,
                &context,
                &original_cut,
                &current_cut,
                &software,
                &components,
                &mut changed_worker,
                limits,
                &cancelled,
            ),
            Err(SourceCommandError::Conflict(_))
        ));
        drop(changed_worker);
        fs::write(&implementation_path, original_implementation).unwrap();
        // A mixed native environment in a newly selected complete cut is
        // still refused; decoded JSON equivalence or a reused receipt cannot
        // authorize a different retained byte stream.
        let expression_home = Path::new(fixture["expression_ref"].as_str().unwrap())
            .parent()
            .unwrap();
        let environment_ref = expression_home.join("source-create-environment.json");
        let environment_path = isolated.path().join(&environment_ref);
        let environment_original = fs::read(&environment_path).unwrap();
        let mut environment: Value = serde_json::from_slice(&environment_original).unwrap();
        environment["backend"] = Value::String("mixed Python/native runtime".into());
        let mut altered_raw = serde_json::to_vec(&environment).unwrap();
        altered_raw.push(b'\n');
        fs::write(&environment_path, &altered_raw).unwrap();
        let mut altered_files = current_files.clone();
        altered_files.insert(environment_ref.to_str().unwrap().to_owned(), altered_raw);
        let altered_store = temporary.path().join("altered-cut");
        let altered = super::validation_cut_cases::write_cut_store(&altered_files, &altered_store);
        let altered_cut =
            super::command_form_cases::open_cut(&altered_store, altered, deadline, &cancelled);
        let mut mixed_worker = work_worker(&altered_cut, deadline, &cancelled);
        assert!(matches!(
            replay_isolated_work_expression_from_captures(
                &filesystem,
                &context,
                &original_cut,
                &altered_cut,
                &software,
                &components,
                &mut mixed_worker,
                limits,
                &cancelled,
            ),
            Err(SourceCommandError::Conflict(_)) | Err(SourceCommandError::SchemaExecution { .. })
        ));
        drop(mixed_worker);
        fs::write(&environment_path, environment_original).unwrap();
        assert_eq!(
            fs::read(
                isolated
                    .path()
                    .join(fixture["expression_ref"].as_str().unwrap())
            )
            .unwrap(),
            current_files[fixture["expression_ref"].as_str().unwrap()]
        );
        assert_eq!(fs::read(&control).unwrap(), committed_control);

        // The maintained fixture builder refreshes the derived catalogue
        // after the Rust-native first publication. Rust, not Python prepare,
        // now reads that catalogue and the committed native predecessor to
        // prepare and publish the next sibling.
        let refresh = r#"
import sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:3])
work_ref=sys.argv[3]
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_expression_commands as fixture
case=fixture.NativeExpressionTests(methodName='runTest')
case.root=root
case.work_path=root/work_ref
case.rebuild()
"#;
        let refresh_out = temporary.path().join("work-refresh.stdout");
        let refresh_err = temporary.path().join("work-refresh.stderr");
        let mut builder = Command::new("python3")
            .args(["-c", refresh])
            .arg(&repository)
            .arg(isolated.path())
            .arg(fixture["work_ref"].as_str().unwrap())
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdout(Stdio::from(fs::File::create(&refresh_out).unwrap()))
            .stderr(Stdio::from(fs::File::create(&refresh_err).unwrap()))
            .spawn()
            .unwrap();
        let status = loop {
            if let Some(status) = builder.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline
                || fs::metadata(&refresh_out).unwrap().len() > 1_048_576
                || fs::metadata(&refresh_err).unwrap().len() > 1_048_576
            {
                builder.kill().unwrap();
                builder.wait().unwrap();
                panic!("bounded maintained Work catalogue refresh refused");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(Instant::now() < deadline);
        assert!(fs::metadata(&refresh_out).unwrap().len() <= 1_048_576);
        assert!(fs::metadata(&refresh_err).unwrap().len() <= 1_048_576);
        assert!(
            status.success(),
            "maintained Work catalogue refresh: {}",
            String::from_utf8_lossy(&fs::read(&refresh_err).unwrap())
        );

        let second_config = serde_json::to_vec(&fixture["second_config"]).unwrap();
        fs::write(&owner, &second_config).unwrap();
        let second_files = authored_work_files(isolated.path());
        let second_store = temporary.path().join("second-cut");
        let second_revision =
            super::validation_cut_cases::write_cut_store(&second_files, &second_store);
        let second_cut = super::command_form_cases::open_cut(
            &second_store,
            second_revision,
            deadline,
            &cancelled,
        );
        let mut second_inputs = second_files.clone();
        for reference in fixture["implementations"].as_array().unwrap() {
            let reference = reference.as_str().unwrap();
            assert!(
                second_inputs
                    .insert(
                        reference.into(),
                        fs::read(isolated.path().join(reference)).unwrap()
                    )
                    .is_none()
            );
        }
        let second_fs =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled).unwrap();
        let mut proposal_ctx = super::command_form_cases::context(
            &second_inputs,
            second_config.clone(),
            serde_json::to_vec(&fixture["second_proposal"]).unwrap(),
            second_revision,
        );
        proposal_ctx.effective_uid = fs::metadata(isolated.path()).unwrap().uid().into();
        let mut prepare_worker = work_worker(&second_cut, deadline, &cancelled);
        let prepared = prepare_isolated_work_expression_from_proposal(
            &second_fs,
            &proposal_ctx,
            &second_cut,
            &software,
            &components,
            &mut prepare_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(prepare_worker);
        assert!(!prepared.projected_outputs().is_empty());
        assert_eq!(
            prepared
                .request()
                .object_get("fields")
                .unwrap()
                .object_get("expression_claim_refs")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let preview_raw = tos_foundation::canonical_bytes_v1(
            prepared.request(),
            tos_foundation::CanonicalProfile::SourceCommandInputV1,
            tos_foundation::JsonLimits::default(),
        )
        .unwrap();
        let mut second_request: Value = serde_json::from_slice(&preview_raw).unwrap();
        assert_eq!(second_request["command_id"], "preview:uncommitted");
        // The preview identifier is not a durable command identity. The
        // maintained caller supplies its delegated child ID for publication.
        second_request["command_id"] = fixture["second_config"]["expression_id"].clone();
        let request_raw = serde_json::to_vec(&second_request).unwrap();
        let mut create_ctx = super::command_form_cases::context(
            &second_inputs,
            second_config,
            request_raw,
            second_revision,
        );
        create_ctx.effective_uid = fs::metadata(isolated.path()).unwrap().uid().into();
        let mut second_worker = work_worker(&second_cut, deadline, &cancelled);
        let created = execute_isolated_work_expression_from_captures(
            &second_fs,
            &create_ctx,
            &second_cut,
            &software,
            &components,
            &mut second_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(second_worker);
        assert!(!created.replayed());
        let final_files = authored_work_files(isolated.path());
        let final_store = temporary.path().join("final-cut");
        let final_revision =
            super::validation_cut_cases::write_cut_store(&final_files, &final_store);
        let final_cut =
            super::command_form_cases::open_cut(&final_store, final_revision, deadline, &cancelled);
        let mut final_worker = work_worker(&final_cut, deadline, &cancelled);
        let second_replay = replay_isolated_work_expression_from_captures(
            &second_fs,
            &create_ctx,
            &second_cut,
            &final_cut,
            &software,
            &components,
            &mut final_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(final_worker);
        assert!(second_replay.replayed());
        assert_eq!(second_replay.transaction_id(), created.transaction_id());
        assert_eq!(second_replay.receipt(), created.receipt());
        let final_work: Value = serde_json::from_slice(
            &fs::read(isolated.path().join(fixture["work_ref"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        assert_eq!(final_work["record_version"], 6);
    }
}
