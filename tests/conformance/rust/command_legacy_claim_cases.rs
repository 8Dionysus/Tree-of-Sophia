//! One retained historical Claim owner lifecycle over the maintained Python
//! oracle and the fixed native child. Synthetic inputs grant no admission.
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_source_store::{ReadLimits, SoftwareCaptureReader};

const FIXTURE: &str = r#"
import copy,hashlib,json,resource,shutil,sys
from pathlib import Path
resource.setrlimit(resource.RLIMIT_CPU,(45,45))
resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824))
repo,bounded_root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repo/'mechanics/growth-cycle/tests'),
    str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),
    str(repo/'scripts'),str(repo/'access/tests')]
import source_assembly_fixture
from source_assembly_fixture import SourceAssemblyFixture
import source_commands as commands
import source_historical_claims as legacy
import source_revisions as packages
import test_historical_claim_adapter as maintained

oracle_calls=0
def oracle(owner,request):
    global oracle_calls
    oracle_calls+=1
    return commands.run_legacy_oracle_command(owner,request)

class ExistingRoot:
    def __enter__(self): return str(bounded_root)
    def __exit__(self,*args): return False
class TempfileProxy:
    def __init__(self,wrapped): self.wrapped=wrapped
    def __getattr__(self,name): return getattr(self.wrapped,name)
    def TemporaryDirectory(self,*args,**kwargs): return ExistingRoot()

