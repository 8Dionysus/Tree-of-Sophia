//! Non-OCR owner-local initial TextLayer through the maintained default CLI.
//! Recovery reconstructs its genuine completed native stage, without a crash claim.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn assert_authored_text_unchanged(root: &Path, expected: &BTreeMap<String, Vec<u8>>) {
    let actual = super::command_text_cases::authored_text_files(root);
    let paths = actual
        .keys()
        .chain(expected.keys())
        .collect::<std::collections::BTreeSet<_>>();
    let changed = paths
        .into_iter()
        .filter(|path| actual.get(*path) != expected.get(*path))
        .collect::<Vec<_>>();
    assert!(
        changed.is_empty(),
        "authored Text bytes changed at: {changed:?}"
    );
}

fn text_fixture(
    repository: &Path,
    root: &Path,
    output: &Path,
    errors: &Path,
    deadline: Instant,
) -> Value {
    let script = r#"
import json,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts'),str(repository/'tests')]
import test_source_text_unit_commands as unit
import test_source_text_layer_commands as layer
class ExistingRoot:
    def __init__(self,*args,**kwargs): self.name=str(root)
    def cleanup(self): pass
original=unit.tempfile.TemporaryDirectory
unit.tempfile.TemporaryDirectory=ExistingRoot
try:
    case=layer.NativeLayerCommandTests(methodName='runTest')
    case.setUp()
finally:
    unit.tempfile.TemporaryDirectory=original
print(json.dumps({'public':str(case.public),'private':str(case.store),
    'context':str(case.context_path),'owner':str(case.owner),
    'source_ref':case.source_ref,'payload':str(case.payload),'content_sha256':layer.source._digest(case.content)[7:],
    'content_bytes':len(case.content),'config':case.config,
    'unit_config':case.seed.config,'unit_proposal':case.seed.proposal,
    'unit_layer_schema':unit.native.LAYER_CONFIG,
    'implementations':sorted(set(layer.layers.IMPLEMENTATIONS))},ensure_ascii=False,separators=(',',':')))
"#;
    fixture_json(repository, root, output, errors, deadline, script)
}

fn fixture_json(
    repository: &Path,
    root: &Path,
    output: &Path,
    errors: &Path,
    deadline: Instant,
    script: &str,
) -> Value {
    let mut child = Command::new(crate::maintained_python())
        .args(["-c", script])
        .arg(repository)
        .arg(root)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::from(fs::File::create(output).unwrap()))
        .stderr(Stdio::from(fs::File::create(errors).unwrap()))
        .spawn()
        .unwrap();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline
            || fs::metadata(output).unwrap().len() > 1_048_576
            || fs::metadata(errors).unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded maintained Text fixture refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < deadline);
    assert!(fs::metadata(output).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(errors).unwrap().len() <= 1_048_576);
    assert!(
        status.success(),
        "{}",
        String::from_utf8_lossy(&fs::read(errors).unwrap())
    );
    serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
}

fn package_files(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut total = 0usize;
    let files: BTreeMap<_, _> = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.path().symlink_metadata().unwrap();
            assert!(metadata.is_file() && metadata.len() <= 8_388_608);
            assert_eq!(metadata.mode() & 0o777, 0o600);
            let raw = fs::read(entry.path()).unwrap();
            total = total.checked_add(raw.len()).unwrap();
            assert!(total <= 12_582_912);
            (entry.file_name().to_str().unwrap().to_owned(), raw)
        })
        .collect();
    assert!(!files.is_empty() && files.len() <= 12);
    files
}

#[test]
fn native_owner_text_cli_extracts_replays_and_recovers_completed_stage() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    // Admitted E/C/W stay in their immutable locations; no image copies.
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let before_images: Vec<_> = images.iter().map(|p| custody(p)).collect();
    let fixture = text_fixture(
        &repository,
        temporary.path(),
        &temporary.path().join("fixture.stdout"),
        &temporary.path().join("fixture.stderr"),
        deadline,
    );
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let private = PathBuf::from(fixture["private"].as_str().unwrap());
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let source_ref = fixture["source_ref"].as_str().unwrap();
    let payload = PathBuf::from(fixture["payload"].as_str().unwrap());
    assert!(fs::metadata(&payload).unwrap().len() <= 8_388_608);
    let payload_raw = fs::read(&payload).unwrap();
    let context_raw = fs::read(&context).unwrap();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in fixture["implementations"].as_array().unwrap() {
        let reference = reference.as_str().unwrap();
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 8_388_608);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,"assessment_schema_worker":null,
        "native_executable":images[1],"native_executable_sha256":before_images[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),
        "original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":before_images[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
            "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("native-text-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let observe = |request: &Value| {
        super::command_text_cases::native_owner_cli_observation(
            &repository,
            &owner,
            &invocation_path,
            request,
            deadline,
        )
    };
    let invoke = |request: &Value| -> Value {
        let (status, raw, errors) = observe(request);
        assert!(
            status.success(),
            "Text CLI: {}",
            String::from_utf8_lossy(&errors)
        );
        let envelope: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(
            envelope["schema_version"],
            "tos_local_native_source_result_v1"
        );
        assert_eq!(envelope["authentication"], "local-unix-account");
        assert_eq!(envelope["grants_admission"], false);
        let result = envelope["result"].clone();
        assert_eq!(
            result["schema_version"],
            "tos_local_text_layer_create_result_v1"
        );
        assert_eq!(result["content_disclosure"], "withheld");
        assert_eq!(result["grants_admission"], false);
        result
    };
    let described = invoke(&serde_json::json!({"operation":"describe"}));
    assert_eq!(described["target_exists"], false);
    assert_eq!(described["receipt_sha256"], Value::Null);
    let preview = invoke(&serde_json::json!({"operation":"prepare-create"}));
    assert_eq!(
        preview["owner_configuration"],
        described["owner_configuration"]
    );
    let request = serde_json::json!({"schema_version":"tos_local_source_command_v1",
        "operation":"text-layer.create","command_id":"native-owner-text-completed-stage",
        "expected_configuration":preview["owner_configuration"],
        "expected_dependencies":preview["expected_dependencies"],
        "expected_source":null,"expected_revision":null});
    let created = invoke(&request);
    assert_eq!(created["replayed"], false);
    let package = private.join(source_ref).parent().unwrap().to_path_buf();
    assert_eq!(fs::metadata(&package).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(&package).unwrap().uid(),
        fs::metadata(&private).unwrap().uid()
    );
    let retained = package_files(&package);
    let content = &retained["content.txt"];
    assert_eq!(
        content.len() as u64,
        fixture["content_bytes"].as_u64().unwrap()
    );
    assert_eq!(
        Digest256::of_bytes(content).to_hex(),
        fixture["content_sha256"].as_str().unwrap()
    );
    assert_eq!(
        created["receipt_sha256"],
        Digest256::of_bytes(
            retained["source-create-receipt.json"]
                .strip_suffix(b"\n")
                .expect("canonical receipt line")
        )
        .to_prefixed()
    );
    assert!(!public.join(source_ref).exists());
    assert_authored_text_unchanged(&public, &authored);
    // Fresh CLI process, same absolute protected root/config and same producer.
    let replay = invoke(&request);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["receipt_sha256"], created["receipt_sha256"]);
    assert_eq!(package_files(&package), retained);
    let after_description = invoke(&serde_json::json!({"operation":"describe"}));
    assert_eq!(after_description["target_exists"], true);
    assert_eq!(after_description["receipt_sha256"], Value::Null);
    let controls: Vec<_> = fs::read_dir(&private)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(".native-construction-")
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .ends_with(".pending")
        })
        .collect();
    assert_eq!(controls.len(), 1);
    let control = &controls[0];
    let plan = fs::read(control.join("plan.json")).unwrap();
    assert!(plan.len() <= 18_874_368);
    let output = control.join("output");
    assert!(!output.exists());
    // Genuine completed native package and original plan are moved back to
    // the existing pre-rename boundary; no provenance/control is fabricated.
    fs::rename(&package, &output).unwrap();
    fs::File::open(package.parent().unwrap())
        .unwrap()
        .sync_all()
        .unwrap();
    fs::File::open(control).unwrap().sync_all().unwrap();
    let owner_raw = fs::read(&owner).unwrap();
    let mut revoked: Value = serde_json::from_slice(&owner_raw).unwrap();
    revoked["allowed_operations"] = serde_json::json!([]);
    fs::write(&owner, serde_json::to_vec(&revoked).unwrap()).unwrap();
    assert!(!observe(&request).0.success());
    assert_eq!(package_files(&output), retained);
    assert_eq!(fs::read(control.join("plan.json")).unwrap(), plan);
    assert!(!package.exists());
    assert_authored_text_unchanged(&public, &authored);
    assert_eq!(fs::read(&context).unwrap(), context_raw);
    assert_eq!(fs::read(&payload).unwrap(), payload_raw);
    fs::write(&owner, &owner_raw).unwrap();
    fs::write(output.join("content.txt"), b"changed staged content").unwrap();
    let changed = package_files(&output);
    assert!(!observe(&request).0.success());
    assert_eq!(package_files(&output), changed);
    assert_eq!(fs::read(control.join("plan.json")).unwrap(), plan);
    assert!(!package.exists());
    assert_authored_text_unchanged(&public, &authored);
    assert_eq!(fs::read(&context).unwrap(), context_raw);
    assert_eq!(fs::read(&payload).unwrap(), payload_raw);
    fs::write(output.join("content.txt"), content).unwrap();
    let recovered = invoke(&request);
    assert_eq!(recovered["replayed"], true);
    assert_eq!(recovered["receipt_sha256"], created["receipt_sha256"]);
    assert_eq!(package_files(&package), retained);
    assert!(!output.exists());
    assert_eq!(fs::read(control.join("plan.json")).unwrap(), plan);
    assert_authored_text_unchanged(&public, &authored);
    assert_eq!(fs::read(&owner).unwrap(), owner_raw);
    assert_eq!(fs::read(&context).unwrap(), context_raw);
    assert_eq!(fs::read(&payload).unwrap(), payload_raw);
    assert_eq!(
        images.iter().map(|p| custody(p)).collect::<Vec<_>>(),
        before_images
    );
    assert!(Instant::now() < deadline);
}

