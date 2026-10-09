//! One isolated Work→Expression owner path. A checked-in source fixture seeds
//! the exact Work and Claim proposal; Rust owns fixture preparation, mutation,
//! catalog projection and retained-transaction observation.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_command::SourceCommandError;
use tos_command::source_creation_store::{
    CreationFilesystem, IsolatedCreationRoot, execute_isolated_work_expression_from_captures,
    prepare_isolated_work_expression_from_proposal, replay_isolated_work_expression_from_captures,
};
use tos_validation::FormatProfile;
use tos_validation::executor::ExecutorBudget;
use tos_validation::item_rules::ItemLimits;

const WORK_IMPLEMENTATIONS: [&str; 13] = [
    "rust/crates/tos-command/src/source_assessment_journal.rs",
    "rust/crates/tos-command/src/source_command.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "rust/crates/tos-command/src/source_native_cli.rs",
    "rust/crates/tos-command/src/source_private_assessment_sources.rs",
    "rust/crates/tos-command/src/source_revisions.rs",
    "rust/crates/tos-command/src/source_work_expression.rs",
    "rust/crates/tos-command/src/source_work_transaction.rs",
    "rust/crates/tos-compiler/src/source_witness_catalog.rs",
    "scripts/source_bibliographic_topology.py",
    "scripts/source_metadata_snapshot.py",
    "scripts/source_record_profiles.py",
    "scripts/source_witness_human_forms.py",
];

fn seed_native_work_fixture(repository: &Path, root: &Path) -> (PathBuf, Vec<u8>, Value) {
    let seed = repository.join(
        "rust/crates/tos-command/tests/fixtures/source-native-python-v1/fixtures/work-expression",
    );
    let files = authored_work_files(&seed);
    assert!(!files.is_empty(), "checked-in Work fixture source is empty");
    for (reference, raw) in files {
        let target = root.join(reference);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, raw).unwrap();
    }

    let mut configuration: Value =
        serde_json::from_slice(&fs::read(seed.join("compound-owner.json")).unwrap()).unwrap();
    configuration["uid"] = json!(fs::metadata(root).unwrap().uid());
    configuration["source_root"] = json!(root.canonicalize().unwrap().to_string_lossy());
    let owner = root.join("compound-owner.json");
    fs::write(&owner, serde_json::to_vec(&configuration).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let owner_raw = fs::read(&owner).unwrap();
    (owner, owner_raw, configuration)
}

fn sibling_work_configuration(configuration: &Value, suffix: &str) -> Value {
    let mut sibling = configuration.clone();
    let work_home = Path::new(configuration["work_source_path"].as_str().unwrap())
        .parent()
        .unwrap();
    let expression_path = work_home
        .join("expressions")
        .join(suffix)
        .join("expression.json");
    sibling["expression_id"] = json!(format!("tos.expression.synthetic.{suffix}"));
    sibling["expression_source_path"] = json!(expression_path.to_str().unwrap());
    sibling["claim_id"] = json!(format!("tos.claim.synthetic.{suffix}"));
    sibling["provenance_event_id"] = json!(format!("tos.event.synthetic.{suffix}"));
    sibling["allowed_expression_form_ids"] = json!([format!("tos.form.synthetic.{suffix}.name")]);
    sibling["allowed_claim_form_ids"] = json!([format!("tos.form.synthetic.{suffix}.statement")]);
    sibling
}

