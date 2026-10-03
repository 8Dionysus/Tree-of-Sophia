//! One real private Profile lifecycle from the maintained synthetic fixture.
//! Synthetic assertions grant neither source admission nor publication.
use super::*;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn rule_software_component(name: &str) -> bool {
    !name.starts_with("ToS/")
        || matches!(
            name,
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

fn charge_fixture_member(reference: &str, size: Option<u64>, census: &mut (usize, u64)) {
    assert!(reference.len() <= 512 && reference.split('/').count() <= 16);
    if let Some(size) = size {
        assert!(size <= 8_388_608);
        census.0 = census.0.checked_add(1).unwrap();
        census.1 = census.1.checked_add(size).unwrap();
        assert!(census.0 <= 256 && census.1 <= 8_388_608);
    }
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
            assert!(entries <= 4352);
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            let path = entry.path();
            let reference = path.strip_prefix(root).unwrap().to_str().unwrap();
            if kind.is_dir() {
                if tos_source_store::has_authored_source_descendants_v1(reference) {
                    charge_fixture_member(reference, None, &mut census);
                    directories_seen = directories_seen.checked_add(1).unwrap();
                    assert!(directories_seen <= 4096);
                    directories.push(path);
                }
            } else if tos_source_store::is_authored_source_path_v1(reference)
                && reference != "ToS/source-witnesses/.historical-create.writer.lock"
            {
                assert!(kind.is_file());
                let size = entry.metadata().unwrap().len();
                charge_fixture_member(reference, Some(size), &mut census);
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
    // The two Python owner tools may spawn Git. Their selected child process
    // group is the only termination target if this bounded call fails.
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
            panic!("bounded selected software capture refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step);
    assert!(
        output.metadata().unwrap().len() <= 1_048_576
            && errors.metadata().unwrap().len() <= 1_048_576
    );
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
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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
    for name in names.iter().filter(|name| rule_software_component(name)) {
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
            max_selected_object_bytes: 2_097_152,
            json: JsonLimits::default(),
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let paths = names
        .iter()
        .filter(|name| rule_software_component(name))
        .map(|name| RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    (fixture, components)
}

fn fixture(repository: &Path, root: &Path, deadline: Instant) -> Value {
    let script = r#"
import json,sys,stat,resource
resource.setrlimit(resource.RLIMIT_CPU,(20,20))
resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824))
from pathlib import Path
repo,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repo/'mechanics/growth-cycle/tests'),str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repo/'scripts'),str(repo/'tests')]
import test_source_owner_profile_commands as maintained
import source_commands as commands
class ExistingRoot:
    def __init__(self,*args,**kwargs): self.name=str(root)
    def cleanup(self): pass
original=maintained.tempfile.TemporaryDirectory
maintained.tempfile.TemporaryDirectory=ExistingRoot
try:
    case=maintained.OwnerLocalProfileCommandTests(methodName='runTest')
    case.setUp()
finally:
    maintained.tempfile.TemporaryDirectory=original
request={'schema_version':'tos_local_source_command_v1','operation':'prepare-create','record':case.record,'forms':case.forms}
prepared=commands.run_legacy_oracle_command(case.owner,request)
creation={**request,'operation':'source.create','command_id':'synthetic-native-private-profile-create',
    'expected_configuration':prepared['owner_configuration'],'expected_source':None,'expected_revision':None,
    'expected_dependencies':prepared['expected_dependencies']}
created=commands.run_legacy_oracle_command(case.owner,creation)
saved=case.files()
# Restore only the exact just-created disposable fixture package. Oracle and
# native commands then receive the same selected owner, roots and source bytes.
for name,raw in saved.items():
    p=case.path.parent/name
    if not p.is_file() or p.is_symlink() or p.read_bytes()!=raw: raise RuntimeError('oracle fixture changed')
    p.unlink()
case.path.parent.rmdir()
for p in case.public.joinpath('ToS').rglob('*'):
    if p.is_symlink(): raise RuntimeError('synthetic fixture symlink')
    if p.is_file() and p.relative_to(case.public).as_posix()!='ToS/source-witnesses/.historical-create.writer.lock':
        p.chmod(0o755 if stat.S_IMODE(p.stat().st_mode)&0o111 else 0o644)
print(json.dumps({'public':str(case.public),'private':str(case.store),'owner':str(case.owner),
    'context':str(case.context_path),'source_ref':case.source_ref,'record':case.record,'forms':case.forms,
    'config':case.config,'preview':prepared,'created_source':created['source'],
    'oracle_files':{name:raw.hex() for name,raw in saved.items()}},ensure_ascii=False,allow_nan=False))
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
            || output.metadata().unwrap().len() > 8_388_608
            || errors.metadata().unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded maintained private Profile fixture refused");
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
        "private Profile fixture: {}",
        String::from_utf8_lossy(&error)
    );
    serde_json::from_slice(&raw).unwrap()
}

