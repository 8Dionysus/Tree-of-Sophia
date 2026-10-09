//! The frozen source snapshot supplies a synthetic owner, rights, source
//! records and an acquired EPUB. Rust performs private publication and replay.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_text_alignment_entry::{
    execute_owner_alignment_from_captures, prepare_owner_alignment_from_captures,
};
use tos_command::source_text_layer_derived_entry::{
    execute_derived_text_layer_from_captures, prepare_derived_text_layer_from_captures,
};
use tos_command::source_text_layer_entry::{
    execute_initial_text_layer_from_captures, prepare_initial_text_layer_from_captures,
};
use tos_command::source_text_unit_entry::{
    execute_first_text_unit_from_captures, prepare_first_text_unit_from_captures,
};
use tos_validation::FormatProfile;
use tos_validation::executor::ExecutorBudget;

const INITIAL_TEXT_NATIVE_OWNER_PATHS: &[&str] = &[
    "rust/crates/tos-command/src/source_text_layer_native.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_text_unit_native.rs",
    "rust/crates/tos-command/src/source_text_layer_entry.rs",
    "rust/crates/tos-command/src/source_text_unit_entry.rs",
];

const ALIGNMENT_NATIVE_OWNER_PATHS: &[&str] = &[
    "rust/crates/tos-command/src/source_text_alignment_entry.rs",
    "rust/crates/tos-command/src/source_text_identity.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_text_private_store.rs",
    "rust/crates/tos-command/src/source_sign_native.rs",
];

const DERIVED_TEXT_NATIVE_OWNER_PATHS: &[&str] = &[
    "rust/crates/tos-command/src/source_text_layer_derived_entry.rs",
    "rust/crates/tos-command/src/source_text_layer_derived_proposal.rs",
    "rust/crates/tos-command/src/source_text_layer_native.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_text_private_store.rs",
];

const DERIVED_OCR_NATIVE_OWNER_PATHS: &[&str] = &[
    "rust/crates/tos-command/src/source_text_layer_derived_entry.rs",
    "rust/crates/tos-command/src/source_text_layer_derived_proposal.rs",
    "rust/crates/tos-command/src/source_text_layer_native.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_text_owner_ocr.rs",
    "rust/crates/tos-command/src/source_text_layer_payload.rs",
    "rust/crates/tos-command/src/source_text_private_store.rs",
];

fn text_fixture(root: &Path) -> NativePythonFixture {
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
    'source_ref':case.source_ref,'content_sha256':layer.source._digest(case.content)[7:],
    'content_bytes':len(case.content),'config':case.config,
    'unit_config':case.seed.config,'unit_proposal':case.seed.proposal,
    'unit_layer_schema':unit.native.LAYER_CONFIG,
    'implementations':sorted(set(layer.layers.IMPLEMENTATIONS))},ensure_ascii=False,separators=(',',':')))
"#;
    let fixture = super::native_python_fixture(
        "text-initial-layer",
        &[("source-root", root)],
        INITIAL_TEXT_NATIVE_OWNER_PATHS,
    );
    super::assert_native_python_fixture(&fixture, script, INITIAL_TEXT_NATIVE_OWNER_PATHS);
    fixture
}

fn alignment_fixture(root: &Path) -> NativePythonFixture {
    let script = r#"
import json,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts'),str(repository/'tests')]
import test_source_text_unit_commands as unit
import test_source_alignment_commands as alignment
class ExistingRoot:
    def __init__(self,*args,**kwargs): self.name=str(root)
    def cleanup(self): pass
original=unit.tempfile.TemporaryDirectory
unit.tempfile.TemporaryDirectory=ExistingRoot
try:
    case=alignment.NativeAlignmentCommandTests(methodName='runTest')
    case.setUp()
finally:
    unit.tempfile.TemporaryDirectory=original
print(json.dumps({'public':str(case.public),'private':str(case.private),
    'context':str(case.context_path),'owner':str(case.owner),
    'source_ref':case.source_ref,'config':case.config,'proposal':case.proposal,
    'implementations':sorted(set(alignment.align.IMPLEMENTATIONS))},ensure_ascii=False,separators=(',',':')))
"#;
    let fixture = super::native_python_fixture(
        "text-alignment",
        &[("source-root", root)],
        ALIGNMENT_NATIVE_OWNER_PATHS,
    );
    super::assert_native_python_fixture(&fixture, script, ALIGNMENT_NATIVE_OWNER_PATHS);
    fixture
}

