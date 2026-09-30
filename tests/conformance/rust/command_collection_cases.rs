//! Actual native Collection consumer, including retained Python multi-file interruption.
use super::*;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_creation_store::IsolatedCreationRoot;
use tos_foundation::Digest256;
const FACTORY: &str = r#"
import json,sys,tempfile
from pathlib import Path
repository,root,unused=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_collection_commands as fixture
class SelectedRoot:
    def __init__(self,*a,**kw): self.name=str(root)
    def cleanup(self): pass
original=tempfile.TemporaryDirectory;tempfile.TemporaryDirectory=SelectedRoot
try:
    case=fixture.NativeCollectionTests(methodName='runTest');case.setUp()
finally: tempfile.TemporaryDirectory=original
case.owner.chmod(0o600)
implementations=sorted(fixture.attachment.IMPLEMENTATIONS)
for ref in implementations:
    p=root/ref;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((repository/ref).read_bytes());p.chmod(0o644)
print(json.dumps({'config':case.config,'proposal':case.proposal(),'implementations':implementations,'owner':str(case.owner),'collection_ref':case.collection_ref,'work_ref':case.work_ref},ensure_ascii=False))
"#;
const UPDATE: &str = r#"
import json,sys
from pathlib import Path
repository,root,unused=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_collection_commands as fixture
case=fixture.NativeCollectionTests(methodName='runTest');case.root=root
case.collection_ref='ToS/source-witnesses/collections/synthetic/parent/collection.json'
case.work_ref='ToS/source-witnesses/works/synthetic/membership/work.json'
case.collection_path=root/case.collection_ref;case.work_path=root/case.work_ref;case.owner=root/'membership-owner.json'
case.config=json.loads(case.owner.read_bytes());case.collection=json.loads(case.collection_path.read_bytes());case.work=json.loads(case.work_path.read_bytes())
case.forms=[{'form_id':case.config['allowed_collection_form_ids'][0],'field_id':'metadata.preferred-name'}]
action=json.load(sys.stdin)
if action['action']=='next':
    case.select_claim('second');case.owner.chmod(0o600)
    print(json.dumps({'config':case.config,'proposal':case.proposal()}))
elif action['action']=='rebuild':
    case.rebuild();print('{}')
elif action['action']=='crash':
    pending=case.crash(action['request'],edge=8)
    case.config['allowed_operations']=[fixture.attachment.RECOVERY]
    case.config['principal_id']='model:synthetic-recoverer'
    case.owner.write_text(json.dumps(case.config));case.owner.chmod(0o600)
    print(json.dumps({'config':case.config,'request':case.recovery(pending,action['decision'])}))