/// The maintained Python fixture supplies synthetic authored input only.
/// Assessment/journal operations below always cross the captured native CLI.
#[test]
fn native_private_assessment_v4_append_replay_and_revocation_preserve_native_bytes() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let script = r#"
import json,sys,tempfile,unittest
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.dont_write_bytecode=True
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts'),str(repository/'tests')]
from datetime import datetime,timedelta,timezone
import test_knowledge_assessment as policy_fixture
# Native commands use real time; retain a finite synthetic grant window.
policy_fixture.END=(datetime.now(timezone.utc)+timedelta(days=7)).isoformat()
import test_owner_local_assessment as maintained
class ExistingRoot:
    serial=0
    def __init__(self,*args,**kwargs):
        type(self).serial+=1
        path=root/('synthetic-owner-'+str(type(self).serial))
        path.mkdir(mode=0o700)
        self.name=str(path)
    def cleanup(self): pass
    def __enter__(self): return self.name
    def __exit__(self,*args): pass
original=tempfile.TemporaryDirectory
try:
    tempfile.TemporaryDirectory=ExistingRoot
    fixture=maintained.OwnerLocalAssessmentFixture(unittest.TestCase(methodName='runTest'))
    # Native assessment uses its real clock; renew only synthetic fixture grants.
    for grant in fixture.config['authorities']+fixture.config['competencies']:
        grant['payload']['valid_until']='2099-01-01T00:00:00Z'
    fixture._grants()
    fixture.save()
finally:
    tempfile.TemporaryDirectory=original
assert fixture.native.packet['reviews']==[]
print(json.dumps({'public':str(fixture.public),'private':str(fixture.private),
    'context':str(fixture.context_path),'owner':str(fixture.owner),
    'subject_id':fixture.identifier,'request':fixture.request(),
    'preserved':[str(fixture.packet_path),str(fixture.content_path),str(fixture.public/fixture.native.layer_ref)],
    'absent_original':str(fixture.public/fixture.native.original_ref),
    'forbidden':[fixture.native.text[3:8],fixture.content_ref,fixture.binding['packet_ref'],
        fixture.binding['packet_sha256'],fixture.binding['text_layer']['record_sha256'],
        'ordered_anchor_refs','exact_sha256','source_record_refs']},ensure_ascii=False,separators=(',',':')))
"#;
    let fixture = fixture_json(
        &repository,
        temporary.path(),
        &temporary.path().join("assessment-fixture.stdout"),
        &temporary.path().join("assessment-fixture.stderr"),
        deadline,
        script,
    );
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let before_images: Vec<_> = images.iter().map(|path| custody(path)).collect();
    let preserved: BTreeMap<PathBuf, Vec<u8>> = fixture["preserved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| {
            let path = PathBuf::from(path.as_str().unwrap());
            assert!(path.symlink_metadata().unwrap().is_file());
            assert!(fs::metadata(&path).unwrap().len() <= 8_388_608);
            let raw = fs::read(&path).unwrap();
            (path, raw)
        })
        .collect();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    // Bounded changed feature sources, not a handler-mandated component schema.
    // The selected executable digest independently binds the actual native code.
    for reference in assessment_feature_sources() {
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 2_097_152);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
        assert!(Instant::now() < deadline);
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("assessment-source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,
        "native_executable":images[1],"native_executable_sha256":before_images[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),
        "original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":before_images[2].4.to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":before_images[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
            "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("assessment-native-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let invoke = |request: &Value| -> Value {
        let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
            &repository,
            &owner,
            &invocation_path,
            request,
            deadline,
        );
        assert!(
            status.success(),
            "private assessment CLI: {}",
            String::from_utf8_lossy(&errors)
        );
        let response: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(response["schema_version"], "tos_local_assessment_result_v1");
        assert_eq!(response["visibility"], "local_only");
        assert_eq!(response["publication_authorized"], false);
        for forbidden in fixture["forbidden"].as_array().unwrap() {
            assert!(
                !String::from_utf8_lossy(&raw).contains(forbidden.as_str().unwrap()),
                "private native evidence disclosed in result"
            );
        }
        assert!(Instant::now() < deadline);
        response
    };
    let describe = serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
        "operation":"describe","subject_id":fixture["subject_id"]});
    let described = invoke(&describe);
    let mut request = fixture["request"].clone();
    // Currentness comes from the selected native owner, not fixture/oracle output.
    request["expected_snapshot"] = described["owner_snapshot"].clone();
    let committed = invoke(&request);
    assert_eq!(committed["result"]["current_admission"]["can_use"], true);
    assert_eq!(
        committed["result"]["receipt"]["events"][0]["assessment"]["reviewer"]["kind"],
        "agent"
    );
    let replay = invoke(&request);
    assert_eq!(replay["result"]["replayed"], true);
    assert_eq!(replay["result"]["receipt"], committed["result"]["receipt"]);
    let mut config: Value = serde_json::from_slice(&fs::read(&owner).unwrap()).unwrap();
    let grant = &mut config["authorities"][0];
    grant["version"] = Value::from(grant["version"].as_u64().unwrap().checked_add(1).unwrap());
    grant["payload"]["authority_version"] = Value::from(
        grant["payload"]["authority_version"]
            .as_u64()
            .unwrap()
            .checked_add(1)
            .unwrap(),
    );
    grant["payload"]["state"] = Value::from("revoked");
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    request["expected_snapshot"] = invoke(&describe)["owner_snapshot"].clone();
    let revoked = invoke(&request);
    assert_eq!(revoked["result"]["replayed"], true);
    assert_eq!(
        revoked["result"]["receipt"]["admission_at_commit"]["can_use"],
        true
    );
    assert_eq!(revoked["result"]["current_admission"]["can_use"], false);
    for (path, raw) in preserved {
        assert_eq!(fs::read(path).unwrap(), raw);
    }
    assert!(!Path::new(fixture["absent_original"].as_str().unwrap()).exists());
    for (index, path) in images.iter().enumerate() {
        assert_eq!(custody(path), before_images[index]);
    }
    assert!(Instant::now() < deadline);
}

