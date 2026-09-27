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
    execute_isolated_claim_creation_from_captures, run_claim_command,
    run_claim_command_from_captures,
};
use tos_command::source_command::{PreparedCommand, SourceCommandError};
use tos_command::source_forms::metadata_subject;
use tos_command::source_operation::{SourceOperationError, bind_selected_candidate};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::operation::OperationLimits;

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
    let (mut files, mut claim, _, source_path) = claim_fixture();
    claim["claim_version"] = Value::from(1);
    let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
        .iter()
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
    let mut worker = schemas(&cut, deadline, &cancellation);
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
        oracle_status.success(),
        "{}",
        String::from_utf8_lossy(&fs::read(&oracle_stderr).unwrap())
    );
    assert!(fs::metadata(&oracle_stdout).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(&oracle_stderr).unwrap().len() <= 1_048_576);
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
    let mut current_files = authored.clone();
    for (name, raw) in created.files() {
        current_files.insert(format!("{}/{name}", created.home().as_str()), raw.clone());
    }
    let current = successor(&current_files, &store, base);
    let current_cut = open_cut(&store, current, deadline, &cancellation);
    drop(created);
    let mut replay_worker = schemas(&cut, deadline, &cancellation);
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
    assert!(replayed.replayed);
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
    let evidence_path = isolated
        .path()
        .join(claim["evidence_refs"][0].as_str().unwrap());
    let evidence_original = fs::read(&evidence_path).unwrap();
    fs::write(&evidence_path, b"Changed after the selected Claim cut.\n").unwrap();
    let mut stale_source_worker = schemas(&cut, deadline, &cancellation);
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
    fs::write(&evidence_path, evidence_original).unwrap();
    let prior = fs::read(&owner).unwrap();
    let mut revoked = configuration;
    revoked["allowed_operations"] = serde_json::json!([]);
    fs::write(&owner, source_bytes(&source_value(&revoked))).unwrap();
    let mut revoked_worker = schemas(&cut, deadline, &cancellation);
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
    fs::write(&owner, prior).unwrap();
}
