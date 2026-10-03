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
    let temporary = tempfile::tempdir().unwrap();
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
    let temporary = tempfile::tempdir().unwrap();
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

fn native_layer_journal_fixture(repository: &Path, root: &Path, deadline: Instant) -> Value {
    let script = r#"
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
    fx=maintained.QualityJournalFixture(test)
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
        'preserved':[str(fx.fx.store/fx.fx.source_ref),str(fx.fx.store/fx.packet_ref),str(fx.fx.payload)]}
finally:
    tempfile.TemporaryDirectory=original
print(json.dumps(result,ensure_ascii=False,separators=(',',':')))
"#;
    fixture_json(
        repository,
        root,
        &root.join("layer-fixture.stdout"),
        &root.join("layer-fixture.stderr"),
        deadline,
        script,
    )
}

fn native_layer_journal_case() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::tempdir().unwrap();
    let fixture = native_layer_journal_fixture(&repository, temporary.path(), deadline);
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
    let store = temporary.path().join("layer-journal-source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
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
    assert_eq!(
        comparison["result"]["source_comparison"]["payload"]["source_member_utf8"],
        fixture["source_member"]
    );
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
    native_layer_journal_case();
}

#[test]
#[ignore = "requires retained signed synthetic OCR evidence and admitted native owner/worker"]
fn native_private_assessment_v6_retained_signed_ocr_comparison_preserves_source() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::tempdir().unwrap();
    let script = r#"
import json,runpy,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
f=runpy.run_path(str(repository/'tests/conformance/rust/journal_v6_fixture.py'))
print(json.dumps(f['prepare'](repository,root),separators=(',',':')))
"#;
    let fixture = fixture_json(
        &repository,
        temporary.path(),
        &temporary.path().join("v6-prepare.stdout"),
        &temporary.path().join("v6-prepare.stderr"),
        deadline,
        script,
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
    let invoke = |selected_owner: &Path, request: &Value| -> Value {
        let (status, raw, errors) = observe(selected_owner, request);
        assert!(
            status.success(),
            "native v6 retained OCR: {}",
            String::from_utf8_lossy(&errors)
        );
        serde_json::from_slice(&raw).unwrap()
    };
    let prepared = invoke(
        &owner,
        &serde_json::json!({
        "schema_version":"tos_local_source_command_v1","operation":"prepare-create"}),
    );
    let created = invoke(
        &owner,
        &serde_json::json!({
        "schema_version":"tos_local_source_command_v1","operation":"text-layer.record-owner-ocr",
        "command_id":"synthetic-v6-authenticated-ocr-record",
        "expected_configuration":prepared["owner_configuration"],
        "expected_dependencies":prepared["expected_dependencies"],
        "expected_source":null,"expected_revision":null}),
    );
    assert_eq!(created["replayed"], false);
    let finish_script = r#"
import json,runpy,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
f=runpy.run_path(str(repository/'tests/conformance/rust/journal_v6_fixture.py'))
print(json.dumps(f['finish'](repository,root),separators=(',',':')))
"#;
    let selected = fixture_json(
        &repository,
        temporary.path(),
        &temporary.path().join("v6-finish.stdout"),
        &temporary.path().join("v6-finish.stderr"),
        deadline,
        finish_script,
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
    assert_authored_text_unchanged(&public, &authored);
    for (index, path) in images.iter().enumerate() {
        assert_eq!(custody(path), image_before_native[index]);
    }
    assert!(Instant::now() < deadline);
}
