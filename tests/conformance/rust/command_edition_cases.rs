//! One maintained Edition lifecycle through protected native invocation.
//! Preserves current union, cold old replay and deterministic retained Python recovery.
use super::*;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_creation_store::IsolatedCreationRoot;
const FACTORY: &str = r#"
import json,sys,tempfile
from pathlib import Path
repository,root,unused=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_edition_commands as fixture
class SelectedRoot:
    def __init__(self,*a,**kw): self.name=str(root)
    def cleanup(self): pass
original=tempfile.TemporaryDirectory;tempfile.TemporaryDirectory=SelectedRoot
try:
    case=fixture.NativeEditionTests(methodName='runTest');case.setUp()
finally: tempfile.TemporaryDirectory=original
case.owner.chmod(0o600)
implementations=sorted(fixture.edition.IMPLEMENTATIONS)
for ref in implementations:
    p=root/ref;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((repository/ref).read_bytes());p.chmod(0o644)
note=case.expression_path.with_name('unrelated-note.txt');note.write_bytes(b'unchanged unrelated note\n')
print(json.dumps({'config':case.config,'proposal':case.proposal(),'origin_claim':case.origin_request['claim'],'implementations':implementations,'owner':str(case.owner),'edition_ref':case.expression_ref,'work_ref':case.config['work_source_path'],'origin_ref':case.expression_path.with_name('source-claims.jsonl').relative_to(root).as_posix()},ensure_ascii=False))
"#;
const UPDATE: &str = r#"
import json,sys
from pathlib import Path
repository,root,unused=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_edition_commands as fixture
case=fixture.NativeEditionTests(methodName='runTest');case.root=root
import test_source_expression_commands as origin_fixture
case.origin=origin_fixture.NativeExpressionTests(methodName='runTest');case.origin.root=root
case.origin.work_ref='ToS/source-witnesses/works/synthetic/parent/work.json';case.origin.work_path=root/case.origin.work_ref
case.expression_ref=json.loads((root/'edition-owner.json').read_bytes())['expression_source_path']
case.expression_path=root/case.expression_ref;case.owner=root/'edition-owner.json'
case.config=json.loads(case.owner.read_bytes());case.expression=json.loads(case.expression_path.read_bytes())
case.extra_records=[];case.extra_claims=[]

action=json.load(sys.stdin)
if action['action']=='next':
    case.origin_request={'claim':action['origin_claim']}
    case.select_child('second');case.owner.chmod(0o600)
    print(json.dumps({'config':case.config,'proposal':case.proposal()}))
elif action['action']=='rebuild':
    case.rebuild();print('{}')
