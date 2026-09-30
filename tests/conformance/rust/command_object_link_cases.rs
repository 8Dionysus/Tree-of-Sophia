//! Actual ObjectLink CLI, retained cold replay and deterministic owner recovery.
use super::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_creation_store::IsolatedCreationRoot;
const FACTORY: &str = r#"
import json,sys,tempfile
from pathlib import Path
repository,root,unused=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_link_commands as fixture
class SelectedRoot:
    def __init__(self,*a,**kw): self.name=str(root)
    def cleanup(self): pass
original=tempfile.TemporaryDirectory;tempfile.TemporaryDirectory=SelectedRoot
try:
    # Literal owner-source census precedes the fixture's first source copy.
    refs=[p for p in (repository/'ToS/contracts').glob('*.schema.json')]
    refs += [repository/'ToS/doctrine/semantic-interchange'/name for name in ('entity-types.v1.json','relation-types.v1.json')]
    refs += [repository/ref for ref in fixture.links.IMPLEMENTATIONS]
    refs += [repository/'rust/crates/tos-command/src/source_serialization.rs']
    assert len(refs)<=256 and sum(p.stat().st_size for p in refs)<=8388608
    assert all(not p.is_symlink() and p.is_file() and len(p.relative_to(repository).parts)<=16 and len(p.relative_to(repository).as_posix())<=512 for p in refs)
    case=fixture.NativeObjectLinkTests(methodName='runTest');case.setUp()
finally: tempfile.TemporaryDirectory=original
case.owner.chmod(0o600)
implementations=sorted(fixture.links.IMPLEMENTATIONS)
for ref in implementations:
    p=root/ref;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((repository/ref).read_bytes());p.chmod(0o644)
print(json.dumps({'config':case.config,'proposal':case.proposal(),'implementations':implementations,'owner':str(case.owner)}))
"#;
const UPDATE: &str = r#"
import json,sys
from pathlib import Path
repository,root,unused=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
import test_source_link_commands as fixture
case=fixture.NativeObjectLinkTests(methodName='runTest');case.root=root;case.owner=root/'link-owner.json'
case.config=json.loads(case.owner.read_bytes());case.subject_ref=case.config['subject_source_path'];case.subject_path=root/case.subject_ref
case.subject=json.loads(case.subject_path.read_bytes())
action=json.load(sys.stdin)
if action['action']=='oracle':
    request=case.request();result=fixture.commands.run_legacy_oracle_command(case.owner,request)
    print(json.dumps(result))
elif action['action']=='rebuild':
    case.rebuild();print('{}')
elif action['action']=='crash':
    request,pending=case.crash(edge=3)
    case.config.update(principal_id='model:synthetic-recoverer',allowed_operations=[fixture.links.RECOVERY]);case.save_config();case.owner.chmod(0o600)
    print(json.dumps({'request':request,'recovery':case.recovery(pending,action['decision'])}))
