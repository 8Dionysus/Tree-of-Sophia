//! Actual maintained record-revision default-native caller, all seven owner schemas.
//! Private access constructs genuine deterministic pending plans, never authority.
use super::*;
use crate::source_creation_store::IsolatedCreationRoot;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tos_foundation::{JsonLimits, SourceRevision};
use tos_source_store::{CorpusReader, CutReadLimits, ReadLimits, SoftwareCaptureSelectionV1};
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

fn bounded_child(mut child: std::process::Child, out: &Path, err: &Path, deadline: Instant) {
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline
            || fs::metadata(out).unwrap().len() > 1_048_576
            || fs::metadata(err).unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded maintained Work setup refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < deadline);
    assert!(fs::metadata(out).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(err).unwrap().len() <= 1_048_576);
    assert!(status.success(), "maintained Work setup failed");
}

fn clean_git(command: &mut Command) -> &mut Command {
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
}

fn authored(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut directories = vec![root.join("ToS")];
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() {
                directories.push(entry.path());
                continue;
            }
            assert!(kind.is_file());
            let path = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            if path == "ToS/source-witnesses/.historical-create.writer.lock"
                || !tos_source_store::is_authored_source_path_v1(&path)
            {
                continue;
            }
            let raw = fs::read(entry.path()).unwrap();
            total = total.checked_add(raw.len()).unwrap();
            assert!(raw.len() <= 8_388_608 && total <= 33_554_432);
            assert!(files.insert(path, raw).is_none());
            assert!(files.len() <= 4096);
        }
    }
    files
}

