//! One full private Claim lifecycle through the composed native owner route.
//! All records and grants are synthetic; no source admission follows.
use super::*;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

const MAX_SOURCE_FILES: usize = 256;
const MAX_SOURCE_BYTES: u64 = 8_388_608;
const MAX_SOURCE_PATH_BYTES: usize = 512;
const MAX_SOURCE_DEPTH: usize = 16;

fn selected_software_source(reference: &str) -> bool {
    !reference.starts_with("ToS/")
        || matches!(
            reference,
            "ToS/contracts/human-form.schema.json"
                | "ToS/contracts/human-form-set.schema.json"
                | "ToS/contracts/human-form-template.schema.json"
                | "ToS/contracts/provenance-event-v2.schema.json"
        )
}

fn canonical(value: &Value) -> Vec<u8> {
    tos_foundation::canonical_raw_bytes_v1(
        &serde_json::to_vec(value).unwrap(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
    )
    .unwrap()
}

fn request_digest(value: &Value) -> String {
    Digest256::of_bytes(
        &tos_foundation::canonical_raw_bytes_v1(
            &serde_json::to_vec(value).unwrap(),
            CanonicalProfile::SourceCommandInputV1,
            JsonLimits::default(),
        )
        .unwrap(),
    )
    .to_prefixed()
}

fn assert_same_keys(left: &Value, right: &Value) {
    let left = left.as_object().unwrap().keys().collect::<Vec<_>>();
    let right = right.as_object().unwrap().keys().collect::<Vec<_>>();
    assert_eq!(left, right);
}

fn stable_form_payload(value: &Value) -> Value {
    fn normalize(value: &mut Value) {
        match value {
            Value::Array(rows) => rows.iter_mut().for_each(normalize),
            Value::Object(fields) => {
                if let Some(Value::Array(history)) = fields.get_mut("growth_history") {
                    for receipt in history {
                        if let Some(receipt) = receipt.as_object_mut() {
                            receipt.remove("request_digest");
                            receipt.remove("recorded_at");
                            receipt.remove("previous_revision");
                        }
                    }
                }
                fields.values_mut().for_each(normalize);
            }
            _ => {}
        }
    }

    let mut normalized = value.clone();
    normalize(&mut normalized);
    normalized
}

fn charge_path(reference: &str) {
    assert!(
        reference.len() <= MAX_SOURCE_PATH_BYTES,
        "source path exceeds the cap"
    );
    assert!(
        reference.split('/').count() <= MAX_SOURCE_DEPTH,
        "source path is too deep"
    );
}

fn add_source(reference: &str, size: u64, sources: &mut BTreeMap<String, u64>) {
    charge_path(reference);
    assert!(
        size <= MAX_SOURCE_BYTES,
        "one selected source exceeds the byte cap"
    );
    if let Some(previous) = sources.insert(reference.to_owned(), size) {
        assert_eq!(previous, size, "one logical source has two physical sizes");
    }
    assert!(
        sources.len() <= MAX_SOURCE_FILES,
        "selected source count exceeds the cap"
    );
    assert!(
        sources.values().copied().sum::<u64>() <= MAX_SOURCE_BYTES,
        "selected source closure exceeds the byte cap"
    );
}

fn authored_preflight(root: &Path, deadline: Instant) -> (usize, u64) {
    let mut directories = vec![root.join("ToS")];
    let mut entries = 0usize;
    let mut directories_seen = 0usize;
    let mut census = (0usize, 0u64);
    while let Some(directory) = directories.pop() {
        assert!(Instant::now() < deadline);
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            entries = entries.checked_add(1).unwrap();
            assert!(
                entries <= 4352,
                "synthetic source walk exceeded its entry cap"
            );
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "synthetic source tree contains a symlink"
            );
            let path = entry.path();
            let reference = path.strip_prefix(root).unwrap().to_str().unwrap();
            charge_path(reference);
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
                census.0 = census.0.checked_add(1).unwrap();
                census.1 = census.1.checked_add(size).unwrap();
                assert!(census.0 <= MAX_SOURCE_FILES && census.1 <= MAX_SOURCE_BYTES);
            }
        }
    }
    census
}