fn work_expression_proposal(work: &Value, configuration: &Value) -> Value {
    let work_ref = configuration["work_source_path"].as_str().unwrap();
    let expression_ref = configuration["expression_source_path"].as_str().unwrap();
    let mut expression = work.clone();
    expression["record_id"] = configuration["expression_id"].clone();
    expression["record_type"] = json!("expression");
    expression["record_version"] = json!(1);
    expression["identity_status"] = json!("provisional");
    expression["preferred_label"] = json!("Синтетическое английское выражение");
    expression["work_ref"] = work["record_id"].clone();
    expression["language"] = json!("en");
    expression["expression_role"] = json!("translation");
    expression["responsibility_claim_refs"] = json!([]);
    expression["embodiment_claim_refs"] = json!([]);
    expression["derivation_claim_refs"] = json!([]);
    expression
        .as_object_mut()
        .unwrap()
        .remove("expression_claim_refs");
    expression
        .as_object_mut()
        .unwrap()
        .remove("chronology_claim_refs");

    json!({
        "schema_version": "tos_local_work_expression_command_v1",
        "operation": "prepare-create",
        "record": expression,
        "claim": {
            "schema_version": "tos_source_relation_claim_v1",
            "claim_id": configuration["claim_id"],
            "claim_type": "relation",
            "claim_version": 1,
            "assertion_layer": "bibliographic_assertion",
            "predicate": "has_expression",
            "subject_ref": work["record_id"],
            "object": configuration["expression_id"],
            "epistemic_status": "observed",
            "polarity": "positive",
            "review_status": "unreviewed",
            "visibility": "public_metadata_only",
            "maker": {"maker_type": "model", "agent_ref": "model:synthetic"},
            "provenance_event_ref": configuration["provenance_event_id"],
            "evidence_refs": [work_ref, expression_ref],
            "assessment_refs": [],
            "qualifiers": {
                "statement": "The synthetic records declare this link, without textual equivalence.",
                "statement_language": "en",
                "statement_script": "Latn",
                "unknown_qualification": {
                    "flag": false,
                    "missing": null,
                    "text": "Keep this context."
                }
            }
        },
        "forms": [
            {"form_id": configuration["allowed_work_form_ids"][0], "field_id": "metadata.preferred-name"},
            {"form_id": configuration["allowed_work_form_ids"][1], "field_id": "metadata.source-note"}
        ],
        "expression_forms": [{
            "form_id": configuration["allowed_expression_form_ids"][0],
            "field_id": "metadata.preferred-name"
        }],
        "claim_forms": [{
            "form_id": configuration["allowed_claim_form_ids"][0],
            "field_id": "claim.statement"
        }],
        "reason": "Synthetic exact typed child addition; no admission."
    })
}

pub(super) fn authored_work_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut directories = vec![root.join("ToS")];
    let mut files = BTreeMap::new();
    let mut bytes = 0usize;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "synthetic Work member is a link");
            if kind.is_dir() {
                directories.push(entry.path());
            } else {
                assert!(kind.is_file());
                let reference = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                // This exact historical-create sidecar is live lock state,
                // not a portable authored member of the selected cut.
                if reference != "ToS/source-witnesses/.historical-create.writer.lock"
                    && tos_source_store::is_authored_source_path_v1(&reference)
                {
                    let raw = fs::read(entry.path()).unwrap();
                    assert!(raw.len() <= 8_388_608);
                    bytes = bytes.checked_add(raw.len()).unwrap();
                    assert!(bytes <= 33_554_432);
                    assert!(files.insert(reference, raw).is_none());
                    assert!(files.len() <= 4096);
                }
            }
        }
    }
    files
}

fn work_worker(
    cut: &tos_source_store::CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_validation::source_cut::CutWorkerSchemaExecutor {
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!budget.execution_wall.is_zero());
    super::command_form_cases::schemas_for_profile_with_budget(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        budget,
        deadline,
        cancelled,
    )
}

