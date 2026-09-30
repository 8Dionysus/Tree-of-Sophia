//! Actual separate-process Item owner over the maintained acquired-File fixture.
use super::*;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_creation_store::IsolatedCreationRoot;
use tos_foundation::Digest256;

// Keep the same live fixture directory on failure for exact physical diagnosis.
// No copy or second capture is created, and successful calls clean up normally.
struct ItemFailureFixture(Option<tempfile::TempDir>);
impl ItemFailureFixture {
    fn new() -> Self {
        Self(Some(tempfile::tempdir().unwrap()))
    }
    fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
}
impl Drop for ItemFailureFixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(directory) = self.0.take() {
                eprintln!(
                    "Item failed fixture retained at {}",
                    directory.keep().display()
                );
            }
        }
    }
}

const FACTORY: &str = r#"
import json,sys,tempfile
from pathlib import Path
repository,root,recovery=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_item_commands as fixture
class SelectedRoot:
    calls=0
    def __init__(self,*args,**kwargs):
        self.name=str(root if SelectedRoot.calls==0 else recovery)
        SelectedRoot.calls+=1
        Path(self.name).mkdir(parents=True,exist_ok=True,mode=0o700)
    def cleanup(self): pass
original=tempfile.TemporaryDirectory
tempfile.TemporaryDirectory=SelectedRoot
try:
    case=fixture.NativeItemTests(methodName='runTest');case.setUp()
finally:
    tempfile.TemporaryDirectory=original
case.owner.chmod(0o600);case.input.chmod(0o600);case.payload_root.chmod(0o700);case.recovery_root.chmod(0o700)
implementations=sorted(fixture.item.IMPLEMENTATIONS)
for reference in implementations:
    target=root/reference;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((repository/reference).read_bytes())
print(json.dumps({'config':case.config,'proposal':case.proposal(),'implementations':implementations,
    'edition_ref':case.edition_ref,'owner':str(case.owner),'input':str(case.input)},ensure_ascii=False,separators=(',',':')))