fn capture_process(command: &mut Command, deadline: Instant) -> Vec<u8> {
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
    let step = deadline.min(Instant::now() + Duration::from_secs(60));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step
            || output.metadata().unwrap().len() > 1_048_576
            || errors.metadata().unwrap().len() > 1_048_576
        {
            let _ = Command::new("/usr/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            panic!("bounded selected Claim software capture refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step);
    assert!(output.metadata().unwrap().len() <= 1_048_576);
    assert!(errors.metadata().unwrap().len() <= 1_048_576);
    output.seek(SeekFrom::Start(0)).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut raw = Vec::new();
    let mut error = Vec::new();
    output.read_to_end(&mut raw).unwrap();
    errors.read_to_end(&mut error).unwrap();
    assert!(
        status.success(),
        "selected software capture: {}",
        String::from_utf8_lossy(&error)
    );
    raw
}

fn selected_capture(
    repository: &Path,
    names: &[String],
    deadline: Instant,
) -> (
    super::source_cut_cases::SoftwareCaptureFixture,
    tos_source_store::SoftwareComponentSelectionV1,
) {
    let commit = String::from_utf8(capture_process(
        Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(["rev-parse", "HEAD^{commit}"]),
        deadline,
    ))
    .unwrap()
    .trim()
    .to_owned();
    assert!(
        commit.len() == 40
            && commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let temporary = tempfile::tempdir().unwrap();
    let capture = temporary.path().join("software-capture");
    let restored = temporary.path().join("software-restored");
    let tool = temporary.path().join("corpus_archive.py");
    let program = capture_process(
        Command::new("git")
            .arg("-C")
            .arg(repository)
            .arg("show")
            .arg(format!("{commit}:scripts/corpus_archive.py")),
        deadline,
    );
    fs::write(&tool, program).unwrap();
    let wrapper = "import resource,runpy,sys;resource.setrlimit(resource.RLIMIT_CPU,(20,20));resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824));sys.argv=sys.argv[1:];runpy.run_path(sys.argv[0],run_name='__main__')";
    let mut command = Command::new(crate::maintained_python());
    command
        .args(["-c", wrapper])
        .arg(&tool)
        .arg("capture")
        .arg("--repo-root")
        .arg(repository)
        .arg("--commit")
        .arg(&commit)
        .arg("--output")
        .arg(&capture);
    for name in names
        .iter()
        .filter(|name| selected_software_source(name.as_str()))
    {
        command.arg("--include-prefix").arg(name);
    }
    capture_process(&mut command, deadline);
    capture_process(
        Command::new(crate::maintained_python())
            .args(["-c", wrapper])
            .arg(&tool)
            .arg("restore")
            .arg("--capture")
            .arg(&capture)
            .arg("--output")
            .arg(&restored),
        deadline,
    );
    assert!(fs::metadata(capture.join("capture.json")).unwrap().len() <= 1_048_576);
    let raw = fs::read(capture.join("capture.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(manifest["source_git_commit"], commit);
    let selection = tos_source_store::SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree: manifest["source_git_tree"].as_str().unwrap().to_owned(),
        capture_manifest_sha256: Digest256::of_bytes(&raw),
    };
    let fixture = super::source_cut_cases::SoftwareCaptureFixture {
        temporary,
        capture,
        restored,
        selection,
    };
    let cancelled = AtomicBool::new(false);
    let software = tos_source_store::SoftwareCaptureReader::open(
        &fixture.capture,
        &fixture.restored,
        fixture.selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let paths = names
        .iter()
        .filter(|name| selected_software_source(name.as_str()))
        .map(|name| RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    (fixture, components)
}

fn fixture(repository: &Path, root: &Path, deadline: Instant) -> Value {
    let script = r#"
import json,sys,stat,resource
resource.setrlimit(resource.RLIMIT_CPU,(90,90))
resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824))
from pathlib import Path
repo,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repo/'mechanics/growth-cycle/tests'),str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repo/'scripts'),str(repo/'tests')]
import test_source_owner_claim_commands as maintained
import source_commands as commands
class ExistingRoot:
    def __init__(self,*args,**kwargs): self.name=str(root)
    def cleanup(self): pass
original=maintained.tempfile.TemporaryDirectory
maintained.tempfile.TemporaryDirectory=ExistingRoot
try:
    case=maintained.PrivateClaimCommandTests(methodName='runTest')
    case.setUp()
finally:
    maintained.tempfile.TemporaryDirectory=original

def private_snapshot(directory):
    pending=[directory]
    entries=0
    directories=0
    members=[]
    file_bytes=0
    while pending:
        current=pending.pop()
        for path in current.iterdir():
            entries+=1
            if entries>4352: raise RuntimeError('synthetic private fixture entry cap')
            if path.is_symlink(): raise RuntimeError('synthetic private fixture symlink')
            relative=path.relative_to(directory).as_posix()
            if len(relative.encode('utf-8'))>512 or len(relative.split('/'))>16:
                raise RuntimeError('synthetic private fixture path cap')
            info=path.lstat()
            if path.is_dir():
                directories+=1
                if directories>4096: raise RuntimeError('synthetic private fixture directory cap')
                pending.append(path)
            else:
                if not path.is_file(): raise RuntimeError('synthetic private fixture non-file')
                size=info.st_size
                if info.st_nlink!=1: raise RuntimeError('synthetic private fixture alias')
                if len(members)>=256 or size>8388608:
                    raise RuntimeError('synthetic private fixture file cap')
                file_bytes+=size
                if file_bytes>8388608: raise RuntimeError('synthetic private fixture byte cap')
                members.append((path,relative,size,stat.S_IMODE(info.st_mode)))
    result={}
    for path,relative,size,mode in members:
        raw=path.read_bytes()
        if len(raw)!=size or stat.S_IMODE(path.stat().st_mode)!=mode:
            raise RuntimeError('synthetic private fixture changed during census')
        result[relative]=(raw,mode)
    return result

def remove_tree(path):
    if path.is_symlink(): raise RuntimeError('synthetic private cleanup symlink')
    if path.is_dir():
        for child in path.iterdir(): remove_tree(child)
        path.rmdir()
    elif path.exists():
        path.unlink()

def restore_private(directory, saved):
    private_snapshot(directory)  # Bound the exact tree before deleting this disposable fixture.
    for child in list(directory.iterdir()): remove_tree(child)
    for relative,(raw,mode) in saved.items():
        target=directory/relative
        target.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        target.write_bytes(raw)
        target.chmod(mode)
    for path in sorted(directory.rglob('*'),key=lambda item:len(item.parts),reverse=True):
        if path.is_dir(): path.chmod(0o700)
    directory.chmod(0o700)

initial_private=private_snapshot(case.local.private)
calls={'oracle':0}
def oracle(request):
    calls['oracle']+=1
    print(f"private Claim oracle enter {calls['oracle']} {request['operation']}",file=sys.stderr,flush=True)
    result=commands.run_legacy_oracle_command(case.owner,
        {'schema_version':'tos_local_source_command_v1',**request})
    print(f"private Claim oracle exit {calls['oracle']} {request['operation']}",file=sys.stderr,flush=True)
    return result

before=oracle({'operation':'describe'})
preview=oracle({'operation':'prepare-create','claims':[case.claim],'forms':case.forms})
creation={'operation':'claims.create','command_id':'synthetic-create','claims':[case.claim],
    'forms':case.forms,'expected_configuration':preview['owner_configuration'],
    'expected_source':None,'expected_revision':None,
    'expected_dependencies':preview['expected_dependencies'],'expected_inputs':preview['source_bindings']}
created=oracle(creation)
creation_replay=oracle(creation)
created_forms=json.loads((case.path.parent/commands.claim_forms_path(case.path,case.claim['claim_id']).name).read_bytes())

form_preview=oracle({'operation':'prepare','claim_id':case.claim['claim_id'],
    'form_id':case.form_id,'field_id':'claim.statement'})
form_request={'operation':'apply','claim_id':case.claim['claim_id'],'command_id':'synthetic-form',
    'expected_source':form_preview['source'],'expected_revision':form_preview['revision'],
    'expected_configuration':form_preview['owner_configuration'],
    'expected_dependencies':form_preview['expected_dependencies'],
    'expected_inputs':form_preview['source_bindings'],'changes':[form_preview['prepared_change']]}
formed=oracle(form_request)
form_replay=oracle(form_request)
formed_forms=json.loads((case.path.parent/commands.claim_forms_path(case.path,case.claim['claim_id']).name).read_bytes())

proposal={'claim_id':case.claim['claim_id'],
    'fields':{'qualifiers':{'statement':'Исправленная синтетическая возможность; не установленный разбор.'}},
    'forms':[{key:value for key,value in case.forms[0].items() if key!='claim_id'}],
    'reason':'Synthetic description correction without a new relation identity.'}
revision_preview=oracle({'operation':'prepare-revise',**proposal})
revision={'operation':'claim.revise','command_id':'synthetic-revise',**proposal,
    'expected_configuration':revision_preview['owner_configuration'],
    'expected_source':revision_preview['source'],'expected_revision':revision_preview['revision'],
    'expected_dependencies':revision_preview['expected_dependencies'],
    'expected_inputs':revision_preview['source_bindings']}
revised=oracle(revision)
revision_replay=oracle(revision)
creation_replay_after=oracle(creation)
inspected=oracle({'operation':'inspect-version','claim_id':case.claim['claim_id'],
    'source':created['sources'][0]})
after=oracle({'operation':'describe'})
current_record=json.loads(case.path.read_bytes())
form_file=commands.claim_forms_path(case.path,case.claim['claim_id']).name
current_forms=json.loads((case.path.parent/form_file).read_bytes())
oracle_calls=calls['oracle']

# All oracle mutations were confined to this private fixture root. Restore its
# exact initial source files before invoking the native owner path.
restore_private(case.local.private,initial_private)
print(json.dumps({'public':str(case.local.public),'private':str(case.local.private),
    'owner':str(case.owner),'context':str(case.local.config_path),'source_ref':case.source_ref,
    'form_file':form_file,'claim':case.claim,'forms':case.forms,'config':case.config,
    'before':before,'preview':preview,'creation':creation,'created':created,
    'creation_replay':creation_replay,'created_forms':created_forms,
    'form_preview':form_preview,'form_request':form_request,
    'formed':formed,'form_replay':form_replay,'formed_forms':formed_forms,'proposal':proposal,
    'revision_preview':revision_preview,'revision':revision,'revised':revised,
    'revision_replay':revision_replay,'creation_replay_after':creation_replay_after,
    'inspected':inspected,'after':after,'current_record':current_record,
    'current_forms':current_forms,'oracle_calls':oracle_calls},ensure_ascii=False,allow_nan=False))
"#;
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut child = Command::new(crate::maintained_python())
        .args(["-c", script])
        .arg(repository)
        .arg(root)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .process_group(0)
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(errors.try_clone().unwrap()))
        .spawn()
        .unwrap();
    let step = deadline.min(Instant::now() + Duration::from_secs(90));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        let deadline_reached = Instant::now() >= step;
        let output_bytes = output.metadata().unwrap().len();
        let error_bytes = errors.metadata().unwrap().len();
        if deadline_reached || output_bytes > 8_388_608 || error_bytes > 1_048_576 {
            let _ = Command::new("/usr/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let status = child.wait().unwrap();
            errors.seek(SeekFrom::Start(0)).unwrap();
            let mut error = Vec::new();
            errors.take(1_048_576).read_to_end(&mut error).unwrap();
            panic!(
                "bounded maintained private Claim fixture refused: deadline_reached={deadline_reached} output_bytes={output_bytes} stderr_bytes={error_bytes} post_kill_status={status}; {}",
                String::from_utf8_lossy(&error)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step);
    assert!(output.metadata().unwrap().len() <= 8_388_608);
    assert!(errors.metadata().unwrap().len() <= 1_048_576);
    output.seek(SeekFrom::Start(0)).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut raw = Vec::new();
    let mut error = Vec::new();
    output.read_to_end(&mut raw).unwrap();
    errors.read_to_end(&mut error).unwrap();
    assert!(
        status.success(),
        "private Claim fixture ({status}): {}",
        String::from_utf8_lossy(&error)
    );
    serde_json::from_slice(&raw).unwrap()
}

fn private_snapshot(root: &Path, deadline: Instant) -> BTreeMap<String, (Vec<u8>, u32)> {
    let mut pending = vec![root.to_path_buf()];
    let mut entries = 0usize;
    let mut directories = 0usize;
    let mut census = Vec::new();
    let mut file_bytes = 0u64;
    while let Some(directory) = pending.pop() {
        assert!(Instant::now() < deadline);
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            entries = entries.checked_add(1).unwrap();
            assert!(entries <= 4352, "synthetic private fixture entry cap");
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "synthetic private tree contains a symlink"
            );
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
            charge_path(relative);
            if kind.is_dir() {
                directories = directories.checked_add(1).unwrap();
                assert!(
                    directories <= 4096,
                    "synthetic private fixture directory cap"
                );
                pending.push(path);
            } else {
                assert!(kind.is_file());
                let metadata = entry.metadata().unwrap();
                assert_eq!(
                    metadata.nlink(),
                    1,
                    "synthetic private tree contains a hard-link alias"
                );
                let size = metadata.len();
                assert!(census.len() < MAX_SOURCE_FILES && size <= MAX_SOURCE_BYTES);
                file_bytes = file_bytes.checked_add(size).unwrap();
                assert!(
                    file_bytes <= MAX_SOURCE_BYTES,
                    "synthetic private fixture byte cap"
                );
                let mode = metadata.permissions().mode() & 0o777;
                census.push((entry.path(), relative.to_owned(), size, mode));
            }
        }
    }
    let mut files = BTreeMap::new();
    for (path, relative, size, mode) in census {
        assert!(Instant::now() < deadline);
        let raw = fs::read(&path).unwrap();
        assert_eq!(
            raw.len() as u64,
            size,
            "synthetic private file changed after census"
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            mode
        );
        files.insert(relative, (raw, mode));
    }
    files
}