"#;
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
        assert!(serde_json::to_vec(input).unwrap().len() <= 1_048_576);
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
            panic!("bounded maintained ObjectLink fixture refused");
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
        "ObjectLink fixture {}",
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
        "ObjectLink actual operation={} expected_success={} owner={} invocation={}",
        request["operation"].as_str().unwrap_or("<absent>"),
        success,
        owner.display(),
        invocation.display()
    );
    let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
        repository, owner, invocation, request, deadline,
    );
    assert_eq!(
        status.success(),
        success,
        "ObjectLink native CLI output={} stderr={}",
        String::from_utf8_lossy(&raw),
        String::from_utf8_lossy(&errors)
    );
    let envelope: Value = serde_json::from_slice(&raw).unwrap();
    if success {
        envelope["result"].clone()
    } else {
        envelope
    }
}
fn freeze_invocation(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn link_request(preview: &Value, proposal: &Value) -> Value {
    let mut request = proposal.clone();
    request["operation"] = json!("object.link.create");
    request["command_id"] = json!("test:object-link");
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    request["expected_publication"] = preview["expected_publication"].clone();
    request
}
fn read_link_body(root: &Path, config: &Value) -> (Value, Value, Value, Value, Value) {
    let link = root.join(config["link_source_path"].as_str().unwrap());
    let claim = root.join(config["claim_source_path"].as_str().unwrap());
    let read = |path: &Path| {
        assert!(fs::metadata(path).unwrap().len() <= 1_048_576);
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap()
    };
    let link_forms = link.with_file_name("link.human-forms.json");
    // Claim form filename is dictated by the maintained compound receipt.
    let receipt = read(&claim.with_file_name("object-link-creation-receipt.json"));
    let forms_ref = receipt["forms"]["claim"]["source_ref"].as_str();
    let claim_forms = if let Some(reference) = forms_ref {
        root.join(reference)
    } else {
        let id = config["claim_id"].as_str().unwrap();
        claim.with_file_name(format!(
            "source-claims.{}.human-forms.json",
            tos_foundation::Digest256::of_bytes(id.as_bytes()).to_hex()
        ))
    };
    let provenance = read(&claim.with_file_name("source-create-provenance.jsonl"));
    (
        read(&link),
        read(&claim),
        read(&link_forms),
        read(&claim_forms),
        provenance,
    )
}
// Consumer-only physical scratch envelope; library budgets stay unchanged.
// Sixteen full fixture allocations cover source/oracle roots, archive capture,
// compressed archive, restored software, cut object copies and transaction
// staging/retained packages. The fixed allowances cover manifests, child I/O,
// owner-tool copy and growth outputs, plus explicit filesystem headroom.
fn physical_fixture_budget(files: &BTreeMap<String, Vec<u8>>) -> u64 {
    const BLOCK: u64 = 4096;
    assert!(files.len() <= 256);
    let mut logical = 0u64;
    let mut allocated = 0u64;
    for (path, raw) in files {
        assert!(path.len() <= 512 && path.split('/').count() <= 16);
        assert!(
            !path.starts_with('/')
                && !path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
        );
        logical = logical.checked_add(raw.len() as u64).unwrap();
        // Round every file separately; allow a block per path directory and
        // two further blocks for file/directory inode and metadata allocation.
        allocated = allocated
            .checked_add(
                (raw.len() as u64).div_ceil(BLOCK) * BLOCK
                    + (path.split('/').count() as u64 + 2) * BLOCK,
            )
            .unwrap();
    }
    assert!(logical <= 8_388_608);
    let total = allocated
        .checked_mul(16)
        .unwrap()
        .checked_add(128 * 1_048_576)
        .unwrap()
        .checked_add(256 * 1_048_576)
        .unwrap();
    assert!(total <= 1_073_741_824);
    total
}

// Keep only the already bounded active fixture on failure; no copy is made.
// Successful iterations retain TempDir's ordinary cleanup.
struct FailureFixture {
    directory: Option<tempfile::TempDir>,
    physically_bounded: bool,
}
impl FailureFixture {
    fn path(&self) -> &Path {
        self.directory.as_ref().unwrap().path()
    }
}
impl Drop for FailureFixture {
    fn drop(&mut self) {
        if std::thread::panicking() && self.physically_bounded {
            let path = self.directory.take().unwrap().keep();
            eprintln!(
                "ObjectLink bounded failed fixture retained={} (same allocation; no copy)",
                path.display()
            );
        }
    }
}

// Current snapshots own the complete current membership; retained ancestry
// does not supply missing current contracts. Account both full snapshots.
fn bounded_successor(
    base_files: &BTreeMap<String, Vec<u8>>,
    current_files: &BTreeMap<String, Vec<u8>>,
    store: &Path,
    base: tos_foundation::SourceRevision,
    stage: &str,
) -> tos_foundation::SourceRevision {
    assert!(
        base_files
            .keys()
            .all(|path| current_files.contains_key(path)),
        "ObjectLink {stage} unexpectedly retires a base path"
    );
    let unchanged = base_files == current_files;
    let members = if unchanged {
        base_files.len()
    } else {
        base_files.len().checked_add(current_files.len()).unwrap()
    };
    let base_bytes = base_files
        .values()
        .try_fold(0u64, |total, raw| total.checked_add(raw.len() as u64))
        .unwrap();
    let bytes = if unchanged {
        base_bytes
    } else {
        current_files
            .values()
            .try_fold(base_bytes, |total, raw| total.checked_add(raw.len() as u64))
            .unwrap()
    };
    assert!(
        members <= 256 && bytes <= 8_388_608,
        "ObjectLink {stage} exact lineage exceeds existing cut budget: members={members} bytes={bytes}"
    );
    eprintln!(
        "ObjectLink {stage} cut base_members={} current_members={} unchanged={unchanged} aggregate_members={members} aggregate_bytes={bytes}",
        base_files.len(),
        current_files.len()
    );
    if unchanged {
        base
    } else {
        super::validation_cut_cases::write_cut_store_on_base(current_files, store, Some(base))
    }
}

#[test]
fn native_object_link_cli_creates_cold_replays_and_recovers_original_package() {
    use super::command_text_cases::{alignment_image_digest, authored_text_files};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(360);
    let cancelled = AtomicBool::new(false);
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must supply immutable native owner CLI"),
    );
    let worker = super::validation_cut_cases::selected_worker_path();
    assert!(native.is_absolute());
    assert!(
        fs::metadata(std::env::current_exe().unwrap())
            .unwrap()
            .len()
            <= 536_870_912
    );
    for image in [&native, &worker] {
        assert!(fs::metadata(image).unwrap().len() <= 536_870_912);
    }
    eprintln!(
        "ObjectLink whole case: 3 source roots + oracle; 7 native CLI children (each Python-to-native exec), 8 Python fixture children plus 2 retained crash writers, 3 captured_components calls (6 direct Git +6 capture/restore Python children plus archive-tool internal Git); 360s whole deadline, 60s child, 1MiB stdout/stderr; each cut<=256 members/8MiB, native/worker<=512MiB; no native kill race"
    );
    for decision in [None, Some("resume"), Some("rollback")] {
        let mut temporary = FailureFixture {
            directory: Some(tempfile::tempdir().unwrap()),
            physically_bounded: false,
        };
        eprintln!(
            "ObjectLink actual iteration={:?} fixture={}",
            decision,
            temporary.path().display()
        );
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let fixture = python(
            &repository,
            isolated.path(),
            isolated.path(),
            FACTORY,
            None,
            deadline,
        );
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        let config = &fixture["config"];
        let subject = isolated
            .path()
            .join(config["subject_source_path"].as_str().unwrap());
        let before = fs::read(&subject).unwrap();
        let sentinel = subject.parent().unwrap().join("payload/opaque.bin");
        let private_before = fs::read(&sentinel).unwrap();
        let mut files = authored_text_files(isolated.path());
        for name in fixture["implementations"].as_array().unwrap() {
            let name = name.as_str().unwrap();
            files.insert(name.into(), fs::read(repository.join(name)).unwrap());
        }
        files.insert(
            "rust/crates/tos-command/src/source_serialization.rs".into(),
            fs::read(repository.join("rust/crates/tos-command/src/source_serialization.rs"))
                .unwrap(),
        );
        let scratch_bound = physical_fixture_budget(&files);
        temporary.physically_bounded = true;
        // The publication guard rechecks authenticated software at this root.
        // FACTORY copies the Python implementation cohort; include the native
        // serialization source already selected into the same capture below.
        let serialization_ref = "rust/crates/tos-command/src/source_serialization.rs";
        let serialization_path = isolated.path().join(serialization_ref);
        fs::create_dir_all(serialization_path.parent().unwrap()).unwrap();
        fs::write(&serialization_path, &files[serialization_ref]).unwrap();
        fs::set_permissions(&serialization_path, fs::Permissions::from_mode(0o644)).unwrap();
        for (reference, raw) in files
            .iter()
            .filter(|(reference, _)| !reference.starts_with("ToS/"))
        {
            assert_eq!(
                fs::read(isolated.path().join(reference)).unwrap(),
                *raw,
                "ObjectLink selected software physical copy: {reference}"
            );
        }
        eprintln!(
            "ObjectLink physical scratch <={} B including 256MiB headroom; F<=8MiB entries<=256 path<=512B depth<=16; images supplied outside scratch",
            scratch_bound
        );
        let (capture, _software, components) =
            super::command_record_cases::captured_components(&files, deadline, &cancelled);
        let authored: BTreeMap<String, Vec<u8>> = files
            .iter()
            .filter(|(name, _)| name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect();
        let store = temporary.path().join("selected-store");
        let base = super::validation_cut_cases::write_cut_store(&authored, &store);
        let invocation_path = temporary.path().join("object-link-invocation.json");
        let mut invocation = json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,"owner_context":null,"assessment_schema_worker":null,
            "native_executable":native,"native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
            "corpus_store":store,"source_revision":base.0.to_prefixed(),"original_source_revision":base.0.to_prefixed(),
            "software_capture":capture.capture,"software_restored_root":capture.restored,
            "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":worker,"sha256":alignment_image_digest(&worker).to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":256,"max_total_bytes":8388608,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        freeze_invocation(&invocation_path, &invocation);
        if let Some(decision) = decision {
            let pending = python(
                &repository,
                isolated.path(),
                isolated.path(),
                UPDATE,
                Some(&json!({"action":"crash","decision":decision})),
                deadline,
            );
            let pending_files = authored_text_files(isolated.path());
            physical_fixture_budget(&pending_files);
            let pending_revision =
                bounded_successor(&authored, &pending_files, &store, base, "pending-recovery");
            invocation["source_revision"] = json!(pending_revision.0.to_prefixed());
            freeze_invocation(&invocation_path, &invocation);
            let result = cli(
                &repository,
                &owner,
                &invocation_path,
                &pending["recovery"],
                true,
                deadline,
            );
            assert_eq!(
                result["recovery"]["outcome"],
                json!(if decision == "resume" {
                    "committed"
                } else {
                    "rolled-back"
                })
            );
            assert_eq!(
                result["recovery"]["recovery_authorization"]["principal_id"],
                json!("model:synthetic-recoverer")
            );
            if decision == "resume" {
                assert_eq!(result["receipt"]["principal_id"], json!("model:synthetic"));
                read_link_body(isolated.path(), config);
            } else {
                assert!(
                    !isolated
                        .path()
                        .join(config["link_source_path"].as_str().unwrap())
                        .exists()
                );
                assert!(
                    !isolated
                        .path()
                        .join(config["claim_source_path"].as_str().unwrap())
                        .exists()
                );
            }
        } else {
            let description = cli(
                &repository,
                &owner,
                &invocation_path,
                &json!({"schema_version":fixture["proposal"]["schema_version"],"operation":"describe"}),
                true,
                deadline,
            );
            assert_eq!(description["grants_admission"], json!(false));
            let preview = cli(
                &repository,
                &owner,
                &invocation_path,
                &fixture["proposal"],
                true,
                deadline,
            );
            let request = link_request(&preview, &fixture["proposal"]);
            let mut bad = request.clone();
            bad["link"]["uri"] = json!("https://example.invalid/undelegated");
            cli(&repository, &owner, &invocation_path, &bad, false, deadline);
            let result = cli(
                &repository,
                &owner,
                &invocation_path,
                &request,
                true,
                deadline,
            );
            assert_eq!(result["replayed"], json!(false));
            assert_eq!(result["grants_admission"], json!(false));
            let native_body = read_link_body(isolated.path(), config);
            assert_eq!(
                native_body.1["qualifiers"]["availability_is_rights_conclusion"],
                json!(false)
            );
            assert_eq!(
                native_body.1["qualifiers"]["unknown_context"],
                json!({"flag":false,"missing":null})
            );
            for group in result["materializations"].as_object().unwrap().values() {
                assert!(
                    group
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|view| view["state"] == "ready")
                );
            }
            let oracle =
                IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
            python(
                &repository,
                oracle.path(),
                oracle.path(),
                FACTORY,
                None,
                deadline,
            );
            let oracle_result = python(
                &repository,
                oracle.path(),
                oracle.path(),
                UPDATE,
                Some(&json!({"action":"oracle"})),
                deadline,
            );
            let oracle_body = read_link_body(oracle.path(), config);
            assert_eq!(native_body.0, oracle_body.0);
            assert_eq!(native_body.1, oracle_body.1);
            assert_eq!(native_body.2, oracle_body.2);
            assert_eq!(native_body.3, oracle_body.3);
            assert_eq!(
                native_body.4.pointer("/method/procedure/name"),
                Some(&json!("native-object-link-serialization"))
            );
            assert_eq!(
                oracle_body.4.pointer("/method/procedure/name"),
                Some(&json!("native-object-link-metadata-serialization"))
            );
            for pointer in [
                "/rights_and_visibility/publication_authorized",
                "/review_and_authority/promotion_authorized",
                "/review_and_authority/accepted_uses",
            ] {
                assert_eq!(
                    native_body.4.pointer(pointer),
                    oracle_body.4.pointer(pointer)
                );
            }
            assert_eq!(result["source_profiles"], oracle_result["source_profiles"]);
            python(
                &repository,
                isolated.path(),
                isolated.path(),
                UPDATE,
                Some(&json!({"action":"rebuild"})),
                deadline,
            );
            let current_files = authored_text_files(isolated.path());
            physical_fixture_budget(&current_files);
            let current = bounded_successor(
                &authored,
                &current_files,
                &store,
                base,
                "committed-cold-replay",
            );
            invocation["source_revision"] = json!(current.0.to_prefixed());
            invocation["original_source_revision"] = json!(base.0.to_prefixed());
            freeze_invocation(&invocation_path, &invocation);
            let retained = read_link_body(isolated.path(), config);
            let retained_bytes = authored_text_files(isolated.path());
            let replay = cli(
                &repository,
                &owner,
                &invocation_path,
                &request,
                true,
                deadline,
            );
            assert_eq!(replay["replayed"], json!(true));
            assert_eq!(read_link_body(isolated.path(), config), retained);
            assert_eq!(authored_text_files(isolated.path()), retained_bytes);
        }
        assert_eq!(fs::read(&subject).unwrap(), before);
        assert_eq!(fs::read(&sentinel).unwrap(), private_before);
        assert!(Instant::now() < deadline);
    }
}
