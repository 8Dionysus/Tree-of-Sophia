use super::command_form_cases::{context as cut_context, open_cut, schemas};
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_command::{CommandContext, SourceCommandError, SourceFile};
use tos_command::source_revisions::{
    RetainedRevisionTransaction, RevisionPublication, RevisionTransactionStatus,
    prepare_record_revision, prepare_record_revision_from_captures,
    prepare_record_revision_with_profile_cut, read_record_revision_publication,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonString, JsonValue, RelativePath,
    SourceRevision, canonical_bytes_v1, parse_json,
};

use tos_validation::FormatProfile;

/// Profiles keep independent plans/receipts/child state while one sealed image
/// owns executable custody for this bounded record scenario.
fn profile_workers(
    cut: &tos_source_store::CorpusCutReader,
    image: &tos_validation::executor::VerifiedWorkerImageHandle,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (
    tos_validation::source_cut::CutWorkerSchemaExecutor,
    tos_validation::source_cut::CutWorkerSchemaExecutor,
) {
    use tos_validation::executor::ExecutorBudget;
    let make = |profile| {
        super::command_form_cases::schemas_for_profile_with_image(
            cut,
            profile,
            image,
            ExecutorBudget::laboratory(),
            deadline,
            cancelled,
        )
    };
    (
        make(FormatProfile::LegacyPythonObserved20260923),
        make(FormatProfile::AssertedSourceCandidateV1),
    )
}

#[test]
fn sign_uses_current_native_content_assessment_and_replays_its_original_package() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::process::Command;
    use tos_command::source_creation::prepare_sign_promotion_from_captures;
    use tos_command::source_creation_store::{
        CreationDurability, CreationFilesystem, IsolatedCreationRoot,
    };
    use tos_validation::assessment::AssessmentLimits;
    use tos_validation::executor::BatchBudget;
    use tos_validation::source_cut::CutSchemaExecutor;

    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
    // The normal maintained synthetic factory authors the judgments and the
    // separately delegated issuance grant. Relocation happens before its
    // public rebind, review append and Sign describe; no ready report is fed
    // to Rust. Fresh synthetic competence and authority are valid through 2099
    // before their refs/reviews are authored. Native current-clock expiry
    // checks and the maintained separate expiry-negative cases stay unchanged.
    let factory = r#"
import json,sys,shutil,stat
from pathlib import Path
from unittest.mock import patch
repo,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repo/'mechanics/growth-cycle/tests'),str(repo/'tests'),str(repo/'scripts'),str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
from test_occurrence_assessment_guard import OccurrenceAssessmentGuardTests
from test_native_text_assessment import NativeAssessmentFixture
import test_knowledge_assessment as assessment_policy_fixtures
import source_commands as commands
original=NativeAssessmentFixture.__init__
def selected(self,test,**kwargs):
    original(self,test,**kwargs)
    shutil.copytree(self.root,root,dirs_exist_ok=True)
    self.root=self.native.root=root
    self.owner=root/'assessment-owner.json'
    self.config['source_root']=str(root)
    self.config['journal_directory']=str(root/'assessment-journal')
    (root/'assessment-journal').mkdir(mode=0o700)
    self.save()
test=OccurrenceAssessmentGuardTests(methodName='runTest')
try:
    with (patch.object(assessment_policy_fixtures,'END','2099-01-01T00:00:00Z'),
          patch.object(NativeAssessmentFixture,'__init__',selected)):
        f,owner,config,request=test.sign_command_fixture()
    preview={key:value for key,value in request.items() if key not in {'command_id','expected_configuration','expected_source','expected_revision','expected_dependencies'}}
    preview['operation']='prepare-create'
    # Mirror run_local_command's actual input freeze before its private builder.
    # Keep every original ordered output buffer; do not re-encode expected files.
    preview=commands._json_object(commands._canonical(preview))
    configured,_,_=commands._configuration(owner)
    _,outputs,_=commands._prepare_creation(configured,preview)
    response=commands.run_legacy_oracle_command(owner,preview)
    # Complete private custody for this synthetic root, under existing v1 law.
    # Inclusion does not grant native content disclosure or source admission.
    def eligible(p):
        ref=p.relative_to(root).as_posix()
        return p.is_file() and not {'.git','payload','owner-local'}.intersection(p.relative_to(root).parts) and (not (ref.startswith('ToS/derived-exports/') or ref.startswith('ToS/source-witnesses/catalog/')) or ref.endswith('.md'))
    selected=[p for p in sorted((root/'ToS').rglob('*')) if eligible(p)]
    # Construct only this synthetic authored fixture in the supported v1 modes.
    # Private root/owner/journal permissions and content bytes stay independent.
    for p in selected:
        if p.is_symlink(): raise RuntimeError('synthetic authored symlink')
        p.chmod(0o755 if stat.S_IMODE(p.stat().st_mode)&0o111 else 0o644)
    authored={p.relative_to(root).as_posix():p.read_bytes().hex() for p in selected}
    modes={p.relative_to(root).as_posix():stat.S_IMODE(p.stat().st_mode) for p in selected}
    head=root/'assessment-journal'/__import__('hashlib').sha256(f.identifier.encode()).hexdigest()/'head'
    print(json.dumps({'owner':str(owner),'assessment_owner':str(f.owner),'config_raw':owner.read_bytes().hex(),'request':request,'preview_request':preview,'preview_raw':commands._canonical(response).hex(),'outputs':{name:raw.hex() for name,raw in outputs.items()},'authored':authored,'modes':modes,'content':f.native.content_ref,'scope_source':f.occurrence_path,'head':str(head),'original_payload':f.native.original_ref},ensure_ascii=False,allow_nan=False))
finally:
    test.doCleanups()
