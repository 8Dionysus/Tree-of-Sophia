//! The maintained fixture supplies a synthetic owner, rights, source records
//! and an acquired EPUB. Rust performs the private publication and replay.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
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
    'source_ref':case.source_ref,'content_sha256':layer.source._digest(case.content)[7:],
    'content_bytes':len(case.content),'config':case.config,
    'unit_config':case.seed.config,'unit_proposal':case.seed.proposal,
    'unit_layer_schema':unit.native.LAYER_CONFIG,
    'implementations':sorted(set(layer.layers.IMPLEMENTATIONS))},ensure_ascii=False,separators=(',',':')))
"#;
    let mut child = Command::new("/usr/bin/python3")
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

fn derived_fixture(
    repository: &Path,
    root: &Path,
    output: &Path,
    errors: &Path,
    kind: &str,
    deadline: Instant,
) -> Value {
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
    let mut child = Command::new("/usr/bin/python3")
        .args(["-c", script])
        .arg(repository)
        .arg(root)
        .arg(kind)
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
            panic!("bounded maintained derived Text fixture refused");
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

fn authored_text_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
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
                if tos_source_store::is_authored_source_path_v1(&reference) {
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

fn source_value(value: &Value) -> JsonValue {
    parse_json(
        &serde_json::to_vec(value).unwrap(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}

#[test]
fn native_text_layer_extracts_private_epub_and_cold_replays() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::tempdir().unwrap();
    let fixture = text_fixture(
        &repository,
        temporary.path(),
        &temporary.path().join("text-fixture.stdout"),
        &temporary.path().join("text-fixture.stderr"),
        deadline,
    );
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let private = PathBuf::from(fixture["private"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let source_ref = fixture["source_ref"].as_str().unwrap();
    let authored = authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in fixture["implementations"].as_array().unwrap() {
        let reference = reference.as_str().unwrap();
        let raw = fs::read(repository.join(reference)).unwrap();
        assert!(captured.insert(reference.to_owned(), raw).is_none());
    }
    let (_capture, software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = super::command_form_cases::open_cut(&store, selected, deadline, &cancelled);
    let mut preview_worker = text_worker(&cut, deadline, &cancelled);
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
    let mut create_worker = text_worker(&cut, deadline, &cancelled);
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
    let mut replay_worker = text_worker(&cut, deadline, &cancelled);
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
    let mut unit_preview_worker = text_worker(&cut, deadline, &cancelled);
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
    let mut unit_worker = text_worker(&cut, deadline, &cancelled);
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
    let mut unit_replay_worker = text_worker(&cut, deadline, &cancelled);
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
    assert!(unit_replay.replayed);
    assert_eq!(unit_created.receipt, unit_replay.receipt);
    let derived = derived_fixture(
        &repository,
        temporary.path(),
        &temporary.path().join("derived-fixture.stdout"),
        &temporary.path().join("derived-fixture.stderr"),
        "normalize",
        deadline,
    );
    let derived_owner = PathBuf::from(derived["owner"].as_str().unwrap());
    let derived_ref = derived["source_ref"].as_str().unwrap();
    let mut derived_preview_worker = text_worker(&cut, deadline, &cancelled);
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
    let mut derived_worker = text_worker(&cut, deadline, &cancelled);
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
    let mut derived_replay_worker = text_worker(&cut, deadline, &cancelled);
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
        let details = derived_fixture(
            &repository,
            temporary.path(),
            &temporary.path().join(format!("{kind}-fixture.stdout")),
            &temporary.path().join(format!("{kind}-fixture.stderr")),
            kind,
            deadline,
        );
        let owner_path = PathBuf::from(details["owner"].as_str().unwrap());
        let source_ref = details["source_ref"].as_str().unwrap();
        let mut preview_worker = text_worker(&cut, deadline, &cancelled);
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
        let mut worker = text_worker(&cut, deadline, &cancelled);
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
        let mut retry_worker = text_worker(&cut, deadline, &cancelled);
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