fn assessment_feature_sources() -> [&'static str; 13] {
    [
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
        "rust/crates/tos-command/src/lib.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_native_private_cli.rs",
        "rust/crates/tos-command/src/source_native_private_assessment_cli.rs",
        "rust/crates/tos-command/src/source_assessment_journal.rs",
        "rust/crates/tos-command/src/source_private_assessment_sources.rs",
        "rust/crates/tos-command/src/source_private_assessment_layers.rs",
        "rust/crates/tos-command/src/source_private_claim.rs",
        "rust/crates/tos-command/src/source_sign.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-validation/src/assessment.rs",
    ]
}

fn native_layer_journal_fixture(
    repository: &Path,
    root: &Path,
    deadline: Instant,
    derived: bool,
) -> Value {
    let template = r#"
import copy,json,sys,tempfile,unittest
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.dont_write_bytecode=True
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts'),str(repository/'tests')]
from datetime import datetime,timedelta,timezone
import test_knowledge_assessment as policy_fixture
# Native commands use real time; retain a finite synthetic grant window.
policy_fixture.END=(datetime.now(timezone.utc)+timedelta(days=7)).isoformat()
class ExistingRoot:
    serial=0
    def __init__(self,*args,**kwargs):
        type(self).serial+=1
        path=root/('synthetic-layer-'+str(type(self).serial))
        path.mkdir(mode=0o700)
        self.name=str(path)
    def cleanup(self): pass
    def __enter__(self): return self.name
    def __exit__(self,*args): pass
original=tempfile.TemporaryDirectory
try:
    tempfile.TemporaryDirectory=ExistingRoot
    test=unittest.TestCase(methodName='runTest')
    import test_native_layer_quality_journal as maintained
    if JOURNAL_DERIVED_METHOD:
        from test_native_text_layer_assessment import NativeDerivedLayerAssessmentFixture
        # Late handler imports must use this exact fixture source tree.
        sys.path[:0]=[str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
        layer=NativeDerivedLayerAssessmentFixture(test)
        layer.member=layer.seed.member
        layer.payload=layer.seed.payload
        fx=maintained.QualityJournalFixture(test,layer_fixture=layer)
    else:
        fx=maintained.QualityJournalFixture(test)
    # Keep these synthetic native grants finite and independent of Python's fixed NOW.
    for grant in fx.config['authorities']+fx.config['competencies']:
        grant['payload']['valid_until']='2099-01-01T00:00:00Z'
    for index,competency in enumerate(fx.config['competencies']):
        fx.config['authorities'][index]['payload']['competence_refs']=[maintained.Record.from_payload(**competency).ref]
    fx.save()
    assert fx.fx.layer['admission']['human_review_performed'] is False
    index=next(i for i,g in enumerate(fx.config['authorities']) if g['payload']['actor_id']==fx.config['principal_id'])
    def template(record,profile,name):
        review=copy.deepcopy(fx.policy_fixture.review(index).assessment)
        review.update(assessment_id='tos.review.'+name,subject=record.ref,policy=fx.policy.ref,profile_id=profile,
            decision='admit',limits=[],supersedes=[],authority=maintained.Record.from_payload(**fx.config['authorities'][index]).ref,
            competence=maintained.Record.from_payload(**fx.config['competencies'][index]).ref,evidence=[])
        return {'schema_version':'tos_local_assessment_command_v1','operation':'append','subject_id':record.id,
            'expected_subject':record.ref,'command_id':name,'assessments':[review]}
    result={'public':str(fx.fx.public),'context':str(fx.fx.context_path),'owner':str(fx.owner),
        'layer_subject':fx.layer_record.ref,'unit_subject':fx.unit.ref,
        'layer_template':template(fx.layer_record,'text-layer-quality','synthetic-native-layer-admit'),
        'unit_template':template(fx.unit,'source-observation','synthetic-native-unit-admit'),
        'source_member':fx.fx.member.decode(),'content':fx.fx.content.decode(),
        'derived':JOURNAL_DERIVED_METHOD,
        'preserved':([str(path) for path in sorted(fx.fx.store.rglob('*')) if path.is_file() and fx.journal not in path.parents] if JOURNAL_DERIVED_METHOD else [str(fx.fx.store/fx.fx.source_ref),str(fx.fx.store/fx.packet_ref),str(fx.fx.payload)])}
finally:
    tempfile.TemporaryDirectory=original
print(json.dumps(result,ensure_ascii=False,separators=(',',':')))
"#;
    let script = template.replace(
        "JOURNAL_DERIVED_METHOD",
        if derived { "True" } else { "False" },
    );
    fixture_json(
        repository,
        root,
        &root.join("layer-fixture.stdout"),
        &root.join("layer-fixture.stderr"),
        deadline,
        &script,
    )
}

fn native_layer_journal_case(derived: bool) {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let fixture = native_layer_journal_fixture(&repository, temporary.path(), deadline, derived);
    eprintln!(
        "native layer journal cost: phase=fixture elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let image_before: Vec<_> = images.iter().map(|path| custody(path)).collect();
    eprintln!(
        "native layer journal cost: phase=image-custody elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let preserved: BTreeMap<PathBuf, Vec<u8>> = fixture["preserved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let path = PathBuf::from(value.as_str().unwrap());
            assert!(path.symlink_metadata().unwrap().is_file());
            assert!(fs::metadata(&path).unwrap().len() <= 8_388_608);
            (path.clone(), fs::read(path).unwrap())
        })
        .collect();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in assessment_feature_sources() {
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 2_097_152);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
        assert!(Instant::now() < deadline);
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    eprintln!(
        "native layer journal cost: phase=software-capture elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let store = temporary.path().join("layer-journal-source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    eprintln!(
        "native layer journal cost: phase=captured elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,"native_executable":images[1],"native_executable_sha256":image_before[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":image_before[2].4.to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":image_before[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("layer-journal-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    // An unselected layer in a unit's closure remains supporting evidence.
    let configuration_raw = fs::read(&owner).unwrap();
    let mut supporting_only: Value = serde_json::from_slice(&configuration_raw).unwrap();
    supporting_only["native_text_layers"] = serde_json::json!([]);
    supporting_only["quality_dependencies"] = serde_json::json!({});
    fs::write(&owner, serde_json::to_vec(&supporting_only).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let (denied_status, _, denied_error) = super::command_text_cases::native_owner_cli_observation(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"describe","subject_id":fixture["layer_subject"]["id"]}),
        deadline,
    );
    let denied_prefix = String::from_utf8_lossy(&denied_error[..denied_error.len().min(16_384)]);
    assert!(
        !denied_status.success(),
        "supporting-only native refusal unexpectedly succeeded: status={denied_status:?} stderr_bytes={} prefix={denied_prefix}",
        denied_error.len()
    );
    assert!(
        String::from_utf8_lossy(&denied_error)
            .contains("native supporting evidence is not an assessment target"),
        "supporting-only native refusal differs: status={denied_status:?} stderr_bytes={} prefix={denied_prefix}",
        denied_error.len()
    );
    fs::write(&owner, configuration_raw).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    eprintln!(
        "native layer journal cost: phase=supporting-refusal elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let ordinal = std::cell::Cell::new(0usize);
    let invoke = |request: &Value| -> Value {
        let number = ordinal.get() + 1;
        ordinal.set(number);
        let step_started = Instant::now();
        eprintln!(
            "native layer journal cost: call={number} phase=start operation={} elapsed_ms={} remaining_ms={}",
            request["operation"].as_str().unwrap_or("<absent>"),
            started.elapsed().as_millis(),
            deadline.saturating_duration_since(step_started).as_millis()
        );
        let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
            &repository,
            &owner,
            &invocation_path,
            request,
            deadline,
        );
        eprintln!(
            "native layer journal cost: call={number} phase=terminal operation={} elapsed_ms={} child_ms={} success={}",
            request["operation"].as_str().unwrap_or("<absent>"),
            started.elapsed().as_millis(),
            step_started.elapsed().as_millis(),
            status.success()
        );
        assert!(
            status.success(),
            "native layer journal: {}",
            String::from_utf8_lossy(&errors)
        );
        let result: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(result["schema_version"], "tos_local_assessment_result_v1");
        assert_eq!(result["visibility"], "local_only");
        assert_eq!(result["publication_authorized"], false);
        assert!(Instant::now() < deadline);
        result
    };
    let describe = |subject: &Value| {
        invoke(
            &serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"describe","subject_id":subject["id"]}),
        )
    };
    let layer = describe(&fixture["layer_subject"]);
    assert!(
        layer["result"]["command_context"]["supported_operations"]
            .as_array()
            .unwrap()
            .contains(&Value::from("read-layer-comparison"))
    );
    let comparison = invoke(
        &serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"read-layer-comparison","subject_id":fixture["layer_subject"]["id"],"expected_subject":fixture["layer_subject"],"expected_snapshot":layer["owner_snapshot"]}),
    );
    assert!(
        !serde_json::to_string(&layer)
            .unwrap()
            .contains(fixture["content"].as_str().unwrap())
    );
    let comparison_payload = &comparison["result"]["source_comparison"]["payload"];
    if derived {
        assert_eq!(
            comparison_payload["schema_version"],
            "tos_native_text_layer_derivation_comparison_v1"
        );
        assert_eq!(
            comparison_payload["source_view"]["source_member_utf8"],
            fixture["source_member"]
        );
        assert_eq!(comparison_payload["inherited_quality"], "not-transferred");
        assert_eq!(comparison_payload["lineage"].as_array().unwrap().len(), 2);
        assert_eq!(
            comparison_payload["lineage"][1]["record_payload"]["admission"]["review_status"],
            "unreviewed"
        );
        assert_eq!(comparison_payload["performs_semantic_assessment"], false);
    } else {
        assert_eq!(
            comparison_payload["source_member_utf8"],
            fixture["source_member"]
        );
    }
    let append_request = |template: &Value, described: &Value| {
        let mut request = template.clone();
        request["expected_snapshot"] = described["owner_snapshot"].clone();
        request["expected_revision"] = described["result"]["revision"].clone();
        let command = &described["result"]["command_context"];
        let sources = command.get("required_sources").and_then(Value::as_array);
        let admissions = command.get("required_admissions").and_then(Value::as_array);
        request["assessments"][0]["evidence"] = Value::Array(
            sources.into_iter().flatten().cloned()
                .chain(admissions.into_iter().flatten().map(|row| row["basis"].clone()))
                .map(|reference| serde_json::json!({"record":reference,"stance":"supports","locator":"Synthetic explicit comparison or quality context."}))
                .collect());
        request
    };
    let request = append_request(&fixture["layer_template"], &layer);
    let quality = invoke(&request);
    assert_eq!(quality["result"]["current_admission"]["can_use"], true);
    assert_eq!(invoke(&request)["result"]["replayed"], true);
    let unit = describe(&fixture["unit_subject"]);
    let mut dependent_request = append_request(&fixture["unit_template"], &unit);
    let dependent = invoke(&dependent_request);
    assert_eq!(dependent["result"]["current_admission"]["can_use"], true);
    let mut withdrawal = fixture["layer_template"].clone();
    withdrawal["command_id"] = Value::from("synthetic-native-layer-withdraw");
    withdrawal["assessments"][0]["assessment_id"] =
        Value::from("tos.review.synthetic-native-layer-withdraw");
    withdrawal["assessments"][0]["decision"] = Value::from("withdraw");
    withdrawal["assessments"][0]["supersedes"] =
        serde_json::json!([quality["result"]["current_admission"]["assessment_refs"][0]]);
    let withdrawn = invoke(&append_request(
        &withdrawal,
        &describe(&fixture["layer_subject"]),
    ));
    assert_eq!(withdrawn["result"]["current_admission"]["can_use"], false);
    let closed_unit = describe(&fixture["unit_subject"]);
    assert_eq!(closed_unit["result"]["current_admission"]["can_use"], false);
    dependent_request["expected_snapshot"] = closed_unit["owner_snapshot"].clone();
    let replay = invoke(&dependent_request);
    assert_eq!(replay["result"]["replayed"], true);
    assert_eq!(
        replay["result"]["receipt"]["admission_at_commit"]["can_use"],
        true
    );
    assert_eq!(replay["result"]["current_admission"]["can_use"], false);
    let mut renewal = fixture["layer_template"].clone();
    renewal["command_id"] = Value::from("synthetic-native-layer-renewed");
    renewal["assessments"][0]["assessment_id"] =
        Value::from("tos.review.synthetic-native-layer-renewed");
    let renewed_quality = invoke(&append_request(
        &renewal,
        &describe(&fixture["layer_subject"]),
    ));
    assert_eq!(
        renewed_quality["result"]["current_admission"]["can_use"],
        true
    );
    let stale_unit = describe(&fixture["unit_subject"]);
    assert_eq!(stale_unit["result"]["current_admission"]["can_use"], false);
    dependent_request["expected_snapshot"] = stale_unit["owner_snapshot"].clone();
    let stale_replay = invoke(&dependent_request);
    assert_eq!(stale_replay["result"]["replayed"], true);
    assert_eq!(
        stale_replay["result"]["receipt"]["admission_at_commit"]["can_use"],
        true
    );
    assert_eq!(
        stale_replay["result"]["current_admission"]["can_use"],
        false
    );
    let mut reassessed = fixture["unit_template"].clone();
    reassessed["command_id"] = Value::from("synthetic-native-unit-reassessed");
    reassessed["assessments"][0]["assessment_id"] =
        Value::from("tos.review.synthetic-native-unit-reassessed");
    let new_dependent = invoke(&append_request(&reassessed, &stale_unit));
    assert_eq!(
        new_dependent["result"]["current_admission"]["can_use"],
        true
    );
    for (path, raw) in preserved {
        assert_eq!(fs::read(path).unwrap(), raw);
    }
    for (index, path) in images.iter().enumerate() {
        assert_eq!(custody(path), image_before[index]);
    }
    assert!(Instant::now() < deadline);
}