"#;
fn python(
    repository: &Path,
    root: &Path,
    recovery: &Path,
    script: &str,
    input: Option<&Value>,
    deadline: Instant,
) -> Value {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut stdin = tempfile::tempfile().unwrap();
    if let Some(input) = input {
        stdin
            .write_all(&serde_json::to_vec(input).unwrap())
            .unwrap();
    }
    stdin.seek(SeekFrom::Start(0)).unwrap();
    let mut stdout = tempfile::tempfile().unwrap();
    let mut stderr = tempfile::tempfile().unwrap();
    let python = std::env::var_os("TOS_MAINTAINED_PYTHON")
        .expect("explicit maintained fixture interpreter");
    let mut child = Command::new(python)
        .args(["-c", script])
        .arg(repository)
        .arg(root)
        .arg(recovery)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout.try_clone().unwrap()))
        .stderr(Stdio::from(stderr.try_clone().unwrap()))
        .spawn()
        .unwrap();
    let step = deadline.min(Instant::now() + Duration::from_secs(60));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step
            || stdout.metadata().unwrap().len() > 1_048_576
            || stderr.metadata().unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded maintained Collection fixture refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    stdout.seek(SeekFrom::Start(0)).unwrap();
    stderr.seek(SeekFrom::Start(0)).unwrap();
    let mut raw = Vec::new();
    let mut errors = Vec::new();
    stdout.read_to_end(&mut raw).unwrap();
    stderr.read_to_end(&mut errors).unwrap();
    assert!(
        status.success(),
        "Collection fixture {}",
        String::from_utf8_lossy(&errors)
    );
    assert!(Instant::now() < step && raw.len() <= 1_048_576 && errors.len() <= 1_048_576);
    serde_json::from_slice(&raw).unwrap()
}
fn cli(
    repository: &Path,
    owner: &Path,
    invocation: &Path,
    request: &Value,
    success: bool,
    deadline: Instant,
) -> Value {
    let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
        repository, owner, invocation, request, deadline,
    );
    assert_eq!(
        status.success(),
        success,
        "Item native CLI output={} stderr={}",
        String::from_utf8_lossy(&raw),
        String::from_utf8_lossy(&errors)
    );
    serde_json::from_slice(&raw).unwrap()
}
fn freeze_invocation(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn configured_digest(value: &Value) -> String {
    let raw = serde_json::to_vec(value).unwrap();
    let value = tos_foundation::parse_json(
        &raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .unwrap();
    Digest256::of_bytes(
        &tos_foundation::canonical_bytes_v1(
            value.root(),
            tos_foundation::CanonicalProfile::SourceCommandInputV1,
            tos_foundation::JsonLimits::default(),
        )
        .unwrap(),
    )
    .to_prefixed()
}
fn prepared_request(preview: &Value, id: &str) -> Value {
    let mut request = preview["prepared_request"].clone();
    request["command_id"] = json!(id);
    request
}

#[test]
fn native_collection_cli_attaches_replays_and_recovers_multi_file_membership() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    for decision in [None, Some("resume"), Some("rollback")] {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let fixture = python(
            &repository,
            isolated.path(),
            isolated.path(),
            FACTORY,
            None,
            deadline,
        );
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        assert!(fs::metadata(&owner).unwrap().len() <= 4096);
        let collection = isolated
            .path()
            .join(fixture["collection_ref"].as_str().unwrap());
        let work = isolated.path().join(fixture["work_ref"].as_str().unwrap());
        let before = fs::read(&collection).unwrap();
        let forms_path = collection.with_file_name("collection.human-forms.json");
        let prior_forms: Value = serde_json::from_slice(&fs::read(&forms_path).unwrap()).unwrap();
        let work_before = fs::read(&work).unwrap();
        let note = collection.with_file_name("unrelated-note.txt");
        let note_before = fs::read(&note).unwrap();
        let authored = super::command_work_cases::authored_work_files(isolated.path());
        assert!(
            authored.len() <= 2048
                && authored.values().map(Vec::len).sum::<usize>() <= 8 * 1024 * 1024
        );
        let mut capture_files = authored.clone();
        for reference in fixture["implementations"].as_array().unwrap() {
            let reference = reference.as_str().unwrap();
            assert!(
                capture_files
                    .insert(
                        reference.into(),
                        fs::read(isolated.path().join(reference)).unwrap()
                    )
                    .is_none()
            );
        }
        assert!(capture_files.values().map(Vec::len).sum::<usize>() <= 8 * 1024 * 1024);
        let (capture, _software, components) =
            super::command_record_cases::captured_components(&capture_files, deadline, &cancelled);
        let store = temporary.path().join("collection-cut");
        let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
        let native = PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
                .expect("OPS must supply immutable native owner CLI"),
        );
        assert!(native.is_absolute());
        let worker = super::validation_cut_cases::selected_worker_path();
        let invocation_path = temporary.path().join("native-collection-invocation.json");
        let mut invocation = json!({"schema_version":"tos_local_native_collection_invocation_v1","owner_config":owner,
        "native_executable":native,"native_executable_sha256":super::command_text_cases::alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":null,
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":worker,"sha256":super::command_text_cases::alignment_image_digest(&worker).to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        freeze_invocation(&invocation_path, &invocation);

        let preview = cli(
            &repository,
            &owner,
            &invocation_path,
            &fixture["proposal"],
            true,
            deadline,
        );
        assert_eq!(fs::read(&collection).unwrap(), before);
        let request = prepared_request(&preview, "native-collection-first");
        if let Some(decision) = decision {
            let renewal = python(
                &repository,
                isolated.path(),
                isolated.path(),
                UPDATE,
                Some(&json!({"action":"crash","request":request,"decision":decision})),
                deadline,
            );
            let result = cli(
                &repository,
                &owner,
                &invocation_path,
                &renewal["request"],
                true,
                deadline,
            );
            assert_eq!(result["recovery"]["committed"], json!(decision == "resume"));
            assert_eq!(
                result["recovery"]["publication"]["recovery_authorization"]["principal_id"],
                json!("model:synthetic-recoverer")
            );
            if decision == "rollback" {
                assert!(result["receipt"].is_null());
                assert_eq!(fs::read(&collection).unwrap(), before);
            } else {
                assert_eq!(result["receipt"]["principal_id"], json!("model:synthetic"));
            }
        } else {
            let mut bad = fixture["proposal"].clone();
            bad["work"]["notes"] = json!("unselected change");
            cli(&repository, &owner, &invocation_path, &bad, false, deadline);
            let result = cli(
                &repository,
                &owner,
                &invocation_path,
                &request,
                true,
                deadline,
            );
            assert_eq!(result["replayed"], json!(false));
            let forms: Value = serde_json::from_slice(&fs::read(&forms_path).unwrap()).unwrap();
            assert_eq!(forms["prior_forms"], prior_forms["forms"]);
            for group in result["materializations"].as_object().unwrap().values() {
                assert!(
                    group
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|view| view["state"] == "ready")
                );
            }
            assert_eq!(
                result["source_profiles"]["contains_work"]["relation_type_id"],
                json!("tos.relation.contains-work")
            );
            let claim_path = isolated
                .path()
                .join(fixture["config"]["claim_source_path"].as_str().unwrap());
            let claim_before = fs::read(&claim_path).unwrap();
            python(
                &repository,
                isolated.path(),
                isolated.path(),
                UPDATE,
                Some(&json!({"action":"rebuild"})),
                deadline,
            );
            let after = super::command_work_cases::authored_work_files(isolated.path());
            let current = super::validation_cut_cases::write_cut_store_on_base(
                &after,
                &store,
                Some(selected),
            );
            invocation["source_revision"] = json!(current.0.to_prefixed());
            invocation["original_source_revision"] = json!(selected.0.to_prefixed());
            freeze_invocation(&invocation_path, &invocation);
            assert_eq!(
                cli(
                    &repository,
                    &owner,
                    &invocation_path,
                    &request,
                    true,
                    deadline
                )["replayed"],
                json!(true)
            );
            let next = python(
                &repository,
                isolated.path(),
                isolated.path(),
                UPDATE,
                Some(&json!({"action":"next"})),
                deadline,
            );
            invocation["original_source_revision"] = Value::Null;
            freeze_invocation(&invocation_path, &invocation);
            let next_preview = cli(
                &repository,
                &owner,
                &invocation_path,
                &next["proposal"],
                true,
                deadline,
            );
            let next_request = prepared_request(&next_preview, "native-collection-second");
            cli(
                &repository,
                &owner,
                &invocation_path,
                &next_request,
                true,
                deadline,
            );
            let parent: Value = serde_json::from_slice(&fs::read(&collection).unwrap()).unwrap();
            assert_eq!(
                parent["membership_claim_refs"],
                json!([fixture["config"]["claim_id"], next["config"]["claim_id"]])
            );
            assert_eq!(fs::read(&claim_path).unwrap(), claim_before);
            python(
                &repository,
                isolated.path(),
                isolated.path(),
                UPDATE,
                Some(&json!({"action":"rebuild"})),
                deadline,
            );
            let final_files = super::command_work_cases::authored_work_files(isolated.path());
            let final_revision = super::validation_cut_cases::write_cut_store_on_base(
                &final_files,
                &store,
                Some(current),
            );
            invocation["source_revision"] = json!(final_revision.0.to_prefixed());
            invocation["original_source_revision"] = json!(selected.0.to_prefixed());
            freeze_invocation(&owner, &fixture["config"]);
            freeze_invocation(&invocation_path, &invocation);
            assert_eq!(
                cli(
                    &repository,
                    &owner,
                    &invocation_path,
                    &request,
                    true,
                    deadline
                )["replayed"],
                json!(true)
            );
        }
        assert_eq!(fs::read(&work).unwrap(), work_before);
        assert_eq!(fs::read(&note).unwrap(), note_before);
    }
}
