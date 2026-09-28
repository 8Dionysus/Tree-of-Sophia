//! Composed Claim successor/history over the existing claim_v1 source wording.
//! The fixture contains a v2 Claim but no Claim predecessor package: that exact
//! incomplete input must refuse. The positive baseline below is explicitly a
//! synthetic initial version of its body, not invented historical v1 evidence.
use super::command_form_cases::{context, open_cut, schemas, successor};
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_claims::{
    CLAIM_GROUNDING_RULE_INPUTS, CLAIM_REVISION_RULE_INPUTS,
    execute_isolated_claim_creation_from_captures, execute_isolated_claim_revision_from_captures,
    prepare_isolated_claim_creation_from_captures, prepare_isolated_claim_revision_from_captures,
    run_claim_command, run_claim_command_from_captures,
};
use tos_command::source_command::{PreparedCommand, SourceCommandError};
use tos_command::source_forms::metadata_subject;
use tos_command::source_operation::{SourceOperationError, bind_selected_candidate};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::operation::OperationLimits;
use tos_validation::{FormatProfile, executor::ExecutorBudget};

fn selected_context(
    files: &BTreeMap<String, Vec<u8>>,
    software: &BTreeMap<String, Vec<u8>>,
    configuration: &Value,
    request: &Value,
    base: SourceRevision,
) -> tos_command::source_command::CommandContext {
    let mut inputs = files.clone();
    for (name, raw) in software {
        assert!(inputs.insert(name.clone(), raw.clone()).is_none());
    }
    context(
        &inputs,
        serde_json::to_vec(configuration).unwrap(),
        serde_json::to_vec(request).unwrap(),
        base,
    )
}
fn checked_command(
    ctx: &tos_command::source_command::CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PreparedCommand, SourceCommandError> {
    run_claim_command_from_captures(ctx, cut, software, components, worker, deadline, cancel)
}

// Independent maintained owner oracle. Its output is bounded, ephemeral and
// compared as data; this Python process is never a production serialization
// producer, authority issuer or native admission substitute.
fn maintained_oracle(
    files: &BTreeMap<String, Vec<u8>>,
    configuration: &Value,
    request: &Value,
    repository: &Path,
) -> Value {
    use std::process::{Command, Stdio};
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("source");
    let mut total = 0usize;
    for (name, raw) in files {
        total += raw.len();
        assert!(total <= 8_388_608);
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, raw).unwrap();
    }
    let mut config = configuration.clone();
    config["source_root"] = Value::String(root.to_str().unwrap().into());
    let config_path = temporary.path().join("owner.json");
    let request_path = temporary.path().join("request.json");
    fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::write(&request_path, serde_json::to_vec(request).unwrap()).unwrap();
    let stdout_path = temporary.path().join("oracle.stdout");
    let stderr_path = temporary.path().join("oracle.stderr");
    let script = "import json,sys\nfrom pathlib import Path\nr=Path(sys.argv[1]);sys.path[:0]=[str(r/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(r/'scripts')]\nimport claim_revisions as c\nconfig=json.loads(Path(sys.argv[2]).read_text());request=json.loads(Path(sys.argv[3]).read_text());path=Path(config['source_root'])/config['source_path']\nfiles={p.name:p.read_bytes() for p in path.parent.iterdir() if p.is_file()};record=c._claims(files[path.name])[config['claim_id']]\n_,_,_,_,dependencies,bindings=c._proposal(config,path,files,record,request)\nprint(json.dumps({'expected_dependencies':dependencies,'source_bindings':bindings},ensure_ascii=False,sort_keys=True,separators=(',',':')))\n";
    let mut child = Command::new("python3")
        .args(["-c", script])
        .arg(repository)
        .arg(&config_path)
        .arg(&request_path)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::from(fs::File::create(&stdout_path).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr_path).unwrap()))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline
            || fs::metadata(&stdout_path).unwrap().len() > 1_048_576
            || fs::metadata(&stderr_path).unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded maintained Claim oracle refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(fs::metadata(&stdout_path).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(&stderr_path).unwrap().len() <= 1_048_576);
    assert!(
        status.success(),
        "{}",
        String::from_utf8_lossy(&fs::read(stderr_path).unwrap())
    );
    serde_json::from_slice(&fs::read(stdout_path).unwrap()).unwrap()
}