#[test]
fn native_private_assessment_v5_quality_dependency_withdrawal_preserves_source() {
    native_layer_journal_case(false);
}

#[test]
fn native_derived_layer_assessment_lineage_quality_withdrawal_preserves_source() {
    native_layer_journal_case(true);
}

#[test]
#[ignore = "requires retained signed synthetic OCR evidence and admitted native owner/worker"]
fn native_private_assessment_v6_retained_signed_ocr_comparison_preserves_source() {
    native_owner_ocr_comparison_case(false);
}

#[test]
#[ignore = "requires separately retained genuine current synthetic PageOCR producer and exact native products"]
fn native_retained_page_ocr_assessment_comparison_preserves_original_and_signed_capture() {
    native_owner_ocr_comparison_case(true);
}

fn native_owner_ocr_comparison_case(retained_page: bool) {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(if retained_page { 480 } else { 240 });
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let fixture_module = if retained_page {
        "journal_page_fixture.py"
    } else {
        "journal_v6_fixture.py"
    };
    let script = r#"
import json,runpy,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
f=runpy.run_path(str(repository/'tests/conformance/rust/__FIXTURE__'))
print(json.dumps(f['prepare'](repository,root),separators=(',',':')))
"#
    .replace("__FIXTURE__", fixture_module);
    let fixture = fixture_json(
        &repository,
        temporary.path(),
        &temporary.path().join("v6-prepare.stdout"),
        &temporary.path().join("v6-prepare.stderr"),
        deadline,
        &script,
    );
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let image_before_native: Vec<_> = images.iter().map(|path| custody(path)).collect();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in assessment_feature_sources() {
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 2_097_152);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
        assert!(Instant::now() < deadline);
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("layer-journal-source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,"native_executable":images[1],"native_executable_sha256":image_before_native[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":image_before_native[2].4.to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":image_before_native[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("layer-journal-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let observe = |selected_owner: &Path, request: &Value| {
        let mut selected_invocation = invocation.clone();
        selected_invocation["owner_config"] = Value::from(selected_owner.to_str().unwrap());
        if request["schema_version"] != "tos_local_assessment_command_v1" {
            selected_invocation["assessment_schema_worker"] = Value::Null;
        }
        fs::write(
            &invocation_path,
            serde_json::to_vec(&selected_invocation).unwrap(),
        )
        .unwrap();
        super::command_text_cases::native_owner_cli_observation(
            &repository,
            selected_owner,
            &invocation_path,
            request,
            deadline,
        )
    };
    let invocation_count = std::cell::Cell::new(0u32);
    let invoke = |selected_owner: &Path, request: &Value| -> Value {
        let ordinal = invocation_count.get() + 1;
        invocation_count.set(ordinal);
        let (status, raw, errors) = observe(selected_owner, request);
        assert!(
            status.success(),
            "native retained OCR invocation {} operation {}: {}",
            ordinal,
            request["operation"].as_str().unwrap_or("<absent>"),
            String::from_utf8_lossy(&errors)
        );
        let envelope: Value = serde_json::from_slice(&raw).unwrap();
        if request["schema_version"] == "tos_local_assessment_command_v1" {
            envelope
        } else {
            assert_eq!(
                envelope["schema_version"],
                "tos_local_native_source_result_v1"
            );
            assert_eq!(envelope["authentication"], "local-unix-account");
            assert_eq!(envelope["grants_admission"], false);
            let result = envelope["result"].clone();
            assert!(result.is_object());
            assert_eq!(
                result["schema_version"],
                "tos_local_text_layer_derive_result_v1"
            );
            assert_eq!(result["content_disclosure"], "withheld");
            assert_eq!(result["grants_admission"], false);
            assert!(result["owner_configuration"].is_string());
            if request["operation"] == "prepare-create" {
                assert!(result["expected_dependencies"].is_string());
            }
            result
        }
    };
    let prepared = invoke(
        &owner,
        &serde_json::json!({
        "operation":"prepare-create"}),
    );
    let record_request = serde_json::json!({
        "schema_version":"tos_local_source_command_v1","operation":if retained_page { "text-layer.record-owner-page-ocr" } else { "text-layer.record-owner-ocr" },
        "command_id":if retained_page { "synthetic-current-page-authenticated-ocr-record" } else { "synthetic-v6-authenticated-ocr-record" },
        "expected_configuration":prepared["owner_configuration"],
        "expected_dependencies":prepared["expected_dependencies"],
        "expected_source":null,"expected_revision":null});
    let created = invoke(&owner, &record_request);
    assert_eq!(created["replayed"], false);
    if retained_page {
        let replayed = invoke(&owner, &record_request);
        assert_eq!(replayed["replayed"], true);
        assert_eq!(replayed["receipt_sha256"], created["receipt_sha256"]);
    }
    let finish_script = r#"
import json,runpy,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
f=runpy.run_path(str(repository/'tests/conformance/rust/__FIXTURE__'))
print(json.dumps(f['finish'](repository,root),separators=(',',':')))
"#
    .replace("__FIXTURE__", fixture_module);
    let selected = fixture_json(
        &repository,
        temporary.path(),
        &temporary.path().join("v6-finish.stdout"),
        &temporary.path().join("v6-finish.stderr"),
        deadline,
        &finish_script,
    );
    let assessment_owner = PathBuf::from(selected["assessment_owner"].as_str().unwrap());
    let package = PathBuf::from(selected["package"].as_str().unwrap());
    let retained = package_files(&package);
    assert_eq!(
        Digest256::of_bytes(&retained["owner-ocr-signature.sigstore.json"]).to_hex(),
        selected["original_signature_sha256"].as_str().unwrap()
    );
    let receipt: Value = serde_json::from_slice(&retained["owner-ocr-receipt.json"]).unwrap();
    assert_eq!(
        receipt["owner"]["source_ref"],
        selected["original_owner_source_ref"]
    );
    assert_eq!(
        Digest256::of_bytes(&retained["owner-ocr-receipt.json"]).to_hex(),
        selected["original_receipt_sha256"].as_str().unwrap()
    );
    let image = PathBuf::from(selected["image_path"].as_str().unwrap());
    let image_before = custody(&image);
    let original_pdf =
        retained_page.then(|| PathBuf::from(selected["source_pdf_path"].as_str().unwrap()));
    let original_before = original_pdf.as_ref().map(|path| custody(path));
    let described = invoke(
        &assessment_owner,
        &serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1","operation":"describe",
        "subject_id":selected["subject"]["id"]}),
    );
    let request = serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1","operation":"read-layer-comparison",
        "subject_id":selected["subject"]["id"],"expected_subject":selected["subject"],
        "expected_snapshot":described["owner_snapshot"]});
    let compared = invoke(&assessment_owner, &request);
    assert_eq!(compared["schema_version"], "tos_local_assessment_result_v1");
    assert_eq!(compared["visibility"], "local_only");
    assert_eq!(compared["publication_authorized"], false);
    assert_eq!(compared["result"]["current_admission"]["can_use"], false);
    assert_eq!(compared["result"]["revision"], Value::Null);
    assert_eq!(
        compared["result"]["source_comparison"]["payload"]["source_image"]["sha256"],
        selected["image_sha256"]
    );
    if retained_page {
        let comparison = &compared["result"]["source_comparison"]["payload"];
        assert_eq!(
            comparison["owner_execution"]["input_verification"]["render_execution"],
            "not_performed"
        );
        assert_eq!(
            comparison["owner_execution"]["input_verification"]["historical_receipt_signature"],
            "absent"
        );
        assert_ne!(
            comparison["source_scope"]["file_sha256"],
            comparison["source_image"]["sha256"]
        );
        assert_eq!(
            comparison["input_representation"],
            fixture["input_representation"]
        );
    }
    assert_eq!(
        compared["result"]["source_comparison"]["payload"]["performs_semantic_assessment"],
        false
    );

    assert_eq!(
        compared["result"]["source_comparison"]["payload"]["source_image"]["model_disclosure_authorized"],
        false
    );
    let mut expired: Value = serde_json::from_slice(&fs::read(&assessment_owner).unwrap()).unwrap();
    expired["native_text_layers"][0]["image_access"]["expires_at"] =
        Value::from("2000-01-01T00:00:00Z");
    fs::write(&assessment_owner, serde_json::to_vec(&expired).unwrap()).unwrap();
    assert!(!observe(&assessment_owner, &request).0.success());
    assert_eq!(package_files(&package), retained);
    assert_eq!(custody(&image), image_before);
    if let Some(path) = original_pdf.as_ref() {
        assert_eq!(custody(path), original_before.unwrap());
    }
    assert_authored_text_unchanged(&public, &authored);
    for (index, path) in images.iter().enumerate() {
        assert_eq!(custody(path), image_before_native[index]);
    }
    assert!(Instant::now() < deadline);
}