#[test]
fn native_private_profile_cli_preserves_owner_lifecycle_and_cold_archives() {
    use super::command_text_cases::{
        alignment_image_digest, alignment_native_cli, authored_text_files,
    };
    let deadline = Instant::now() + Duration::from_secs(240);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let mut names = vec![
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_profile_commands.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py".to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py".to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_text_unit_commands.py"
            .to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py".to_owned(),
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py"
            .to_owned(),
        "scripts/source_owner_record_profiles.py".to_owned(),
        "scripts/source_record_profiles.py".to_owned(),
        "scripts/source_owner_context.py".to_owned(),
        "scripts/native_text_binding.py".to_owned(),
        "scripts/source_witness_human_forms.py".to_owned(),
        "scripts/corpus_archive.py".to_owned(),
        "mechanics/growth-cycle/tests/test_source_owner_profile_commands.py".to_owned(),
        "mechanics/growth-cycle/tests/test_occurrence_growth.py".to_owned(),
        "tests/test_native_text_binding.py".to_owned(),
        "rust/crates/tos-command/src/source_native_cli.rs".to_owned(),
        "rust/crates/tos-command/src/source_command.rs".to_owned(),
        "rust/crates/tos-command/src/source_forms.rs".to_owned(),
        "rust/crates/tos-command/src/source_revisions.rs".to_owned(),
        "rust/crates/tos-command/src/source_sign_native.rs".to_owned(),
        "rust/crates/tos-command/src/source_creation_store.rs".to_owned(),
        "rust/crates/tos-command/src/source_legacy_claim_store.rs".to_owned(),
        "rust/crates/tos-command/src/source_native_private_cli.rs".to_owned(),
        "rust/crates/tos-command/src/source_private_profile.rs".to_owned(),
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
    assert!(names.len() <= 256);
    let mut census = (0usize, 0u64);
    for name in &names {
        let n = fs::metadata(repository.join(name)).unwrap().len();
        charge_fixture_member(name, Some(n), &mut census);
    }
    let fixture_bytes = census.1;
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must select protected native Profile image"),
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
    assert!(
        native_bytes <= 536_870_912 && worker_bytes <= 536_870_912 && consumer_bytes <= 536_870_912
    );
    assert!(Instant::now() < deadline);
    eprintln!(
        "private Profile F={fixture_bytes} E={native_bytes} C={consumer_bytes} W={worker_bytes} P={python_bytes} native_processes=10 launcher_processes=10 fixture_processes=1 explicit_oracle_calls=2 capture_direct_git=2 capture_python=2 capture_inner_git=4 workers<=10"
    );
    let (capture, components) = selected_capture(&repository, &names, deadline);
    let temporary = tempfile::tempdir().unwrap();
    let selected = fixture(&repository, temporary.path(), deadline);
    let public = PathBuf::from(selected["public"].as_str().unwrap());
    let private = PathBuf::from(selected["private"].as_str().unwrap());
    let owner = PathBuf::from(selected["owner"].as_str().unwrap());
    let (authored_count, authored_bytes) = authored_preflight(&public, deadline);
    let mut authored = authored_text_files(&public);
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
    let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
        "owner_context":selected["context"],"owner_config":owner,
        "assessment_schema_worker":null,"native_executable":native,"native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,"source_revision":current.0.to_prefixed(),"original_source_revision":original.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":worker,"sha256":alignment_image_digest(&worker).to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
            "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("profile-invocation.json");
    fs::write(&invocation_path, canonical(&invocation)).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let call = |request: &Value| {
        eprintln!(
            "Profile native operation={}",
            request["operation"].as_str().unwrap()
        );
        let outer = alignment_native_cli(&repository, &owner, &invocation_path, request, deadline);
        assert_eq!(outer["schema_version"], "tos_local_native_source_result_v1");
        assert_eq!(outer["grants_admission"], false);
        outer["result"].clone()
    };
    let preview_request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create",
        "record":selected["record"],"forms":selected["forms"]});
    let preview = call(&preview_request);
    assert_eq!(preview, selected["preview"]);
    let creation = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"source.create",
        "command_id":"synthetic-native-private-profile-create","record":selected["record"],"forms":selected["forms"],
        "expected_configuration":preview["owner_configuration"],"expected_source":null,"expected_revision":null,
        "expected_dependencies":preview["expected_dependencies"]});
    let created = call(&creation);
    assert_eq!(created["source"], selected["created_source"]);
    let home = private
        .join(selected["source_ref"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_path_buf();
    for (name, raw) in selected["oracle_files"].as_object().unwrap() {
        if name
            == Path::new(selected["source_ref"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
            || name.ends_with("human-forms.json")
        {
            assert_eq!(
                fs::read(home.join(name)).unwrap(),
                decode_hex(raw.as_str().unwrap())
            );
        }
    }
    let frozen = fs::read(home.join("source-create-receipt.json")).unwrap();
    let replay = call(&creation);
    assert_eq!(replay["replayed"], true);
    assert_eq!(
        fs::read(home.join("source-create-receipt.json")).unwrap(),
        frozen
    );
    let proposal = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-revise",
        "fields":{"notes":"Corrected synthetic account; the native use is unchanged."},"forms":selected["forms"],
        "reason":"Synthetic descriptive correction, not identity replacement."});
    let prepared = call(&proposal);
    let mut revision = proposal.clone();
    revision["operation"] = serde_json::json!("record.revise");
    revision["command_id"] = serde_json::json!("synthetic-native-private-profile-revise");
    for (target, source) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
        ("expected_dependencies", "expected_dependencies"),
    ] {
        revision[target] = prepared[source].clone();
    }
    let revised = call(&revision);
    assert_ne!(revised["source"], created["source"]);
    let archived = call(
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-version","source":created["source"]}),
    );
    assert_eq!(archived["record"], selected["record"]);
    let prepared_form = call(
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare",
        "form_id":selected["config"]["allowed_form_ids"][1],"field_id":"metadata.source-note"}),
    );
    let apply = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"apply",
        "command_id":"synthetic-native-private-profile-form","expected_configuration":prepared_form["owner_configuration"],
        "expected_source":prepared_form["source"],"expected_revision":prepared_form["revision"],
        "expected_dependencies":prepared_form["expected_dependencies"],"changes":[prepared_form["prepared_change"]]});
    let applied = call(&apply);
    assert_eq!(applied["publication_authorized"], false);
    let replay = call(&apply);
    assert_eq!(replay["replayed"], true);
    let described = call(
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
    );
    assert_eq!(described["source"], revised["source"]);
    assert_eq!(described["grants_admission"], false);
    assert_eq!(
        fs::read(home.join("source-create-receipt.json")).unwrap(),
        frozen
    );
}