old_tempfile=source_assembly_fixture.tempfile
source_assembly_fixture.tempfile=TempfileProxy(old_tempfile)
try:
    assembly=SourceAssemblyFixture(code_root=repo,
        source_root=repo/'access/tests/fixtures/source-assembly')
    with assembly.historical_fixture() as (root,history,real,old_claims,rebuild):
        rebuild()
        record={**copy.deepcopy(history[0][1]),
            'record_id':'tos.historical-event.creation-fixture',
            'extensions':{'unknown':{'negative':False,'missing':None}}}
        claims=[{**copy.deepcopy(claim),'claim_id':f'tos.claim.creation-fixture-{index}',
            'subject_ref':record['record_id']} for index,claim in enumerate(old_claims)]
        relative='ToS/source-witnesses/history/new-subject/historical-event.json'
        config={'schema_version':'tos_local_historical_create_owner_v1','uid':__import__('os').getuid(),
            'principal_id':'software:test-fixture','maker_type':'software','source_root':str(root),
            'source_path':relative,'record_id':record['record_id'],
            'authority_ref':'synthetic-test-only:creation-not-assessment',
            'allowed_form_ids':['tos.form.creation-name','tos.form.creation-hover'],
            'allowed_claim_ids':[claim['claim_id'] for claim in claims],
            'allowed_operations':[commands.CREATION_OPERATION],
            'expires_at':'2099-01-01T00:00:00Z'}
        owner=root/'owner.json'
        owner.write_text(json.dumps(config))
        context=oracle(owner,{'schema_version':'tos_local_source_command_v1','operation':'describe'})
        if context['target_exists'] or context['allowed_operations']!=['historical.create']:
            raise RuntimeError('maintained historical creation context differs')
        request={'schema_version':'tos_local_source_command_v1','operation':'historical.create',
            'command_id':'synthetic:create-first','expected_configuration':context['owner_configuration'],
            'expected_source':None,'expected_revision':None,'record':record,'claims':claims,
            'forms':[{'form_id':'tos.form.creation-name','field_id':'metadata.preferred-name'},
                {'form_id':'tos.form.creation-hover','field_id':'metadata.source-note'}]}
        prepared=oracle(owner,{'schema_version':'tos_local_source_command_v1','operation':'prepare',
            'record':record})
        if (root/relative).parent.exists() or not prepared.get('prepared_source'):
            raise RuntimeError('maintained historical source preparation differs')
        initial_preview=oracle(owner,{'schema_version':'tos_local_source_command_v1',
            'operation':'prepare-create','record':record,'claims':claims,'forms':request['forms']})
        request['expected_dependencies']=initial_preview['expected_dependencies']
        if (root/relative).parent.exists():
            raise RuntimeError('maintained historical creation preview wrote its target')
        for ref in (*legacy.CONTRACT_REFS,'ToS/contracts/provenance-event-v2.schema.json',
                'ToS/contracts/human-form.schema.json','ToS/contracts/human-form-set.schema.json',
                'ToS/contracts/human-form-template.schema.json'):
            path=root/ref
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_bytes((repo/ref).read_bytes())
        config.update(schema_version='tos_local_historical_create_owner_v2',
            provenance_event_id='tos.event.creation-fixture')
        date={**copy.deepcopy(request['claims'][0]),'claim_id':'tos.claim.creation-fixture-date',
            'predicate':'historical_dating','object':{'kind':'date-assertion','role':'historical-time',
                'calendar':None,'year_numbering':None,'certainty':'uncertain','value':'1900-01-01',
                'source_wording':{'text':'Synthetic date only','language':'en'}},
            'qualifiers':{'synthetic_evidence_limit':
                'No historical claim; temporal reader fixture only.'}}
        request['claims'].append(date)
        config['allowed_claim_ids'].append(date['claim_id'])
        owner.write_text(json.dumps(config))
        for claim in request['claims']:
            claim['provenance_event_ref']=config['provenance_event_id']
        preview=oracle(owner,{'schema_version':'tos_local_source_command_v1',
            'operation':'prepare-create','record':request['record'],'claims':request['claims'],
            'forms':request['forms']})
        request.update(expected_configuration=preview['owner_configuration'],
            expected_dependencies=preview['expected_dependencies'])
        created=oracle(owner,request)
        record_path=root/config['source_path']
        stream_path=record_path.with_name(legacy.BASENAME)
        original_package=packages._package(stream_path.parent)
        if created['receipt']['files'][legacy.BASENAME]['sha256']!=commands._digest(
                original_package[legacy.BASENAME]):
            raise RuntimeError('maintained captured Claim stream differs')

        case=maintained.HistoricalClaimAdapterTests(methodName='runTest')
        case.root=root
        case.creation_owner=owner
        case.creation_config=config
        case.creation_request=request
        case.record_path=record_path
        case.path=stream_path
        case.original=original_package
        revision_owner,revision_config=case.grant(0)
        proposal=case.proposal(0)
        revision_preview_request={'schema_version':'tos_local_source_command_v1',
            'operation':'prepare-revise',**proposal}
        revision_preview=oracle(revision_owner,revision_preview_request)
        revision_request={'schema_version':'tos_local_source_command_v1','operation':'claim.revise',
            'command_id':f'test:legacy-0-{revision_preview["source"]["version"]}',
            'expected_source':revision_preview['source'],'expected_revision':revision_preview['revision'],
            'expected_configuration':revision_preview['owner_configuration'],
            'expected_dependencies':revision_preview['expected_dependencies'],
            'expected_inputs':revision_preview['source_bindings'],**proposal}
        revision_result=oracle(revision_owner,revision_request)
        revision_replay=oracle(revision_owner,revision_request)
        inspected=oracle(revision_owner,{'schema_version':'tos_local_source_command_v1',
            'operation':'inspect-version','source':revision_preview['source']})
        after_revision=packages._package(stream_path.parent)

        form_config={key:value for key,value in revision_config.items()
            if key not in {'allowed_fields','allowed_qualifier_fields','allowed_evidence_refs'}}
        form_config.update(schema_version=legacy.FORM_CONFIG,allowed_operations=['form.create'],
            allowed_form_ids=['tos.form.synthetic-historical-extra-name'],
            allowed_form_field_ids=['claim.name'])
        form_owner=root/'form-owner.json'
        form_owner.write_text(json.dumps(form_config))
        form_prepare_request={'schema_version':'tos_local_source_command_v1','operation':'prepare',
            'form_id':form_config['allowed_form_ids'][0],'field_id':'claim.name'}
        form_preview=oracle(form_owner,form_prepare_request)
        form_request={'schema_version':'tos_local_source_command_v1','operation':'apply',
            'command_id':'test:legacy-form','expected_source':form_preview['source'],
            'expected_revision':form_preview['revision'],
            'expected_configuration':form_preview['owner_configuration'],
            'changes':[form_preview['prepared_change']]}
        form_result=oracle(form_owner,form_request)
        form_replay=oracle(form_owner,form_request)
        final_package=packages._package(stream_path.parent)
        form_path=commands.claim_forms_path(stream_path,request['claims'][0]['claim_id'])
        if final_package[legacy.BASENAME]!=after_revision[legacy.BASENAME]:
            raise RuntimeError('maintained form writer changed the historical Claim stream')
        archive_ref=revision_result['receipt']['archive_path']
        if not archive_ref.startswith('ToS/source-witnesses/.record-revisions/'):
            raise RuntimeError('maintained Claim archive path is not receipt-bound')
        archive_path=root/archive_ref
        if archive_path.is_symlink():
            raise RuntimeError('maintained synthetic archive unexpectedly became a symlink')
        if archive_path.exists(): shutil.rmtree(archive_path)
        current_package=packages._package(stream_path.parent)
        for name in current_package.keys()-original_package.keys():
            (stream_path.parent/name).unlink()
        for name,raw in original_package.items():
            (stream_path.parent/name).write_bytes(raw)
        print(json.dumps({'root':str(root),'source_path':config['source_path'],
            'stream_path':stream_path.relative_to(root).as_posix(),
            'generated_prefix':stream_path.parent.relative_to(root).as_posix()+'/',
            'claim_id':request['claims'][0]['claim_id'],
            'record_id':config['record_id'],'revision_owner':str(revision_owner),
            'form_owner':str(form_owner),'revision_config':revision_config,
            'form_config':form_config,'revision_preview_request':revision_preview_request,
            'revision_request':revision_request,'form_prepare_request':form_prepare_request,
            'form_request':form_request,'archive_path':archive_ref,
            'claim_form_path':form_path.relative_to(root).as_posix(),
            'initial_stream_sha256':hashlib.sha256(original_package[legacy.BASENAME]).hexdigest(),
            'revised_stream_sha256':hashlib.sha256(after_revision[legacy.BASENAME]).hexdigest(),
            'final_form_payload':json.loads(final_package[form_path.name]),
            'oracle_calls':oracle_calls,
            'expected':{'revision_preview':revision_preview,'revision_result':revision_result,
                'revision_replay':revision_replay,'inspect':inspected,'form_preview':form_preview,
                'form_result':form_result,'form_replay':form_replay}},
            ensure_ascii=False,allow_nan=False,separators=(',',':')))