"#;
    let mut oracle = Command::new(crate::maintained_python());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") || key == "PYTHONPATH" || key == "PYTHONHOME" {
            oracle.env_remove(key);
        }
    }
    let output = oracle
        .args(["-c", factory])
        .arg(&repository)
        .arg(isolated.path())
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "maintained Sign factory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let oracle: Value = serde_json::from_slice(&output.stdout).unwrap();
    let authored = oracle["authored"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, raw)| (name.clone(), decode_hex(raw.as_str().unwrap())))
        .collect::<BTreeMap<_, _>>();
    assert!(
        !isolated
            .path()
            .join(required(&oracle, "original_payload"))
            .exists()
    );
    assert!(authored.keys().all(|name| {
        !name
            .split('/')
            .any(|part| matches!(part, ".git" | "owner-local" | "payload"))
    }));
    assert!(authored.contains_key(required(&oracle, "content")));
    assert!(
        authored
            .keys()
            .any(|name| name.split('/').any(|part| part == "local-content"))
    );
    let modes = oracle["modes"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, mode)| (name.clone(), u32::try_from(mode.as_u64().unwrap()).unwrap()))
        .collect::<BTreeMap<_, _>>();
    let mut files = authored.clone();
    // Rule implementation and native serialization observations are selected
    // through a separate exact software capture, never the authored cut.
    for name in [
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py",
        "scripts/source_witness_human_forms.py",
        "scripts/build_source_witness_catalog.py",
        "scripts/source_record_profiles.py",
        "scripts/native_text_binding.py",
        "scripts/source_owner_context.py",
        "scripts/source_witness_bibliographic_graph_common.py",
        "rust/crates/tos-command/src/source_creation.rs",
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_revisions.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "rust/crates/tos-command/src/source_sign.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
    ] {
        let raw = fs::read(repository.join(name)).unwrap();
        let target = isolated.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, &raw).unwrap();
        files.insert(name.into(), raw);
    }
    let store = temporary.path().join("sign-cut");
    let revision =
        super::validation_cut_cases::write_cut_store_with_modes(&authored, &store, &modes);
    let cut = open_cut(&store, revision, deadline, &cancellation);
    // Cheap complete selection check before any worker/content evaluation.
    let selected = cut
        .current()
        .members()
        .map(|member| {
            (
                member.path.as_str().to_owned(),
                (member.sha256, member.size_bytes, member.mode),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected = authored
        .iter()
        .map(|(name, raw)| {
            (
                name.clone(),
                (Digest256::of_bytes(raw), raw.len() as u64, modes[name]),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(selected, expected);
    let mut actual = BTreeMap::new();
    let mut directories = vec![isolated.path().join("ToS")];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path
                .strip_prefix(isolated.path())
                .unwrap()
                .to_str()
                .unwrap();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                if tos_source_store::has_authored_source_descendants_v1(name) {
                    directories.push(path);
                }
            } else if tos_source_store::is_authored_source_path_v1(name) {
                assert!(metadata.is_file());
                let raw = fs::read(&path).unwrap();
                actual.insert(
                    name.to_owned(),
                    (
                        Digest256::of_bytes(&raw),
                        raw.len() as u64,
                        metadata.mode() & 0o7777,
                    ),
                );
            }
        }
    }
    assert_eq!(
        selected, actual,
        "complete actual eligible snapshot before worker execution"
    );
    let native_custody_before = authored
        .iter()
        .filter(|(name, _)| {
            name.as_str() == required(&oracle, "content")
                || name.split('/').any(|part| part == "local-content")
        })
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let (_capture, software, components) = captured_components(&files, deadline, &cancellation);
    let owner = Path::new(required(&oracle, "owner"));
    let config_raw = decode_hex(required(&oracle, "config_raw"));
    let mut context = cut_context(
        &files,
        config_raw.clone(),
        canonical_json(&oracle["preview_request"]),
        revision,
    );
    context.effective_uid = u64::from(fs::metadata(isolated.path()).unwrap().uid());
    context.recorded_at = "2026-09-26T12:00:00Z".into();
    let limits = AssessmentLimits {
        max_input_bytes: 8_388_608,
        max_work: 4_194_304,
        batch: BatchBudget::laboratory(),
        deadline,
    };
    let image = super::command_form_cases::schema_image(
        tos_validation::executor::ExecutorBudget::laboratory(),
        deadline,
        &cancellation,
    );
    // Re-enter the native Sign owner with changed current v2 scope inputs.
    // These are distinct route checks: requested-use fencing, Sign risk floor,
    // and exact native-content readiness. Each refusal precedes package creation.
    let assessment_owner = Path::new(required(&oracle, "assessment_owner"));
    let assessment_owner_raw = fs::read(assessment_owner).unwrap();
    let candidate_id = oracle["request"]["record"]["promotion_basis"]["candidate"]["id"]
        .as_str()
        .unwrap();
    let assessment_head = PathBuf::from(required(&oracle, "head"));
    let assessment_head_raw = fs::read(&assessment_head).unwrap();
    let native_content = isolated.path().join(required(&oracle, "content"));
    let native_content_raw = fs::read(&native_content).unwrap();
    for refused_scope in ["research-use", "low-risk", "metadata-only"] {
        let mut altered: Value = serde_json::from_slice(&assessment_owner_raw).unwrap();
        match refused_scope {
            "research-use" => {
                altered["subjects"][candidate_id]["requested_use"] = serde_json::json!("research");
            }
            "low-risk" => {
                altered["subjects"][candidate_id]["risk"] = serde_json::json!("low");
            }
            "metadata-only" => {
                for selection in altered["native_text_units"].as_array_mut().unwrap() {
                    selection["read_scope"] = serde_json::json!("metadata_only");
                }
            }
            _ => unreachable!(),
        }
        fs::write(assessment_owner, canonical_json(&altered)).unwrap();
        fs::set_permissions(assessment_owner, fs::Permissions::from_mode(0o600)).unwrap();
        let (mut negative_local, mut negative_assessment) =
            profile_workers(&cut, &image, deadline, &cancellation);
        assert!(
            prepare_sign_promotion_from_captures(
                owner,
                &context,
                &cut,
                &software,
                &components,
                &mut negative_local,
                &mut negative_assessment,
                limits,
                &cancellation,
            )
            .is_err(),
            "native Sign preparation must reject {refused_scope}"
        );
        fs::write(assessment_owner, &assessment_owner_raw).unwrap();
        fs::set_permissions(assessment_owner, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(fs::read(&assessment_head).unwrap(), assessment_head_raw);
        assert_eq!(fs::read(&native_content).unwrap(), native_content_raw);
    }
    assert_eq!(fs::read(assessment_owner).unwrap(), assessment_owner_raw);

    let (mut local_worker, mut assessment_worker) =
        profile_workers(&cut, &image, deadline, &cancellation);
    let prepared = prepare_sign_promotion_from_captures(
        owner,
        &context,
        &cut,
        &software,
        &components,
        &mut local_worker,
        &mut assessment_worker,
        limits,
        &cancellation,
    )
    .unwrap();
    let preview = prepared.preview().unwrap();
    assert_eq!(
        bytes(&preview),
        decode_hex(required(&oracle, "preview_raw")),
        "entire maintained Sign preparation including current full basis"
    );
    for (name, raw) in prepared.files() {
        assert_eq!(
            *raw,
            decode_hex(oracle["outputs"][name].as_str().unwrap()),
            "original Sign buffer {name}"
        );
    }
    local_worker.finish(deadline, &cancellation).unwrap();
    // The request retains the Python author's exact basis/configuration and
    // dependency values; Rust must independently reconstruct them again.
    context.request_raw = canonical_json(&oracle["request"]);
    let (mut local_worker, mut assessment_worker) =
        profile_workers(&cut, &image, deadline, &cancellation);
    let prepared = prepare_sign_promotion_from_captures(
        owner,
        &context,
        &cut,
        &software,
        &components,
        &mut local_worker,
        &mut assessment_worker,
        limits,
        &cancellation,
    )
    .unwrap();
    let serialized = prepared
        .serialize(
            &software,
            &components,
            &mut local_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
    local_worker.finish(deadline, &cancellation).unwrap();
    assert!(matches!(
        serialized.command().commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    ));
    let filesystem =
        CreationFilesystem::select_isolated(&isolated, owner, deadline, &cancellation).unwrap();
    // Every change is made to this finite test root, then restored. The same
    // package must refuse current content/config/head/source-scope drift.
    for target in [
        isolated.path().join(required(&oracle, "content")),
        PathBuf::from(required(&oracle, "assessment_owner")),
        PathBuf::from(required(&oracle, "head")),
        isolated.path().join(required(&oracle, "scope_source")),
        owner.to_path_buf(),
    ] {
        let original = fs::read(&target).unwrap();
        let changed = if target == Path::new(required(&oracle, "assessment_owner")) {
            let mut config: Value = serde_json::from_slice(&original).unwrap();
            let candidate = oracle["request"]["record"]["promotion_basis"]["candidate"]["id"]
                .as_str()
                .unwrap();
            config["subjects"][candidate]["requested_use"] = serde_json::json!("research");
            canonical_json(&config)
        } else if target == owner {
            let mut config: Value = serde_json::from_slice(&original).unwrap();
            config["allowed_operations"] = serde_json::json!([]);
            canonical_json(&config)
        } else if target == Path::new(required(&oracle, "head")) {
            b"not-a-journal-revision\n".to_vec()
        } else if target == isolated.path().join(required(&oracle, "scope_source")) {
            let mut occurrence: Value = serde_json::from_slice(&original).unwrap();
            occurrence["native_text_binding"]["source_record_refs"]["work"] =
                serde_json::json!("ToS/source-witnesses/works/unselected/work.json");
            canonical_json(&occurrence)
        } else {
            let mut changed = original.clone();
            changed.push(b'\n');
            changed
        };
        fs::write(&target, changed).unwrap();
        let (mut local, mut assessment) = profile_workers(&cut, &image, deadline, &cancellation);
        assert!(
            filesystem
                .publish_sign_isolated(
                    &serialized,
                    &cut,
                    &software,
                    &components,
                    &mut local,
                    &mut assessment,
                    limits,
                    &cancellation
                )
                .is_err(),
            "changed selected input {}",
            target.display()
        );
        fs::write(&target, original).unwrap();
        // Owner configurations retain their original private mode.
        if target == owner || target == Path::new(required(&oracle, "assessment_owner")) {
            fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let native_before = fs::read(isolated.path().join(required(&oracle, "content"))).unwrap();
    let (mut local_worker, mut assessment_worker) =
        profile_workers(&cut, &image, deadline, &cancellation);
    let published = filesystem
        .publish_sign_isolated(
            &serialized,
            &cut,
            &software,
            &components,
            &mut local_worker,
            &mut assessment_worker,
            limits,
            &cancellation,
        )
        .unwrap();
    assert!(!published.replayed);
    assert_eq!(published.durability, CreationDurability::DirectoriesSynced);
    let home = isolated.path().join(published.home.as_str());
    assert_eq!(
        fs::read(home.join("sign.json")).unwrap(),
        decode_hex(oracle["outputs"]["sign.json"].as_str().unwrap())
    );
    assert_eq!(
        fs::read(isolated.path().join(required(&oracle, "content"))).unwrap(),
        native_before
    );
    for (name, original) in &native_custody_before {
        assert_eq!(fs::read(isolated.path().join(name)).unwrap(), *original);
    }
    let content_path = isolated.path().join(required(&oracle, "content"));
    let mut changed_content = native_before.clone();
    changed_content.push(b'\n');
    fs::write(&content_path, changed_content).unwrap();
    let (mut local_worker, mut assessment_worker) =
        profile_workers(&cut, &image, deadline, &cancellation);
    assert!(
        filesystem
            .replay_sign_isolated(
                &serialized,
                &cut,
                &software,
                &components,
                &mut local_worker,
                &mut assessment_worker,
                limits,
                &cancellation
            )
            .is_err(),
        "replay cannot inherit earlier content verification"
    );
    fs::write(&content_path, &native_before).unwrap();
    let (mut local_worker, mut assessment_worker) =
        profile_workers(&cut, &image, deadline, &cancellation);
    let replay = filesystem
        .replay_sign_isolated(
            &serialized,
            &cut,
            &software,
            &components,
            &mut local_worker,
            &mut assessment_worker,
            limits,
            &cancellation,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.receipt_sha256, published.receipt_sha256);
    for (name, original) in &native_custody_before {
        assert_eq!(fs::read(isolated.path().join(name)).unwrap(), *original);
    }
    let form_set: Value =
        serde_json::from_slice(&fs::read(home.join("sign.human-forms.json")).unwrap()).unwrap();
    assert!(form_set["forms"].as_array().unwrap().iter().all(|form| {
        form["bindings"]
            .as_object()
            .unwrap()
            .values()
            .any(|binding| binding["pointer"] == "/promotion_basis")
    }));
}

const SOURCE: &[u8] = include_bytes!(
    "../../../rust/crates/tos-command/tests/fixtures/source_forms_shadow/source.initial.json"
);
const SOURCE_PATH: &str = "ToS/source-witnesses/works/fixture/work.json";
fn parse(raw: &[u8]) -> JsonValue {
    parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
        .unwrap()
        .into_root()
}
fn bytes(v: &JsonValue) -> Vec<u8> {
    canonical_bytes_v1(
        v,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits::default(),
    )
    .unwrap()
}
fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn obj(values: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn set(v: &mut JsonValue, key: &str, value: JsonValue) {
    let JsonValue::Object(fields) = v else {
        panic!("object")
    };
    if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *old = value
    } else {
        fields.push((JsonString::from_utf8(key), value));
    }
}
fn arr(values: &[&str]) -> JsonValue {
    JsonValue::Array(values.iter().map(|s| text(s)).collect())
}
fn file(path: &str, raw: &[u8]) -> SourceFile {
    SourceFile {
        path: RelativePath::parse(path).unwrap(),
        raw: raw.to_vec(),
    }
}
fn context(selected: bool) -> CommandContext {
    let source = parse(SOURCE);
    let config = obj(vec![
        (
            "schema_version",
            text(if selected {
                "tos_local_corpus_revision_owner_v2"
            } else {
                "tos_local_corpus_revision_owner_v1"
            }),
        ),
        ("uid", parse(b"1000")),
        ("principal_id", text("test-reviewer")),
        ("source_root", text("/selected-owner-root")),
        ("source_path", text(SOURCE_PATH)),
        ("authority_ref", text("owner-test-authority")),
        ("expires_at", text("2030-01-01T00:00:00Z")),
        ("record_id", source.object_get("record_id").unwrap().clone()),
        ("record_type", text("work")),
        (
            "allowed_operations",
            arr(if selected {
                &["record.revise", "record.recover"]
            } else {
                &["record.revise"]
            }),
        ),
        ("allowed_fields", arr(&["preferred_label", "notes"])),
        ("allowed_form_ids", arr(&["tos.form.revision.fixture-name"])),
    ]);
    let mut files = vec![file(SOURCE_PATH, SOURCE)];
    macro_rules! owner {
        ($path:literal) => {
            files.push(file($path, include_bytes!(concat!("../../../", $path))));
        };
    }
    owner!("ToS/contracts/corpus-record.schema.json");
    owner!("ToS/contracts/knowledge-assessment.schema.json");
    owner!("ToS/contracts/human-form.schema.json");
    owner!("ToS/contracts/human-form-set.schema.json");
    owner!("ToS/contracts/human-form-template.schema.json");
    owner!("mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py");
    owner!("mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py");
    owner!("mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py");
    owner!("mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py");
    owner!("mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py");
    owner!("scripts/source_record_profiles.py");
    owner!("scripts/native_text_binding.py");
    owner!("scripts/source_owner_context.py");
    owner!("scripts/source_witness_human_forms.py");
    if selected {
        owner!("scripts/source_metadata_snapshot.py");
        owner!(
            "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py"
        );
        owner!(
            "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_selected_revisions.py"
        );
    }
    CommandContext {
        base_revision: SourceRevision(Digest256::of_bytes(b"bounded-fixture-cut")),
        configuration_raw: bytes(&config),
        request_raw: bytes(&proposal()),
        recorded_at: "2026-09-26T12:00:00Z".into(),
        effective_uid: 1000,
        files,
    }
}
fn proposal() -> JsonValue {
    obj(vec![
        ("schema_version", text("tos_local_source_command_v1")),
        ("operation", text("prepare-revise")),
        (
            "fields",
            obj(vec![("preferred_label", text("Revised descriptive label"))]),
        ),
        (
            "forms",
            JsonValue::Array(vec![obj(vec![
                ("form_id", text("tos.form.revision.fixture-name")),
                ("field_id", text("metadata.preferred-name")),
            ])]),
        ),
        ("reason", text("fixture correction")),
    ])
}
fn run(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
) -> Result<tos_command::source_command::PreparedCommand, SourceCommandError> {
    run_with_cut(ctx, publication, false)
}
fn run_with_cut(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    profile_cut: bool,
) -> Result<tos_command::source_command::PreparedCommand, SourceCommandError> {
    let files = ctx
        .files
        .iter()
        .map(|f| (f.path.as_str().to_string(), f.raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let revision = super::validation_cut_cases::write_cut_store(&authored, &root);
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(120);
    let cut = open_cut(&root, revision, deadline, &cancel);
    let mut worker = schemas(&cut, deadline, &cancel);
    let mut bound = cut_context(
        &files,
        ctx.configuration_raw.clone(),
        ctx.request_raw.clone(),
        revision,
    );
    bound.recorded_at = ctx.recorded_at.clone();
    bound.effective_uid = ctx.effective_uid;
    assert!(
        cut.current()
            .members()
            .all(|member| member.path.as_str().starts_with("ToS/"))
    );
    let (_capture, software, components) = captured_components(&files, deadline, &cancel);
    if profile_cut {
        assert!(matches!(
            prepare_record_revision_with_profile_cut(
                &bound,
                publication,
                &cut,
                &mut worker,
                deadline,
                &cancel
            ),
            Err(SourceCommandError::Unsupported(_))
        ));
    }
    prepare_record_revision_from_captures(
        &bound,
        publication,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
}
pub(super) fn captured_components(
    files: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (
    super::source_cut_cases::SoftwareCaptureFixture,
    tos_source_store::SoftwareCaptureReader,
    tos_source_store::SoftwareComponentSelectionV1,
) {
    use tos_source_store::{ReadLimits, SoftwareCaptureReader};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let commit_output = std::process::Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .unwrap();
    assert!(commit_output.status.success());
    let commit = String::from_utf8(commit_output.stdout)
        .unwrap()
        .trim()
        .to_owned();
    let software_names = files
        .keys()
        .filter(|name| !name.starts_with("ToS/"))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let capture =
        super::source_cut_cases::captured_software_fixture(&repository, &commit, &software_names);
    let software = SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
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
    let paths = software_names
        .iter()
        .map(|name| RelativePath::parse(name).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    (capture, software, components)
}

#[test]
fn initial_source_packages_use_real_native_capture_and_isolated_atomic_publication() {
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::process::{Command, Stdio};
    use tos_command::source_creation::prepare_source_creation_from_captures;
    use tos_command::source_creation_store::{
        CreationDurability, CreationFilesystem, IsolatedCreationRoot,
        execute_isolated_creation_from_captures,
    };
    use tos_validation::source_cut::CutSchemaExecutor;

    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    // Existing maintained creator support law, plus the actual native buffer
    // producer sources. The capture is byte evidence; it is not a build proof.
    let inputs = [
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
        "scripts/source_witness_human_forms.py",
        "scripts/build_source_witness_catalog.py",
        "scripts/source_record_profiles.py",
        "scripts/native_text_binding.py",
        "scripts/source_owner_context.py",
        "scripts/source_witness_bibliographic_graph_common.py",
        "rust/crates/tos-command/src/source_creation.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "ToS/contracts/historical-record.schema.json",
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/historical-claim.schema.json",
        "ToS/contracts/claim-packet.schema.json",
        "ToS/contracts/knowledge-assessment.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
        "ToS/contracts/source-metadata-record.schema.json",
        "ToS/contracts/semantic-description-record.schema.json",
        "ToS/contracts/research-corpus-record.schema.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ];
    let files: BTreeMap<String, Vec<u8>> = inputs
        .iter()
        .map(|name| (name.to_string(), fs::read(repository.join(name)).unwrap()))
        .collect();
    let (_captured, software, components) = captured_components(&files, deadline, &cancellation);
    let temporary = tempfile::tempdir().unwrap();
    let authored: BTreeMap<_, _> = files
        .iter()
        .filter(|(path, _)| path.starts_with("ToS/"))
        .map(|(path, raw)| (path.clone(), raw.clone()))
        .collect();
    let store = temporary.path().join("selected-store");
    let revision = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = open_cut(&store, revision, deadline, &cancellation);

    let image = super::command_form_cases::schema_image(
        tos_validation::executor::ExecutorBudget::laboratory(),
        deadline,
        &cancellation,
    );

    for (family, kind, relative) in [
        (
            "tos_local_historical_create_owner_v2",
            "historical-event",
            "ToS/source-witnesses/history/new-subject/historical-event.json",
        ),
        (
            "tos_local_corpus_create_owner_v1",
            "work",
            "ToS/source-witnesses/works/new-subject/work.json",
        ),
        (
            "tos_local_profile_create_owner_v1",
            "research-corpus",
            "ToS/source-witnesses/research-corpora/new-subject/research-corpus.json",
        ),
    ] {
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
        for (path, bytes) in &files {
            let target = isolated.path().join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, bytes).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        }
        fs::create_dir_all(
            isolated
                .path()
                .join(relative)
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .unwrap();
        let identity = format!("tos.{kind}.synthetic-native-create");
        let mut record = serde_json::json!({"schema_version":"tos_corpus_record_v1", "record_type":kind,
            "record_id":identity,"record_version":1,"preferred_label":"Synthetic mechanics subject", "variant_labels":[],
            "identity_status":"provisional","source_refs":["synthetic-test-only:creation-not-assessment"],
            "external_identifiers":[],"same_as_posture":"no_equivalence_claim","visibility":"public_metadata_only",
            "supersedes_ref":null,"notes":"Synthetic proposed source metadata only; no historical existence, textual judgment or admission asserted.",
            "field_languages":{"preferred_label":{"language":"en","script":"Latn"},"notes":{"language":"en","script":"Latn"}}});
        if kind == "historical-event" {
            record["schema_version"] = serde_json::json!("tos_historical_record_v1");
        }
        if kind == "work" {
            record.as_object_mut().unwrap().remove("visibility");
            record["expression_claim_refs"] = serde_json::json!([]);
        }
        if kind == "research-corpus" {
            record["schema_version"] = serde_json::json!("tos_research_corpus_record_v1");
            record["semantic_scope"] = serde_json::json!({"scope_note":"Finite synthetic mechanics test only.","identity_criterion":"The same test purpose, not source assessment or membership.","language":"en","script":"Latn"});
            record["semantic_content"] = serde_json::json!({"research_purpose":"Exercise an authored metadata profile.","selection_criterion":"Only individually selected synthetic test material.","coverage_account":"No membership declared; no historical emptiness asserted.","language":"en","script":"Latn","uninterpreted":[false,null,0]});
        }
        let operation = if kind == "historical-event" {
            "historical.create"
        } else {
            "source.create"
        };
        let uid = fs::metadata(isolated.path()).unwrap().uid();
        let mut config = serde_json::json!({"schema_version":family,"uid":uid,"principal_id":"software:test-fixture", "maker_type":"software",
            "source_root":isolated.path(),"source_path":relative,"record_id":identity,
            "authority_ref":"synthetic-test-only:creation-not-assessment","allowed_form_ids":["tos.form.creation.fixture-name"],
            "allowed_operations":[operation],"expires_at":"2099-01-01T00:00:00Z"});
        config["provenance_event_id"] = serde_json::json!("tos.event.synthetic-native-create");
        if kind == "historical-event" {
            config["allowed_claim_ids"] = serde_json::json!([]);
        } else {
            if kind == "work" {
                config["record_type"] = serde_json::json!(kind);
            } else {
                config["profile_type_id"] = serde_json::json!("tos.entity.research-corpus");
            }
        }
        let config_raw = canonical_json(&config);
        let owner = isolated.path().join("owner.json");
        fs::write(&owner, &config_raw).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation)
                .unwrap();
        let mut request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create", "record":record,
            "forms":[{"form_id":"tos.form.creation.fixture-name","field_id":"metadata.preferred-name"}]});
        if kind == "historical-event" {
            request["claims"] = serde_json::json!([]);
        }
        let mut context = cut_context(
            &files,
            config_raw.clone(),
            canonical_json(&request),
            revision,
        );
        context.effective_uid = u64::from(uid);
        let mut worker =
            super::command_form_cases::schemas_with_image(&cut, &image, deadline, &cancellation);
        let prepared = prepare_source_creation_from_captures(
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        let preview = prepared.preview().unwrap();

        // Actual maintained whole prepare oracle on this same owner-selected
        // filesystem. It prepares buffers only and never writes source bytes.
        let script = "import json,sys;from pathlib import Path;repo=Path(sys.argv[1]);sys.path[:0]=[str(repo/'scripts'),str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')];import source_commands as commands;request=commands._json_object(commands._canonical(json.load(sys.stdin)));config,_,_=commands._configuration(Path(sys.argv[2]));_,files,_=commands._prepare_creation(config,request);result=commands.run_legacy_oracle_command(Path(sys.argv[2]),request);print(json.dumps({'result_raw':commands._canonical(result).hex(),'files':{name:raw.hex() for name,raw in files.items()}},ensure_ascii=False,allow_nan=False))";
        let mut oracle = Command::new(crate::maintained_python());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_")
                || key == "PYTHONPATH"
                || key == "PYTHONHOME"
            {
                oracle.env_remove(key);
            }
        }
        let mut child = oracle
            .args(["-c", script])
            .arg(&repository)
            .arg(&owner)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&canonical_json(&request))
            .unwrap();
        let oracle = child.wait_with_output().unwrap();
        assert!(
            oracle.status.success(),
            "{kind}: {}",
            String::from_utf8_lossy(&oracle.stderr)
        );
        let oracle: Value = serde_json::from_slice(&oracle.stdout).unwrap();
        assert_eq!(
            bytes(&preview),
            decode_hex(required(&oracle, "result_raw")),
            "{kind} full prepare result"
        );
        for (name, raw) in prepared.files() {
            let hex = raw
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(
                oracle["files"][name].as_str(),
                Some(hex.as_str()),
                "{kind} original output {name}"
            );
        }
        request["operation"] = serde_json::json!(operation);
        request["command_id"] = serde_json::json!("synthetic:create-first");
        request["expected_configuration"] =
            serde_json::from_slice(&bytes(preview.object_get("owner_configuration").unwrap()))
                .unwrap();
        request["expected_dependencies"] =
            serde_json::from_slice(&bytes(preview.object_get("expected_dependencies").unwrap()))
                .unwrap();
        request["expected_source"] = Value::Null;
        request["expected_revision"] = Value::Null;
        context.request_raw = canonical_json(&request);
        let prepared = prepare_source_creation_from_captures(
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        let serialized = prepared
            .serialize(&software, &components, &mut worker, deadline, &cancellation)
            .unwrap();
        worker.finish(deadline, &cancellation).unwrap();
        assert!(matches!(
            serialized.command().commit(),
            Err(SourceCommandError::MissingProductionAdmission)
        ));
        // Unchanged source and separately captured software are reselected
        // from the actual owner filesystem, rather than from public context.
        for path in [
            "ToS/contracts/human-form.schema.json",
            "rust/crates/tos-command/src/source_serialization.rs",
        ] {
            let target = isolated.path().join(path);
            let original = fs::read(&target).unwrap();
            let mut substituted = original.clone();
            substituted.push(b'\n');
            fs::write(&target, substituted).unwrap();
            assert!(
                matches!(
                    filesystem.publish_isolated(
                        &serialized,
                        &cut,
                        &software,
                        &components,
                        deadline,
                        &cancellation
                    ),
                    Err(SourceCommandError::Conflict(_))
                ),
                "{kind} unchanged dependency {path}"
            );
            fs::write(&target, original).unwrap();
        }
        let incomplete_components = software
            .select_components(&[RelativePath::parse(
                "rust/crates/tos-command/src/source_serialization.rs",
            )
            .unwrap()])
            .unwrap();
        assert!(matches!(
            filesystem.publish_isolated(
                &serialized,
                &cut,
                &software,
                &incomplete_components,
                deadline,
                &cancellation
            ),
            Err(SourceCommandError::Conflict(_))
        ));
        let stopped = AtomicBool::new(true);
        assert!(matches!(
            filesystem.publish_isolated(
                &serialized,
                &cut,
                &software,
                &components,
                deadline,
                &stopped
            ),
            Err(SourceCommandError::Denied(_))
        ));
        assert!(matches!(
            filesystem.publish_isolated(
                &serialized,
                &cut,
                &software,
                &components,
                Instant::now(),
                &cancellation
            ),
            Err(SourceCommandError::Denied(_))
        ));
        let home = isolated.path().join(relative).parent().unwrap().to_owned();
        // An empty competing directory must survive the genuine NOREPLACE gate.
        fs::create_dir(&home).unwrap();
        assert!(matches!(
            filesystem.publish_isolated(
                &serialized,
                &cut,
                &software,
                &components,
                deadline,
                &cancellation
            ),
            Err(SourceCommandError::Conflict(_))
        ));
        assert!(home.is_dir() && fs::read_dir(&home).unwrap().next().is_none());
        fs::remove_dir(&home).unwrap(); // this test's exact empty competitor only
        // The successful operation must traverse the production whole entry.
        // It makes its own time/runtime capture, so the earlier serialized
        // package remains the independent negative oracle rather than a byte
        // substitute for this newly published package.
        let mut operation_worker =
            super::command_form_cases::schemas_with_image(&cut, &image, deadline, &cancellation);
        let (executed, published, published_result) = execute_isolated_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &software,
            &components,
            &mut operation_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        assert!(!published.replayed);
        assert_eq!(published.durability, CreationDurability::DirectoriesSynced);
        assert_eq!(
            published_result.object_get("target_exists"),
            Some(&JsonValue::Bool(true))
        );
        assert_eq!(
            published_result.object_get("replayed"),
            Some(&JsonValue::Bool(false))
        );
        assert_eq!(
            published_result.object_get("grants_admission"),
            Some(&JsonValue::Bool(false))
        );
        assert_eq!(
            published_result.object_get("receipt"),
            executed.command().response.object_get("receipt")
        );
        assert_eq!(
            published_result.object_get("record_id"),
            preview.object_get("record_id")
        );
        for (name, hex) in oracle["files"].as_object().unwrap() {
            let original = decode_hex(hex.as_str().unwrap());
            assert_eq!(
                executed.prepared().files().get(name),
                Some(&original),
                "{kind} executed original output {name}"
            );
            assert_eq!(
                serialized.prepared().files().get(name),
                Some(&original),
                "{kind} independent original output {name}"
            );
        }
        for (name, bytes) in executed.prepared().files() {
            assert_eq!(fs::read(home.join(name)).unwrap(), *bytes);
        }
        assert_eq!(
            fs::read_dir(&home).unwrap().count(),
            executed.prepared().files().len()
        );
        assert_eq!(
            published.receipt_sha256,
            Digest256::of_bytes(&executed.prepared().files()["source-create-receipt.json"])
        );
        let replay = filesystem
            .replay_isolated(
                &executed,
                &cut,
                &software,
                &components,
                deadline,
                &cancellation,
            )
            .unwrap();
        assert!(replay.replayed && replay.receipt_sha256 == published.receipt_sha256);
        // A separately captured clock/runtime package cannot replay an exact
        // different publication. If both captures happened to be byte-equal,
        // the positive replay above already covers that package.
        if serialized.prepared().files() != executed.prepared().files() {
            assert!(matches!(
                filesystem.replay_isolated(
                    &serialized,
                    &cut,
                    &software,
                    &components,
                    deadline,
                    &cancellation
                ),
                Err(SourceCommandError::Conflict(_))
            ));
        }
        let mut revoked = config.clone();
        revoked["allowed_operations"] = serde_json::json!([]);
        fs::write(&owner, canonical_json(&revoked)).unwrap();
        assert!(matches!(
            filesystem.replay_isolated(
                &executed,
                &cut,
                &software,
                &components,
                deadline,
                &cancellation
            ),
            Err(SourceCommandError::Conflict(_))
        ));
        assert!(
            fs::read_dir(isolated.path().join("ToS"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".source-create-"))
        );
    }
}

fn retain_transport(
    ctx: &mut CommandContext,
    transaction: &RetainedRevisionTransaction,
    committed: bool,
) -> RevisionPublication {
    let mut members = BTreeMap::new();
    for file in &transaction.before {
        members
            .entry(file.path.as_str().to_string())
            .or_insert_with(|| (None, None))
            .0 = Some(file.raw.clone());
    }
    for file in &transaction.after {
        members
            .entry(file.path.as_str().to_string())
            .or_insert_with(|| (None, None))
            .1 = Some(file.raw.clone());
    }
    let directory = format!(
        "ToS/source-witnesses/.metadata-transactions/{}",
        &transaction.transaction_id[7..]
    );
    let mut selected = Vec::new();
    let mut parents = BTreeMap::new();
    for (path, (before, after)) in members {
        let mut item = obj(vec![("path", text(&path))]);
        for (side, raw) in [("before", before), ("after", after)] {
            let binding = if let Some(raw) = raw {
                let digest = Digest256::of_bytes(&raw);
                let blob = format!("{directory}/{}.blob", digest.to_hex());
                ctx.files.retain(|f| f.path.as_str() != blob);
                ctx.files.push(file(&blob, &raw));
                obj(vec![
                    ("sha256", text(&digest.to_prefixed())),
                    ("bytes", parse(raw.len().to_string().as_bytes())),
                ])
            } else {
                JsonValue::Null
            };
            set(&mut item, side, binding);
        }
        selected.push(item);
        let mut parent = path.rsplit_once('/').unwrap().0;
        loop {
            parents.insert(
                parent.to_string(),
                obj(vec![
                    ("device", parse(b"0")),
                    ("inode", parse(b"1")),
                    ("mode", parse(b"16832")),
                    ("uid", parse(b"1000")),
                ]),
            );
            if parent == "ToS/source-witnesses" {
                break;
            }
            parent = parent.rsplit_once('/').unwrap().0;
        }
    }
    let manifest = obj(vec![
        (
            "schema_version",
            text("tos_selected_metadata_transaction_v1"),
        ),
        ("transaction_id", text(&transaction.transaction_id)),
        (
            "base_publication",
            obj(vec![
                ("token", JsonValue::Null),
                ("generation", parse(b"0")),
            ]),
        ),
        (
            "plan",
            obj(vec![
                ("authorization", parse(&transaction.authorization_raw)),
                ("files", JsonValue::Array(selected)),
                ("new_directories", JsonValue::Array(vec![])),
            ]),
        ),
        (
            "parents",
            JsonValue::Object(
                parents
                    .into_iter()
                    .map(|(k, v)| (JsonString::from_utf8(&k), v))
                    .collect(),
            ),
        ),
    ]);
    let mut manifest_raw = bytes(&manifest);
    manifest_raw.push(b'\n');
    let digest = Digest256::of_bytes(&manifest_raw).to_prefixed();
    let manifest_path = format!("{directory}/manifest.json");
    ctx.files.retain(|f| f.path.as_str() != manifest_path);
    ctx.files.push(file(&manifest_path, &manifest_raw));
    let mut state = obj(vec![
        ("schema_version", text("tos_source_metadata_publication_v1")),
        ("generation", parse(if committed { b"2" } else { b"1" })),
        ("transition_id", text("00000000000000000000000000000000")),
        ("phase", text(if committed { "ready" } else { "pending" })),
        ("transaction_id", text(&transaction.transaction_id)),
        ("manifest_sha256", text(&digest)),
        (
            "outcome",
            if committed {
                text("committed")
            } else {
                JsonValue::Null
            },
        ),
        ("recovery_authorization", JsonValue::Null),
    ]);
    let token = Digest256::of_bytes(&bytes(&state)).to_prefixed();
    set(&mut state, "token", text(&token));
    let control = "ToS/source-witnesses/.metadata-publication.json";
    ctx.files.retain(|f| f.path.as_str() != control);
    ctx.files.push(file(control, &bytes(&state)));
    if committed {
        ctx.files.push(file(
            &format!("{directory}/completion.json"),
            &bytes(&obj(vec![
                (
                    "schema_version",
                    text("tos_selected_metadata_completion_v1"),
                ),
                ("publication", state),
            ])),
        ));
    }
    read_record_revision_publication(ctx, &[&transaction.transaction_id]).unwrap()
}
fn apply_request(ctx: &mut CommandContext, publication: Option<&RevisionPublication>) -> JsonValue {
    let prepared = run(ctx, publication).unwrap();
    assert!(prepared.changes.is_empty());
    let mut request = proposal();
    set(&mut request, "operation", text("record.revise"));
    set(&mut request, "command_id", text("fixture-revision-1"));
    for (dest, src) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
        ("expected_dependencies", "expected_dependencies"),
    ] {
        set(
            &mut request,
            dest,
            prepared.response.object_get(src).unwrap().clone(),
        );
    }
    if publication.is_some() {
        set(
            &mut request,
            "expected_publication",
            prepared
                .response
                .object_get("expected_publication")
                .unwrap()
                .clone(),
        );
    }
    ctx.request_raw = bytes(&request);
    request
}
fn apply_proposed(ctx: &mut CommandContext, changes: &[tos_command::source_command::SourceChange]) {
    for change in changes {
        ctx.files.retain(|f| f.path != change.path);
        if let Some(raw) = &change.after {
            ctx.files.push(SourceFile {
                path: change.path.clone(),
                raw: raw.clone(),
            });
        }
    }
}
#[test]
fn flat_whole_successor_bytes_replay_inspection_and_retained_fixity() {
    let mut ctx = context(false);
    let request = apply_request(&mut ctx, None);
    let prepared = run(&ctx, None).unwrap();
    assert_eq!(
        prepared.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    let changed = |suffix: &str| {
        prepared
            .changes
            .iter()
            .find(|c| c.path.as_str().ends_with(suffix))
            .unwrap()
            .after
            .as_ref()
            .unwrap()
    };
    assert_eq!(
        Digest256::of_bytes(changed("/work.json")).to_hex(),
        "cf940bd44717ea12e65cbf2927a5d3afd6350c73f8e0c10fba4bf88233510613"
    );
    assert_eq!(
        Digest256::of_bytes(changed("/work.human-forms.json")).to_hex(),
        "a4086c6079086cbbd41c20bf37a421daa355725ce782c0963a09dfe89a7bd363"
    );
    assert_eq!(prepared.changes.len(), 5); // three current members + blob + manifest
    let mut python_whitespace = proposal();
    set(&mut python_whitespace, "reason", text("\u{001c}\u{001f}"));
    let mut whitespace_ctx = ctx.clone();
    whitespace_ctx.request_raw = bytes(&python_whitespace);
    assert!(matches!(
        run(&whitespace_ctx, None),
        Err(SourceCommandError::Invalid(_))
    ));
    apply_proposed(&mut ctx, &prepared.changes);
    let replay = run(&ctx, None).unwrap();
    assert!(replay.replayed);
    assert!(replay.changes.is_empty());
    ctx.request_raw = bytes(&obj(vec![
        ("schema_version", text("tos_local_source_command_v1")),
        ("operation", text("inspect-version")),
        (
            "source",
            request.object_get("expected_source").unwrap().clone(),
        ),
    ]));
    let inspected = run(&ctx, None).unwrap();
    assert_eq!(
        inspected.response.object_get("record"),
        Some(&parse(SOURCE))
    );
    let archive = ctx
        .files
        .iter_mut()
        .find(|f| f.path.as_str().ends_with(".blob"))
        .unwrap();
    archive.raw.push(b' ');
    assert!(matches!(
        run(&ctx, None),
        Err(SourceCommandError::Conflict(_))
    ));
}
#[test]
fn current_account_expiry_scope_and_selected_exact_recovery_are_independent() {
    let mut ctx = context(true);
    let initial_publication = RevisionPublication::default();
    let request = apply_request(&mut ctx, Some(&initial_publication));
    let prepared = run(&ctx, Some(&initial_publication)).unwrap();
    let receipt = prepared.response.object_get("receipt").unwrap();
    let publication = receipt.object_get("publication").unwrap();
    let transaction_id = publication
        .object_get("transaction_id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let config = parse(&ctx.configuration_raw);
    let authorization = obj(vec![
        (
            "schema_version",
            text("tos_selected_metadata_revision_authorization_v1"),
        ),
        (
            "principal_id",
            config.object_get("principal_id").unwrap().clone(),
        ),
        (
            "authority_ref",
            config.object_get("authority_ref").unwrap().clone(),
        ),
        ("source_path", text(SOURCE_PATH)),
        ("record_id", config.object_get("record_id").unwrap().clone()),
        ("record_type", text("work")),
        ("request", request.clone()),
    ]);
    let after = prepared
        .changes
        .iter()
        .filter(|c| {
            c.path
                .as_str()
                .starts_with("ToS/source-witnesses/works/fixture/")
        })
        .map(|c| SourceFile {
            path: c.path.clone(),
            raw: c.after.clone().unwrap(),
        })
        .collect();
    let transaction = RetainedRevisionTransaction {
        transaction_id: transaction_id.clone(),
        status: RevisionTransactionStatus::Pending,
        authorization_raw: bytes(&authorization),
        before: vec![file(SOURCE_PATH, SOURCE)],
        after,
    };
    // Retain the archive and only one successor member to model interruption.
    let partial = prepared
        .changes
        .iter()
        .filter(|c| {
            c.path.as_str().contains("/.record-revisions/") || c.path.as_str() == SOURCE_PATH
        })
        .cloned()
        .collect::<Vec<_>>();
    apply_proposed(&mut ctx, &partial);
    let mut publication = retain_transport(&mut ctx, &transaction, false);
    ctx.request_raw = bytes(&obj(vec![
        ("schema_version", text("tos_local_source_command_v1")),
        ("operation", text("record.recover")),
        ("transaction_id", text(&transaction_id)),
        ("decision", text("rollback")),
        (
            "expected_configuration",
            request
                .object_get("expected_configuration")
                .unwrap()
                .clone(),
        ),
    ]));
    let rollback = run(&ctx, Some(&publication)).unwrap();
    assert_eq!(rollback.changes.len(), 3);
    assert_eq!(
        rollback.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    assert_eq!(
        rollback
            .changes
            .iter()
            .find(|c| c.path.as_str() == SOURCE_PATH)
            .unwrap()
            .after
            .as_deref(),
        Some(SOURCE)
    );
    assert_eq!(
        rollback
            .changes
            .iter()
            .filter(|c| c.after.is_none())
            .count(),
        2
    );
    ctx.effective_uid = 1001;
    assert!(matches!(
        run(&ctx, Some(&publication)),
        Err(SourceCommandError::Denied(_))
    ));
    ctx.effective_uid = 1000;
    ctx.recorded_at = "2031-01-01T00:00:00Z".into();
    assert!(matches!(
        run(&ctx, Some(&publication)),
        Err(SourceCommandError::Denied(_))
    ));
    ctx.recorded_at = "2026-09-26T12:00:00Z".into();
    // Exact normal retry resumes the original proposal; committed replay must
    // independently reconstruct the retained publication and predecessor.
    ctx.request_raw = bytes(&request);
    let resume = run(&ctx, Some(&publication)).unwrap();
    apply_proposed(&mut ctx, &resume.changes);
    publication = retain_transport(&mut ctx, &transaction, true);
    assert!(run(&ctx, Some(&publication)).unwrap().replayed);
    let mut revoked = config;
    set(&mut revoked, "allowed_operations", arr(&["record.recover"]));
    ctx.configuration_raw = bytes(&revoked);
    assert!(matches!(
        run(&ctx, Some(&publication)),
        Err(SourceCommandError::Denied(_))
    ));
}

// A complete cut, rather than the proposal's selected file list, owns native
// namespace absence. Reuse the actual worker harness and maintained registry.
fn profile_context() -> CommandContext {
    let mut ctx = context(false);
    let source_path = "ToS/source-witnesses/research/fixture/lexeme.json";
    let record = obj(vec![
        ("schema_version", text("tos_lexical_description_record_v1")),
        ("record_type", text("lexeme")),
        ("record_id", text("tos.lexeme.revision.fixture")),
        ("record_version", parse(b"1")),
        ("preferred_label", text("Fixture lexeme")),
        ("identity_status", text("provisional")),
        (
            "source_refs",
            arr(&["ToS/contracts/lexical-description-record.schema.json"]),
        ),
        ("external_identifiers", JsonValue::Array(vec![])),
        ("same_as_posture", text("no_equivalence_claim")),
        ("visibility", text("public_metadata_only")),
        (
            "notes",
            text("Synthetic description of a lexical referent."),
        ),
        (
            "field_languages",
            obj(vec![
                (
                    "preferred_label",
                    obj(vec![("language", text("en")), ("script", JsonValue::Null)]),
                ),
                (
                    "notes",
                    obj(vec![("language", text("en")), ("script", JsonValue::Null)]),
                ),
            ]),
        ),
        (
            "semantic_scope",
            obj(vec![
                ("scope_note", text("Synthetic fixture.")),
                (
                    "identity_criterion",
                    text("One synthetic lexical referent."),
                ),
                ("language", text("en")),
                ("script", JsonValue::Null),
            ]),
        ),
        (
            "semantic_content",
            obj(vec![
                ("lexical_account", text("Synthetic lexical grouping.")),
                ("grammatical_account", text("Grammar remains unknown.")),
                ("language", text("en")),
                ("script", JsonValue::Null),
            ]),
        ),
    ]);
    ctx.files.retain(|f| f.path.as_str() != SOURCE_PATH);
    ctx.files.push(file(source_path, &bytes(&record)));
    macro_rules! owner {
        ($path:literal) => {
            ctx.files
                .push(file($path, include_bytes!(concat!("../../../", $path))));
        };
    }
    owner!("ToS/doctrine/semantic-interchange/entity-types.v1.json");
    owner!("ToS/contracts/semantic-entity-type-registry.schema.json");
    owner!("ToS/contracts/source-metadata-record.schema.json");
    owner!("ToS/contracts/semantic-description-record.schema.json");
    owner!("ToS/contracts/lexical-description-record.schema.json");
    owner!("ToS/contracts/semantic-annotation-packet-v2.schema.json");
    let mut config = parse(&ctx.configuration_raw);
    let JsonValue::Object(fields) = &mut config else {
        panic!("configuration")
    };
    fields.retain(|(name, _)| name.as_str() != Some("record_type"));
    set(
        &mut config,
        "schema_version",
        text("tos_local_profile_revision_owner_v1"),
    );
    set(&mut config, "profile_type_id", text("tos.entity.lexeme"));
    set(&mut config, "source_path", text(source_path));
    set(
        &mut config,
        "record_id",
        text("tos.lexeme.revision.fixture"),
    );
    ctx.configuration_raw = bytes(&config);
    ctx
}

#[test]
fn profile_native_inventory_uses_anchored_membership() {
    let mut ctx = profile_context();
    let packet_path = "ToS/source-witnesses/research/fixture/semantic-annotation.fixture.json";
    let packet_raw = include_bytes!(
        "../../fixtures/native-text-binding/semantic-annotation-v2-abc/variant-a-occurrences-only.json"
    );
    ctx.files.push(file(packet_path, packet_raw));
    let files = ctx
        .files
        .iter()
        .map(|f| (f.path.as_str().to_string(), f.raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let revision = super::validation_cut_cases::write_cut_store(&authored, &root);
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(120);
    let cut = open_cut(&root, revision, deadline, &cancel);
    let mut worker = schemas(&cut, deadline, &cancel);
    let (_capture, software, components) = captured_components(&files, deadline, &cancel);
    let mut bound = cut_context(
        &files,
        ctx.configuration_raw.clone(),
        ctx.request_raw.clone(),
        revision,
    );
    bound.recorded_at = ctx.recorded_at.clone();
    bound.effective_uid = ctx.effective_uid;
    assert!(matches!(
        prepare_record_revision(&bound, None, &mut worker, deadline, &cancel),
        Err(SourceCommandError::Unsupported(_))
    ));
    let prepared = prepare_record_revision_from_captures(
        &bound,
        None,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    assert!(
        prepared
            .reads
            .iter()
            .any(|input| input.path.as_str() == packet_path)
    );
    let mut omitted = bound.clone();
    omitted.files.retain(|f| f.path.as_str() != packet_path);
    assert!(matches!(
        prepare_record_revision_from_captures(
            &omitted,
            None,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Unsupported(_))
    ));
    let mut changed = bound.clone();
    changed
        .files
        .iter_mut()
        .find(|f| f.path.as_str() == packet_path)
        .unwrap()
        .raw
        .push(b' ');
    assert!(matches!(
        prepare_record_revision_from_captures(
            &changed,
            None,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
}

fn nested(value: &mut JsonValue, keys: &[&str], replacement: JsonValue) {
    if keys.len() == 1 {
        set(value, keys[0], replacement);
        return;
    }
    let JsonValue::Object(fields) = value else {
        panic!("nested fixture object")
    };
    let (_, child) = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(keys[0]))
        .unwrap();
    nested(child, &keys[1..], replacement);
}

// Rebind the existing public laboratory skeletons to one synthetic source home.
// Neither the original payload nor the text representation is included in this
// metadata-only cut. The fixture's recorded rights gate has no real authority.
fn native_profile_context() -> CommandContext {
    let mut ctx = profile_context();
    let home = "ToS/source-witnesses/research/fixture";
    let packet_path = format!("{home}/source-text-unit.fixture.json");
    let layer_path = format!("{home}/source-text-layer.fixture.json");
    let anchor_path = format!("{home}/source-anchor-v2.fixture.json");
    let manifest_path = format!("{home}/item.manifest.json");
    let rights_path = format!("{home}/rights.json");
    let policy_path = format!("{home}/policy.json");
    let authority_path = format!("{home}/authority.json");
    let content_path = format!("{home}/public-synthetic-content.txt");
    let notice = "Synthetic fixture only; no linguistic or rights judgment.";
    let support = bytes(&obj(vec![("notice", text(notice))]));
    ctx.files.push(file(&policy_path, &support));
    ctx.files.push(file(&authority_path, &support));
    let support_digest = Digest256::of_bytes(&support).to_hex();
    let mut packet = parse(include_bytes!(
        "../../fixtures/native-text-binding/source-text-unit-v1-abc/variant-a-source-layout-observation.json"
    ));
    let mut layer = parse(include_bytes!(
        "../../fixtures/native-text-binding/source-text-layer-abc/variant-a.layer.json"
    ));
    let mut anchor = parse(include_bytes!(
        "../../fixtures/native-text-binding/source-anchor-v2-abc/variant-b.anchor.json"
    ));
    let source_binding = layer.object_get("source_binding").unwrap().clone();
    // The three borrowed metadata packets are separate synthetic exercises.
    // This explicitly constructed joint fixture uses the layer's existing
    // unknown-language declaration and exact source item/file binding. It
    // does not alter owner schemas or claim actual content/rights admission.
    nested(
        &mut packet,
        &["source_layer", "language"],
        layer
            .object_get("representation")
            .unwrap()
            .object_get("language")
            .unwrap()
            .clone(),
    );
    nested(
        &mut anchor,
        &["target", "item_id"],
        source_binding.object_get("item_ref").unwrap().clone(),
    );
    assert_eq!(
        anchor.object_get("target").unwrap().object_get("file_id"),
        source_binding.object_get("source_file_ref")
    );
    assert_eq!(
        anchor
            .object_get("target")
            .unwrap()
            .object_get("file_sha256"),
        source_binding.object_get("source_file_sha256")
    );
    let content_sha = packet
        .object_get("source_layer")
        .unwrap()
        .object_get("text_layer_sha256")
        .unwrap()
        .clone();
    let content_sha_text = content_sha.as_str().unwrap();
    let mut scope_fields = vec![];
    let mut source_records = vec![];
    for kind in ["work", "expression", "edition", "item"] {
        let key = format!("{kind}_ref");
        let id = source_binding.object_get(&key).unwrap().clone();
        scope_fields.push((JsonString::from_utf8(&key), id.clone()));
        let record_path = format!("{home}/{kind}.json");
        source_records.push((JsonString::from_utf8(kind), text(&record_path)));
        let mut record = obj(vec![
            ("schema_version", text("tos_corpus_record_v1")),
            ("record_type", text(kind)),
            ("record_id", id),
            ("preferred_label", text(notice)),
            ("identity_status", text("provisional")),
            ("source_refs", arr(&[&policy_path])),
            ("external_identifiers", JsonValue::Array(vec![])),
            ("same_as_posture", text("no_equivalence_claim")),
            ("record_version", parse(b"1")),
            ("notes", text(notice)),
        ]);
        match kind {
            "work" => set(
                &mut record,
                "expression_claim_refs",
                JsonValue::Array(vec![]),
            ),
            "expression" => {
                set(
                    &mut record,
                    "work_ref",
                    source_binding.object_get("work_ref").unwrap().clone(),
                );
                set(&mut record, "language", text("und"));
                set(&mut record, "expression_role", text("source_language"));
                set(
                    &mut record,
                    "responsibility_claim_refs",
                    JsonValue::Array(vec![]),
                );
                set(
                    &mut record,
                    "embodiment_claim_refs",
                    JsonValue::Array(vec![]),
                );
            }
            "edition" => {
                set(
                    &mut record,
                    "embodies_expression_refs",
                    JsonValue::Array(vec![
                        source_binding.object_get("expression_ref").unwrap().clone(),
                    ]),
                );
                set(
                    &mut record,
                    "publication_claim_refs",
                    JsonValue::Array(vec![]),
                );
                set(&mut record, "exemplar_claim_refs", JsonValue::Array(vec![]));
            }
            "item" => set(&mut record, "item_manifest_ref", text(&manifest_path)),
            _ => unreachable!(),
        }
        ctx.files.push(file(&record_path, &bytes(&record)));
    }
    let original_id = source_binding
        .object_get("source_file_ref")
        .unwrap()
        .clone();
    let original_sha = source_binding
        .object_get("source_file_sha256")
        .unwrap()
        .clone();
    scope_fields.push((JsonString::from_utf8("file_ref"), original_id.clone()));
    scope_fields.push((JsonString::from_utf8("file_sha256"), original_sha.clone()));
    set(&mut packet, "source_scope", JsonValue::Object(scope_fields));
    set(&mut packet, "content_posture", text("source_bound"));
    // Follow the maintained NativeTextBindingFixture's synthetic native
    // proposal posture (tests/test_native_text_binding.py), rather than
    // promoting the laboratory's synthetic maker or source-attested units.
    // These declarations describe this fixture constructor, not a retained
    // production producer, content verification or source assessment.
    let mut schemes = packet
        .object_get("schemes")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    let scheme = &mut schemes[0];
    nested(scheme, &["method", "maker_kind"], text("software"));
    nested(
        scheme,
        &["method", "agent_ref"],
        text("software:synthetic-test-fixture"),
    );
    nested(scheme, &["method", "method_name"], text(notice));
    nested(scheme, &["method", "configuration_ref"], text(&policy_path));
    nested(scheme, &["method", "locale"], text("und"));
    nested(scheme, &["method", "software_refs"], arr(&[&policy_path]));
    let method = scheme.object_get("method").unwrap().clone();
    set(&mut packet, "schemes", JsonValue::Array(schemes));
    let mut units = packet
        .object_get("units")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    for unit in &mut units {
        set(unit, "boundary_posture", text("method_proposed"));
        set(unit, "status_reason", text(notice));
    }
    set(&mut packet, "units", JsonValue::Array(units));
    let mut segmentations = packet
        .object_get("segmentations")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    for segmentation in &mut segmentations {
        set(segmentation, "status", text("proposed"));
        set(segmentation, "status_reason", text(notice));
        set(segmentation, "maker", method.clone());
    }
    set(
        &mut packet,
        "segmentations",
        JsonValue::Array(segmentations),
    );
    nested(
        &mut packet,
        &["source_layer", "text_layer_ref"],
        text(&layer_path),
    );
    let anchors = packet
        .object_get("anchors")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    let mut rebound = vec![];
    let mut end = 0;
    for mut row in anchors {
        end = end.max(
            row.object_get("selector")
                .unwrap()
                .object_get("end")
                .unwrap()
                .as_u64()
                .unwrap(),
        );
        set(&mut row, "text_layer_ref", text(&layer_path));
        nested(
            &mut row,
            &["source_return", "locator_ref"],
            text(&content_path),
        );
        rebound.push(row);
    }
    set(&mut packet, "anchors", JsonValue::Array(rebound));
    nested(
        &mut packet,
        &["rights_and_visibility", "rights_record_refs"],
        arr(&[&rights_path]),
    );
    nested(
        &mut layer,
        &["representation", "content_file_id"],
        text(&format!("tos.file.sha256.{content_sha_text}")),
    );
    nested(
        &mut layer,
        &["representation", "content_sha256"],
        content_sha.clone(),
    );
    nested(
        &mut layer,
        &["representation", "content_ref"],
        text(&content_path),
    );
    nested(
        &mut layer,
        &["representation", "language"],
        packet
            .object_get("source_layer")
            .unwrap()
            .object_get("language")
            .unwrap()
            .clone(),
    );
    nested(
        &mut layer,
        &["representation", "text_scope"],
        obj(vec![
            ("start", parse(b"0")),
            ("end", parse(end.to_string().as_bytes())),
            ("position_unit", text("unicode_code_point")),
            ("interval", text("half_open")),
        ]),
    );
    nested(
        &mut layer,
        &["representation", "publication_authorized"],
        JsonValue::Bool(true),
    );
    nested(
        &mut layer,
        &["representation", "publication_authority_refs"],
        JsonValue::Array(vec![obj(vec![
            ("ref", text(&authority_path)),
            ("sha256", text(&support_digest)),
        ])]),
    );
    nested(
        &mut layer,
        &["editorial_policy", "policy_ref"],
        text(&policy_path),
    );
    nested(
        &mut layer,
        &["editorial_policy", "policy_sha256"],
        text(&support_digest),
    );
    nested(
        &mut layer,
        &["derivation", "maker", "configuration_ref"],
        text(&policy_path),
    );
    nested(
        &mut layer,
        &["derivation", "maker", "configuration_digest"],
        text(&support_digest),
    );
    let anchor_raw = bytes(&anchor);
    nested(
        &mut layer,
        &["source_binding", "anchors"],
        JsonValue::Array(vec![obj(vec![
            ("anchor_id", anchor.object_get("anchor_id").unwrap().clone()),
            ("anchor_record_ref", text(&anchor_path)),
            (
                "anchor_record_sha256",
                text(&Digest256::of_bytes(&anchor_raw).to_hex()),
            ),
        ])]),
    );
    ctx.files.push(file(&anchor_path, &anchor_raw));
    let rights = obj(vec![
        ("schema_version", text("tos_rights_record_v1")),
        ("rights_id", text("tos.rights.revision.fixture")),
        (
            "scope_refs",
            JsonValue::Array(vec![
                source_binding.object_get("item_ref").unwrap().clone(),
                original_id.clone(),
            ]),
        ),
        ("assessment_status", text("licensed")),
        ("jurisdictions_reviewed", JsonValue::Array(vec![])),
        ("source_refs", arr(&[&policy_path])),
        ("permissions", JsonValue::Array(vec![])),
        ("restrictions", arr(&[notice])),
        ("visibility", text("public_payload")),
        ("redistribution_posture", text("authorized")),
        ("derivative_posture", text("allowed")),
        (
            "assessed_by",
            obj(vec![
                ("maker_type", text("model")),
                ("agent_ref", text("model:synthetic-fixture")),
            ]),
        ),
        ("assessed_at", text("2026-09-08T00:00:00Z")),
        ("rationale", text(notice)),
        ("review_status", text("unreviewed")),
        ("record_version", parse(b"1")),
    ]);
    let rights_raw = bytes(&rights);
    nested(
        &mut layer,
        &["representation", "rights_record_refs"],
        JsonValue::Array(vec![obj(vec![
            ("ref", text(&rights_path)),
            ("sha256", text(&Digest256::of_bytes(&rights_raw).to_hex())),
        ])]),
    );
    ctx.files.push(file(&rights_path, &rights_raw));
    let manifest = obj(vec![
        ("schema_version", text("tos_source_item_manifest_v1")),
        (
            "item_id",
            source_binding.object_get("item_ref").unwrap().clone(),
        ),
        ("item_kind", text("born_digital")),
        (
            "embodiment_ref",
            source_binding.object_get("edition_ref").unwrap().clone(),
        ),
        ("storage_posture", text("local_gitignored_payload")),
        (
            "payload_files",
            JsonValue::Array(vec![obj(vec![
                ("file_id", original_id),
                ("relative_path", text("payload/synthetic-original.txt")),
                ("original_basename", text("synthetic-original.txt")),
                ("media_type", text("text/plain")),
                ("byte_size", parse(b"1")),
                ("sha256", original_sha),
                ("fixity_verified_at", text("2026-09-08T00:00:00Z")),
            ])]),
        ),
        ("acquisition_event_ref", text("tos.event.revision.fixture")),
        ("rights_ref", text(&rights_path)),
        ("provenance_ref", text(&policy_path)),
        ("forensic_report_ref", text(&policy_path)),
        ("resource_inventory_ref", text(&policy_path)),
        ("visibility", text("public_payload")),
        ("manifest_version", parse(b"1")),
    ]);
    ctx.files.push(file(&manifest_path, &bytes(&manifest)));
    let layer_raw = bytes(&layer);
    let packet_raw = bytes(&packet);
    let unit = &packet.object_get("units").unwrap().as_array().unwrap()[0];
    let segment = &packet
        .object_get("segmentations")
        .unwrap()
        .as_array()
        .unwrap()[0];
    let binding = obj(vec![
        ("schema_version", text("tos_native_text_unit_binding_v1")),
        ("packet_ref", text(&packet_path)),
        (
            "packet_sha256",
            text(&Digest256::of_bytes(&packet_raw).to_hex()),
        ),
        ("packet_id", packet.object_get("packet_id").unwrap().clone()),
        (
            "packet_version",
            packet.object_get("packet_version").unwrap().clone(),
        ),
        ("unit_id", unit.object_get("unit_id").unwrap().clone()),
        (
            "unit_version",
            unit.object_get("unit_version").unwrap().clone(),
        ),
        (
            "ordered_anchor_refs",
            unit.object_get("ordered_anchor_refs").unwrap().clone(),
        ),
        (
            "segmentation_id",
            segment.object_get("segmentation_id").unwrap().clone(),
        ),
        (
            "segmentation_version",
            segment.object_get("segmentation_version").unwrap().clone(),
        ),
        (
            "text_layer",
            obj(vec![
                ("record_ref", text(&layer_path)),
                (
                    "record_sha256",
                    text(&Digest256::of_bytes(&layer_raw).to_hex()),
                ),
                ("layer_id", layer.object_get("layer_id").unwrap().clone()),
                (
                    "layer_version",
                    layer.object_get("layer_version").unwrap().clone(),
                ),
            ]),
        ),
        ("source_record_refs", JsonValue::Object(source_records)),
    ]);
    ctx.files.push(file(&packet_path, &packet_raw));
    ctx.files.push(file(&layer_path, &layer_raw));
    let old = ctx
        .files
        .iter()
        .find(|f| f.path.as_str().ends_with("/lexeme.json"))
        .unwrap();
    let mut record = parse(&old.raw);
    set(
        &mut record,
        "schema_version",
        text("tos_occurrence_description_record_v1"),
    );
    set(&mut record, "record_type", text("occurrence"));
    set(
        &mut record,
        "record_id",
        text("tos.occurrence.revision.fixture"),
    );
    set(
        &mut record,
        "semantic_content",
        obj(vec![
            ("occurrence_account", text(notice)),
            ("context_account", text(notice)),
            ("language", text("en")),
            ("script", JsonValue::Null),
        ]),
    );
    set(&mut record, "native_text_binding", binding);
    ctx.files
        .retain(|f| !f.path.as_str().ends_with("/lexeme.json"));
    ctx.files
        .push(file(&format!("{home}/occurrence.json"), &bytes(&record)));
    let mut config = parse(&ctx.configuration_raw);
    set(
        &mut config,
        "source_path",
        text(&format!("{home}/occurrence.json")),
    );
    set(
        &mut config,
        "record_id",
        text("tos.occurrence.revision.fixture"),
    );
    set(
        &mut config,
        "profile_type_id",
        text("tos.entity.occurrence"),
    );
    ctx.configuration_raw = bytes(&config);
    macro_rules! owner {
        ($path:literal) => {
            ctx.files
                .push(file($path, include_bytes!(concat!("../../../", $path))));
        };
    }
    owner!("ToS/contracts/native-text-unit-binding.schema.json");
    owner!("ToS/contracts/occurrence-description-record.schema.json");
    owner!("ToS/contracts/source-text-unit-packet-v1.schema.json");
    owner!("ToS/contracts/source-text-layer.schema.json");
    owner!("ToS/contracts/source-anchor-v2.schema.json");
    owner!("ToS/contracts/rights-record.schema.json");
    owner!("ToS/contracts/source-item-manifest.schema.json");
    ctx
}

#[test]
fn profile_native_binding_checks_metadata_closure_without_content_read() {
    use tos_validation::text_metadata_rules::{
        TextMetadataLimits, TextMetadataState, inspect_source_anchor_v2_metadata,
        inspect_source_text_layer_metadata, inspect_source_text_unit_v1_metadata,
    };
    let ctx = native_profile_context();
    assert!(
        !ctx.files
            .iter()
            .any(|f| f.path.as_str().ends_with(".txt") || f.path.as_str().contains("/payload/"))
    );
    let cancelled = AtomicBool::new(false);
    let limits = TextMetadataLimits {
        max_packet_bytes: 1_048_576,
        max_state_bytes: 8_388_608,
        max_issues: 128,
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let packet = ctx
        .files
        .iter()
        .find(|f| f.path.as_str().ends_with("/source-text-unit.fixture.json"))
        .unwrap();
    let layer = ctx
        .files
        .iter()
        .find(|f| f.path.as_str().ends_with("/source-text-layer.fixture.json"))
        .unwrap();
    let anchor = ctx
        .files
        .iter()
        .find(|f| f.path.as_str().ends_with("/source-anchor-v2.fixture.json"))
        .unwrap();
    for report in [
        inspect_source_text_unit_v1_metadata(&packet.raw, packet.path.as_str(), limits, &cancelled)
            .unwrap(),
        inspect_source_text_layer_metadata(&layer.raw, layer.path.as_str(), limits, &cancelled)
            .unwrap(),
        inspect_source_anchor_v2_metadata(&anchor.raw, anchor.path.as_str(), limits, &cancelled)
            .unwrap(),
    ] {
        assert_eq!(
            report.state,
            TextMetadataState::CheckedMetadata,
            "{report:?}"
        );
        assert!(report.issues.is_empty(), "{report:?}");
        assert_eq!(report.scope, "owner-metadata-predicates-only");
    }
    // Retaining the original laboratory maker after changing source posture
    // must still be rejected by the actual owner rule.
    let mut incompatible = parse(&packet.raw);
    let mut schemes = incompatible
        .object_get("schemes")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    nested(
        &mut schemes[0],
        &["method", "maker_kind"],
        text("synthetic_fixture"),
    );
    set(&mut incompatible, "schemes", JsonValue::Array(schemes));
    let rejected = inspect_source_text_unit_v1_metadata(
        &bytes(&incompatible),
        packet.path.as_str(),
        limits,
        &cancelled,
    )
    .unwrap();
    assert_eq!(rejected.state, TextMetadataState::InvalidInput);
    assert!(
        rejected
            .issues
            .iter()
            .any(|issue| issue.code == "metadata-owner-predicate"
                && issue.subject == packet.path.as_str()
                && issue
                    .message
                    .starts_with("synthetic scheme maker escaped the synthetic laboratory:")),
        "{rejected:?}"
    );
    let prepared = run_with_cut(&ctx, None, true).unwrap();
    assert_eq!(prepared.handler_id, "public-profile-revision");
    assert_eq!(
        prepared.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    let mut missing = ctx.clone();
    missing
        .files
        .retain(|f| !f.path.as_str().ends_with("/rights.json"));
    assert!(matches!(
        run_with_cut(&missing, None, true),
        Err(SourceCommandError::Unsupported(_))
    ));
    let mut wrong = ctx.clone();
    let expression = wrong
        .files
        .iter_mut()
        .find(|f| f.path.as_str().ends_with("/expression.json"))
        .unwrap();
    let mut value = parse(&expression.raw);
    set(&mut value, "work_ref", text("tos.work.revision.another"));
    expression.raw = bytes(&value);
    assert!(matches!(
        run_with_cut(&wrong, None, true),
        Err(SourceCommandError::Conflict(_))
    ));
    let mut changed_software = ctx.clone();
    changed_software
        .files
        .iter_mut()
        .find(|f| f.path.as_str() == "scripts/native_text_binding.py")
        .unwrap()
        .raw
        .push(b' ');
    assert!(matches!(
        run_with_cut(&changed_software, None, true),
        Err(SourceCommandError::Conflict(_))
    ));
}

// The retained Python handler is the independent oracle; native tests do not
// monkeypatch its engine or dispatch it through the native invocation.
fn initial_creation_python_oracle(
    repository: &Path,
    owner: &Path,
    request: &Value,
    deadline: Instant,
) -> Value {
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::process::{Command, Stdio};
    let mut input = tempfile::tempfile().unwrap();
    let raw = canonical_json(request);
    assert!(raw.len() <= 1_048_576);
    input.write_all(&raw).unwrap();
    input.seek(SeekFrom::Start(0)).unwrap();
    let mut output = tempfile::tempfile().unwrap();
    let mut errors = tempfile::tempfile().unwrap();
    let mut child =
        Command::new(crate::maintained_python())
            .args(["-c", "import json,sys;from pathlib import Path;repo=Path(sys.argv[1]);sys.path[:0]=[str(repo/'scripts'),str(repo/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')];import source_commands as commands;request=commands._json_object(commands._canonical(json.load(sys.stdin)));print(json.dumps(commands.run_legacy_oracle_command(Path(sys.argv[2]),request),ensure_ascii=False,allow_nan=False))"])
            .arg(repository)
            .arg(owner)
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
            panic!("creation oracle bounded refusal");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < step);
    assert!(
        output.metadata().unwrap().len() <= 1_048_576
            && errors.metadata().unwrap().len() <= 1_048_576
    );
    let mut raw = Vec::new();
    let mut error = Vec::new();
    output.seek(SeekFrom::Start(0)).unwrap();
    output.read_to_end(&mut raw).unwrap();
    errors.seek(SeekFrom::Start(0)).unwrap();
    errors.read_to_end(&mut error).unwrap();
    assert!(
        status.success(),
        "creation oracle: {}",
        String::from_utf8_lossy(&error)
    );
    serde_json::from_slice(&raw).unwrap()
}

#[test]
fn native_initial_creation_cli_preserves_oracle_and_cold_retained_receipt() {
    use super::command_text_cases::{
        alignment_image_digest, alignment_native_cli, authored_text_files,
    };
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::IsolatedCreationRoot;
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let inputs = [
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
        "scripts/source_witness_human_forms.py",
        "scripts/build_source_witness_catalog.py",
        "scripts/source_record_profiles.py",
        "scripts/native_text_binding.py",
        "scripts/source_owner_context.py",
        "scripts/source_witness_bibliographic_graph_common.py",
        "rust/crates/tos-command/src/source_native_creation_cli.rs",
        "rust/crates/tos-command/src/source_creation_cli_selection.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_creation.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "ToS/contracts/historical-record.schema.json",
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/historical-claim.schema.json",
        "ToS/contracts/claim-packet.schema.json",
        "ToS/contracts/knowledge-assessment.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
        "ToS/contracts/source-metadata-record.schema.json",
        "ToS/contracts/semantic-description-record.schema.json",
        "ToS/contracts/research-corpus-record.schema.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ];

    let mut fixture_bytes = 0u64;
    for name in inputs {
        let size = fs::metadata(repository.join(name)).unwrap().len();
        assert!(size <= 8_388_608);
        fixture_bytes = fixture_bytes.checked_add(size).unwrap();
        assert!(fixture_bytes <= 33_554_432);
    }
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must select protected native creation image"),
    );
    assert!(native.is_absolute());
    let worker = super::validation_cut_cases::selected_worker_path();
    let native_bytes = fs::metadata(&native).unwrap().len();
    let worker_bytes = fs::metadata(&worker).unwrap().len();
    let consumer_bytes = fs::metadata(std::env::current_exe().unwrap())
        .unwrap()
        .len();
    assert!(
        native_bytes <= 536_870_912 && worker_bytes <= 536_870_912 && consumer_bytes <= 536_870_912,
        "native creation image bound exceeded: owner={native_bytes} worker={worker_bytes} consumer={consumer_bytes} max=536870912"
    );
    assert!(Instant::now() < deadline);
    eprintln!(
        "creation CLI F={} E={} C={} W={} native_processes=6 oracle_processes=3 workers<=6",
        fixture_bytes, native_bytes, consumer_bytes, worker_bytes
    );
    let files: BTreeMap<String, Vec<u8>> = inputs
        .iter()
        .map(|name| (name.to_string(), fs::read(repository.join(name)).unwrap()))
        .collect();
    assert_eq!(
        files.values().map(|raw| raw.len() as u64).sum::<u64>(),
        fixture_bytes
    );
    let (capture, software, components) = captured_components(&files, deadline, &cancellation);
    let temporary = tempfile::tempdir().unwrap();
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let store = temporary.path().join("selected-store");
    let base = super::validation_cut_cases::write_cut_store(&authored, &store);
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
    for (name, raw) in &files {
        let target = isolated.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let source = "ToS/source-witnesses/history/cli-subject/historical-event.json";
    let home = Path::new(source).parent().unwrap();
    fs::create_dir_all(isolated.path().join(home.parent().unwrap())).unwrap();
    let record = serde_json::json!({"schema_version":"tos_historical_record_v1", "record_type":"historical-event",
        "record_id":"tos.historical-event.synthetic-native-create", "record_version":1,
        "preferred_label":"Synthetic mechanics subject", "variant_labels":[], "identity_status":"provisional",
        "source_refs":["synthetic-test-only:creation-not-assessment"], "external_identifiers":[],
        "same_as_posture":"no_equivalence_claim", "visibility":"public_metadata_only", "supersedes_ref":null,
        "notes":"Synthetic proposed source metadata only; no historical existence, textual judgment or admission asserted.",
        "field_languages":{"preferred_label":{"language":"en","script":"Latn"},"notes":{"language":"en","script":"Latn"}}});
    let config = serde_json::json!({"schema_version":"tos_local_historical_create_owner_v2",
        "uid":fs::metadata(isolated.path()).unwrap().uid(), "principal_id":"software:test-fixture",
        "maker_type":"software", "source_root":isolated.path(), "source_path":source,
        "record_id":record["record_id"], "authority_ref":"synthetic-test-only:creation-not-assessment",
        "allowed_form_ids":["tos.form.creation.fixture-name"], "allowed_claim_ids":[],
        "allowed_operations":["historical.create"], "expires_at":"2099-01-01T00:00:00Z",
        "provenance_event_id":"tos.event.synthetic-native-create"});
    let owner = isolated.path().join("owner.json");
    fs::write(&owner, canonical_json(&config)).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let invocation_path = temporary.path().join("native-creation-invocation.json");
    let mut invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1", "owner_context":null, "owner_config":owner,
        "assessment_schema_worker":null,
        "native_executable":native, "native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
        "corpus_store":store,"source_revision":base.0.to_prefixed(),"original_source_revision":base.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member| member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":worker,"sha256":alignment_image_digest(&worker).to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
            "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,
            "worker_address_space_bytes":1073741824}});
    let write_invocation = |value: &Value| {
        fs::write(&invocation_path, canonical_json(value)).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    };
    write_invocation(&invocation);
    let mut preview = Value::Null;
    for request in [
        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare","record":record}),
        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","record":record,
            "claims":[],"forms":[{"form_id":"tos.form.creation.fixture-name","field_id":"metadata.preferred-name"}]}),
    ] {
        let oracle = initial_creation_python_oracle(&repository, &owner, &request, deadline);
        let actual =
            alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
        assert_eq!(
            actual["schema_version"],
            "tos_local_native_source_result_v1"
        );
        assert_eq!(
            actual["result"], oracle,
            "entire maintained {} result",
            request["operation"]
        );
        assert_eq!(actual["grants_admission"], false);
        if request["operation"] == "prepare-create" {
            preview = actual["result"].clone();
        }
    }
    let request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"historical.create",
        "record":record,"claims":[],"forms":[{"form_id":"tos.form.creation.fixture-name","field_id":"metadata.preferred-name"}],
        "command_id":"synthetic:whole-native-creation-cli", "expected_configuration":preview["owner_configuration"],
        "expected_dependencies":preview["expected_dependencies"],"expected_source":null,"expected_revision":null});
    let created = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(created["result"]["replayed"], false);
    let original = fs::read_dir(isolated.path().join(home))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (name, reference) in preview["prepared_files"].as_object().unwrap() {
        let raw = original.get(name).unwrap();
        assert_eq!(reference["sha256"], Digest256::of_bytes(raw).to_prefixed());
        assert_eq!(reference["bytes"], raw.len());
    }
    let cold = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(cold["result"]["replayed"], true);
    assert_eq!(cold["result"]["receipt"], created["result"]["receipt"]);
    let mut current_files = authored_text_files(isolated.path());
    current_files.remove("ToS/source-witnesses/.historical-create.writer.lock");
    assert!(current_files.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let current =
        super::validation_cut_cases::write_cut_store_on_base(&current_files, &store, Some(base));
    invocation["source_revision"] = serde_json::json!(current.0.to_prefixed());
    write_invocation(&invocation);
    let successor_cold =
        alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(successor_cold["result"]["replayed"], true);
    assert_eq!(
        successor_cold["result"]["receipt"],
        created["result"]["receipt"]
    );
    assert_eq!(
        fs::read_dir(isolated.path().join(home)).unwrap().count(),
        original.len()
    );
    for (name, raw) in original {
        assert_eq!(
            fs::read(isolated.path().join(home).join(name)).unwrap(),
            raw
        );
    }
    assert!(Instant::now() < deadline);
    drop(software);
    drop(components);
    temporary.close().unwrap();
}