#[test]
fn native_work_expression_publishes_replays_and_prepares_next_sibling() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    // The protected writer publishes the first child. Its committed native
    // lineage is then consumed by a fresh preview and a second publication.
    {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let (owner, owner_raw, configuration) =
            seed_native_work_fixture(&repository, isolated.path());
        super::compiler_source_cases::publish_native_catalog_fixture(
            &repository,
            isolated.path(),
            deadline,
        );
        let work_ref = configuration["work_source_path"].as_str().unwrap();
        let expression_ref = configuration["expression_source_path"]
            .as_str()
            .unwrap()
            .to_owned();
        let base_work: Value =
            serde_json::from_slice(&fs::read(isolated.path().join(work_ref)).unwrap()).unwrap();
        let first_proposal = work_expression_proposal(&base_work, &configuration);
        let mut files = authored_work_files(isolated.path());
        let authored = files.clone();
        assert!(authored.contains_key(work_ref));
        for reference in WORK_IMPLEMENTATIONS {
            let raw = fs::read(repository.join(reference)).unwrap();
            let target = isolated.path().join(reference);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, &raw).unwrap();
            assert!(files.insert(reference.to_owned(), raw).is_none());
        }
        let (_capture, software, components) =
            super::command_record_cases::captured_components(&files, deadline, &cancelled);
        let store = temporary.path().join("original-cut");
        let original = super::validation_cut_cases::write_cut_store(&authored, &store);
        let original_cut =
            super::command_form_cases::open_cut(&store, original, deadline, &cancelled);
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled).unwrap();
        let limits = ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline,
        };
        let proposal_raw = serde_json::to_vec(&first_proposal).unwrap();
        let mut proposal_context =
            super::command_form_cases::context(&files, owner_raw.clone(), proposal_raw, original);
        proposal_context.effective_uid = fs::metadata(isolated.path()).unwrap().uid().into();
        let mut proposal_worker = work_worker(&original_cut, deadline, &cancelled);
        let first_prepared = prepare_isolated_work_expression_from_proposal(
            &filesystem,
            &proposal_context,
            &original_cut,
            &software,
            &components,
            &mut proposal_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(proposal_worker);
        let mut first_request: Value = serde_json::from_slice(
            &tos_foundation::canonical_bytes_v1(
                first_prepared.request(),
                tos_foundation::CanonicalProfile::SourceCommandInputV1,
                tos_foundation::JsonLimits::default(),
            )
            .unwrap(),
        )
        .unwrap();
        first_request["command_id"] = configuration["expression_id"].clone();
        let first_request_raw = serde_json::to_vec(&first_request).unwrap();
        let mut context =
            super::command_form_cases::context(&files, owner_raw, first_request_raw, original);
        context.effective_uid = fs::metadata(isolated.path()).unwrap().uid().into();
        let control = isolated
            .path()
            .join("ToS/source-witnesses/.metadata-publication.json");
        let mut worker = work_worker(&original_cut, deadline, &cancelled);
        let created_first = execute_isolated_work_expression_from_captures(
            &filesystem,
            &context,
            &original_cut,
            &software,
            &components,
            &mut worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(worker);
        assert!(!created_first.replayed());
        assert_ne!(
            fs::read(isolated.path().join(work_ref)).unwrap(),
            authored[work_ref]
        );
        assert!(isolated.path().join(&expression_ref).exists());
        assert_eq!(
            created_first.receipt().object_get("grants_admission"),
            Some(&JsonValue::Bool(false))
        );
        let current_files = authored_work_files(isolated.path());
        let current_store = temporary.path().join("current-cut");
        let current = super::validation_cut_cases::write_cut_store(&current_files, &current_store);
        let current_cut =
            super::command_form_cases::open_cut(&current_store, current, deadline, &cancelled);
        let mut replay_worker = work_worker(&current_cut, deadline, &cancelled);
        let replay = replay_isolated_work_expression_from_captures(
            &filesystem,
            &context,
            &original_cut,
            &current_cut,
            &software,
            &components,
            &mut replay_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(replay_worker);
        assert!(replay.replayed());
        assert_eq!(replay.transaction_id(), created_first.transaction_id());
        assert_eq!(replay.manifest_sha256(), created_first.manifest_sha256());
        assert_eq!(replay.receipt(), created_first.receipt());
        let committed_control = fs::read(&control).unwrap();
        // Changed required implementation bytes cannot pass a retained
        // current-source/software fence, even when the original cut survives.
        let implementation = WORK_IMPLEMENTATIONS[0];
        let implementation_path = isolated.path().join(implementation);
        let original_implementation = fs::read(&implementation_path).unwrap();
        fs::write(
            &implementation_path,
            [original_implementation.as_slice(), b"\n"].concat(),
        )
        .unwrap();
        let mut changed_worker = work_worker(&current_cut, deadline, &cancelled);
        assert!(matches!(
            replay_isolated_work_expression_from_captures(
                &filesystem,
                &context,
                &original_cut,
                &current_cut,
                &software,
                &components,
                &mut changed_worker,
                limits,
                &cancelled,
            ),
            Err(SourceCommandError::Conflict(_))
        ));
        drop(changed_worker);
        fs::write(&implementation_path, original_implementation).unwrap();
        // A mixed native environment in a newly selected complete cut is
        // still refused; decoded JSON equivalence or a reused receipt cannot
        // authorize a different retained byte stream.
        let expression_home = Path::new(&expression_ref).parent().unwrap();
        let environment_ref = expression_home.join("source-create-environment.json");
        let environment_path = isolated.path().join(&environment_ref);
        let environment_original = fs::read(&environment_path).unwrap();
        let mut environment: Value = serde_json::from_slice(&environment_original).unwrap();
        environment["backend"] = Value::String("mixed Python/native runtime".into());
        let mut altered_raw = serde_json::to_vec(&environment).unwrap();
        altered_raw.push(b'\n');
        fs::write(&environment_path, &altered_raw).unwrap();
        let mut altered_files = current_files.clone();
        altered_files.insert(environment_ref.to_str().unwrap().to_owned(), altered_raw);
        let altered_store = temporary.path().join("altered-cut");
        let altered = super::validation_cut_cases::write_cut_store(&altered_files, &altered_store);
        let altered_cut =
            super::command_form_cases::open_cut(&altered_store, altered, deadline, &cancelled);
        let mut mixed_worker = work_worker(&altered_cut, deadline, &cancelled);
        assert!(matches!(
            replay_isolated_work_expression_from_captures(
                &filesystem,
                &context,
                &original_cut,
                &altered_cut,
                &software,
                &components,
                &mut mixed_worker,
                limits,
                &cancelled,
            ),
            Err(SourceCommandError::Conflict(_)) | Err(SourceCommandError::SchemaExecution { .. })
        ));
        drop(mixed_worker);
        fs::write(&environment_path, environment_original).unwrap();
        assert_eq!(
            fs::read(isolated.path().join(&expression_ref)).unwrap(),
            current_files[&expression_ref]
        );
        assert_eq!(fs::read(&control).unwrap(), committed_control);

        // Refresh only this fixture's derived catalogue with the existing native
        // compiler projector after the first Rust publication.
        super::compiler_source_cases::publish_native_catalog_fixture(
            &repository,
            isolated.path(),
            deadline,
        );
        let second_config = sibling_work_configuration(&configuration, "second");
        fs::write(&owner, serde_json::to_vec(&second_config).unwrap()).unwrap();
        let second_files = authored_work_files(isolated.path());
        let second_store = temporary.path().join("second-cut");
        let second_revision =
            super::validation_cut_cases::write_cut_store(&second_files, &second_store);
        let second_cut = super::command_form_cases::open_cut(
            &second_store,
            second_revision,
            deadline,
            &cancelled,
        );
        let mut second_inputs = second_files.clone();
        for reference in WORK_IMPLEMENTATIONS {
            assert!(
                second_inputs
                    .insert(
                        reference.into(),
                        fs::read(isolated.path().join(reference)).unwrap()
                    )
                    .is_none()
            );
        }
        let second_fs =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled).unwrap();
        let mut proposal_ctx = super::command_form_cases::context(
            &second_inputs,
            canonical_json(&second_config),
            serde_json::to_vec(&work_expression_proposal(&base_work, &second_config)).unwrap(),
            second_revision,
        );
        proposal_ctx.effective_uid = fs::metadata(isolated.path()).unwrap().uid().into();
        let mut prepare_worker = work_worker(&second_cut, deadline, &cancelled);
        let prepared = prepare_isolated_work_expression_from_proposal(
            &second_fs,
            &proposal_ctx,
            &second_cut,
            &software,
            &components,
            &mut prepare_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(prepare_worker);
        assert!(!prepared.projected_outputs().is_empty());
        assert_eq!(
            prepared
                .request()
                .object_get("fields")
                .unwrap()
                .object_get("expression_claim_refs")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let preview_raw = tos_foundation::canonical_bytes_v1(
            prepared.request(),
            tos_foundation::CanonicalProfile::SourceCommandInputV1,
            tos_foundation::JsonLimits::default(),
        )
        .unwrap();
        let mut second_request: Value = serde_json::from_slice(&preview_raw).unwrap();
        assert_eq!(second_request["command_id"], "preview:uncommitted");
        // The preview identifier is not a durable command identity. The
        // maintained caller supplies its delegated child ID for publication.
        second_request["command_id"] = second_config["expression_id"].clone();
        let request_raw = serde_json::to_vec(&second_request).unwrap();
        let mut create_ctx = super::command_form_cases::context(
            &second_inputs,
            canonical_json(&second_config),
            request_raw,
            second_revision,
        );
        create_ctx.effective_uid = fs::metadata(isolated.path()).unwrap().uid().into();
        let mut second_worker = work_worker(&second_cut, deadline, &cancelled);
        let created = execute_isolated_work_expression_from_captures(
            &second_fs,
            &create_ctx,
            &second_cut,
            &software,
            &components,
            &mut second_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(second_worker);
        assert!(!created.replayed());
        let final_files = authored_work_files(isolated.path());
        let final_store = temporary.path().join("final-cut");
        let final_revision =
            super::validation_cut_cases::write_cut_store(&final_files, &final_store);
        let final_cut =
            super::command_form_cases::open_cut(&final_store, final_revision, deadline, &cancelled);
        let mut final_worker = work_worker(&final_cut, deadline, &cancelled);
        let second_replay = replay_isolated_work_expression_from_captures(
            &second_fs,
            &create_ctx,
            &second_cut,
            &final_cut,
            &software,
            &components,
            &mut final_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(final_worker);
        assert!(second_replay.replayed());
        assert_eq!(second_replay.transaction_id(), created.transaction_id());
        assert_eq!(second_replay.receipt(), created.receipt());
        let final_work: Value =
            serde_json::from_slice(&fs::read(isolated.path().join(work_ref)).unwrap()).unwrap();
        assert_eq!(final_work["record_version"], 6);
    }
}
