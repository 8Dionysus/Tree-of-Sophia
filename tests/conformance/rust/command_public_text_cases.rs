//! One installed-consumer case. Python authors synthetic source inputs and the
//! explicit partition oracle only; every command traverses the actual selected
//! native executable. No builder, fallback executable, or inferred grant exists.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[test]
fn public_text_native_cli_preserves_whole_public_closure_and_cold_replay() {
    use super::command_text_cases::{
        alignment_image_digest, alignment_native_cli, authored_text_files,
        native_owner_cli_observation,
    };
    use std::io::{Read, Seek, SeekFrom};
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancellation = AtomicBool::new(false);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must select installed public Text native image"),
    );
    let worker = super::validation_cut_cases::selected_worker_path();
    assert!(native.is_absolute() && worker.is_absolute());
    let e = fs::metadata(&native).unwrap().len();
    let w = fs::metadata(&worker).unwrap().len();
    let c = fs::metadata(std::env::current_exe().unwrap())
        .unwrap()
        .len();
    for bytes in [e, w, c] {
        assert!(bytes <= 536_870_912);
    }
    let selected = [
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_native_public_text_cli.rs",
        "rust/crates/tos-command/src/source_public_text_entry.rs",
        "rust/crates/tos-command/src/source_public_text_owner.rs",
        "rust/crates/tos-command/src/source_public_text_proposal.rs",
        "rust/crates/tos-command/src/source_text_private_store.rs",
        "rust/crates/tos-command/src/source_text_identity.rs",
        "rust/crates/tos-command/src/source_text_unit_proposal.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "mechanics/growth-cycle/tests/test_source_public_native_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_public_native_commands.py",
    ];
    let mut f = 0u64;
    for name in selected {
        let bytes = fs::metadata(repository.join(name)).unwrap().len();
        assert!(bytes <= 8_388_608);
        f = f.checked_add(bytes).unwrap();
    }
    assert!(f <= 33_554_432 && Instant::now() < deadline);
    eprintln!(
        "public Text CLI preflight F={f} E={e} C={c} W={w} native_processes=9 fixture_processes=1 whole_seconds=240 outer_proposed_seconds=260"
    );
    let temporary = tempfile::tempdir().unwrap();
    let isolated = tos_command::source_creation_store::IsolatedCreationRoot::create(
        temporary.path(),
        deadline,
        &cancellation,
    )
    .unwrap();
    let root = isolated.path().join("source");
    let recovery = isolated.path().join("recovery");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&recovery).unwrap();
    fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700)).unwrap();
    let factory = r#"