"#;
// Cheap admission for this finite fixture's frozen runtime recipe, before any
// Git capture or native CLI. These are setup bounds, not Item owner policy.
fn fixture_preflight(root: &Path, recovery: &Path, fixture: &Value) {
    let config = &fixture["config"];
    for key in ["source_root", "payload_root", "input_path", "recovery_root"] {
        assert!(config[key].as_str().unwrap().len() <= 512);
    }
    assert!(
        fs::metadata(fixture["owner"].as_str().unwrap())
            .unwrap()
            .len()
            <= 4096
    );
    assert_eq!(config["byte_size"], json!(928));
    let mut directories = vec![root.to_path_buf(), recovery.to_path_buf()];
    let mut bytes = 0u64;
    let mut entries = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let metadata = entry.path().symlink_metadata().unwrap();
            assert!(!metadata.file_type().is_symlink());
            entries += 1;
            assert!(entries <= 4096);
            if metadata.is_dir() {
                directories.push(entry.path());
            } else {
                assert!(metadata.is_file());
                bytes = bytes.checked_add(metadata.len()).unwrap();
                assert!(bytes <= 8 * 1024 * 1024);
            }
        }
    }
    eprintln!(
        "Item finite fixture setup_bytes={bytes} setup_entries={entries} grant_bytes={}",
        fs::metadata(fixture["owner"].as_str().unwrap())
            .unwrap()
            .len()
    );
}

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
    let mut child = Command::new(crate::maintained_python())
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
            panic!("bounded maintained Item fixture refused");
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
        "Item fixture {}",
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
    eprintln!(
        "Item native invocation operation={} command_id={} decision={}",
        request["operation"], request["command_id"], request["decision"],
    );
    let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
        repository, owner, invocation, request, deadline,
    );
    assert_eq!(
        status.success(),
        success,
        "Item native CLI operation={} command_id={} decision={} output={} stderr={}",
        request["operation"],
        request["command_id"],
        request["decision"],
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
fn native_item_cli_adopts_replays_and_retains_unavailable_inventory() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let temporary = ItemFailureFixture::new();
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
    let recovery = temporary.path().join("item-private-recovery");
    fs::create_dir(&recovery).unwrap();
    fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700)).unwrap();
    let fixture = python(
        &repository,
        isolated.path(),
        &recovery,
        FACTORY,
        None,
        deadline,
    );
    fixture_preflight(isolated.path(), &recovery, &fixture);
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let input = PathBuf::from(fixture["input"].as_str().unwrap());
    let original = fs::read(&input).unwrap();
    let mut config = fixture["config"].clone();
    let before_edition = fs::read(
        isolated
            .path()
            .join(fixture["edition_ref"].as_str().unwrap()),
    )
    .unwrap();
    let authored = super::command_work_cases::authored_work_files(isolated.path());
    let mut capture_files = authored.clone();
    for reference in fixture["implementations"].as_array().unwrap() {
        let reference = reference.as_str().unwrap();
        assert!(
            capture_files
                .insert(
                    reference.to_owned(),
                    fs::read(isolated.path().join(reference)).unwrap()
                )
                .is_none()
        );
    }
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&capture_files, deadline, &cancelled);
    let store = temporary.path().join("item-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must supply immutable native owner CLI"),
    );
    assert!(native.is_absolute());
    let worker = super::validation_cut_cases::selected_worker_path();
    let invocation_path = temporary.path().join("native-item-invocation.json");
    let mut invocation = json!({"schema_version":"tos_local_native_item_invocation_v1","owner_config":owner,
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
    assert!(!preview["inventory"].is_null());
    assert!(preview["inventory_limitation"].is_null());
    assert_eq!(
        fs::read(
            isolated
                .path()
                .join(fixture["edition_ref"].as_str().unwrap())
        )
        .unwrap(),
        before_edition
    );
    let request = prepared_request(&preview, "native-item-first");
    let target = PathBuf::from(config["payload_root"].as_str().unwrap())
        .join(
            Path::new(config["item_source_path"].as_str().unwrap())
                .parent()
                .unwrap()
                .strip_prefix("ToS/source-witnesses")
                .unwrap(),
        )
        .join("payload")
        .join(config["payload_basename"].as_str().unwrap());
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, &original).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let foreign = cli(
        &repository,
        &owner,
        &invocation_path,
        &request,
        false,
        deadline,
    );
    assert_eq!(
        foreign["schema_version"],
        "tos_local_source_command_error_v1"
    );
    assert_eq!(fs::read(&target).unwrap(), original);
    fs::remove_file(&target).unwrap();
    fs::remove_dir_all(target.parent().unwrap().parent().unwrap()).unwrap();
    let protected = fs::read(&owner).unwrap();
    config["payload_expires_at"] = json!("2000-01-01T00:00:00Z");
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
    let expired = cli(
        &repository,
        &owner,
        &invocation_path,
        &request,
        false,
        deadline,
    );
    assert_eq!(
        expired["schema_version"],
        "tos_local_source_command_error_v1"
    );
    assert!(!target.exists());
    fs::write(&owner, &protected).unwrap();
    config = fixture["config"].clone();
    let committed = cli(
        &repository,
        &owner,
        &invocation_path,
        &request,
        true,
        deadline,
    );
    assert_eq!(committed["deposit"]["metadata_committed"], true);
    assert_eq!(committed["replayed"], false);
    assert_eq!(fs::read(&target).unwrap(), original);
    assert_eq!(fs::read(&input).unwrap(), original);
    let item_path = isolated
        .path()
        .join(config["item_source_path"].as_str().unwrap());
    let home = item_path.parent().unwrap();
    let byte_receipt: Value =
        serde_json::from_slice(&fs::read(home.join("item-deposit-receipt.json")).unwrap()).unwrap();
    assert_ne!(
        byte_receipt["private_stage_digest"],
        json!(format!("sha256:{}", "0".repeat(64)))
    );
    let event: Value =
        serde_json::from_slice(&fs::read(home.join("source-create-provenance.jsonl")).unwrap())
            .unwrap();
    assert_eq!(
        event["method"]["procedure"]["name"],
        "native-item-adoption-serialization"
    );
    let current = super::command_work_cases::authored_work_files(isolated.path());
    let current_revision =
        super::validation_cut_cases::write_cut_store_on_base(&current, &store, Some(selected));
    invocation["source_revision"] = json!(current_revision.0.to_prefixed());
    invocation["original_source_revision"] = json!(selected.0.to_prefixed());
    freeze_invocation(&invocation_path, &invocation);
    let replay = cli(
        &repository,
        &owner,
        &invocation_path,
        &request,
        true,
        deadline,
    );
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["receipt"], committed["receipt"]);
    assert_eq!(fs::read(&input).unwrap(), original);
    // The maintained fixture only refreshes its derived catalog and selects a
    // second tiny opaque input. It creates no Item metadata or ready verdict.
    let second = r#"