fn cut(
    files: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (SourceRevision, CorpusCutReader) {
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("revisions")).unwrap();
    let members = files
        .iter()
        .map(|(path, raw)| {
            let sha = Digest256::of_bytes(raw).to_hex();
            fs::write(root.join("objects").join(&sha), raw).unwrap();
            serde_json::json!({"path":path,"sha256":sha,"size_bytes":raw.len(),"mode":420})
        })
        .collect::<Vec<_>>();
    let mut manifest = serde_json::json!({
        "schema_version":"tos_corpus_snapshot_v1", "base_revision":null,
        "files":members,"identities":{},"dependencies":{},"retirements":[],
        "validator_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    });
    let mut body = serde_json::to_vec(&manifest).unwrap();
    body.push(b'\n');
    let revision = SourceRevision(Digest256::of_bytes(&body));
    manifest["revision"] = serde_json::Value::String(revision.0.to_hex());
    let home = root.join("revisions").join(revision.0.to_hex());
    fs::create_dir(&home).unwrap();
    let mut raw = serde_json::to_vec(&manifest).unwrap();
    raw.push(b'\n');
    fs::write(home.join("snapshot.json"), raw).unwrap();
    let reader = CorpusReader::open_existing(
        root,
        ReadLimits {
            max_manifest_bytes: 4_194_304,
            max_manifest_entries: 2048,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
    )
    .unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 2048,
                max_total_bytes: 33_554_432,
                max_member_bytes: 8_388_608,
            },
            deadline,
            cancelled,
        )
        .unwrap();
    (revision, cut)
}

fn software(
    repository: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    scratch: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (SoftwareCaptureReader, SoftwareComponentSelectionV1) {
    let commit_out = scratch.join("commit.stdout");
    let commit_err = scratch.join("commit.stderr");
    bounded_child(
        clean_git(
            Command::new("git")
                .arg("-C")
                .arg(repository)
                .args(["rev-parse", "HEAD^{commit}"]),
        )
        .stdout(Stdio::from(fs::File::create(&commit_out).unwrap()))
        .stderr(Stdio::from(fs::File::create(&commit_err).unwrap()))
        .spawn()
        .unwrap(),
        &commit_out,
        &commit_err,
        deadline,
    );
    let sha = String::from_utf8(fs::read(&commit_out).unwrap())
        .unwrap()
        .trim()
        .to_owned();
    assert!(sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let archive_out = scratch.join("archive.stdout");
    let archive_err = scratch.join("archive.stderr");
    bounded_child(
        clean_git(
            Command::new("git")
                .arg("-C")
                .arg(repository)
                .arg("show")
                .arg(format!("{sha}:scripts/corpus_archive.py")),
        )
        .stdout(Stdio::from(fs::File::create(&archive_out).unwrap()))
        .stderr(Stdio::from(fs::File::create(&archive_err).unwrap()))
        .spawn()
        .unwrap(),
        &archive_out,
        &archive_err,
        deadline,
    );
    let tool = scratch.join("corpus_archive.py");
    fs::write(&tool, fs::read(&archive_out).unwrap()).unwrap();
    let capture = scratch.join("capture");
    let restored = scratch.join("restored");
    let names = files
        .keys()
        .filter(|name| !name.starts_with("ToS/"))
        .collect::<Vec<_>>();
    let mut command = Command::new("python3");
    command
        .arg(&tool)
        .arg("capture")
        .arg("--repo-root")
        .arg(repository)
        .arg("--commit")
        .arg(&sha)
        .arg("--output")
        .arg(&capture)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1");
    for name in &names {
        command.arg("--include-prefix").arg(name);
    }
    let capture_out = scratch.join("capture.stdout");
    let capture_err = scratch.join("capture.stderr");
    bounded_child(
        command
            .stdout(Stdio::from(fs::File::create(&capture_out).unwrap()))
            .stderr(Stdio::from(fs::File::create(&capture_err).unwrap()))
            .spawn()
            .unwrap(),
        &capture_out,
        &capture_err,
        deadline,
    );
    let restore_out = scratch.join("restore.stdout");
    let restore_err = scratch.join("restore.stderr");
    bounded_child(
        Command::new("python3")
            .arg(&tool)
            .arg("restore")
            .arg("--capture")
            .arg(&capture)
            .arg("--output")
            .arg(&restored)
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdout(Stdio::from(fs::File::create(&restore_out).unwrap()))
            .stderr(Stdio::from(fs::File::create(&restore_err).unwrap()))
            .spawn()
            .unwrap(),
        &restore_out,
        &restore_err,
        deadline,
    );
    let manifest_raw = fs::read(capture.join("capture.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_raw).unwrap();
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: sha,
        source_git_tree: manifest["source_git_tree"].as_str().unwrap().to_owned(),
        capture_manifest_sha256: Digest256::of_bytes(&manifest_raw),
    };
    let software = SoftwareCaptureReader::open(
        &capture,
        &restored,
        selection,
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 2_097_152,
            json: JsonLimits::default(),
        },
        deadline,
        cancelled,
    )
    .unwrap();
    let paths = names
        .iter()
        .map(|name| RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    (software, components)
}

fn worker(
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> CutWorkerSchemaExecutor {
    let path = std::env::var_os("TOS_SCHEMA_WORKER_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap())
                .join("debug/tos-schema-worker")
        });
    assert!(path.is_absolute() && path.is_file());
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    CutWorkerSchemaExecutor::from_cut(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            sha256: revision_cli_image(&path).0,
            absolute_path: path,
        },
        budget,
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 262_144,
        },
        deadline,
        cancelled,
    )
    .unwrap()
}

type Side = Option<(usize, Digest256)>;

struct SelectedWitness {
    path: String,
    before: Side,
    after: Side,
}

fn side(raw: Option<&[u8]>) -> Side {
    raw.map(|raw| (raw.len(), Digest256::of_bytes(raw)))
}

fn current_side(root: &Path, reference: &str) -> Side {
    match fs::read(root.join(reference)) {
        Ok(raw) => side(Some(&raw)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("selected Work member read: {error}"),
    }
}

fn mixed_selected(root: &Path, selected: &[SelectedWitness]) -> bool {
    let mut before = false;
    let mut after = false;
    for member in selected {
        let actual = current_side(root, &member.path);
        assert!(
            actual == member.before || actual == member.after,
            "selected Work member entered a third state: {}",
            member.path
        );
        if member.before == member.after {
            continue;
        }
        before |= actual == member.before;
        after |= actual == member.after;
    }
    before && after
}

fn revision_cli_image(path: &Path) -> (Digest256, (u64, u64, u64)) {
    use std::io::Read;
    use tos_foundation::Digest256Hasher;
    assert!(path.is_absolute());
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file() && metadata.permissions().mode() & 0o022 == 0);
    assert!(metadata.len() <= 536_870_912);
    let mut reader = fs::File::open(path).unwrap();
    let opened = reader.metadata().unwrap();
    assert_eq!(
        (opened.dev(), opened.ino(), opened.len()),
        (metadata.dev(), metadata.ino(), metadata.len())
    );
    let mut total = 0u64;
    let mut hash = Digest256Hasher::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let count = reader.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).unwrap();
        assert!(total <= opened.len());
        hash.update(&buffer[..count]);
    }
    let after = reader.metadata().unwrap();
    assert_eq!(
        (
            total,
            opened.dev(),
            opened.ino(),
            opened.len(),
            opened.mtime(),
            opened.mtime_nsec(),
            opened.ctime(),
            opened.ctime_nsec()
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
    (
        hash.finalize(),
        (metadata.dev(), metadata.ino(), metadata.len()),
    )
}

fn revision_cli_observe(
    repository: &Path,
    owner: &Path,
    invocation: &Path,
    request: &serde_json::Value,
    deadline: Instant,
) -> (bool, serde_json::Value) {
    use std::io::{Read, Seek, SeekFrom, Write};
    let raw = serde_json::to_vec(request).unwrap();
    assert!(raw.len() <= 1_048_576);
    let mut input = tempfile::tempfile().unwrap();
    input.write_all(&raw).unwrap();
    input.seek(SeekFrom::Start(0)).unwrap();
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut child =
        Command::new("/usr/bin/python3")
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
    let step = deadline.min(Instant::now() + Duration::from_secs(60));
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= step
            || output.metadata().unwrap().len() > 1_048_576
            || errors.metadata().unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded record revision actual native caller refused");
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
    output.read_to_end(&mut raw).unwrap();
    let mut diagnostic = Vec::new();
    errors.read_to_end(&mut diagnostic).unwrap();
    if !status.success() {
        return (
            false,
            serde_json::json!({"diagnostic":String::from_utf8_lossy(&diagnostic)}),
        );
    }
    let response: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(
        response["schema_version"],
        "tos_local_native_source_result_v1"
    );
    assert_eq!(response["grants_admission"], false);
    assert_eq!(response["result"]["grants_admission"], false);
    (true, response["result"].clone())
}

fn fixture(
    repository: &Path,
    root: &Path,
    scratch: &Path,
    scenario: usize,
    deadline: Instant,
) -> serde_json::Value {
    // The maintained factories own subject shapes, form construction and grants.
    // Redirect their TemporaryDirectory before construction, not after delegation.
    let script = r#"
import json,sys,unittest
from pathlib import Path
repo,root=map(Path,sys.argv[1:3]); scenario=int(sys.argv[3]); family=4 if scenario in (7,8) else min(scenario,6)
sys.path[:0]=[str(repo/'mechanics/growth-cycle/tests'),str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repo/'scripts'),str(repo/'tests')]
import test_source_revisions as fixtures
import test_source_selected_revisions as selected
import test_source_native_metadata_commands as native
import source_commands as source
import source_revisions as revisions
class ExistingRoot:
    def __init__(self,*args,**kwargs): self.name=str(root)
    def cleanup(self): pass
old=fixtures.tempfile.TemporaryDirectory
fixtures.tempfile.TemporaryDirectory=ExistingRoot
try:
    if family==6:
        case=unittest.TestCase(); f=native.NativeFixture(case,'artifact')
    elif family in (4,5) or scenario in (7,8):
        case=selected.SelectedSourceRevisionTests(); case.setUp(); f=case.fixture
    else:
        cls={0:fixtures.SourceRevisionTests,1:fixtures.ProfileSourceRevisionTests,2:fixtures.ProfileSourceRevisionTests,3:fixtures.NativeSourceRevisionTests}.get(family,fixtures.SourceRevisionTests)
        f=cls(); f.setUp()
finally:
    fixtures.tempfile.TemporaryDirectory=old
if scenario==9:
    # A separate legacy fixture supplies an exact retry, not a recover grant.
    raise AssertionError('legacy scenario uses family zero argument')
if family==2: f.config['schema_version']=source.PROFILE_SCOPE_REVISION_CONFIG
if family==5: f.config['schema_version']=source.CORPUS_COMPLETE_REVISION_CONFIG
f.owner.write_bytes(revisions._encode(f.config)); f.owner.chmod(0o600)
for ref in ('ToS/contracts/knowledge-assessment.schema.json','ToS/contracts/human-form.schema.json','ToS/contracts/human-form-set.schema.json','ToS/contracts/human-form-template.schema.json'):
    path=root/ref;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes((repo/ref).read_bytes())
request=f.request() if family==6 else f.request('native:revision-family-'+str(scenario))
proposal={k:v for k,v in request.items() if k not in ('command_id','expected_configuration','expected_source','expected_revision','expected_dependencies','expected_publication')}
proposal['operation']='prepare-revise'
print(json.dumps({'owner':str(f.owner),'source_path':f.relative,'record':f.record,'proposal':proposal,'request':request,'untouched':[str(p.relative_to(root)) for p in f.path.parent.rglob('*') if p.is_file() and p not in (f.path,f.formpath)]},ensure_ascii=False,separators=(',',':')))
"#;
    let out = scratch.join("fixture.stdout");
    let err = scratch.join("fixture.stderr");
    bounded_child(
        Command::new("/usr/bin/python3")
            .args(["-c", script])
            .arg(repository)
            .arg(root)
            .arg(scenario.to_string())
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdout(Stdio::from(fs::File::create(&out).unwrap()))
            .stderr(Stdio::from(fs::File::create(&err).unwrap()))
            .spawn()
            .unwrap(),
        &out,
        &err,
        deadline,
    );
    serde_json::from_slice(&fs::read(out).unwrap()).unwrap()
}

const IMPLEMENTATIONS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "scripts/source_record_profiles.py",
    "scripts/native_text_binding.py",
    "scripts/source_owner_context.py",
    "scripts/source_witness_human_forms.py",
    "scripts/source_metadata_snapshot.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_selected_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_native_metadata_commands.py",
    "scripts/build_source_witness_catalog.py",
];