fn same_file_set(
    before: &BTreeMap<String, (Vec<u8>, u32)>,
    after: &BTreeMap<String, (Vec<u8>, u32)>,
) {
    assert_eq!(
        before, after,
        "a replay changed the selected private package or archive"
    );
}

#[test]
fn native_private_claim_cli_preserves_create_forms_revision_and_cold_replay() {
    let deadline = Instant::now() + Duration::from_secs(720);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let mut names = vec![
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_profile_commands.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py".to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py".to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_text_unit_commands.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py".to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_claim_commands.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_claim_commands.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py".to_owned(),
        "scripts/source_owner_record_profiles.py".to_owned(),
        "scripts/source_record_profiles.py".to_owned(),
        "scripts/source_owner_context.py".to_owned(),
        "scripts/native_text_binding.py".to_owned(),
        "scripts/source_witness_human_forms.py".to_owned(),
        "scripts/source_owner_claim_profiles.py".to_owned(),
        "scripts/corpus_archive.py".to_owned(),
        "mechanics/growth-cycle/tests/test_source_owner_claim_commands.py".to_owned(),
        "mechanics/growth-cycle/tests/test_occurrence_growth.py".to_owned(),
        "tests/test_source_owner_claim_profiles.py".to_owned(),
        "tests/test_source_owner_record_profiles.py".to_owned(),
        "tests/test_native_text_binding.py".to_owned(),
        "rust/crates/tos-command/src/source_native_cli.rs".to_owned(),
        "rust/crates/tos-command/src/source_command.rs".to_owned(),
        "rust/crates/tos-command/src/source_claims.rs".to_owned(),
        "rust/crates/tos-command/src/source_forms.rs".to_owned(),
        "rust/crates/tos-command/src/source_revisions.rs".to_owned(),
        "rust/crates/tos-command/src/source_sign_native.rs".to_owned(),
        "rust/crates/tos-command/src/source_creation_store.rs".to_owned(),
        "rust/crates/tos-command/src/source_legacy_claim_store.rs".to_owned(),
        "rust/crates/tos-command/src/source_native_private_cli.rs".to_owned(),
        "rust/crates/tos-command/src/source_private_profile.rs".to_owned(),
        "rust/crates/tos-command/src/source_private_claim.rs".to_owned(),
        "rust/crates/tos-command/src/source_private_owner_store.rs".to_owned(),
        "rust/crates/tos-command/src/source_private_serialization.rs".to_owned(),
        "rust/crates/tos-command/src/source_text_owner.rs".to_owned(),
        "rust/crates/tos-command/src/source_text_private_store.rs".to_owned(),
        "rust/crates/tos-command/src/source_serialization.rs".to_owned(),
        "ToS/doctrine/semantic-interchange/entity-types.v1.json".to_owned(),
        "ToS/doctrine/semantic-interchange/relation-types.v1.json".to_owned(),
    ];
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        assert!(!entry.file_type().unwrap().is_symlink());
        if entry.file_type().unwrap().is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .ends_with(".schema.json")
        {
            names.push(format!(
                "ToS/contracts/{}",
                entry.file_name().to_str().unwrap()
            ));
        }
    }
    names.sort();
    names.dedup();
    assert!(names.len() <= MAX_SOURCE_FILES);
    for name in &names {
        charge_path(name);
    }

    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must select the protected native private Claim image"),
    );
    assert!(native.is_absolute());
    let worker = super::validation_cut_cases::selected_worker_path();
    let native_bytes = fs::metadata(&native).unwrap().len();
    let worker_bytes = fs::metadata(&worker).unwrap().len();
    let consumer_bytes = fs::metadata(std::env::current_exe().unwrap())
        .unwrap()
        .len();
    let python_bytes = fs::metadata(crate::maintained_python().canonicalize().unwrap())
        .unwrap()
        .len();
    assert!(python_bytes <= 536_870_912);
    assert!(native_bytes <= 536_870_912 && worker_bytes <= 536_870_912);
    assert!(consumer_bytes <= 536_870_912);
    assert!(Instant::now() < deadline);

    // Charge the actual selected source before capture, restoration, fixture
    // construction or image-body reads. The later union also charges generated
    // authored fixture members before native publication.
    let mut selected_source_sizes = BTreeMap::new();
    for name in &names {
        assert!(Instant::now() < deadline);
        let path = repository.join(name);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        add_source(name, metadata.len(), &mut selected_source_sizes);
    }

    let (capture, components) = selected_capture(&repository, &names, deadline);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let selected = fixture(&repository, temporary.path(), deadline);
    let public = PathBuf::from(selected["public"].as_str().unwrap());
    let private = PathBuf::from(selected["private"].as_str().unwrap());
    let owner = PathBuf::from(selected["owner"].as_str().unwrap());
    let context = PathBuf::from(selected["context"].as_str().unwrap());
    assert!(owner.is_absolute() && context.is_absolute());

    let (authored_count, authored_bytes) = authored_preflight(&public, deadline);
    let mut authored = super::command_text_cases::authored_text_files(&public);
    authored.remove("ToS/source-witnesses/.historical-create.writer.lock");
    assert_eq!(authored.len(), authored_count);
    assert_eq!(
        authored.values().map(|raw| raw.len() as u64).sum::<u64>(),
        authored_bytes
    );
    let original_files = authored
        .iter()
        .filter(|(name, _)| name.as_str() == "ToS/contracts/owner-local-source-context.schema.json")
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(original_files.len(), 1);
    let store = temporary.path().join("selected-store");
    let original = super::validation_cut_cases::write_cut_store(&original_files, &store);
    let current =
        super::validation_cut_cases::write_cut_store_on_base(&authored, &store, Some(original));
    assert_ne!(original, current);

    let mut source_sizes = BTreeMap::new();
    for member in components.members() {
        add_source(member.path.as_str(), member.size_bytes, &mut source_sizes);
    }
    for (name, raw) in &authored {
        add_source(name, raw.len() as u64, &mut source_sizes);
    }
    let fixture_bytes = source_sizes.values().copied().sum::<u64>();
    assert!(source_sizes.len() <= MAX_SOURCE_FILES && fixture_bytes <= MAX_SOURCE_BYTES);

    let invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_context":context,
        "owner_config":owner,
        "assessment_schema_worker":null,
        "native_executable":native,
        "native_executable_sha256":super::command_text_cases::alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,
        "source_revision":current.0.to_prefixed(),
        "original_source_revision":original.0.to_prefixed(),
        "software_capture":capture.capture,
        "software_restored_root":capture.restored,
        "software_selection":{
            "source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()
        },
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":worker,"sha256":super::command_text_cases::alignment_image_digest(&worker).to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
            "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}
    });
    let invocation_path = temporary.path().join("claim-invocation.json");
    fs::write(&invocation_path, canonical(&invocation)).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();

    let mut native_calls = 0usize;
    let private_seed = private_snapshot(&private, deadline);
    let call = |request: &Value, native_calls: &mut usize| {
        assert!(Instant::now() < deadline);
        *native_calls = (*native_calls).checked_add(1).unwrap();
        let outer = super::command_text_cases::alignment_native_cli(
            &repository,
            &owner,
            &invocation_path,
            request,
            deadline,
        );
        assert_eq!(outer["schema_version"], "tos_local_native_source_result_v1");
        assert_eq!(outer["grants_admission"], false);
        outer["result"].clone()
    };
    let request = |operation: &str, fields: Value| {
        let mut value = serde_json::json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":operation
        });
        for (key, field) in fields.as_object().unwrap() {
            value[key.as_str()] = field.clone();
        }
        value
    };

    let before = call(
        &request("describe", serde_json::json!({})),
        &mut native_calls,
    );
    assert_eq!(before, selected["before"]);
    assert_eq!(before["target_exists"], false);
    let preview_request = request(
        "prepare-create",
        serde_json::json!({"claims":[selected["claim"]],"forms":selected["forms"]}),
    );
    let preview = call(&preview_request, &mut native_calls);
    let oracle_preview = &selected["preview"];
    assert_eq!(
        preview["owner_configuration"],
        oracle_preview["owner_configuration"]
    );
    assert_eq!(
        preview["prepared_sources"],
        oracle_preview["prepared_sources"]
    );
    assert_eq!(preview["prepared_files"], oracle_preview["prepared_files"]);
    assert_eq!(
        preview["prepared_materializations"],
        oracle_preview["prepared_materializations"]
    );
    assert!(preview["expected_source"].is_null());
    assert!(preview["expected_revision"].is_null());
    let mut creation = selected["creation"].clone();
    creation["expected_configuration"] = preview["owner_configuration"].clone();
    creation["expected_source"] = Value::Null;
    creation["expected_revision"] = Value::Null;
    creation["expected_dependencies"] = preview["expected_dependencies"].clone();
    creation["expected_inputs"] = preview["source_bindings"].clone();
    let creation_request = request("claims.create", creation.clone());
    let created = call(&creation_request, &mut native_calls);
    let oracle_created = &selected["created"];
    assert_eq!(
        created["owner_configuration"],
        oracle_created["owner_configuration"]
    );
    assert_eq!(created["sources"], preview["prepared_sources"]);
    assert_eq!(created["sources"], oracle_created["sources"]);
    assert_eq!(
        created["materializations"],
        oracle_created["materializations"]
    );
    assert_eq!(created["publication_authorized"], false);
    assert_eq!(created["grants_admission"], false);
    assert_eq!(
        created["receipt"]["request_digest"],
        request_digest(&creation_request)
    );
    assert_eq!(created["receipt"]["command_id"], creation["command_id"]);
    assert_eq!(
        created["receipt"]["owner_configuration"],
        creation["expected_configuration"]
    );
    assert_eq!(
        created["receipt"]["dependencies"],
        creation["expected_dependencies"]
    );
    assert_eq!(
        created["receipt"]["source_bindings"],
        creation["expected_inputs"]
    );
    assert_ne!(created["revision"], oracle_created["revision"]);

    let home = private
        .join(selected["source_ref"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(home.metadata().unwrap().permissions().mode() & 0o777, 0o700);
    for entry in fs::read_dir(&home).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        assert_eq!(
            entry.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let source_file = Path::new(selected["source_ref"].as_str().unwrap())
        .file_name()
        .unwrap();
    let stored_claim: Value =
        serde_json::from_slice(&fs::read(home.join(source_file)).unwrap()).unwrap();
    assert_eq!(stored_claim, selected["claim"]);
    let created_forms: Value = serde_json::from_slice(
        &fs::read(home.join(selected["form_file"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(created_forms, selected["created_forms"]);
    let provenance: Value =
        serde_json::from_slice(&fs::read(home.join("source-create-provenance.jsonl")).unwrap())
            .unwrap();
    assert_eq!(provenance["activity"]["event_type"], "annotation");
    assert_eq!(
        provenance["rights_and_visibility"]["content_visibility"],
        "local_only"
    );
    assert!(provenance["method"]["software_components"]
        .as_array()
        .unwrap()
        .iter()
        .any(|component| component["artifact_ref"]
            == "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_claim_commands.py"));

    let after_create = private_snapshot(&private, deadline);
    let create_replay = call(&creation_request, &mut native_calls);
    assert_eq!(create_replay["replayed"], true);
    assert_eq!(create_replay["receipt"], created["receipt"]);
    same_file_set(&after_create, &private_snapshot(&private, deadline));

    let form_preview = call(
        &request(
            "prepare",
            serde_json::json!({"claim_id":selected["claim"]["claim_id"],
                "form_id":selected["config"]["allowed_form_ids"][0],"field_id":"claim.statement"}),
        ),
        &mut native_calls,
    );
    let oracle_form_preview = &selected["form_preview"];
    assert_eq!(
        form_preview["owner_configuration"],
        oracle_form_preview["owner_configuration"]
    );
    assert_eq!(form_preview["source"], oracle_form_preview["source"]);
    assert_eq!(
        form_preview["source_fields"],
        oracle_form_preview["source_fields"]
    );
    assert_eq!(
        form_preview["materializations"],
        oracle_form_preview["materializations"]
    );
    assert_eq!(
        form_preview["prepared_change"],
        oracle_form_preview["prepared_change"]
    );
    let form_request = request(
        "apply",
        serde_json::json!({
            "claim_id":selected["claim"]["claim_id"],
            "command_id":selected["form_request"]["command_id"],
            "expected_configuration":form_preview["owner_configuration"],
            "expected_source":form_preview["source"],
            "expected_revision":form_preview["revision"],
            "changes":[form_preview["prepared_change"]]
        }),
    );
    let formed = call(&form_request, &mut native_calls);
    let oracle_formed = &selected["formed"];
    assert_eq!(formed["publication_authorized"], false);
    assert_eq!(formed["grants_admission"], false);
    assert_eq!(
        formed["receipt"]["request_digest"],
        request_digest(&form_request)
    );
    assert_eq!(formed["receipt"]["command_id"], form_request["command_id"]);
    assert_eq!(formed["receipt"]["source"], form_preview["source"]);
    assert_eq!(
        formed["receipt"]["previous_revision"],
        form_request["expected_revision"]
    );
    assert_eq!(formed["source"], form_preview["source"]);
    assert_eq!(formed["source"], oracle_formed["source"]);
    assert_eq!(
        formed["materializations"],
        oracle_formed["materializations"]
    );
    assert_ne!(formed["revision"], oracle_formed["revision"]);
    let formed_forms: Value = serde_json::from_slice(
        &fs::read(home.join(selected["form_file"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(
        stable_form_payload(&formed_forms),
        stable_form_payload(&selected["formed_forms"])
    );
    let after_form = private_snapshot(&private, deadline);
    let form_replay = call(&form_request, &mut native_calls);
    assert_eq!(form_replay["replayed"], true);
    assert_eq!(form_replay["receipt"], formed["receipt"]);
    same_file_set(&after_form, &private_snapshot(&private, deadline));

    let revision_preview = call(
        &request("prepare-revise", selected["proposal"].clone()),
        &mut native_calls,
    );
    let oracle_revision_preview = &selected["revision_preview"];
    assert_eq!(
        revision_preview["owner_configuration"],
        oracle_revision_preview["owner_configuration"]
    );
    assert_eq!(
        revision_preview["source"],
        oracle_revision_preview["source"]
    );
    assert_eq!(
        revision_preview["sources"],
        oracle_revision_preview["sources"]
    );
    assert_eq!(
        revision_preview["source_fields"],
        oracle_revision_preview["source_fields"]
    );
    assert_eq!(
        revision_preview["materializations"],
        oracle_revision_preview["materializations"]
    );
    assert_eq!(
        revision_preview["prepared_source"],
        oracle_revision_preview["prepared_source"]
    );
    assert_eq!(
        revision_preview["prepared_forms"],
        oracle_revision_preview["prepared_forms"]
    );
    assert_eq!(
        revision_preview["prepared_materializations"],
        oracle_revision_preview["prepared_materializations"]
    );
    assert_eq!(revision_preview["source"], created["sources"][0]);
    assert_eq!(revision_preview["revision"], formed["revision"]);
    let mut revision = selected["revision"].clone();
    revision["expected_configuration"] = revision_preview["owner_configuration"].clone();
    revision["expected_source"] = revision_preview["source"].clone();
    revision["expected_revision"] = revision_preview["revision"].clone();
    revision["expected_dependencies"] = revision_preview["expected_dependencies"].clone();
    revision["expected_inputs"] = revision_preview["source_bindings"].clone();
    let revision_request = request("claim.revise", revision.clone());
    let revised = call(&revision_request, &mut native_calls);
    let oracle_revised = &selected["revised"];
    assert_eq!(
        revised["owner_configuration"],
        oracle_revised["owner_configuration"]
    );
    assert_eq!(revised["source"]["id"], oracle_revised["source"]["id"]);
    assert_eq!(
        revised["source"]["version"],
        oracle_revised["source"]["version"]
    );
    assert_eq!(revised["source"]["version"], 2);
    assert_eq!(revised["publication_authorized"], false);
    assert_eq!(revised["grants_admission"], false);
    assert_eq!(
        revised["receipt"]["request_digest"],
        request_digest(&revision_request)
    );
    assert_eq!(revised["receipt"]["command_id"], revision["command_id"]);
    assert_eq!(
        revised["receipt"]["dependencies"],
        revision["expected_dependencies"]
    );
    assert_eq!(
        revised["receipt"]["source_bindings"],
        revision["expected_inputs"]
    );
    assert_eq!(
        revised["receipt"]["forms"],
        revision_preview["prepared_forms"]
    );
    assert_eq!(
        revised["receipt"]["previous_source"],
        revision_preview["source"]
    );
    assert_eq!(
        revised["receipt"]["previous_revision"],
        revision_preview["revision"]
    );
    assert_eq!(revised["source"], revision_preview["prepared_source"]);
    assert_ne!(revised["revision"], oracle_revised["revision"]);
    assert!(
        revised["receipt"]["archive_path"]
            .as_str()
            .unwrap()
            .contains(".record-revisions/")
    );
    let current_record: Value =
        serde_json::from_slice(&fs::read(home.join(source_file)).unwrap()).unwrap();
    assert_eq!(current_record, selected["current_record"]);
    let current_forms: Value = serde_json::from_slice(
        &fs::read(home.join(selected["form_file"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(
        stable_form_payload(&current_forms),
        stable_form_payload(&selected["current_forms"])
    );

    let inspected = call(
        &request(
            "inspect-version",
            serde_json::json!({"claim_id":selected["claim"]["claim_id"],
                "source":created["sources"][0]}),
        ),
        &mut native_calls,
    );
    assert_eq!(inspected["record"], selected["claim"]);
    assert_eq!(inspected["record"], selected["inspected"]["record"]);
    assert_eq!(inspected["inspected_source"], created["sources"][0]);
    assert_same_keys(&inspected["files"], &selected["inspected"]["files"]);

    let after_revision = private_snapshot(&private, deadline);
    let revision_replay = call(&revision_request, &mut native_calls);
    assert_eq!(revision_replay["replayed"], true);
    assert_eq!(revision_replay["receipt"], revised["receipt"]);
    same_file_set(&after_revision, &private_snapshot(&private, deadline));
    let after_cold_form_replay = private_snapshot(&private, deadline);
    let form_replay_after_revision = call(&form_request, &mut native_calls);
    assert_eq!(form_replay_after_revision["replayed"], true);
    assert_eq!(form_replay_after_revision["receipt"], formed["receipt"]);
    same_file_set(
        &after_cold_form_replay,
        &private_snapshot(&private, deadline),
    );
    let after_cold_create_replay = private_snapshot(&private, deadline);
    let create_replay_after_revision = call(&creation_request, &mut native_calls);
    assert_eq!(create_replay_after_revision["replayed"], true);
    assert_eq!(create_replay_after_revision["receipt"], created["receipt"]);
    same_file_set(
        &after_cold_create_replay,
        &private_snapshot(&private, deadline),
    );
    let described = call(
        &request("describe", serde_json::json!({})),
        &mut native_calls,
    );
    assert_eq!(described["source"]["id"], revised["source"]["id"]);
    assert_eq!(described["source"]["version"], 2);
    assert_eq!(described["grants_admission"], false);
    let private_final = private_snapshot(&private, deadline);

    eprintln!(
        "private Claim F={fixture_bytes} source_files={} public_files={authored_count} public_bytes={authored_bytes} private_seed_files={} private_seed_bytes={} private_final_files={} private_final_bytes={} E={native_bytes} C={consumer_bytes} W={worker_bytes} P={python_bytes} native_cli_calls={native_calls} native_children={native_calls} oracle_calls={} fixture_processes=1 capture_direct_git=2 capture_python=2 capture_inner_git=4 schema_workers<={} publication=false admission=false",
        source_sizes.len(),
        private_seed.len(),
        private_seed
            .values()
            .map(|(raw, _)| raw.len())
            .sum::<usize>(),
        private_final.len(),
        private_final
            .values()
            .map(|(raw, _)| raw.len())
            .sum::<usize>(),
        selected["oracle_calls"],
        native_calls,
    );
}