import json,sys,hashlib
from pathlib import Path
repository,root,recovery=map(Path,sys.argv[1:]);supplied=json.load(sys.stdin)
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_item_commands as fixture
case=fixture.NativeItemTests(methodName='runTest');case.root=root;case.config=supplied['config'];case.owner=root/'item-owner.json';case.edition_ref=supplied['edition_ref'];case.edition_path=root/case.edition_ref;case.edition=json.loads(case.edition_path.read_bytes());case.write=lambda ref,value:fixture.edition_fixture.expression_fixture.NativeExpressionTests.write(case,ref,value)
case.origin=fixture.edition_fixture.NativeEditionTests(methodName='runTest');case.origin.root=root;case.origin.expression_path=case.edition_path.parent.parent.parent/'expression.json';case.origin.extra_records=[];case.origin.extra_claims=[];case.origin.origin=fixture.edition_fixture.expression_fixture.NativeExpressionTests(methodName='runTest');case.origin.origin.work_path=case.origin.expression_path.parent.parent.parent/'work.json'
case.input=root/'opaque-input.bin';case.input.write_bytes(b'Synthetic opaque retained File.\n');case.input.chmod(0o600);case.config.update(input_path=str(case.input),media_type='application/octet-stream',byte_size=case.input.stat().st_size,sha256=hashlib.sha256(case.input.read_bytes()).hexdigest(),payload_basename='opaque.bin',original_basename='opaque.bin')
case.select_item('opaque');case.owner.chmod(0o600);case.rebuild()
# Reuse the original proposal's supplied rights posture and explicit Claim
# grammar with only the separately delegated second identities and locators.
p=supplied['proposal'];p['record'].update(record_id=case.config['item_id'],item_manifest_ref=str(Path(case.config['item_source_path']).with_name('item.manifest.json')));p['claim'].update(claim_id=case.config['claim_id'],object=case.config['item_id'],provenance_event_ref=case.config['provenance_event_id'],evidence_refs=[case.edition_ref,case.config['item_source_path']]);p['rights'].update(rights_id=case.config['rights_id'],scope_refs=[case.config['item_id'],case.config['file_id']]);p['item_forms'][0]['form_id']=case.config['allowed_item_form_ids'][0];p['claim_forms'][0]['form_id']=case.config['allowed_claim_form_ids'][0]
print(json.dumps({'config':case.config,'proposal':p,'input':str(case.input)},ensure_ascii=False,separators=(',',':')))
"#;
    let second_fixture = python(
        &repository,
        isolated.path(),
        &recovery,
        second,
        Some(&fixture),
        deadline,
    );
    config = second_fixture["config"].clone();
    let before_second = fs::read(
        isolated
            .path()
            .join(fixture["edition_ref"].as_str().unwrap()),
    )
    .unwrap();
    let after_catalog = super::command_work_cases::authored_work_files(isolated.path());
    let second_revision = super::validation_cut_cases::write_cut_store_on_base(
        &after_catalog,
        &store,
        Some(current_revision),
    );
    invocation["source_revision"] = json!(second_revision.0.to_prefixed());
    invocation["original_source_revision"] = Value::Null;
    freeze_invocation(&invocation_path, &invocation);
    let opaque_preview = cli(
        &repository,
        &owner,
        &invocation_path,
        &second_fixture["proposal"],
        true,
        deadline,
    );
    assert!(opaque_preview["inventory"].is_null());
    assert!(!opaque_preview["inventory_limitation"].is_null());
    let opaque_request = prepared_request(&opaque_preview, "native-item-byte-only");
    let bytes_only = cli(
        &repository,
        &owner,
        &invocation_path,
        &opaque_request,
        true,
        deadline,
    );
    assert_eq!(bytes_only["deposit"]["metadata_committed"], false);
    assert!(bytes_only["receipt"].is_null());
    assert!(
        !isolated
            .path()
            .join(config["item_source_path"].as_str().unwrap())
            .exists()
    );
    assert_eq!(
        fs::read(
            isolated
                .path()
                .join(fixture["edition_ref"].as_str().unwrap())
        )
        .unwrap(),
        before_second
    );
    config["allowed_operations"] = json!(["item.adoption.recover"]);
    config["authority_ref"] = json!("test-only:current-native-byte-recovery");
    config["payload_authority_ref"] = json!("test-only:current-native-payload-recovery");
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut recovery_request = json!({"schema_version":"tos_local_item_adoption_command_v1","operation":"item.adoption.recover","decision":"resume","transaction_id":bytes_only["transaction_id"],"expected_configuration":configured_digest(&config)});
    let resumed = cli(
        &repository,
        &owner,
        &invocation_path,
        &recovery_request,
        true,
        deadline,
    );
    assert_eq!(resumed["deposit"]["state"], "deposited");
    assert_eq!(resumed["deposit"]["metadata_committed"], false);
    recovery_request["decision"] = json!("rollback");
    let rolled = cli(
        &repository,
        &owner,
        &invocation_path,
        &recovery_request,
        true,
        deadline,
    );
    assert_eq!(rolled["deposit"]["state"], "rolled-back-retained");
    let opaque_target = PathBuf::from(config["payload_root"].as_str().unwrap())
        .join(
            Path::new(config["item_source_path"].as_str().unwrap())
                .parent()
                .unwrap()
                .strip_prefix("ToS/source-witnesses")
                .unwrap(),
        )
        .join("payload")
        .join(config["payload_basename"].as_str().unwrap());
    assert_eq!(
        fs::read(&opaque_target).unwrap(),
        fs::read(second_fixture["input"].as_str().unwrap()).unwrap()
    );
    assert_eq!(fs::read(&target).unwrap(), original);
    assert_eq!(fs::read(&input).unwrap(), original);
    // Existing maintained interruption controls create a genuine retained
    // source recipe. The separate native owner reconstructs every buffer and
    // consumes the current explicit recovery grant for resume/rollback.
    for decision in ["resume", "rollback", "orphan"] {
        let branch = ItemFailureFixture::new();
        let selected_root =
            IsolatedCreationRoot::create(branch.path(), deadline, &cancelled).unwrap();
        let recovery = branch.path().join("item-private-recovery");
        fs::create_dir(&recovery).unwrap();
        fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700)).unwrap();
        let fixture = python(
            &repository,
            selected_root.path(),
            &recovery,
            FACTORY,
            None,
            deadline,
        );
        fixture_preflight(selected_root.path(), &recovery, &fixture);
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        let mut config = fixture["config"].clone();
        if decision == "rollback" {
            config["payload_root"] = json!(selected_root.path().join("ToS/source-witnesses"));
            fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
        }
        let before = fs::read(
            selected_root
                .path()
                .join(fixture["edition_ref"].as_str().unwrap()),
        )
        .unwrap();
        let authored = super::command_work_cases::authored_work_files(selected_root.path());
        let mut files = authored.clone();
        for reference in fixture["implementations"].as_array().unwrap() {
            let reference = reference.as_str().unwrap();
            files.insert(
                reference.into(),
                fs::read(selected_root.path().join(reference)).unwrap(),
            );
        }
        let (capture, _software, components) =
            super::command_record_cases::captured_components(&files, deadline, &cancelled);
        let store = branch.path().join("item-recovery-cut");
        let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
        let mut invocation = invocation.clone();
        invocation["owner_config"] = json!(owner);
        invocation["corpus_store"] = json!(store);
        invocation["source_revision"] = json!(selected.0.to_prefixed());
        invocation["original_source_revision"] = Value::Null;
        invocation["software_capture"] = json!(capture.capture);
        invocation["software_restored_root"] = json!(capture.restored);
        invocation["software_selection"] = json!({"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()});
        invocation["software_components"] = json!(
            components
                .members()
                .map(|m| m.path.as_str())
                .collect::<Vec<_>>()
        );
        let invocation_path = branch.path().join("native-item-recovery-invocation.json");
        freeze_invocation(&invocation_path, &invocation);
        let preview = cli(
            &repository,
            &owner,
            &invocation_path,
            &fixture["proposal"],
            true,
            deadline,
        );
        let original_request =
            prepared_request(&preview, &format!("native-item-{decision}-recovery"));
        let interrupted = r#"
import json,sys,subprocess
from pathlib import Path
from unittest.mock import patch
repository,root,recovery=map(Path,sys.argv[1:]);supplied=json.load(sys.stdin)
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_expression_commands as fixture
import source_item_commands as item
import source_commands as commands
owner=root/'item-owner.json'
if supplied['decision']=='orphan':
    original=item.transactions._publish_state
    def interrupt(root,state,previous):
        if state['phase']=='pending':raise OSError('existing synthetic pre-pending interruption')
        return original(root,state,previous)
    with patch.object(item.transactions,'_publish_state',interrupt):
        try:commands.run_legacy_oracle_command(owner,supplied['request'])
        except OSError:pass
        else:raise AssertionError('actual maintained orphan interruption absent')
    observed=item.transactions.inspect_transaction(root,item._transaction_id(supplied['request']))
    assert observed['status']=='orphan'
else:
    result=subprocess.run([sys.executable,'-c',fixture.CRASH_WRITER,str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(owner),'2'],input=json.dumps(supplied['request']),text=True,capture_output=True,timeout=50)
    assert result.returncode==86,(result.returncode,result.stdout,result.stderr)
    observed=item.transactions.read_pending_transaction(root)
    assert observed is not None
print(json.dumps({'transaction_id':item._transaction_id(supplied['request'])}))
"#;
        let stopped = python(
            &repository,
            selected_root.path(),
            &recovery,
            interrupted,
            Some(&json!({"request":original_request,"decision":decision})),
            deadline,
        );
        config["allowed_operations"] = json!(["item.adoption.recover"]);
        config["authority_ref"] = json!("test-only:current-exact-native-metadata-recovery");
        fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
        let recovery_request = json!({"schema_version":"tos_local_item_adoption_command_v1","operation":"item.adoption.recover","decision":if decision=="rollback"{"rollback"}else{"resume"},"transaction_id":stopped["transaction_id"],"expected_configuration":configured_digest(&config)});
        let result = cli(
            &repository,
            &owner,
            &invocation_path,
            &recovery_request,
            true,
            deadline,
        );
        let original_bytes = fs::read(fixture["input"].as_str().unwrap()).unwrap();
        let target = PathBuf::from(config["payload_root"].as_str().unwrap())
            .join(
                Path::new(config["item_source_path"].as_str().unwrap())
                    .parent()
                    .unwrap()
                    .strip_prefix("ToS/source-witnesses")
                    .unwrap(),
            )
            .join("payload")
            .join(config["payload_basename"].as_str().unwrap());
        assert_eq!(fs::read(&target).unwrap(), original_bytes);
        if decision == "rollback" {
            assert!(result["receipt"].is_null());
            assert_eq!(result["deposit"]["state"], "rolled-back-retained");
            assert!(
                !selected_root
                    .path()
                    .join(config["item_source_path"].as_str().unwrap())
                    .exists()
            );
            assert_eq!(
                fs::read(
                    selected_root
                        .path()
                        .join(fixture["edition_ref"].as_str().unwrap())
                )
                .unwrap(),
                before
            );
        } else {
            assert_eq!(result["deposit"]["metadata_committed"], true);
            assert!(!result["receipt"].is_null());
            assert!(
                selected_root
                    .path()
                    .join(config["item_source_path"].as_str().unwrap())
                    .exists()
            );
        }
    }
}