#[test]
fn native_public_assessment_v1_v2_v3_append_replay_and_revocation_preserve_source() {
    native_public_assessment_versions(&[1, 2, 3]);
}

#[test]
fn native_public_assessment_v3_append_replay_revocation_and_metadata_scope_preserve_source() {
    native_public_assessment_versions(&[3]);
}

#[test]
fn native_public_v2_assessed_form_batch_matches_builder_and_rechecks_drift() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(600);
    let cancelled = AtomicBool::new(false);
    for (count, source_copy, ready) in [(1usize, false, false), (6, true, true)] {
        let temporary = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let fixture_script = r#"
import json,sys,tempfile
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),
    str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
from datetime import datetime,timedelta,timezone
import test_knowledge_assessment as policy_fixture
policy_fixture.END=(datetime.now(timezone.utc)+timedelta(days=7)).isoformat()
counter=0
class OwnedTemporary:
    def __init__(self,*args,**kwargs):
        global counter
        counter+=1
        directory=root/('owned-'+str(counter))
        directory.mkdir(mode=0o700)
        self.name=str(directory)
    def cleanup(self): pass
original=tempfile.TemporaryDirectory
tempfile.TemporaryDirectory=OwnedTemporary
try:
    from test_assessment_read_batch import AssessmentReadBatchTests
    case=AssessmentReadBatchTests(methodName='runTest')
    fx=case.fixture(__COUNT__,ready=__READY__,source_copy=__SOURCE_COPY__)
