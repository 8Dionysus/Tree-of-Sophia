//! One installed-consumer case. Python authors synthetic source inputs and the
//! explicit partition oracle only; every command traverses the actual selected
//! native executable. No builder, fallback executable, or inferred grant exists.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

struct PublicTextFailureFixture(Option<tempfile::TempDir>);
impl PublicTextFailureFixture {
    fn new() -> Self {
        Self(Some(tempfile::tempdir().unwrap()))
    }
    fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
}
impl Drop for PublicTextFailureFixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(directory) = self.0.take() {
                eprintln!(
                    "PublicText failed fixture retained at {}",
                    directory.keep().display()
                );
            }
        }
    }
}
fn physical_bytes(root: &Path, cap: u64) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let mut directories = vec![root.to_path_buf()];
    let mut bytes = 0u64;
    let mut entries = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let meta = fs::symlink_metadata(entry.path()).unwrap();
            assert!(!meta.file_type().is_symlink());
            entries += 1;
            assert!(entries <= 16384);
            bytes = bytes
                .checked_add(meta.blocks().checked_mul(512).unwrap())
                .unwrap();
            assert!(
                bytes <= cap,
                "PublicText allocated fixture exceeds its physical reservation"
            );
            if meta.is_dir() {
                directories.push(entry.path());
            }
        }
    }
    bytes
}
#[test]
fn public_text_native_cli_preserves_whole_public_closure_and_cold_replay() {
    use super::command_text_cases::{
        alignment_image_digest, alignment_native_cli as base_alignment_native_cli,
        authored_text_files, native_owner_cli_observation,
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
        assert!(bytes <= 2_097_152);
        f = f.checked_add(bytes).unwrap();
    }
    assert!(f <= 33_554_432 && Instant::now() < deadline);
    eprintln!(
        "public Text CLI preflight F={f} E={e} C={c} W={w} native_processes=9 fixture_processes=1 whole_seconds=240 outer_proposed_seconds=260"
    );
    let mut temporary = PublicTextFailureFixture::new();
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
    let python =
        std::env::var_os("TOS_MAINTAINED_PYTHON").expect("explicit maintained fixture interpreter");
    let mut child = Command::new(python)
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
    let step = deadline.min(Instant::now() + Duration::from_secs(60));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step
            || stdout.metadata().unwrap().len() > 2_097_152
            || stderr.metadata().unwrap().len() > 262_144
        {
            let _ = child.kill();
            let _ = child.wait();
            panic!("bounded public Text fixture deadline or output refused");
        }
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
    physical_bytes(temporary.path(), 180 * 1024 * 1024);
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
    let capture_bytes = physical_bytes(capture.temporary.path(), 72 * 1024 * 1024);
    physical_bytes(temporary.path(), 180 * 1024 * 1024);
    eprintln!("public Text capture allocated_bytes={capture_bytes}");
    let alignment_native_cli =
        |repository: &Path, owner: &Path, invocation: &Path, request: &Value, deadline: Instant| {
            physical_bytes(temporary.path(), 180 * 1024 * 1024);
            let result =
                base_alignment_native_cli(repository, owner, invocation, request, deadline);
            physical_bytes(temporary.path(), 180 * 1024 * 1024);
            result
        };
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

    let retain_pair = std::env::var("TOS_PUBLIC_TEXT_RETAIN_SOURCE_READ_PAIR")
        .map(|value| {
            assert_eq!(value, "1");
            true
        })
        .unwrap_or(false);
    let input = root.join(oracle["input_ref"].as_str().unwrap());
    let original_input = fs::read(&input).unwrap();
    assert!(original_input.len() <= 131_072);
    let input_mode = fs::metadata(&input).unwrap().permissions().mode();
    let stamp = |path: &Path| {
        let meta = fs::symlink_metadata(path).unwrap();
        assert!(!meta.file_type().is_symlink());
        (
            meta.dev(),
            meta.ino(),
            meta.uid(),
            meta.permissions().mode(),
        )
    };
    let pair_controls = retain_pair.then(|| {
        assert_eq!(root.canonicalize().unwrap(), root);
        assert_eq!(store.canonicalize().unwrap(), store);
        assert_eq!(invocation["source_revision"], current.0.to_prefixed());
        assert_eq!(invocation["corpus_store"], store.to_str().unwrap());
        let batch = temporary.path().parent().unwrap().parent().unwrap();
        let paths = [
            (owner.clone(), 1_048_576),
            (invocation_path.clone(), 1_048_576),
            (root.join("LICENSE"), 1_048_576),
            (
                store
                    .join("revisions")
                    .join(original.0.to_hex())
                    .join("snapshot.json"),
                1_048_576,
            ),
            (
                store
                    .join("revisions")
                    .join(current.0.to_hex())
                    .join("snapshot.json"),
                1_048_576,
            ),
            (batch.join("guard.json"), 4_194_304),
            (batch.join("bound-canonical-lease.json"), 1_048_576),
        ];
        let controls = paths
            .into_iter()
            .map(|(path, cap)| {
                let meta = fs::symlink_metadata(&path).unwrap();
                assert!(meta.is_file() && meta.len() <= cap);
                let raw = fs::read(&path).unwrap();
                assert!(raw.len() as u64 <= cap);
                (path.clone(), raw, stamp(&path), cap)
            })
            .collect::<Vec<_>>();
        (stamp(&root), stamp(&store), controls)
    });
    let mut changed = original_input.clone();
    changed.extend_from_slice(b"changed");
    fs::write(&input, changed).unwrap();
    let (status, out, err) =
        native_owner_cli_observation(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(status.code(), Some(2));
    // The maintained Python transport wraps nonzero native exits in ValueError.
    // Require the exact Rust cause as well, so unrelated refusals cannot pass.
    assert_eq!(
        serde_json::from_slice::<Value>(&out).unwrap()["error"],
        "ValueError"
    );
    assert!(
        String::from_utf8_lossy(&err).contains("Conflict(\"public Text input declaration\")"),
        "public Text changed input refusal: {}",
        String::from_utf8_lossy(&err)
    );
    for (name, raw) in &package {
        assert_eq!(
            fs::read(home.join(name)).unwrap().as_slice(),
            raw.as_slice()
        );
    }
    if let Some((root_stamp, store_stamp, controls)) = &pair_controls {
        let grant: Value = serde_json::from_slice(&controls[0].1).unwrap();
        assert_eq!(
            Digest256::of_bytes(&original_input).to_hex(),
            grant["source"]["sha256"]
        );
        assert_eq!(
            original_input.len() as u64,
            grant["source"]["byte_size"].as_u64().unwrap()
        );
        fs::write(&input, &original_input).unwrap();
        assert_eq!(fs::read(&input).unwrap(), original_input);
        assert_eq!(
            fs::metadata(&input).unwrap().permissions().mode(),
            input_mode
        );
        assert_eq!(stamp(&root), *root_stamp);
        assert_eq!(stamp(&store), *store_stamp);
        let restored = authored_text_files(&root);
        assert_eq!(
            restored, current_files,
            "restored exact positive authored source vector"
        );
        drop(restored);
        for (path, raw, identity, cap) in controls {
            assert!(fs::symlink_metadata(path).unwrap().len() <= *cap);
            assert_eq!(stamp(path), *identity);
            assert_eq!(fs::read(path).unwrap(), *raw);
        }
        let invocation_now: Value =
            serde_json::from_slice(&fs::read(&invocation_path).unwrap()).unwrap();
        assert_eq!(invocation_now, invocation);
        assert_eq!(invocation_now["source_revision"], current.0.to_prefixed());
        assert_eq!(invocation_now["corpus_store"], store.to_str().unwrap());
        let vector = current_files.iter().map(|(path, raw)| {
            serde_json::json!({"ref":path,"sha256":Digest256::of_bytes(raw).to_hex(),"bytes":raw.len()})
        }).collect::<Vec<_>>();
        let receipt_refs = controls.iter().map(|(path, raw, _, _)| {
            serde_json::json!({"path":path,"sha256":Digest256::of_bytes(raw).to_hex(),"bytes":raw.len()})
        }).collect::<Vec<_>>();
        let binding_ref = home
            .join("native-bindings.json")
            .strip_prefix(&root)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let handoff = serde_json::json!({
            "schema_version":"tos_public_text_source_read_handoff_v1",
            "source_root":root,"corpus_store":store,"original_source_revision":original.0.to_prefixed(),"source_revision":current.0.to_prefixed(),
            "source_vector":vector,"binding_ref":binding_ref,"binding_sha256":Digest256::of_bytes(&package["native-bindings.json"]).to_hex(),
            "bindings":serde_json::from_slice::<Value>(&package["native-bindings.json"]).unwrap(),
            "package_ref":source_path,"package_sha256":Digest256::of_bytes(&package["source-text-unit.v1.json"]).to_hex(),
            "protected_owner_config":owner,"declared_publication_authority":grant["publication_authority"],"license_ref":"LICENSE",
            "restored_input":{"ref":oracle["input_ref"],"sha256":Digest256::of_bytes(&original_input).to_hex(),"bytes":original_input.len(),"mode":input_mode & 0o777},
            "pinned_controls":receipt_refs,"native_invocation_role":"archival execution evidence; software capture not retained, no future native replay claim",
            "software_capture_retained":false,"producer_source":capture.selection.source_git_commit,
            "positive_source_vector_restored":true,"whole_pass_acceptance":"requires external canonical terminal and postguards",
            "rights_or_local_condition_approval_inferred":false,"reader_database_authored":false
        });
        let raw = canonical_json(&handoff);
        assert!(raw.len() <= 1_048_576 && Instant::now() < deadline);
        let path = temporary
            .path()
            .join("public-text-source-read-handoff.json");
        fs::write(&path, raw).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(Instant::now() < deadline);
    drop(software);
    drop(components);
    let allocated = physical_bytes(temporary.path(), 180 * 1024 * 1024);
    eprintln!(
        "public Text whole allocated_bytes={}",
        allocated + capture_bytes
    );
    assert!(Instant::now() < deadline);
    if retain_pair {
        let retained = temporary.0.take().unwrap().keep();
        eprintln!(
            "PublicText positive source-read handoff {}",
            retained
                .join("public-text-source-read-handoff.json")
                .display()
        );
    } else {
        temporary.0.take().unwrap().close().unwrap();
    }
}