// The generated catalogue is selected from this protected synthetic owner
// root, separately from its authored cut. Use the maintained producer, so a
// derived row cannot attest to itself during an identity proposal.
fn publish_fixture_catalog(repository: &Path, root: &Path, deadline: Instant) {
    use std::process::{Command, Stdio};
    let output = root.join("fixture-catalog.stdout");
    let errors = root.join("fixture-catalog.stderr");
    let script = "import pathlib,sys;sys.path.insert(0,str(pathlib.Path(sys.argv[1])/'scripts'));import build_source_witness_catalog as catalog;root=pathlib.Path(sys.argv[2]);catalog.write_outputs(root,catalog.render_outputs(root))";
    let mut child = Command::new("python3")
        .args(["-c", script])
        .arg(repository)
        .arg(root)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::from(fs::File::create(&output).unwrap()))
        .stderr(Stdio::from(fs::File::create(&errors).unwrap()))
        .spawn()
        .unwrap();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline
            || fs::metadata(&output).unwrap().len() > 1_048_576
            || fs::metadata(&errors).unwrap().len() > 1_048_576
        {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded maintained generated catalogue refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < deadline);
    assert!(fs::metadata(&output).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(&errors).unwrap().len() <= 1_048_576);
    assert!(
        status.success(),
        "{}",
        String::from_utf8_lossy(&fs::read(errors).unwrap())
    );
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
fn source_bytes(value: &JsonValue) -> Vec<u8> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits::default(),
    )
    .unwrap()
}
fn response(prepared: &PreparedCommand) -> Value {
    serde_json::from_slice(&source_bytes(&prepared.response)).unwrap()
}
fn source_ref(value: &Value) -> Value {
    serde_json::from_slice(&source_bytes(
        &metadata_subject(&source_value(value)).unwrap(),
    ))
    .unwrap()
}
fn apply_changes(files: &mut BTreeMap<String, Vec<u8>>, prepared: &PreparedCommand) {
    for change in &prepared.changes {
        assert_eq!(
            change.before,
            files
                .get(change.path.as_str())
                .map(|raw| Digest256::of_bytes(raw))
        );
        match &change.after {
            Some(raw) => {
                files.insert(change.path.as_str().into(), raw.clone());
            }
            None => {
                files.remove(change.path.as_str());
            }
        }
    }
}
fn claim_fixture() -> (BTreeMap<String, Vec<u8>>, Value, Value, String) {
    let repository = super::validation_cut_cases::repository();
    let packet =
        repository.join("rust/crates/tos-command/tests/fixtures/source_forms_shadow/claim_v1");
    let source: Value =
        serde_json::from_slice(&fs::read(packet.join("source.json")).unwrap()).unwrap();
    let old_owner: Value =
        serde_json::from_slice(&fs::read(packet.join("owner.json")).unwrap()).unwrap();
    let source_path = old_owner["source_path"].as_str().unwrap().to_owned();
    let evidence = source["evidence_refs"][0].as_str().unwrap();
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            files.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    for name in ["entity-types.v1.json", "relation-types.v1.json"] {
        let path = format!("ToS/doctrine/semantic-interchange/{name}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    files.insert(
        evidence.into(),
        b"Synthetic command fixture evidence; not historical verification.\n".to_vec(),
    );
    // Bounded endpoint scaffolding uses the fixture's exact IDs and actual
    // selected owner schemas. These stubs are synthetic test metadata.
    for (kind, id, schema) in [
        (
            "letter",
            source["subject_ref"].as_str().unwrap(),
            "tos_document_record_v1",
        ),
        (
            "agent",
            source["object"].as_str().unwrap(),
            "tos_corpus_record_v1",
        ),
    ] {
        let mut record = serde_json::json!({"schema_version":schema,"record_type":kind,"record_id":id,"preferred_label":"Synthetic command fixture endpoint; no assessment","identity_status":"provisional","source_refs":[evidence],"external_identifiers":[],"same_as_posture":"no_equivalence_claim","record_version":1});
        if kind == "letter" {
            record["visibility"] = Value::String("public_metadata_only".into());
        }
        files.insert(
            format!("ToS/source-witnesses/{kind}s/command-claim-fixture/{kind}.json"),
            source_bytes(&source_value(&record)),
        );
    }
    let configuration = serde_json::json!({"schema_version":"tos_local_claim_revision_owner_v1","uid":1000,"principal_id":old_owner["principal_id"],"source_root":"synthetic:anchored-command-fixture","source_path":source_path,"authority_ref":"synthetic:claim-revision-conformance-no-admission","expires_at":"2099-01-01T00:00:00Z","claim_id":source["claim_id"],"allowed_operations":["claim.revise"],"allowed_fields":["qualifiers"],"allowed_evidence_refs":source["evidence_refs"],"allowed_form_ids":old_owner["allowed_form_ids"],"allowed_form_field_ids":["claim.statement","claim.name","claim.caption","claim.hover"]});
    (files, source, configuration, source_path)
}

#[test]
fn claim_successor_retains_bytes_replays_current_scope_and_refuses_unissued_admission() {
    let (mut files, fixture, mut configuration, source_path) = claim_fixture();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("claim-store");
    configuration["source_root"] = Value::String(root.to_str().unwrap().into());
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(180);
    // Do not silently reinterpret a v2 fixture as an initial package.
    files.insert(
        source_path.clone(),
        [source_bytes(&source_value(&fixture)), b"\n".to_vec()].concat(),
    );
    let missing_root = temporary.path().join("incomplete-existing-fixture-store");
    let missing_base = super::validation_cut_cases::write_cut_store(&files, &missing_root);
    let missing_cut = open_cut(&missing_root, missing_base, deadline, &cancel);
    let mut worker = schemas(&missing_cut, deadline, &cancel);
    let describe =
        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"});
    assert!(matches!(
        run_claim_command(
            &context(
                &files,
                serde_json::to_vec(&configuration).unwrap(),
                serde_json::to_vec(&describe).unwrap(),
                missing_base
            ),
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    // Explicit synthetic baseline preserves every authored fixture qualifier.
    let mut initial = fixture.clone();
    initial["claim_version"] = Value::from(1);
    let mut initial_stream = b" \r\n".to_vec();
    initial_stream.extend(source_bytes(&source_value(&initial)));
    initial_stream.extend(b"\r\n\t\n");
    files.insert(source_path.clone(), initial_stream.clone());
    // Independent synthetic initial cut: never attach an invented predecessor
    // or a v2 -> v1 regression to the incomplete existing fixture's ancestry.
    let base = super::validation_cut_cases::write_cut_store(&files, &root);
    let cut = open_cut(&root, base, deadline, &cancel);
    let mut worker = schemas(&cut, deadline, &cancel);
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
    assert!(commit_output.stdout.len() <= 41);
    let commit = String::from_utf8(commit_output.stdout)
        .unwrap()
        .trim()
        .to_owned();
    let rule_names: std::collections::BTreeSet<&str> = CLAIM_GROUNDING_RULE_INPUTS
        .iter()
        .chain(CLAIM_REVISION_RULE_INPUTS)
        .copied()
        .filter(|name| !name.starts_with("ToS/"))
        .collect();
    let capture = super::source_cut_cases::captured_software_fixture(
        &repository,
        &commit,
        &rule_names.iter().copied().collect::<Vec<_>>(),
    );
    let software = SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 128,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
        deadline,
        &cancel,
    )
    .unwrap();
    let components = software
        .select_components(
            &rule_names
                .iter()
                .map(|name| RelativePath::parse(name).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let mut software_files = BTreeMap::new();
    for name in rule_names {
        let path = RelativePath::parse(name).unwrap();
        let raw = software
            .read_selected_component(&components, &path, 1_048_576, deadline, &cancel)
            .unwrap();
        assert_eq!(
            raw,
            fs::read(repository.join(name)).unwrap(),
            "oracle rule bytes differ from selected capture"
        );
        software_files.insert(name.into(), raw);
    }
    let ids = configuration["allowed_form_ids"].as_array().unwrap();
    let selections: Vec<Value> = ids
        .iter()
        .zip([
            "claim.statement",
            "claim.name",
            "claim.caption",
            "claim.hover",
        ])
        .map(|(id, field)| serde_json::json!({"form_id":id,"field_id":field}))
        .collect();
    let wording = format!(
        "{} [command fixture successor]",
        initial["qualifiers"]["statement"].as_str().unwrap()
    );
    let proposal = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-revise","fields":{"qualifiers":{"statement":wording}},"forms":selections,"reason":"Synthetic source-copy Claim successor conformance; no assessment."});
    let preview_ctx = selected_context(&files, &software_files, &configuration, &proposal, base);
    assert!(
        matches!(
            run_claim_command(&preview_ctx, &mut worker, deadline, &cancel),
            Err(SourceCommandError::Unsupported(_))
        ),
        "a selected byte list cannot establish complete Claim inventory"
    );
    let mut partial = preview_ctx.clone();
    partial
        .files
        .retain(|input| !input.path.as_str().ends_with("/letter.json"));
    assert!(
        matches!(
            checked_command(
                &partial,
                &cut,
                &software,
                &components,
                &mut worker,
                deadline,
                &cancel
            ),
            Err(SourceCommandError::Unsupported(_))
        ),
        "complete current inventory cannot be inferred from omitted selected metadata"
    );
    let preview = checked_command(
        &preview_ctx,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    let preview_response = response(&preview);
    let oracle = maintained_oracle(&files, &configuration, &proposal, &repository);
    assert_eq!(
        preview_response["expected_dependencies"], oracle["expected_dependencies"],
        "maintained dependency fingerprint"
    );
    assert_eq!(
        preview_response["source_bindings"], oracle["source_bindings"],
        "maintained declared source bindings"
    );
    let mut request = proposal.clone();
    request["operation"] = Value::String("claim.revise".into());
    request["command_id"] = Value::String("conformance:claim-successor".into());
    for (request_key, response_key) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
        ("expected_dependencies", "expected_dependencies"),
        ("expected_inputs", "source_bindings"),
    ] {
        request[request_key] = preview_response[response_key].clone();
    }
    let ctx = selected_context(&files, &software_files, &configuration, &request, base);
    let prepared = checked_command(
        &ctx,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    let result = response(&prepared);
    assert!(!prepared.replayed);
    assert_eq!(result["receipt"]["previous_source"], source_ref(&initial));
    assert_eq!(result["receipt"]["source"]["version"], 2);
    assert_eq!(
        prepared.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    // Independent expected row update protects richer qualifier preservation,
    // sibling whitespace and the exact maintained canonical row convention.
    let mut expected = initial.clone();
    expected["qualifiers"]["statement"] = proposal["fields"]["qualifiers"]["statement"].clone();
    expected["claim_version"] = Value::from(2);
    let mut expected_stream = b" \r\n".to_vec();
    expected_stream.extend(source_bytes(&source_value(&expected)));
    expected_stream.extend(b"\r\n\t\n");
    let write = prepared
        .changes
        .iter()
        .find(|change| change.path.as_str() == source_path)
        .unwrap();
    assert_eq!(write.after.as_ref().unwrap(), &expected_stream);
    let archive = result["receipt"]["archive_path"].as_str().unwrap();
    let archive_stream = format!(
        "{archive}/{}.blob",
        Digest256::of_bytes(&initial_stream).to_hex()
    );
    assert_eq!(
        prepared
            .changes
            .iter()
            .find(|change| change.path.as_str() == archive_stream)
            .unwrap()
            .after
            .as_ref()
            .unwrap(),
        &initial_stream
    );
    let manifest: Value = serde_json::from_slice(
        prepared
            .changes
            .iter()
            .find(|change| change.path.as_str() == format!("{archive}/manifest.json"))
            .unwrap()
            .after
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest["files"],
        serde_json::json!({"source-claims.jsonl":{"blob":format!("{}.blob",Digest256::of_bytes(&initial_stream).to_hex()),"sha256":Digest256::of_bytes(&initial_stream).to_prefixed(),"bytes":initial_stream.len()}})
    );
    let history_path = format!(
        "{}/claim-revision-history.json",
        source_path.rsplit_once('/').unwrap().0
    );
    assert!(
        prepared
            .changes
            .iter()
            .any(|change| change.path.as_str() == history_path)
    );
    apply_changes(&mut files, &prepared);
    let candidate = successor(&files, &root, base);
    let candidate_cut = open_cut(&root, candidate, deadline, &cancel);
    let bound = bind_selected_candidate(
        &ctx,
        prepared,
        &cut,
        &software,
        &components,
        &candidate_cut,
        RelativePath::parse("protected-owner/claim-command.json").unwrap(),
        OperationLimits {
            max_member_bytes: 8_388_608,
            max_total_bytes: 33_554_432,
            max_state_bytes: 33_554_432,
            max_reads: 8192,
            max_changes: 64,
            deadline,
        },
        &cancel,
    )
    .unwrap();
    assert_eq!(bound.binding().base_revision(), base);
    assert_eq!(bound.binding().candidate_revision(), candidate);
    assert!(matches!(
        bound.commit(),
        Err(SourceOperationError::MissingFullSourceAdmission)
    ));
    let mut worker = schemas(&candidate_cut, deadline, &cancel);
    let replay_ctx = selected_context(&files, &software_files, &configuration, &request, candidate);
    let replay = checked_command(
        &replay_ctx,
        &candidate_cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    assert!(replay.replayed);
    assert!(replay.changes.is_empty());
    assert_eq!(response(&replay)["receipt"], result["receipt"]);
    let inspect = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-version","source":source_ref(&initial)});
    let inspected = checked_command(
        &selected_context(&files, &software_files, &configuration, &inspect, candidate),
        &candidate_cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    assert_eq!(response(&inspected)["record"], initial);
    assert_eq!(
        response(&inspected)["files"]["source-claims.jsonl"],
        serde_json::json!({"archive_path":archive_stream,"sha256":Digest256::of_bytes(&initial_stream).to_prefixed(),"bytes":initial_stream.len()})
    );
    assert!(inspected.changes.is_empty());
    // A bound candidate and retained receipt do not restore current authority.
    let mut revoked = configuration.clone();
    revoked["allowed_operations"] = serde_json::json!([]);
    let revoked_ctx = selected_context(&files, &software_files, &revoked, &request, candidate);
    assert!(matches!(
        checked_command(
            &revoked_ctx,
            &candidate_cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Denied(_))
    ));
    let mut reused = request.clone();
    reused["reason"] = Value::String("Different request sharing retained command identity.".into());
    assert!(matches!(
        checked_command(
            &selected_context(&files, &software_files, &configuration, &reused, candidate),
            &candidate_cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    let mut corrupted = files.clone();
    corrupted.insert(archive_stream, b"{}\n".to_vec());
    let corrupt_revision = successor(&corrupted, &root, candidate);
    let corrupt_cut = open_cut(&root, corrupt_revision, deadline, &cancel);
    let mut corrupt_worker = schemas(&corrupt_cut, deadline, &cancel);
    assert!(matches!(
        checked_command(
            &selected_context(
                &corrupted,
                &software_files,
                &configuration,
                &inspect,
                corrupt_revision
            ),
            &corrupt_cut,
            &software,
            &components,
            &mut corrupt_worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
}

#[test]
fn initial_claim_creation_publishes_five_native_files_and_cold_replays() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let (mut files, mut claim, mut revision_owner, source_path) = claim_fixture();
    claim["claim_version"] = Value::from(1);
    let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
        .iter()
        .chain(CLAIM_REVISION_RULE_INPUTS)
        .copied()
        .filter(|name| !name.starts_with("ToS/"))
        .collect::<Vec<_>>();
    software_names.extend([
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
    ]);
    software_names.sort_unstable();
    software_names.dedup();
    for name in software_names {
        files.insert(name.into(), fs::read(repository.join(name)).unwrap());
    }
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    let (_capture, software, components) =
        super::command_record_cases::captured_components(&files, deadline, &cancellation);
    let temporary = tempfile::tempdir().unwrap();
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let store = temporary.path().join("selected-store");
    let base = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = open_cut(&store, base, deadline, &cancellation);
    let claim_worker = || {
        let mut budget = ExecutorBudget::laboratory();
        budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!budget.execution_wall.is_zero());
        super::command_form_cases::schemas_for_profile_with_budget(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            budget,
            deadline,
            &cancellation,
        )
    };
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
    for (name, raw) in &files {
        let target = isolated.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    fs::create_dir_all(
        isolated
            .path()
            .join(&source_path)
            .parent()
            .unwrap()
            .parent()
            .unwrap(),
    )
    .unwrap();
    let uid = fs::metadata(isolated.path()).unwrap().uid();
    let configuration = serde_json::json!({
        "schema_version":"tos_local_claim_create_owner_v1", "uid":uid,
        "principal_id":claim["maker"]["agent_ref"], "maker_type":claim["maker"]["maker_type"],
        "source_root":isolated.path(), "source_path":source_path,
        "authority_ref":"synthetic-test-only:claim-creation-not-assessment",
        "expires_at":"2099-01-01T00:00:00Z", "provenance_event_id":claim["provenance_event_ref"],
        "allowed_operations":["claims.create"], "allowed_claim_ids":[claim["claim_id"]],
        "allowed_subject_refs":[claim["subject_ref"]], "allowed_object_refs":[claim["object"]],
        "allowed_predicates":[claim["predicate"]], "allowed_evidence_refs":claim["evidence_refs"]
    });
    let config_raw = source_bytes(&source_value(&configuration));
    let owner = isolated.path().join("owner.json");
    fs::write(&owner, &config_raw).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let filesystem =
        CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation).unwrap();
    let preview_request = serde_json::json!({
        "schema_version":"tos_local_source_command_v1", "operation":"prepare-create",
        "claims":[claim.clone()]
    });
    let mut context = selected_context(
        &authored,
        &files
            .iter()
            .filter(|(name, _)| !name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect(),
        &configuration,
        &preview_request,
        base,
    );
    context.effective_uid = u64::from(uid);
    let mut worker = claim_worker();
    let preview = checked_command(
        &context,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    let expected = response(&preview);
    assert_eq!(
        expected["prepared_files"]["source-claims.jsonl"]["sha256"],
        serde_json::json!(
            Digest256::of_bytes(&[source_bytes(&source_value(&claim)), b"\n".to_vec()].concat())
                .to_prefixed()
        )
    );
    let script = "import json,sys;from pathlib import Path;r=Path(sys.argv[1]);sys.path[:0]=[str(r/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(r/'scripts')];import source_commands as c;owner=Path(sys.argv[2]);request=json.load(sys.stdin);v=c.run_local_command(owner,request);print(json.dumps({'dependencies':v['expected_dependencies'],'bindings':v['source_bindings'],'files':v['prepared_files']},ensure_ascii=False,separators=(',',':')))";
    let oracle_stdout = temporary.path().join("claim-create-oracle.stdout");
    let oracle_stderr = temporary.path().join("claim-create-oracle.stderr");
    let mut oracle = std::process::Command::new("python3")
        .args(["-c", script])
        .arg(&repository)
        .arg(&owner)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::from(
            fs::File::create(&oracle_stdout).unwrap(),
        ))
        .stderr(std::process::Stdio::from(
            fs::File::create(&oracle_stderr).unwrap(),
        ))
        .spawn()
        .unwrap();
    use std::io::Write;
    oracle
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&preview_request).unwrap())
        .unwrap();
    let oracle_status = loop {
        if let Some(status) = oracle.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline
            || fs::metadata(&oracle_stdout).unwrap().len() > 1_048_576
            || fs::metadata(&oracle_stderr).unwrap().len() > 1_048_576
        {
            oracle.kill().unwrap();
            oracle.wait().unwrap();
            panic!("bounded maintained Claim creation oracle refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        Instant::now() < deadline,
        "maintained Claim oracle deadline"
    );
    assert!(fs::metadata(&oracle_stdout).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(&oracle_stderr).unwrap().len() <= 1_048_576);
    assert!(
        oracle_status.success(),
        "{}",
        String::from_utf8_lossy(&fs::read(&oracle_stderr).unwrap())
    );
    let oracle: Value = serde_json::from_slice(&fs::read(&oracle_stdout).unwrap()).unwrap();
    assert_eq!(expected["expected_dependencies"], oracle["dependencies"]);
    assert_eq!(expected["source_bindings"], oracle["bindings"]);
    assert_eq!(expected["prepared_files"], oracle["files"]);
    let mut request = preview_request;
    request["operation"] = Value::String("claims.create".into());
    request["command_id"] = Value::String("synthetic:claim-creation-first".into());
    request["expected_configuration"] = expected["owner_configuration"].clone();
    request["expected_revision"] = Value::Null;
    request["expected_dependencies"] = expected["expected_dependencies"].clone();
    request["expected_inputs"] = expected["source_bindings"].clone();
    context.request_raw = serde_json::to_vec(&request).unwrap();
    let (created, publication, result) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &cut,
        &software,
        &components,
        &mut worker,
        None,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(worker);
    assert!(!publication.replayed);
    assert_eq!(created.files().len(), 5);
    assert_eq!(
        created.command().commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    for (name, raw) in created.files() {
        assert_eq!(
            fs::read(isolated.path().join(created.home().as_str()).join(name)).unwrap(),
            *raw
        );
    }
    let result = serde_json::from_slice::<Value>(&source_bytes(&result)).unwrap();
    assert_eq!(
        result["receipt"]["schema_version"],
        "tos_local_claim_create_receipt_v1"
    );
    assert_eq!(result["receipt"]["grants_admission"], false);
    // The maintained form writer's operational lock is a private 0600
    // sidecar, not an authored cut member. Exercise the actual owner writer
    // spelling and mode while leaving the original five creation files intact.
    let lock_stdout = temporary.path().join("claim-form-lock.stdout");
    let lock_stderr = temporary.path().join("claim-form-lock.stderr");
    let lock_script = "import sys;from pathlib import Path;r=Path(sys.argv[1]);sys.path[:0]=[str(r/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(r/'scripts')];import source_commands as c;form=c.claim_forms_path(Path(sys.argv[2]),sys.argv[3]);\nwith c._locked(form): pass";
    let mut lock_writer = std::process::Command::new("python3")
        .args(["-c", lock_script])
        .arg(&repository)
        .arg(isolated.path().join(&source_path))
        .arg(claim["claim_id"].as_str().unwrap())
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(std::process::Stdio::from(
            fs::File::create(&lock_stdout).unwrap(),
        ))
        .stderr(std::process::Stdio::from(
            fs::File::create(&lock_stderr).unwrap(),
        ))
        .spawn()
        .unwrap();
    let lock_status = loop {
        if let Some(status) = lock_writer.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline
            || fs::metadata(&lock_stdout).unwrap().len() > 1_048_576
            || fs::metadata(&lock_stderr).unwrap().len() > 1_048_576
        {
            lock_writer.kill().unwrap();
            lock_writer.wait().unwrap();
            panic!("bounded maintained Claim form lock writer refused");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(Instant::now() < deadline, "maintained form lock deadline");
    assert!(fs::metadata(&lock_stdout).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(&lock_stderr).unwrap().len() <= 1_048_576);
    assert!(
        lock_status.success(),
        "{}",
        String::from_utf8_lossy(&fs::read(&lock_stderr).unwrap())
    );
    let form_name = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim["claim_id"].as_str().unwrap().as_bytes()).to_hex()
    );
    let lock_path = isolated
        .path()
        .join(&source_path)
        .with_file_name(format!(".{form_name}.writer.lock"));
    assert_eq!(
        fs::metadata(&lock_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(fs::read(&lock_path).unwrap().is_empty());
    let mut current_files = authored.clone();
    for (name, raw) in created.files() {
        current_files.insert(format!("{}/{name}", created.home().as_str()), raw.clone());
    }
    let current = successor(&current_files, &store, base);
    let current_cut = open_cut(&store, current, deadline, &cancellation);
    let original_creation_files = created.files().clone();
    let creation_home = created.home().as_str().to_owned();
    drop(created);
    let mut replay_worker = claim_worker();
    let (restored, replayed, replay_result) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut replay_worker,
        None,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(replay_worker);
    assert!(replayed.replayed);
    assert!(fs::read(&lock_path).unwrap().is_empty());
    assert!(restored.command().changes.is_empty());
    assert_eq!(replay_result, restored.command().response);
    assert_eq!(
        restored.files()["source-create-receipt.json"],
        fs::read(
            isolated
                .path()
                .join(restored.home().as_str())
                .join("source-create-receipt.json")
        )
        .unwrap()
    );
    drop(restored);
    fs::write(&lock_path, b"unexpected operational lock data").unwrap();
    let mut nonempty_lock_worker = claim_worker();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut nonempty_lock_worker,
            None,
            deadline,
            &cancellation,
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    drop(nonempty_lock_worker);
    fs::write(&lock_path, b"").unwrap();
    let evidence_path = isolated
        .path()
        .join(claim["evidence_refs"][0].as_str().unwrap());
    let evidence_original = fs::read(&evidence_path).unwrap();
    fs::write(&evidence_path, b"Changed after the selected Claim cut.\n").unwrap();
    let mut stale_source_worker = claim_worker();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut stale_source_worker,
            None,
            deadline,
            &cancellation,
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    drop(stale_source_worker);
    fs::write(&evidence_path, evidence_original).unwrap();
    let prior = fs::read(&owner).unwrap();
    let mut revoked = configuration;
    revoked["allowed_operations"] = serde_json::json!([]);
    fs::write(&owner, source_bytes(&source_value(&revoked))).unwrap();
    let mut revoked_worker = claim_worker();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut revoked_worker,
            None,
            deadline,
            &cancellation,
        ),
        Err(SourceCommandError::Conflict(_) | SourceCommandError::Denied(_))
    ));
    drop(revoked_worker);
    fs::write(&owner, prior).unwrap();

    // Correct the genuinely published Claim through its native isolated owner.
    // The maintained producer is an independent proposal oracle only; it
    // never mutates this root or supplies the native publication authority.
    revision_owner["uid"] = serde_json::json!(uid);
    revision_owner["source_root"] = serde_json::json!(isolated.path());
    let revision_owner_path = isolated.path().join("revision-owner.json");
    fs::write(
        &revision_owner_path,
        serde_json::to_vec(&revision_owner).unwrap(),
    )
    .unwrap();
    fs::set_permissions(&revision_owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let revised_wording = format!(
        "{} [retained correction; no assessment]",
        claim["qualifiers"]["statement"].as_str().unwrap()
    );
    let correction = serde_json::json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-revise",
        "fields":{"qualifiers":{"statement":revised_wording}},
        "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
        "reason":"Retained source-copy correction in an isolated Claim corpus; no assessment."
    });
    let revision_filesystem = CreationFilesystem::select_isolated(
        &isolated,
        &revision_owner_path,
        deadline,
        &cancellation,
    )
    .unwrap();
    let revision_software = files
        .iter()
        .filter(|(name, _)| !name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut revision_context = selected_context(
        &current_files,
        &revision_software,
        &revision_owner,
        &correction,
        current,
    );
    revision_context.effective_uid = u64::from(uid);
    let mut preview_budget = ExecutorBudget::laboratory();
    preview_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!preview_budget.execution_wall.is_zero());
    let mut revision_preview_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        preview_budget,
        deadline,
        &cancellation,
    );
    let prepared_correction = prepare_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_preview_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_preview_worker);
    let prepared = response(&prepared_correction);
    let mut oracle_files = current_files.clone();
    oracle_files.insert(
        format!("{creation_home}/.{form_name}.writer.lock"),
        Vec::new(),
    );
    let independent = maintained_oracle(&oracle_files, &revision_owner, &correction, &repository);
    assert_eq!(
        prepared["expected_dependencies"],
        independent["expected_dependencies"]
    );
    assert_eq!(prepared["source_bindings"], independent["source_bindings"]);
    let mut revision_request = correction;
    revision_request["operation"] = Value::String("claim.revise".into());
    revision_request["command_id"] =
        Value::String("synthetic:claim-creation-retained-correction".into());
    revision_request["expected_configuration"] = prepared["owner_configuration"].clone();
    revision_request["expected_source"] = prepared["source"].clone();
    revision_request["expected_revision"] = prepared["revision"].clone();
    revision_request["expected_dependencies"] = prepared["expected_dependencies"].clone();
    revision_request["expected_inputs"] = prepared["source_bindings"].clone();
    revision_context.request_raw = serde_json::to_vec(&revision_request).unwrap();
    let mut revision_budget = ExecutorBudget::laboratory();
    revision_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!revision_budget.execution_wall.is_zero());
    let mut revision_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        revision_budget,
        deadline,
        &cancellation,
    );
    let (revised_plan, revision_publication) = execute_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_worker);
    assert!(!revision_publication.replayed);
    assert_eq!(
        revised_plan.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    let correction = serde_json::json!({"result":response(&revised_plan)});
    assert_eq!(correction["result"]["replayed"], false);
    assert_eq!(correction["result"]["source"]["version"], 2);
    assert_eq!(correction["result"]["receipt"]["grants_admission"], false);
    assert_eq!(
        correction["result"]["receipt"]["previous_source"],
        result["receipt"]["claims"][0]
    );
    let revised_home = isolated.path().join(&creation_home);
    let expected_names = original_creation_files
        .keys()
        .cloned()
        .chain([
            form_name.clone(),
            "claim-revision-history.json".into(),
            format!(".{form_name}.writer.lock"),
        ])
        .collect::<std::collections::BTreeSet<_>>();
    let actual_names = fs::read_dir(&revised_home)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_names, expected_names);
    assert!(fs::read(&lock_path).unwrap().is_empty());
    let mut corrected_files = current_files;
    for name in &actual_names {
        if name.ends_with(".writer.lock") {
            continue;
        }
        let current_path = revised_home.join(name);
        assert_eq!(
            fs::metadata(&current_path).unwrap().permissions().mode() & 0o7777,
            0o600,
            "maintained corrected metadata uses private inode mode"
        );
        let raw = fs::read(current_path).unwrap();
        if name.starts_with("source-create-") {
            assert_eq!(&raw, &original_creation_files[name]);
        }
        corrected_files.insert(format!("{creation_home}/{name}"), raw);
    }
    let form: Value =
        serde_json::from_slice(&corrected_files[&format!("{creation_home}/{form_name}")]).unwrap();
    assert_eq!(form["forms"][0]["role"], "statement");
    assert_eq!(
        form["forms"][0]["bindings"]["wording"]["pointer"],
        "/qualifiers/statement"
    );
    let history: Value = serde_json::from_slice(
        &corrected_files[&format!("{creation_home}/claim-revision-history.json")],
    )
    .unwrap();
    assert_eq!(history["receipts"].as_array().unwrap().len(), 1);
    assert_eq!(history["receipts"][0], correction["result"]["receipt"]);
    let archive = correction["result"]["receipt"]["archive_path"]
        .as_str()
        .unwrap();
    let archive_dir = isolated.path().join(archive);
    let manifest: Value =
        serde_json::from_slice(&fs::read(archive_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["files"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        original_creation_files
            .keys()
            .cloned()
            .chain([format!(".{form_name}.writer.lock")])
            .collect::<std::collections::BTreeSet<_>>()
    );
    let lock_name = format!(".{form_name}.writer.lock");
    let lock_blob = manifest["files"][lock_name.as_str()]["blob"]
        .as_str()
        .unwrap();
    assert!(fs::read(archive_dir.join(lock_blob)).unwrap().is_empty());
    let archive_entries = fs::read_dir(&archive_dir).unwrap().collect::<Vec<_>>();
    assert!(archive_entries.len() <= 65);
    let mut archive_bytes = 0u64;
    for entry in archive_entries {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let metadata = entry.metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() <= 2_097_152);
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
        archive_bytes += metadata.len();
        assert!(archive_bytes <= 10_485_760);
        corrected_files.insert(format!("{archive}/{name}"), fs::read(entry.path()).unwrap());
    }
    let corrected = successor(&corrected_files, &store, current);
    let corrected_cut = open_cut(&store, corrected, deadline, &cancellation);
    let mut corrected_worker = claim_worker();
    let (restored_after_correction, replay_after_correction, corrected_result) =
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &corrected_cut,
            &software,
            &components,
            &mut corrected_worker,
            None,
            deadline,
            &cancellation,
        )
        .unwrap();
    drop(corrected_worker);
    assert!(replay_after_correction.replayed);
    assert!(restored_after_correction.command().changes.is_empty());
    assert_eq!(restored_after_correction.files(), &original_creation_files);
    assert_eq!(
        corrected_result,
        restored_after_correction.command().response
    );
    assert!(fs::read(&lock_path).unwrap().is_empty());
}

#[test]
fn initial_identity_proposals_retain_selected_catalog_and_cold_replay() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    for (version, predicate, schema) in [
        (
            "v1",
            "identity_transition_proposal",
            "tos_source_identity_transition_claim_v1",
        ),
        (
            "v2",
            "subject_identity_transition_proposal",
            "tos_subject_identity_transition_claim_v1",
        ),
    ] {
        let (mut files, old_claim, _, _) = claim_fixture();
        let agent_path = "ToS/source-witnesses/agents/command-claim-fixture/agent.json";
        let first: Value = serde_json::from_slice(&files[agent_path]).unwrap();
        let mut records = vec![first];
        for (suffix, label) in [
            ("second", "Second synthetic identity endpoint"),
            ("third", "Third synthetic identity endpoint"),
        ] {
            let mut record = records[0].clone();
            record["record_id"] = Value::String(format!("tos.agent.command-identity-{suffix}"));
            record["preferred_label"] = Value::String(label.into());
            let path = format!("ToS/source-witnesses/agents/command-identity-{suffix}/agent.json");
            files.insert(path, source_bytes(&source_value(&record)));
            records.push(record);
        }
        let refs = records.iter().map(source_ref).collect::<Vec<_>>();
        let predecessor = refs[0].clone();
        let successors = refs[1..].to_vec();
        let identities = refs
            .iter()
            .map(|reference| reference["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        // The structured value retains its own attributed wording; the
        // qualifier statement below remains a separate Claim assertion.
        let object = serde_json::json!({
            "kind":"identity-transition-proposal", "operation":"split",
            "source_wording":{
                "text":"Synthetic source wording proposes one provisional Agent identity splitting into two; no transition is performed.",
                "language":"en", "script":"Latn"
            },
            "members":identities, "predecessors":[predecessor], "successors":successors,
            "mapping":[
                {"predecessor":refs[0]["id"],"successor":refs[1]["id"]},
                {"predecessor":refs[0]["id"],"successor":refs[2]["id"]}
            ],
            "grounds":"Synthetic exact metadata versions; no identity admission.",
            "counterreading":"The three endpoints may remain distinct.",
            "scope":"Proposal for source metadata comparison only.",
            "unresolved_links":[], "supersedes_proposal":null
        });
        let mut claim = old_claim;
        claim["claim_id"] = Value::String(format!("tos.claim.synthetic-identity-{version}"));
        claim["claim_version"] = Value::from(1);
        claim["claim_type"] = Value::String("relation".into());
        claim["assertion_layer"] = Value::String("identity_assertion".into());
        claim["schema_version"] = Value::String(schema.into());
        claim["predicate"] = Value::String(predicate.into());
        claim["subject_ref"] = refs[0]["id"].clone();
        claim["object"] = object.clone();
        claim["qualifiers"] = serde_json::json!({
            "statement":"The selected metadata supports only a proposed split.",
            "statement_language":"en", "statement_script":"Latn"
        });
        claim["assessment_refs"] = serde_json::json!([]);
        claim["supersedes_claim_ref"] = Value::Null;
        let source_path = format!(
            "ToS/source-witnesses/relations/command-identity-{version}/source-claims.jsonl"
        );
        let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
            .iter()
            .chain(CLAIM_REVISION_RULE_INPUTS)
            .copied()
            .filter(|name| !name.starts_with("ToS/"))
            .collect::<Vec<_>>();
        software_names.extend([
            "rust/crates/tos-command/src/source_claims.rs",
            "rust/crates/tos-command/src/source_serialization.rs",
        ]);
        software_names.sort_unstable();
        software_names.dedup();
        for name in software_names {
            files.insert(name.into(), fs::read(repository.join(name)).unwrap());
        }
        let cancellation = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(240);
        let (_capture, software, components) =
            super::command_record_cases::captured_components(&files, deadline, &cancellation);
        let temporary = tempfile::tempdir().unwrap();
        let authored = files
            .iter()
            .filter(|(name, _)| name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        let store = temporary.path().join("selected-store");
        let base = super::validation_cut_cases::write_cut_store(&authored, &store);
        let cut = open_cut(&store, base, deadline, &cancellation);
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
        for (name, raw) in &files {
            let target = isolated.path().join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, raw).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        }
        fs::create_dir_all(
            isolated
                .path()
                .join(&source_path)
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .unwrap();
        publish_fixture_catalog(&repository, isolated.path(), deadline);
        let uid = fs::metadata(isolated.path()).unwrap().uid();
        let configuration = serde_json::json!({
            "schema_version":format!("tos_local_identity_proposal_create_owner_{version}"), "uid":uid,
            "principal_id":claim["maker"]["agent_ref"],
            "maker_type":claim["maker"]["maker_type"],
            "source_root":isolated.path(), "source_path":source_path,
            "authority_ref":"synthetic-test-only:identity-proposal-no-admission",
            "expires_at":"2099-01-01T00:00:00Z",
            "provenance_event_id":claim["provenance_event_ref"],
            "allowed_operations":["claims.create"],
            "allowed_claim_ids":[claim["claim_id"]],
            "allowed_subject_refs":[claim["subject_ref"]],
            "allowed_object_refs":refs.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
            "allowed_object_values":[object],
            "allowed_related_claim_refs":[],
            "allowed_predicates":[claim["predicate"]],
            "allowed_evidence_refs":claim["evidence_refs"]
        });
        let owner = isolated.path().join("identity-owner.json");
        fs::write(&owner, source_bytes(&source_value(&configuration))).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation)
                .unwrap();
        let software_files = files
            .iter()
            .filter(|(name, _)| !name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        let preview_request = serde_json::json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"prepare-create", "claims":[claim]
        });
        let mut context = selected_context(
            &authored,
            &software_files,
            &configuration,
            &preview_request,
            base,
        );
        context.effective_uid = u64::from(uid);
        let mut budget = ExecutorBudget::laboratory();
        budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!budget.execution_wall.is_zero());
        let mut worker = super::command_form_cases::schemas_for_profile_with_budget(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            budget,
            deadline,
            &cancellation,
        );
        let preview = prepare_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        let preview = response(&preview);
        assert_eq!(preview["grants_admission"], false);
        assert_eq!(
            preview["source_bindings"]["identity_proposals"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
        let mut request = preview_request;
        request["operation"] = Value::String("claims.create".into());
        request["command_id"] =
            Value::String(format!("synthetic:identity-proposal-create-{version}"));
        request["expected_configuration"] = preview["owner_configuration"].clone();
        request["expected_revision"] = Value::Null;
        request["expected_dependencies"] = preview["expected_dependencies"].clone();
        request["expected_inputs"] = preview["source_bindings"].clone();
        context.request_raw = serde_json::to_vec(&request).unwrap();
        let (created, publication, _) = execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &cut,
            &software,
            &components,
            &mut worker,
            None,
            deadline,
            &cancellation,
        )
        .unwrap();
        drop(worker);
        assert!(!publication.replayed);
        assert_eq!(created.files().len(), 5);
        let original = created.files().clone();
        let mut current_files = authored;
        for (name, raw) in created.files() {
            current_files.insert(format!("{}/{}", created.home().as_str(), name), raw.clone());
        }
        let current = successor(&current_files, &store, base);
        let current_cut = open_cut(&store, current, deadline, &cancellation);
        publish_fixture_catalog(&repository, isolated.path(), deadline);
        let mut replay_budget = ExecutorBudget::laboratory();
        replay_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!replay_budget.execution_wall.is_zero());
        let mut replay_worker = super::command_form_cases::schemas_for_profile_with_budget(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            replay_budget,
            deadline,
            &cancellation,
        );
        let mut current_budget = ExecutorBudget::laboratory();
        current_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!current_budget.execution_wall.is_zero());
        let mut current_worker = super::command_form_cases::schemas_for_profile_with_budget(
            &current_cut,
            FormatProfile::LegacyPythonObserved20260923,
            current_budget,
            deadline,
            &cancellation,
        );
        let (restored, replayed, result) = execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut replay_worker,
            Some(&mut current_worker),
            deadline,
            &cancellation,
        )
        .unwrap();
        assert!(replayed.replayed);
        assert_eq!(restored.files(), &original);
        assert_eq!(result, restored.command().response);
        assert_eq!(
            response(restored.command())["receipt"]["grants_admission"],
            false
        );
        drop(restored);
        drop(replay_worker);
        drop(current_worker);

        // Revise the profiled Claim through the same protected native owner.
        // The original catalogue companion remains bound to creation while
        // the current generated catalogue and cut advance with this command.
        let form_name = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(claim["claim_id"].as_str().unwrap().as_bytes()).to_hex()
        );
        let lock_name = format!(".{form_name}.writer.lock");
        let home = created.home().as_str();
        let lock_path = isolated.path().join(home).join(&lock_name);
        fs::write(&lock_path, b"").unwrap();
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();
        let revision_owner = serde_json::json!({
            "schema_version":format!("tos_local_identity_proposal_revision_owner_{version}"),
            "uid":uid, "principal_id":claim["maker"]["agent_ref"],
            "source_root":isolated.path(), "source_path":source_path,
            "authority_ref":"synthetic-test-only:identity-correction-no-admission",
            "expires_at":"2099-01-01T00:00:00Z", "claim_id":claim["claim_id"],
            "allowed_operations":["claim.revise"], "allowed_fields":["qualifiers"],
            "allowed_evidence_refs":claim["evidence_refs"],
            "allowed_form_ids":[format!("tos.form.synthetic-identity-{version}")],
            "allowed_form_field_ids":["claim.statement"],
            "allowed_object_values":[claim["object"]],
            "allowed_object_refs":refs.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
            "allowed_related_claim_refs":[]
        });
        let revision_owner_path = isolated.path().join("identity-revision-owner.json");
        fs::write(
            &revision_owner_path,
            source_bytes(&source_value(&revision_owner)),
        )
        .unwrap();
        fs::set_permissions(&revision_owner_path, fs::Permissions::from_mode(0o600)).unwrap();
        let revision_filesystem = CreationFilesystem::select_isolated(
            &isolated,
            &revision_owner_path,
            deadline,
            &cancellation,
        )
        .unwrap();
        let correction = serde_json::json!({
            "schema_version":"tos_local_source_command_v1", "operation":"prepare-revise",
            "fields":{"qualifiers":{"statement":format!(
                "{} [retained identity wording correction; no admission]",
                claim["qualifiers"]["statement"].as_str().unwrap()
            )}},
            "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
            "reason":"Retain an isolated identity proposal wording correction."
        });
        let mut revision_context = selected_context(
            &current_files,
            &software_files,
            &revision_owner,
            &correction,
            current,
        );
        revision_context.effective_uid = u64::from(uid);
        let mut preview_budget = ExecutorBudget::laboratory();
        preview_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!preview_budget.execution_wall.is_zero());
        let mut revision_preview_worker =
            super::command_form_cases::schemas_for_profile_with_budget(
                &current_cut,
                FormatProfile::LegacyPythonObserved20260923,
                preview_budget,
                deadline,
                &cancellation,
            );
        let revision_preview = prepare_isolated_claim_revision_from_captures(
            &revision_filesystem,
            &revision_context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut revision_preview_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        drop(revision_preview_worker);
        let prepared = response(&revision_preview);
        let mut oracle_files = current_files.clone();
        oracle_files.insert(format!("{home}/{lock_name}"), Vec::new());
        let catalog_home = isolated.path().join("ToS/source-witnesses/catalog");
        for entry in fs::read_dir(&catalog_home).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                let leaf = entry.file_name().into_string().unwrap();
                oracle_files.insert(
                    format!("ToS/source-witnesses/catalog/{leaf}"),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
        let independent =
            maintained_oracle(&oracle_files, &revision_owner, &correction, &repository);
        assert_eq!(
            prepared["expected_dependencies"],
            independent["expected_dependencies"]
        );
        assert_eq!(prepared["source_bindings"], independent["source_bindings"]);
        let mut revision_request = correction;
        revision_request["operation"] = Value::String("claim.revise".into());
        revision_request["command_id"] =
            Value::String(format!("synthetic:identity-proposal-correction-{version}"));
        revision_request["expected_configuration"] = prepared["owner_configuration"].clone();
        revision_request["expected_source"] = prepared["source"].clone();
        revision_request["expected_revision"] = prepared["revision"].clone();
        revision_request["expected_dependencies"] = prepared["expected_dependencies"].clone();
        revision_request["expected_inputs"] = prepared["source_bindings"].clone();
        revision_context.request_raw = serde_json::to_vec(&revision_request).unwrap();
        let mut revision_budget = ExecutorBudget::laboratory();
        revision_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!revision_budget.execution_wall.is_zero());
        let mut revision_worker = super::command_form_cases::schemas_for_profile_with_budget(
            &current_cut,
            FormatProfile::LegacyPythonObserved20260923,
            revision_budget,
            deadline,
            &cancellation,
        );
        let (revised, revision_publication) = execute_isolated_claim_revision_from_captures(
            &revision_filesystem,
            &revision_context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut revision_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        drop(revision_worker);
        assert!(!revision_publication.replayed);
        assert_eq!(
            revised.commit(),
            Err(SourceCommandError::MissingProductionAdmission)
        );
        let revised_result = response(&revised);
        assert_eq!(revised_result["receipt"]["grants_admission"], false);
        let archive = revised_result["receipt"]["archive_path"].as_str().unwrap();
        assert!(
            isolated
                .path()
                .join(archive)
                .join("manifest.json")
                .is_file()
        );
        let mut corrected_files = current_files;
        apply_changes(&mut corrected_files, &revised);
        for change in &revised.changes {
            if let Some(raw) = &change.after {
                assert_eq!(
                    fs::read(isolated.path().join(change.path.as_str())).unwrap(),
                    *raw
                );
            }
        }
        let corrected = successor(&corrected_files, &store, current);
        let corrected_cut = open_cut(&store, corrected, deadline, &cancellation);
        publish_fixture_catalog(&repository, isolated.path(), deadline);
        let mut original_budget = ExecutorBudget::laboratory();
        original_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!original_budget.execution_wall.is_zero());
        let mut original_worker = super::command_form_cases::schemas_for_profile_with_budget(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            original_budget,
            deadline,
            &cancellation,
        );
        let mut latest_budget = ExecutorBudget::laboratory();
        latest_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!latest_budget.execution_wall.is_zero());
        let mut latest_worker = super::command_form_cases::schemas_for_profile_with_budget(
            &corrected_cut,
            FormatProfile::LegacyPythonObserved20260923,
            latest_budget,
            deadline,
            &cancellation,
        );
        let (restored_after_revision, original_replayed, original_result) =
            execute_isolated_claim_creation_from_captures(
                &filesystem,
                &context,
                &cut,
                &corrected_cut,
                &software,
                &components,
                &mut original_worker,
                Some(&mut latest_worker),
                deadline,
                &cancellation,
            )
            .unwrap();
        assert!(original_replayed.replayed);
        assert_eq!(restored_after_revision.files(), &original);
        assert_eq!(original_result, restored_after_revision.command().response);
        assert!(fs::read(&lock_path).unwrap().is_empty());
        drop(original_worker);
        drop(latest_worker);
        let mut retry_context = selected_context(
            &corrected_files,
            &software_files,
            &revision_owner,
            &revision_request,
            corrected,
        );
        retry_context.effective_uid = u64::from(uid);
        let mut retry_budget = ExecutorBudget::laboratory();
        retry_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!retry_budget.execution_wall.is_zero());
        let mut retry_worker = super::command_form_cases::schemas_for_profile_with_budget(
            &corrected_cut,
            FormatProfile::LegacyPythonObserved20260923,
            retry_budget,
            deadline,
            &cancellation,
        );
        let (retried, retry_publication) = execute_isolated_claim_revision_from_captures(
            &revision_filesystem,
            &retry_context,
            &cut,
            &corrected_cut,
            &software,
            &components,
            &mut retry_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        assert!(retry_publication.replayed);
        assert!(retried.changes.is_empty());
        assert_eq!(response(&retried)["receipt"], revised_result["receipt"]);
    }
}

#[test]
fn initial_collection_order_binds_retained_version_and_cold_replays() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let (mut files, _, _, _) = claim_fixture();
    let collection = "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996";
    for leaf in [
        "collection.json",
        "collection.human-forms.json",
        "membership-claims.jsonl",
        "responsibility-claims.jsonl",
        "source-revision-history.json",
        "structure/work-boundaries/work-boundary-map.json",
        "structure/work-boundaries/anchors.jsonl",
    ] {
        let path = format!("{collection}/{leaf}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    let history: Value =
        serde_json::from_slice(&files[&format!("{collection}/source-revision-history.json")])
            .unwrap();
    for receipt in history["receipts"].as_array().unwrap() {
        let path = receipt["archive_path"].as_str().unwrap();
        let mut members = fs::read_dir(repository.join(path))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        members.sort();
        assert!(members.len() <= 64);
        for member in members {
            let relative = member.strip_prefix(&repository).unwrap().to_str().unwrap();
            files.insert(relative.into(), fs::read(&member).unwrap());
        }
    }
    for work in [
        "also-sprach-zarathustra",
        "jenseits-von-gut-und-boese",
        "zur-genealogie-der-moral",
        "der-fall-wagner",
        "goetzen-daemmerung",
        "der-antichrist",
        "ecce-homo",
    ] {
        let path = format!("ToS/source-witnesses/works/friedrich-nietzsche/{work}/work.json");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    let source_claim =
        "ToS/source-witnesses/relations/mysl-1996-volume-2-member-order/source-claims.jsonl";
    let mut claim: Value =
        serde_json::from_slice(&fs::read(repository.join(source_claim)).unwrap()).unwrap();
    claim["claim_id"] = Value::String("tos.claim.synthetic-collection-order".into());
    claim["claim_version"] = Value::from(1);
    claim["provenance_event_ref"] = Value::String("tos.event.synthetic-collection-order".into());
    for evidence in claim["evidence_refs"].as_array().unwrap() {
        let path = evidence.as_str().unwrap();
        if !files.contains_key(path) {
            files.insert(path.into(), fs::read(repository.join(path)).unwrap());
        }
    }
    let source_path = "ToS/source-witnesses/relations/command-collection-order/source-claims.jsonl";
    let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
        .iter()
        .chain(CLAIM_REVISION_RULE_INPUTS)
        .copied()
        .filter(|name| !name.starts_with("ToS/"))
        .collect::<Vec<_>>();
    software_names.extend([
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
    ]);
    software_names.sort_unstable();
    software_names.dedup();
    for name in software_names {
        files.insert(name.into(), fs::read(repository.join(name)).unwrap());
    }
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    let (_capture, software, components) =
        super::command_record_cases::captured_components(&files, deadline, &cancellation);
    let temporary = tempfile::tempdir().unwrap();
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let store = temporary.path().join("selected-store");
    let base = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = open_cut(&store, base, deadline, &cancellation);
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
    for (name, raw) in &files {
        let target = isolated.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    fs::create_dir_all(
        isolated
            .path()
            .join(source_path)
            .parent()
            .unwrap()
            .parent()
            .unwrap(),
    )
    .unwrap();
    publish_fixture_catalog(&repository, isolated.path(), deadline);
    let uid = fs::metadata(isolated.path()).unwrap().uid();
    let configuration = serde_json::json!({
        "schema_version":"tos_local_claim_create_owner_v4", "uid":uid,
        "principal_id":claim["maker"]["agent_ref"],
        "maker_type":claim["maker"]["maker_type"],
        "source_root":isolated.path(), "source_path":source_path,
        "authority_ref":"synthetic-test-only:collection-order-no-membership-grant",
        "expires_at":"2099-01-01T00:00:00Z",
        "provenance_event_id":claim["provenance_event_ref"],
        "allowed_operations":["claims.create"], "allowed_claim_ids":[claim["claim_id"]],
        "allowed_subject_refs":[claim["subject_ref"]], "allowed_object_refs":[],
        "allowed_object_values":[claim["object"]],
        "allowed_predicates":[claim["predicate"]],
        "allowed_evidence_refs":claim["evidence_refs"]
    });
    let owner = isolated.path().join("order-owner.json");
    fs::write(&owner, source_bytes(&source_value(&configuration))).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let filesystem =
        CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation).unwrap();
    let software_files = files
        .iter()
        .filter(|(name, _)| !name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let preview_request = serde_json::json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-create", "claims":[claim]
    });
    let mut context = selected_context(
        &authored,
        &software_files,
        &configuration,
        &preview_request,
        base,
    );
    context.effective_uid = u64::from(uid);
    let archive_manifest = format!(
        "{}/manifest.json",
        history["receipts"][0]["archive_path"].as_str().unwrap()
    );
    let mut broken_authored = authored.clone();
    assert!(
        broken_authored
            .insert(archive_manifest, b"{}\n".to_vec())
            .is_some()
    );
    let broken_store = temporary.path().join("broken-history-store");
    let broken_revision =
        super::validation_cut_cases::write_cut_store(&broken_authored, &broken_store);
    let broken_cut = open_cut(&broken_store, broken_revision, deadline, &cancellation);
    let mut broken_context = selected_context(
        &broken_authored,
        &software_files,
        &configuration,
        &preview_request,
        broken_revision,
    );
    broken_context.effective_uid = u64::from(uid);
    let mut broken_budget = ExecutorBudget::laboratory();
    broken_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!broken_budget.execution_wall.is_zero());
    let mut broken_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &broken_cut,
        FormatProfile::LegacyPythonObserved20260923,
        broken_budget,
        deadline,
        &cancellation,
    );
    assert!(matches!(
        prepare_isolated_claim_creation_from_captures(
            &filesystem,
            &broken_context,
            &broken_cut,
            &software,
            &components,
            &mut broken_worker,
            deadline,
            &cancellation
        ),
        Err(SourceCommandError::Invalid(_) | SourceCommandError::Conflict(_))
    ));
    drop(broken_worker);
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!budget.execution_wall.is_zero());
    let mut worker = super::command_form_cases::schemas_for_profile_with_budget(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        budget,
        deadline,
        &cancellation,
    );
    let preview = response(
        &prepare_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancellation,
        )
        .unwrap(),
    );
    assert_eq!(preview["grants_admission"], false);
    let binding = &preview["source_bindings"]["collection_orders"];
    assert_eq!(binding.as_object().unwrap().len(), 1);
    assert_eq!(
        binding["tos.claim.synthetic-collection-order"]["establishes_membership"],
        false
    );
    assert_eq!(
        binding["tos.claim.synthetic-collection-order"]["collection"]["version_status"],
        "historical"
    );
    let mut request = preview_request;
    request["operation"] = Value::String("claims.create".into());
    request["command_id"] = Value::String("synthetic:collection-order-create".into());
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_revision"] = Value::Null;
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    request["expected_inputs"] = preview["source_bindings"].clone();
    context.request_raw = serde_json::to_vec(&request).unwrap();
    let catalog_path = isolated
        .path()
        .join("ToS/source-witnesses/catalog/collections.jsonl");
    let catalog_raw = fs::read(&catalog_path).unwrap();
    fs::write(&catalog_path, b"{}\n").unwrap();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &cut,
            &software,
            &components,
            &mut worker,
            None,
            deadline,
            &cancellation
        ),
        Err(SourceCommandError::Conflict(_) | SourceCommandError::Invalid(_))
    ));
    assert!(!isolated.path().join(source_path).parent().unwrap().exists());
    fs::write(&catalog_path, catalog_raw).unwrap();
    drop(worker);
    let mut create_budget = ExecutorBudget::laboratory();
    create_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!create_budget.execution_wall.is_zero());
    let mut worker = super::command_form_cases::schemas_for_profile_with_budget(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        create_budget,
        deadline,
        &cancellation,
    );
    let (created, publication, _) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &cut,
        &software,
        &components,
        &mut worker,
        None,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(worker);
    assert!(!publication.replayed);
    let original = created.files().clone();
    let mut current_files = authored;
    for (name, raw) in created.files() {
        current_files.insert(format!("{}/{}", created.home().as_str(), name), raw.clone());
    }
    let current = successor(&current_files, &store, base);
    let current_cut = open_cut(&store, current, deadline, &cancellation);
    publish_fixture_catalog(&repository, isolated.path(), deadline);
    let mut original_budget = ExecutorBudget::laboratory();
    original_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!original_budget.execution_wall.is_zero());
    let mut original_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        original_budget,
        deadline,
        &cancellation,
    );
    let mut current_budget = ExecutorBudget::laboratory();
    current_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!current_budget.execution_wall.is_zero());
    let mut current_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        current_budget,
        deadline,
        &cancellation,
    );
    let (restored, replayed, result) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut original_worker,
        Some(&mut current_worker),
        deadline,
        &cancellation,
    )
    .unwrap();
    assert!(replayed.replayed);
    assert_eq!(restored.files(), &original);
    assert_eq!(result, restored.command().response);
    assert_eq!(
        response(restored.command())["receipt"]["grants_admission"],
        false
    );
    drop(restored);
    drop(original_worker);
    drop(current_worker);

    // The historical Collection basis remains fixed while the attributed
    // order Claim itself receives one native source-copy correction.
    let form_name = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim["claim_id"].as_str().unwrap().as_bytes()).to_hex()
    );
    let lock_name = format!(".{form_name}.writer.lock");
    let home = created.home().as_str();
    let lock_path = isolated.path().join(home).join(&lock_name);
    fs::write(&lock_path, b"").unwrap();
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();
    let revision_owner = serde_json::json!({
        "schema_version":"tos_local_claim_revision_owner_v1", "uid":uid,
        "principal_id":claim["maker"]["agent_ref"],
        "source_root":isolated.path(), "source_path":source_path,
        "authority_ref":"synthetic-test-only:collection-order-correction-no-membership-grant",
        "expires_at":"2099-01-01T00:00:00Z", "claim_id":claim["claim_id"],
        "allowed_operations":["claim.revise"], "allowed_fields":["qualifiers"],
        "allowed_evidence_refs":claim["evidence_refs"],
        "allowed_form_ids":["tos.form.synthetic-collection-order"],
        "allowed_form_field_ids":["claim.statement"]
    });
    let revision_owner_path = isolated.path().join("order-revision-owner.json");
    fs::write(
        &revision_owner_path,
        source_bytes(&source_value(&revision_owner)),
    )
    .unwrap();
    fs::set_permissions(&revision_owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let revision_filesystem = CreationFilesystem::select_isolated(
        &isolated,
        &revision_owner_path,
        deadline,
        &cancellation,
    )
    .unwrap();
    let correction = serde_json::json!({
        "schema_version":"tos_local_source_command_v1", "operation":"prepare-revise",
        "fields":{"qualifiers":{"statement":format!(
            "{} [retained order wording correction; no membership grant]",
            claim["qualifiers"]["statement"].as_str().unwrap()
        )}},
        "forms":[{"form_id":"tos.form.synthetic-collection-order","field_id":"claim.statement"}],
        "reason":"Retain an isolated attributed order wording correction."
    });
    let mut revision_context = selected_context(
        &current_files,
        &software_files,
        &revision_owner,
        &correction,
        current,
    );
    revision_context.effective_uid = u64::from(uid);
    let mut preview_budget = ExecutorBudget::laboratory();
    preview_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!preview_budget.execution_wall.is_zero());
    let mut revision_preview_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        preview_budget,
        deadline,
        &cancellation,
    );
    let revision_preview = prepare_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_preview_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_preview_worker);
    let prepared = response(&revision_preview);
    assert_eq!(
        prepared["source_bindings"]["collection_orders"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    let mut oracle_files = current_files.clone();
    oracle_files.insert(format!("{home}/{lock_name}"), Vec::new());
    for entry in fs::read_dir(isolated.path().join("ToS/source-witnesses/catalog")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let leaf = entry.file_name().into_string().unwrap();
            oracle_files.insert(
                format!("ToS/source-witnesses/catalog/{leaf}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    let independent = maintained_oracle(&oracle_files, &revision_owner, &correction, &repository);
    assert_eq!(
        prepared["expected_dependencies"],
        independent["expected_dependencies"]
    );
    assert_eq!(prepared["source_bindings"], independent["source_bindings"]);
    let mut revision_request = correction;
    revision_request["operation"] = Value::String("claim.revise".into());
    revision_request["command_id"] = Value::String("synthetic:collection-order-correction".into());
    revision_request["expected_configuration"] = prepared["owner_configuration"].clone();
    revision_request["expected_source"] = prepared["source"].clone();
    revision_request["expected_revision"] = prepared["revision"].clone();
    revision_request["expected_dependencies"] = prepared["expected_dependencies"].clone();
    revision_request["expected_inputs"] = prepared["source_bindings"].clone();
    revision_context.request_raw = serde_json::to_vec(&revision_request).unwrap();
    let mut revision_budget = ExecutorBudget::laboratory();
    revision_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!revision_budget.execution_wall.is_zero());
    let mut revision_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        revision_budget,
        deadline,
        &cancellation,
    );
    let (revised, revision_publication) = execute_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_worker);
    assert!(!revision_publication.replayed);
    assert_eq!(
        revised.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    let revised_result = response(&revised);
    assert_eq!(revised_result["receipt"]["grants_admission"], false);
    let archive = revised_result["receipt"]["archive_path"].as_str().unwrap();
    assert!(
        isolated
            .path()
            .join(archive)
            .join("manifest.json")
            .is_file()
    );
    let mut corrected_files = current_files;
    apply_changes(&mut corrected_files, &revised);
    for change in &revised.changes {
        if let Some(raw) = &change.after {
            assert_eq!(
                fs::read(isolated.path().join(change.path.as_str())).unwrap(),
                *raw
            );
        }
    }
    let corrected = successor(&corrected_files, &store, current);
    let corrected_cut = open_cut(&store, corrected, deadline, &cancellation);
    publish_fixture_catalog(&repository, isolated.path(), deadline);
    let mut original_budget = ExecutorBudget::laboratory();
    original_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!original_budget.execution_wall.is_zero());
    let mut original_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        original_budget,
        deadline,
        &cancellation,
    );
    let mut latest_budget = ExecutorBudget::laboratory();
    latest_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!latest_budget.execution_wall.is_zero());
    let mut latest_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &corrected_cut,
        FormatProfile::LegacyPythonObserved20260923,
        latest_budget,
        deadline,
        &cancellation,
    );
    let (restored_after_revision, original_replayed, original_result) =
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &corrected_cut,
            &software,
            &components,
            &mut original_worker,
            Some(&mut latest_worker),
            deadline,
            &cancellation,
        )
        .unwrap();
    assert!(original_replayed.replayed);
    assert_eq!(restored_after_revision.files(), &original);
    assert_eq!(original_result, restored_after_revision.command().response);
    assert!(fs::read(&lock_path).unwrap().is_empty());
    drop(original_worker);
    drop(latest_worker);
    let mut retry_context = selected_context(
        &corrected_files,
        &software_files,
        &revision_owner,
        &revision_request,
        corrected,
    );
    retry_context.effective_uid = u64::from(uid);
    let mut retry_budget = ExecutorBudget::laboratory();
    retry_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!retry_budget.execution_wall.is_zero());
    let mut retry_worker = super::command_form_cases::schemas_for_profile_with_budget(
        &corrected_cut,
        FormatProfile::LegacyPythonObserved20260923,
        retry_budget,
        deadline,
        &cancellation,
    );
    let (retried, retry_publication) = execute_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &retry_context,
        &cut,
        &corrected_cut,
        &software,
        &components,
        &mut retry_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    assert!(retry_publication.replayed);
    assert!(retried.changes.is_empty());
    assert_eq!(response(&retried)["receipt"], revised_result["receipt"]);
}