import json,sys,shutil,time
from pathlib import Path
repo,root,recovery=map(Path,sys.argv[1:])
sys.path[:0]=[str(repo/'mechanics/growth-cycle/tests'),str(repo/'tests'),str(repo/'scripts'),str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
from test_source_public_native_commands import PublicNativeCommandTests
import source_public_native_commands as public
import source_commands as source
test=PublicNativeCommandTests(methodName='runTest')
try:
    test.setUp()
    shutil.copytree(test.root,root,dirs_exist_ok=True)
    test.root=root; test.recovery=recovery; test.fixture.root=root
    test.owner=root.parent/'protected-public-grant.json'
    test.config['source_root']=str(root); test.config['recovery_root']=str(recovery)
    test.config['limits']['max_seconds']=20
    test.write_owner()
    config,digest,path=public.configuration(test.config,owner_config=test.owner)
    subject,deps,files,bindings=public._prepare(config,digest,deadline=time.monotonic()+20)
    print(json.dumps({'owner':str(test.owner),'source_path':test.config['source_path'],'input_ref':test.input_ref,'content':test.content,'configuration':digest,'packet':json.loads(files[public.BASENAME]),'layer':json.loads(files['source-text-layer.v1.json']),'bindings':bindings},ensure_ascii=False))
finally:
    test.doCleanups()
"#;
    let mut stdout = tempfile::tempfile().unwrap();
    let mut stderr = tempfile::tempfile().unwrap();
    let mut child = Command::new("/usr/bin/python3")
        .args(["-c", factory])
        .arg(&repository)
        .arg(&root)
        .arg(&recovery)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::from(stdout.try_clone().unwrap()))
        .stderr(Stdio::from(stderr.try_clone().unwrap()))
        .spawn()
        .unwrap();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("whole public Text fixture deadline");
        }
        assert!(
            stdout.metadata().unwrap().len() <= 2_097_152
                && stderr.metadata().unwrap().len() <= 262_144
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        stdout.metadata().unwrap().len() <= 2_097_152
            && stderr.metadata().unwrap().len() <= 262_144
    );
    stdout.seek(SeekFrom::Start(0)).unwrap();
    stderr.seek(SeekFrom::Start(0)).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    stdout.read_to_end(&mut out).unwrap();
    stderr.read_to_end(&mut err).unwrap();
    assert!(
        status.success(),
        "public Text fixture {}",
        String::from_utf8_lossy(&err)
    );
    let oracle: Value = serde_json::from_slice(&out).unwrap();
    let owner = PathBuf::from(oracle["owner"].as_str().unwrap());
    let source_path = oracle["source_path"].as_str().unwrap();
    let home = root.join(source_path).parent().unwrap().to_path_buf();
    let mut files = authored_text_files(&root);
    for name in selected {
        if !name.starts_with("ToS/") {
            files.insert(name.to_owned(), fs::read(repository.join(name)).unwrap());
        }
    }
    let full_f = files
        .values()
        .try_fold(0u64, |n, raw| n.checked_add(raw.len() as u64))
        .unwrap();
    assert!(full_f <= 33_554_432 && Instant::now() < deadline);
    eprintln!("public Text CLI complete fixture F={full_f} E={e} C={c} W={w}");
    let (capture, software, components) =
        super::command_record_cases::captured_components(&files, deadline, &cancellation);
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let store = temporary.path().join("selected-store");
    let original = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation_path = isolated.path().join("native-public-text-invocation.json");
    let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1","owner_context":null,"assessment_schema_worker":null,"owner_config":owner,
        "native_executable":native,"native_executable_sha256":alignment_image_digest(&native).to_prefixed(),"corpus_store":store,"source_revision":original.0.to_prefixed(),"original_source_revision":original.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,"software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),"schema_worker":{"absolute_path":worker,"sha256":alignment_image_digest(&worker).to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let write_profile = |value: &Value| {
        fs::write(&invocation_path, canonical_json(value)).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    };
    write_profile(&invocation);
    let describe = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
        deadline,
    );
    assert_eq!(describe["result"]["status"], "ready");
    assert_eq!(describe["result"]["configuration"], oracle["configuration"]);
    let prepared = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create"}),
        deadline,
    );
    let request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"native-text.create","command_id":"synthetic:whole-public-text-native-cli","expected_configuration":prepared["result"]["configuration"],"expected_dependencies":prepared["result"]["dependencies"],"expected_source":null,"expected_revision":null});
    let created = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(
        created["schema_version"],
        "tos_local_native_source_result_v1"
    );
    assert_eq!(created["result"]["status"], "created");
    let package = fs::read_dir(&home)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(package.len(), 12);
    assert_eq!(
        package["content.txt"],
        oracle["content"].as_str().unwrap().as_bytes()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&package["source-text-unit.v1.json"]).unwrap(),
        oracle["packet"]
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&package["source-text-layer.v1.json"]).unwrap(),
        oracle["layer"]
    );
    assert_eq!(created["result"]["native_bindings"], oracle["bindings"]);
    let cold = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(cold["result"]["status"], "replayed");
    assert_eq!(
        cold["result"]["receipt_digest"],
        created["result"]["receipt_digest"]
    );
    let mut current_files = authored_text_files(&root);
    current_files.remove("ToS/source-witnesses/.historical-create.writer.lock");
    assert!(current_files.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let current = super::validation_cut_cases::write_cut_store_on_base(
        &current_files,
        &store,
        Some(original),
    );
    invocation["source_revision"] = serde_json::json!(current.0.to_prefixed());
    write_profile(&invocation);
    let successor = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(successor["result"]["status"], "replayed");
    assert_eq!(
        successor["result"]["receipt_digest"],
        created["result"]["receipt_digest"]
    );
    let inspected = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-recovery","command_id":request["command_id"]}),
        deadline,
    );
    assert_eq!(inspected["result"]["status"], "committed");
    // Only this isolated synthetic installed package is removed. Its durable
    // private control and original request/capture remain untouched.
    fs::remove_dir_all(&home).unwrap();
    let pending = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-recovery","command_id":request["command_id"]}),
        deadline,
    );
    assert_eq!(pending["result"]["status"], "retained_plan");
    let resumed = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(resumed["result"]["status"], "created");
    assert_eq!(
        resumed["result"]["receipt_digest"],
        created["result"]["receipt_digest"]
    );
    let recovered = fs::read_dir(&home)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        recovered, package,
        "retained pending resume preserves all original bytes"
    );

    let input = root.join(oracle["input_ref"].as_str().unwrap());
    let mut changed = fs::read(&input).unwrap();
    changed.extend_from_slice(b"changed");
    fs::write(&input, changed).unwrap();
    let (status, out, _) =
        native_owner_cli_observation(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&out).unwrap()["error"],
        "JournalConflict"
    );
    for (name, raw) in package {
        assert_eq!(fs::read(home.join(name)).unwrap(), raw);
    }
    assert!(Instant::now() < deadline);
    drop(software);
    drop(components);
    temporary.close().unwrap();
}