finally:
    tempfile.TemporaryDirectory=original
print(json.dumps({'owner':str(fx.owner),'source_root':str(fx.root),
    'journal':fx.config['journal_directory'],'form_ids':fx.ids,'forms':fx.forms,
    'source':{'id':fx.source.id,'version':fx.source.version,'payload':fx.source.payload,
        'origin_id':fx.source.origin_id,'ref':fx.source.ref},
    'source_path':fx.config['source_records'][0]['path'],
    'form_path':fx.config['source_records'][1]['path'],'nodes':fx.nodes},
    ensure_ascii=False,separators=(',',':')))
"#
            .replace("__COUNT__", &count.to_string())
            .replace("__READY__", if ready { "True" } else { "False" })
            .replace(
                "__SOURCE_COPY__",
                if source_copy { "True" } else { "False" },
            );
        let fixture = fixture_json(
            &repository,
            temporary.path(),
            &temporary.path().join("public-read-fixture.stdout"),
            &temporary.path().join("public-read-fixture.stderr"),
            deadline,
            &fixture_script,
        );
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        let source_root = PathBuf::from(fixture["source_root"].as_str().unwrap());
        let invocation_path = temporary
            .path()
            .join("native-assessment-read-invocation.json");
        let manifest_path = temporary
            .path()
            .join("native-assessment-read-manifest.json");
        fs::write(&manifest_path, serde_json::to_vec(&fixture).unwrap()).unwrap();
        let images = [
            std::env::current_exe().unwrap(),
            PathBuf::from(
                std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
            ),
            PathBuf::from(
                std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("native worker required"),
            ),
        ];
        let before: Vec<_> = images
            .iter()
            .map(|path| super::command_text_cases::alignment_image_digest(path))
            .collect();
        let authored = super::command_text_cases::authored_text_files(&source_root);
        let mut captured = authored.clone();
        for reference in assessment_feature_sources().into_iter().chain([
            "scripts/source_witness_human_forms.py",
            "scripts/validate_tree_node_contracts.py",
            "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py",
            "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
            "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
            "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
            "rust/crates/tos-command/src/source_forms.rs",
            "rust/crates/tos-validation/src/source_forms/source_copy_kernel.rs",
        ]) {
            let path = repository.join(reference);
            assert!(path.is_file() && fs::metadata(&path).unwrap().len() <= 2_097_152);
            assert!(
                captured
                    .insert(reference.to_owned(), fs::read(path).unwrap())
                    .is_none()
            );
        }
        assert!(
            captured.len() <= 2048 && captured.values().map(Vec::len).sum::<usize>() <= 33_554_432
        );
        let (capture, _software, components) =
            super::command_record_cases::captured_components(&captured, deadline, &cancelled);
        let store = temporary.path().join("public-assessment-read-cut");
        let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
        let invocation = serde_json::json!({
            "schema_version":"tos_local_native_assessment_read_invocation_v1",
            "owner_config":owner,
            "native_executable":images[1],
            "native_executable_sha256":before[1].to_prefixed(),
            "corpus_store":store,
            "source_revision":selected.0.to_prefixed(),
            "original_source_revision":selected.0.to_prefixed(),
            "software_capture":capture.capture,
            "software_restored_root":capture.restored,
            "software_selection":{"source_git_commit":capture.selection.source_git_commit,
                "source_git_tree":capture.selection.source_git_tree,
                "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":images[2],"sha256":before[2].to_prefixed()},
            "assessment_schema_worker":{"absolute_path":images[2],"sha256":before[2].to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
                "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
                "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}
        });
        fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();

        let exercise = r#"
import copy,json,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),
    str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
from source_witness_human_forms import AssessedFormSnapshot,write_assessed_candidate
from assessment_journal import AssessmentJournal,JournalConflict,run_legacy_oracle_command
from knowledge_assessment import Record,SubjectContext
manifest=json.loads((root/'native-assessment-read-manifest.json').read_text())
owner=Path(manifest['owner']); ids=manifest['form_ids']; invocation=root/'native-assessment-read-invocation.json'
nodes=manifest['nodes']; original=copy.deepcopy(nodes)
journal=Path(manifest['journal'])
def files(path):
    return {entry.relative_to(path).as_posix():entry.read_bytes()
        for entry in path.rglob('*') if entry.is_file()}
def normalize(nodes):
    result=copy.deepcopy(nodes)
    for node in result:
        for packet in node.get('properties',{}).get('human_forms',[]):
            if packet.get('form',{}).get('id') in ids:
                packet['assessment_snapshot']['owner_snapshot']='same-current-snapshot'
    return result
def selected_packets(nodes):
    return [packet for node in nodes for packet in node.get('properties',{}).get('human_forms',[])
        if packet.get('form',{}).get('id') in ids]
journal_before=files(journal)
source_before=(Path(manifest['source_root'])/manifest['source_path']).read_bytes()
form_before=(Path(manifest['source_root'])/manifest['form_path']).read_bytes()
retained=AssessedFormSnapshot.for_retained_reference_fixture(owner,ids)
expected=retained.materialize(copy.deepcopy(nodes)); retained.verify_current()
native=AssessedFormSnapshot(owner,ids,native_invocation=invocation)
actual=native.materialize(copy.deepcopy(nodes)); native.verify_current()
assert normalize(actual)==normalize(expected), 'native batch differs from retained public-v2 read'
assert nodes==original, 'batch changed its caller-owned graph'
assert files(journal)==journal_before, 'a read batch wrote or locked the journal'
assert (Path(manifest['source_root'])/manifest['source_path']).read_bytes()==source_before
assert (Path(manifest['source_root'])/manifest['form_path']).read_bytes()==form_before
packets=selected_packets(actual)
assert len(packets)==len(ids)
if len(ids)==1:
    assert packets[0]['state']=='needs-assessment' and packets[0]['display_text'] is None
else:
    assert len(ids)==6 and all(packet['state']=='ready' and packet['admission']['can_use'] for packet in packets)

# Every bad final selection goes through the real native batch consumer and
# rejects the whole request. Cross-subject/path binding is independent of an
# otherwise valid first row; the journal and authored source stay untouched.
for mutation in ('version','subject','source-path','form-path','duplicate'):
    bad=copy.deepcopy(native._native_request)
    final=bad['selections'][-1]
    if mutation=='version':
        final['form_ref']['version']+=1
    elif mutation=='subject':
        final['subject_ref']=copy.deepcopy(final['form_ref'])
    elif mutation=='source-path':
        final['source_path']='ToS/source-witnesses/another/source.json'
    elif mutation=='form-path':
        final['form_path']='ToS/source-witnesses/another/source.human-forms.json'
    else:
        bad['selections'].append(copy.deepcopy(final))
    try:
        native._run_native_materialization(bad)
    except (ValueError,PermissionError,JournalConflict):
        pass
    else:
        raise AssertionError('a partially bound native batch unexpectedly succeeded: '+mutation)
    assert files(journal)==journal_before, 'a rejected batch changed journal bytes'
    assert (Path(manifest['source_root'])/manifest['source_path']).read_bytes()==source_before
    assert (Path(manifest['source_root'])/manifest['form_path']).read_bytes()==form_before