finally:
    source_assembly_fixture.tempfile=old_tempfile
"#;

#[derive(Default)]
struct ProcessLedger {
    direct_spawns: usize,
    git_spawns: usize,
    capture_python_spawns: usize,
    capture_git_spawns: usize,
    fixture_python_spawns: usize,
    native_cli_python_spawns: usize,
    native_owner_spawns: usize,
    native_schema_worker_spawns: usize,
}

fn canonical(value: &Value) -> Vec<u8> {
    tos_foundation::canonical_raw_bytes_v1(
        &serde_json::to_vec(value).unwrap(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
    )
    .unwrap()
}

fn bounded_process(
    command: &mut Command,
    deadline: Instant,
    output_cap: u64,
    error_cap: u64,
    ledger: &mut ProcessLedger,
) -> (std::process::ExitStatus, Vec<u8>, Vec<u8>) {
    assert!(Instant::now() < deadline);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") || key == "PYTHONPATH" || key == "PYTHONHOME" {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("PYTHONDONTWRITEBYTECODE", "1");
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(errors.try_clone().unwrap()))
        .spawn()
        .unwrap();
    ledger.direct_spawns = ledger.direct_spawns.checked_add(1).unwrap();
    let step_deadline = deadline.min(Instant::now() + Duration::from_secs(75));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step_deadline
            || output.metadata().unwrap().len() > output_cap
            || errors.metadata().unwrap().len() > error_cap
        {
            let _ = Command::new("/usr/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            panic!("bounded Legacy Claim subprocess refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step_deadline);
    assert!(output.metadata().unwrap().len() <= output_cap);
    assert!(errors.metadata().unwrap().len() <= error_cap);
    output.seek(SeekFrom::Start(0)).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut output_bytes = Vec::new();
    let mut error_bytes = Vec::new();
    output.read_to_end(&mut output_bytes).unwrap();
    errors.read_to_end(&mut error_bytes).unwrap();
    (status, output_bytes, error_bytes)
}

fn fixture(repository: &Path, root: &Path, deadline: Instant, ledger: &mut ProcessLedger) -> Value {
    let script = root.parent().unwrap().join("legacy-claim-fixture.py");
    fs::write(&script, FIXTURE).unwrap();
    let mut command = Command::new(crate::maintained_python());
    command
        .args(["-c", "import resource,runpy,sys;resource.setrlimit(resource.RLIMIT_CPU,(45,45));resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824));p=sys.argv[1];sys.argv=sys.argv[1:];runpy.run_path(p,run_name='__main__')"])
        .arg(&script)
        .arg(repository)
        .arg(root);
    ledger.fixture_python_spawns = ledger.fixture_python_spawns.checked_add(1).unwrap();
    let (status, output, errors) =
        bounded_process(&mut command, deadline, 4_194_304, 1_048_576, ledger);
    assert!(
        status.success(),
        "Legacy Claim fixture: {}",
        String::from_utf8_lossy(&errors)
    );
    serde_json::from_slice(&output).unwrap()
}

fn source_members(root: &Path, deadline: Instant) -> (BTreeMap<String, Vec<u8>>, u64) {
    let mut directories = vec![root.join("ToS")];
    let mut entries = 0usize;
    let mut directories_seen = 0usize;
    let mut count = 0usize;
    let mut bytes = 0u64;
    while let Some(directory) = directories.pop() {
        assert!(Instant::now() < deadline);
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            entries = entries.checked_add(1).unwrap();
            assert!(entries <= 4352);
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            let path = entry.path();
            let reference = path.strip_prefix(root).unwrap().to_str().unwrap();
            assert!(reference.len() <= 512 && reference.split('/').count() <= 16);
            if kind.is_dir() {
                if tos_source_store::has_authored_source_descendants_v1(reference) {
                    directories_seen = directories_seen.checked_add(1).unwrap();
                    assert!(directories_seen <= 4096);
                    directories.push(path);
                }
            } else if tos_source_store::is_authored_source_path_v1(reference)
                && reference != "ToS/source-witnesses/.historical-create.writer.lock"
            {
                assert!(kind.is_file());
                let size = entry.metadata().unwrap().len();
                assert!(size <= 8_388_608);
                count = count.checked_add(1).unwrap();
                bytes = bytes.checked_add(size).unwrap();
                assert!(count <= 256 && bytes <= 8_388_608);
            }
        }
    }
    let mut files = super::command_text_cases::authored_text_files(root);
    files.remove("ToS/source-witnesses/.historical-create.writer.lock");
    assert_eq!(files.len(), count);
    assert_eq!(
        files.values().map(|raw| raw.len() as u64).sum::<u64>(),
        bytes
    );
    (files, bytes)
}