fn derived_fixture(root: &Path, kind: &str) -> NativePythonFixture {
    let script = r#"
import copy,json,os,sys,unicodedata
from pathlib import Path
repository,root=map(Path,sys.argv[1:3])
kind=sys.argv[3]
sys.path[:0]=[str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import source_text_layer_commands as layers
import source_commands as source
from source_text_layer_proposal import derivation_policy
store=root/'private'
original=json.loads((root/'layer-owner.json').read_bytes())
source_ref=original['source_path']
prior_raw=(store/source_ref).read_bytes()
prior=json.loads(prior_raw)
prefix=source_ref.split('layers/first/')[0]
variants={'normalize':('normalized','5','6','text-layer.normalize'),
    'correct':('corrected','7','8','text-layer.correct'),
    'record-ocr':('recorded','a','b','text-layer.record-ocr')}
home,layer_digit,event_digit,operation=variants[kind]
layer_id='tos.text-layer.sid-'+layer_digit*32
rights_ref=prefix+'rights/derived-'+home+'.json'
source_rights_ref=original['derivation_access']['rights_record_refs'][0]['ref']
rights=json.loads((root/'public'/source_rights_ref).read_bytes())
rights.update(rights_id='tos.rights.synthetic.native-derived-'+home,scope_refs=[layer_id])
rights_raw=source._canonical(rights)+b'\n'
rights_path=store/rights_ref
rights_path.write_bytes(rights_raw)
rights_path.chmod(0o600)
config=copy.deepcopy(original)
config.update(schema_version=layers.DERIVE_CONFIG,
    source_path=prefix+'layers/'+home+'/source-text-layer.v1.json',
    allowed_operations=[operation],
    input={'kind':'text_layer','binding':{'schema_version':'tos_native_text_layer_binding_v1',
        'text_layer':{'record_ref':source_ref,'record_sha256':source._digest(prior_raw)[7:],
            'layer_id':prior['layer_id'],'layer_version':prior['layer_version']},
        'source_record_refs':original['source_record_refs']}},
    material={},policy=derivation_policy(operation,unicode_form='NFC') if kind=='normalize' else derivation_policy(operation))
config.pop('member')
config.pop('selector')
config['identities']={'layer_id':layer_id,'provenance_event_id':'tos.event.sid-'+event_digit*32}
text=(store/source_ref).parent.joinpath('content.txt').read_bytes()
if kind!='record-ocr':
    config['source_access']={'read_scope':'exact_text_layer','access_allowed':True,
        'byte_size':len(text),'authority_ref':'operator:synthetic-layer-reading',
        'expires_at':'2099-01-01T00:00:00Z'}
if kind=='normalize':
    config['maker'].update(method='tos.unicode.normalize.v1',version=config['policy']['unicode_database_version'])
elif kind=='correct':
    config['maker'].update(maker_type='human',method='supplied synthetic correction',version='1')
    before=text.decode('utf-8')
    start=before.index('test')
    config['material']={'edits':[{'start':start,'end':start+4,'input_exact':'test',
        'input_sha256':source._digest(b'test')[7:],'output_exact':'trial',
        'reason':'Synthetic source-visible correction proposal, not accepted reading.','confidence':0.7}]}
else:
    anchor=prior['source_binding']['anchors'][0]
    config['input']={'kind':'acquired_file','anchor':{'anchor_id':anchor['anchor_id'],
        'record_ref':anchor['anchor_record_ref'],'record_sha256':anchor['anchor_record_sha256']}}
    config['source_access']=copy.deepcopy(original['source_access'])
    supplied='Supplied OCR & transcript\r\n\u212b fi\ufb01.'
    supplied_ref=prefix+'supplied-result.txt'
    supplied_path=store/supplied_ref
    supplied_path.write_bytes(supplied.encode('utf-8'))
    supplied_path.chmod(0o600)
    config['material']={'content_ref':supplied_ref,'content_sha256':source._digest(supplied.encode('utf-8'))[7:],
        'byte_size':len(supplied.encode('utf-8')),'access_allowed':True,
        'authority_ref':'operator:synthetic-supplied-result-read','expires_at':'2099-01-01T00:00:00Z',
        'provider_execution':'not_observed','reported_maker':{'maker_type':'software',
            'agent_ref':'synthetic:unverified-reported-producer','method':'supplied method declaration','version':'1'}}
    config['maker'].update(method='record explicitly supplied text',version='1')
config['derivation_access'].update(operation={'normalize':'unicode_normalization','correct':'correction','record-ocr':'ocr'}[kind],rights_record_refs=[
    original['derivation_access']['rights_record_refs'][0],
    {'ref':rights_ref,'sha256':source._digest(rights_raw)[7:]}])
owner=root/('derived-'+kind+'-owner.json')
owner.write_bytes(source._canonical(config)+b'\n')
owner.chmod(0o600)
expected={'normalize':lambda:unicodedata.normalize('NFC',text.decode('utf-8')).encode('utf-8'),
    'correct':lambda:text.replace(b'test',b'trial'),
    'record-ocr':lambda:supplied.encode('utf-8')}[kind]()
print(json.dumps({'owner':str(owner),'source_ref':config['source_path'],
    'expected_sha256':source._digest(expected)[7:],'expected_bytes':len(expected)},
    ensure_ascii=False,separators=(',',':')))
"#;
    let id = match kind {
        "normalize" => "text-derived-normalize",
        "correct" => "text-derived-correct",
        "record-ocr" => "text-derived-record-ocr",
        _ => panic!("unknown captured derived Text fixture kind {kind}"),
    };
    let destination = root.join(format!("derived-{kind}"));
    let owner_paths = if kind == "record-ocr" {
        DERIVED_OCR_NATIVE_OWNER_PATHS
    } else {
        DERIVED_TEXT_NATIVE_OWNER_PATHS
    };
    let fixture = super::native_python_fixture(id, &[("private-root", &destination)], owner_paths);
    super::assert_native_python_fixture(&fixture, script, owner_paths);
    fixture
}

pub(super) fn authored_text_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut directories = vec![root.join("ToS")];
    let mut files = BTreeMap::new();
    let mut bytes = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
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
                // Exact maintained mutex is live control, not authored source.
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

fn text_worker(
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

fn text_worker_with_image(
    cut: &tos_source_store::CorpusCutReader,
    image: &tos_validation::executor::VerifiedWorkerImageHandle,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_validation::source_cut::CutWorkerSchemaExecutor {
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!budget.execution_wall.is_zero());
    super::command_form_cases::schemas_for_profile_with_image(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        image,
        budget,
        deadline,
        cancelled,
    )
}

fn source_value(value: &Value) -> JsonValue {
    parse_json(
        &serde_json::to_vec(value).unwrap(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}

fn alignment_owner_bytes(value: &Value) -> Vec<u8> {
    tos_foundation::canonical_bytes_v1(
        &source_value(value),
        tos_foundation::CanonicalProfile::CorpusSnapshotV1,
        tos_foundation::JsonLimits::default(),
    )
    .unwrap()
}

pub(super) fn alignment_image_digest(path: &Path) -> Digest256 {
    use std::io::Read;
    let mut image = fs::File::open(path).unwrap();
    let before = image.metadata().unwrap();
    assert!(before.is_file() && before.len() > 0);
    let mut digest = tos_foundation::Digest256Hasher::new();
    let mut buffer = [0u8; 65_536];
    let mut total = 0u64;
    loop {
        let n = image.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).unwrap();
        assert!(total <= before.len());
        digest.update(&buffer[..n]);
    }
    let after = image.metadata().unwrap();
    assert_eq!(
        (
            total,
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec()
        ),
        (
            after.len(),
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        )
    );
    digest.finalize()
}

pub(super) fn alignment_native_cli(
    repository: &Path,
    owner: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> Value {
    let (status, output_bytes, error_bytes) =
        native_owner_cli_observation(repository, owner, invocation, request, deadline);
    assert!(
        status.success(),
        "native CLI output={} stderr={}",
        String::from_utf8_lossy(&output_bytes),
        String::from_utf8_lossy(&error_bytes)
    );
    serde_json::from_slice(&output_bytes).unwrap()
}

pub(super) fn native_owner_cli_observation(
    repository: &Path,
    owner: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> (std::process::ExitStatus, Vec<u8>, Vec<u8>) {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut input = tempfile::tempfile().unwrap();
    input.write_all(&alignment_owner_bytes(request)).unwrap();
    input.seek(SeekFrom::Start(0)).unwrap();
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut child =
        Command::new(crate::maintained_python())
            .arg(repository.join(
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
            ))
            .arg("--owner-config")
            .arg(owner)
            .arg("--native-invocation")
            .arg(invocation)
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::from(input))
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(errors.try_clone().unwrap()))
            .spawn()
            .unwrap();
    let step_deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        let now = Instant::now();
        let output_len = output.metadata().unwrap().len();
        let error_len = errors.metadata().unwrap().len();
        if now >= step_deadline || output_len > 1_048_576 || error_len > 1_048_576 {
            let reason = if now >= deadline {
                "whole deadline"
            } else if now >= step_deadline {
                "60-second child deadline"
            } else if output_len > 1_048_576 {
                "stdout cap"
            } else {
                "stderr cap"
            };
            let kill = child.kill();
            let stopped = child.wait();
            output.seek(SeekFrom::Start(0)).unwrap();
            errors.seek(SeekFrom::Start(0)).unwrap();
            let mut out_prefix = Vec::new();
            let mut err_prefix = Vec::new();
            (&mut output)
                .take(16_384)
                .read_to_end(&mut out_prefix)
                .unwrap();
            (&mut errors)
                .take(16_384)
                .read_to_end(&mut err_prefix)
                .unwrap();
            let operation = request["operation"].as_str().unwrap_or("<absent>");
            panic!(
                "bounded native owner CLI refused: operation={} reason={reason} kill={kill:?} post_kill_status={stopped:?} stdout_bytes={output_len} stderr_bytes={error_len} output_prefix={} stderr_prefix={}",
                operation.chars().take(256).collect::<String>(),
                String::from_utf8_lossy(&out_prefix),
                String::from_utf8_lossy(&err_prefix)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step_deadline);
    assert!(output.metadata().unwrap().len() <= 1_048_576);
    assert!(errors.metadata().unwrap().len() <= 1_048_576);
    output.seek(SeekFrom::Start(0)).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut output_bytes = Vec::new();
    let mut error_bytes = Vec::new();
    output.read_to_end(&mut output_bytes).unwrap();
    errors.read_to_end(&mut error_bytes).unwrap();
    (status, output_bytes, error_bytes)
}

#[test]
fn native_owner_alignment_preserves_versions_competition_and_cold_replay() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let fixture_capture = alignment_fixture(temporary.path());
    let fixture = fixture_capture.packets.get("factory").unwrap();
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let private = PathBuf::from(fixture["private"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let original_path = fixture["source_ref"].as_str().unwrap().to_owned();
    let mut config = fixture["config"].clone();
    let original_config = config.clone();
    let mut proposal = fixture["proposal"].clone();
    let authored = authored_text_files(&public);
    // The protected OPS runner uses umask 077. This synthetic public fixture
    // must explicitly carry the portable authored-cut mode, while its private
    // owner/context, excluded payload and owner-local files retain their modes.
    for reference in authored.keys() {
        fs::set_permissions(public.join(reference), fs::Permissions::from_mode(0o644)).unwrap();
    }
    eprintln!(
        "Alignment fixture F_authored_cut={} G_grant_bytes={}",
        authored.len(),
        fs::metadata(&owner).unwrap().len()
    );
    let mut captured = authored.clone();
    for reference in &fixture_capture
        .software_identity_migration
        .native_owner_paths
    {
        assert!(
            captured
                .insert(
                    reference.clone(),
                    fs::read(repository.join(reference.as_str())).unwrap()
                )
                .is_none()
        );
    }
    let (capture, software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("alignment-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = super::command_form_cases::open_cut(&store, selected, deadline, &cancelled);

    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must supply the retained TOS_NATIVE_OWNER_COMMAND_PATH"),
    );
    assert!(
        native.is_absolute(),
        "native owner executable must be absolute"
    );
    let worker_image = super::validation_cut_cases::selected_worker_path();
    let invocation_path = temporary.path().join("native-alignment-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&serde_json::json!({
        "schema_version":"tos_local_native_owner_invocation_v1",
        "owner_context":context,"owner_config":owner,
        "native_executable":native,
        "native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{
            "source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed(),
        },
        "software_components":components.members().map(|member| member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{
            "absolute_path":worker_image,
            "sha256":alignment_image_digest(&worker_image).to_prefixed(),
        },
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
            "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824},
    })).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();

    let preview = alignment_native_cli(&repository, &owner, &invocation_path, &proposal, deadline);
    assert_eq!(preview["target_exists"], false);
    assert_eq!(
        preview["schema_version"],
        "tos_local_native_alignment_result_v1"
    );
    let mut request = proposal.clone();
    request["operation"] = config["allowed_operations"][0].clone();
    request["command_id"] = Value::String("synthetic-native-alignment-first".into());
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    request["expected_source"] = Value::Null;
    request["expected_revision"] = Value::Null;
    let first = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(first["replayed"], false);
    assert_eq!(first["grants_admission"], false);
    assert_eq!(first["aligner_executed"], false);
    let first_home = private.join(&original_path).parent().unwrap().to_path_buf();
    let first_raw = fs::read(first_home.join("native-translation-alignment.v1.json")).unwrap();
    let first_body: Value = serde_json::from_slice(&first_raw).unwrap();
    assert_eq!(first_body["claim"]["claim_version"], 1);
    assert_eq!(first_body["status"], "proposed");
    assert!(!public.join(&original_path).exists());
    let saved: BTreeMap<_, _> = fs::read_dir(&first_home)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    let retained_inputs: Value = serde_json::from_slice(
        saved
            .get(&std::ffi::OsString::from("source-create-inputs.json"))
            .unwrap(),
    )
    .unwrap();
    let resolver_key_upper = retained_inputs["inputs"].as_array().unwrap().len();
    eprintln!(
        "Alignment first retained distinct (path,category) union F_resolver_upper={resolver_key_upper}"
    );
    assert!(resolver_key_upper <= 128);
    let replay = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["receipt_sha256"], first["receipt_sha256"]);
    let first_receipt = parse_json(
        &fs::read(first_home.join("source-create-receipt.json")).unwrap(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    assert_eq!(
        saved,
        fs::read_dir(&first_home)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect()
    );
    let first_ref = serde_json::json!({
        "record_ref": original_path,
        "record_id": first_body["record_id"],
        "record_version": 1,
        "sha256": Digest256::of_bytes(&first_raw).to_hex(),
    });
    let inspection = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect"}),
        deadline,
    );
    assert_eq!(inspection["metadata_verified"], true);
    assert_eq!(inspection["content_verified"], false);
    assert_eq!(inspection["record_version"], 1);
    let recovery = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1",
            "operation":"inspect-recovery","command_id":"synthetic-native-alignment-first"}),
        deadline,
    );
    assert_eq!(recovery["recovery_state"], "committed");

    // The second package is a description of the same Alignment and a new
    // Claim version. The first seven private files remain byte-for-byte.
    let second_path = original_path.replace("alignments/first/", "alignments/second/");
    config["source_path"] = Value::String(second_path.clone());
    config["change_kind"] = Value::String("describe".into());
    config["predecessor"] = first_ref.clone();
    config["allowed_operations"] = serde_json::json!(["alignment.revise"]);
    config["provenance_event_id"] =
        Value::String("tos.event.synthetic.native-alignment.second".into());
    config["maker"]["provenance_event_ref"] = config["provenance_event_id"].clone();
    proposal["operation"] = Value::String("prepare-revise".into());
    proposal["qualifications"]["status_reason"] =
        Value::String("Revised supplied description.".into());
    fs::write(&owner, alignment_owner_bytes(&config)).unwrap();
    let mut worker = text_worker(&cut, deadline, &cancelled);
    // A deterministic native current-grant refusal uses this same worker.
    // The retained Python boundary separately exercises revocation between
    // second-side rights and the first content read; no race hook is added.
    let mut revoked = config.clone();
    revoked["source_access"]["expires_at"] = Value::String("2000-01-01T00:00:00Z".into());
    fs::write(&owner, alignment_owner_bytes(&revoked)).unwrap();
    assert!(matches!(
        prepare_owner_alignment_from_captures(
            &context,
            &owner,
            &source_value(&proposal),
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancelled,
        ),
        Err(tos_command::source_command::SourceCommandError::Denied(_))
    ));
    assert!(!private.join(&second_path).parent().unwrap().exists());
    assert_eq!(
        fs::read(first_home.join("native-translation-alignment.v1.json")).unwrap(),
        first_raw
    );
    fs::write(&owner, alignment_owner_bytes(&config)).unwrap();
    let preview = prepare_owner_alignment_from_captures(
        &context,
        &owner,
        &source_value(&proposal),
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(worker);
    let mut second_request = proposal.clone();
    second_request["operation"] = Value::String("alignment.revise".into());
    second_request["command_id"] = Value::String("synthetic-native-alignment-second".into());
    second_request["expected_configuration"] = Value::String(preview.owner_configuration);
    second_request["expected_dependencies"] = Value::String(preview.expected_dependencies);
    second_request["expected_source"] = Value::Null;
    second_request["expected_revision"] = Value::Null;
    let mut worker = text_worker(&cut, deadline, &cancelled);
    let second = execute_owner_alignment_from_captures(
        &context,
        &owner,
        &source_value(&second_request),
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(worker);
    assert!(!second.replayed);
    let second_raw = fs::read(private.join(&second_path)).unwrap();
    let second_body: Value = serde_json::from_slice(&second_raw).unwrap();
    assert_eq!(second_body["record_version"], 2);
    assert_eq!(second_body["claim"]["claim_version"], 2);
    assert_eq!(
        second_body["claim"]["claim_id"],
        first_body["claim"]["claim_id"]
    );
    assert_eq!(
        fs::read(first_home.join("native-translation-alignment.v1.json")).unwrap(),
        first_raw
    );
    let historical = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1",
            "operation":"inspect-version","source":first_ref.clone()}),
        deadline,
    );
    assert_eq!(historical["record_version"], 1);
    assert_eq!(historical["history_depth"], 2);
    let described = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1",
            "operation":"describe"}),
        deadline,
    );
    assert_eq!(described["target_exists"], true);
    assert_eq!(described["grants_admission"], false);

    // Remapping advances the Alignment but starts a different Claim.
    let second_ref = serde_json::json!({
        "record_ref": second_path, "record_id": second_body["record_id"],
        "record_version": 2, "sha256": Digest256::of_bytes(&second_raw).to_hex(),
    });
    let third_path = original_path.replace("alignments/first/", "alignments/third/");
    config["source_path"] = Value::String(third_path.clone());
    config["change_kind"] = Value::String("remap".into());
    config["predecessor"] = second_ref;
    config["claim_id"] = Value::String(format!(
        "tos.translation-alignment-claim.sid-{}",
        "d".repeat(32)
    ));
    config["provenance_event_id"] =
        Value::String("tos.event.synthetic.native-alignment.third".into());
    config["maker"]["provenance_event_ref"] = config["provenance_event_id"].clone();
    proposal["mapping"]["correspondence_shape"] = Value::String("one_to_one".into());
    proposal["mapping"]["ordered_target_anchor_refs"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    fs::write(&owner, alignment_owner_bytes(&config)).unwrap();
    let mut worker = text_worker(&cut, deadline, &cancelled);
    let preview = prepare_owner_alignment_from_captures(
        &context,
        &owner,
        &source_value(&proposal),
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(worker);
    let mut third_request = proposal.clone();
    third_request["operation"] = Value::String("alignment.revise".into());
    third_request["command_id"] = Value::String("synthetic-native-alignment-third".into());
    third_request["expected_configuration"] = Value::String(preview.owner_configuration);
    third_request["expected_dependencies"] = Value::String(preview.expected_dependencies);
    third_request["expected_source"] = Value::Null;
    third_request["expected_revision"] = Value::Null;
    let mut worker = text_worker(&cut, deadline, &cancelled);
    let remapped = execute_owner_alignment_from_captures(
        &context,
        &owner,
        &source_value(&third_request),
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(worker);
    assert!(!remapped.replayed);
    let third_body: Value =
        serde_json::from_slice(&fs::read(private.join(&third_path)).unwrap()).unwrap();
    assert_eq!(third_body["record_version"], 3);
    assert_eq!(third_body["claim"]["claim_version"], 1);
    assert_ne!(
        third_body["claim"]["claim_id"],
        first_body["claim"]["claim_id"]
    );
    assert_eq!(fs::read(private.join(&second_path)).unwrap(), second_raw);

    // An independently identified competitor cites, but never mutates, the
    // exact first record. This remains an unassessed supplied proposal.
    let fourth_path = original_path.replace("alignments/first/", "alignments/alternative/");
    config["source_path"] = Value::String(fourth_path.clone());
    config["change_kind"] = Value::String("competing".into());
    config["predecessor"] = Value::Null;
    config["competing_records"] = serde_json::json!([first_ref]);
    config["record_id"] = Value::String(format!(
        "tos.translation-alignment-record.sid-{}",
        "e".repeat(32)
    ));
    config["alignment_id"] =
        Value::String(format!("tos.translation-alignment.sid-{}", "f".repeat(32)));
    config["claim_id"] = Value::String(format!(
        "tos.translation-alignment-claim.sid-{}",
        "9".repeat(32)
    ));
    config["allowed_operations"] = serde_json::json!(["alignment.create"]);
    config["provenance_event_id"] =
        Value::String("tos.event.synthetic.native-alignment.alternative".into());
    config["maker"]["provenance_event_ref"] = config["provenance_event_id"].clone();
    proposal["operation"] = Value::String("prepare-create".into());
    fs::write(&owner, alignment_owner_bytes(&config)).unwrap();
    let mut worker = text_worker(&cut, deadline, &cancelled);
    let preview = prepare_owner_alignment_from_captures(
        &context,
        &owner,
        &source_value(&proposal),
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(worker);
    let mut fourth_request = proposal;
    fourth_request["operation"] = Value::String("alignment.create".into());
    fourth_request["command_id"] = Value::String("synthetic-native-alignment-alternative".into());
    fourth_request["expected_configuration"] = Value::String(preview.owner_configuration);
    fourth_request["expected_dependencies"] = Value::String(preview.expected_dependencies);
    fourth_request["expected_source"] = Value::Null;
    fourth_request["expected_revision"] = Value::Null;
    let mut worker = text_worker(&cut, deadline, &cancelled);
    let competitor = execute_owner_alignment_from_captures(
        &context,
        &owner,
        &source_value(&fourth_request),
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(worker);
    assert!(!competitor.replayed);
    let competing_body: Value =
        serde_json::from_slice(&fs::read(private.join(fourth_path)).unwrap()).unwrap();
    assert_eq!(competing_body["record_version"], 1);
    assert_eq!(
        competing_body["competing_records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_ne!(competing_body["alignment_id"], first_body["alignment_id"]);
    assert_eq!(
        fs::read(first_home.join("native-translation-alignment.v1.json")).unwrap(),
        first_raw
    );
    fs::write(&owner, alignment_owner_bytes(&original_config)).unwrap();
    // The original package binds the native CLI ELF in its retained input
    // document. Replay through that same producer identity; the conformance
    // process is a different ELF, even though it links the same command source.
    let original_again =
        alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(original_again["replayed"], true);
    assert_eq!(original_again["receipt_sha256"], first["receipt_sha256"]);
    assert_eq!(
        parse_json(
            &fs::read(first_home.join("source-create-receipt.json")).unwrap(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root(),
        first_receipt
    );
    assert_eq!(
        saved,
        fs::read_dir(&first_home)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect()
    );
}

#[test]
fn native_text_layer_extracts_private_epub_and_cold_replays() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let fixture_capture = text_fixture(temporary.path());
    let fixture = fixture_capture.packets.get("factory").unwrap();
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let private = PathBuf::from(fixture["private"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let source_ref = fixture["source_ref"].as_str().unwrap();
    let authored = authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in &fixture_capture
        .software_identity_migration
        .native_owner_paths
    {
        let raw = fs::read(repository.join(reference.as_str())).unwrap();
        assert!(captured.insert(reference.clone(), raw).is_none());
    }
    let (_capture, software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = super::command_form_cases::open_cut(&store, selected, deadline, &cancelled);
    let mut worker_image_budget = ExecutorBudget::laboratory();
    worker_image_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!worker_image_budget.execution_wall.is_zero());
    let worker_image =
        super::command_form_cases::schema_image(worker_image_budget, deadline, &cancelled);
    let mut preview_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let preview = prepare_initial_text_layer_from_captures(
        &context,
        &owner,
        &cut,
        &software,
        &components,
        &mut preview_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(preview_worker);
    assert_eq!(preview.source_path, source_ref);
    let request = source_value(&serde_json::json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"text-layer.create",
        "command_id":"synthetic-native-text-layer-1",
        "expected_configuration":preview.owner_configuration,
        "expected_dependencies":preview.expected_dependencies,
        "expected_source":null,
        "expected_revision":null
    }));
    let mut create_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let created = execute_initial_text_layer_from_captures(
        &context,
        &owner,
        &request,
        &cut,
        &software,
        &components,
        &mut create_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(create_worker);
    assert!(!created.replayed);
    assert!(!created.grants_admission);
    let package = private.join(source_ref).parent().unwrap().to_path_buf();
    let retained = fs::read(package.join("content.txt")).unwrap();
    assert_eq!(
        retained.len(),
        fixture["content_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(
        Digest256::of_bytes(&retained).to_hex(),
        fixture["content_sha256"].as_str().unwrap()
    );
    assert_eq!(
        fs::metadata(&package).unwrap().uid(),
        fs::metadata(&private).unwrap().uid()
    );
    assert!(!public.join(source_ref).exists());
    let saved: BTreeMap<_, _> = fs::read_dir(&package)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    let mut replay_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let replay = execute_initial_text_layer_from_captures(
        &context,
        &owner,
        &request,
        &cut,
        &software,
        &components,
        &mut replay_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(replay_worker);
    assert!(replay.replayed);
    assert_eq!(created.receipt, replay.receipt);
    let after: BTreeMap<_, _> = fs::read_dir(&package)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    assert_eq!(after, saved);
    let layer_raw = fs::read(package.join("source-text-layer.v1.json")).unwrap();
    let layer: Value = serde_json::from_slice(&layer_raw).unwrap();
    let mut unit_config = fixture["unit_config"].clone();
    unit_config["schema_version"] = fixture["unit_layer_schema"].clone();
    unit_config["source_binding"] = serde_json::json!({
        "schema_version":"tos_native_text_layer_binding_v1",
        "text_layer":{
            "record_ref":source_ref,
            "record_sha256":Digest256::of_bytes(&layer_raw).to_hex(),
            "layer_id":layer["layer_id"],
            "layer_version":layer["layer_version"]
        },
        "source_record_refs":fixture["config"]["source_record_refs"]
    });
    let unit_owner = temporary.path().join("unit-layer-owner.json");
    fs::write(&unit_owner, serde_json::to_vec(&unit_config).unwrap()).unwrap();
    fs::set_permissions(&unit_owner, fs::Permissions::from_mode(0o600)).unwrap();
    let unit_proposal = fixture["unit_proposal"].clone();
    let mut unit_preview_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let unit_preview = prepare_first_text_unit_from_captures(
        &context,
        &unit_owner,
        &source_value(&unit_proposal),
        &cut,
        &software,
        &components,
        &mut unit_preview_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(unit_preview_worker);
    let mut unit_request = unit_proposal;
    unit_request["operation"] = Value::String("text-unit.create".into());
    unit_request["command_id"] = Value::String("synthetic-native-first-unit-1".into());
    unit_request["expected_configuration"] = Value::String(unit_preview.owner_configuration);
    unit_request["expected_dependencies"] = Value::String(unit_preview.expected_dependencies);
    unit_request["expected_source"] = Value::Null;
    unit_request["expected_revision"] = Value::Null;
    let unit_request = source_value(&unit_request);
    let mut unit_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let unit_created = execute_first_text_unit_from_captures(
        &context,
        &unit_owner,
        &unit_request,
        &cut,
        &software,
        &components,
        &mut unit_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(unit_worker);
    assert!(!unit_created.replayed);
    assert!(!unit_created.grants_admission);
    let unit_home = private
        .join(&unit_preview.source_path)
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(unit_home.join("source-text-unit.v1.json").exists());
    let mut unit_replay_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let unit_replay = execute_first_text_unit_from_captures(
        &context,
        &unit_owner,
        &unit_request,
        &cut,
        &software,
        &components,
        &mut unit_replay_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(unit_replay_worker);
    assert!(unit_replay.replayed);
    assert_eq!(unit_created.receipt, unit_replay.receipt);
    let derived_capture = derived_fixture(temporary.path(), "normalize");
    let derived = derived_capture.packets.get("factory").unwrap();
    let derived_owner = PathBuf::from(derived["owner"].as_str().unwrap());
    let derived_ref = derived["source_ref"].as_str().unwrap();
    let mut derived_preview_worker =
        text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let derived_preview = prepare_derived_text_layer_from_captures(
        &context,
        &derived_owner,
        &cut,
        &software,
        &components,
        &mut derived_preview_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(derived_preview_worker);
    assert_eq!(derived_preview.source_path, derived_ref);
    let derived_request = source_value(&serde_json::json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"text-layer.normalize",
        "command_id":"synthetic-native-text-normalize-1",
        "expected_configuration":derived_preview.owner_configuration,
        "expected_dependencies":derived_preview.expected_dependencies,
        "expected_source":null,
        "expected_revision":null
    }));
    let mut derived_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let derived_result = execute_derived_text_layer_from_captures(
        &context,
        &derived_owner,
        &derived_request,
        &cut,
        &software,
        &components,
        &mut derived_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(derived_worker);
    assert!(!derived_result.replayed);
    assert!(!derived_result.grants_admission);
    let derived_home = private.join(derived_ref).parent().unwrap().to_path_buf();
    let normalized = fs::read(derived_home.join("content.txt")).unwrap();
    assert_eq!(
        normalized.len(),
        derived["expected_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(
        Digest256::of_bytes(&normalized).to_hex(),
        derived["expected_sha256"].as_str().unwrap()
    );
    assert_eq!(fs::read(package.join("content.txt")).unwrap(), retained);
    let mut derived_replay_worker =
        text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
    let derived_replay = execute_derived_text_layer_from_captures(
        &context,
        &derived_owner,
        &derived_request,
        &cut,
        &software,
        &components,
        &mut derived_replay_worker,
        deadline,
        &cancelled,
    )
    .unwrap();
    drop(derived_replay_worker);
    assert!(derived_replay.replayed);
    assert_eq!(derived_result.receipt, derived_replay.receipt);
    assert!(!public.join(derived_ref).exists());

    // The same already-created Rust predecessor also feeds one exact edit
    // proposal and one separately supplied OCR result. Neither branch calls
    // OCR inference or accepts the proposal as an assessed reading.
    for (kind, expected_role) in [
        ("correct", "diplomatic_transcription"),
        ("record-ocr", "raw_ocr"),
    ] {
        let details_capture = derived_fixture(temporary.path(), kind);
        let details = details_capture.packets.get("factory").unwrap();
        let owner_path = PathBuf::from(details["owner"].as_str().unwrap());
        let source_ref = details["source_ref"].as_str().unwrap();
        let mut preview_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
        let preview = prepare_derived_text_layer_from_captures(
            &context,
            &owner_path,
            &cut,
            &software,
            &components,
            &mut preview_worker,
            deadline,
            &cancelled,
        )
        .unwrap();
        drop(preview_worker);
        assert_eq!(preview.source_path, source_ref);
        let operation = if kind == "correct" {
            "text-layer.correct"
        } else {
            "text-layer.record-ocr"
        };
        let request = source_value(&serde_json::json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":operation,
            "command_id":format!("synthetic-native-text-{kind}-1"),
            "expected_configuration":preview.owner_configuration,
            "expected_dependencies":preview.expected_dependencies,
            "expected_source":null,
            "expected_revision":null
        }));
        let mut worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
        let created = execute_derived_text_layer_from_captures(
            &context,
            &owner_path,
            &request,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancelled,
        )
        .unwrap();
        drop(worker);
        assert!(!created.replayed);
        assert!(!created.grants_admission);
        let home = private.join(source_ref).parent().unwrap().to_path_buf();
        let content = fs::read(home.join("content.txt")).unwrap();
        assert_eq!(
            content.len(),
            details["expected_bytes"].as_u64().unwrap() as usize
        );
        assert_eq!(
            Digest256::of_bytes(&content).to_hex(),
            details["expected_sha256"].as_str().unwrap()
        );
        let layer: Value =
            serde_json::from_slice(&fs::read(home.join("source-text-layer.v1.json")).unwrap())
                .unwrap();
        assert_eq!(layer["layer_role"], expected_role);
        let mut retry_worker = text_worker_with_image(&cut, &worker_image, deadline, &cancelled);
        let retry = execute_derived_text_layer_from_captures(
            &context,
            &owner_path,
            &request,
            &cut,
            &software,
            &components,
            &mut retry_worker,
            deadline,
            &cancelled,
        )
        .unwrap();
        drop(retry_worker);
        assert!(retry.replayed);
        assert_eq!(retry.receipt, created.receipt);
        assert_eq!(fs::read(package.join("content.txt")).unwrap(), retained);
        assert!(!public.join(source_ref).exists());
    }
}