# Native grammar is frozen in the explicitly selected immutable cut. A changed
# selected object must refuse both cached and fresh reads, never use root caches.
import hashlib
invocation_value=json.loads(invocation.read_text())
grammar_source=Path(manifest['source_root'])/'ToS/contracts/knowledge-assessment.schema.json'
grammar=Path(invocation_value['corpus_store'])/'objects'/hashlib.sha256(grammar_source.read_bytes()).hexdigest()
assert grammar.is_file() and not grammar.is_relative_to(Path(manifest['source_root']))
grammar_before=grammar.read_bytes()
try:
    schema=json.loads(grammar_before)
    schema['properties']['rationale']['const']='A new independently owned narrow grammar.'
    grammar.write_text(json.dumps(schema))
    for probe in (native.verify_current,
            lambda: AssessedFormSnapshot(owner,ids,native_invocation=invocation).materialize(copy.deepcopy(nodes))):
        try:
            probe()
        except (ValueError,PermissionError,JournalConflict):
            pass
        else:
            raise AssertionError('selected grammar drift escaped the native batch guard')
finally:
    grammar.write_bytes(grammar_before)
native.verify_current()
assert files(journal)==journal_before and nodes==original

# The builder's final guard invokes the native consumer after staging fsync.
# Grant and publication changes at that point cannot expose a candidate.
import os
from unittest.mock import patch
import source_metadata_snapshot as publication
owner_before=owner.read_bytes()
control=Path(manifest['source_root'])/publication.CONTROL_REF
control_before=control.read_bytes() if control.exists() else None
post_sync_target=owner.parent/'native-post-fsync-candidate.json'
original_fsync=os.fsync
for mutation in ('grant','pending','ready-epoch'):
    changed=[]
    def sync_then_change(descriptor):
        original_fsync(descriptor)
        if changed:
            return
        changed.append(True)
        if mutation=='grant':
            config=json.loads(owner_before)
            config['subjects'][ids[0]]['access_allowed']=False
            owner.write_text(json.dumps(config))
        else:
            state={'schema_version':publication.STATE_SCHEMA,'generation':1,
                'transition_id':'1'*32,'phase':'pending' if mutation=='pending' else 'ready',
                'transaction_id':'sha256:'+'1'*64,'manifest_sha256':'sha256:'+'2'*64,
                'outcome':None if mutation=='pending' else 'rolled-back','recovery_authorization':None}
            state['token']=publication._digest(publication._canonical(state))
            control.write_text(json.dumps(state))
    try:
        with patch.object(os,'fsync',sync_then_change):
            try:
                write_assessed_candidate(post_sync_target,json.dumps(actual),native)
            except (ValueError,PermissionError,JournalConflict,publication.PublicationStateError):
                pass
            else:
                raise AssertionError('post-fsync drift escaped native final currentness: '+mutation)
        assert changed and not post_sync_target.exists()
        assert not list(post_sync_target.parent.glob('.tos-assessed-*'))
    finally:
        owner.write_bytes(owner_before)
        if control_before is None:
            control.unlink(missing_ok=True)
        else:
            control.write_bytes(control_before)
    native.verify_current()
    assert files(journal)==journal_before

# A current expired authority cannot reuse a previously ready materialization.
# Its actual native clock evaluates the finite configured validity interval.
if len(ids)>1:
    from datetime import datetime,timedelta,timezone
    config=json.loads(owner_before)
    for authority in config['authorities']:
        authority['payload']['valid_until']=(datetime.now(timezone.utc)-timedelta(days=1)).isoformat()
    try:
        owner.write_text(json.dumps(config))
        try:
            native.verify_current()
        except (ValueError,PermissionError,JournalConflict):
            pass
        else:
            raise AssertionError('expired authority reused a ready native snapshot')
        fresh_expired=AssessedFormSnapshot(owner,ids,native_invocation=invocation)
        expired=fresh_expired.materialize(copy.deepcopy(nodes))
        assert all(packet['state']=='needs-assessment' and packet['display_text'] is None
            and packet['admission']['can_use'] is False for packet in selected_packets(expired))
        assert files(journal)==journal_before
    finally:
        owner.write_bytes(owner_before)
    native.verify_current()

target=owner.parent/'native-assessed-candidate.json'
if len(ids)==1:
    config=json.loads(owner.read_text())
    config['authorities'][0]['payload']['state']='revoked'
    owner.write_text(json.dumps(config))
    try:
        native.verify_current()
    except JournalConflict:
        pass
    else:
        raise AssertionError('owner drift did not invalidate the native snapshot')
    try:
        write_assessed_candidate(target,json.dumps(actual),native)
    except JournalConflict:
        pass
    else:
        raise AssertionError('candidate escaped the final currentness guard')
    assert not target.exists() and not list(target.parent.glob('.tos-assessed-*'))
else:
    form=manifest['forms'][0]
    subject=Record.from_payload(form['form_id'],form['form_version'],form)
    source=Record.from_payload(manifest['source']['id'],manifest['source']['version'],
        manifest['source']['payload'],origin_id=manifest['source']['origin_id'])
    assessor_module=__import__('test_knowledge_assessment')
    assessor_module.END=json.loads(owner.read_text())['authorities'][0]['payload']['valid_until']
    assessor=assessor_module.AssessmentPolicyTests(methodName='runTest'); assessor.setUp()
    for index in range(len(assessor.authorities)):
        competence=assessor.competencies[index]
        assessor.competencies[index]=Record.from_payload(competence.id,competence.version,
            {**competence.payload,'assertion_layers':['human_projection']})
        authority=assessor.authorities[index]
        assessor.authorities[index]=Record.from_payload(authority.id,authority.version,
            {**authority.payload,'assertion_layers':['human_projection'],'subject_prefixes':['tos.form.'],
             'competence_refs':[assessor.competencies[index].ref]})
    assessor.subject=subject; assessor.source=source
    assessor.context=SubjectContext(subject,'human_projection','low',('ru','de'),'fixture-writer','research',True)
    assessor.records=[subject,source,assessor.source_b,assessor.eval_evidence,assessor.executor]
    history=AssessmentJournal(journal); revision,chain=history._load(subject.id)
    old=chain[0]['events'][0]['assessment']
    described=run_legacy_oracle_command(owner,{'schema_version':'tos_local_assessment_command_v1',
        'operation':'describe','subject_id':subject.id})
    withdrawn=assessor.review(profile='interpretation',decision='withdraw',
        name='tos.review.native-read-batch-withdrawal').assessment
    withdrawn['supersedes']=[Record.from_payload(old['assessment_id'],1,old).ref]
    run_legacy_oracle_command(owner,{'schema_version':'tos_local_assessment_command_v1',
        'operation':'append','subject_id':subject.id,'expected_subject':subject.ref,
        'expected_snapshot':described['owner_snapshot'],'expected_revision':revision,
        'command_id':'native-read-batch-withdrawal','assessments':[withdrawn]})
    journal_after_withdrawal=files(journal)
    try:
        native.verify_current()
    except JournalConflict:
        pass
    else:
        raise AssertionError('committed withdrawal did not invalidate the ready snapshot')
    try:
        write_assessed_candidate(target,json.dumps(actual),native)
    except JournalConflict:
        pass
    else:
        raise AssertionError('withdrawn assessment escaped the final currentness guard')
    assert not target.exists() and not list(target.parent.glob('.tos-assessed-*'))
    fresh_retained=AssessedFormSnapshot.for_retained_reference_fixture(owner,ids)
    expected=fresh_retained.materialize(copy.deepcopy(nodes)); fresh_retained.verify_current()
    fresh_native=AssessedFormSnapshot(owner,ids,native_invocation=invocation)
    actual=fresh_native.materialize(copy.deepcopy(nodes)); fresh_native.verify_current()
    assert normalize(actual)==normalize(expected), 'fresh withdrawn native batch differs from retained reader'
    current=selected_packets(actual)
    assert current[0]['state']=='needs-assessment' and current[0]['display_text'] is None
    assert current[0]['admission']['can_use'] is False
    assert all(packet['state']=='ready' for packet in current[1:])
    assert files(journal)==journal_after_withdrawal, 'fresh read batch wrote or locked the journal'
    assert (Path(manifest['source_root'])/manifest['source_path']).read_bytes()==source_before
    assert (Path(manifest['source_root'])/manifest['form_path']).read_bytes()==form_before
print(json.dumps({'form_count':len(ids),'native_route':True,'parity':True,
    'no_partial':True,'no_read_writes':True,'fresh_currentness_guard':True},separators=(',',':')))