fn charge_software(names: &[String], repository: &Path, total: u64, files: usize) -> (u64, usize) {
    let mut total = total;
    let mut files = files;
    for name in names {
        assert!(name.len() <= 512 && name.split('/').count() <= 16);
        let metadata = fs::symlink_metadata(repository.join(name)).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        let size = metadata.len();
        assert!(size <= 8_388_608);
        total = total.checked_add(size).unwrap();
        files = files.checked_add(1).unwrap();
        assert!(files <= 256 && total <= 8_388_608);
    }
    (total, files)
}

fn selected_software(
    repository: &Path,
    names: &[String],
    deadline: Instant,
    ledger: &mut ProcessLedger,
) -> (
    super::source_cut_cases::SoftwareCaptureFixture,
    SoftwareCaptureReader,
    tos_source_store::SoftwareComponentSelectionV1,
) {
    let output = {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(repository)
            .args(["rev-parse", "HEAD^{commit}"]);
        ledger.git_spawns += 1;
        bounded_process(&mut command, deadline, 1_048_576, 1_048_576, ledger)
    };
    assert!(output.0.success());
    let commit = String::from_utf8(output.1).unwrap().trim().to_owned();
    assert!(
        commit.len() == 40
            && commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    let temporary = tempfile::tempdir().unwrap();
    let capture = temporary.path().join("software-capture");
    let restored = temporary.path().join("software-restored");
    let tool = temporary.path().join("corpus_archive.py");
    let source = {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(repository)
            .arg("show")
            .arg(format!("{commit}:scripts/corpus_archive.py"));
        ledger.git_spawns += 1;
        bounded_process(&mut command, deadline, 1_048_576, 1_048_576, ledger)
    };
    assert!(source.0.success());
    fs::write(&tool, source.1).unwrap();
    let wrapper = "import resource,runpy,sys;resource.setrlimit(resource.RLIMIT_CPU,(20,20));resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824));sys.argv=sys.argv[1:];runpy.run_path(sys.argv[0],run_name='__main__')";
    let mut capture_command = Command::new(crate::maintained_python());
    capture_command
        .args(["-c", wrapper])
        .arg(&tool)
        .arg("capture")
        .arg("--repo-root")
        .arg(repository)
        .arg("--commit")
        .arg(&commit)
        .arg("--output")
        .arg(&capture);
    for name in names {
        capture_command.arg("--include-prefix").arg(name);
    }
    ledger.capture_python_spawns += 1;
    let captured = bounded_process(&mut capture_command, deadline, 1_048_576, 1_048_576, ledger);
    assert!(
        captured.0.success(),
        "software capture: {}",
        String::from_utf8_lossy(&captured.2)
    );
    // corpus_archive.capture performs two git rev-parse calls, one git
    // ls-tree call, and one git cat-file --batch process for this capture.
    ledger.capture_git_spawns += 4;
    ledger.direct_spawns += 4;
    let mut restore_command = Command::new(crate::maintained_python());
    restore_command
        .args(["-c", wrapper])
        .arg(&tool)
        .arg("restore")
        .arg("--capture")
        .arg(&capture)
        .arg("--output")
        .arg(&restored);
    ledger.capture_python_spawns += 1;
    let restored_run =
        bounded_process(&mut restore_command, deadline, 1_048_576, 1_048_576, ledger);
    assert!(
        restored_run.0.success(),
        "software restore: {}",
        String::from_utf8_lossy(&restored_run.2)
    );
    let manifest_raw = fs::read(capture.join("capture.json")).unwrap();
    assert!(manifest_raw.len() <= 1_048_576);
    let manifest: Value = serde_json::from_slice(&manifest_raw).unwrap();
    assert_eq!(manifest["source_git_commit"], commit);
    let selection = tos_source_store::SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree: manifest["source_git_tree"].as_str().unwrap().to_owned(),
        capture_manifest_sha256: tos_foundation::Digest256::of_bytes(&manifest_raw),
    };
    let fixture = super::source_cut_cases::SoftwareCaptureFixture {
        temporary,
        capture,
        restored,
        selection,
    };
    let cancelled = AtomicBool::new(false);
    let software = SoftwareCaptureReader::open(
        &fixture.capture,
        &fixture.restored,
        fixture.selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 2_097_152,
            json: JsonLimits::default(),
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let paths = names
        .iter()
        .map(|name| tos_foundation::RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    assert_eq!(components.members().count(), names.len());
    for name in names {
        let path = tos_foundation::RelativePath::parse(name).unwrap();
        let captured = software
            .read_selected_component(&components, &path, 8_388_608, deadline, &cancelled)
            .unwrap();
        assert_eq!(
            captured,
            fs::read(repository.join(name)).unwrap(),
            "selected source {name}"
        );
    }
    (fixture, software, components)
}

fn comparable(value: &Value, ignore_revision: bool) -> Value {
    let mut normalized = value.clone();
    if let Some(object) = normalized.as_object_mut() {
        if ignore_revision {
            object.remove("revision");
        }
        if let Some(receipt) = object.get_mut("receipt").and_then(Value::as_object_mut) {
            receipt.remove("recorded_at");
        }
    }
    normalized
}

fn request_digest(value: &Value) -> String {
    tos_foundation::Digest256::of_bytes(&canonical(value)).to_prefixed()
}

fn archive_reference(record_id: &str, revision: &str) -> String {
    let revision = revision.strip_prefix("sha256:").unwrap();
    assert_eq!(revision.len(), 64);
    format!(
        "ToS/source-witnesses/.record-revisions/{}-{revision}",
        tos_foundation::Digest256::of_bytes(record_id.as_bytes()).to_hex()
    )
}

fn assert_oracle(actual: &Value, expected: &Value, ignore_revision: bool, label: &str) {
    assert_eq!(
        comparable(actual, ignore_revision),
        comparable(expected, ignore_revision),
        "native Legacy Claim {label} differs from maintained oracle"
    );
}

fn assert_form_mutation(actual: &Value, expected: &Value, request: &Value, label: &str) {
    let digest = request_digest(request);
    let expected_revision = &request["expected_revision"];
    assert_eq!(actual["receipt"]["request_digest"], digest);
    assert_eq!(actual["receipt"]["previous_revision"], *expected_revision);
    let mut expected = expected.clone();
    expected["receipt"]["request_digest"] = Value::String(digest);
    expected["receipt"]["previous_revision"] = expected_revision.clone();
    assert_oracle(actual, &expected, true, label);
}

fn assert_revision_mutation(
    actual: &Value,
    expected: &Value,
    request: &Value,
    record_id: &str,
    label: &str,
) -> String {
    let digest = request_digest(request);
    let previous_revision = request["expected_revision"].as_str().unwrap();
    let archive = archive_reference(record_id, previous_revision);
    assert_eq!(actual["receipt"]["request"], *request);
    assert_eq!(actual["receipt"]["request_digest"], digest);
    assert_eq!(actual["receipt"]["previous_revision"], previous_revision);
    assert_eq!(actual["receipt"]["archive_path"], archive);
    let mut expected = expected.clone();
    expected["receipt"]["request"] = request.clone();
    expected["receipt"]["request_digest"] = Value::String(digest);
    expected["receipt"]["previous_revision"] = Value::String(previous_revision.to_owned());
    expected["receipt"]["archive_path"] = Value::String(archive.clone());
    assert_oracle(actual, &expected, true, label);
    archive
}

fn assert_inspection(actual: &Value, expected: &Value, archive: &str, label: &str) {
    let mut expected = expected.clone();
    for (name, expected_location) in expected["files"].as_object_mut().unwrap() {
        let digest = expected_location["sha256"].as_str().unwrap();
        let blob = format!("{}.blob", digest.strip_prefix("sha256:").unwrap());
        let reference = format!("{archive}/{blob}");
        assert_eq!(actual["files"][name]["archive_path"], reference);
        expected_location["archive_path"] = Value::String(reference);
    }
    assert_oracle(actual, &expected, true, label);
}

fn comparable_form_payload(value: &Value, request: &Value, verify_actual: bool) -> Value {
    let mut normalized = value.clone();
    let command_id = request["command_id"].as_str().unwrap();
    let digest = request_digest(request);
    let previous_revision = request["expected_revision"].clone();
    let receipts = normalized["growth_history"].as_array_mut().unwrap();
    let mut found = false;
    for receipt in receipts {
        if receipt["command_id"].as_str() != Some(command_id) {
            continue;
        }
        assert!(!found, "form command receipt is duplicated");
        found = true;
        if verify_actual {
            assert_eq!(receipt["request_digest"], digest);
            assert_eq!(receipt["previous_revision"], previous_revision);
        }
        let receipt = receipt.as_object_mut().unwrap();
        receipt.insert("request_digest".into(), Value::String(digest.clone()));
        receipt.insert("previous_revision".into(), previous_revision.clone());
        receipt.remove("recorded_at");
    }
    assert!(found, "form command receipt is absent");
    normalized
}

fn native_call(
    repository: &Path,
    owner: &Path,
    invocation_path: &Path,
    invocation: &Value,
    request: &Value,
    deadline: Instant,
    ledger: &mut ProcessLedger,
) -> Value {
    assert!(Instant::now() < deadline);
    fs::write(invocation_path, canonical(invocation)).unwrap();
    fs::set_permissions(invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    // Each successful owner call starts the CLI, the protected owner, and
    // one disposable schema worker child.
    ledger.direct_spawns = ledger.direct_spawns.checked_add(3).unwrap();
    ledger.native_cli_python_spawns += 1;
    ledger.native_owner_spawns += 1;
    ledger.native_schema_worker_spawns += 1;
    let response = super::command_text_cases::alignment_native_cli(
        repository,
        owner,
        invocation_path,
        request,
        deadline,
    );
    assert_eq!(
        response["schema_version"],
        "tos_local_native_source_result_v1"
    );
    assert_eq!(response["grants_admission"], false);
    response["result"].clone()
}

#[test]
fn native_legacy_historical_claim_revision_forms_and_cold_lineage_match_oracle() {
    let deadline = Instant::now() + Duration::from_secs(240);
    let mut ledger = ProcessLedger::default();
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("historical-source-root");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let fixture = fixture(&repository, &root, deadline, &mut ledger);
    let root = PathBuf::from(fixture["root"].as_str().unwrap());
    let revision_owner = PathBuf::from(fixture["revision_owner"].as_str().unwrap());
    let form_owner = PathBuf::from(fixture["form_owner"].as_str().unwrap());
    fs::set_permissions(&revision_owner, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&form_owner, fs::Permissions::from_mode(0o600)).unwrap();

    let (authored, authored_bytes) = source_members(&root, deadline);
    let generated_prefix = fixture["generated_prefix"].as_str().unwrap();
    let original = authored
        .iter()
        .filter(|(name, _)| !name.starts_with(generated_prefix))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    assert!(!original.is_empty() && original.len() < authored.len());
    assert!(
        authored
            .keys()
            .any(|name| name.starts_with(generated_prefix))
    );
    let initial_stream = fs::read(root.join(fixture["stream_path"].as_str().unwrap())).unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&initial_stream).to_hex(),
        fixture["initial_stream_sha256"].as_str().unwrap()
    );

    let mut software_names = vec![
        "access/tests/source_assembly_fixture.py",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/agents/friedrich-nietzsche/agent.json",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/places/chemnitz/place.json",
        "access/tests/fixtures/source-assembly/ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_version_reader.py",
        "mechanics/growth-cycle/tests/test_source_commands.py",
        "mechanics/growth-cycle/tests/test_historical_claim_adapter.py",
        "scripts/source_witness_bibliographic_graph_common.py",
        "scripts/build_source_witness_catalog.py",
        "scripts/source_record_profiles.py",
        "scripts/source_witness_human_forms.py",
        "scripts/source_metadata_snapshot.py",
        "scripts/corpus_archive.py",
        "rust/crates/tos-command/src/bin/tos-native-owner-command.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_native_legacy_claim_cli.rs",
        "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
        "rust/crates/tos-command/src/source_legacy_claim_store.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_command.rs",
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_revisions.rs",
        "rust/crates/tos-command/src/source_operation.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "tests/conformance/rust/command_legacy_claim_cases.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    software_names.sort();
    software_names.dedup();
    let (fixture_bytes, fixture_files) =
        charge_software(&software_names, &repository, authored_bytes, authored.len());
    let software_bytes = fixture_bytes - authored_bytes;
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must select the protected native owner command image"),
    );
    assert!(native.is_absolute());
    let worker = super::validation_cut_cases::selected_worker_path();
    let native_bytes = fs::metadata(&native).unwrap().len();
    let worker_bytes = fs::metadata(&worker).unwrap().len();
    let consumer_bytes = fs::metadata(std::env::current_exe().unwrap())
        .unwrap()
        .len();
    let python = crate::maintained_python().canonicalize().unwrap();
    let python_bytes = fs::metadata(&python).unwrap().len();
    assert!(native_bytes <= 536_870_912 && worker_bytes <= 536_870_912);
    assert!(consumer_bytes <= 536_870_912 && python_bytes <= 536_870_912);
    let selected_image_bytes = native_bytes
        .checked_add(consumer_bytes)
        .and_then(|sum| sum.checked_add(worker_bytes))
        .and_then(|sum| sum.checked_add(python_bytes))
        .unwrap();
    assert!(Instant::now() < deadline);
    let (capture, _software, components) =
        selected_software(&repository, &software_names, deadline, &mut ledger);

    let store = temporary.path().join("legacy-cut-store");
    let original_revision = super::validation_cut_cases::write_cut_store(&original, &store);
    let current_revision = super::validation_cut_cases::write_cut_store_on_base(
        &authored,
        &store,
        Some(original_revision),
    );
    assert_ne!(original_revision, current_revision);
    let cancelled = AtomicBool::new(false);
    let _original_cut =
        super::command_form_cases::open_cut(&store, original_revision, deadline, &cancelled);
    let _current_cut =
        super::command_form_cases::open_cut(&store, current_revision, deadline, &cancelled);

    let invocation_path = temporary.path().join("legacy-claim-invocation.json");
    let mut invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_context":null,
        "owner_config":revision_owner,
        "assessment_schema_worker":null,
        "native_executable":native,
        "native_executable_sha256":super::command_text_cases::alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,
        "source_revision":current_revision.0.to_prefixed(),
        "original_source_revision":original_revision.0.to_prefixed(),
        "software_capture":capture.capture,
        "software_restored_root":capture.restored,
        "software_selection":{
            "source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed(),
        },
        "software_components":components.members().map(|member| member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{
            "absolute_path":worker,
            "sha256":super::command_text_cases::alignment_image_digest(&worker).to_prefixed(),
        },
        "budgets":{
            "max_revisions":4,"max_members":256,"max_total_bytes":8388608,
            "max_member_bytes":8388608,"max_schema_receipts":128,
            "max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,
            "worker_address_space_bytes":1073741824
        }
    });

    let expected = &fixture["expected"];
    let revision_preview_request = &fixture["revision_preview_request"];
    let preview = native_call(
        &repository,
        &revision_owner,
        &invocation_path,
        &invocation,
        revision_preview_request,
        deadline,
        &mut ledger,
    );
    assert_oracle(
        &preview,
        &expected["revision_preview"],
        false,
        "prepare-revise",
    );
    let mut revision_request = fixture["revision_request"].clone();
    revision_request["expected_source"] = preview["source"].clone();
    revision_request["expected_revision"] = preview["revision"].clone();
    revision_request["expected_configuration"] = preview["owner_configuration"].clone();
    revision_request["expected_dependencies"] = preview["expected_dependencies"].clone();
    revision_request["expected_inputs"] = preview["source_bindings"].clone();
    let revised = native_call(
        &repository,
        &revision_owner,
        &invocation_path,
        &invocation,
        &revision_request,
        deadline,
        &mut ledger,
    );
    let archive_ref = assert_revision_mutation(
        &revised,
        &expected["revision_result"],
        &revision_request,
        fixture["record_id"].as_str().unwrap(),
        "claim.revise",
    );
    assert_eq!(revised["replayed"], false);
    assert_eq!(revised["grants_admission"], false);
    let revised_stream = fs::read(root.join(fixture["stream_path"].as_str().unwrap())).unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&revised_stream).to_hex(),
        fixture["revised_stream_sha256"].as_str().unwrap()
    );
    assert_ne!(revised_stream, initial_stream);
    let archive = root.join(&archive_ref);
    assert!(archive.is_dir() && !archive.is_symlink());
    assert!(archive.join("manifest.json").is_file());

    let (revised_authored, revised_bytes) = source_members(&root, deadline);
    let max_authored_bytes = authored_bytes.max(revised_bytes);
    let max_fixture_bytes = software_bytes.checked_add(max_authored_bytes).unwrap();
    let max_fixture_files = software_names.len() + authored.len().max(revised_authored.len());
    assert!(max_fixture_bytes <= 8_388_608 && max_fixture_files <= 256);
    let revised_revision =
        super::command_form_cases::successor(&revised_authored, &store, current_revision);
    invocation["source_revision"] = Value::String(revised_revision.0.to_prefixed());
    let replay = native_call(
        &repository,
        &revision_owner,
        &invocation_path,
        &invocation,
        &revision_request,
        deadline,
        &mut ledger,
    );
    let replay_archive = assert_revision_mutation(
        &replay,
        &expected["revision_replay"],
        &revision_request,
        fixture["record_id"].as_str().unwrap(),
        "cold claim replay",
    );
    assert_eq!(replay_archive, archive_ref);
    assert_eq!(replay["replayed"], true);
    let inspected = native_call(
        &repository,
        &revision_owner,
        &invocation_path,
        &invocation,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1",
            "operation":"inspect-version","source":preview["source"]}),
        deadline,
        &mut ledger,
    );
    assert_inspection(
        &inspected,
        &expected["inspect"],
        &archive_ref,
        "inspect-version",
    );
    assert_eq!(inspected["record"]["claim_id"], fixture["claim_id"]);

    invocation["owner_config"] = Value::String(form_owner.to_string_lossy().into_owned());
    let form_preview = native_call(
        &repository,
        &form_owner,
        &invocation_path,
        &invocation,
        &fixture["form_prepare_request"],
        deadline,
        &mut ledger,
    );
    assert_oracle(
        &form_preview,
        &expected["form_preview"],
        false,
        "forms prepare",
    );
    let mut form_request = fixture["form_request"].clone();
    form_request["expected_source"] = form_preview["source"].clone();
    form_request["expected_revision"] = form_preview["revision"].clone();
    form_request["expected_configuration"] = form_preview["owner_configuration"].clone();
    form_request["changes"] = serde_json::json!([form_preview["prepared_change"].clone()]);
    let form_result = native_call(
        &repository,
        &form_owner,
        &invocation_path,
        &invocation,
        &form_request,
        deadline,
        &mut ledger,
    );
    assert_form_mutation(
        &form_result,
        &expected["form_result"],
        &form_request,
        "forms apply",
    );
    assert_eq!(form_result["replayed"], false);
    assert_eq!(form_result["grants_admission"], false);
    let final_form = fs::read(root.join(fixture["claim_form_path"].as_str().unwrap())).unwrap();
    let actual_form: Value = serde_json::from_slice(&final_form).unwrap();
    assert_eq!(
        comparable_form_payload(&actual_form, &form_request, true),
        comparable_form_payload(&fixture["final_form_payload"], &form_request, false),
        "forms preserve the exact legacy Claim set apart from runtime receipt fields"
    );
    assert_eq!(
        tos_foundation::Digest256::of_bytes(
            &fs::read(root.join(fixture["stream_path"].as_str().unwrap())).unwrap()
        )
        .to_hex(),
        fixture["revised_stream_sha256"].as_str().unwrap(),
        "separate form writer preserves the revised Claim bytes"
    );

    let (final_authored, final_bytes) = source_members(&root, deadline);
    let max_authored_bytes = max_authored_bytes.max(final_bytes);
    let max_fixture_bytes = software_bytes.checked_add(max_authored_bytes).unwrap();
    let max_fixture_files = software_names.len()
        + authored
            .len()
            .max(revised_authored.len())
            .max(final_authored.len());
    assert!(max_fixture_bytes <= 8_388_608 && max_fixture_files <= 256);
    let final_revision =
        super::command_form_cases::successor(&final_authored, &store, revised_revision);
    invocation["source_revision"] = Value::String(final_revision.0.to_prefixed());
    let form_replay = native_call(
        &repository,
        &form_owner,
        &invocation_path,
        &invocation,
        &form_request,
        deadline,
        &mut ledger,
    );
    assert_form_mutation(
        &form_replay,
        &expected["form_replay"],
        &form_request,
        "cold forms replay",
    );
    assert_eq!(form_replay["replayed"], true);
    assert_eq!(form_replay["grants_admission"], false);
    assert!(Instant::now() < deadline);
    assert_eq!(ledger.git_spawns, 2);
    assert_eq!(ledger.capture_python_spawns, 2);
    assert_eq!(ledger.capture_git_spawns, 4);
    assert_eq!(ledger.fixture_python_spawns, 1);
    assert_eq!(ledger.native_cli_python_spawns, 7);
    assert_eq!(ledger.native_owner_spawns, 7);
    assert_eq!(ledger.native_schema_worker_spawns, 7);
    assert_eq!(ledger.direct_spawns, 30);

    eprintln!(
        "Legacy Claim F={max_fixture_bytes} bytes/{max_fixture_files} files (max authored state {max_authored_bytes} bytes; initial closure {fixture_bytes} bytes/{fixture_files} files), E={native_bytes}, C={consumer_bytes}, W={worker_bytes}, P={python_bytes}, E+C+W+P={selected_image_bytes}; direct_process_spawns={} (git={}, capture_python={}, capture_git={}, fixture_python={}, native_cli_python={}, native_owner_children={}, native_schema_workers={}); legacy_oracle_calls={}",
        ledger.direct_spawns,
        ledger.git_spawns,
        ledger.capture_python_spawns,
        ledger.capture_git_spawns,
        ledger.fixture_python_spawns,
        ledger.native_cli_python_spawns,
        ledger.native_owner_spawns,
        ledger.native_schema_worker_spawns,
        fixture["oracle_calls"].as_u64().unwrap()
    );
}