elif action['action']=='crash':
    pending=case.crash(action['request'])
    case.config['allowed_operations']=[fixture.edition.RECOVERY]
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
    let python =
        std::env::var_os("TOS_MAINTAINED_PYTHON").expect("explicit maintained fixture interpreter");
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
            panic!("bounded maintained Edition fixture refused");
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
        "Edition fixture {}",
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
        "Edition native CLI output={} stderr={}",
        String::from_utf8_lossy(&raw),
        String::from_utf8_lossy(&errors)
    );
    serde_json::from_slice(&raw).unwrap()
}
fn freeze_invocation(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn fixture_bounds(files: &std::collections::BTreeMap<String, Vec<u8>>) {
    let total = files.values().map(Vec::len).sum::<usize>();
    let maximum = files.values().map(Vec::len).max().unwrap_or(0);
    assert!(files.len() <= 2048 && total <= 8 * 1024 * 1024 && maximum <= 2 * 1024 * 1024);
    assert!(
        files
            .keys()
            .all(|path| path.len() <= 1024 && path.split('/').count() <= 24)
    );
    eprintln!(
        "edition fixture preflight: members={} bytes={} max={}",
        files.len(),
        total,
        maximum
    );
}
fn prepared_request(preview: &Value, id: &str) -> Value {
    let mut request = preview["prepared_request"].clone();
    request["command_id"] = json!(id);
    request
}

#[test]
fn native_edition_cli_preserves_topology_cold_replay_and_retained_recovery() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    for decision in [None, Some("resume")] {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let native_owner_paths = [
            "rust/crates/tos-command/src/source_expression_edition.rs",
            "rust/crates/tos-command/src/source_native_edition_cli.rs",
        ];
        let captured = super::native_python_fixture(
            "edition-base",
            &[("source-root", isolated.path())],
            &native_owner_paths,
        );
        super::assert_native_python_fixture(&captured, FACTORY, &native_owner_paths);
        let fixture = captured.packets.get("factory").unwrap();
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        assert!(fs::metadata(&owner).unwrap().len() <= 4096);
        let edition = isolated
            .path()
            .join(fixture["edition_ref"].as_str().unwrap());
        let work = isolated.path().join(fixture["work_ref"].as_str().unwrap());
        let before = fs::read(&edition).unwrap();
        let forms_path = edition.with_file_name("expression.human-forms.json");
        let prior_forms: Value = serde_json::from_slice(&fs::read(&forms_path).unwrap()).unwrap();
        let work_before = fs::read(&work).unwrap();
        let note = edition.with_file_name("unrelated-note.txt");
        let origin = isolated
            .path()
            .join(fixture["origin_ref"].as_str().unwrap());
        let origin_before = fs::read(&origin).unwrap();
        let note_before = fs::read(&note).unwrap();
        let authored = super::command_work_cases::authored_work_files(isolated.path());
        fixture_bounds(&authored);
        assert!(
            authored.len() <= 2048
                && authored.values().map(Vec::len).sum::<usize>() <= 8 * 1024 * 1024
        );
        let mut capture_files = authored.clone();
        for reference in &native_owner_paths {
            let reference = *reference;
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
        eprintln!(
            "edition preflight: authored_members={} authored_bytes={} authored_max={} capture_bytes={}",
            authored.len(),
            authored.values().map(Vec::len).sum::<usize>(),
            authored.values().map(Vec::len).max().unwrap_or(0),
            capture_files.values().map(Vec::len).sum::<usize>()
        );

        let (capture, _software, components) =
            super::command_record_cases::captured_components(&capture_files, deadline, &cancelled);
        let store = temporary.path().join("edition-cut");
        let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
        let native = PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
                .expect("OPS must supply immutable native owner CLI"),
        );
        assert!(native.is_absolute());
        let worker = super::validation_cut_cases::selected_worker_path();
        let invocation_path = temporary.path().join("native-edition-invocation.json");
        let mut invocation = json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,
        "native_executable":native,"native_executable_sha256":super::command_text_cases::alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":null,"owner_context":null,"assessment_schema_worker":null,
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
        assert_eq!(fs::read(&edition).unwrap(), before);
        let request = prepared_request(&preview, "native-edition-first");
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
            assert_eq!(result["receipt"]["principal_id"], json!("model:synthetic"));
            let parent: Value = serde_json::from_slice(&fs::read(&edition).unwrap()).unwrap();
            assert_eq!(
                parent["embodiment_claim_refs"],
                json!([fixture["config"]["claim_id"]])
            );
        } else {
            let mut bad = fixture["proposal"].clone();
            bad["record"]["embodies_expression_refs"] = json!([]);
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
            assert_eq!(result["grants_admission"], json!(false));
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
                result["source_profiles"]["embodied_by"]["relation_type_id"],
                json!("tos.relation.embodied-by")
            );
            let claim_path = isolated
                .path()
                .join(fixture["config"]["edition_source_path"].as_str().unwrap());
            let claim_path = claim_path.with_file_name("source-claims.jsonl");
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
            fixture_bounds(&after);
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
                Some(&json!({"action":"next","origin_claim":fixture["origin_claim"]})),
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
            let next_request = prepared_request(&next_preview, "native-edition-second");
            cli(
                &repository,
                &owner,
                &invocation_path,
                &next_request,
                true,
                deadline,
            );
            let parent: Value = serde_json::from_slice(&fs::read(&edition).unwrap()).unwrap();
            assert_eq!(
                parent["embodiment_claim_refs"],
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
            fixture_bounds(&final_files);
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
        assert_eq!(fs::read(&origin).unwrap(), origin_before);
    }
}