"#;
        let result = fixture_json(
            &repository,
            temporary.path(),
            &temporary.path().join("public-read-exercise.stdout"),
            &temporary.path().join("public-read-exercise.stderr"),
            deadline,
            exercise,
        );
        assert_eq!(result["form_count"], count);
        assert_eq!(result["native_route"], true);
        assert_eq!(result["parity"], true);
        assert_eq!(result["no_partial"], true);
        assert_eq!(result["no_read_writes"], true);
        assert_eq!(result["fresh_currentness_guard"], true);
        for (path, expected) in images.iter().zip(&before) {
            assert_eq!(
                super::command_text_cases::alignment_image_digest(path),
                *expected,
                "native read must not modify selected executable or worker"
            );
        }
        assert!(Instant::now() < deadline);
    }
}

fn native_public_assessment_versions(versions: &[u8]) {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(600);
    let cancelled = AtomicBool::new(false);
    for &version in versions {
        let temporary = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let script = format!(
            r#"
import json,runpy,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
f=runpy.run_path(str(repository/'tests/conformance/rust/journal_public_fixture.py'))
print(json.dumps(f['prepare'](repository,root,{version}),ensure_ascii=False,separators=(',',':')))
"#
        );
        let fixture = fixture_json(
            &repository,
            temporary.path(),
            &temporary.path().join("public-fixture.stdout"),
            &temporary.path().join("public-fixture.stderr"),
            deadline,
            &script,
        );
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        let public = PathBuf::from(fixture["public"].as_str().unwrap());
        let context = PathBuf::from(fixture["context"].as_str().unwrap());
        let images = [
            std::env::current_exe().unwrap(),
            PathBuf::from(
                std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
            ),
            PathBuf::from(
                std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("native worker required"),
            ),
        ];
        let custody = |path: &Path| {
            let metadata = path.symlink_metadata().unwrap();
            assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
            assert_eq!(metadata.mode() & 0o022, 0);
            (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mode(),
                super::command_text_cases::alignment_image_digest(path),
            )
        };
        let before: Vec<_> = images.iter().map(|path| custody(path)).collect();
        let authored = super::command_text_cases::authored_text_files(&public);
        let mut captured = authored.clone();
        for reference in assessment_feature_sources() {
            let path = repository.join(reference);
            assert!(
                path.symlink_metadata().unwrap().is_file()
                    && fs::metadata(&path).unwrap().len() <= 2_097_152
            );
            assert!(
                captured
                    .insert(reference.to_owned(), fs::read(path).unwrap())
                    .is_none()
            );
        }
        assert!(
            captured.len() <= 2048 && captured.values().map(Vec::len).sum::<usize>() <= 33_554_432
        );
        let (capture, _software, components) =
            super::command_record_cases::captured_components(&captured, deadline, &cancelled);
        let store = temporary.path().join("public-journal-cut");
        let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
        let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
            "owner_config":owner,"owner_context":context,"native_executable":images[1],"native_executable_sha256":before[1].4.to_prefixed(),
            "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":selected.0.to_prefixed(),
            "software_capture":capture.capture,"software_restored_root":capture.restored,
            "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":images[2],"sha256":before[2].4.to_prefixed()},
            "assessment_schema_worker":{"absolute_path":images[2],"sha256":before[2].4.to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        let invocation_path = temporary.path().join("public-journal-invocation.json");
        fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
        let invocation_count = std::cell::Cell::new(0u32);
        let observe = |request: &Value| {
            invocation_count.set(invocation_count.get().checked_add(1).unwrap());
            super::command_text_cases::native_owner_cli_observation(
                &repository,
                &owner,
                &invocation_path,
                request,
                deadline,
            )
        };
        let invoke = |request: &Value| -> Value {
            let (status, raw, errors) = observe(request);
            assert!(
                status.success(),
                "public Journal v{version} call {} operation {}: {}",
                invocation_count.get(),
                request["operation"].as_str().unwrap_or("missing"),
                String::from_utf8_lossy(&errors)
            );
            serde_json::from_slice(&raw).unwrap()
        };
        let describe = serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"describe","subject_id":fixture["subject_id"]});
        let described = invoke(&describe);
        assert_eq!(
            described, fixture["expected_describe"],
            "whole public description v{version}"
        );
        let mut request = fixture["request"].clone();
        request["expected_snapshot"] = described["owner_snapshot"].clone();
        let mut inspect = serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"inspect","subject_id":fixture["subject_id"],
            "expected_subject":request["expected_subject"],"expected_snapshot":request["expected_snapshot"]});
        if version == 2 {
            inspect["operation"] = Value::from("materialize-form");
            let pending = invoke(&inspect);
            assert_eq!(
                pending["result"]["materialization"]["state"],
                "needs-assessment"
            );
            assert!(pending["result"]["materialization"]["display_text"].is_null());
        }
        let committed = invoke(&request);
        assert_eq!(committed["result"]["current_admission"]["can_use"], true);
        if version == 2 {
            let ready = invoke(&inspect);
            assert_eq!(ready["result"]["materialization"]["state"], "ready");
            assert!(
                ready["result"]["materialization"]["display_text"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty())
            );
        } else {
            assert_eq!(invoke(&inspect)["result"]["batch_count"], 1);
        }
        let replay = invoke(&request);
        assert_eq!(replay["result"]["replayed"], true);
        assert_eq!(replay["result"]["receipt"], committed["result"]["receipt"]);
        let mut config: Value = serde_json::from_slice(&fs::read(&owner).unwrap()).unwrap();
        let original_authority = config["authorities"][0].clone();
        let authority = &mut config["authorities"][0];
        authority["version"] = Value::from(
            authority["version"]
                .as_u64()
                .unwrap()
                .checked_add(1)
                .unwrap(),
        );
        authority["payload"]["authority_version"] = Value::from(
            authority["payload"]["authority_version"]
                .as_u64()
                .unwrap()
                .checked_add(1)
                .unwrap(),
        );
        authority["payload"]["state"] = Value::from("revoked");
        fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        request["expected_snapshot"] = invoke(&describe)["owner_snapshot"].clone();
        inspect["expected_snapshot"] = request["expected_snapshot"].clone();
        if version == 2 {
            let closed = invoke(&inspect);
            assert_eq!(
                closed["result"]["materialization"]["state"],
                "needs-assessment"
            );
            assert!(closed["result"]["materialization"]["display_text"].is_null());
        }
        let revoked = invoke(&request);
        assert_eq!(revoked["result"]["replayed"], true);
        assert_eq!(
            revoked["result"]["receipt"]["admission_at_commit"]["can_use"],
            true
        );
        assert_eq!(revoked["result"]["current_admission"]["can_use"], false);
        if version == 3 {
            config["authorities"][0] = original_authority;
            config["native_text_units"][0]["read_scope"] = Value::from("metadata_only");
            // The genuine metadata-only resolver view has a new subject digest.
            // Keep request.expected_subject unchanged for the stale downgrade refusal.
            config["subjects"][fixture["subject_id"].as_str().unwrap()]["record"] =
                fixture["metadata_subject"].clone();
            fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
            fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
            let metadata = invoke(&describe);
            assert_eq!(
                metadata["result"]["command_context"]["supported_operations"],
                serde_json::json!(["describe", "inspect"])
            );
            request["expected_snapshot"] = metadata["owner_snapshot"].clone();
            assert!(!observe(&request).0.success());
        }
        let mut unknown = describe.clone();
        unknown["subject_id"] = Value::from("tos.subject.outside-public-selection");
        assert!(!observe(&unknown).0.success());
        for preserved in fixture["preserved"].as_array().unwrap() {
            let digest = super::command_text_cases::alignment_image_digest(Path::new(
                preserved["path"].as_str().unwrap(),
            ));
            assert_eq!(digest.to_hex(), preserved["sha256"].as_str().unwrap());
        }
        for (index, path) in images.iter().enumerate() {
            assert_eq!(custody(path), before[index]);
        }
        assert!(Instant::now() < deadline);
    }
}