fn freeze_invocation(path: &Path, value: &serde_json::Value) {
    let raw = serde_json::to_vec(value).unwrap();
    assert!(raw.len() <= 1_048_576);
    fs::write(path, raw).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn command_context(
    revision: SourceRevision,
    owner: &[u8],
    request: &serde_json::Value,
    files: &BTreeMap<String, Vec<u8>>,
) -> CommandContext {
    CommandContext {
        base_revision: revision,
        configuration_raw: owner.to_vec(),
        request_raw: serde_json::to_vec(request).unwrap(),
        recorded_at: "2026-09-29T12:34:56+00:00".into(),
        effective_uid: rustix::process::getuid().as_raw().into(),
        files: files
            .iter()
            .map(|(path, raw)| SourceFile {
                path: RelativePath::parse(path).unwrap(),
                raw: raw.clone(),
            })
            .collect(),
    }
}

fn pending_factory(
    filesystem: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Vec<SelectedWitness> {
    let (config, family) = revision::configuration(ctx).unwrap();
    let publication = family
        .selected()
        .then(|| revision::read_record_revision_publication(ctx, &[]).unwrap());
    let mut schema = worker(cut, deadline, cancelled);
    let proposal = revision::prepare_record_revision_from_captures(
        ctx,
        publication.as_ref(),
        cut,
        software,
        components,
        &mut schema,
        deadline,
        cancelled,
    )
    .unwrap();
    schema.finish(deadline, cancelled).unwrap();
    drop(schema);
    let before = package(ctx, &config).unwrap();
    let names = revision::names(cmd::text(&config, "source_path").unwrap()).unwrap();
    let record = cmd::parse(&before[&names[0]]).unwrap();
    let plan = plan(ctx, &config, family, &record, &proposal).unwrap();
    let witnesses = plan
        .files
        .iter()
        .map(|f| SelectedWitness {
            path: f.path.as_str().into(),
            before: side(f.before.as_deref()),
            after: side(f.after.as_deref()),
        })
        .collect::<Vec<_>>();
    let snapshot = PublicationSnapshot::select(filesystem, deadline, cancelled).unwrap();
    let archive = tx::record_revision_archive(
        filesystem,
        ctx,
        &record,
        &before,
        &revision::revision(&before).unwrap(),
        deadline,
        cancelled,
        true,
    )
    .unwrap();
    let selected = physical::selected_sides(&plan).unwrap();
    let guard_plan = plan.clone();
    let prior =
        physical::original_prior_publication(cut, snapshot.token.as_deref(), deadline, cancelled)
            .unwrap();
    let fence = tx::WorkCorpusFence::hold(filesystem, deadline, cancelled).unwrap();
    let mut stopped = false;
    let result = fence.apply(
        plan,
        &snapshot,
        |_, extent| {
            physical::physical_current(
                filesystem,
                ctx,
                cut,
                &selected,
                &empty_bindings(),
                Some(&snapshot),
                &archive,
                prior.as_deref().zip(snapshot.token.as_deref()),
                &extent,
                deadline,
                cancelled,
            )?;
            dependencies_current(
                filesystem,
                &proposal,
                &guard_plan,
                &archive,
                &extent,
                deadline,
                cancelled,
            )?;
            if extent.pending_state.is_some() && mixed_selected(&filesystem.root_path, &witnesses) {
                stopped = true;
                return Err(SourceCommandError::Conflict(
                    "deterministic exact revision pending",
                ));
            }
            Ok(())
        },
        deadline,
        cancelled,
    );
    assert!(stopped && result.is_err());
    witnesses
}

#[test]
fn native_record_revisions_cover_fixed_handlers_process_cold_and_exact_recovery() {
    let repository = repository();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("immutable native CLI required"),
    );
    let worker_path = PathBuf::from(
        std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("immutable worker required"),
    );
    let consumer = std::env::current_exe().unwrap();
    let image_guards = [
        revision_cli_image(&consumer),
        revision_cli_image(&native),
        revision_cli_image(&worker_path),
    ];
    // Seven fresh fixed schemas, selected resume and rollback, legacy exact retry.
    // All ten roots are sequential, and every cold operation retains the same
    // absolute protected root/config; no archive relocation claim is made.
    for scenario in 0..10 {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let family = if scenario == 9 { 0 } else { scenario };
        let fixture = fixture(
            &repository,
            isolated.path(),
            temporary.path(),
            family,
            deadline,
        );
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        let owner_raw = fs::read(&owner).unwrap();
        let config: serde_json::Value = serde_json::from_slice(&owner_raw).unwrap();
        let schema_index = if scenario == 9 {
            0
        } else if scenario >= 7 {
            4
        } else {
            scenario
        };
        assert_eq!(
            config["schema_version"],
            [
                "tos_local_source_revision_owner_v1",
                "tos_local_profile_revision_owner_v1",
                "tos_local_profile_revision_owner_v2",
                "tos_local_corpus_revision_owner_v1",
                "tos_local_corpus_revision_owner_v2",
                "tos_local_corpus_revision_owner_v3",
                "tos_local_native_metadata_revision_owner_v1"
            ][schema_index]
        );
        let untouched: BTreeMap<String, Vec<u8>> = fixture["untouched"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let reference = p.as_str().unwrap();
                let path = isolated.path().join(reference);
                assert!(
                    path.symlink_metadata().unwrap().is_file()
                        && fs::metadata(&path).unwrap().len() <= 8_388_608
                );
                (reference.to_owned(), fs::read(path).unwrap())
            })
            .collect();
        assert!(untouched.values().map(Vec::len).sum::<usize>() <= 33_554_432);
        let initial = authored(isolated.path());
        let mut files = initial.clone();
        for reference in IMPLEMENTATIONS {
            let source = repository.join(reference);
            assert!(source.symlink_metadata().unwrap().is_file());
            assert!(fs::metadata(&source).unwrap().len() <= 2_097_152);
            let raw = fs::read(source).unwrap();
            let destination = isolated.path().join(reference);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, &raw).unwrap();
            assert!(files.insert((*reference).into(), raw).is_none());
        }
        assert!(files.len() <= 2048 && files.values().map(Vec::len).sum::<usize>() <= 33_554_432);
        let (software, components) =
            software(&repository, &files, temporary.path(), deadline, &cancelled);
        let store = temporary.path().join("cut");
        let (original_revision, original) = cut(&initial, &store, deadline, &cancelled);
        let selection = software.selection();
        let invocation_path = temporary.path().join("native-revision-invocation.json");
        let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
            "owner_config":owner,"owner_context":null,"assessment_schema_worker":null,
            "native_executable":native,"native_executable_sha256":image_guards[1].0.to_prefixed(),
            "corpus_store":store,"source_revision":original_revision.0.to_prefixed(),"original_source_revision":original_revision.0.to_prefixed(),
            "software_capture":temporary.path().join("capture"),"software_restored_root":temporary.path().join("restored"),
            "software_selection":{"source_git_commit":selection.source_git_commit,"source_git_tree":selection.source_git_tree,"capture_manifest_sha256":selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":worker_path,"sha256":image_guards[2].0.to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
                "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        freeze_invocation(&invocation_path, &invocation);
        let invoke = |request: &serde_json::Value| {
            let (success, result) =
                revision_cli_observe(&repository, &owner, &invocation_path, request, deadline);
            assert!(success, "scenario {scenario}: {result}");
            result
        };
        let described = invoke(
            &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
        );
        assert_eq!(
            described["schema_version"],
            if schema_index >= 4 {
                "tos_local_source_revision_result_v2"
            } else {
                "tos_local_source_revision_result_v1"
            }
        );
        assert_eq!(
            described["supported_operations"],
            if schema_index >= 4 {
                serde_json::json!(["record.revise", "record.recover"])
            } else {
                serde_json::json!(["record.revise"])
            }
        );
        let preview = invoke(&fixture["proposal"]);
        assert_eq!(
            preview["owner_configuration"],
            described["owner_configuration"]
        );
        let oracle = &fixture["request"];
        for (field, prepared) in [
            ("expected_configuration", "owner_configuration"),
            ("expected_source", "source"),
            ("expected_revision", "revision"),
            ("expected_dependencies", "expected_dependencies"),
        ] {
            assert_eq!(
                preview[prepared], oracle[field],
                "scenario {scenario} {field}"
            );
        }
        let mut request = oracle.clone();
        request["command_id"] = serde_json::json!(format!("native:revision-{scenario}"));
        let source_path = fixture["source_path"].as_str().unwrap();
        let source = isolated.path().join(source_path);
        let names = revision::names(source_path).unwrap();
        let home = source.parent().unwrap();
        let before_selected: BTreeMap<_, _> = names
            .iter()
            .map(|n| (n.clone(), fs::read(home.join(n)).ok()))
            .collect();
        if scenario >= 7 {
            let filesystem =
                CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled)
                    .unwrap();
            let ctx = command_context(original_revision, &owner_raw, &request, &files);
            let witnesses = pending_factory(
                &filesystem,
                &ctx,
                &original,
                &software,
                &components,
                deadline,
                &cancelled,
            );
            let retained_pending = tx::read_pending(&filesystem, deadline, &cancelled)
                .unwrap()
                .unwrap();
            let transaction = retained_pending.plan.transaction_id.clone();
            let control = fs::read(isolated.path().join(CONTROL)).unwrap();
            let mut unrelated = initial.clone();
            unrelated
                .get_mut(fixture["source_path"].as_str().unwrap())
                .unwrap()
                .push(b'\n');
            let (wrong_original, _) = cut(&unrelated, &store, deadline, &cancelled);
            let recovery = if scenario == 9 {
                request.clone()
            } else {
                serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"record.recover",
                "transaction_id":transaction,"decision":if scenario==8 {"rollback"} else {"resume"},"expected_configuration":preview["owner_configuration"]})
            };
            let saved_sides = witnesses
                .iter()
                .map(|w| current_side(isolated.path(), &w.path))
                .collect::<Vec<_>>();
            // Wrong original, grant revocation, changed dependency, source third
            // state and transaction mismatch each retain the exact pending bytes.
            for negative in 0..5 {
                let mut selected_invocation = invocation.clone();
                let mut selected_request = recovery.clone();
                let mut restore = None;
                match negative {
                    0 => {
                        selected_invocation["original_source_revision"] =
                            serde_json::json!(wrong_original.0.to_prefixed());
                        freeze_invocation(&invocation_path, &selected_invocation);
                    }
                    1 => {
                        let mut config: serde_json::Value =
                            serde_json::from_slice(&owner_raw).unwrap();
                        config["allowed_operations"] = serde_json::json!([]);
                        fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
                        restore = Some((owner.clone(), owner_raw.clone()));
                    }
                    2 => {
                        let path = isolated
                            .path()
                            .join("ToS/contracts/corpus-record.schema.json");
                        let raw = fs::read(&path).unwrap();
                        fs::write(&path, [raw.as_slice(), b"\n"].concat()).unwrap();
                        restore = Some((path, raw));
                    }
                    3 => {
                        let raw = fs::read(&source).unwrap();
                        fs::write(&source, b"{}\n").unwrap();
                        restore = Some((source.clone(), raw));
                    }
                    _ => {
                        if scenario == 9 {
                            selected_request["command_id"] = serde_json::json!("wrong-exact-retry");
                        } else {
                            selected_request["transaction_id"] = serde_json::json!(
                                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            );
                        }
                    }
                }
                let refusal_sides = witnesses
                    .iter()
                    .map(|w| current_side(isolated.path(), &w.path))
                    .collect::<Vec<_>>();
                assert!(
                    !revision_cli_observe(
                        &repository,
                        &owner,
                        &invocation_path,
                        &selected_request,
                        deadline
                    )
                    .0
                );
                assert_eq!(
                    witnesses
                        .iter()
                        .map(|w| current_side(isolated.path(), &w.path))
                        .collect::<Vec<_>>(),
                    refusal_sides
                );
                assert_eq!(fs::read(isolated.path().join(CONTROL)).unwrap(), control);
                if let Some((path, raw)) = restore {
                    fs::write(path, raw).unwrap();
                }
                freeze_invocation(&invocation_path, &invocation);
            }
            assert_eq!(
                witnesses
                    .iter()
                    .map(|w| current_side(isolated.path(), &w.path))
                    .collect::<Vec<_>>(),
                saved_sides
            );
            let result = invoke(&recovery);
            assert!(!pending(&filesystem, deadline, &cancelled).unwrap());
            if scenario == 8 {
                for (name, raw) in &before_selected {
                    assert_eq!(fs::read(home.join(name)).ok(), *raw);
                }
                // Continue with the terminal rollback carrier in the next exact
                // current cut and an independently prepared new command.
                let current = authored(isolated.path());
                let (revision, _) = cut(&current, &store, deadline, &cancelled);
                invocation["source_revision"] = serde_json::json!(revision.0.to_prefixed());
                invocation["original_source_revision"] =
                    serde_json::json!(revision.0.to_prefixed());
                freeze_invocation(&invocation_path, &invocation);
                let next = invoke(&fixture["proposal"]);
                request["command_id"] = serde_json::json!("native:after-rollback");
                for (field, prepared) in [
                    ("expected_configuration", "owner_configuration"),
                    ("expected_source", "source"),
                    ("expected_revision", "revision"),
                    ("expected_dependencies", "expected_dependencies"),
                    ("expected_publication", "expected_publication"),
                ] {
                    request[field] = next[prepared].clone();
                }
                invoke(&request);
            } else {
                assert!(result["receipt"].is_object());
            }
        } else {
            let created = invoke(&request);
            assert_eq!(created["replayed"], false);
            let after = authored(isolated.path());
            let (current_revision, _) = cut(&after, &store, deadline, &cancelled);
            invocation["source_revision"] = serde_json::json!(current_revision.0.to_prefixed());
            freeze_invocation(&invocation_path, &invocation);
            let replay = invoke(&request);
            assert_eq!(replay["replayed"], true);
            assert_eq!(created["receipt"], replay["receipt"]);
            assert_eq!(authored(isolated.path()), after);
            let inspected = invoke(
                &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-version","source":preview["source"]}),
            );
            assert_eq!(inspected["record"], fixture["record"]);
            // The maintained inspector names exact archived byte locators.
            for (name, binding) in inspected["files"].as_object().unwrap() {
                let original = initial
                    .get(&format!(
                        "{}/{name}",
                        source_path.rsplit_once('/').unwrap().0
                    ))
                    .unwrap();
                assert_eq!(
                    fs::read(
                        isolated
                            .path()
                            .join(binding["archive_path"].as_str().unwrap())
                    )
                    .unwrap(),
                    *original
                );
            }
            let mut wrong = invocation.clone();
            wrong["original_source_revision"] = serde_json::json!(current_revision.0.to_prefixed());
            freeze_invocation(&invocation_path, &wrong);
            assert!(
                !revision_cli_observe(&repository, &owner, &invocation_path, &request, deadline).0
            );
            assert_eq!(authored(isolated.path()), after);
            freeze_invocation(&invocation_path, &invocation);
        }
        let current: serde_json::Value =
            serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
        let mut expected = fixture["record"].clone();
        for (field, value) in request["fields"].as_object().unwrap() {
            expected[field] = value.clone();
        }
        expected["record_version"] = serde_json::json!(2);
        assert_eq!(current, expected);
        // Only exact selected record/forms/history and retained archive/control
        // may change; all prior unrelated flat and descendant bytes survive.
        let selected_refs = names
            .iter()
            .map(|n| format!("{}/{n}", source_path.rsplit_once('/').unwrap().0))
            .collect::<BTreeSet<_>>();
        for (path, raw) in &initial {
            if !selected_refs.contains(path) {
                assert_eq!(fs::read(isolated.path().join(path)).unwrap(), *raw);
            }
        }
        for (path, raw) in &untouched {
            assert_eq!(fs::read(isolated.path().join(path)).unwrap(), *raw);
        }
        assert_eq!(fs::read(&owner).unwrap(), owner_raw);
    }
    assert_eq!(
        [
            revision_cli_image(&consumer),
            revision_cli_image(&native),
            revision_cli_image(&worker_path)
        ],
        image_guards
    );
    assert!(Instant::now() < deadline);
}
